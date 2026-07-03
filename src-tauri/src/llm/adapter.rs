#![allow(dead_code)]
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

/// 适配器内部用于标识连接阶段超时的错误标记。
pub const CONNECTION_TIMEOUT_MARKER: &str = "LLM_CONNECTION_TIMEOUT";
/// 适配器内部用于标识生成/读取阶段超时的错误标记。
pub const GENERATION_TIMEOUT_MARKER: &str = "LLM_GENERATION_TIMEOUT";

/// 发送 HTTP 请求并在连接阶段应用超时。
/// 超时或 reqwest 连接错误均映射为 CONNECTION_TIMEOUT_MARKER。
pub async fn send_with_connection_timeout(
    request: reqwest::RequestBuilder,
    connect_timeout: Duration,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    match timeout(connect_timeout, request.send()).await {
        Ok(Ok(resp)) => Ok(resp),
        Ok(Err(e)) => {
            if e.is_timeout() {
                Err(CONNECTION_TIMEOUT_MARKER.into())
            } else {
                Err(e.into())
            }
        }
        Err(_) => Err(CONNECTION_TIMEOUT_MARKER.into()),
    }
}

/// 以流式方式读取响应体。
///
/// 超时策略（v0.14.0 三层防护，修复 vllm "连接成功但首字节迟迟不来"半挂问题）：
/// 1. **首字节超时**：第一个 chunk 使用 `min(generation_timeout, 60s)`，避免
///    vllm 连接建立后长时间不发任何字节时等满 240s。
/// 2. **per-chunk 超时**：后续每个 chunk 用 `generation_timeout`，允许本地模型
///    慢速但持续输出。
/// 3. **绝对超时**：从开始读取到结束不超过 `generation_timeout * 1.5`，防止
///    vllm 偶发吐字节反复刷新 per-chunk 计时器导致无限挂起。
pub async fn read_body_with_generation_timeout(
    response: reqwest::Response,
    generation_timeout: Duration,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut stream = response.bytes_stream();
    let mut chunks: Vec<Vec<u8>> = Vec::new();

    // 绝对截止时间：generation_timeout 的 1.5 倍，作为最后防线
    let absolute_deadline = tokio::time::Instant::now() + generation_timeout * 3 / 2;
    // 首字节超时：最多 60 秒，防止服务端连接成功但不响应
    let first_chunk_timeout = generation_timeout.min(Duration::from_secs(60));
    let mut first = true;

    loop {
        let chunk_timeout = if first {
            first_chunk_timeout
        } else {
            generation_timeout
        };
        // 取 per-chunk 超时与绝对截止时间的较早者
        let effective_deadline = tokio::time::Instant::now()
            .checked_add(chunk_timeout)
            .unwrap_or(absolute_deadline)
            .min(absolute_deadline);

        match tokio::time::timeout_at(effective_deadline, stream.next()).await {
            Ok(Some(Ok(bytes))) => {
                chunks.push(bytes.to_vec());
                first = false;
            }
            Ok(Some(Err(e))) => return Err(e.into()),
            Ok(None) => break,
            Err(_) => {
                // 判断是绝对超时还是 per-chunk/首字节超时
                if tokio::time::Instant::now() >= absolute_deadline {
                    return Err(format!(
                        "{} (absolute deadline exceeded after {:?})",
                        GENERATION_TIMEOUT_MARKER,
                        generation_timeout * 3 / 2
                    )
                    .into());
                }
                return Err(GENERATION_TIMEOUT_MARKER.into());
            }
        }
    }
    Ok(chunks.into_iter().flatten().collect())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFormat {
    JsonObject,
}

impl ResponseFormat {
    /// OpenAI / OpenAI-compatible API 要求的对象格式：`{"type":"json_object"}`
    pub fn openai_value(&self) -> serde_json::Value {
        match self {
            Self::JsonObject => serde_json::json!({"type": "json_object"}),
        }
    }

    /// Ollama `format` 字段接受的字符串：`"json"`
    pub fn ollama_value(&self) -> &'static str {
        match self {
            Self::JsonObject => "json",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// 结构化输出格式。OpenAI/Ollama 适配器会映射为对应 API 字段；Anthropic
    /// 暂不支持， 仍靠 prompt 约束输出 JSON。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    /// v0.23.61: 系统提示词覆盖（替换适配器 hardcoded 默认值）。
    /// 优先级：LlmProfile.system_prompt_override >
    /// AppConfig.writer_system_prompt_override > 适配器内置默认。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// v0.26.0: 生成链路 trace_id，透传到进度事件与 trace 存储
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub trace_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub content: String,
    pub model: String,
    pub tokens_used: i32,
    pub cost: f64,
}

#[async_trait::async_trait]
pub trait LlmAdapter: Send + Sync {
    async fn generate(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, Box<dyn std::error::Error>>;

    async fn generate_stream(
        &self,
        request: GenerateRequest,
    ) -> Result<
        tokio::sync::mpsc::Receiver<Result<String, Box<dyn std::error::Error + Send + Sync>>>,
        Box<dyn std::error::Error + Send + Sync>,
    >;

    fn model_name(&self) -> String;

    /// 克隆自身为新的 Box<dyn LlmAdapter>，用于缓存复用
    fn box_clone(&self) -> Box<dyn LlmAdapter>;
}
