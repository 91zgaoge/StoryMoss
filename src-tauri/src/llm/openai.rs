use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{GenerateRequest, GenerateResponse, LlmAdapter};

pub struct OpenAiAdapter {
    client: Client,
    api_key: String,
    model: String,
    api_base: String,
    default_max_tokens: i32,
    default_temperature: f32,
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: i32,
    temperature: f32,
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
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            api_key,
            model,
            api_base: api_base.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            default_max_tokens: max_tokens,
            default_temperature: temperature,
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

        let openai_resp: OpenAiResponse = response.json().await?;
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
}
