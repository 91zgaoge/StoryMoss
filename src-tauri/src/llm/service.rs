#![allow(dead_code)]
//! LLM Service - 统一的大语言模型服务
//!
//! 提供同步生成和流式生成两种模式
//! 支持多提供商配置管理和自动切换

use std::{
    collections::{hash_map::DefaultHasher, HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::sync::Semaphore;

/// 持有 tokio::task::AbortHandle 并在 drop 时调用 abort，
/// 确保心跳任务在 panic、early return 或任何错误路径下都能可靠终止。
struct AbortOnDrop(tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::timeout;

use super::{
    adapter::{GenerateRequest, GenerateResponse},
    anthropic::AnthropicAdapter,
    ollama::OllamaAdapter,
    openai::OpenAiAdapter,
};
use crate::{
    config::settings::{AppConfig, LlmProfile, LlmProvider},
    error::AppError,
    events::{emit_generation_status, GenerationPhase},
    memory::tokenizer::count_tokens,
    router::{
        Complexity, Priority, RoutingRequest, TaskType, UnifiedModelRegistry, UnifiedModelRouter,
    },
};

/// Prompt/Response 缓存键
#[derive(Debug, Clone)]
struct PromptCacheKey {
    provider: String,
    model: String,
    max_tokens: Option<i32>,
    temperature: Option<f32>,
    prompt_hash: u64,
}

impl PartialEq for PromptCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider
            && self.model == other.model
            && self.max_tokens == other.max_tokens
            && self.temperature.map(|f| f.to_bits()) == other.temperature.map(|f| f.to_bits())
            && self.prompt_hash == other.prompt_hash
    }
}

impl Eq for PromptCacheKey {}

impl Hash for PromptCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.provider.hash(state);
        self.model.hash(state);
        self.max_tokens.hash(state);
        self.temperature.map(|f| f.to_bits()).hash(state);
        self.prompt_hash.hash(state);
    }
}

/// 缓存条目
struct PromptCacheEntry {
    response: GenerateResponse,
    created_at: Instant,
}

/// Prompt/Response 缓存：对确定性请求（如 test_connection、风格分析）复用结果。
#[derive(Clone)]
pub struct PromptCache {
    inner: Arc<Mutex<HashMap<PromptCacheKey, PromptCacheEntry>>>,
    ttl: Duration,
    max_entries: usize,
}

impl PromptCache {
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            ttl,
            max_entries,
        }
    }

    fn key(
        profile: &LlmProfile,
        prompt: &str,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> PromptCacheKey {
        let mut hasher = DefaultHasher::new();
        prompt.hash(&mut hasher);
        PromptCacheKey {
            provider: format!("{:?}", profile.provider),
            model: profile.model.clone(),
            max_tokens,
            temperature,
            prompt_hash: hasher.finish(),
        }
    }

    pub fn get(
        &self,
        profile: &LlmProfile,
        prompt: &str,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Option<GenerateResponse> {
        let key = Self::key(profile, prompt, max_tokens, temperature);
        let mut inner = self.inner.lock().ok()?;
        if let Some(entry) = inner.get(&key) {
            if entry.created_at.elapsed() < self.ttl {
                log::debug!("[LlmService] Prompt cache hit");
                return Some(entry.response.clone());
            }
            inner.remove(&key);
        }
        None
    }

    pub fn put(
        &self,
        profile: &LlmProfile,
        prompt: &str,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        response: GenerateResponse,
    ) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if inner.len() >= self.max_entries {
            let oldest = inner
                .iter()
                .min_by_key(|(_, entry)| entry.created_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                inner.remove(&k);
            }
        }
        let key = Self::key(profile, prompt, max_tokens, temperature);
        inner.insert(
            key,
            PromptCacheEntry {
                response,
                created_at: Instant::now(),
            },
        );
    }
}

fn default_prompt_cache() -> PromptCache {
    PromptCache::new(Duration::from_secs(300), 100)
}

/// 流式生成事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub chunk: String,
    pub is_first: bool,
    pub is_last: bool,
    pub model: String,
}

/// 生成完成事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationComplete {
    pub full_text: String,
    pub model: String,
    pub tokens_used: i32,
    pub cost: f64,
    pub duration_ms: u64,
}

/// 生成错误事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationError {
    pub error: String,
    pub error_code: String,
}

/// LLM生成进度事件 — 携带Pipeline步骤上下文，让用户知道"当前在进行哪一步"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGeneratingProgress {
    pub stage: String, // "connecting" | "generating" | "completed" | "error"
    pub message: String,
    pub elapsed_seconds: u64,
    pub model: String,
    /// Pipeline步骤上下文（可选）— 用于Bootstrap等长流程，显示"步骤名 X/Y"
    pub pipeline_context: Option<PipelineContext>,
}

/// Pipeline步骤上下文 — 让进度消息显示当前所处的Pipeline步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineContext {
    pub step_name: String,
    pub step_number: usize,
    pub total_steps: usize,
    pub action: String,
}

/// 封装一次 LLM 调用记录所需的数据，避免 `record_llm_call` 参数过多。
struct LlmCallRecord<'a> {
    model_id: &'a str,
    model_name: Option<&'a str>,
    purpose: &'a str,
    prompt: &'a str,
    response: Option<&'a GenerateResponse>,
    duration_ms: u64,
    error: Option<&'a AppError>,
}

/// LLM服务 - 管理所有LLM调用
pub struct LlmService {
    app_handle: AppHandle,
    config: Arc<Mutex<AppConfig>>,
    cancel_senders: Arc<Mutex<HashMap<String, Option<tokio::sync::mpsc::Sender<()>>>>>,
    /// 按配置缓存适配器，避免每次调用重复创建 reqwest::Client
    adapter_cache: Arc<Mutex<HashMap<String, Box<dyn super::LlmAdapter>>>>,
    /// Prompt/Response 缓存，对确定性请求复用结果
    prompt_cache: PromptCache,
    /// 已被取消的 request_id 集合，用于协作式取消检查
    cancelled_requests: Arc<Mutex<HashSet<String>>>,
    /// 本地模型 Writer 全局并发限制（默认 1）
    writer_local_semaphore: Arc<Semaphore>,
    /// 远端模型 Writer 全局并发限制（默认 2）
    writer_remote_semaphore: Arc<Semaphore>,
}

impl LlmService {
    /// 创建新的 LLM 服务实例。
    ///
    /// 如果全局 LLM 服务已经初始化（通过
    /// `init_llm_service`），则直接返回其克隆， 从而复用 reqwest 连接池与
    /// adapter 缓存；否则创建新实例。
    pub fn new(app_handle: AppHandle) -> Self {
        // 优先返回全局共享实例，避免每次命令重建 client 与缓存。
        if let Some(service) = get_llm_service() {
            return service;
        }

        let app_dir = app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

        let config = AppConfig::load(&app_dir).unwrap_or_default();
        let writer_local_concurrency = config.writer_local_concurrency.max(1);
        let writer_remote_concurrency = config.writer_remote_concurrency.max(1);

        Self {
            app_handle,
            config: Arc::new(Mutex::new(config)),
            cancel_senders: Arc::new(Mutex::new(HashMap::new())),
            adapter_cache: Arc::new(Mutex::new(HashMap::new())),
            prompt_cache: default_prompt_cache(),
            cancelled_requests: Arc::new(Mutex::new(HashSet::new())),
            writer_local_semaphore: Arc::new(Semaphore::new(writer_local_concurrency)),
            writer_remote_semaphore: Arc::new(Semaphore::new(writer_remote_concurrency)),
        }
    }

    /// 获取 Writer 全局并发许可。本地模型与远端模型使用不同的 Semaphore，
    /// 默认分别为 1 和 2，避免本地服务端被多个 Writer 同时压垮。
    pub async fn acquire_writer_permit(
        &self,
        is_local: bool,
    ) -> Result<tokio::sync::SemaphorePermit<'_>, AppError> {
        let sem = if is_local {
            &self.writer_local_semaphore
        } else {
            &self.writer_remote_semaphore
        };
        sem.acquire()
            .await
            .map_err(|e| AppError::internal(format!("Failed to acquire writer permit: {}", e)))
    }

    /// 重新加载配置，同时清空适配器缓存
    pub fn reload_config(&self) {
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());

        match AppConfig::load(&app_dir) {
            Ok(config) => {
                if let Ok(mut guard) = self.config.lock() {
                    *guard = config;
                }
                if let Ok(mut cache) = self.adapter_cache.lock() {
                    cache.clear();
                }
            }
            Err(e) => {
                log::warn!("[LLM] Failed to reload config: {}", e);
            }
        }
    }

    /// 获取当前活跃的LLM配置
    pub fn get_active_profile(&self) -> Option<LlmProfile> {
        let guard = self.config.lock().ok()?;
        guard.get_active_llm_profile().cloned()
    }

    /// 获取指定ID的LLM配置（仅返回 enabled 的模型）
    fn get_profile_by_id(&self, profile_id: &str) -> Option<LlmProfile> {
        let guard = self.config.lock().ok()?;
        guard
            .llm_profiles
            .get(profile_id)
            .filter(|p| p.enabled)
            .cloned()
    }

    /// 根据路由请求选择最合适的 LLM 配置
    pub fn select_profile_for_request(
        &self,
        request: &RoutingRequest,
    ) -> Result<LlmProfile, AppError> {
        let guard = self.config.lock().map_err(|_| {
            AppError::internal("Failed to lock AppConfig while selecting model".to_string())
        })?;
        let registry = UnifiedModelRegistry::from_app_config(&*guard);
        let router = UnifiedModelRouter::new(registry);
        let decision = router.route(request)?;
        guard
            .llm_profiles
            .get(&decision.model_id)
            .cloned()
            .ok_or_else(|| AppError::NotFound {
                resource: "llm_profile".to_string(),
                id: decision.model_id.clone(),
            })
    }

    /// 根据任务请求生成文本：先路由选择模型，再调用指定模型生成
    pub async fn generate_for_request(
        &self,
        request: RoutingRequest,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
    ) -> Result<GenerateResponse, AppError> {
        let (_, result) = self
            .generate_for_request_with_request_id(
                request,
                prompt,
                max_tokens,
                temperature,
                context_label,
                None,
                None,
                None,
                None,
                None,
            )
            .await;
        result
    }

    /// 根据任务请求生成文本，返回 (request_id, Result)，支持取消
    ///
    /// v0.14.0: 优先通过 ModelGateway 执行，实现模型健康感知与自动 fallback。
    /// 若网关不可用则回退到旧版本地路由。
    pub async fn generate_for_request_with_request_id(
        &self,
        request: RoutingRequest,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        request_id: Option<String>,
        timeout_seconds_override: Option<u64>,
        max_retries_override: Option<u32>,
        // v0.22.0: SING 意图感知调度
        intent_verb: Option<&str>,
        intent_object: Option<&str>,
    ) -> (String, Result<GenerateResponse, AppError>) {
        let req_id = request_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // 优先尝试模型网关
        let gateway = self
            .app_handle
            .state::<crate::model_gateway::executor::GatewayExecutor>();
        let gateway_request = crate::model_gateway::types::GatewayRequest {
            prompt: prompt.clone(),
            agent_id: context_label.unwrap_or("llm_service").to_string(),
            task: request.task,
            complexity: Some(request.complexity),
            budget_priority: request.budget_priority,
            speed_priority: request.speed_priority,
            estimated_input_tokens: request.estimated_input_tokens,
            max_tokens,
            temperature,
            stream: false,
            request_id: req_id.clone(),
            context_label: context_label.map(|s| s.to_string()),
            timeout_seconds_override,
            max_retries_override,
            // v0.22.0: 注入意图动词-宾语，激活 classify_by_intention
            intent_verb: intent_verb.map(|s| s.to_string()),
            intent_object: intent_object.map(|s| s.to_string()),
        };
        match gateway.generate(gateway_request).await {
            Ok(resp) => return (req_id, Ok(resp)),
            Err(e) => {
                log::warn!("[LlmService] ModelGateway 调用失败，回退旧路径: {}", e);
            }
        }

        // 回退：旧版本地路由
        let profile = match self.select_profile_for_request(&request) {
            Ok(p) => p,
            Err(e) => return (req_id, Err(e)),
        };
        self.generate_with_profile_and_request_id(
            &profile.id,
            prompt,
            max_tokens,
            temperature,
            context_label,
            Some(req_id),
            None,
            None,
        )
        .await
    }

    /// 简化入口：仅指定任务类型，使用默认复杂度/优先级进行路由生成
    pub async fn generate_for_task(
        &self,
        task: TaskType,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
    ) -> Result<GenerateResponse, AppError> {
        let request = RoutingRequest {
            task,
            complexity: Complexity::Medium,
            budget_priority: Priority::Low,
            speed_priority: Priority::Low,
            estimated_input_tokens: 0,
            constraints: vec![],
        };
        self.generate_for_request(request, prompt, max_tokens, temperature, context_label)
            .await
    }

    /// 路由生成，支持 Pipeline 步骤上下文
    pub async fn generate_for_request_with_context_and_pipeline(
        &self,
        request: RoutingRequest,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        pipeline_ctx: Option<PipelineContext>,
    ) -> Result<GenerateResponse, AppError> {
        let profile = self.select_profile_for_request(&request)?;
        self.generate_with_profile_context_and_pipeline(
            &profile.id,
            prompt,
            max_tokens,
            temperature,
            context_label,
            pipeline_ctx,
        )
        .await
    }

    /// 计算实际生效的超时秒数
    ///
    /// 统一所有生成路径的超时策略：优先使用 profile 配置；若未设置（0）则回退到
    /// 项目默认 300 秒，避免本地大模型被 120 秒误超时。
    fn effective_timeout_seconds(profile: &LlmProfile) -> u64 {
        if profile.timeout_seconds > 0 {
            profile.timeout_seconds
        } else {
            crate::config::settings::DEFAULT_LLM_TIMEOUT_SECONDS
        }
    }

    /// 将本次 LLM 调用记录到 `llm_calls` 表，并输出结构化指标日志。
    ///
    /// 失败不返回错误，避免记录本身阻塞主流程。
    fn record_llm_call(&self, record: LlmCallRecord<'_>) {
        let prompt_len = record.prompt.chars().count() as i32;
        let model_family = record.model_id;
        let prompt_tokens = count_tokens(record.prompt, model_family) as i32;
        let completion_tokens = record.response.map(|r| r.tokens_used).unwrap_or(0);
        let total_tokens = prompt_tokens + completion_tokens;
        let provider = self
            .get_active_profile()
            .map(|p| format!("{:?}", p.provider));
        let cached = record.response.is_some() && prompt_tokens == 0;

        // 统一结构化指标日志，便于后续通过日志聚合分析耗时、超时、缓存命中率。
        let metrics = serde_json::json!({
            "event": "llm_call",
            "provider": provider,
            "model": record.model_id,
            "purpose": record.purpose,
            "prompt_len": prompt_len,
            "prompt_tokens": prompt_tokens,
            "completion_tokens": completion_tokens,
            "total_tokens": total_tokens,
            "duration_ms": record.duration_ms,
            "success": record.error.is_none(),
            "error_type": record.error.map(|e| format!("{:?}", std::mem::discriminant(e))),
            "cached": cached,
        });
        log::info!(target: "llm_metrics", "{}", metrics);

        if let Some(pool) = self.app_handle.try_state::<crate::db::DbPool>() {
            use crate::db::{repositories_pipeline::LlmCallRepository, RecordLlmCallRequest};
            let repo = LlmCallRepository::new(pool.inner().clone());
            let req = RecordLlmCallRequest {
                story_id: None,
                draft_id: None,
                model_id: record.model_id.to_string(),
                model_name: record.model_name.map(|s| s.to_string()),
                purpose: record.purpose.to_string(),
                prompt_tokens,
                completion_tokens,
                duration_ms: record.duration_ms as i32,
                success: record.error.is_none(),
                error_message: record.error.map(|e| e.to_string()),
                // v0.11.0: 路由与审核字段，后续通过 feedback 闭环回填
                task_type: None,
                quality_score: None,
                latency_ms: Some(record.duration_ms as i32),
                route_decision: None,
                audit_feedback: None,
            };
            let preview = if record.prompt.len() > 200 {
                &record.prompt[..200]
            } else {
                record.prompt
            };
            let metadata = serde_json::json!({
                "provider": provider,
                "cached": cached,
            })
            .to_string();
            if let Err(e) = repo.create(
                req,
                total_tokens,
                record.duration_ms as i32,
                Some(preview),
                Some(&metadata),
            ) {
                log::warn!("[LLM] Failed to record llm_call: {}", e);
            }
        }
    }

    /// 判断错误是否可重试（超时/网络/5xx）
    fn is_retriable_error(error: &AppError) -> bool {
        match error {
            // v0.11.8: 连接阶段超时可重试 1 次；生成阶段超时不可重试。
            AppError::LlmConnectionTimeout { .. } => true,
            AppError::LlmGenerationTimeout { .. } => false,
            // v0.11.2: 模型响应超时通常是服务端/本地模型无法及时处理，重试只会
            // 让用户等待更久（候选阶段 120s 超时 × 重试会轻松超过 500s）。
            // 因此超时错误不再重试，立即反馈给用户。
            AppError::LlmTimeout { .. } => false,
            AppError::Internal { message, .. } => {
                let m = message.to_lowercase();
                m.contains("timeout")
                    || m.contains("connection")
                    || m.contains("network")
                    || m.contains("5")
                    || m.contains("502")
                    || m.contains("503")
                    || m.contains("504")
            }
            _ => false,
        }
    }

    /// 适配器缓存键
    fn adapter_cache_key(profile: &LlmProfile) -> String {
        format!(
            "{:?}|{}|{:?}|{}|{}|{}",
            profile.provider,
            profile.model,
            profile.api_base,
            profile.max_tokens,
            profile.temperature,
            profile.timeout_seconds
        )
    }

    /// 创建适配器（优先从缓存获取）
    fn create_adapter(&self, profile: &LlmProfile) -> Result<Box<dyn super::LlmAdapter>, AppError> {
        let key = Self::adapter_cache_key(profile);
        if let Ok(cache) = self.adapter_cache.lock() {
            if let Some(adapter) = cache.get(&key) {
                log::debug!("[LLM] Reusing cached adapter for key: {}", key);
                return Ok(adapter.box_clone());
            }
        }

        let timeout_seconds = Self::effective_timeout_seconds(profile);
        // v0.15.5: 从 AppConfig 读取，默认 30s
        let connect_timeout_seconds = self
            .config
            .lock()
            .ok()
            .map(|c| c.llm_connect_timeout_secs)
            .unwrap_or(30u64);

        let adapter: Box<dyn super::LlmAdapter> = match profile.provider {
            LlmProvider::OpenAI
            | LlmProvider::Custom
            | LlmProvider::DeepSeek
            | LlmProvider::Qwen => Box::new(OpenAiAdapter::new(
                profile.api_key.clone(),
                profile.model.clone(),
                profile.api_base.clone(),
                profile.max_tokens,
                profile.temperature,
                timeout_seconds,
                connect_timeout_seconds,
            )),
            LlmProvider::Anthropic => Box::new(AnthropicAdapter::new(
                profile.api_key.clone(),
                profile.model.clone(),
                profile.api_base.clone(),
                profile.max_tokens,
                profile.temperature,
                timeout_seconds,
                connect_timeout_seconds,
            )),
            LlmProvider::Ollama => Box::new(OllamaAdapter::new(
                profile.api_key.clone(),
                profile.model.clone(),
                profile.api_base.clone(),
                profile.max_tokens,
                profile.temperature,
                timeout_seconds,
                connect_timeout_seconds,
            )),
            _ => {
                log::error!("[LLM] Unsupported provider: {:?}", profile.provider);
                return Err(AppError::validation_failed(
                    format!("Provider {:?} not supported", profile.provider),
                    Some("provider"),
                ));
            }
        };

        if let Ok(mut cache) = self.adapter_cache.lock() {
            cache.insert(key, adapter.box_clone());
        }
        Ok(adapter)
    }

    /// 发送 LLM 生成进度事件
    fn emit_llm_progress(
        &self,
        stage: &str,
        message: &str,
        elapsed_seconds: u64,
        model: &str,
        pipeline_ctx: Option<&PipelineContext>,
        request_id: Option<&str>,
    ) {
        let _ = self.app_handle.emit(
            "llm-generating-progress",
            LlmGeneratingProgress {
                stage: stage.to_string(),
                message: message.to_string(),
                elapsed_seconds,
                model: model.to_string(),
                pipeline_context: pipeline_ctx.cloned(),
            },
        );

        // C1: 同时发射统一生成状态事件，使用 request_id 作为 task_id 标识
        if let Some(req_id) = request_id {
            let phase = match stage {
                "connecting" | "sent" => GenerationPhase::PreparingContext,
                "generating" => GenerationPhase::GeneratingCandidates,
                "completed" => GenerationPhase::FinalOutput,
                _ => GenerationPhase::GeneratingCandidates,
            };
            emit_generation_status(
                &self.app_handle,
                req_id,
                phase,
                if stage == "completed" { 1.0 } else { 0.4 },
                message,
                Some(req_id.to_string()),
            );
        }
    }

    /// 同步生成文本（带上下文描述 + 600秒整体超时 + 心跳进度）
    pub async fn generate(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<GenerateResponse, AppError> {
        log::info!("[LLM] generate() called");
        let (_, result) = self
            .generate_with_request_id(prompt, max_tokens, temperature, None, None, None)
            .await;
        result
    }

    /// 同步生成文本，支持上下文描述
    pub async fn generate_with_context(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
    ) -> Result<GenerateResponse, AppError> {
        let (_, result) = self
            .generate_with_request_id(prompt, max_tokens, temperature, context_label, None, None)
            .await;
        result
    }

    /// 同步生成文本，支持上下文描述 + Pipeline步骤上下文
    pub async fn generate_with_context_and_pipeline(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        pipeline_ctx: Option<PipelineContext>,
    ) -> Result<GenerateResponse, AppError> {
        let (_, result) = self
            .generate_with_request_id(
                prompt,
                max_tokens,
                temperature,
                context_label,
                pipeline_ctx,
                None,
            )
            .await;
        result
    }

    /// 同步生成核心逻辑：使用指定 profile，支持 pipeline 上下文与 request_id。
    async fn execute_generation(
        &self,
        profile: LlmProfile,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        pipeline_ctx: Option<PipelineContext>,
        request_id: Option<String>,
        timeout_seconds_override: Option<u64>,
        max_retries_override: Option<u32>,
    ) -> (String, Result<GenerateResponse, AppError>) {
        if !profile.enabled {
            let msg = format!("模型 {} 已被禁用，请在设置中启用或切换可用模型", profile.id);
            log::warn!(
                "[LLM] Refusing generation for disabled profile: {}",
                profile.id
            );
            return (
                request_id.unwrap_or_default(),
                Err(AppError::validation_failed(msg, Some("model_disabled"))),
            );
        }

        let model_name = profile.model.clone();
        let provider = profile.provider.clone();
        let pipeline_ref = pipeline_ctx.as_ref();
        let effective_timeout =
            timeout_seconds_override.unwrap_or_else(|| Self::effective_timeout_seconds(&profile));
        let effective_retries = max_retries_override.unwrap_or(2u32);

        // v0.11.8: 让 adapter 内部超时使用 override 值，这样移除外层
        // tokio::time::timeout 后仍然能保证单候选/整体超时策略生效。
        let mut profile_for_adapter = profile.clone();
        if let Some(t) = timeout_seconds_override {
            profile_for_adapter.timeout_seconds = t;
        }

        log::info!(
            target: "llm_metrics",
            "[LLM] execute_generation start: model={}, provider={:?}, timeout_seconds={}, max_retries={}",
            model_name,
            provider,
            effective_timeout,
            effective_retries
        );
        let adapter = match self.create_adapter(&profile_for_adapter) {
            Ok(a) => a,
            Err(e) => {
                log::error!(
                    "[LLM] Failed to create adapter for provider {:?}: {}",
                    provider,
                    e
                );
                return (request_id.unwrap_or_default(), Err(e));
            }
        };

        let req = GenerateRequest {
            prompt,
            max_tokens,
            temperature,
            top_p: profile.top_p,
            frequency_penalty: profile.frequency_penalty,
            presence_penalty: profile.presence_penalty,
        };

        let label = context_label.unwrap_or("");
        // v0.14.4: 识别后台静默调用（如模型健康探测），跳过心跳发射避免误导前端
        // 模型探测在应用启动时自动触发，不应让前端误以为用户的生成任务正在进行
        // v0.16.2: 时间线 2/3 的后台审计与洞察 LLM 调用同样静默——主流程已发出
        // GenerationPhase::Completed，若它们继续 emit 普通 progress event
        // 会让前端误以为主流程仍在跑（实测「写第二章」场景下被这些后台事件
        // 拖到 200s 假超时）。
        let is_silent_background = matches!(
            label,
            "model_gateway_probe"
                | "input_hint"
                | "intent_detection"
                | "async-audit-inspector"
                | "async-insight"
                | "async-deep-insight"
                | "background-summary"
        );
        let step_prefix = pipeline_ref
            .map(|p| format!("[{} {}/{}] ", p.step_name, p.step_number, p.total_steps))
            .unwrap_or_default();

        let connecting_msg = if label.is_empty() {
            format!("{}正在连接模型...", step_prefix)
        } else {
            format!("{}正在连接模型 [{}]...", step_prefix, label)
        };
        let sent_msg = if label.is_empty() {
            format!("{}已发送请求，等待响应...", step_prefix)
        } else {
            format!("{}已发送请求 [{}]，等待响应...", step_prefix, label)
        };
        let completed_msg = if label.is_empty() {
            format!("{}AI 响应完成", step_prefix)
        } else {
            format!("{}{} 完成", step_prefix, label)
        };

        if !is_silent_background {
            self.emit_llm_progress(
                "connecting",
                &connecting_msg,
                0,
                &model_name,
                pipeline_ref,
                request_id.as_deref(),
            );
        }

        // 启动心跳任务
        // v0.14.4: 后台静默调用（如 model_gateway_probe）跳过心跳，
        // 避免应用启动时探测请求误触发前端"AI 正在深度思考中..."状态栏
        let app_handle = self.app_handle.clone();
        let model = model_name.clone();
        let label_owned = label.to_string();
        let pipeline_ctx_for_heartbeat = pipeline_ctx.clone();
        let heartbeat_handle = if is_silent_background {
            tokio::spawn(async {
                // 静默调用：不发心跳
            })
        } else {
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(10));
                let start = std::time::Instant::now();
                loop {
                    interval.tick().await;
                    let elapsed = start.elapsed().as_secs();
                    let step_prefix_hb = pipeline_ctx_for_heartbeat
                        .as_ref()
                        .map(|p| format!("[{} {}/{}] ", p.step_name, p.step_number, p.total_steps))
                        .unwrap_or_default();
                    let message = if label_owned.is_empty() {
                        format!(
                            "{}AI 正在深度思考中...（已等待 {} 秒）",
                            step_prefix_hb, elapsed
                        )
                    } else {
                        format!(
                            "{}正在{}...（已等待 {} 秒）",
                            step_prefix_hb, label_owned, elapsed
                        )
                    };
                    let emit_result = app_handle.emit(
                        "llm-generating-progress",
                        LlmGeneratingProgress {
                            stage: "generating".to_string(),
                            message: message.clone(),
                            elapsed_seconds: elapsed,
                            model: model.clone(),
                            pipeline_context: pipeline_ctx_for_heartbeat.clone(),
                        },
                    );
                    // v0.13.2: 用 warn! 级别记录心跳，确保无论日志过滤设置如何都能输出
                    // 如果 emit 失败（如序列化错误），同步记录错误原因
                    if let Err(e) = &emit_result {
                        log::warn!("[Heartbeat] emit FAILED after {}s: {}", elapsed, e);
                    } else {
                        log::warn!(
                            "[Heartbeat] fired: elapsed={}s msg=\"{}\"",
                            elapsed,
                            message
                        );
                    }
                    // v0.11.5: 不再在 600 秒后停止心跳。只要生成仍在继续，
                    // 前端就应该持续收到进度反馈；生成结束时 heartbeat_handle
                    // 会被 abort。
                }
            })
        };
        let _heartbeat_guard = AbortOnDrop(heartbeat_handle.abort_handle());

        if !is_silent_background {
            self.emit_llm_progress(
                "sent",
                &sent_msg,
                0,
                &model_name,
                pipeline_ref,
                request_id.as_deref(),
            );
        }

        // Wave 1: 注册取消通道（同步生成也支持取消）
        let request_id = request_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let (cancel_tx, mut cancel_rx) = tokio::sync::mpsc::channel::<()>(1);
        {
            let mut senders = self.cancel_senders.lock().unwrap();
            senders.insert(request_id.clone(), Some(cancel_tx));
        }

        let start_time = std::time::Instant::now();
        let timeout_seconds =
            timeout_seconds_override.unwrap_or_else(|| Self::effective_timeout_seconds(&profile));
        let max_retries = max_retries_override.unwrap_or(2u32);
        // v0.15.5: 从 AppConfig 读取，默认 30s
        let connect_timeout_seconds = self
            .config
            .lock()
            .ok()
            .map(|c| c.llm_connect_timeout_secs)
            .unwrap_or(30u64);

        // v0.11.8: 超时与重试策略
        // - adapter.generate 内部已拆分连接超时（10s）与生成超时（timeout_seconds），
        //   并在读取响应流时按 chunk 刷新计时器。
        // - 连接超时最多重试 1 次；生成超时/其他不可重试错误直接返回。
        let mut result: Result<GenerateResponse, AppError> =
            Err(AppError::internal("Generation did not run".to_string()));
        for attempt in 0..=max_retries {
            let adapter = adapter.box_clone();
            let req = req.clone();
            let attempt_result = tokio::select! {
                r = adapter.generate(req) => {
                    match r {
                        Ok(resp) => Ok(resp),
                        Err(e) => {
                            let err_msg = e.to_string();
                            if err_msg.contains(super::adapter::CONNECTION_TIMEOUT_MARKER) {
                                Err(AppError::internal(format!(
                                    "无法连接到模型 {}（{}ms 内未响应），请检查该模型的服务端是否正常运行",
                                    model_name, connect_timeout_seconds * 1000
                                )))
                            } else if err_msg.contains(super::adapter::GENERATION_TIMEOUT_MARKER) {
                                Err(AppError::llm_generation_timeout(timeout_seconds * 1000))
                            } else {
                                Err(AppError::internal(format!("Generation failed: {}", e)))
                            }
                        }
                    }
                }
                _ = cancel_rx.recv() => {
                    log::info!("[LLM] Generation cancelled for request_id: {}", request_id);
                    Err(AppError::cancelled("生成已取消"))
                }
            };

            match attempt_result {
                Ok(resp) => {
                    result = Ok(resp);
                    break;
                }
                Err(e) => {
                    result = Err(e.clone());
                    let is_connection_timeout = matches!(e, AppError::LlmConnectionTimeout { .. });
                    let can_retry = if is_connection_timeout {
                        // 连接超时仅额外重试 1 次（attempt 0 -> attempt 1）
                        attempt == 0
                    } else {
                        Self::is_retriable_error(&e) && attempt < max_retries
                    };
                    if !can_retry {
                        break;
                    }
                    // 指数退避：500ms, 1000ms, 2000ms...
                    let backoff_ms = 500u64 * 2u64.pow(attempt);
                    log::info!(
                        "[LLM] Retrying generation in {}ms (attempt {})",
                        backoff_ms,
                        attempt + 2
                    );
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                }
            }
        }

        let _ = self
            .cancel_senders
            .lock()
            .unwrap()
            .remove(&request_id)
            .flatten();

        heartbeat_handle.abort();
        let _ = heartbeat_handle.await;

        match result {
            Ok(response) => {
                let duration = start_time.elapsed().as_millis() as u64;
                log::info!(
                    "[LLM] Sync generation completed in {}ms response_len={}",
                    duration,
                    response.content.len()
                );
                self.record_llm_call(LlmCallRecord {
                    model_id: &model_name,
                    model_name: Some(&model_name),
                    purpose: label,
                    prompt: &req.prompt,
                    response: Some(&response),
                    duration_ms: duration,
                    error: None,
                });
                if !is_silent_background {
                    self.emit_llm_progress(
                        "completed",
                        &completed_msg,
                        0,
                        &model_name,
                        pipeline_ref,
                        Some(request_id.as_str()),
                    );
                }
                (request_id.clone(), Ok(response))
            }
            Err(e) => {
                let is_timeout = matches!(
                    e,
                    AppError::LlmTimeout { .. }
                        | AppError::LlmConnectionTimeout { .. }
                        | AppError::LlmGenerationTimeout { .. }
                );
                let duration = start_time.elapsed().as_millis() as u64;
                self.record_llm_call(LlmCallRecord {
                    model_id: &model_name,
                    model_name: Some(&model_name),
                    purpose: label,
                    prompt: &req.prompt,
                    response: None,
                    duration_ms: duration,
                    error: Some(&e),
                });
                if !is_silent_background {
                    self.emit_llm_progress(
                        "error",
                        &e.to_string(),
                        if is_timeout { 600 } else { 0 },
                        &model_name,
                        pipeline_ref,
                        Some(request_id.as_str()),
                    );
                }
                (request_id.clone(), Err(e))
            }
        }
    }

    /// 同步生成文本，返回 (request_id, Result) — 供上层取消使用
    ///
    /// `request_id`: 上层传入的取消标识；为 None 时内部生成 UUID。
    pub async fn generate_with_request_id(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        pipeline_ctx: Option<PipelineContext>,
        request_id: Option<String>,
    ) -> (String, Result<GenerateResponse, AppError>) {
        let profile = match self.get_active_profile() {
            Some(p) => p,
            None => {
                log::error!("[LLM] Active profile not found");
                return (
                    request_id.unwrap_or_default(),
                    Err(AppError::internal("No active LLM profile configured")),
                );
            }
        };
        self.execute_generation(
            profile,
            prompt,
            max_tokens,
            temperature,
            context_label,
            pipeline_ctx,
            request_id,
            None,
            None,
        )
        .await
    }

    /// 使用指定模型配置同步生成文本（带统一超时 + 心跳进度）
    pub async fn generate_with_profile(
        &self,
        profile_id: &str,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<GenerateResponse, AppError> {
        let (_, result) = self
            .generate_with_profile_and_request_id(
                profile_id,
                prompt,
                max_tokens,
                temperature,
                None,
                None,
                None,
                None,
            )
            .await;
        result
    }

    /// 使用指定模型配置同步生成文本，支持上下文描述
    pub async fn generate_with_profile_and_context(
        &self,
        profile_id: &str,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
    ) -> Result<GenerateResponse, AppError> {
        let (_, result) = self
            .generate_with_profile_and_request_id(
                profile_id,
                prompt,
                max_tokens,
                temperature,
                context_label,
                None,
                None,
                None,
            )
            .await;
        result
    }

    /// 使用指定模型配置同步生成文本，返回 (request_id, Result) — 供上层取消使用
    pub async fn generate_with_profile_and_request_id(
        &self,
        profile_id: &str,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        request_id: Option<String>,
        timeout_seconds_override: Option<u64>,
        max_retries_override: Option<u32>,
    ) -> (String, Result<GenerateResponse, AppError>) {
        log::info!(
            "[LLM] Starting sync generation with profile={} prompt_len={}",
            profile_id,
            prompt.len()
        );

        let profile = match self.get_profile_by_id(profile_id) {
            Some(p) => p,
            None => {
                log::error!("[LLM] Active profile not found: {}", profile_id);
                return (
                    request_id.unwrap_or_default(),
                    Err(AppError::not_found("llm_profile", profile_id)),
                );
            }
        };

        self.execute_generation(
            profile,
            prompt,
            max_tokens,
            temperature,
            context_label,
            None,
            request_id,
            timeout_seconds_override,
            max_retries_override,
        )
        .await
    }

    /// 使用指定模型配置同步生成文本，支持上下文描述 + Pipeline步骤上下文
    pub async fn generate_with_profile_context_and_pipeline(
        &self,
        profile_id: &str,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        pipeline_ctx: Option<PipelineContext>,
    ) -> Result<GenerateResponse, AppError> {
        let (_, result) = self
            .generate_with_profile_context_and_pipeline_and_request_id(
                profile_id,
                prompt,
                max_tokens,
                temperature,
                context_label,
                pipeline_ctx,
                None,
                None,
                None,
            )
            .await;
        result
    }

    /// 使用指定模型配置同步生成文本，支持 Pipeline 步骤上下文，返回
    /// (request_id, Result)
    pub async fn generate_with_profile_context_and_pipeline_and_request_id(
        &self,
        profile_id: &str,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        pipeline_ctx: Option<PipelineContext>,
        request_id: Option<String>,
        timeout_seconds_override: Option<u64>,
        max_retries_override: Option<u32>,
    ) -> (String, Result<GenerateResponse, AppError>) {
        log::info!(
            "[LLM] Starting sync generation with profile={} prompt_len={}",
            profile_id,
            prompt.len()
        );

        let profile = match self.get_profile_by_id(profile_id) {
            Some(p) => p,
            None => {
                log::error!("[LLM] Active profile not found: {}", profile_id);
                return (
                    request_id.unwrap_or_default(),
                    Err(AppError::not_found("llm_profile", profile_id)),
                );
            }
        };

        self.execute_generation(
            profile,
            prompt,
            max_tokens,
            temperature,
            context_label,
            pipeline_ctx,
            request_id,
            timeout_seconds_override,
            max_retries_override,
        )
        .await
    }

    /// 流式生成文本（启动 30 秒超时 + chunk 60 秒超时）
    ///
    /// 通过Tauri事件向前端发送生成进度
    /// 事件名称: `llm-stream-chunk`, `llm-stream-complete`, `llm-stream-error`
    pub async fn generate_stream(
        &self,
        request_id: String,
        prompt: String,
        context: Option<String>,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<(), AppError> {
        let start_time = std::time::Instant::now();

        let profile = self.get_active_profile().ok_or_else(|| {
            AppError::validation_failed(
                "没有可用的活跃语言模型，请先在设置中启用模型",
                Some("no_active_model"),
            )
        })?;

        if !profile.enabled {
            return Err(AppError::validation_failed(
                format!("活跃模型 {} 已被禁用，请在设置中启用或切换", profile.id),
                Some("model_disabled"),
            ));
        }

        // 构建增强提示词
        let enhanced_prompt = self.build_writing_prompt(&prompt, context.as_deref());

        log::info!(
            "[LLM] Starting stream generation with request_id: {}",
            request_id
        );
        log::debug!(
            "[LLM] Prompt: {}...",
            &enhanced_prompt[..enhanced_prompt.len().min(100)]
        );

        let adapter = self.create_adapter(&profile)?;

        let request = GenerateRequest {
            prompt: enhanced_prompt,
            max_tokens,
            temperature,
            top_p: profile.top_p,
            frequency_penalty: profile.frequency_penalty,
            presence_penalty: profile.presence_penalty,
        };

        // 流式首 chunk 超时：本地模型冷启动可能需要更久，按 profile 超时动态计算，
        // 最低 30 秒、最高 120 秒，避免硬编码 30 秒误杀本地大模型。
        let startup_timeout_seconds = Self::effective_timeout_seconds(&profile).min(120).max(30);
        let mut rx = timeout(
            Duration::from_secs(startup_timeout_seconds),
            adapter.generate_stream(request),
        )
        .await
        .map_err(|_| AppError::llm_timeout(startup_timeout_seconds * 1000))?
        .map_err(|e| AppError::internal(format!("Stream setup failed: {}", e)))?;

        let mut full_text = String::new();
        let mut is_first = true;

        // 注册取消通道
        let (cancel_tx, mut cancel_rx) = tokio::sync::mpsc::channel::<()>(1);
        {
            let mut senders = self.cancel_senders.lock().unwrap();
            senders.insert(request_id.clone(), Some(cancel_tx));
        }

        // chunk 超时：60 秒没有收到新数据就中断（与本地大模型实际速度匹配）
        let chunk_timeout_seconds = 60u64;
        let chunk_timeout = Duration::from_secs(chunk_timeout_seconds);

        // 流式 batching：聚合高频小 chunk 再跨 webview 发送，降低 IPC 开销。
        // 首 chunk 立即发送；后续按 80ms 或 40 字符批量发送。
        const BATCH_MAX_CHARS: usize = 40;
        const BATCH_TIMEOUT: Duration = Duration::from_millis(80);
        let mut buffer = String::with_capacity(BATCH_MAX_CHARS);
        let mut flush_deadline: Option<Instant> = None;

        let flush_buffer =
            |buffer: &mut String, full_text: &mut String, is_first: &mut bool, is_last: bool| {
                if buffer.is_empty() && !is_last {
                    return;
                }
                full_text.push_str(buffer);
                let chunk = std::mem::take(buffer);
                let stream_chunk = StreamChunk {
                    chunk,
                    is_first: *is_first,
                    is_last,
                    model: profile.model.clone(),
                };
                let _ = self
                    .app_handle
                    .emit(&format!("llm-stream-chunk-{}", request_id), stream_chunk);
                *is_first = false;
            };

        loop {
            let deadline_sleep = async {
                match flush_deadline {
                    Some(d) => {
                        let now = Instant::now();
                        if d > now {
                            tokio::time::sleep(d - now).await;
                        }
                    }
                    None => std::future::pending::<()>().await,
                }
            };

            tokio::select! {
                biased;
                _ = cancel_rx.recv() => {
                    log::info!("[LLM] Generation cancelled for request_id: {}", request_id);
                    flush_buffer(&mut buffer, &mut full_text, &mut is_first, false);
                    break;
                }
                _ = deadline_sleep, if flush_deadline.is_some() => {
                    flush_buffer(&mut buffer, &mut full_text, &mut is_first, false);
                    flush_deadline = None;
                }
                chunk_result = timeout(chunk_timeout, rx.recv()) => {
                    match chunk_result {
                        Ok(Some(Ok(chunk))) => {
                            buffer.push_str(&chunk);
                            if is_first || buffer.len() >= BATCH_MAX_CHARS {
                                flush_buffer(&mut buffer, &mut full_text, &mut is_first, false);
                                flush_deadline = None;
                            } else if flush_deadline.is_none() {
                                flush_deadline = Some(Instant::now() + BATCH_TIMEOUT);
                            }
                        }
                        Ok(Some(Err(e))) => {
                            flush_buffer(&mut buffer, &mut full_text, &mut is_first, false);
                            let _ = self.cancel_senders.lock().unwrap().remove(&request_id);
                            let error = GenerationError {
                                error: e.to_string(),
                                error_code: "STREAM_ERROR".to_string(),
                            };
                            let _ = self.app_handle.emit(&format!("llm-stream-error-{}", request_id), error);
                            return Err(AppError::internal(format!("Stream error: {}", e)));
                        }
                        Ok(None) => {
                            flush_buffer(&mut buffer, &mut full_text, &mut is_first, true);
                            break;
                        }
                        Err(_) => {
                            // chunk 超时
                            flush_buffer(&mut buffer, &mut full_text, &mut is_first, false);
                            let _ = self.cancel_senders.lock().unwrap().remove(&request_id);
                            let error = GenerationError {
                                error: format!("模型响应超时（{}秒内未收到新数据）", chunk_timeout_seconds),
                                error_code: "CHUNK_TIMEOUT".to_string(),
                            };
                            let _ = self.app_handle.emit(&format!("llm-stream-error-{}", request_id), error);
                            return Err(AppError::llm_timeout(chunk_timeout_seconds * 1000));
                        }
                    }
                }
            }
        }

        let _ = self.cancel_senders.lock().unwrap().remove(&request_id);
        self.clear_cancellation(&request_id);

        // 发送完成事件
        let duration = start_time.elapsed().as_millis() as u64;
        let complete = GenerationComplete {
            full_text: full_text.clone(),
            model: profile.model.clone(),
            tokens_used: full_text.len() as i32 / 2, // 粗略估计
            cost: 0.001,                             // 粗略估计
            duration_ms: duration,
        };

        let _ = self
            .app_handle
            .emit(&format!("llm-stream-complete-{}", request_id), complete);

        log::info!("[LLM] Stream generation completed in {}ms", duration);

        Ok(())
    }

    /// 构建写作专用提示词
    fn build_writing_prompt(&self, user_input: &str, context: Option<&str>) -> String {
        build_writing_prompt(user_input, context)
    }

    /// 带 Prompt/Response 缓存的同步生成（主要用于 test_connection
    /// 等确定性请求）
    async fn generate_cached(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<GenerateResponse, AppError> {
        let profile = match self.get_active_profile() {
            Some(p) => p,
            None => {
                return Err(AppError::internal("No active LLM profile configured"));
            }
        };

        if let Some(cached) = self
            .prompt_cache
            .get(&profile, &prompt, max_tokens, temperature)
        {
            return Ok(cached);
        }

        let response = self
            .generate(prompt.clone(), max_tokens, temperature)
            .await?;
        self.prompt_cache
            .put(&profile, &prompt, max_tokens, temperature, response.clone());
        Ok(response)
    }

    /// 测试连接
    pub async fn test_connection(&self) -> Result<(bool, u64), AppError> {
        let profile = self
            .get_active_profile()
            .ok_or_else(|| AppError::internal("No active LLM profile configured"))?;

        let base_url = profile.api_base.as_deref().unwrap_or("default");
        log::debug!("[LLM] Testing connection to {}", base_url);

        let start = std::time::Instant::now();

        // 发送一个简单的测试请求；使用缓存避免频繁测试时重复调用模型
        let test_prompt = "Hello, respond with 'OK' only.";

        match self
            .generate_cached(test_prompt.to_string(), Some(10), Some(0.0))
            .await
        {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                log::info!("[LLM] Connection test passed for {}", base_url);
                Ok((true, latency))
            }
            Err(e) => {
                log::warn!("[LLM] Connection test failed: {}", e);
                Err(e)
            }
        }
    }

    /// 取消指定 request_id 的流式生成，并传播取消信号到关联的后台 ingest 任务。
    pub fn cancel_generation(&self, request_id: &str) {
        {
            let mut cancelled = self.cancelled_requests.lock().unwrap();
            cancelled.insert(request_id.to_string());
        }
        let mut senders = self.cancel_senders.lock().unwrap();
        if let Some(opt_sender) = senders.get_mut(request_id) {
            if let Some(sender) = opt_sender.take() {
                let _ = sender.try_send(());
                log::info!("[LLM] Cancel signal sent for request_id: {}", request_id);
            } else {
                log::info!(
                    "[LLM] Cancel already requested for request_id: {}",
                    request_id
                );
            }
        } else {
            log::info!(
                "[LLM] No active generation found for request_id: {}",
                request_id
            );
        }

        // C2: 传播取消到该 request_id 对应的后台记忆 ingest 任务。
        crate::memory::writer::cancel_ingest_token(request_id);
    }

    /// v0.14.0: 取消所有进行中的 LLM 生成。
    ///
    /// 在 `smart_execute` 整体超时或前端超时时调用，确保不会留下孤儿 LLM 任务
    /// 继续占用模型服务端资源。遍历所有已注册的 `cancel_senders` 发送取消信号。
    pub fn cancel_all_generations(&self) {
        let mut senders = self.cancel_senders.lock().unwrap();
        let count = senders.len();
        if count == 0 {
            return;
        }
        log::warn!(
            "[LLM] Cancelling all {} in-flight generation(s) due to timeout/cancel",
            count
        );
        for (request_id, opt_sender) in senders.iter_mut() {
            // 标记为已取消
            self.cancelled_requests
                .lock()
                .unwrap()
                .insert(request_id.clone());
            if let Some(sender) = opt_sender.take() {
                let _ = sender.try_send(());
            }
            // 传播取消到后台 ingest
            crate::memory::writer::cancel_ingest_token(request_id);
        }
        senders.clear();
    }

    /// 检查指定 request_id 是否已被请求取消
    pub fn is_cancelled(&self, request_id: &str) -> bool {
        self.cancelled_requests.lock().unwrap().contains(request_id)
    }

    /// 清理已完成的 request_id 取消记录（可选，防止集合无限增长）
    pub fn clear_cancellation(&self, request_id: &str) {
        let mut cancelled = self.cancelled_requests.lock().unwrap();
        cancelled.remove(request_id);
    }

    /// 解析当前用户ID（从 .machine_id 文件）
    fn resolve_user_id(&self) -> Option<String> {
        let app_dir = self.app_handle.path().app_data_dir().ok()?;
        let machine_id_path = app_dir.join(".machine_id");
        if machine_id_path.exists() {
            std::fs::read_to_string(&machine_id_path)
                .ok()
                .map(|s| s.trim().to_string())
        } else {
            None
        }
    }
}

/// 构建写作专用提示词（纯函数，无需 self）
fn build_writing_prompt(user_input: &str, context: Option<&str>) -> String {
    let mut prompt = String::new();

    // 系统提示
    prompt.push_str("你是一位专业的小说创作助手，擅长中文写作。\n\n");

    // 上下文
    if let Some(ctx) = context {
        prompt.push_str("【前文上下文】\n");
        prompt.push_str(ctx);
        prompt.push_str("\n\n");
    }

    // 用户输入
    prompt.push_str("【续写要求】\n");
    prompt.push_str(user_input);
    prompt.push_str("\n\n");

    // 输出要求
    prompt.push_str("请直接输出续写内容，不要添加解释。保持文风一致，情节连贯。");

    prompt
}

// GLOBAL: LLM 服务单例。
// SAFETY: OnceCell 保证仅初始化一次。通过 init_llm_service() 在 setup()
// 中设置。内部存储 Arc<LlmService>，初始化后 get_llm_service() 直接 clone
// Arc，无需再加全局 Mutex。
// NOTE: 当前使用全局静态是因为大量命令处理器直接调用
// get_llm_service()。 长期目标：通过 Tauri State 注入，消除全局依赖。
static LLM_SERVICE: once_cell::sync::OnceCell<std::sync::Arc<LlmService>> =
    once_cell::sync::OnceCell::new();

/// 初始化LLM服务
pub fn init_llm_service(app_handle: AppHandle) {
    let service = LlmService::new(app_handle);
    let _ = LLM_SERVICE.set(std::sync::Arc::new(service));
}

/// 获取LLM服务
pub fn get_llm_service() -> Option<LlmService> {
    LLM_SERVICE.get().map(|s| s.as_ref().clone())
}

impl Clone for LlmService {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            config: Arc::clone(&self.config),
            cancel_senders: Arc::clone(&self.cancel_senders),
            adapter_cache: Arc::clone(&self.adapter_cache),
            prompt_cache: self.prompt_cache.clone(),
            cancelled_requests: Arc::clone(&self.cancelled_requests),
            writer_local_semaphore: Arc::clone(&self.writer_local_semaphore),
            writer_remote_semaphore: Arc::clone(&self.writer_remote_semaphore),
        }
    }
}

// =============================================================================
// W4-B5: 配额逻辑单元测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_writing_prompt_without_context() {
        let prompt = build_writing_prompt("写一个关于太空的故事", None);
        assert!(prompt.contains("你是一位专业的小说创作助手"));
        assert!(prompt.contains("【续写要求】"));
        assert!(prompt.contains("写一个关于太空的故事"));
        assert!(prompt.contains("请直接输出续写内容"));
        assert!(!prompt.contains("【前文上下文】"));
    }

    #[test]
    fn test_build_writing_prompt_with_context() {
        let prompt = build_writing_prompt("继续写下去", Some("之前的章节内容..."));
        assert!(prompt.contains("你是一位专业的小说创作助手"));
        assert!(prompt.contains("【前文上下文】"));
        assert!(prompt.contains("之前的章节内容..."));
        assert!(prompt.contains("【续写要求】"));
        assert!(prompt.contains("继续写下去"));
        assert!(prompt.contains("请直接输出续写内容"));
    }

    #[test]
    fn test_build_writing_prompt_empty_input() {
        let prompt = build_writing_prompt("", None);
        assert!(prompt.contains("【续写要求】"));
        assert!(prompt.contains("请直接输出续写内容"));
    }

    #[test]
    fn test_stream_chunk_serialization() {
        let chunk = StreamChunk {
            chunk: "Hello".to_string(),
            is_first: true,
            is_last: false,
            model: "gpt-4".to_string(),
        };
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("Hello"));
        assert!(json.contains("gpt-4"));

        let deserialized: StreamChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.chunk, "Hello");
        assert!(deserialized.is_first);
        assert!(!deserialized.is_last);
    }

    #[test]
    fn test_generation_complete_serialization() {
        let complete = GenerationComplete {
            full_text: "全文内容".to_string(),
            model: "claude-3".to_string(),
            tokens_used: 100,
            cost: 0.002,
            duration_ms: 1234,
        };
        let json = serde_json::to_string(&complete).unwrap();
        let deserialized: GenerationComplete = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.full_text, "全文内容");
        assert_eq!(deserialized.tokens_used, 100);
        assert_eq!(deserialized.cost, 0.002);
        assert_eq!(deserialized.duration_ms, 1234);
    }

    #[test]
    fn test_generation_error_serialization() {
        let error = GenerationError {
            error: "连接超时".to_string(),
            error_code: "TIMEOUT".to_string(),
        };
        let json = serde_json::to_string(&error).unwrap();
        let deserialized: GenerationError = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.error, "连接超时");
        assert_eq!(deserialized.error_code, "TIMEOUT");
    }

    #[test]
    fn test_llm_generating_progress_serialization() {
        let pipeline_ctx = PipelineContext {
            step_name: "内容评审".to_string(),
            step_number: 2,
            total_steps: 5,
            action: "正在生成评审意见".to_string(),
        };
        let progress = LlmGeneratingProgress {
            stage: "generating".to_string(),
            message: "AI 正在深度思考中...".to_string(),
            elapsed_seconds: 30,
            model: "gpt-4".to_string(),
            pipeline_context: Some(pipeline_ctx),
        };
        let json = serde_json::to_string(&progress).unwrap();
        let deserialized: LlmGeneratingProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.stage, "generating");
        assert_eq!(deserialized.elapsed_seconds, 30);
        let ctx = deserialized.pipeline_context.unwrap();
        assert_eq!(ctx.step_name, "内容评审");
        assert_eq!(ctx.step_number, 2);
        assert_eq!(ctx.total_steps, 5);
    }

    #[test]
    fn test_llm_generating_progress_without_pipeline_context() {
        let progress = LlmGeneratingProgress {
            stage: "connecting".to_string(),
            message: "正在连接模型...".to_string(),
            elapsed_seconds: 0,
            model: "deepseek-chat".to_string(),
            pipeline_context: None,
        };
        let json = serde_json::to_string(&progress).unwrap();
        let deserialized: LlmGeneratingProgress = serde_json::from_str(&json).unwrap();
        assert!(deserialized.pipeline_context.is_none());
    }

    #[test]
    fn test_generate_request_serialization() {
        let req = GenerateRequest {
            prompt: "Hello".to_string(),
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: GenerateRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.prompt, "Hello");
        assert_eq!(deserialized.max_tokens, Some(100));
        assert_eq!(deserialized.temperature, Some(0.7));
    }

    #[test]
    fn test_generate_response_serialization() {
        let resp = GenerateResponse {
            content: "World".to_string(),
            model: "gpt-4".to_string(),
            tokens_used: 10,
            cost: 0.001,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: GenerateResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.content, "World");
        assert_eq!(deserialized.tokens_used, 10);
    }

    #[test]
    fn test_prompt_cache_hit_and_miss() {
        use std::time::Duration;

        let cache = PromptCache::new(Duration::from_secs(60), 10);
        let profile = LlmProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            provider: LlmProvider::OpenAI,
            model_source: crate::config::settings::ModelSource::Local,
            model: "gpt-4".to_string(),
            api_key: "key".to_string(),
            api_base: None,
            is_local_model: false,
            max_tokens: 100,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: 300,
            is_default: false,
            capabilities: vec![],
            enabled: true,
            kind: crate::config::settings::ModelKind::Chat,
            max_context_length: 8192,
            quality_tier: crate::config::settings::QualityTier::Medium,
            speed_tier: crate::config::settings::SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec![],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
        };
        let response = GenerateResponse {
            content: "OK".to_string(),
            model: "gpt-4".to_string(),
            tokens_used: 1,
            cost: 0.0,
        };

        assert!(cache.get(&profile, "hello", Some(10), Some(0.0)).is_none());
        cache.put(&profile, "hello", Some(10), Some(0.0), response.clone());
        let hit = cache.get(&profile, "hello", Some(10), Some(0.0));
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().content, "OK");

        // 不同参数未命中
        assert!(cache.get(&profile, "hello", Some(20), Some(0.0)).is_none());
        assert!(cache.get(&profile, "world", Some(10), Some(0.0)).is_none());
    }

    #[test]
    fn test_prompt_cache_ttl_eviction() {
        use std::time::Duration;

        let cache = PromptCache::new(Duration::from_millis(10), 10);
        let profile = LlmProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: None,
            provider: LlmProvider::OpenAI,
            model_source: crate::config::settings::ModelSource::Local,
            model: "gpt-4".to_string(),
            api_key: "key".to_string(),
            api_base: None,
            is_local_model: false,
            max_tokens: 100,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: 300,
            is_default: false,
            capabilities: vec![],
            enabled: true,
            kind: crate::config::settings::ModelKind::Chat,
            max_context_length: 8192,
            quality_tier: crate::config::settings::QualityTier::Medium,
            speed_tier: crate::config::settings::SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec![],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
        };
        let response = GenerateResponse {
            content: "OK".to_string(),
            model: "gpt-4".to_string(),
            tokens_used: 1,
            cost: 0.0,
        };

        cache.put(&profile, "hello", None, None, response);
        assert!(cache.get(&profile, "hello", None, None).is_some());
        std::thread::sleep(Duration::from_millis(50));
        assert!(cache.get(&profile, "hello", None, None).is_none());
    }

    // ==================== 阶段一超时/重试策略 ====================

    #[test]
    fn test_connection_timeout_is_retriable_but_generation_timeout_is_not() {
        use crate::error::AppError;

        let connection_err = AppError::llm_connection_timeout(10_000);
        let generation_err = AppError::llm_generation_timeout(120_000);
        let generic_timeout = AppError::llm_timeout(120_000);

        assert!(LlmService::is_retriable_error(&connection_err));
        assert!(!LlmService::is_retriable_error(&generation_err));
        assert!(!LlmService::is_retriable_error(&generic_timeout));
    }

    #[test]
    fn test_timeout_error_codes() {
        use crate::error::AppError;

        assert_eq!(
            AppError::llm_connection_timeout(10_000).code(),
            "LLM_CONNECTION_TIMEOUT"
        );
        assert_eq!(
            AppError::llm_generation_timeout(120_000).code(),
            "LLM_GENERATION_TIMEOUT"
        );
        assert_eq!(AppError::llm_timeout(120_000).code(), "LLM_TIMEOUT");
        assert_eq!(AppError::cancelled("用户取消").code(), "CANCELLATION");
    }
}
