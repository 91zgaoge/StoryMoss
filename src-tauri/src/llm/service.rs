//! LLM Service - 统一的大语言模型服务
//! 
//! 提供同步生成和流式生成两种模式
//! 支持多提供商配置管理和自动切换
#![allow(dead_code)]

use super::adapter::{GenerateRequest, GenerateResponse};
use super::anthropic::AnthropicAdapter;
use super::ollama::OllamaAdapter;
use super::openai::OpenAiAdapter;
use crate::config::settings::{AppConfig, LlmProfile, LlmProvider};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{timeout, Duration};

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

/// LLM服务 - 管理所有LLM调用
pub struct LlmService {
    app_handle: AppHandle,
    config: Arc<Mutex<AppConfig>>,
    cancel_senders: Arc<Mutex<HashMap<String, tokio::sync::mpsc::Sender<()>>>>,
}

impl LlmService {
    pub fn new(app_handle: AppHandle) -> Self {
        let app_dir = app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        
        let config = AppConfig::load(&app_dir).unwrap_or_default();
        
        Self {
            app_handle,
            config: Arc::new(Mutex::new(config)),
            cancel_senders: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 重新加载配置
    pub fn reload_config(&self) {
        let app_dir = self.app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        
        if let Ok(config) = AppConfig::load(&app_dir) {
            if let Ok(mut guard) = self.config.lock() {
                *guard = config;
            }
        }
    }

    /// 获取当前活跃的LLM配置
    pub fn get_active_profile(&self) -> Option<LlmProfile> {
        let guard = self.config.lock().ok()?;
        guard.get_active_llm_profile().cloned()
    }

    /// 获取指定ID的LLM配置
    fn get_profile_by_id(&self, profile_id: &str) -> Option<LlmProfile> {
        let guard = self.config.lock().ok()?;
        guard.llm_profiles.get(profile_id).cloned()
    }

    /// 创建适配器
    fn create_adapter(&self, profile: &LlmProfile) -> Result<Box<dyn super::LlmAdapter>, String> {
        match profile.provider {
            LlmProvider::OpenAI | LlmProvider::Custom | LlmProvider::DeepSeek | LlmProvider::Qwen => {
                Ok(Box::new(OpenAiAdapter::new(
                    profile.api_key.clone(),
                    profile.model.clone(),
                    profile.api_base.clone(),
                    profile.max_tokens,
                    profile.temperature,
                )))
            }
            LlmProvider::Anthropic => {
                Ok(Box::new(AnthropicAdapter::new(
                    profile.api_key.clone(),
                    profile.model.clone(),
                    profile.api_base.clone(),
                    profile.max_tokens,
                    profile.temperature,
                )))
            }
            LlmProvider::Ollama => {
                Ok(Box::new(OllamaAdapter::new(
                    profile.api_key.clone(),
                    profile.model.clone(),
                    profile.api_base.clone(),
                    profile.max_tokens,
                    profile.temperature,
                )))
            }
            _ => Err(format!("Provider {:?} not supported", profile.provider)),
        }
    }

    /// 发送 LLM 生成进度事件
    fn emit_llm_progress(&self, stage: &str, message: &str, elapsed_seconds: u64, model: &str, pipeline_ctx: Option<&PipelineContext>) {
        let _ = self.app_handle.emit("llm-generating-progress", LlmGeneratingProgress {
            stage: stage.to_string(),
            message: message.to_string(),
            elapsed_seconds,
            model: model.to_string(),
            pipeline_context: pipeline_ctx.cloned(),
        });
    }

    /// 同步生成文本（带上下文描述 + 600秒整体超时 + 心跳进度）
    pub async fn generate(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<GenerateResponse, String> {
        self.generate_with_context_and_pipeline(prompt, max_tokens, temperature, None, None).await
    }
    
    /// 同步生成文本，支持上下文描述（v5.2.3: 进度消息更具体）
    pub async fn generate_with_context(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
    ) -> Result<GenerateResponse, String> {
        self.generate_with_context_and_pipeline(prompt, max_tokens, temperature, context_label, None).await
    }
    
    /// 同步生成文本，支持上下文描述 + Pipeline步骤上下文（v5.2.4: 让进度消息显示当前步骤）
    pub async fn generate_with_context_and_pipeline(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
        pipeline_ctx: Option<PipelineContext>,
    ) -> Result<GenerateResponse, String> {

        let profile = self.get_active_profile()
            .ok_or("No active LLM profile configured")?;
        
        let model_name = profile.model.clone();
        let adapter = self.create_adapter(&profile)?;
        
        let request = GenerateRequest {
            prompt,
            max_tokens,
            temperature,
        };
        
        // 构建带有上下文的进度消息
        let label = context_label.unwrap_or("");
        let pipeline_ref = pipeline_ctx.as_ref();
        let step_prefix = pipeline_ref.map(|p| format!("[{} {}/{}] ", p.step_name, p.step_number, p.total_steps)).unwrap_or_default();
        
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
        
        // 发送开始连接事件
        self.emit_llm_progress("connecting", &connecting_msg, 0, &model_name, pipeline_ref);
        
        // 启动心跳任务：每10秒发送一次进度（v5.2.3: 间隔从2秒改为10秒，用户能看清文字）
        let app_handle = self.app_handle.clone();
        let model = model_name.clone();
        let label_owned = label.to_string();
        let pipeline_ctx_for_heartbeat = pipeline_ctx.clone();
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            let start = std::time::Instant::now();
            let mut tick_count = 0;
            loop {
                interval.tick().await;
                tick_count += 1;
                let elapsed = start.elapsed().as_secs();
                let step_prefix_hb = pipeline_ctx_for_heartbeat.as_ref().map(|p| {
                    format!("[{} {}/{}] ", p.step_name, p.step_number, p.total_steps)
                }).unwrap_or_default();
                let message = if label_owned.is_empty() {
                    format!("{}AI 正在深度思考中...（已等待 {} 秒）", step_prefix_hb, elapsed)
                } else {
                    format!("{}正在{}...（已等待 {} 秒）", step_prefix_hb, label_owned, elapsed)
                };
                let _ = app_handle.emit("llm-generating-progress", LlmGeneratingProgress {
                    stage: "generating".to_string(),
                    message,
                    elapsed_seconds: elapsed,
                    model: model.clone(),
                    pipeline_context: pipeline_ctx_for_heartbeat.clone(),
                });
                // 最多心跳60次（600秒），匹配前端Bootstrap超时
                if tick_count >= 60 {
                    break;
                }
            }
        });
        
        // 发送已发送请求事件
        self.emit_llm_progress("sent", &sent_msg, 0, &model_name, pipeline_ref);
        
        // 使用作用域块确保 Box<dyn StdError> 在 heartbeat_handle.await 之前被销毁
        // v5.2.2: 本地大模型生成长文本可能需要5-10分钟，将超时从120秒延长到600秒
        let result = {
            let r = timeout(Duration::from_secs(600), adapter.generate(request)).await;
            match r {
                Ok(Ok(resp)) => Ok(resp),
                Ok(Err(e)) => Err(format!("Generation failed: {}", e)),
                Err(_) => Err("模型生成超时（600秒无响应），本地模型可能较慢".to_string()),
            }
        };
        
        // 取消心跳任务
        heartbeat_handle.abort();
        let _ = heartbeat_handle.await;
        
        match result {
            Ok(response) => {
                self.emit_llm_progress("completed", &completed_msg, 0, &model_name, pipeline_ref);
                Ok(response)
            }
            Err(e) => {
                let is_timeout = e.contains("超时");
                self.emit_llm_progress("error", &e, if is_timeout { 600 } else { 0 }, &model_name, pipeline_ref);
                Err(e)
            }
        }
    }

    /// 使用指定模型配置同步生成文本（带600秒超时 + 心跳进度）
    pub async fn generate_with_profile(
        &self,
        profile_id: &str,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<GenerateResponse, String> {
        self.generate_with_profile_and_context(profile_id, prompt, max_tokens, temperature, None).await
    }

    /// 使用指定模型配置同步生成文本，支持上下文描述
    pub async fn generate_with_profile_and_context(
        &self,
        profile_id: &str,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
        context_label: Option<&str>,
    ) -> Result<GenerateResponse, String> {
        let profile = self.get_profile_by_id(profile_id)
            .ok_or_else(|| format!("LLM profile '{}' not found", profile_id))?;
        
        let model_name = profile.model.clone();
        let adapter = self.create_adapter(&profile)?;
        
        let request = GenerateRequest {
            prompt,
            max_tokens,
            temperature,
        };
        
        // 构建带有上下文的进度消息
        let label = context_label.unwrap_or("");
        let connecting_msg = if label.is_empty() { "正在连接模型...".to_string() } else { format!("正在连接模型 [{}]...", label) };
        let sent_msg = if label.is_empty() { "已发送请求，等待响应...".to_string() } else { format!("已发送请求 [{}]，等待响应...", label) };
        let completed_msg = if label.is_empty() { "AI 响应完成".to_string() } else { format!("{} 完成", label) };
        
        // 发送开始连接事件
        self.emit_llm_progress("connecting", &connecting_msg, 0, &model_name, None);
        
        // 启动心跳任务：每10秒发送一次进度（v5.2.3: 间隔从3秒改为10秒）
        let app_handle = self.app_handle.clone();
        let model = model_name.clone();
        let label_owned = label.to_string();
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            let start = std::time::Instant::now();
            let mut tick_count = 0;
            loop {
                interval.tick().await;
                tick_count += 1;
                let elapsed = start.elapsed().as_secs();
                let message = if label_owned.is_empty() {
                    format!("AI 正在生成中...（已等待 {} 秒）", elapsed)
                } else {
                    format!("正在{}...（已等待 {} 秒）", label_owned, elapsed)
                };
                let _ = app_handle.emit("llm-generating-progress", LlmGeneratingProgress {
                    stage: "generating".to_string(),
                    message,
                    elapsed_seconds: elapsed,
                    model: model.clone(),
                    pipeline_context: None,
                });
                // 最多心跳60次（600秒）
                if tick_count >= 60 {
                    break;
                }
            }
        });
        
        // 发送已发送请求事件
        self.emit_llm_progress("sent", &sent_msg, 0, &model_name, None);
        
        // 使用作用域块确保 Box<dyn StdError> 在 heartbeat_handle.await 之前被销毁
        // v5.2.2: 本地大模型生成长文本可能需要5-10分钟，将超时从120秒延长到600秒
        let result = {
            let r = timeout(Duration::from_secs(600), adapter.generate(request)).await;
            match r {
                Ok(Ok(resp)) => Ok(resp),
                Ok(Err(e)) => Err(format!("Generation failed: {}", e)),
                Err(_) => Err("模型生成超时（600秒无响应），本地模型可能较慢".to_string()),
            }
        };
        
        // 取消心跳任务
        heartbeat_handle.abort();
        let _ = heartbeat_handle.await;
        
        match result {
            Ok(response) => {
                self.emit_llm_progress("completed", &completed_msg, 0, &model_name, None);
                Ok(response)
            }
            Err(e) => {
                let is_timeout = e.contains("超时");
                self.emit_llm_progress("error", &e, if is_timeout { 600 } else { 0 }, &model_name, None);
                Err(e)
            }
        }
    }

    /// 流式生成文本（带整体 90 秒超时 + chunk 15 秒超时）
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
    ) -> Result<(), String> {
        let start_time = std::time::Instant::now();

        let profile = self.get_active_profile()
            .ok_or("No active LLM profile configured")?;

        // 构建增强提示词
        let enhanced_prompt = self.build_writing_prompt(&prompt, context.as_deref());

        log::info!("[LLM] Starting stream generation with request_id: {}", request_id);
        log::debug!("[LLM] Prompt: {}...", &enhanced_prompt[..enhanced_prompt.len().min(100)]);

        let adapter = self.create_adapter(&profile)?;

        let request = GenerateRequest {
            prompt: enhanced_prompt,
            max_tokens,
            temperature,
        };

        // 整体流式生成启动超时 30 秒（建立连接 + 收到第一个 chunk）
        let mut rx = timeout(Duration::from_secs(30), adapter.generate_stream(request))
            .await
            .map_err(|_| "模型连接超时（30秒内未开始响应）".to_string())?
            .map_err(|e| format!("Stream setup failed: {}", e))?;

        let mut full_text = String::new();
        let mut is_first = true;

        // 注册取消通道
        let (cancel_tx, mut cancel_rx) = tokio::sync::mpsc::channel::<()>(1);
        {
            let mut senders = self.cancel_senders.lock().unwrap();
            senders.insert(request_id.clone(), cancel_tx);
        }

        // chunk 超时：15 秒没有收到新数据就中断
        let chunk_timeout = Duration::from_secs(15);

        loop {
            tokio::select! {
                chunk_result = timeout(chunk_timeout, rx.recv()) => {
                    match chunk_result {
                        Ok(Some(Ok(chunk))) => {
                            full_text.push_str(&chunk);
                            let stream_chunk = StreamChunk {
                                chunk,
                                is_first,
                                is_last: false,
                                model: profile.model.clone(),
                            };
                            let _ = self.app_handle.emit(&format!("llm-stream-chunk-{}", request_id), stream_chunk);
                            is_first = false;
                        }
                        Ok(Some(Err(e))) => {
                            let _ = self.cancel_senders.lock().unwrap().remove(&request_id);
                            let error = GenerationError {
                                error: e.to_string(),
                                error_code: "STREAM_ERROR".to_string(),
                            };
                            let _ = self.app_handle.emit(&format!("llm-stream-error-{}", request_id), error);
                            return Err(format!("Stream error: {}", e));
                        }
                        Ok(None) => break,
                        Err(_) => {
                            // chunk 超时
                            let _ = self.cancel_senders.lock().unwrap().remove(&request_id);
                            let error = GenerationError {
                                error: "模型响应超时（15秒内未收到新数据）".to_string(),
                                error_code: "CHUNK_TIMEOUT".to_string(),
                            };
                            let _ = self.app_handle.emit(&format!("llm-stream-error-{}", request_id), error);
                            return Err("Stream chunk timed out after 15 seconds".to_string());
                        }
                    }
                }
                _ = cancel_rx.recv() => {
                    log::info!("[LLM] Generation cancelled for request_id: {}", request_id);
                    break;
                }
            }
        }

        let _ = self.cancel_senders.lock().unwrap().remove(&request_id);

        // 发送完成事件
        let duration = start_time.elapsed().as_millis() as u64;
        let complete = GenerationComplete {
            full_text: full_text.clone(),
            model: profile.model.clone(),
            tokens_used: full_text.len() as i32 / 2, // 粗略估计
            cost: 0.001, // 粗略估计
            duration_ms: duration,
        };

        let _ = self.app_handle.emit(&format!("llm-stream-complete-{}", request_id), complete);
        
        log::info!("[LLM] Stream generation completed in {}ms", duration);
        
        Ok(())
    }

    /// 构建写作专用提示词
    fn build_writing_prompt(&self, user_input: &str, context: Option<&str>) -> String {
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


    /// 测试连接
    pub async fn test_connection(&self) -> Result<(bool, u64), String> {
        let _profile = self.get_active_profile()
            .ok_or("No active LLM profile configured")?;
        
        let start = std::time::Instant::now();
        
        // 发送一个简单的测试请求
        let test_prompt = "Hello, respond with 'OK' only.";
        
        match self.generate(test_prompt.to_string(), Some(10), Some(0.0)).await {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                Ok((true, latency))
            }
            Err(e) => Err(e),
        }
    }

    /// 取消指定 request_id 的流式生成
    pub fn cancel_generation(&self, request_id: &str) {
        let mut senders = self.cancel_senders.lock().unwrap();
        if let Some(sender) = senders.remove(request_id) {
            let _ = sender.try_send(());
            log::info!("[LLM] Cancel signal sent for request_id: {}", request_id);
        } else {
            log::warn!("[LLM] No active generation found for request_id: {}", request_id);
        }
    }
}

/// 全局LLM服务实例
static LLM_SERVICE: once_cell::sync::OnceCell<std::sync::Mutex<Option<LlmService>>> = once_cell::sync::OnceCell::new();

/// 初始化LLM服务
pub fn init_llm_service(app_handle: AppHandle) {
    let service = LlmService::new(app_handle);
    let _ = LLM_SERVICE.set(std::sync::Mutex::new(Some(service)));
}

/// 获取LLM服务
pub fn get_llm_service() -> Option<LlmService> {
    LLM_SERVICE.get()
        .and_then(|s| s.lock().ok())
        .and_then(|s| s.as_ref().cloned())
}

impl Clone for LlmService {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            config: Arc::clone(&self.config),
            cancel_senders: Arc::clone(&self.cancel_senders),
        }
    }
}


