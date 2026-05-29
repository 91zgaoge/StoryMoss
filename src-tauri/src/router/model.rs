#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub provider: ModelProvider,
    pub model_id: String,
    pub api_base: Option<String>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub timeout_seconds: u64,
    pub retry_attempts: u32,
    pub cost_per_1k_input: f64,
    pub cost_per_1k_output: f64,
    pub capabilities: ModelCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelProvider {
    OpenAI,
    Anthropic,
    Azure,
    Ollama,
    DeepSeek,
    Qwen,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub max_context_length: u32,
    pub supports_streaming: bool,
    pub supports_functions: bool,
    pub supports_vision: bool,
    pub quality_tier: QualityTier,
    pub speed_tier: SpeedTier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum QualityTier {
    Low,
    Medium,
    High,
    Ultra,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpeedTier {
    Fast,
    Normal,
    Slow,
    VerySlow,
}

impl ModelConfig {
    pub fn gpt4() -> Self {
        Self {
            id: "gpt-4".to_string(),
            name: "GPT-4".to_string(),
            provider: ModelProvider::OpenAI,
            model_id: "gpt-4".to_string(),
            api_base: None,
            max_tokens: 8192,
            temperature: 0.7,
            timeout_seconds: 60,
            retry_attempts: 3,
            cost_per_1k_input: 0.03,
            cost_per_1k_output: 0.06,
            capabilities: ModelCapabilities {
                max_context_length: 8192,
                supports_streaming: true,
                supports_functions: true,
                supports_vision: false,
                quality_tier: QualityTier::High,
                speed_tier: SpeedTier::Normal,
            },
        }
    }

    pub fn gpt4_turbo() -> Self {
        Self {
            id: "gpt-4-turbo".to_string(),
            name: "GPT-4 Turbo".to_string(),
            provider: ModelProvider::OpenAI,
            model_id: "gpt-4-turbo-preview".to_string(),
            api_base: None,
            max_tokens: 4096,
            temperature: 0.7,
            timeout_seconds: 60,
            retry_attempts: 3,
            cost_per_1k_input: 0.01,
            cost_per_1k_output: 0.03,
            capabilities: ModelCapabilities {
                max_context_length: 128000,
                supports_streaming: true,
                supports_functions: true,
                supports_vision: true,
                quality_tier: QualityTier::Ultra,
                speed_tier: SpeedTier::Normal,
            },
        }
    }
}
