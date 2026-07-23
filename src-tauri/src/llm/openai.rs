use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{GenerateRequest, GenerateResponse, LlmAdapter};

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
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
    /// v0.30.25: DeepSeek 等推理模型把思维链放在 reasoning_content 字段，
    /// content 可能为空。serde default 确保非推理模型不受影响。
    #[serde(skip_serializing, default)]
    reasoning_content: Option<String>,
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

/// v0.30.25: 推理模型（DeepSeek 等）可能把实际内容放在 reasoning_content 而
/// content 为空。 此纯函数实现 fallback 逻辑，供单测验证。
fn resolve_content(content: &str, reasoning_content: &Option<String>) -> String {
    if content.is_empty() {
        if let Some(ref rc) = reasoning_content {
            if !rc.is_empty() {
                log::warn!(
                    "[OpenAI] content 为空，使用 reasoning_content fallback（{} 字符）",
                    rc.chars().count()
                );
                return rc.clone();
            }
        }
    }
    content.to_string()
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
            },
            Message {
                role: "user".to_string(),
                content: prompt,
                reasoning_content: None,
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
        let openai_resp: OpenAiResponse =
            tokio::task::spawn_blocking(move || serde_json::from_slice(&bytes))
                .await
                .map_err(|e| format!("deserialization task panicked: {}", e))?
                .map_err(|e| format!("OpenAI response parse error: {}", e))?;
        let content = openai_resp
            .choices
            .first()
            .map(|c| resolve_content(&c.message.content, &c.message.reasoning_content))
            .unwrap_or_default();

        let cost = self.calculate_cost(&openai_resp.model, openai_resp.usage.total_tokens);

        Ok(GenerateResponse {
            content,
            model: openai_resp.model,
            tokens_used: openai_resp.usage.total_tokens,
            cost,
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
                if line.is_empty() || !line.starts_with("data: ") {
                    continue;
                }
                let data = &line[6..];
                if data == "[DONE]" {
                    break;
                }
                match serde_json::from_str::<OpenAiStreamResponse>(data) {
                    Ok(parsed) => {
                        if let Some(choice) = parsed.choices.first() {
                            // v0.30.25: 优先转发 content，为空时 fallback reasoning_content
                            let text = choice
                                .delta
                                .content
                                .as_ref()
                                .filter(|c| !c.is_empty())
                                .or(choice.delta.reasoning_content.as_ref())
                                .filter(|c| !c.is_empty());
                            if let Some(content) = text {
                                if tx.send(Ok(content.clone())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("SSE parse error: {}", e).into())).await;
                        break;
                    }
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
    use super::{resolve_content, sanitize_top_p};

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

    // ===== v0.30.25: reasoning_content fallback 测试 =====

    #[test]
    fn resolve_content_uses_content_when_nonempty() {
        let rc = Some("思维链内容".to_string());
        assert_eq!(resolve_content("实际回答", &rc), "实际回答");
        assert_eq!(resolve_content("实际回答", &None), "实际回答");
    }

    #[test]
    fn resolve_content_falls_back_to_reasoning_when_content_empty() {
        let rc = Some("这是推理模型的实际内容".to_string());
        assert_eq!(resolve_content("", &rc), "这是推理模型的实际内容");
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
}
