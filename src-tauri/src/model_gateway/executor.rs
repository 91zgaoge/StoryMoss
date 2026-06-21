//! Model Gateway — 执行层与 fallback
//!
//! v0.14.0: 按候选链顺序执行，主模型失败时自动降级。

use tauri::{AppHandle, Manager};

use super::{
    dispatcher::TaskClassifier,
    health::{HealthRegistry, ProbeEngine},
    registry::GatewayRegistry,
    types::{GatewayRequest, GatewayRoutingDecision, ProbeResult},
};
use crate::{
    error::AppError,
    llm::{adapter::GenerateRequest, service::LlmService, GenerateResponse as LlmGenerateResponse},
};

/// 网关执行器
#[derive(Clone)]
pub struct GatewayExecutor {
    app_handle: AppHandle,
    pub registry: GatewayRegistry,
    classifier: TaskClassifier,
    probe_engine: ProbeEngine,
    llm_service: LlmService,
}

impl GatewayExecutor {
    pub fn new(app_handle: AppHandle, registry: GatewayRegistry, llm_service: LlmService) -> Self {
        Self {
            app_handle,
            registry,
            classifier: TaskClassifier::new(),
            probe_engine: ProbeEngine::new(),
            llm_service,
        }
    }

    pub fn health_registry(&self) -> std::sync::Arc<std::sync::Mutex<HealthRegistry>> {
        self.probe_engine.registry()
    }

    /// 选择候选模型链（v0.15.0 三维打分：算力 50% + 偏好 30% + 适配 20%）
    pub fn select_candidates(
        &self,
        request: &GatewayRequest,
    ) -> Result<GatewayRoutingDecision, AppError> {
        let routing_request = self.classifier.classify(request);
        let router = crate::router::UnifiedModelRouter::new(self.registry.inner.clone());

        // 先获取基础路由决策与候选链
        let mut decision = router.route(&routing_request)?;

        // 应用健康权重与任务匹配微调
        let health_registry = self.health_registry();
        let health = health_registry.lock().ok();
        let mut re_scored: Vec<(f64, &crate::config::settings::LlmProfile)> = decision
            .candidates
            .iter()
            .filter_map(|c| self.registry.get(&c.model_id).map(|m| (c.score, m)))
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
                (score, m)
            })
            .collect();
        re_scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

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

        if let Some(primary) = candidates.first() {
            decision.model_id = primary.model_id.clone();
            decision.model_name = primary.model_name.clone();
        }
        decision.candidates = candidates;

        // v0.22.0: 算力档案消费闭环（Phase D）
        // v0.22.1: 按 TaskClass 应用差异化权重（意见5）
        //   HeavyCreation → 优先质量分（quality 80%）
        //   LightTool → 优先速度分（speed 60%）
        if let Some(pool) = self.app_handle.try_state::<crate::db::DbPool>() {
            if let Ok(profiles) =
                super::capability_store::CapabilityStore::new(pool.inner().clone()).load_all()
            {
                let task_class = TaskClassifier::classify_task(request);
                let mut candidates = decision.candidates.clone();
                for c in &mut candidates {
                    if let Some(cap) = profiles.iter().find(|p| p.model_id == c.model_id) {
                        let ttfb = cap.short_ttfb_ms_p50.unwrap_or(5000) as f64;
                        let tps = cap.sustained_tps.unwrap_or(10.0);
                        let success = cap.success_rate_24h.unwrap_or(0.9);
                        let cap_score = cap.capability_score.unwrap_or(0.5);

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
                candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                decision.candidates = candidates;
            }
        }

        Ok(decision)
    }

    /// 统一生成入口：选择候选链并顺序执行 fallback
    pub async fn generate(&self, request: GatewayRequest) -> Result<LlmGenerateResponse, AppError> {
        let decision = self.select_candidates(&request)?;

        if decision.candidates.is_empty() {
            return Err(AppError::Internal {
                message: "没有可用的模型候选".to_string(),
            });
        }

        let mut last_error: Option<AppError> = None;
        for (idx, candidate) in decision.candidates.iter().enumerate() {
            let Some(profile) = self.registry.get(&candidate.model_id) else {
                continue;
            };

            let generate_request = GenerateRequest {
                prompt: request.prompt.clone(),
                max_tokens: request.max_tokens,
                temperature: request.temperature,
                ..Default::default()
            };

            // 实际调用底层 LlmService 的按 profile 执行接口
            // TODO: 接入 LlmService::generate_with_profile_and_request_id
            let context_label = request.context_label.as_deref();
            let (_, result) = self
                .llm_service
                .generate_with_profile_and_request_id(
                    &profile.id,
                    request.prompt.clone(),
                    request.max_tokens,
                    request.temperature,
                    context_label,
                    Some(request.request_id.clone()),
                    request.timeout_seconds_override,
                    request.max_retries_override,
                )
                .await;
            match result {
                Ok(resp) => {
                    // 记录成功探测近似数据（实际由调用层记录）
                    return Ok(resp);
                }
                Err(e) => {
                    log::warn!(
                        "[Gateway] 模型 {} 调用失败（第 {} 候选）: {}",
                        candidate.model_id,
                        idx + 1,
                        e
                    );
                    last_error = Some(e);
                    continue;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| AppError::Internal {
            message: "所有候选模型均调用失败".to_string(),
        }))
    }

    /// 轻量探测：用极短 prompt 测试模型可用性
    pub async fn probe_model(&self, model_id: &str) -> Result<ProbeResult, AppError> {
        let Some(profile) = self.registry.get(model_id) else {
            return Err(AppError::Internal {
                message: format!("模型 {} 不存在", model_id),
            });
        };

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
                self.probe_engine.record_probe(profile, &result);
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
                self.probe_engine.record_probe(profile, &result);
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

        let pool = match crate::get_pool() {
            Some(p) => p,
            None => {
                log::error!("[GatewayExecutor] 无法获取 DB 连接池，基准跳过");
                return;
            }
        };
        let benchmarker = StreamBenchmark::new(pool.clone());
        let store = CapabilityStore::new(pool);

        for profile in &profiles {
            log::info!("[GatewayExecutor] 基准短任务 {}...", profile.name);
            let short = benchmarker.run_benchmark(profile, false).await;
            log::info!("[GatewayExecutor] 基准长任务 {}...", profile.name);
            let long = benchmarker.run_benchmark(profile, true).await;

            let cap = crate::model_gateway::types::CapabilityProfile {
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
