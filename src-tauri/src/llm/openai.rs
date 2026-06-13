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
    #[allow(dead_code)]
    timeout_seconds: u64,
    #[allow(dead_code)]
    connect_timeout_seconds: u64,
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
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
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
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamChoice {
    delta: OpenAiDelta,
}

#[derive(Debug, Deserialize, Default)]
struct OpenAiDelta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiStreamResponse {
    choices: Vec<OpenAiStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    total_tokens: i32,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
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
            api_base: api_base.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            default_max_tokens: max_tokens,
            default_temperature: temperature,
            timeout_seconds,
            connect_timeout_seconds,
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

    fn build_messages(&self, prompt: String) -> Vec<Message> {
        vec![
            Message {
                role: "system".to_string(),
                content: "You are a professional creative writing assistant.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: prompt,
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
        let openai_req = OpenAiRequest {
            model: self.model.clone(),
            messages: self.build_messages(request.prompt),
            max_tokens: request.max_tokens.unwrap_or(self.default_max_tokens),
            temperature: request.temperature.unwrap_or(self.default_temperature),
            top_p: request.top_p,
            frequency_penalty: request.frequency_penalty,
            presence_penalty: request.presence_penalty,
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

        // 将同步 JSON 反序列化隔离到 blocking 线程池，避免大响应阻塞 async runtime。
        let bytes = response.bytes().await?;
        let openai_resp: OpenAiResponse =
            tokio::task::spawn_blocking(move || serde_json::from_slice(&bytes))
                .await
                .map_err(|e| format!("deserialization task panicked: {}", e))?
                .map_err(|e| format!("OpenAI response parse error: {}", e))?;
        let content = openai_resp
            .choices
            .first()
            .map(|c| c.message.content.clone())
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
            messages: self.build_messages(request.prompt),
            max_tokens: request.max_tokens.unwrap_or(self.default_max_tokens),
            temperature: request.temperature.unwrap_or(self.default_temperature),
            top_p: request.top_p,
            frequency_penalty: request.frequency_penalty,
            presence_penalty: request.presence_penalty,
            stream: true,
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
                            if let Some(content) = &choice.delta.content {
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
