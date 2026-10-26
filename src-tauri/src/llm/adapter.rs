#![allow(dead_code)]
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::time::timeout;

/// 适配器内部用于标识连接阶段超时的错误标记。
pub const CONNECTION_TIMEOUT_MARKER: &str = "LLM_CONNECTION_TIMEOUT";
/// 适配器内部用于标识「连接已建立但未在首字节时限内返回响应头」的错误标记。
pub const RESPONSE_HEADERS_TIMEOUT_MARKER: &str = "LLM_RESPONSE_HEADERS_TIMEOUT";
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

/// 发送 HTTP 请求并等待响应头（首字节），用于非流式调用。
///
/// v0.73.6: TCP/TLS 建连由 `ClientBuilder::connect_timeout` 负责；此处的
/// 超时是「连接建立后等模型返回响应头」的时限，语义对齐用户设置里的
/// 「首字节超时」。此前这里误用 `connect_timeout`（15s）：deepseek 等
/// 推理模型在非流式调用中原生思考阶段不返回响应头，15s 一到就被判成
/// 「连接模型超时」，重试也仍卡在同一阶段。
pub async fn send_with_response_timeout(
    request: reqwest::RequestBuilder,
    response_timeout: Duration,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    match timeout(response_timeout, request.send()).await {
        Ok(Ok(resp)) => Ok(resp),
        Ok(Err(e)) => {
            if e.is_timeout() {
                Err(CONNECTION_TIMEOUT_MARKER.into())
            } else {
                Err(e.into())
            }
        }
        Err(_) => Err(RESPONSE_HEADERS_TIMEOUT_MARKER.into()),
    }
}

/// 以流式方式读取响应体。
///
/// 超时策略（v0.14.0 三层防护，修复 vllm "连接成功但首字节迟迟不来"半挂问题）：
/// 1. **首字节超时**：第一个 chunk 使用 `min(generation_timeout,
///    first_chunk_cap)`， 避免 vllm 连接建立后长时间不发任何字节时等满
///    generation_timeout。
/// 2. **per-chunk 超时**：后续每个 chunk 用 `generation_timeout`，允许本地模型
///    慢速但持续输出。
/// 3. **绝对超时**：从开始读取到结束不超过 `generation_timeout * 1.5`，防止
///    vllm 偶发吐字节反复刷新 per-chunk 计时器导致无限挂起。
pub async fn read_body_with_generation_timeout(
    response: reqwest::Response,
    generation_timeout: Duration,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    read_body_with_generation_timeout_ex(response, generation_timeout, Duration::from_secs(60))
        .await
}

/// 同 [`read_body_with_generation_timeout`]，但允许自定义首字节超时上限
/// （来自 `AppConfig.llm_first_chunk_timeout_secs`）。
pub async fn read_body_with_generation_timeout_ex(
    response: reqwest::Response,
    generation_timeout: Duration,
    first_chunk_cap: Duration,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut stream = response.bytes_stream();
    let mut chunks: Vec<Vec<u8>> = Vec::new();

    // 绝对截止时间：generation_timeout 的 1.5 倍，作为最后防线
    let absolute_deadline = tokio::time::Instant::now() + generation_timeout * 3 / 2;
    // 首字节超时：不超过 first_chunk_cap，防止服务端连接成功但不响应
    let first_chunk_timeout = generation_timeout.min(first_chunk_cap);
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

/// 发给模型的原生工具定义（OpenAI/Ollama function calling / Anthropic tools）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema object（`{"type":"object","properties":...}`）。
    pub parameters: serde_json::Value,
}

/// 模型返回的一次原生工具调用。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Agency `args_schema` 是「字段→中文说明」对象，不是 JSON Schema。
/// 已是 `{"type":"object",...}` 则原样返回。
pub fn informal_args_to_json_schema(args: &serde_json::Value) -> serde_json::Value {
    if args.get("type").and_then(|v| v.as_str()) == Some("object") {
        return args.clone();
    }
    let mut properties = serde_json::Map::new();
    if let Some(obj) = args.as_object() {
        for (key, value) in obj {
            let description = value.as_str().unwrap_or("").to_string();
            properties.insert(
                key.clone(),
                serde_json::json!({
                    "type": "string",
                    "description": description,
                }),
            );
        }
    }
    serde_json::json!({
        "type": "object",
        "properties": properties,
    })
}

/// OpenAI / Ollama chat `tools` 数组。
pub fn openai_tools_payload(specs: &[ToolSpec]) -> Vec<serde_json::Value> {
    specs
        .iter()
        .map(|spec| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": spec.name,
                    "description": spec.description,
                    "parameters": spec.parameters,
                }
            })
        })
        .collect()
}

/// Anthropic messages `tools` 数组（`input_schema` 而非 `parameters`）。
pub fn anthropic_tools_payload(specs: &[ToolSpec]) -> Vec<serde_json::Value> {
    specs
        .iter()
        .map(|spec| {
            serde_json::json!({
                "name": spec.name,
                "description": spec.description,
                "input_schema": spec.parameters,
            })
        })
        .collect()
}

/// 从 OpenAI-compatible assistant message JSON 抽出 tool_calls。
pub fn extract_openai_tool_calls(message: &serde_json::Value) -> Vec<ToolCall> {
    let Some(calls) = message.get("tool_calls").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    calls
        .iter()
        .filter_map(|call| {
            let id = call
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let function = call.get("function")?;
            let name = function.get("name")?.as_str()?.to_string();
            let arguments = match function.get("arguments") {
                Some(serde_json::Value::String(s)) => {
                    serde_json::from_str(s).unwrap_or(serde_json::json!({}))
                }
                Some(other) => other.clone(),
                None => serde_json::json!({}),
            };
            Some(ToolCall {
                id,
                name,
                arguments,
            })
        })
        .collect()
}

/// 从 Anthropic `content` 数组抽出 `tool_use` 块。
pub fn extract_anthropic_tool_use(blocks: &serde_json::Value) -> Vec<ToolCall> {
    let Some(items) = blocks.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|block| block.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
        .filter_map(|block| {
            Some(ToolCall {
                id: block.get("id")?.as_str()?.to_string(),
                name: block.get("name")?.as_str()?.to_string(),
                arguments: block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})),
            })
        })
        .collect()
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
    /// v0.54.0: 原生 function calling。None 时请求体不带 tools 字段。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub tools: Option<Vec<ToolSpec>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub content: String,
    pub model: String,
    pub tokens_used: i32,
    pub cost: f64,
    /// v0.54.0: 原生工具调用。文本 JSON action 路径保持为空。
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
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

#[cfg(test)]
mod native_tools_contract {
    use super::*;

    #[test]
    fn generate_request_omits_tools_field_when_none() {
        let req = GenerateRequest {
            prompt: "hi".into(),
            ..Default::default()
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("tools").is_none());
        assert!(req.tools.is_none());
    }

    #[test]
    fn informal_args_become_json_schema_object() {
        let informal = serde_json::json!({
            "zone": "asset|draft",
            "key": "可选"
        });
        let schema = informal_args_to_json_schema(&informal);
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["zone"]["type"], "string");
        assert_eq!(schema["properties"]["zone"]["description"], "asset|draft");
        assert_eq!(schema["properties"]["key"]["description"], "可选");
    }

    #[test]
    fn already_json_schema_is_left_intact() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "zone": { "type": "string" }
            }
        });
        assert_eq!(informal_args_to_json_schema(&schema), schema);
    }

    #[test]
    fn openai_tools_payload_wraps_function() {
        let specs = [ToolSpec {
            name: "board_read".into(),
            description: "读黑板".into(),
            parameters: informal_args_to_json_schema(&serde_json::json!({"zone": "分区"})),
        }];
        let payload = openai_tools_payload(&specs);
        assert_eq!(payload[0]["type"], "function");
        assert_eq!(payload[0]["function"]["name"], "board_read");
        assert_eq!(payload[0]["function"]["parameters"]["type"], "object");
    }

    #[test]
    fn anthropic_tools_payload_uses_input_schema() {
        let specs = [ToolSpec {
            name: "story_info".into(),
            description: "故事信息".into(),
            parameters: informal_args_to_json_schema(&serde_json::json!({})),
        }];
        let payload = anthropic_tools_payload(&specs);
        assert_eq!(payload[0]["name"], "story_info");
        assert_eq!(payload[0]["input_schema"]["type"], "object");
        assert!(payload[0].get("parameters").is_none());
    }

    #[test]
    fn extract_openai_tool_calls_from_assistant_message() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": null,
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "board_read",
                    "arguments": "{\"zone\":\"asset\"}"
                }
            }]
        });
        let calls = extract_openai_tool_calls(&message);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "board_read");
        assert_eq!(calls[0].arguments["zone"], "asset");
        assert_eq!(calls[0].id, "call_1");
    }

    #[test]
    fn extract_openai_tool_calls_empty_when_text_only() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "hello"
        });
        assert!(extract_openai_tool_calls(&message).is_empty());
    }

    #[test]
    fn extract_anthropic_tool_use_from_content_blocks() {
        let blocks = serde_json::json!([
            { "type": "text", "text": "ok" },
            {
                "type": "tool_use",
                "id": "toolu_1",
                "name": "story_info",
                "input": {}
            }
        ]);
        let calls = extract_anthropic_tool_use(&blocks);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "story_info");
        assert_eq!(calls[0].id, "toolu_1");
        assert_eq!(calls[0].arguments, serde_json::json!({}));
    }

    /// v0.73.6: 连接已建立但服务端迟迟不返回响应头 → 必须归类为首字节超时，
    /// 不能被误判为「连接超时」（此前 15s 杀掉推理模型的思考阶段）。
    #[tokio::test]
    async fn response_headers_timeout_uses_dedicated_marker() {
        use tokio::{io::AsyncReadExt, net::TcpListener};

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            // 读完请求头后保持连接、不回任何响应字节
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            tokio::time::sleep(Duration::from_secs(5)).await;
        });

        let req = reqwest::Client::new()
            .post(format!("http://{}/v1/chat/completions", addr))
            .json(&serde_json::json!({"model": "x", "messages": []}));
        let err = send_with_response_timeout(req, Duration::from_millis(100))
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), RESPONSE_HEADERS_TIMEOUT_MARKER);
    }
}
