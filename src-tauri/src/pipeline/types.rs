use serde::{Deserialize, Serialize};

/// 管线配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub enable_refine: bool,
    pub enable_review: bool,
    pub enable_finalize_post_process: bool,
    pub refine_prompt_template: Option<String>,
    pub review_focus: Option<String>,
    pub review_dimensions: Vec<String>,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            enable_refine: true,
            enable_review: true,
            enable_finalize_post_process: true,
            refine_prompt_template: None,
            review_focus: None,
            review_dimensions: vec![
                "continuity".to_string(),
                "logic".to_string(),
                "character".to_string(),
                "foreshadow".to_string(),
                "pacing".to_string(),
                "style".to_string(),
            ],
        }
    }
}

/// 管线执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineResult {
    pub draft_id: String,
    pub chapter_number: i32,
    pub refined_draft_id: Option<String>,
    pub review_id: Option<String>,
    pub finalized_draft_id: Option<String>,
    pub post_process_run_id: Option<String>,
    pub success: bool,
    pub message: String,
}

/// 管线错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineError {
    pub phase: String,
    pub message: String,
    pub recoverable: bool,
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} (recoverable: {})",
            self.phase, self.message, self.recoverable
        )
    }
}

impl std::error::Error for PipelineError {}

/// 修稿结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineResult {
    pub revision_id: String,
    pub original_content: String,
    pub refined_content: String,
    pub word_count: i32,
    pub change_summary: Option<String>,
}

/// 审稿结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub review_id: String,
    pub overall_score: f32,
    pub dimensions: Vec<ReviewDimensionResult>,
    pub issues: Vec<ReviewIssueResult>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDimensionResult {
    pub name: String,
    pub score: f32,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssueResult {
    pub severity: String,
    pub dimension: String,
    pub description: String,
    pub suggestion: String,
}

/// 后处理步骤定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessStepDef {
    pub key: String,
    pub label: String,
    pub critical: bool,
}

/// 定稿后处理进度回调（用于流式反馈到前端）
pub trait PipelineCallbacks: Send + Sync {
    fn log(&self, message: &str);
    fn progress(&self, phase: &str, percent: f32);
    fn on_chunk(&self, chunk: &str);
}

/// 静默回调（无 UI 时使用）
pub struct SilentCallbacks;

impl PipelineCallbacks for SilentCallbacks {
    fn log(&self, _message: &str) {}
    fn progress(&self, _phase: &str, _percent: f32) {}
    fn on_chunk(&self, _chunk: &str) {}
}

/// 后处理选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessOptions {
    pub only_failed: bool,
}

/// 章节信息（定稿时用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterInfo {
    pub chapter_number: i32,
    pub title: Option<String>,
}
