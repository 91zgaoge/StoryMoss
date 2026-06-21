//! Agent domain types.
//!
//! Pure data definitions shared across the agent system and creative engine.
//! Agent execution behavior remains in `crate::agents`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::domain::{agent_context::AgentContext, subscription::SubscriptionTier};

/// Agent类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Writer,             // 写作助手
    Inspector,          // 质检员
    OutlinePlanner,     // 大纲规划师
    StyleMimic,         // 风格模仿师
    PlotAnalyzer,       // 情节分析师
    MemoryCompressor,   // 记忆压缩师
    Commentator,        // 古典评点家
    KnowledgeDistiller, // 知识蒸馏师
}

impl AgentType {
    pub fn name(&self) -> &'static str {
        match self {
            AgentType::Writer => "写作助手",
            AgentType::Inspector => "质检员",
            AgentType::OutlinePlanner => "大纲规划师",
            AgentType::StyleMimic => "风格模仿师",
            AgentType::PlotAnalyzer => "情节分析师",
            AgentType::MemoryCompressor => "记忆压缩师",
            AgentType::Commentator => "古典评点家",
            AgentType::KnowledgeDistiller => "知识蒸馏师",
        }
    }
}

/// Agent执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub content: String,
    pub score: Option<f32>, // 0.0 - 1.0
    pub suggestions: Vec<String>,
    /// 关联的 LLM request_id，供上层取消使用
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Agent任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: String,
    pub agent_type: AgentType,
    pub context: AgentContext,
    pub input: String,
    pub parameters: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<SubscriptionTier>,
}
