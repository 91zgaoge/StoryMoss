//! 诊断数据存储
//!
//! 用于给前端超时/失败诊断提供“最后发给 LLM 的提示词全文”。
//! 通过 Tauri State 注入，避免使用全局 static。

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::creative_engine::context_prioritizer::ContextHealthMetrics;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LastLlmPrompt {
    pub request_id: String,
    pub context_label: String,
    pub model_id: String,
    pub model_name: String,
    pub provider: String,
    pub prompt: String,
    /// 提示词字符数
    pub prompt_chars: usize,
    /// 提示词 token 估算（仅参考）
    pub prompt_tokens: usize,
    pub updated_at: String,
}

#[derive(Debug, Default)]
pub struct DiagnosticStore {
    last_llm_prompt: Mutex<Option<LastLlmPrompt>>,
    context_health: Mutex<Option<ContextHealthMetrics>>,
    review_notes: Mutex<Option<serde_json::Value>>,
}

impl DiagnosticStore {
    pub fn new() -> Self {
        Self {
            last_llm_prompt: Mutex::new(None),
            context_health: Mutex::new(None),
            review_notes: Mutex::new(None),
        }
    }

    pub fn set_last_llm_prompt(&self, info: LastLlmPrompt) {
        if let Ok(mut guard) = self.last_llm_prompt.lock() {
            *guard = Some(info);
        }
    }

    pub fn get_last_llm_prompt(&self) -> Option<LastLlmPrompt> {
        self.last_llm_prompt.lock().ok().and_then(|g| g.clone())
    }

    pub fn clear(&self) {
        if let Ok(mut guard) = self.last_llm_prompt.lock() {
            *guard = None;
        }
    }

    /// v0.25.0: 记录最近一次 Writer 系统提示词的上下文健康度指标。
    pub fn set_context_health(&self, metrics: ContextHealthMetrics) {
        if let Ok(mut guard) = self.context_health.lock() {
            *guard = Some(metrics);
        }
    }

    /// v0.25.0: 获取最近一次 Writer 系统提示词的上下文健康度指标。
    pub fn get_context_health(&self) -> Option<ContextHealthMetrics> {
        self.context_health.lock().ok().and_then(|g| g.clone())
    }

    /// v0.26.0: 记录最近一次子代理审查笔记。
    pub fn set_review_notes(&self, notes: serde_json::Value) {
        if let Ok(mut guard) = self.review_notes.lock() {
            *guard = Some(notes);
        }
    }

    /// v0.26.0: 获取最近一次子代理审查笔记。
    pub fn get_review_notes(&self) -> Option<serde_json::Value> {
        self.review_notes.lock().ok().and_then(|g| g.clone())
    }
}
