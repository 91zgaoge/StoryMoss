//! 创作工作流引擎
//!
//! 实现从构思到成稿的完整自动化工作流。
//! 幕后编排，幕前只呈现最终结果。
//!
//! 创作阶段：
//! Conception → Outlining → SceneDesign → Writing → Review → Iteration →
//! Ingestion

pub mod engine;
pub mod quality;

#[allow(unused_imports)]
pub use engine::*;
#[allow(unused_imports)]
pub use quality::*;
use serde::{Deserialize, Serialize};

/// 创作模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationMode {
    /// AI 草稿 + 人修改
    AiDraftHumanEdit,
    /// 人草稿 + AI 润色
    HumanDraftAiPolish,
    /// 纯 AI 创作（一键）
    AiOnly,
}

impl CreationMode {
    pub fn name(&self) -> &'static str {
        match self {
            CreationMode::AiDraftHumanEdit => "AI草稿+人修改",
            CreationMode::HumanDraftAiPolish => "人草稿+AI润色",
            CreationMode::AiOnly => "一键创作",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            CreationMode::AiDraftHumanEdit => "AI生成初稿，用户在幕前修改完善",
            CreationMode::HumanDraftAiPolish => "用户先写草稿，AI润色提升",
            CreationMode::AiOnly => "输入一句话，自动完成全流程创作",
        }
    }
}

/// 工作流执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecutionResult {
    pub success: bool,
    pub current_phase: String,
    pub completed_phases: Vec<String>,
    pub output: Option<String>,
    pub quality_report: Option<quality::QualityReport>,
    pub error: Option<String>,
}

/// 工作流进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowProgressEvent {
    pub workflow_id: String,
    pub phase: String,
    pub stage: WorkflowStage,
    pub message: String,
    pub progress: f32, // 0.0 - 1.0
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStage {
    Started,
    InProgress,
    AgentExecuting,
    WaitingForUser,
    Completed,
    Failed,
}
