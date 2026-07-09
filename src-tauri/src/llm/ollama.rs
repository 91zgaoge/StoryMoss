use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{GenerateRequest, GenerateResponse, LlmAdapter};

#[derive(Clone)]
pub struct OllamaAdapter {
    client: Client,
    model: String,
    api_base: String,
    default_max_tokens: i32,
    default_temperature: f32,
    generation_timeout: std::time::Duration,
    connect_timeout: std::time::Duration,
    first_chunk_timeout: std::time::Duration,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    /// Ollama JSON mode: https://github.com/ollama/ollama/blob/main/docs/api.md#generate-request-with-format
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
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
        let client = Client::builder()
            .connect_timeout(connect_timeout)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            model,
            api_base: api_base.unwrap_or_else(|| "http://localhost:11434".to_string()),
            default_max_tokens: max_tokens,
            default_temperature: temperature,
            generation_timeout,
            connect_timeout,
            first_chunk_timeout,
        }
    }
}

#[async_trait::async_trait]
impl LlmAdapter for OllamaAdapter {
    async fn generate(
        &self,
        request: GenerateRequest,
    ) -> Result<GenerateResponse, Box<dyn std::error::Error>> {
        use super::adapter::{read_body_with_generation_timeout_ex, send_with_connection_timeout};

        // v0.23.65: Ollama /api/generate 端点无 system 字段，把非空
        // system_prompt 前置拼入 prompt，使 writer_system 写作准则对
        // Ollama 模型也生效（与 OpenAI/Anthropic 适配器语义对齐）。
        let prompt = match request
            .system_prompt
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            Some(sys) => format!("{}\n\n{}", sys, request.prompt),
            None => request.prompt,
        };

        let ollama_req = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            options: Some(OllamaOptions {
                temperature: request.temperature.unwrap_or(self.default_temperature),
                num_predict: request.max_tokens.unwrap_or(self.default_max_tokens),
                top_p: request.top_p,
            }),
            format: request
                .response_format
                .as_ref()
                .map(|f| f.ollama_value().to_string()),
        };

        let response = send_with_connection_timeout(
            self.client
                .post(format!("{}/api/generate", self.api_base))
                .header("Content-Type", "application/json")
                .json(&ollama_req),
            self.connect_timeout,
        )
        .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Ollama API error: {}", error_text).into());
        }

        // v0.11.8: 流式读取响应体，每收到 chunk 刷新一次生成超时计时器。
        let bytes = read_body_with_generation_timeout_ex(
            response,
            self.generation_timeout,
            self.first_chunk_timeout,
        )
        .await?;

        // 将同步 JSON 反序列化隔离到 blocking 线程池，避免大响应阻塞 async runtime。
        let ollama_resp: OllamaResponse =
            tokio::task::spawn_blocking(move || serde_json::from_slice(&bytes))
                .await
                .map_err(|e| format!("deserialization task panicked: {}", e))?
                .map_err(|e| format!("Ollama response parse error: {}", e))?;

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
        // v0.23.65: 与 generate 同理，system_prompt 前置拼入 prompt。
        let prompt = match request
            .system_prompt
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            Some(sys) => format!("{}\n\n{}", sys, request.prompt),
            None => request.prompt,
        };

        let ollama_req = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: true,
            options: Some(OllamaOptions {
                temperature: request.temperature.unwrap_or(self.default_temperature),
                num_predict: request.max_tokens.unwrap_or(self.default_max_tokens),
                top_p: request.top_p,
            }),
            format: request
                .response_format
                .as_ref()
                .map(|f| f.ollama_value().to_string()),
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

    fn box_clone(&self) -> Box<dyn super::LlmAdapter> {
        Box::new(self.clone())
    }
}
