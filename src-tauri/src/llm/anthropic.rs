use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{GenerateRequest, GenerateResponse, LlmAdapter};

#[derive(Clone)]
pub struct AnthropicAdapter {
    client: Client,
    api_key: String,
    model: String,
    api_base: String,
    default_max_tokens: i32,
    default_temperature: f32,
    #[allow(dead_code)]
    timeout_seconds: u64,
    #[allow(dead_code)]
    connect_timeout_seconds: u64,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: i32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    model: String,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: i32,
    output_tokens: i32,
}

impl AnthropicAdapter {
    pub fn new(
        api_key: String,
        model: String,
        api_base: Option<String>,
        max_tokens: i32,
        temperature: f32,
        timeout_seconds: u64,
        connect_timeout_seconds: u64,
    ) -> Self {
        let timeout = if timeout_seconds > 0 {
            Duration::from_secs(timeout_seconds)
        } else {
            Duration::from_secs(300)
        };
        let connect_timeout = if connect_timeout_seconds > 0 {
            Duration::from_secs(connect_timeout_seconds)
        } else {
            Duration::from_secs(10)
        };
        let client = Client::builder()
            .timeout(timeout)
            .connect_timeout(connect_timeout)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            api_key,
            model,
            api_base: api_base.unwrap_or_else(|| "https://api.anthropic.com/v1".to_string()),
            default_max_tokens: max_tokens,
            default_temperature: temperature,
            timeout_seconds,
            connect_timeout_seconds,
        }
    }

    fn calculate_cost(&self, input_tokens: i32, output_tokens: i32) -> f64 {
        let rate = match self.model.as_str() {
            "claude-3-opus-20240229" => 0.015,
            "claude-3-sonnet-20240229" => 0.003,
            "claude-3-haiku-20240307" => 0.00025,
            _ => 0.003,
        };
        ((input_tokens + output_tokens) as f64 / 1000.0) * rate
    }
}

#[async_trait::async_trait]
impl LlmAdapter for AnthropicAdapter {
    async fn generate(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, Box<dyn std::error::Error>> {
        let anthropic_req = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: request.max_tokens.unwrap_or(self.default_max_tokens),
            temperature: request.temperature.unwrap_or(self.default_temperature),
            top_p: request.top_p,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: request.prompt,
            }],
            system: Some("You are a professional creative writing assistant.".to_string()),
            stream: false,
        };

        let response = self
            .client
            .post(format!("{}/messages", self.api_base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&anthropic_req)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Anthropic API error: {}", error_text).into());
        }

        // 将同步 JSON 反序列化隔离到 blocking 线程池，避免大响应阻塞 async runtime。
        let bytes = response.bytes().await?;
        let anthropic_resp: AnthropicResponse =
            tokio::task::spawn_blocking(move || serde_json::from_slice(&bytes))
                .await
                .map_err(|e| format!("deserialization task panicked: {}", e))?
                .map_err(|e| format!("Anthropic response parse error: {}", e))?;
        let content = anthropic_resp
            .content
            .into_iter()
            .filter(|c| c.content_type == "text")
            .map(|c| c.text)
            .collect::<Vec<String>>()
            .join("");

        let total_tokens = anthropic_resp.usage.input_tokens + anthropic_resp.usage.output_tokens;
        let cost = self.calculate_cost(
            anthropic_resp.usage.input_tokens,
            anthropic_resp.usage.output_tokens,
        );

        Ok(GenerateResponse {
            content,
            model: anthropic_resp.model,
            tokens_used: total_tokens,
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
        let anthropic_req = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: request.max_tokens.unwrap_or(self.default_max_tokens),
            temperature: request.temperature.unwrap_or(self.default_temperature),
            top_p: request.top_p,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: request.prompt,
            }],
            system: Some("You are a professional creative writing assistant.".to_string()),
            stream: true,
        };

        let response = self
            .client
            .post(format!("{}/messages", self.api_base))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .header("Accept", "text/event-stream")
            .json(&anthropic_req)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Anthropic API error: {}", error_text).into());
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
                if line.is_empty() {
                    continue;
                }
                if line.starts_with("event: ") {
                    continue;
                }
                if !line.starts_with("data: ") {
                    continue;
                }
                let data = &line[6..];
                if data == "[DONE]" {
                    break;
                }
                match serde_json::from_str::<serde_json::Value>(data) {
                    Ok(json) => {
                        if let Some(text) = json
                            .get("delta")
                            .and_then(|d| d.get("text"))
                            .and_then(|t| t.as_str())
                        {
                            if tx.send(Ok(text.to_string())).await.is_err() {
                                break;
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
