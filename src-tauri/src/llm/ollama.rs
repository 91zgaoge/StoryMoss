use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{GenerateRequest, GenerateResponse, LlmAdapter};

pub struct OllamaAdapter {
    client: Client,
    model: String,
    api_base: String,
    default_max_tokens: i32,
    default_temperature: f32,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: i32,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    model: String,
    response: String,
    done: bool,
    #[serde(default)]
    eval_count: i32,
}

impl OllamaAdapter {
    pub fn new(
        _api_key: String,
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
            model,
            api_base: api_base.unwrap_or_else(|| "http://localhost:11434".to_string()),
            default_max_tokens: max_tokens,
            default_temperature: temperature,
        }
    }
}

#[async_trait::async_trait]
impl LlmAdapter for OllamaAdapter {
    async fn generate(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, Box<dyn std::error::Error>> {
        let ollama_req = OllamaRequest {
            model: self.model.clone(),
            prompt: request.prompt,
            stream: false,
            options: Some(OllamaOptions {
                temperature: request.temperature.unwrap_or(self.default_temperature),
                num_predict: request.max_tokens.unwrap_or(self.default_max_tokens),
            }),
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.api_base))
            .header("Content-Type", "application/json")
            .json(&ollama_req)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Ollama API error: {}", error_text).into());
        }

        let ollama_resp: OllamaResponse = response.json().await?;

        Ok(GenerateResponse {
            content: ollama_resp.response,
            model: ollama_resp.model,
            tokens_used: ollama_resp.eval_count.max(0),
            cost: 0.0,
        })
    }

    async fn generate_stream(
        &self,
        request: GenerateRequest,
    ) -> Result<
        tokio::sync::mpsc::Receiver<Result<String, Box<dyn std::error::Error + Send + Sync>>>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let ollama_req = OllamaRequest {
            model: self.model.clone(),
            prompt: request.prompt,
            stream: true,
            options: Some(OllamaOptions {
                temperature: request.temperature.unwrap_or(self.default_temperature),
                num_predict: request.max_tokens.unwrap_or(self.default_max_tokens),
            }),
        };

        let response = self
            .client
            .post(format!("{}/api/generate", self.api_base))
            .header("Content-Type", "application/json")
            .json(&ollama_req)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Ollama API error: {}", error_text).into());
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
                match serde_json::from_str::<OllamaResponse>(&line) {
                    Ok(parsed) => {
                        if !parsed.response.is_empty() {
                            if tx.send(Ok(parsed.response)).await.is_err() {
                                break;
                            }
                        }
                        if parsed.done {
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(format!("NDJSON parse error: {}", e).into()))
                            .await;
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
