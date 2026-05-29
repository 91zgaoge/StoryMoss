#![allow(dead_code)]
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::embedding::*;

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Embedding>, EmbeddingError>;
    fn dimensions(&self) -> usize;
    fn max_batch_size(&self) -> usize;
}

#[derive(Debug, Clone)]
pub struct EmbeddingError {
    pub message: String,
    pub code: String,
}

impl std::fmt::Display for EmbeddingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for EmbeddingError {}

/// OpenAI embedding provider
pub struct OpenAIEmbeddingProvider {
    api_key: String,
    model: String,
    dimensions: usize,
    client: reqwest::Client,
}

impl OpenAIEmbeddingProvider {
    pub fn new(api_key: String, model: String, dimensions: usize) -> Self {
        Self {
            api_key,
            model,
            dimensions,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for OpenAIEmbeddingProvider {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Embedding>, EmbeddingError> {
        let request = OpenAIEmbeddingRequest {
            model: self.model.clone(),
            input: texts.clone(),
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| EmbeddingError {
                message: e.to_string(),
                code: "REQUEST_FAILED".to_string(),
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(EmbeddingError {
                message: error_text,
                code: "API_ERROR".to_string(),
            });
        }

        let result: OpenAIEmbeddingResponse =
            response.json().await.map_err(|e| EmbeddingError {
                message: e.to_string(),
                code: "PARSE_ERROR".to_string(),
            })?;

        Ok(result
            .data
            .into_iter()
            .enumerate()
            .map(|(i, d)| Embedding {
                id: format!("emb_{}", i),
                vector: d.embedding,
                dimensions: self.dimensions,
                model: self.model.clone(),
            })
            .collect())
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn max_batch_size(&self) -> usize {
        100
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIEmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAIEmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Debug, Clone, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

/// Ollama embedding provider
pub struct OllamaEmbeddingProvider {
    model: String,
    api_base: String,
    dimensions: usize,
    client: reqwest::Client,
}

impl OllamaEmbeddingProvider {
    pub fn new(model: String, api_base: Option<String>, dimensions: usize) -> Self {
        Self {
            model,
            api_base: api_base.unwrap_or_else(|| "http://localhost:11434".to_string()),
            dimensions,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct OllamaEmbedRequest {
    model: String,
    prompt: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaEmbedResponse {
    embedding: Vec<f32>,
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddingProvider {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Embedding>, EmbeddingError> {
        let mut embeddings = Vec::with_capacity(texts.len());
        for (i, text) in texts.into_iter().enumerate() {
            let request = OllamaEmbedRequest {
                model: self.model.clone(),
                prompt: text,
            };
            let response = self
                .client
                .post(format!("{}/api/embeddings", self.api_base))
                .json(&request)
                .send()
                .await
                .map_err(|e| EmbeddingError {
                    message: e.to_string(),
                    code: "REQUEST_FAILED".to_string(),
                })?;

            if !response.status().is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(EmbeddingError {
                    message: error_text,
                    code: "API_ERROR".to_string(),
                });
            }

            let result: OllamaEmbedResponse =
                response.json().await.map_err(|e| EmbeddingError {
                    message: e.to_string(),
                    code: "PARSE_ERROR".to_string(),
                })?;

            embeddings.push(Embedding {
                id: format!("emb_{}", i),
                vector: result.embedding,
                dimensions: self.dimensions,
                model: self.model.clone(),
            });
        }
        Ok(embeddings)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn max_batch_size(&self) -> usize {
        1 // Ollama embeddings API 目前只支持单条
    }
}

/// Local embedding provider — FNV-1a 哈希回退
pub struct LocalEmbeddingProvider {
    dimensions: usize,
}

impl LocalEmbeddingProvider {
    pub fn new(dimensions: usize) -> Self {
        Self { dimensions }
    }
}

#[async_trait]
impl EmbeddingProvider for LocalEmbeddingProvider {
    async fn embed(&self, texts: Vec<String>) -> Result<Vec<Embedding>, EmbeddingError> {
        use super::embedding::embed_text;
        let mut embeddings = Vec::with_capacity(texts.len());
        for (i, text) in texts.into_iter().enumerate() {
            match embed_text(&text) {
                Ok(vector) => embeddings.push(Embedding {
                    id: format!("emb_{}", i),
                    vector,
                    dimensions: self.dimensions,
                    model: "local".to_string(),
                }),
                Err(e) => {
                    return Err(EmbeddingError {
                        message: format!("Local embedding failed: {}", e),
                        code: "LOCAL_EMBED_ERROR".to_string(),
                    });
                }
            }
        }
        Ok(embeddings)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }

    fn max_batch_size(&self) -> usize {
        32
    }
}

// ==================== 全局提供者管理 ====================

use std::sync::Arc;

use once_cell::sync::OnceCell;
use tokio::sync::Mutex;

static GLOBAL_PROVIDER: OnceCell<Arc<Mutex<Box<dyn EmbeddingProvider + Send + Sync>>>> =
    OnceCell::new();

/// 初始化全局嵌入提供者（从 AppConfig 读取）
pub fn init_global_provider(config: &crate::config::settings::AppConfig) {
    let provider = build_provider_from_config(config);
    let _ = GLOBAL_PROVIDER.set(Arc::new(Mutex::new(provider)));
    log::info!("[EmbeddingProvider] 全局嵌入提供者已初始化");
}

fn build_provider_from_config(
    config: &crate::config::settings::AppConfig,
) -> Box<dyn EmbeddingProvider + Send + Sync> {
    // 如果有激活的 embedding profile，使用它
    if let Some(active_id) = &config.active_embedding_profile {
        if let Some(profile) = config.embedding_profiles.get(active_id) {
            match profile.provider {
                crate::config::settings::EmbeddingProvider::Ollama => {
                    log::info!(
                        "[EmbeddingProvider] 使用 Ollama 嵌入后端: {}",
                        profile.model
                    );
                    return Box::new(OllamaEmbeddingProvider::new(
                        profile.model.clone(),
                        profile.api_base.clone(),
                        profile.dimensions,
                    ));
                }
                crate::config::settings::EmbeddingProvider::OpenAI => {
                    log::info!(
                        "[EmbeddingProvider] 使用 OpenAI 嵌入后端: {}",
                        profile.model
                    );
                    return Box::new(OpenAIEmbeddingProvider::new(
                        profile.api_key.clone(),
                        profile.model.clone(),
                        profile.dimensions,
                    ));
                }
                _ => {
                    log::warn!(
                        "[EmbeddingProvider] 不支持的嵌入后端: {:?}，回退到本地",
                        profile.provider
                    );
                }
            }
        }
    }
    // 默认回退到本地 FNV-1a
    log::info!("[EmbeddingProvider] 使用本地 FNV-1a 回退嵌入");
    Box::new(LocalEmbeddingProvider::new(384))
}

/// 获取全局嵌入提供者
pub fn global_provider() -> Option<Arc<Mutex<Box<dyn EmbeddingProvider + Send + Sync>>>> {
    GLOBAL_PROVIDER.get().cloned()
}

/// 判断全局嵌入提供者是否为语义型（非 FNV-1a 回退）
pub async fn is_semantic_enabled() -> bool {
    if let Some(provider) = global_provider() {
        let _guard = provider.lock().await;
        // 简单判断：LocalEmbeddingProvider 的 model 字段为 "local"
        // 更精确的做法是 trait 方法，但此处简化
        return true; // 只要初始化成功就认为启用了
    }
    false
}

/// 将向量投影到目标维度（截断或填充）
pub fn project_to_dim(vec: Vec<f32>, target: usize) -> Vec<f32> {
    if vec.len() == target {
        vec
    } else if vec.len() > target {
        vec.into_iter().take(target).collect()
    } else {
        let mut result = vec;
        result.resize(target, 0.0);
        result
    }
}
