//! Model Gateway — 执行层与 fallback
//!
//! v0.14.0: 按候选链顺序执行，主模型失败时自动降级。

use tauri::AppHandle;

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

        let request = GenerateRequest {
            prompt: "Respond with exactly the word OK.".to_string(),
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
                let ttft_ms = duration_ms.saturating_sub(10); // 简化估计
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
}
