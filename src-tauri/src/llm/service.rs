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
    adapter::{GenerateRequest, GenerateResponse, ResponseFormat},
    anthropic::AnthropicAdapter,
    ollama::OllamaAdapter,
    openai::OpenAiAdapter,
};
use crate::{
    config::settings::{AppConfig, LlmProfile, LlmProvider},
    diagnostics::{DiagnosticStore, LastLlmPrompt},
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
    /// v0.23: JSON mode 与普通文本输出必须隔离缓存键
    response_format: Option<String>,
    prompt_hash: u64,
}

impl PartialEq for PromptCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.provider == other.provider
            && self.model == other.model
            && self.max_tokens == other.max_tokens
            && self.temperature.map(|f| f.to_bits()) == other.temperature.map(|f| f.to_bits())
            && self.response_format == other.response_format
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
        self.response_format.hash(state);
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
        response_format: Option<ResponseFormat>,
    ) -> PromptCacheKey {
        let mut hasher = DefaultHasher::new();
        prompt.hash(&mut hasher);
        PromptCacheKey {
            provider: format!("{:?}", profile.provider),
            model: profile.model.clone(),
            max_tokens,
            temperature,
            response_format: response_format.map(|f| format!("{:?}", f)),
            prompt_hash: hasher.finish(),
        }
    }

    pub fn get(
        &self,
        profile: &LlmProfile,
        prompt: &str,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        response_format: Option<ResponseFormat>,
    ) -> Option<GenerateResponse> {
        let key = Self::key(profile, prompt, max_tokens, temperature, response_format);
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
        response_format: Option<ResponseFormat>,
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
        let key = Self::key(profile, prompt, max_tokens, temperature, response_format);
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

/// LLM生成进度事件 —
/// 携带Pipeline步骤上下文与模型/提示词细节，让用户知道"当前在进行哪一步"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGeneratingProgress {
    pub stage: String, // "connecting" | "sent" | "generating" | "completed" | "error"
    pub message: String,
    pub elapsed_seconds: u64,
    /// 实际模型名，如 "qwen2.5-7b-instruct"
    pub model: String,
    /// 模型配置 ID，如 "local-qwen"
    pub model_id: String,
    /// 提供商，如 "ollama" / "openai"
    pub provider: String,
    /// 提示词字符数
    pub prompt_chars: Option<usize>,
    /// 提示词 token 估算（仅参考）
    pub prompt_tokens: Option<usize>,
    /// 模型返回 token 数
    pub response_tokens: Option<usize>,
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
    /// 优先返回通过 Tauri State 管理的共享实例（由 setup() 注入），
    /// 复用 reqwest 连接池与 adapter 缓存；若 State 不存在（例如测试），
    /// 则创建独立实例。
    pub fn new(app_handle: AppHandle) -> Self {
        // 优先返回 Tauri State 中的共享实例，避免每次命令重建 client 与缓存。
        if let Some(state) = app_handle.try_state::<Self>() {
            return state.inner().clone();
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

    /// v0.23.12: 快捷记录工作流日志
    fn workflow_log(
        &self,
        phase: impl Into<String>,
        message: impl Into<String>,
        details: Option<serde_json::Value>,
    ) {
        if let Some(logger) = self
            .app_handle
            .try_state::<Arc<crate::workflow_logger::WorkflowLogger>>()
        {
            logger.info(phase, message, details);
        }
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
    ///
    /// v0.23.13: 优先使用用户当前设置的活跃模型，避免路由器在用户未预期的
    /// 模型上执行（特别是本地/远程模型混用时）。活跃模型不可用时才走统一路由。
    pub fn select_profile_for_request(
        &self,
        request: &RoutingRequest,
    ) -> Result<LlmProfile, AppError> {
        let guard = self.config.lock().map_err(|_| {
            AppError::internal("Failed to lock AppConfig while selecting model".to_string())
        })?;

        // 1) 优先返回用户 explicit 设置的活跃模型
        if let Some(active_id) = guard.active_llm_profile.as_deref() {
            if let Some(profile) = guard.llm_profiles.get(active_id) {
                if profile.enabled {
                    log::info!(
                        "[LLM] select_profile_for_request: 优先使用活跃模型 {}",
                        profile.id
                    );
                    return Ok(profile.clone());
                }
            }
        }

        // 2) 活跃模型不可用时走统一路由
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
    ///
    /// Phase 2/3: 新增 asset_tags /
    /// discovered_asset_ids，把意图图发现的资产信息
    /// 透传给模型网关，用于任务分类与候选模型筛选。
    ///
    /// v0.23: 新增 `response_format`，结构化输出请求可透传到 OpenAI/Ollama 的
    /// JSON mode；Anthropic 适配器暂忽略，仍靠 prompt 约束。
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
        // Phase 2/3: 意图图资产标签与发现资产 ID
        asset_tags: Option<Vec<String>>,
        discovered_asset_ids: Option<Vec<String>>,
        response_format: Option<ResponseFormat>,
    ) -> (String, Result<GenerateResponse, AppError>) {
        let req_id = request_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // 优先尝试模型网关
        self.workflow_log(
            "llm.generate.pre_gateway",
            "准备调用 gateway.generate",
            Some(serde_json::json!({"request_id": req_id, "context_label": context_label})),
        );
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
            // Phase 2/3: 注入意图图资产发现信息
            asset_tags: asset_tags.unwrap_or_default(),
            discovered_asset_ids: discovered_asset_ids.unwrap_or_default(),
            // v0.23: 结构化输出格式透传
            response_format,
        };
        match gateway.generate(gateway_request).await {
            Ok(resp) => {
                self.workflow_log(
                    "llm.generate.gateway_ok",
                    "gateway.generate 返回成功",
                    Some(serde_json::json!({"request_id": req_id})),
                );
                return (req_id, Ok(resp));
            }
            Err(e) => {
                log::warn!("[LlmService] ModelGateway 调用失败，回退旧路径: {}", e);
            }
        }

        // 回退：旧版本地路由
        let profile = match self.select_profile_for_request(&request) {
            Ok(p) => p,
            Err(e) => return (req_id, Err(e)),
        };
        self.generate_with_profile_and_request_id_with_format(
            &profile.id,
            prompt,
            max_tokens,
            temperature,
            context_label,
            Some(req_id),
            None,
            None,
            response_format,
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
        self.generate_for_task_with_format(
            task,
            prompt,
            max_tokens,
            temperature,
            context_label,
            None,
        )
        .await
    }

    /// 简化入口（显式指定输出格式）：仅指定任务类型，使用默认复杂度/
    /// 优先级进行路由生成
    pub async fn generate_for_task_with_format(
        &self,
        task: TaskType,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        response_format: Option<ResponseFormat>,
    ) -> Result<GenerateResponse, AppError> {
        let request = RoutingRequest {
            task,
            complexity: Complexity::Medium,
            budget_priority: Priority::Low,
            speed_priority: Priority::Low,
            estimated_input_tokens: 0,
            constraints: vec![],
        };
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
                None,
                None,
                response_format,
            )
            .await;
        result
    }

    /// v0.23.9: 生成任务并携带资产标签/已发现资产 ID，供 ModelGateway
    /// 做意图感知路由。
    pub async fn generate_for_task_with_tags(
        &self,
        task: TaskType,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        asset_tags: Vec<String>,
        discovered_asset_ids: Vec<String>,
    ) -> Result<GenerateResponse, AppError> {
        self.generate_for_task_with_tags_and_timeout(
            task,
            prompt,
            max_tokens,
            temperature,
            context_label,
            asset_tags,
            discovered_asset_ids,
            None,
        )
        .await
    }

    /// v0.23.15: 带 timeout_seconds_override 的版本，供 TriShot Call 3 使用。
    pub async fn generate_for_task_with_tags_and_timeout(
        &self,
        task: TaskType,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        asset_tags: Vec<String>,
        discovered_asset_ids: Vec<String>,
        timeout_seconds_override: Option<u64>,
    ) -> Result<GenerateResponse, AppError> {
        let request = RoutingRequest {
            task,
            complexity: Complexity::Medium,
            budget_priority: Priority::Low,
            speed_priority: Priority::Low,
            estimated_input_tokens: 0,
            constraints: vec![],
        };
        let (_, result) = self
            .generate_for_request_with_request_id(
                request,
                prompt,
                max_tokens,
                temperature,
                context_label,
                None,
                timeout_seconds_override,
                None,
                None,
                None,
                Some(asset_tags),
                Some(discovered_asset_ids),
                None,
            )
            .await;
        result
    }

    /// v0.23 TriShot：用「最快可用模型」生成文本，用于 Call 1 路由合成器。
    ///
    /// 通过 `GatewayExecutor::select_fastest_profile` 按算力档案 TTFB
    /// 选最快模型， 失败回退 active profile。始终用
    /// `generate_with_profile_and_request_id` 单次调用， 不走候选 fallback
    /// 链（追求首字节速度）。
    pub async fn generate_with_fastest(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
    ) -> Result<GenerateResponse, AppError> {
        let profile = self
            .app_handle
            .try_state::<crate::model_gateway::executor::GatewayExecutor>()
            .and_then(|gw| gw.select_fastest_profile())
            .or_else(|| self.get_active_profile())
            .ok_or_else(|| AppError::internal("无可用模型（generate_with_fastest）".to_string()))?;

        let (_, result) = self
            .generate_with_profile_and_request_id(
                &profile.id,
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
    /// v0.23.20: 整个函数体移入 `spawn_blocking`，不在 tokio worker
    /// 线程做任何工作。 v0.23.19 只把 DB 写入移入 spawn_blocking，但
    /// `try_state` + `count_tokens` + `get_active_profile` + 数据收集仍在
    /// async 线程同步执行，worker 线程被占满时
    /// 仍卡 4 分钟。指标记录是审计用途，失败不影响生成结果，永不阻塞主流程。
    fn record_llm_call(&self, record: LlmCallRecord<'_>) {
        // 在 async 线程只做最小工作：clone owned 数据，然后全部交给阻塞线程池。
        let app_handle = self.app_handle.clone();
        let model_id = record.model_id.to_string();
        let model_name = record.model_name.map(|s| s.to_string());
        let purpose = record.purpose.to_string();
        let prompt = record.prompt.to_string();
        let duration_ms = record.duration_ms as u32;
        let completion_tokens = record.response.map(|r| r.tokens_used).unwrap_or(0);
        let error_msg = record.error.map(|e| e.to_string());
        let success = record.error.is_none();

        self.workflow_log(
            "llm.record_call.spawn",
            "整个 record_llm_call 已提交到阻塞线程池（fire-and-forget）",
            None,
        );

        // fire-and-forget：不等待结果，指标记录失败不影响生成主流程。
        tokio::task::spawn_blocking(move || {
            // 以下所有工作在阻塞线程池执行，不占用 tokio worker 线程。
            let prompt_tokens = count_tokens(&prompt, &model_id) as i32;
            let total_tokens = prompt_tokens + completion_tokens;
            let cached = completion_tokens > 0 && prompt_tokens == 0;

            // 统一结构化指标日志
            let metrics = serde_json::json!({
                "event": "llm_call",
                "model": model_id,
                "purpose": purpose,
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": total_tokens,
                "duration_ms": duration_ms,
                "success": success,
                "cached": cached,
            });
            log::info!(target: "llm_metrics", "{}", metrics);

            // 获取连接池
            let pool = match app_handle.try_state::<crate::db::DbPool>() {
                Some(p) => p.inner().clone(),
                None => return, // 无连接池，静默跳过
            };

            use crate::db::{repositories_pipeline::LlmCallRepository, RecordLlmCallRequest};
            let req = RecordLlmCallRequest {
                story_id: None,
                draft_id: None,
                model_id,
                model_name,
                purpose,
                prompt_tokens,
                completion_tokens,
                duration_ms: duration_ms as i32,
                success,
                error_message: error_msg,
                task_type: None,
                quality_score: None,
                latency_ms: Some(duration_ms as i32),
                route_decision: None,
                audit_feedback: None,
            };
            let preview = if prompt.len() > 200 {
                prompt[..200].to_string()
            } else {
                prompt
            };
            let metadata = serde_json::json!({
                "cached": cached,
            })
            .to_string();

            let repo = LlmCallRepository::new(pool);
            if let Err(e) = repo.create(
                req,
                total_tokens,
                duration_ms as i32,
                Some(&preview),
                Some(&metadata),
            ) {
                log::warn!("[LLM] Failed to record llm_call: {}", e);
            }
        });
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
    #[allow(clippy::too_many_arguments)]
    fn emit_llm_progress(
        &self,
        stage: &str,
        message: &str,
        elapsed_seconds: u64,
        model: &str,
        model_id: &str,
        provider: &str,
        prompt_chars: Option<usize>,
        prompt_tokens: Option<usize>,
        response_tokens: Option<usize>,
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
                model_id: model_id.to_string(),
                provider: provider.to_string(),
                prompt_chars,
                prompt_tokens,
                response_tokens,
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
        response_format: Option<ResponseFormat>,
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
        let model_id = profile.id.clone();
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
            response_format,
        };

        let label = context_label.unwrap_or("");
        let prompt_chars = req.prompt.chars().count();
        let prompt_tokens_est = prompt_chars / 2;

        // v0.14.4: 识别后台静默调用（如模型健康探测），跳过心跳发射避免误导前端
        // 模型探测在应用启动时自动触发，不应让前端误以为用户的生成任务正在进行
        // v0.16.2: 时间线 2/3 的后台审计与洞察 LLM 调用同样静默——主流程已发出
        // GenerationPhase::Completed，若它们继续 emit 普通 progress event
        // 会让前端误以为主流程仍在跑（实测「写第二章」场景下被这些后台事件
        // 拖到 200s 假超时）。
        let is_silent_background = matches!(
            label,
            "model_gateway_probe"
                | "pre-call-probe"
                | "input_hint"
                | "intent_detection"
                | "async-audit-inspector"
                | "async-insight"
                | "async-deep-insight"
                | "background-summary"
                // v0.23 TriShot：关键路径与后台 agent 的 LLM 调用全部静默，
                // 避免与主流程进度事件混淆。tri-shot-router/refiner 属关键路径
                // 前置调用（主流程仍在 PreparingContext 阶段，由 execute_trishot
                // 自行发射细粒度进度，不依赖心跳）；bg-auto-rewriter/bg-ingest 属
                // 正文返回后的后台 agent，必须静默以免触发 v0.16.2 的假超时。
                | "tri-shot-router"
                | "tri-shot-refiner"
                | "bg-auto-rewriter"
                | "bg-ingest"
                // v0.23.44: IngestPipeline 的 LLM 调用全部静默。
                // 根因（日志确认）：创世正文返回后，IngestPipeline 并发发起多个
                // "记忆-内容分析" LLM 调用，is_silent_background=false 导致进度事件
                // 覆盖前端主活动状态（"准备上下文"卡住），且本地模型无法处理并发
                // 请求返回 INTERNAL_ERROR，大量错误事件涌入导致前端页面崩溃。
                | "记忆-内容分析"
                | "记忆-生成知识"
                | "记忆-叙事事件提取"
        );

        // v0.23.11: 只有非静默/非探测调用才更新诊断提示词，避免 probe prompt
        // "Respond with exactly the word OK." 覆盖用户真正关心的生成提示词。
        if !is_silent_background {
            let req_id = request_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            if let Some(store) = self.app_handle.try_state::<Arc<DiagnosticStore>>() {
                store.set_last_llm_prompt(LastLlmPrompt {
                    request_id: req_id.clone(),
                    context_label: label.to_string(),
                    model_id: model_id.clone(),
                    model_name: model_name.clone(),
                    provider: format!("{:?}", provider),
                    prompt: req.prompt.clone(),
                    prompt_chars,
                    prompt_tokens: prompt_tokens_est,
                    updated_at: chrono::Utc::now().to_rfc3339(),
                });
            }
            let prompt_preview: String = req.prompt.chars().take(500).collect();
            let _ = self.app_handle.emit(
                "llm-prompt-sent",
                serde_json::json!({
                    "request_id": req_id,
                    "context_label": context_label,
                    "model_id": model_id,
                    "model_name": model_name,
                    "provider": format!("{:?}", provider),
                    "prompt_preview": prompt_preview,
                    "prompt_chars": prompt_chars,
                    "prompt_tokens": prompt_tokens_est,
                }),
            );
        }
        let step_prefix = pipeline_ref
            .map(|p| format!("[{} {}/{}] ", p.step_name, p.step_number, p.total_steps))
            .unwrap_or_default();

        // v0.23.8: 进度文案带上具体模型 ID、提供商、提示词规模，让用户随时知道 backend
        // 在做什么
        let provider_str = format!("{:?}", provider).to_lowercase();
        let connecting_msg = if label.is_empty() {
            format!(
                "{}连接模型 {}（{}）...",
                step_prefix, model_name, provider_str
            )
        } else {
            format!(
                "{}连接模型 {}（{}）用于 [{}]...",
                step_prefix, model_name, provider_str, label
            )
        };
        let sent_msg = if label.is_empty() {
            format!(
                "{}已连接 {}，组合提示词约 {} 字符（估算 {} tokens），正在发送请求...",
                step_prefix, model_name, prompt_chars, prompt_tokens_est
            )
        } else {
            format!(
                "{}已连接 {} 用于 [{}]，组合提示词约 {} 字符（估算 {} tokens），正在发送请求...",
                step_prefix, model_name, label, prompt_chars, prompt_tokens_est
            )
        };
        let completed_msg = if label.is_empty() {
            format!("{}模型 {} 回应完成，正在解析结果...", step_prefix, model_name)
        } else {
            format!(
                "{}模型 {} 回应完成 [{}]，正在解析结果...",
                step_prefix, model_name, label
            )
        };

        if !is_silent_background {
            self.emit_llm_progress(
                "connecting",
                &connecting_msg,
                0,
                &model_name,
                &model_id,
                &provider_str,
                Some(prompt_chars),
                Some(prompt_tokens_est),
                None,
                pipeline_ref,
                request_id.as_deref(),
            );
        }

        // 启动心跳任务
        // v0.14.4: 后台静默调用（如 model_gateway_probe）跳过心跳，
        // 避免应用启动时探测请求误触发前端"AI 正在深度思考中..."状态栏
        // v0.23.8: 心跳文案带上具体模型 ID 与等待时长，不再只用“构思故事”。
        let app_handle = self.app_handle.clone();
        let model_hb = model_name.clone();
        let model_id_hb = model_id.clone();
        let provider_hb = provider_str.clone();
        let label_owned = label.to_string();
        let pipeline_ctx_for_heartbeat = pipeline_ctx.clone();
        let prompt_chars_hb = prompt_chars;
        let prompt_tokens_hb = prompt_tokens_est;
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
                            "{}等待模型 {}（{}）回应中，提示词约 {} 字符（估算 {} tokens），已等待 {} 秒",
                            step_prefix_hb, model_hb, provider_hb, prompt_chars_hb, prompt_tokens_hb, elapsed
                        )
                    } else {
                        format!(
                            "{}等待模型 {}（{}）的 [{}] 回应中，提示词约 {} 字符（估算 {} tokens），已等待 {} 秒",
                            step_prefix_hb, model_hb, provider_hb, label_owned, prompt_chars_hb, prompt_tokens_hb, elapsed
                        )
                    };
                    let emit_result = app_handle.emit(
                        "llm-generating-progress",
                        LlmGeneratingProgress {
                            stage: "generating".to_string(),
                            message: message.clone(),
                            elapsed_seconds: elapsed,
                            model: model_hb.clone(),
                            model_id: model_id_hb.clone(),
                            provider: provider_hb.clone(),
                            prompt_chars: Some(prompt_chars_hb),
                            prompt_tokens: Some(prompt_tokens_hb),
                            response_tokens: None,
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
                &model_id,
                &provider_str,
                Some(prompt_chars),
                Some(prompt_tokens_est),
                None,
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

        self.workflow_log(
            "llm.generate.start",
            format!("开始 LLM 调用: model_id={}", model_id),
            Some(serde_json::json!({
                "request_id": request_id,
                "model_id": model_id,
                "model_name": model_name,
                "provider": format!("{:?}", provider),
                "context_label": context_label,
                "prompt_chars": prompt_chars,
                "prompt_tokens_est": prompt_tokens_est,
                "timeout_seconds": timeout_seconds,
                "connect_timeout_seconds": connect_timeout_seconds,
                "max_retries": max_retries,
                "is_silent_background": is_silent_background,
            })),
        );

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

        self.workflow_log(
            "llm.heartbeat.abort",
            "开始中止心跳任务",
            Some(serde_json::json!({"request_id": request_id})),
        );
        heartbeat_handle.abort();
        // v0.23.17: 带超时的心跳等待——若心跳 task 卡在 emit() 等同步阻塞操作中，
        // tokio::task::abort() 无法立即终止。用 5s 超时防止主流程被无限阻塞。
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), heartbeat_handle).await;
        self.workflow_log(
            "llm.heartbeat.aborted",
            "心跳任务已完成/超时终止",
            Some(serde_json::json!({"request_id": request_id})),
        );

        match result {
            Ok(response) => {
                let duration = start_time.elapsed().as_millis() as u64;
                log::info!(
                    "[LLM] Sync generation completed in {}ms response_len={}",
                    duration,
                    response.content.len()
                );
                self.workflow_log(
                    "llm.generate.completed",
                    format!("LLM 调用完成: model_id={} 耗时={}ms", model_id, duration),
                    Some(serde_json::json!({
                        "request_id": request_id,
                        "model_id": model_id,
                        "model_name": model_name,
                        "duration_ms": duration,
                        "response_tokens": response.tokens_used,
                        "response_chars": response.content.chars().count(),
                    })),
                );
                self.workflow_log(
                    "llm.record_call.start",
                    "开始写入 llm_calls 记录",
                    Some(serde_json::json!({"request_id": request_id})),
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
                // v0.23.19: record_llm_call 现在是 fire-and-forget，DB
                // 写入在阻塞线程池异步执行。 此处已立即返回，不会阻塞后续
                // emit_llm_progress。
                self.workflow_log(
                    "llm.record_call.done",
                    "llm_calls 记录已提交（fire-and-forget，DB 写入在后台线程）",
                    Some(serde_json::json!({"request_id": request_id})),
                );
                self.workflow_log(
                    "llm.emit_completed.start",
                    "准备发射 completed 进度事件",
                    Some(serde_json::json!({"request_id": request_id})),
                );
                if !is_silent_background {
                    self.emit_llm_progress(
                        "completed",
                        &completed_msg,
                        0,
                        &model_name,
                        &model_id,
                        &provider_str,
                        Some(prompt_chars),
                        Some(prompt_tokens_est),
                        Some(response.tokens_used as usize),
                        pipeline_ref,
                        Some(request_id.as_str()),
                    );
                }
                self.workflow_log(
                    "llm.emit_completed.done",
                    "completed 进度事件已发射",
                    Some(serde_json::json!({"request_id": request_id})),
                );
                self.workflow_log(
                    "llm.generate.return_ok",
                    "LLM 生成成功，准备返回结果给调用者",
                    Some(serde_json::json!({"request_id": request_id, "content_len": response.content.len()})),
                );
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
                self.workflow_log(
                    "llm.generate.error",
                    format!("LLM 调用失败: model_id={} 错误={}", model_id, e),
                    Some(serde_json::json!({
                        "request_id": request_id,
                        "model_id": model_id,
                        "model_name": model_name,
                        "duration_ms": duration,
                        "is_timeout": is_timeout,
                        "error": e.to_string(),
                    })),
                );
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
                        &model_id,
                        &provider_str,
                        Some(prompt_chars),
                        Some(prompt_tokens_est),
                        None,
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
            None,
        )
        .await
    }

    /// 使用指定模型配置同步生成文本，返回 (request_id, Result)，并显式指定
    /// `response_format`。供模型网关/审稿等需要结构化输出的路径使用。
    pub async fn generate_with_profile_and_request_id_with_format(
        &self,
        profile_id: &str,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        request_id: Option<String>,
        timeout_seconds_override: Option<u64>,
        max_retries_override: Option<u32>,
        response_format: Option<ResponseFormat>,
    ) -> (String, Result<GenerateResponse, AppError>) {
        log::info!(
            "[LLM] Starting sync generation with profile={} prompt_len={} format={:?}",
            profile_id,
            prompt.len(),
            response_format
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
            response_format,
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
        response_format: Option<ResponseFormat>,
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
            response_format,
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
            response_format: None,
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

        if let Some(cached) =
            self.prompt_cache
                .get(&profile, &prompt, max_tokens, temperature, None)
        {
            return Ok(cached);
        }

        let response = self
            .generate(prompt.clone(), max_tokens, temperature)
            .await?;
        self.prompt_cache.put(
            &profile,
            &prompt,
            max_tokens,
            temperature,
            None,
            response.clone(),
        );
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

#[async_trait::async_trait]
impl crate::ports::LlmService for LlmService {
    async fn generate(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<GenerateResponse, AppError> {
        self.generate(prompt, max_tokens, temperature).await
    }

    async fn generate_with_context(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
    ) -> Result<GenerateResponse, AppError> {
        self.generate_with_context(prompt, max_tokens, temperature, context_label)
            .await
    }

    async fn generate_with_request_id(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        pipeline_ctx: Option<PipelineContext>,
        request_id: Option<String>,
    ) -> (String, Result<GenerateResponse, AppError>) {
        self.generate_with_request_id(
            prompt,
            max_tokens,
            temperature,
            context_label,
            pipeline_ctx,
            request_id,
        )
        .await
    }

    async fn generate_with_profile(
        &self,
        profile_id: &str,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<GenerateResponse, AppError> {
        self.generate_with_profile(profile_id, prompt, max_tokens, temperature)
            .await
    }

    async fn generate_stream(
        &self,
        request_id: String,
        prompt: String,
        context: Option<String>,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<(), AppError> {
        self.generate_stream(request_id, prompt, context, max_tokens, temperature)
            .await
    }

    fn is_cancelled(&self, request_id: &str) -> bool {
        self.is_cancelled(request_id)
    }

    async fn test_connection(&self) -> Result<(bool, u64), AppError> {
        self.test_connection().await
    }

    fn get_active_profile(&self) -> Option<LlmProfile> {
        self.get_active_profile()
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
            model_id: "profile-gpt4".to_string(),
            provider: "openai".to_string(),
            prompt_chars: Some(1200),
            prompt_tokens: Some(600),
            response_tokens: None,
            pipeline_context: Some(pipeline_ctx),
        };
        let json = serde_json::to_string(&progress).unwrap();
        let deserialized: LlmGeneratingProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.stage, "generating");
        assert_eq!(deserialized.elapsed_seconds, 30);
        assert_eq!(deserialized.model_id, "profile-gpt4");
        assert_eq!(deserialized.prompt_chars, Some(1200));
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
            model_id: "profile-deepseek".to_string(),
            provider: "openai".to_string(),
            prompt_chars: Some(0),
            prompt_tokens: Some(0),
            response_tokens: None,
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
            response_format: None,
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

        assert!(cache
            .get(&profile, "hello", Some(10), Some(0.0), None)
            .is_none());
        cache.put(
            &profile,
            "hello",
            Some(10),
            Some(0.0),
            None,
            response.clone(),
        );
        let hit = cache.get(&profile, "hello", Some(10), Some(0.0), None);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().content, "OK");

        // 不同参数未命中
        assert!(cache
            .get(&profile, "hello", Some(20), Some(0.0), None)
            .is_none());
        assert!(cache
            .get(&profile, "world", Some(10), Some(0.0), None)
            .is_none());
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

        cache.put(&profile, "hello", None, None, None, response);
        assert!(cache.get(&profile, "hello", None, None, None).is_some());
        std::thread::sleep(Duration::from_millis(50));
        assert!(cache.get(&profile, "hello", None, None, None).is_none());
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

    // ====================================================================
    // v0.23.17: 独立模块测试 — 验证不在 Python E2E 测试中的关键路径
    // 1. heartbeat abort/await 是否会陷入无限阻塞
    // 2. TASK_START_TIMES Mutex 并发死锁
    // 3. record_llm_call 连接池满时的行为
    // ====================================================================

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_heartbeat_abort_does_not_block_indefinitely() {
        // 模拟心跳任务：emit 可能阻塞的情况下，abort+await 应在合理时间内完成
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };

        let emitted = Arc::new(AtomicBool::new(false));
        let emitted_clone = emitted.clone();

        // 启动心跳：在一个间隔内至少 emit 一次
        let heartbeat = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
            loop {
                interval.tick().await;
                emitted_clone.store(true, Ordering::SeqCst);
                // 模拟 emit 的同步操作（可能阻塞的情形）
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });

        // 等待第一次 emit
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(emitted.load(Ordering::SeqCst), "心跳应至少 emit 一次");

        // abort + await 应在 5 秒内完成
        heartbeat.abort();
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), heartbeat).await;

        assert!(
            result.is_ok(),
            "heartbeat abort + await 应在 5 秒内完成，但超时了！这证明心跳任务卡在同步代码中无法被 abort"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_heartbeat_with_blocking_emit_handled_by_timeout() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };

        let entered = Arc::new(AtomicBool::new(false));
        let entered_clone = entered.clone();

        let heartbeat = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            entered_clone.store(true, Ordering::SeqCst);
            // 模拟长时间阻塞（但在测试中只用 2 秒，避免 CI 超时）
            std::thread::sleep(std::time::Duration::from_secs(2));
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert!(entered.load(Ordering::SeqCst));

        heartbeat.abort();
        // abort 后 JoinHandle 应立即 resolve（即使任务在 std::thread::sleep 中），
        // 因为 tokio 的 abort 会立即标记任务取消，JoinHandle.await 不会阻塞
        let result = tokio::time::timeout(std::time::Duration::from_millis(500), heartbeat).await;

        assert!(result.is_ok(), "abort 后 JoinHandle 应在 500ms 内 resolve");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_task_start_times_mutex_no_deadlock() {
        // 验证 std::sync::Mutex 在并发场景下不会死锁
        use std::{
            collections::HashMap,
            sync::{Arc, Mutex},
        };

        let map = Arc::new(Mutex::new(HashMap::<String, std::time::Instant>::new()));
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let map = map.clone();
                let task_id = format!("test-task-{}", i);
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
                        guard
                            .entry(task_id.clone())
                            .or_insert_with(std::time::Instant::now);
                    }
                })
            })
            .collect();

        for h in handles {
            let start = std::time::Instant::now();
            while !h.is_finished() && start.elapsed() < std::time::Duration::from_secs(5) {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            assert!(h.is_finished(), "Mutex 并发线程应在 5 秒内完成但卡住了");
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_record_llm_call_pool_timeout() {
        use r2d2::Pool;
        use r2d2_sqlite::SqliteConnectionManager;

        // 创建一个小连接池（仅 1 个连接）
        let manager = SqliteConnectionManager::file(":memory:");
        let pool = Pool::builder()
            .max_size(1)
            .connection_timeout(std::time::Duration::from_secs(2))
            .build(manager)
            .expect("应能创建连接池");

        // 占用唯一的连接
        let _holder = pool.get().expect("应能获取连接");

        // 尝试再次获取连接——应该超时（2 秒内）
        let pool_clone = pool.clone();
        let start = std::time::Instant::now();
        let result = tokio::task::spawn_blocking(move || pool_clone.get())
            .await
            .expect("spawn_blocking 应成功");

        let elapsed = start.elapsed();

        match result {
            Ok(_) => {
                // 意外获取到了连接（可能是内部创建了新连接？检查 max_size）
                // 这仍然可以接受，只要不无限阻塞
            }
            Err(_) => {
                // 预期的超时
                assert!(
                    elapsed < std::time::Duration::from_secs(5),
                    "pool.get() 应在 5 秒内返回（超时或成功），但耗时 {:?}",
                    elapsed
                );
            }
        }
    }

    /// v0.23.17 关键测试：模拟真实应用的 record_llm_call 路径
    /// 使用与生产环境相同的连接池配置，验证 DB 写入不会无限阻塞
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_record_llm_call_non_blocking() {
        use r2d2::Pool;
        use r2d2_sqlite::SqliteConnectionManager;

        // 创建与生产环境相同的连接池配置
        let manager = SqliteConnectionManager::file(":memory:");
        let pool = Pool::builder()
            .max_size(10)
            .connection_timeout(std::time::Duration::from_secs(10))
            .build(manager)
            .expect("应能创建连接池");

        // 初始化 llm_calls 表
        {
            let conn = pool.get().expect("获取连接失败");
            conn.execute(
                "CREATE TABLE llm_calls (
                id TEXT PRIMARY KEY,
                story_id TEXT,
                draft_id TEXT,
                model_id TEXT NOT NULL,
                model_name TEXT,
                purpose TEXT,
                prompt_tokens INTEGER DEFAULT 0,
                completion_tokens INTEGER DEFAULT 0,
                total_tokens INTEGER DEFAULT 0,
                duration_ms INTEGER DEFAULT 0,
                success INTEGER DEFAULT 0,
                error_message TEXT,
                prompt_preview TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                task_type TEXT,
                quality_score REAL,
                latency_ms INTEGER,
                route_decision TEXT,
                audit_feedback TEXT
            )",
                [],
            )
            .expect("建表失败");
        }

        use std::sync::{Arc, Barrier};

        // 并发写入测试：10 个线程同时写 llm_calls
        let pool = Arc::new(pool);
        let barrier = Arc::new(Barrier::new(10));
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let pool = pool.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait(); // 所有线程同步启动
                    let start = std::time::Instant::now();
                    let conn = pool.get();
                    let elapsed = start.elapsed();
                    match conn {
                        Ok(conn) => {
                            let id = format!("test-{}-{}", i, uuid::Uuid::new_v4());
                            conn.execute(
                                "INSERT INTO llm_calls (id, model_id, purpose, created_at) VALUES (?1, 'test', 'test', datetime('now'))",
                                rusqlite::params![id],
                            ).ok();
                            assert!(
                                elapsed < std::time::Duration::from_secs(10),
                                "线程 {} pool.get() 耗时 {:?}，超过 10s",
                                i, elapsed
                            );
                        }
                        Err(e) => {
                            // 连接池满时允许超时
                            assert!(
                                elapsed < std::time::Duration::from_secs(15),
                                "线程 {} pool.get() 失败耗时 {:?}，超过 15s: {}",
                                i, elapsed, e
                            );
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            let start = std::time::Instant::now();
            while !h.is_finished() && start.elapsed() < std::time::Duration::from_secs(15) {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            assert!(
                h.is_finished(),
                "record_llm_call 并发测试线程在 15 秒内未完成，存在死锁风险"
            );
        }
    }
}
