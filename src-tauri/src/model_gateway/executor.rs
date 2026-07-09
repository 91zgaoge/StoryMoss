//! Model Gateway — 执行层与 fallback
//!
//! v0.14.0: 按候选链顺序执行，主模型失败时自动降级。

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager, Runtime, Wry};

use super::{
    dispatcher::TaskClassifier,
    health::{HealthRegistry, ProbeEngine},
    registry::GatewayRegistry,
    types::{GatewayRequest, GatewayRoutingDecision, ProbeResult},
};
use crate::{
    db::DbPool,
    error::AppError,
    error_recovery::retry_with_backoff,
    llm::{adapter::GenerateRequest, service::LlmService, GenerateResponse as LlmGenerateResponse},
};

/// v0.23.59: 活跃模型连续失败降级阈值。
///
/// 连续失败达到此值后，活跃模型不再被 `+1000` 强制置顶，
/// 让其他健康候选接管。成功 1 次即清零恢复。
/// 设为 2：容忍 1 次偶发失败（网络抖动/单次超时），2 次连续失败才降级。
const ACTIVE_MODEL_DEMOTION_THRESHOLD: u32 = 2;

/// 网关执行器
pub struct GatewayExecutor<R: Runtime = Wry> {
    app_handle: AppHandle<R>,
    pub registry: Arc<Mutex<GatewayRegistry>>,
    classifier: TaskClassifier,
    probe_engine: ProbeEngine,
    llm_service: LlmService<R>,
    pool: DbPool,
}

impl<R: Runtime> Clone for GatewayExecutor<R> {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            registry: self.registry.clone(),
            classifier: self.classifier.clone(),
            probe_engine: self.probe_engine.clone(),
            llm_service: self.llm_service.clone(),
            pool: self.pool.clone(),
        }
    }
}

impl<R: Runtime> GatewayExecutor<R> {
    /// Issue #4: `pool` must be passed explicitly — never `state::<DbPool>()`
    /// here. When `init_db` fails, setup skips `manage(pool)` and must not
    /// construct this type.
    pub fn new(
        app_handle: AppHandle<R>,
        registry: GatewayRegistry,
        llm_service: LlmService<R>,
        pool: DbPool,
    ) -> Self {
        Self {
            app_handle,
            registry: Arc::new(Mutex::new(registry)),
            classifier: TaskClassifier::new(),
            probe_engine: ProbeEngine::new(),
            llm_service,
            pool,
        }
    }

    pub fn health_registry(&self) -> Arc<Mutex<HealthRegistry>> {
        self.probe_engine.registry()
    }

    fn registry_guard(&self) -> std::sync::MutexGuard<'_, GatewayRegistry> {
        // v0.26.19 Phase 2.3: 中毒锁恢复——任一持有 registry 锁的线程 panic 后，
        //   原 `.expect("registry lock poisoned")` 会让网关路由全部 panic。
        //   改用 `into_inner()` 取回内部数据，避免级联 panic；数据可能不一致
        //   但允许上层逻辑继续（健康探测/候选 fallback 仍可工作）。
        self.registry.lock().unwrap_or_else(|e| e.into_inner())
    }

    // =========================================================================
    // v0.23.66: 模型角色分配 — 按请求的角色偏好解析目标模型
    // =========================================================================

    /// 解析请求对应的角色模型。
    ///
    /// 优先级：
    /// 1. 请求显式指定 `model_role` + 用户已为该角色设置模型 + 模型健康 →
    ///    返回该模型
    /// 2. 用户未设置 → 自动分配（Tool→TTFB最快，Background→最空闲，
    ///    Creative→active）
    /// 3. 角色模型不健康 → 返回 None，让网关正常打分
    fn resolve_role_model(
        &self,
        request: &GatewayRequest,
    ) -> Option<crate::config::settings::LlmProfile> {
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let config = crate::config::AppConfig::load(&app_dir).ok()?;

        // 确定目标角色：显式指定 > TaskClass 推导
        let role = request.model_role.or_else(|| {
            Some(match TaskClassifier::classify_task(request) {
                super::types::TaskClass::LightTool => crate::config::settings::ModelRole::Tool,
                super::types::TaskClass::HeavyCreation => {
                    crate::config::settings::ModelRole::Creative
                }
                _ => crate::config::settings::ModelRole::Creative,
            })
        })?;

        // 1. 用户已为该角色设置模型 → 检查健康后返回
        // v0.26.52: 用户显式指定的角色模型允许 Unknown（刚创建尚未探测），
        // 否则设为创作模型后仍回退到旧
        // active_llm_profile，表现为「默认创作模型不生效」。
        if let Some(profile) = config.get_model_for_role(role) {
            if self.is_promotable_user_model(&profile.id) {
                let failures = self
                    .health_registry()
                    .lock()
                    .ok()
                    .map(|g| g.consecutive_failures(&profile.id))
                    .unwrap_or(0);
                if failures < ACTIVE_MODEL_DEMOTION_THRESHOLD {
                    log::info!(
                        "[Gateway] resolve_role_model: 使用角色指定模型 {:?} → {}",
                        role,
                        profile.id
                    );
                    return Some(profile.clone());
                }
            }
            // 角色模型不可用 → fall through 到自动分配
        }

        // 2. 自动分配
        match role {
            crate::config::settings::ModelRole::Tool => {
                // 工具模型：选 TTFB 最快的可用模型
                self.pick_fastest_for_role()
            }
            crate::config::settings::ModelRole::Background => {
                // 后台模型：避免抢占创作/当前活跃模型
                self.pick_idle_for_background(&config)
            }
            crate::config::settings::ModelRole::Creative => {
                // 创作模型：回退到 active_llm_profile
                self.llm_service.get_active_profile()
            }
        }
    }

    /// 自动分配工具模型：选 TTFB 最快的健康模型
    fn pick_fastest_for_role(&self) -> Option<crate::config::settings::LlmProfile> {
        // 从 capability profiles 按 TTFB 排序选最快
        if let Some(pool_state) = self.app_handle.try_state::<crate::db::DbPool>() {
            if let Ok(profiles) =
                super::capability_store::CapabilityStore::new(pool_state.inner().clone()).load_all()
            {
                let registry = self.registry_guard();
                let mut candidates: Vec<_> = profiles
                    .iter()
                    .filter(|p| {
                        self.is_model_available(&p.model_id) && registry.get(&p.model_id).is_some()
                    })
                    .collect();
                candidates.sort_by_key(|p| p.short_ttfb_ms_p50.unwrap_or(u64::MAX));
                if let Some(fastest) = candidates.first() {
                    return registry.get(&fastest.model_id).cloned();
                }
            }
        }
        // 回退：从注册表选第一个健康的
        let registry = self.registry_guard();
        registry
            .enabled_generative_models()
            .into_iter()
            .find(|p| self.is_model_available(&p.id))
            .cloned()
    }

    /// 自动分配后台模型：避免使用当前活跃模型和创作模型
    fn pick_idle_for_background(
        &self,
        config: &crate::config::AppConfig,
    ) -> Option<crate::config::settings::LlmProfile> {
        let registry = self.registry_guard();
        let active_id = config.active_llm_profile.as_deref();
        let creative_id = config.creative_model_id.as_deref();

        // 优先选一个非活跃、非创作的模型
        let idle = registry
            .enabled_generative_models()
            .into_iter()
            .filter(|p| {
                self.is_model_available(&p.id)
                    && Some(p.id.as_str()) != active_id
                    && Some(p.id.as_str()) != creative_id
            })
            .next()
            .cloned();

        // 回退：如果只有 active/creative 模型可用，用 active
        idle.or_else(|| {
            active_id.and_then(|id| {
                if self.is_model_available(id) {
                    registry.get(id).cloned()
                } else {
                    None
                }
            })
        })
    }

    /// v0.23.13: 从最新配置刷新网关模型注册表，确保新增/修改模型后立即可见。
    pub fn refresh_registry(&self) {
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        match crate::config::AppConfig::load(&app_dir) {
            Ok(config) => {
                if let Ok(mut reg) = self.registry.lock() {
                    reg.reload(&config);
                    log::info!(
                        "[GatewayExecutor] 注册表已刷新，当前启用模型数: {}",
                        reg.inner.len()
                    );
                }
            }
            Err(e) => {
                log::warn!("[GatewayExecutor] 刷新注册表失败: {}", e);
            }
        }
    }

    /// v0.23.12: 快捷记录工作流日志
    fn workflow_log(
        &self,
        phase: impl Into<String>,
        message: impl Into<String>,
        details: Option<serde_json::Value>,
    ) {
        if let Some(logger) = self
            .app_handle
            .try_state::<std::sync::Arc<crate::workflow_logger::WorkflowLogger>>()
        {
            logger.info(phase, message, details);
        }
    }

    /// v0.23.63: 路由细节日志仅输出到控制台 debug，不写入工作流日志文件。
    /// 避免 8 次 LLM 调用产生 ~96 行 gateway 路由噪声。
    fn gateway_debug(
        &self,
        phase: &str,
        message: impl Into<String>,
        details: Option<serde_json::Value>,
    ) {
        let msg: String = message.into();
        if let Some(d) = &details {
            log::debug!("[{}] {} | {}", phase, msg, d);
        } else {
            log::debug!("[{}] {}", phase, msg);
        }
    }

    /// v0.23.13: 模型必须出现在健康注册表且状态为 Healthy/Degraded
    /// 才可用于调度。
    fn is_model_available(&self, model_id: &str) -> bool {
        let health = self.health_registry();
        if let Ok(guard) = health.lock() {
            if let Some(snap) = guard.get(model_id) {
                return matches!(
                    snap.status,
                    super::types::HealthStatus::Healthy | super::types::HealthStatus::Degraded
                );
            }
        }
        false
    }

    /// v0.26.52: 用户显式指定的角色/活跃模型是否可强制置顶。
    ///
    /// 比 `is_model_available` 更宽：允许 Unknown（刚新增尚未探测），
    /// 仍拒绝 Unhealthy。自动候选池继续走 `is_model_available`，避免未探测模型
    /// 被网关自动选中。
    ///
    /// 健康锁在嵌套块内释放后再读 registry，避免与持 registry 再锁 health
    /// 的路径死锁。
    fn is_promotable_user_model(&self, model_id: &str) -> bool {
        let snapshot_status = {
            let health = self.health_registry();
            health
                .lock()
                .ok()
                .and_then(|guard| guard.get(model_id).map(|s| s.status))
        };
        match snapshot_status {
            Some(status) => matches!(
                status,
                super::types::HealthStatus::Healthy
                    | super::types::HealthStatus::Degraded
                    | super::types::HealthStatus::Unknown
            ),
            // 注册表有模型但尚无健康快照：视为 Unknown，允许用户显式选择
            None => self.registry_guard().get(model_id).is_some(),
        }
    }

    /// v0.23.59: 活跃模型连续失败次数达到降级阈值时返回 true。
    ///
    /// 降级后活跃模型不再被 `+1000` 强制置顶，让其他健康候选接管。
    /// 成功 1 次即清零恢复。阈值 2：容忍 1 次偶发失败，2 次连续失败才降级。
    pub fn active_model_demoted(&self) -> bool {
        if let Some(active) = self.llm_service.get_active_profile() {
            let failures = self
                .health_registry()
                .lock()
                .ok()
                .map(|g| g.consecutive_failures(&active.id))
                .unwrap_or(0);
            failures >= ACTIVE_MODEL_DEMOTION_THRESHOLD
        } else {
            false
        }
    }

    /// v0.23.60: 健康数据是否新鲜（<15s 前探测过）。
    ///
    /// 若为 true，网关可跳过内联 5s 预探测，直接信任缓存。
    /// 后台 keepalive 每 10s 刷新一次健康模型，保证正常运行时缓存始终新鲜。
    fn is_health_fresh(&self, model_id: &str) -> bool {
        let health = self.health_registry();
        if let Ok(guard) = health.lock() {
            if let Some(snap) = guard.get(model_id) {
                if let Some(ref ts) = snap.last_checked_at {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                        let age_secs = chrono::Utc::now()
                            .signed_duration_since(dt.with_timezone(&chrono::Utc))
                            .num_seconds();
                        return age_secs >= 0 && age_secs < 15;
                    }
                }
            }
        }
        false
    }

    /// v0.23.60: 供 LlmService 调用的公开健康新鲜度查询。
    pub fn is_health_fresh_public(&self, model_id: &str) -> bool {
        self.is_health_fresh(model_id)
    }

    /// v0.23.59: 记录候选模型真实调用失败（探测通过但生成失败）。
    ///
    /// 标记为 Degraded（保留在候选池中受 -20 惩罚），递增连续失败计数。
    /// 这是让"能说 OK 但无法生成正文"的模型对调度器可见的关键。
    fn record_gateway_failure(
        &self,
        model_id: &str,
        model_name: &str,
        status: super::types::HealthStatus,
        error: Option<String>,
    ) {
        if let Ok(mut guard) = self.health_registry().lock() {
            guard.record_failure(model_id, model_name, status, error);
        }
    }

    /// v0.23.59: 记录候选模型成功（探测通过或真实调用成功），重置连续失败计数。
    fn record_gateway_success(&self, model_id: &str, model_name: &str) {
        if let Ok(mut guard) = self.health_registry().lock() {
            guard.record_success(model_id, model_name);
        }
    }

    /// v0.23.59: 供 LlmService 调用的公开标记 Unhealthy 接口。
    ///
    /// `generate_with_fastest` 在 5s 预探测失败时调用此方法标记模型，
    /// 随后回退到网关候选链（自带探测 + fallback）。
    pub fn mark_unhealthy(&self, model_id: &str, model_name: &str, error: Option<String>) {
        if let Ok(mut guard) = self.health_registry().lock() {
            guard.record_failure(
                model_id,
                model_name,
                super::types::HealthStatus::Unhealthy,
                error,
            );
        }
    }

    /// v0.23.59: 供 LlmService 调用的公开标记成功接口。
    ///
    /// `generate_with_fastest` 在预探测通过后调用此方法重置连续失败计数，
    /// 恢复活跃模型的强制置顶资格。
    pub fn record_success_public(&self, model_id: &str, model_name: &str) {
        if let Ok(mut guard) = self.health_registry().lock() {
            guard.record_success(model_id, model_name);
        }
    }

    /// 选择候选模型链（v0.15.0 三维打分：算力 50% + 偏好 30% + 适配 20%）
    pub fn select_candidates(
        &self,
        request: &GatewayRequest,
        // v0.23.28: 预加载的能力档案，由异步 generate 通过 spawn_blocking 传入，
        // 避免同步 DB 查询阻塞 tokio worker 线程。
        preloaded_capability_profiles: Option<Vec<crate::model_gateway::types::CapabilityProfile>>,
    ) -> Result<GatewayRoutingDecision, AppError> {
        let routing_request = self.classifier.classify(request);
        let router = {
            let guard = self.registry_guard();
            crate::router::UnifiedModelRouter::new(guard.inner.clone())
        };

        // 先获取基础路由决策与候选链
        let mut decision = router.route(&routing_request)?;

        // 应用健康权重与任务匹配微调
        let health_registry = self.health_registry();

        // Phase 2/3: 计算请求 asset_tags 与模型 tags 的重叠，用于候选模型微调
        let request_tag_set: std::collections::HashSet<&str> =
            request.asset_tags.iter().map(|s| s.as_str()).collect();

        // 从注册表一次性取出候选模型（克隆，避免持有锁跨后续计算）
        let candidate_profiles: Vec<(f64, crate::config::settings::LlmProfile)> = {
            let registry = self.registry_guard();
            decision
                .candidates
                .iter()
                .filter_map(|c| registry.get(&c.model_id).map(|m| (c.score, m.clone())))
                .collect()
        };

        // v0.23.34: health 锁限定在块作用域内，块结束后 MutexGuard 自动释放。
        // 根因：std::sync::Mutex 不可重入，若 health 锁未释放，后续
        // is_model_available / health_registry().lock() 会自死锁 → 600s 超时。
        let mut re_scored: Vec<(f64, crate::config::settings::LlmProfile)> = {
            let health = health_registry.lock().ok();
            let result: Vec<_> = candidate_profiles
                .into_iter()
                .map(|(base_score, m)| {
                    let mut score = base_score;
                    // v0.15.0 三维打分
                    // 健康状态约束
                    if let Some(ref h) = health {
                        if let Some(snapshot) = h.get(&m.id) {
                            match snapshot.status {
                                super::types::HealthStatus::Unhealthy => score -= 1000.0,
                                super::types::HealthStatus::Degraded => score -= 20.0,
                                super::types::HealthStatus::Unknown => score *= 0.5,
                                _ => {}
                            }
                        }
                    }

                    // Phase 2/3: 标签重叠加分（最多 +10），让匹配到同类标签的模型优先
                    if !request_tag_set.is_empty() {
                        let overlap = m
                            .tags
                            .iter()
                            .filter(|t| request_tag_set.contains(t.as_str()))
                            .count() as f64;
                        score += (overlap * 3.0).min(10.0);
                    }

                    (score, m)
                })
                .collect();
            // health MutexGuard 在此块结束时释放
            result
        };
        re_scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        // ↑ health 锁已释放，is_model_available 可以安全地重新锁定 health_registry

        // 更新候选链顺序与 primary
        let candidates: Vec<crate::router::RankedCandidate> = re_scored
            .iter()
            .take(3)
            .map(|(score, m)| crate::router::RankedCandidate {
                model_id: m.id.clone(),
                model_name: m.name.clone(),
                score: *score,
                reason: format!("综合得分 {:.1}", score),
            })
            .collect();

        // v0.23.13: 只保留健康检查结果为可用（Healthy/Degraded）的模型。
        let candidates: Vec<_> = candidates
            .into_iter()
            .filter(|c| self.is_model_available(&c.model_id))
            .collect();

        if let Some(primary) = candidates.first() {
            decision.model_id = primary.model_id.clone();
            decision.model_name = primary.model_name.clone();
        }
        decision.candidates = candidates;

        // v0.22.0: 算力档案消费闭环（Phase D）
        // v0.22.1: 按 TaskClass 应用差异化权重（意见5）
        // v0.23.28: 能力档案由异步 generate 通过 spawn_blocking 预加载，
        // 避免同步 DB 查询阻塞 tokio worker 线程。
        if let Some(profiles) = preloaded_capability_profiles {
            if !profiles.is_empty() {
                let task_class = TaskClassifier::classify_task(request);
                let mut candidates = decision.candidates.clone();
                for c in &mut candidates {
                    if let Some(cap) = profiles.iter().find(|p| p.model_id == c.model_id) {
                        let ttfb = cap.short_ttfb_ms_p50.unwrap_or(5000) as f64;
                        let tps = cap.sustained_tps.unwrap_or(10.0);
                        let success = cap.success_rate_24h.unwrap_or(0.9);
                        let cap_score = cap.capability_score.unwrap_or(0.0);

                        let speed_bonus = if ttfb < 2000.0 {
                            5.0 * (1.0 - ttfb / 2000.0) + tps * 0.2
                        } else {
                            0.0
                        };
                        let quality_bonus = success * 3.0 + cap_score * 2.0;

                        use super::types::TaskClass;
                        c.score += match task_class {
                            TaskClass::HeavyCreation => speed_bonus * 0.2 + quality_bonus * 0.8,
                            TaskClass::LightTool => speed_bonus * 0.6 + quality_bonus * 0.4,
                            _ => speed_bonus * 0.4 + quality_bonus * 0.6,
                        };
                    }
                }
                candidates.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                // v0.23.10: 保证用户当前设置的活跃模型始终在候选链中（只要健康），
                // 避免路由结果完全脱离用户预期。
                if let Some(active) = self.llm_service.get_active_profile() {
                    if !candidates.iter().any(|c| c.model_id == active.id) {
                        let active_healthy = self
                            .health_registry()
                            .lock()
                            .ok()
                            .and_then(|h| h.get(&active.id).cloned())
                            .map(|s| s.status != super::types::HealthStatus::Unhealthy)
                            .unwrap_or(true);
                        if active_healthy {
                            let base_score = candidates.first().map(|c| c.score).unwrap_or(50.0);
                            candidates.push(crate::router::RankedCandidate {
                                model_id: active.id.clone(),
                                model_name: active.name.clone(),
                                score: base_score,
                                reason: "当前活跃模型兜底".to_string(),
                            });
                            candidates.sort_by(|a, b| {
                                b.score
                                    .partial_cmp(&a.score)
                                    .unwrap_or(std::cmp::Ordering::Equal)
                            });
                        }
                    }
                }

                decision.candidates = candidates;
            }
        }

        log::debug!(
            "[Gateway] select_candidates: 能力档案段结束, 候选={}",
            decision.candidates.len()
        );

        // v0.23.66: 角色模型优先 — 若请求有角色偏好且用户为该角色指定了模型，
        // 用角色模型替代 active_llm_profile 做强制置顶。
        // 未指定角色时回退到现有 active 逻辑，保持完全向后兼容。
        let role_profile = self.resolve_role_model(request);
        let is_role = role_profile.is_some();
        let promotion_profile = role_profile.or_else(|| self.llm_service.get_active_profile());

        // v0.23.13: 活跃模型强制置顶。v0.23.32: 每步 Mutex 前后加标记诊断。
        // v0.23.59: 连续失败达阈值时跳过强制置顶，交给候选打分选择。
        // v0.23.66: 若已解析到角色模型，使用角色模型；否则回退 active。
        if let Some(ref promote) = promotion_profile {
            log::debug!("[Gateway] select_candidates: 置顶模型={}", promote.id);
            if self.active_model_demoted() {
                let failures = self
                    .health_registry()
                    .lock()
                    .ok()
                    .map(|g| g.consecutive_failures(&promote.id))
                    .unwrap_or(0);
                log::warn!(
                    "[Gateway] select_candidates: {}模型 {} 连续失败 {} 次，跳过强制置顶",
                    if is_role { "角色" } else { "活跃" },
                    promote.id,
                    failures
                );
            } else if self.is_promotable_user_model(&promote.id) {
                if let Some(profile) = self.registry_guard().get(&promote.id).cloned() {
                    log::debug!("[Gateway] select_candidates: 注册表查询成功");
                    let mut candidates = decision.candidates.clone();
                    candidates.retain(|c| c.model_id != promote.id);
                    let top_score = candidates.first().map(|c| c.score).unwrap_or(50.0);
                    let reason = if is_role {
                        format!(
                            "角色模型({:?})强制优先",
                            request
                                .model_role
                                .unwrap_or(crate::config::settings::ModelRole::Creative)
                        )
                    } else {
                        "当前活跃模型强制优先".to_string()
                    };
                    let promote_candidate = crate::router::RankedCandidate {
                        model_id: promote.id.clone(),
                        model_name: promote.name.clone(),
                        score: top_score + 1000.0,
                        reason,
                    };
                    candidates.insert(0, promote_candidate);
                    decision.model_id = promote.id.clone();
                    decision.model_name = promote.name.clone();
                    decision.candidates = candidates;
                    log::debug!(
                        "[Gateway] select_candidates: 强制使用{}模型 {} (score={})",
                        if is_role { "角色" } else { "活跃" },
                        promote.id,
                        top_score + 1000.0
                    );
                }
            } else {
                log::warn!(
                    "[Gateway] select_candidates: {}模型 {} 不可用，继续按网关打分选择",
                    if is_role { "角色" } else { "活跃" },
                    promote.id
                );
            }
        }

        log::debug!(
            "[Gateway] select_candidates 返回, {} 个候选",
            decision.candidates.len()
        );
        Ok(decision)
    }

    /// v0.23 TriShot：选取「最快可用模型」profile，用于 Call 1 路由合成器。
    ///
    /// 策略：从所有 enabled 模型中，按算力档案 `short_ttfb_ms_p50` 升序 +
    /// `success_rate_24h` 降序排序，剔除 Unhealthy。无算力档案时回退到
    /// `select_candidates`（让网关三维打分兜底）。最终都失败则回退 active
    /// profile。
    pub fn select_fastest_profile(&self) -> Option<crate::config::settings::LlmProfile> {
        // v0.23.66: 工具模型优先 — 若用户设置了工具模型且健康，优先使用。
        // select_fastest_profile 主要用于 TriShot Call 1（路由合成），属于 Tool 角色。
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        if let Ok(config) = crate::config::AppConfig::load(&app_dir) {
            if let Some(tool_model) =
                config.get_model_for_role(crate::config::settings::ModelRole::Tool)
            {
                if self.is_model_available(&tool_model.id)
                    && self.registry_guard().get(&tool_model.id).is_some()
                    && !self.active_model_demoted()
                {
                    log::info!(
                        "[Gateway] select_fastest_profile: 使用工具模型 {}",
                        tool_model.id
                    );
                    return Some(tool_model.clone());
                }
            }
        }

        let active = self.llm_service.get_active_profile();

        // v0.23.13: 用户 explicit 设置的活跃模型无条件优先（只要健康检查未判定
        // Unhealthy）。 这是为了避免 TriShot Call 1
        // 等「最快模型」路径绕过用户当前设置的模型，
        // 连接到用户未预期或实际不可用的模型。
        // v0.23.59: 连续失败达阈值时跳过短路，走 TTFB 排序选其他候选。
        if let Some(ref active_profile) = active {
            let active_available = self.is_model_available(&active_profile.id);
            let active_in_registry = self.registry_guard().get(&active_profile.id).is_some();
            let active_demoted = self.active_model_demoted();
            if active_in_registry && active_available && !active_demoted {
                log::info!(
                    "[Gateway] select_fastest_profile: 无条件使用当前活跃模型 {}",
                    active_profile.id
                );
                log::debug!(
                    "[Gateway] select_fastest_profile: 无条件使用当前活跃模型 {}",
                    active_profile.id
                );
                return self.registry_guard().get(&active_profile.id).cloned();
            } else if active_demoted {
                log::warn!(
                    "[Gateway] select_fastest_profile: 活跃模型 {} 连续失败已降级，走 TTFB 排序",
                    active_profile.id
                );
            }
        }

        // 1) 优先用算力档案按 TTFB 选最快模型
        if let Some(pool) = self.app_handle.try_state::<crate::db::DbPool>() {
            if let Ok(profiles) =
                super::capability_store::CapabilityStore::new(pool.inner().clone()).load_all()
            {
                let health = self.health_registry();
                let health_guard = health.lock().ok();

                // v0.23.14: 排序键改为 (ttfb_bucket, -capability_score, -success_rate, id)
                // ttfb_bucket 将 TTFB 按 20% 宽度分桶，同桶内按能力分降序，
                // 避免"快 1ms 但质量差 10 倍"的模型被选中。
                let mut ranked: Vec<(u64, f64, f64, String)> = profiles
                    .iter()
                    .filter(|cap| cap.status != super::types::HealthStatus::Unhealthy)
                    .filter_map(|cap| {
                        // 只保留 registry 中存在且 enabled 的模型
                        self.registry_guard().get(&cap.model_id)?;
                        let ttfb = cap.short_ttfb_ms_p50.unwrap_or(10_000);
                        let success = cap.success_rate_24h.unwrap_or(0.0);
                        let cap_score = cap.capability_score.unwrap_or(0.0);
                        // 若有健康快照且 Unhealthy 则跳过（双保险）
                        if let Some(ref h) = health_guard {
                            if let Some(snap) = h.get(&cap.model_id) {
                                if snap.status == super::types::HealthStatus::Unhealthy {
                                    return None;
                                }
                            }
                        }
                        // 分桶：每 200ms 一桶，同桶内能力分高的优先
                        let ttfb_bucket = (ttfb / 200) * 200;
                        Some((ttfb_bucket, -cap_score, -success, cap.model_id.clone()))
                    })
                    .collect();
                ranked.sort_by(|a, b| {
                    a.0.cmp(&b.0) // ttfb_bucket 升序
                        .then_with(|| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)) // -cap_score 升序（即 cap_score 降序）
                        .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
                    // -success 降序
                });

                if let Some((fastest_ttfb, _, _, fastest_id)) = ranked.first() {
                    // v0.23.12: 用户当前设置的活跃模型优先使用：
                    // 1) 活跃模型无算力档案（用户刚添加或从未探测），直接用它；
                    // 2) 活跃模型有档案且 TTFB 不比最快模型差太多（<= 3x 且至少 3000ms），用它；
                    // 3) 否则才回退到全局最快模型。
                    if let Some(ref active_profile) = active {
                        let active_healthy = health_guard
                            .as_ref()
                            .and_then(|h| h.get(&active_profile.id))
                            .map(|s| s.status != super::types::HealthStatus::Unhealthy)
                            .unwrap_or(true);
                        if active_healthy {
                            let active_cap =
                                profiles.iter().find(|p| p.model_id == active_profile.id);
                            let prefer_active = match active_cap {
                                Some(cap)
                                    if cap.status != super::types::HealthStatus::Unhealthy =>
                                {
                                    let active_ttfb = cap.short_ttfb_ms_p50.unwrap_or(10_000);
                                    let threshold = (*fastest_ttfb * 3).max(3000);
                                    active_ttfb <= threshold
                                }
                                // 没有算力档案时，优先使用用户 explicit 设置的活跃模型
                                None => true,
                                _ => false,
                            };
                            if prefer_active {
                                let active_ttfb =
                                    active_cap.and_then(|c| c.short_ttfb_ms_p50).unwrap_or(0);
                                log::info!(
                                    "[Gateway] select_fastest_profile: 偏好使用当前活跃模型 {} (ttfb_p50={}ms, fastest={}ms)",
                                    active_profile.id, active_ttfb, fastest_ttfb
                                );
                                return self.registry_guard().get(&active_profile.id).cloned();
                            }
                        }
                    }

                    if let Some(profile) = self.registry_guard().get(fastest_id).cloned() {
                        log::info!(
                            "[Gateway] select_fastest_profile: 选中 {} (ttfb_p50={}ms)",
                            profile.id,
                            profiles
                                .iter()
                                .find(|p| &p.model_id == fastest_id)
                                .and_then(|p| p.short_ttfb_ms_p50)
                                .unwrap_or(0)
                        );
                        return Some(profile);
                    }
                }
            }
        }

        // 2) 无算力档案：用 select_candidates 走网关三维打分兜底
        let req = super::types::GatewayRequest::for_fast_routing(String::new(), "tri-shot-router");
        if let Ok(decision) = self.select_candidates(&req, None) {
            if let Some(primary) = decision.candidates.first() {
                if let Some(profile) = self.registry_guard().get(&primary.model_id).cloned() {
                    log::info!(
                        "[Gateway] select_fastest_profile (无档案兜底): 选中 {} (score={:.1})",
                        profile.id,
                        primary.score
                    );
                    return Some(profile);
                }
            }
        }

        // 3) 最终回退：active profile
        log::warn!("[Gateway] select_fastest_profile: 回退到 active profile");
        self.llm_service.get_active_profile()
    }

    /// 统一生成入口：选择候选链并顺序执行 fallback
    pub async fn generate(&self, request: GatewayRequest) -> Result<LlmGenerateResponse, AppError> {
        let request_id = request.request_id.clone();
        let trace_store = self
            .app_handle
            .try_state::<crate::tracing::TraceStore>()
            .map(|s| s.inner().clone());
        if let (Some(trace_id), Some(store)) = (request.trace_id.as_ref(), trace_store.as_ref()) {
            let _ = store.add_step(trace_id, "gateway.generate", "model_gateway");
            let _ = store.associate_request_id(trace_id, &request_id);
        }
        log::debug!("[Gateway] generate.enter request_id={}", request_id);

        // v0.23.28: 用 spawn_blocking 预加载能力档案（同步 DB 查询），
        // 连接池满时不会阻塞 tokio worker 线程导致 Call 3 hang住。
        let pool = self.pool.clone();
        let capability_profiles = tokio::task::spawn_blocking(move || {
            super::capability_store::CapabilityStore::new(pool)
                .load_all()
                .ok()
        })
        .await
        .unwrap_or(None);
        log::debug!(
            "[Gateway] 能力档案加载完成: {} 条",
            capability_profiles.as_ref().map(|v| v.len()).unwrap_or(0)
        );

        log::debug!("[Gateway] 开始 select_candidates");
        let mut decision = self.select_candidates(&request, capability_profiles)?;
        log::debug!(
            "[Gateway] select_candidates 完成: 候选数={}",
            decision.candidates.len()
        );

        // v0.26.0: 记录候选链信息到 trace
        if let (Some(trace_id), Some(store)) = (request.trace_id.as_ref(), trace_store.as_ref()) {
            let candidate_ids: Vec<String> = decision
                .candidates
                .iter()
                .map(|c| c.model_id.clone())
                .collect();
            let _ = store.record_step_detail(trace_id, 0, |step| {
                step.details = Some(serde_json::json!({
                    "candidates": candidate_ids,
                    "primary_model": decision.candidates.first().map(|c| c.model_id.clone()),
                    "candidate_count": decision.candidates.len(),
                }));
            });
        }

        // v0.23.12: 用户当前设置的活跃模型应该作为第一候选，避免路由器选一个
        // 用户没预期的模型（尤其是旧模型或算力档案看起来“快”但实际挂起的模型）。
        // v0.23.59: 连续失败达阈值时跳过再提升，select_candidates 已不强制置顶。
        if let Some(active) = self.llm_service.get_active_profile() {
            if self.active_model_demoted() {
                let failures = self
                    .health_registry()
                    .lock()
                    .ok()
                    .map(|g| g.consecutive_failures(&active.id))
                    .unwrap_or(0);
                log::warn!(
                    "[Gateway] generate: 活跃模型 {} 连续失败 {} 次，跳过候选链首位提升",
                    active.id,
                    failures
                );
            } else if decision.candidates.first().map(|c| c.model_id.as_str())
                != Some(active.id.as_str())
            {
                if let Some(pos) = decision
                    .candidates
                    .iter()
                    .position(|c| c.model_id == active.id)
                {
                    let item = decision.candidates.remove(pos);
                    decision.candidates.insert(0, item);
                    log::debug!("[Gateway] 将活跃模型 {} 提升至候选链首位", active.id);
                } else if self.registry_guard().get(&active.id).is_some()
                    && self.is_model_available(&active.id)
                {
                    // v0.23.50: 活跃模型若已被 select_candidates 判为不可用
                    // （Unhealthy），不再强行插回候选链首位，否则每次 generate()
                    // 都会先 5s 探测这个死模型再失败 continue，造成"卡在最终输出"
                    // 的假象并持续覆盖前端状态。交给候选链里的其他健康模型。
                    decision.candidates.insert(
                        0,
                        crate::router::RankedCandidate {
                            model_id: active.id.clone(),
                            model_name: active.name.clone(),
                            score: decision.candidates.first().map(|c| c.score).unwrap_or(50.0),
                            reason: "当前活跃模型优先".to_string(),
                        },
                    );
                    log::debug!("[Gateway] 将活跃模型 {} 插入候选链首位", active.id);
                }
            }
        }

        log::debug!(
            "[Gateway] 候选链已确定，共 {} 个模型",
            decision.candidates.len()
        );

        if decision.candidates.is_empty() {
            return Err(AppError::Internal {
                message: "没有可用的模型候选".to_string(),
            });
        }

        let mut last_error: Option<AppError> = None;
        log::debug!(
            "[Gateway] 进入候选链循环, {} 个候选项",
            decision.candidates.len()
        );
        for (idx, candidate) in decision.candidates.iter().enumerate() {
            self.workflow_log(
                "gateway.generate.candidate_try",
                format!("尝试候选 [{}/{}]: {} ({})", idx + 1, decision.candidates.len(), candidate.model_name, candidate.model_id),
                Some(serde_json::json!({"request_id": request.request_id, "idx": idx, "model_id": candidate.model_id})),
            );
            let profile = self.registry_guard().get(&candidate.model_id).cloned();
            let Some(profile) = profile else {
                continue;
            };

            // v0.23.47: 调用模型前必须实时检测连接是否正常（5s 超时）。
            // v0.23.60: 若后台 keepalive 已保持健康数据新鲜（<15s），跳过内联探测，
            // 直接信任缓存。keepalive 每 10s 刷新一次，保证正常运行时 0ms 延迟。
            let health_fresh = self.is_health_fresh(&candidate.model_id);
            let probe_passed = if health_fresh {
                log::debug!(
                    "[Gateway] 候选 [{}] 健康数据新鲜（<15s），跳过内联探测",
                    idx + 1
                );
                true
            } else {
                let probe_request_id = format!("pre-call-probe-{}", &candidate.model_id);
                let probe_profile = profile.clone();
                let probe_service = self.llm_service.clone();
                let probe_result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    let (_, result) = probe_service
                        .generate_with_profile_and_request_id(
                            &probe_profile.id,
                            "Respond with exactly the word OK.".to_string(),
                            Some(4),
                            Some(0.0),
                            Some("pre-call-probe"),
                            Some(probe_request_id),
                            Some(5),
                            Some(0),
                        )
                        .await;
                    result
                })
                .await;
                match probe_result {
                    Ok(Ok(_resp)) => {
                        log::debug!("[Gateway] 候选 [{}] 实时探测通过", idx + 1);
                        self.record_gateway_success(&candidate.model_id, &candidate.model_name);
                        true
                    }
                    Ok(Err(e)) => {
                        log::warn!(
                            "[Gateway] 候选 [{}/{}] {} 实时探测失败，跳过: {}",
                            idx + 1,
                            decision.candidates.len(),
                            candidate.model_id,
                            e
                        );
                        self.record_gateway_failure(
                            &candidate.model_id,
                            &candidate.model_name,
                            super::types::HealthStatus::Unhealthy,
                            Some(e.to_string()),
                        );
                        last_error = Some(e);
                        false
                    }
                    Err(_elapsed) => {
                        log::warn!(
                            "[Gateway] 候选 [{}/{}] {} 实时探测超时（5s），跳过",
                            idx + 1,
                            decision.candidates.len(),
                            candidate.model_id
                        );
                        self.record_gateway_failure(
                            &candidate.model_id,
                            &candidate.model_name,
                            super::types::HealthStatus::Unhealthy,
                            Some("pre-call probe timeout (5s)".to_string()),
                        );
                        last_error = Some(AppError::Internal {
                            message: format!("模型 {} 实时探测超时", candidate.model_name),
                        });
                        false
                    }
                }
            }; // end probe_passed assignment

            if !probe_passed {
                continue;
            }

            // 实际调用底层 LlmService 的按 profile 执行接口，透传 response_format
            // 以支持 OpenAI/Ollama 的 JSON mode。
            // v0.23.65: 透传请求级 system_prompt（writer_system 写作准则）。
            let max_retries = request.max_retries_override.unwrap_or(1);
            let candidate_idx = idx + 1;
            let trace_id_for_retry = request.trace_id.clone();
            let outcome = retry_with_backoff(
                || {
                    let service = self.llm_service.clone();
                    let profile_id = profile.id.clone();
                    let prompt = request.prompt.clone();
                    let max_tokens = request.max_tokens;
                    let temperature = request.temperature;
                    let context_label_owned = request.context_label.clone();
                    let request_id = request.request_id.clone();
                    let timeout_override = request.timeout_seconds_override;
                    let retries_override = Some(0u32); // 网关外层接管重试，内层不再重复
                    let response_format = request.response_format;
                    let system_prompt = request.system_prompt.clone();
                    let trace_id = trace_id_for_retry.clone();
                    async move {
                        let (_, result) = service
                            .generate_with_profile_and_request_id_with_format(
                                &profile_id,
                                prompt,
                                max_tokens,
                                temperature,
                                context_label_owned.as_deref(),
                                Some(request_id),
                                timeout_override,
                                retries_override,
                                response_format,
                                system_prompt,
                                trace_id,
                            )
                            .await;
                        result
                    }
                },
                max_retries,
                200,
                2000,
                &format!(
                    "gateway.generate candidate [{}/{}] {}",
                    candidate_idx,
                    decision.candidates.len(),
                    candidate.model_id
                ),
            )
            .await;
            match outcome.into_result() {
                Ok(resp) => {
                    // v0.23.59: 真实调用成功，重置连续失败计数，恢复强制置顶资格
                    self.record_gateway_success(&candidate.model_id, &candidate.model_name);
                    if let (Some(trace_id), Some(store)) =
                        (request.trace_id.as_ref(), trace_store.as_ref())
                    {
                        let _ = store.record_step_detail(trace_id, 0, |step| {
                            step.model_id = Some(candidate.model_id.clone());
                            step.provider = candidate
                                .model_name
                                .split('/')
                                .next()
                                .map(|s| s.to_string());
                        });
                        let _ = store.finish_last_step(trace_id, "completed", None);
                    }
                    return Ok(resp);
                }
                Err(e) => {
                    log::warn!(
                        "[Gateway] 模型 {} 调用失败（第 {} 候选，含重试）: {}",
                        candidate.model_id,
                        candidate_idx,
                        e
                    );
                    // v0.23.59: 真实调用失败标记 Degraded（保留在候选池受 -20 惩罚），
                    // 递增连续失败计数。这是让"能说 OK 但无法生成正文"的模型
                    // 对调度器可见的关键——下次 generate 不再强制置顶它。
                    self.record_gateway_failure(
                        &candidate.model_id,
                        &candidate.model_name,
                        super::types::HealthStatus::Degraded,
                        Some(e.to_string()),
                    );
                    last_error = Some(e);
                    continue;
                }
            }
        }

        // v0.26.0: 网关失败时标记 trace 步骤失败
        if let (Some(trace_id), Some(store)) = (request.trace_id.as_ref(), trace_store.as_ref()) {
            let error_msg = last_error
                .as_ref()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "所有候选模型均调用失败".to_string());
            let _ = store.finish_last_step(trace_id, "failed", Some(error_msg));
        }

        Err(last_error.unwrap_or_else(|| AppError::Internal {
            message: "所有候选模型均调用失败".to_string(),
        }))
    }

    /// 轻量探测：用极短 prompt 测试模型可用性
    pub async fn probe_model(&self, model_id: &str) -> Result<ProbeResult, AppError> {
        let profile = self
            .registry_guard()
            .get(model_id)
            .cloned()
            .ok_or_else(|| AppError::Internal {
                message: format!("模型 {} 不存在", model_id),
            })?;

        // v0.17.1: 优先从 PromptRegistry 读取，回退到 AppConfig.probe_prompt_override，
        // 最后回退到内置默认。让前端能在「提示词」面板编辑探测 prompt。
        let probe_prompt = {
            let from_registry = self
                .app_handle
                .try_state::<crate::db::DbPool>()
                .and_then(|pool| {
                    crate::prompts::registry::resolve_prompt(pool.inner(), "model_gateway_probe")
                        .ok()
                });
            from_registry
                .or_else(|| {
                    // v0.18.1 修复：使用 app_data_dir() 而非 current_dir()
                    let app_dir = self
                        .app_handle
                        .path()
                        .app_data_dir()
                        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
                    crate::config::AppConfig::load(&app_dir)
                        .ok()
                        .and_then(|cfg| {
                            if cfg.probe_prompt_override.is_empty() {
                                None
                            } else {
                                Some(cfg.probe_prompt_override.clone())
                            }
                        })
                })
                .unwrap_or_else(|| "Respond with exactly the word OK.".to_string())
        };

        let request = GenerateRequest {
            prompt: probe_prompt,
            max_tokens: Some(4),
            temperature: Some(0.0),
            ..Default::default()
        };

        let start = std::time::Instant::now();
        let (_, result) = self
            .llm_service
            .generate_with_profile_and_request_id(
                &profile.id,
                request.prompt,
                request.max_tokens,
                request.temperature,
                Some("model_gateway_probe"),
                Some(format!("probe-{}", model_id)),
                Some(30),
                Some(0),
            )
            .await;
        match result {
            Ok(resp) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let total_tokens = resp.tokens_used.max(1);
                let ttft_ms = duration_ms / 3; // v0.15.0: 粗略估计，真实TTFB由benchmark.rs流式基准提供
                let tps = if duration_ms > ttft_ms {
                    total_tokens as f64 * 1000.0 / (duration_ms - ttft_ms).max(1) as f64
                } else {
                    0.0
                };
                let result = ProbeResult {
                    success: true,
                    ttft_ms,
                    total_tokens,
                    duration_ms,
                    tps,
                    error: None,
                };
                self.probe_engine
                    .record_probe(&profile, &result, &self.pool);
                Ok(result)
            }
            Err(e) => {
                let result = ProbeResult {
                    success: false,
                    ttft_ms: 0,
                    total_tokens: 0,
                    duration_ms: start.elapsed().as_millis() as u64,
                    tps: 0.0,
                    error: Some(e.to_string()),
                };
                self.probe_engine
                    .record_probe(&profile, &result, &self.pool);
                Ok(result)
            }
        }
    }

    /// v0.15.0: 启动时运行流式基准（短+长任务），结果写入 capability_store
    /// 供 select_candidates 三维打分使用。真实 TTFB/TPS 替换旧 probe 魔法数。
    pub async fn run_initial_benchmark(&self) {
        use crate::model_gateway::{benchmark::StreamBenchmark, capability_store::CapabilityStore};

        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let profiles = StreamBenchmark::load_enabled_profiles(&app_dir);
        if profiles.is_empty() {
            log::info!("[GatewayExecutor] 无启用模型，跳过基准");
            return;
        }
        log::info!(
            "[GatewayExecutor] 启动首轮流式基准（{} 个模型）",
            profiles.len()
        );

        let benchmarker = StreamBenchmark::new(self.pool.clone());
        let store = CapabilityStore::new(self.pool.clone());

        for profile in &profiles {
            log::info!("[GatewayExecutor] 基准短任务 {}...", profile.name);
            let short = benchmarker.run_benchmark(profile, false).await;
            log::info!("[GatewayExecutor] 基准长任务 {}...", profile.name);
            let long = benchmarker.run_benchmark(profile, true).await;

            let mut cap = crate::model_gateway::types::CapabilityProfile {
                model_id: profile.id.clone(),
                short_ttfb_ms_p50: short.real_ttfb_ms,
                long_ttfb_ms_p50: long.real_ttfb_ms,
                sustained_tps: long.sustained_tps,
                short_output_tps: short.sustained_tps,
                success_rate_24h: Some(if short.success && long.success {
                    1.0
                } else {
                    0.5
                }),
                last_full_benchmark_at: Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64,
                ),
                benchmark_sample_count: 1,
                status: crate::model_gateway::types::HealthStatus::Unknown,
                ..Default::default()
            };
            // v0.23.14: 从实测数据合成 speed/quality/capability 三个分数
            cap.compute_scores();
            if let Err(e) = store.upsert(&cap) {
                log::error!("[GatewayExecutor] 保存算力档案失败 {}: {}", profile.id, e);
            } else {
                log::info!(
                    "[GatewayExecutor] 算力档案已保存 {}: 长TTFB={:?}ms TPS={:?}",
                    profile.name,
                    long.real_ttfb_ms,
                    long.sustained_tps
                );
            }
        }
        log::info!("[GatewayExecutor] 首轮基准完成");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::settings::{
            LlmProfile, LlmProvider, ModelCapability, ModelKind, ModelSource, QualityTier,
            SpeedTier,
        },
        db::connection::create_test_pool,
        llm::service::LlmService,
        router::{UnifiedModel, UnifiedModelRegistry},
    };

    fn test_profile(id: &str, name: &str) -> LlmProfile {
        LlmProfile {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            provider: LlmProvider::OpenAI,
            model_source: ModelSource::default(),
            model: "gpt-4".to_string(),
            api_key: String::new(),
            api_base: None,
            is_local_model: false,
            max_tokens: 1024,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: 30,
            is_default: false,
            enabled: true,
            kind: ModelKind::Chat,
            capabilities: vec![ModelCapability::Chat],
            max_context_length: 8192,
            quality_tier: QualityTier::Medium,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec![],
            supports_system_prompt: true,
            system_prompt_override: None,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
        }
    }

    fn test_executor(
        registry: GatewayRegistry,
    ) -> (
        GatewayExecutor<tauri::test::MockRuntime>,
        tauri::App<tauri::test::MockRuntime>,
    ) {
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        let pool = create_test_pool().unwrap();
        let llm = LlmService::new(handle.clone());
        let executor = GatewayExecutor::new(handle, registry, llm, pool);
        (executor, app)
    }

    #[test]
    fn test_select_candidates_returns_healthy_model() {
        let mut registry_inner = UnifiedModelRegistry::default();
        registry_inner.register(UnifiedModel::Generative(test_profile(
            "test-model",
            "Test Model",
        )));
        let (executor, _app) = test_executor(GatewayRegistry::new(registry_inner));
        executor.record_success_public("test-model", "Test Model");

        let request = crate::model_gateway::types::GatewayRequest::for_fast_routing(
            "hello".to_string(),
            "test-agent",
        );
        let decision = executor.select_candidates(&request, None).unwrap();

        assert!(!decision.candidates.is_empty());
        assert_eq!(decision.candidates[0].model_id, "test-model");
        assert_eq!(decision.candidates[0].model_name, "Test Model");
    }

    #[test]
    fn test_select_candidates_empty_registry_errors() {
        let (executor, _app) = test_executor(GatewayRegistry::default());
        let request = crate::model_gateway::types::GatewayRequest::for_fast_routing(
            "hello".to_string(),
            "test-agent",
        );
        let err = executor.select_candidates(&request, None).unwrap_err();
        assert!(
            err.to_string().contains("没有满足任务"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn test_mark_unhealthy_makes_model_unavailable() {
        let mut registry_inner = UnifiedModelRegistry::default();
        registry_inner.register(UnifiedModel::Generative(test_profile("m", "M")));
        let (executor, _app) = test_executor(GatewayRegistry::new(registry_inner));

        executor.record_success_public("m", "M");
        assert!(executor.is_model_available("m"));

        executor.mark_unhealthy("m", "M", Some("down".to_string()));
        assert!(!executor.is_model_available("m"));
    }

    /// v0.26.52: 刚加入注册表、尚无健康快照的模型对用户显式选择可置顶，
    /// 但对自动候选池仍不可用。
    #[test]
    fn test_promotable_allows_unknown_without_health_snapshot() {
        let mut registry_inner = UnifiedModelRegistry::default();
        registry_inner.register(UnifiedModel::Generative(test_profile("new-m", "New")));
        let (executor, _app) = test_executor(GatewayRegistry::new(registry_inner));

        assert!(
            !executor.is_model_available("new-m"),
            "Unknown/no-snapshot must stay out of auto candidate pool"
        );
        assert!(
            executor.is_promotable_user_model("new-m"),
            "user-selected role/active model must promote before probe completes"
        );
    }

    #[test]
    fn test_promotable_rejects_unhealthy() {
        let mut registry_inner = UnifiedModelRegistry::default();
        registry_inner.register(UnifiedModel::Generative(test_profile("bad", "Bad")));
        let (executor, _app) = test_executor(GatewayRegistry::new(registry_inner));
        executor.mark_unhealthy("bad", "Bad", Some("down".to_string()));
        assert!(!executor.is_promotable_user_model("bad"));
    }
}
