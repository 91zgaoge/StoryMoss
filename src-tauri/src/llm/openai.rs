use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{
    adapter::ToolCall, extract_openai_tool_calls, openai_tools_payload, GenerateRequest,
    GenerateResponse, LlmAdapter,
};

fn deserialize_null_content<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Clone)]
pub struct OpenAiAdapter {
    client: Client,
    api_key: String,
    model: String,
    api_base: String,
    default_max_tokens: i32,
    default_temperature: f32,
    generation_timeout: std::time::Duration,
    connect_timeout: std::time::Duration,
    first_chunk_timeout: std::time::Duration,
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: i32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    #[serde(default, deserialize_with = "deserialize_null_content")]
    content: String,
    /// v0.30.25: DeepSeek 等推理模型把思维链放在 reasoning_content 字段，
    /// content 可能为空。serde default 确保非推理模型不受影响。
    #[serde(skip_serializing, default)]
    reasoning_content: Option<String>,
    /// 原生 function calling；出站消息不序列化此字段。
    #[serde(default, skip_serializing)]
    tool_calls: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    model: String,
    usage: Usage,
    choices: Vec<Choice>,
}

#[derive(Debug, Serialize)]
struct OpenAiStreamRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: i32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    presence_penalty: Option<f32>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiDelta,
}

#[derive(Debug, Deserialize, Default)]
struct OpenAiDelta {
    content: Option<String>,
    /// v0.30.25: 推理模型流式响应的 reasoning_content delta
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamResponse {
    choices: Vec<OpenAiStreamChoice>,
}

/// OpenAI 兼容 API 要求 `top_p` 落在 `(0, 1.0]`；`0` 或不合法值会被服务端拒绝。
/// 过滤后返回 `None` 可使字段不被序列化，让服务端使用默认值。
fn sanitize_top_p(top_p: Option<f32>) -> Option<f32> {
    top_p.filter(|v| *v > 0.0 && *v <= 1.0)
}

#[derive(Debug, Deserialize)]
struct Usage {
    total_tokens: i32,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

/// v0.30.45: 推理模型（DeepSeek 等）的 reasoning_content 是**思维链**（CoT），
/// 不是正文。v0.30.25 的回退假设是错误的--它把 CoT 当正文返回，导致用户看到
/// "这是一个小说续写任务，需要我以专业作者身份..."等推理过程而非小说正文。
/// 现在当 content 为空时不再回退到 reasoning_content，而是返回空字符串，
/// 让调用方处理（重试或报错）。
fn resolve_content(content: &str, reasoning_content: &Option<String>) -> String {
    if content.is_empty() {
        if let Some(ref rc) = reasoning_content {
            if !rc.is_empty() {
                log::warn!(
                    "[OpenAI] content 为空但 reasoning_content 有 {} 字符（思维链/CoT）。\
                     不回退到 reasoning_content--它是模型的推理过程而非正文。\
                     调用方应处理空 content（重试或报错）。",
                    rc.chars().count()
                );
            }
        }
    }
    content.to_string()
}

/// 已解析的 chat completion（`resolve_content` 与 tool_calls 提取均已应用）。
#[derive(Debug)]
struct ParsedCompletion {
    content: String,
    model: String,
    total_tokens: i32,
    tool_calls: Vec<ToolCall>,
}

/// 解析 OpenAI 兼容网关 `/chat/completions` 的响应体。
///
/// 纯函数（无网络、无 &self）：`generate()` 与故障注入测试共用同一条解析
/// 路径。测试直接喂网关真实返回形状的原始 JSON（`reasoning_content`、
/// `content: null`、`finish_reason: "length"`、截断体、空体等），断言行为。
///
/// 关键不变量（v0.30.45）：`content` 为空时**不得**回退到
/// `reasoning_content`——那是思维链（CoT）不是正文。
fn parse_chat_completion_bytes(bytes: &[u8]) -> Result<ParsedCompletion, serde_json::Error> {
    let resp: OpenAiResponse = serde_json::from_slice(bytes)?;
    let first = resp.choices.first();
    let content = first
        .map(|c| resolve_content(&c.message.content, &c.message.reasoning_content))
        .unwrap_or_default();
    let tool_calls = first
        .map(|c| {
            extract_openai_tool_calls(&serde_json::json!({
                "tool_calls": c.message.tool_calls,
            }))
        })
        .unwrap_or_default();
    Ok(ParsedCompletion {
        content,
        model: resp.model,
        total_tokens: resp.usage.total_tokens,
        tool_calls,
    })
}

/// 单行 SSE 的分类（`generate_stream` 读取循环的纯函数核心）。
#[derive(Debug, PartialEq, Eq)]
enum SseLine<'a> {
    /// 非数据行（空行、`event:`/`id:` 字段、注释）——跳过，不算错误。
    Ignore,
    /// `data: [DONE]`——流正常结束。
    Done,
    /// `data: {...}` JSON 负载。
    Data(&'a str),
}

/// 判定单行 SSE 的语义。与网关约定一致：只认带空格前缀的 `data: `。
fn classify_sse_line(line: &str) -> SseLine<'_> {
    if line.is_empty() || !line.starts_with("data: ") {
        return SseLine::Ignore;
    }
    let data = &line[6..];
    if data == "[DONE]" {
        return SseLine::Done;
    }
    SseLine::Data(data)
}

/// 从单个 SSE `data:` 负载提取正文增量。
///
/// - `Ok(Some(text))`：正文增量。
/// - `Ok(None)`：无正文增量（空 delta、`choices` 为空、或**只有
///   `reasoning_content`**——思维链不得当作正文转发）。
/// - `Err`：负载不是合法 JSON（截断/半包）；调用方发错误并终止流。
fn content_delta_from_sse_payload(data: &str) -> Result<Option<String>, serde_json::Error> {
    let parsed: OpenAiStreamResponse = serde_json::from_str(data)?;
    Ok(parsed
        .choices
        .first()
        .and_then(|choice| choice.delta.content.as_ref())
        .filter(|c| !c.is_empty())
        .cloned())
}

impl OpenAiAdapter {
    pub fn new(
        api_key: String,
        model: String,
        api_base: Option<String>,
        max_tokens: i32,
        temperature: f32,
        timeout_seconds: u64,
        connect_timeout_seconds: u64,
        first_chunk_timeout_seconds: u64,
    ) -> Self {
        let generation_timeout = if timeout_seconds > 0 {
            Duration::from_secs(timeout_seconds)
        } else {
            Duration::from_secs(300)
        };
        let connect_timeout = if connect_timeout_seconds > 0 {
            Duration::from_secs(connect_timeout_seconds)
        } else {
            Duration::from_secs(10)
        };
        let first_chunk_timeout = if first_chunk_timeout_seconds > 0 {
            Duration::from_secs(first_chunk_timeout_seconds)
        } else {
            Duration::from_secs(60)
        };
        // v0.11.8: 不再设置 reqwest 全局 timeout；由 generate 内部分阶段控制
        // 连接超时与生成超时，并在读取流时按 chunk 刷新计时器。
        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            api_key,
            model,
            api_base: api_base.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            default_max_tokens: max_tokens,
            default_temperature: temperature,
            generation_timeout,
            connect_timeout,
            first_chunk_timeout,
        }
    }

    fn calculate_cost(&self, model: &str, tokens: i32) -> f64 {
        // Pricing per 1K tokens (as of 2024)
        let rate = match model {
            "gpt-4" => 0.03,
            "gpt-4-turbo" => 0.01,
            "gpt-3.5-turbo" => 0.002,
            _ => 0.002,
        };
        (tokens as f64 / 1000.0) * rate
    }

    fn build_messages(&self, prompt: String, system_prompt: Option<&str>) -> Vec<Message> {
        let system_content = system_prompt
            .filter(|s| !s.trim().is_empty())
            .unwrap_or("You are a professional creative writing assistant.");
        vec![
            Message {
                role: "system".to_string(),
                content: system_content.to_string(),
                reasoning_content: None,
                tool_calls: None,
            },
            Message {
                role: "user".to_string(),
                content: prompt,
                reasoning_content: None,
                tool_calls: None,
            },
        ]
    }
}

#[async_trait::async_trait]
impl LlmAdapter for OpenAiAdapter {
    async fn generate(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, Box<dyn std::error::Error>> {
        use super::adapter::{read_body_with_generation_timeout_ex, send_with_connection_timeout};

        let openai_req = OpenAiRequest {
            model: self.model.clone(),
            messages: self.build_messages(request.prompt, request.system_prompt.as_deref()),
            max_tokens: request.max_tokens.unwrap_or(self.default_max_tokens),
            temperature: request.temperature.unwrap_or(self.default_temperature),
            top_p: sanitize_top_p(request.top_p),
            frequency_penalty: request.frequency_penalty,
            presence_penalty: request.presence_penalty,
            response_format: request.response_format.as_ref().map(|f| f.openai_value()),
            tools: request
                .tools
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(|s| openai_tools_payload(s)),
        };

        let primary_url = format!("{}/chat/completions", self.api_base);
        let fallback_url = format!("{}/v1/chat/completions", self.api_base);

        let mut response = send_with_connection_timeout(
            self.client
                .post(&primary_url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&openai_req),
            self.connect_timeout,
        )
        .await?;

        // Ollama 等本地服务的 OpenAI 兼容 API 使用 /v1/chat/completions
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            response = send_with_connection_timeout(
                self.client
                    .post(&fallback_url)
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Content-Type", "application/json")
                    .json(&openai_req),
                self.connect_timeout,
            )
            .await?;
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("OpenAI API error: {}", error_text).into());
        }

        // v0.11.8: 流式读取响应体，每收到 chunk 刷新一次生成超时计时器。
        let bytes = read_body_with_generation_timeout_ex(
            response,
            self.generation_timeout,
            self.first_chunk_timeout,
        )
        .await?;

        // 将同步 JSON 反序列化隔离到 blocking 线程池，避免大响应阻塞 async runtime。
        let parsed = tokio::task::spawn_blocking(move || parse_chat_completion_bytes(&bytes))
            .await
            .map_err(|e| format!("deserialization task panicked: {}", e))?
            .map_err(|e| format!("OpenAI response parse error: {}", e))?;

        let cost = self.calculate_cost(&parsed.model, parsed.total_tokens);

        Ok(GenerateResponse {
            content: parsed.content,
            model: parsed.model,
            tokens_used: parsed.total_tokens,
            cost,
            tool_calls: parsed.tool_calls,
        })
    }

    async fn generate_stream(
        &self,
        request: GenerateRequest,
    ) -> Result<
        tokio::sync::mpsc::Receiver<Result<String, Box<dyn std::error::Error + Send + Sync>>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let openai_req = OpenAiStreamRequest {
            model: self.model.clone(),
            messages: self.build_messages(request.prompt, request.system_prompt.as_deref()),
            max_tokens: request.max_tokens.unwrap_or(self.default_max_tokens),
            temperature: request.temperature.unwrap_or(self.default_temperature),
            top_p: sanitize_top_p(request.top_p),
            frequency_penalty: request.frequency_penalty,
            presence_penalty: request.presence_penalty,
            stream: true,
            response_format: request.response_format.as_ref().map(|f| f.openai_value()),
        };

        let mut response = self
            .client
            .post(format!("{}/chat/completions", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&openai_req)
            .send()
            .await?;

        // Ollama 等本地服务的 OpenAI 兼容 API 使用 /v1/chat/completions
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            response = self
                .client
                .post(format!("{}/v1/chat/completions", self.api_base))
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json")
                .json(&openai_req)
                .send()
                .await?;
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("OpenAI API error: {}", error_text).into());
        }

        let (tx, rx) = tokio::sync::mpsc::channel::<
            Result<String, Box<dyn std::error::Error + Send + Sync>>,
        >(128);

        tokio::spawn(async move {
            use futures_util::StreamExt;
            use tokio::io::AsyncBufReadExt;

            let stream = response.bytes_stream().map(|result| {
                result.map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err))
            });
            let reader = tokio_util::io::StreamReader::new(stream);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                // v0.30.45: 只转发 content delta，不回退 reasoning_content。
                // reasoning_content 是思维链（CoT），不是正文。
                match classify_sse_line(&line) {
                    SseLine::Ignore => continue,
                    SseLine::Done => break,
                    SseLine::Data(data) => match content_delta_from_sse_payload(data) {
                        Ok(Some(content)) => {
                            if tx.send(Ok(content)).await.is_err() {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(e) => {
                            let _ = tx.send(Err(format!("SSE parse error: {}", e).into())).await;
                            break;
                        }
                    },
                }
            }
        });

        Ok(rx)
    }

    fn model_name(&self) -> String {
        self.model.clone()
    }

    fn box_clone(&self) -> Box<dyn super::LlmAdapter> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_sse_line, content_delta_from_sse_payload, parse_chat_completion_bytes,
        resolve_content, sanitize_top_p, SseLine,
    };

    #[test]
    fn sanitize_top_p_keeps_valid_values() {
        assert_eq!(sanitize_top_p(None), None);
        assert_eq!(sanitize_top_p(Some(0.0)), None);
        assert_eq!(sanitize_top_p(Some(-0.1)), None);
        assert_eq!(sanitize_top_p(Some(1.1)), None);
        assert_eq!(sanitize_top_p(Some(0.1)), Some(0.1));
        assert_eq!(sanitize_top_p(Some(0.5)), Some(0.5));
        assert_eq!(sanitize_top_p(Some(1.0)), Some(1.0));
    }

    // ===== v0.30.45: reasoning_content 不再回退（它是 CoT 不是正文）=====

    #[test]
    fn resolve_content_uses_content_when_nonempty() {
        let rc = Some("思维链内容".to_string());
        assert_eq!(resolve_content("实际回答", &rc), "实际回答");
        assert_eq!(resolve_content("实际回答", &None), "实际回答");
    }

    #[test]
    fn resolve_content_does_not_fall_back_to_reasoning() {
        // v0.30.45: content 为空时不再回退到 reasoning_content
        // reasoning_content 是思维链（CoT），不是正文
        let rc = Some("这是推理模型的思维链".to_string());
        assert_eq!(resolve_content("", &rc), "");
    }

    #[test]
    fn resolve_content_returns_empty_when_both_empty() {
        assert_eq!(resolve_content("", &None), "");
        assert_eq!(resolve_content("", &Some("".to_string())), "");
    }

    #[test]
    fn message_deserializes_with_reasoning_content() {
        use super::Message;
        let json = r#"{"role":"assistant","content":"","reasoning_content":"推理内容"}"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "");
        assert_eq!(msg.reasoning_content.as_deref(), Some("推理内容"));
    }

    #[test]
    fn message_deserializes_without_reasoning_content() {
        use super::Message;
        // 非推理模型不返回 reasoning_content，serde default 确保不受影响
        let json = r#"{"role":"assistant","content":"正常回答"}"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "正常回答");
        assert!(msg.reasoning_content.is_none());
    }

    #[test]
    fn message_deserializes_null_content_with_tool_calls() {
        use super::Message;
        let json = r#"{"role":"assistant","content":null,"tool_calls":[{"id":"c1","type":"function","function":{"name":"board_read","arguments":"{\"zone\":\"asset\"}"}}]}"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content, "");
        assert!(msg.tool_calls.as_ref().is_some_and(|c| !c.is_empty()));
    }

    // ===== 故障注入：真实网关 payload 形状（无网络，纯解析路径）=====
    //
    // 覆盖历史真机故障（AGENTS.md v0.30.45 / v0.51.3；executor v0.30.51）：
    //   1) 推理模型把思维链放 `reasoning_content`，`content` 为空
    //   2) markdown 围栏 JSON（严格 serde_json::from_str 会失败）
    //   3) 截断/空响应体（200 但 body 不完整）
    //   4) 空正文候选回退（网关层 `content.trim().is_empty()` 视为失败）

    /// 组装与 OpenAI 兼容网关一致形状的响应体（含 `id`/`object`/`created`/
    /// `finish_reason`/`usage.prompt_tokens` 等生产字段）。
    fn gateway_payload(message_json: &str, finish_reason: &str) -> String {
        format!(
            r#"{{"id":"chatcmpl-9xYz","object":"chat.completion","created":1730000000,"model":"deepseek-v4","choices":[{{"index":0,"message":{message_json},"finish_reason":"{finish_reason}"}}],"usage":{{"prompt_tokens":1200,"completion_tokens":2048,"total_tokens":3248}}}}"#
        )
    }

    #[test]
    fn payload_reasoning_only_never_returns_cot_as_prose() {
        // 真机故障 1：200 + usage 正常，但 content 为空、思维链在
        // reasoning_content（token 全烧在推理上，finish_reason=length）。
        let raw = gateway_payload(
            r#"{"role":"assistant","content":"","reasoning_content":"这是一个小说续写任务，需要我以专业作者身份完成。让我先梳理节拍卡与状态网……"}"#,
            "length",
        );
        let parsed = parse_chat_completion_bytes(raw.as_bytes()).unwrap();
        assert!(
            parsed.content.is_empty(),
            "content 为空时不得回退思维链，实际: {}",
            parsed.content
        );
        assert!(!parsed.content.contains("小说续写任务"));
        // 元数据仍可用，调用方可据此归因（推理模型烧完 token）。
        assert_eq!(parsed.model, "deepseek-v4");
        assert_eq!(parsed.total_tokens, 3248);
        assert!(parsed.tool_calls.is_empty());
    }

    #[test]
    fn payload_null_content_with_finish_reason_length_yields_empty() {
        // 网关偶发把 content 序列化为 null（deserialize_null_content 归一空串）。
        let raw = gateway_payload(
            r#"{"role":"assistant","content":null,"reasoning_content":"先思考再输出正文"}"#,
            "length",
        );
        let parsed = parse_chat_completion_bytes(raw.as_bytes()).unwrap();
        assert_eq!(parsed.content, "");
    }

    #[test]
    fn payload_whitespace_only_content_is_not_backfilled_with_reasoning() {
        // 只有空白字符的 content 同样不得被思维链顶替；空白本身原样返回，
        // 由网关层（executor: content.trim().is_empty()）判为候选失败。
        let raw = gateway_payload(
            r#"{"role":"assistant","content":"  \n\t ","reasoning_content":"思维链正文"}"#,
            "stop",
        );
        let parsed = parse_chat_completion_bytes(raw.as_bytes()).unwrap();
        assert_eq!(parsed.content, "  \n\t ");
        assert!(parsed.content.trim().is_empty());
        assert!(!parsed.content.contains("思维链"));
    }

    #[test]
    fn payload_content_wins_over_reasoning_when_both_present() {
        let raw = gateway_payload(
            r#"{"role":"assistant","content":"血雾还没落尽，苏会山向西跨院踉跄。","reasoning_content":"我需要让读者感到压迫……"}"#,
            "stop",
        );
        let parsed = parse_chat_completion_bytes(raw.as_bytes()).unwrap();
        assert_eq!(parsed.content, "血雾还没落尽，苏会山向西跨院踉跄。");
        assert!(!parsed.content.contains("我需要让读者"));
    }

    #[test]
    fn payload_empty_choices_yields_empty_content_without_panic() {
        let raw = r#"{"id":"c","object":"chat.completion","created":1,"model":"m","choices":[],"usage":{"prompt_tokens":1,"completion_tokens":0,"total_tokens":1}}"#;
        let parsed = parse_chat_completion_bytes(raw.as_bytes()).unwrap();
        assert_eq!(parsed.content, "");
        assert!(parsed.tool_calls.is_empty());
    }

    #[test]
    fn payload_tool_calls_survive_null_content() {
        let raw = gateway_payload(
            r#"{"role":"assistant","content":null,"tool_calls":[{"id":"call_1","type":"function","function":{"name":"board_read","arguments":"{\"zone\":\"asset\"}"}}]}"#,
            "tool_calls",
        );
        let parsed = parse_chat_completion_bytes(raw.as_bytes()).unwrap();
        assert_eq!(parsed.content, "");
        assert_eq!(parsed.tool_calls.len(), 1);
        assert_eq!(parsed.tool_calls[0].name, "board_read");
        assert_eq!(parsed.tool_calls[0].arguments["zone"], "asset");
    }

    #[test]
    fn payload_truncated_json_fails_without_panic() {
        // 真机故障 3：连接中断/超时导致响应体被截断（字符串中间截断）。
        let full = gateway_payload(
            r#"{"role":"assistant","content":"风声穿过回廊，烛火摇晃。"}"#,
            "stop",
        );
        let cut = full.len() * 3 / 4;
        let truncated = full[..cut].to_string();
        let err = parse_chat_completion_bytes(truncated.as_bytes())
            .expect_err("截断体必须返回 Err 而不是 panic");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn payload_empty_body_fails_without_panic() {
        assert!(parse_chat_completion_bytes(b"").is_err());
        assert!(parse_chat_completion_bytes(b"   ").is_err());
        assert!(parse_chat_completion_bytes(b"<html>502 Bad Gateway</html>").is_err());
    }

    #[test]
    fn payload_fenced_json_content_is_recoverable_for_callers() {
        // 真机故障 2：模型把 JSON 包在 ```json 围栏里、字符串值内裸换行。
        // 适配器只负责取出 content（不解析业务 JSON）；调用方的
        // extract_and_sanitize_json 必须能恢复（含 `, ]` / `, }` 同行尾随逗号）。
        let raw = gateway_payload(
            r#"{"role":"assistant","content":"```json\n{\n  \"story_outline\": \"第一幕：雨夜对决\n第二幕：真相浮现\",\n  \"scene_outline\": \"钟楼对峙\",\n  \"characters\": [\"苏亦铁\",]\n}\n```\n以上是设定。"}"#,
            "stop",
        );
        let parsed = parse_chat_completion_bytes(raw.as_bytes()).unwrap();
        assert!(parsed.content.contains("story_outline"));
        let json = crate::narrative::extract_and_sanitize_json(&parsed.content)
            .expect("围栏 JSON 必须能被 extract_and_sanitize_json 恢复");
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["scene_outline"], "钟楼对峙");
    }

    #[test]
    fn payload_truncated_fenced_json_fails_gracefully() {
        let raw = gateway_payload(
            r#"{"role":"assistant","content":"```json\n{\"story_outline\": \"第一幕：雨夜对"}"#,
            "length",
        );
        let parsed = parse_chat_completion_bytes(raw.as_bytes()).unwrap();
        assert!(
            crate::narrative::extract_and_sanitize_json(&parsed.content).is_err(),
            "截断的围栏 JSON 必须返回 Err（调用方走 salvage/重试），不得 panic"
        );
    }

    // ===== SSE 故障注入（generate_stream 的纯函数核心）=====

    #[test]
    fn sse_line_classification_pins_gateway_framing() {
        assert_eq!(classify_sse_line(""), SseLine::Ignore);
        assert_eq!(classify_sse_line(": keep-alive"), SseLine::Ignore);
        assert_eq!(classify_sse_line("event: message"), SseLine::Ignore);
        // 无空格前缀不是合法 data 行（与生产实现一致，不得被当成负载）。
        assert_eq!(classify_sse_line("data:{\"choices\":[]}"), SseLine::Ignore);
        assert_eq!(classify_sse_line("data: [DONE]"), SseLine::Done);
        assert_eq!(
            classify_sse_line("data: {\"choices\":[]}"),
            SseLine::Data("{\"choices\":[]}")
        );
    }

    #[test]
    fn sse_reasoning_only_delta_is_not_forwarded() {
        // 真机故障 1 的流式形态：reasoning_content delta 一个接一个，
        // content delta 从不出现 → 零正文增量（调用方最终看到空内容）。
        let line = r#"{"choices":[{"index":0,"delta":{"reasoning_content":"我需要先梳理剧情推进方向……"}}]}"#;
        assert_eq!(content_delta_from_sse_payload(line).unwrap(), None);
    }

    #[test]
    fn sse_content_delta_and_empty_delta() {
        let line = r#"{"choices":[{"index":0,"delta":{"content":"血雾还没落尽"}}]}"#;
        assert_eq!(
            content_delta_from_sse_payload(line).unwrap(),
            Some("血雾还没落尽".to_string())
        );
        // finish 帧：delta 为空对象，无正文增量。
        assert_eq!(
            content_delta_from_sse_payload(r#"{"choices":[{"index":0,"delta":{}}]}"#).unwrap(),
            None
        );
        // 空 choices（部分网关的心跳帧）。
        assert_eq!(
            content_delta_from_sse_payload(r#"{"choices":[]}"#).unwrap(),
            None
        );
        // 空字符串 delta 不得被转发（会污染拼装结果）。
        assert_eq!(
            content_delta_from_sse_payload(r#"{"choices":[{"index":0,"delta":{"content":""}}]}"#)
                .unwrap(),
            None
        );
    }

    #[test]
    fn sse_partial_or_truncated_payload_is_error_not_panic() {
        // 半包/截断的 SSE 负载：返回 Err，由调用方发 "SSE parse error" 并终止流。
        assert!(
            content_delta_from_sse_payload(r#"{"choices":[{"delta":{"content":"血雾"#).is_err()
        );
        assert!(content_delta_from_sse_payload("").is_err());
        assert!(content_delta_from_sse_payload("<html>").is_err());
    }

    #[test]
    fn openai_request_omits_tools_when_none() {
        use super::{Message, OpenAiRequest};
        let req = OpenAiRequest {
            model: "gpt".into(),
            messages: vec![Message {
                role: "user".into(),
                content: "hi".into(),
                reasoning_content: None,
                tool_calls: None,
            }],
            max_tokens: 16,
            temperature: 0.2,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            response_format: None,
            tools: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("tools").is_none());
    }
}
