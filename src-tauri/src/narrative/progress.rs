//! 统一进度事件系统
//!
//! 替代 novel-bootstrap-progress 和 book-analysis-progress，
//! 所有长流程（创世、拆书、分析）使用同一套进度事件。

use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// 流水线类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineType {
    Genesis,  // 正向/创世
    Analysis, // 逆向/分析
    Audit,    // 审计/检查
    Export,   // 导出
    Import,   // 导入
}

/// 步骤状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Running,   // 执行中
    Completed, // 已完成
    Failed,    // 失败
    Skipped,   // 已跳过
    Cancelled, // 已取消
}

/// 统一的流水线进度事件
///
/// 所有长流程（Bootstrap、拆书、分析）都发射此事件到前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineProgressEvent {
    pub pipeline_id: String,
    pub pipeline_type: PipelineType,
    pub step_name: String,
    pub step_number: usize,
    pub total_steps: usize,
    pub status: StepStatus,
    pub message: String,
    pub progress_percent: i32,
    pub elapsed_seconds: u64,
    pub metadata: Option<serde_json::Value>,
}

/// LLM 子步骤进度 — 用于报告单个LLM调用的进度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSubProgress {
    pub pipeline_id: String,
    pub step_name: String,
    pub action: String, // "正在连接模型" | "已发送请求" | "AI正在思考"
    pub elapsed_seconds: u64,
    pub model: String,
}

/// 流水线完成事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineCompleteEvent {
    pub pipeline_id: String,
    pub pipeline_type: PipelineType,
    pub success: bool,
    pub total_elapsed_seconds: u64,
    pub elements_created: ElementsCount,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ElementsCount {
    pub characters: usize,
    pub scenes: usize,
    pub foreshadowings: usize,
    pub world_rules: usize,
    pub plot_points: usize,
}

/// 进度发射器 — 封装事件发射逻辑
pub struct ProgressEmitter<R: tauri::Runtime> {
    app_handle: tauri::AppHandle<R>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_type_serialization() {
        let ty = PipelineType::Genesis;
        let json = serde_json::to_string(&ty).unwrap();
        assert_eq!(json, "\"genesis\"");

        let ty = PipelineType::Analysis;
        let json = serde_json::to_string(&ty).unwrap();
        assert_eq!(json, "\"analysis\"");
    }

    #[test]
    fn test_step_status_serialization() {
        assert_eq!(
            serde_json::to_string(&StepStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&StepStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&StepStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&StepStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }

    #[test]
    fn test_pipeline_progress_event_serialization() {
        let event = PipelineProgressEvent {
            pipeline_id: "pipe_001".to_string(),
            pipeline_type: PipelineType::Genesis,
            step_name: "世界观生成".to_string(),
            step_number: 2,
            total_steps: 7,
            status: StepStatus::Running,
            message: "正在生成世界观...".to_string(),
            progress_percent: 28,
            elapsed_seconds: 15,
            metadata: Some(serde_json::json!({"model": "gpt-4"})),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PipelineProgressEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.pipeline_id, "pipe_001");
        assert_eq!(deserialized.step_name, "世界观生成");
        assert_eq!(deserialized.progress_percent, 28);
        assert!(deserialized.metadata.is_some());
    }

    #[test]
    fn test_elements_count_default() {
        let count = ElementsCount::default();
        assert_eq!(count.characters, 0);
        assert_eq!(count.scenes, 0);
        assert_eq!(count.foreshadowings, 0);
        assert_eq!(count.world_rules, 0);
        assert_eq!(count.plot_points, 0);
    }

    #[test]
    fn test_pipeline_complete_event_serialization() {
        let event = PipelineCompleteEvent {
            pipeline_id: "pipe_001".to_string(),
            pipeline_type: PipelineType::Genesis,
            success: true,
            total_elapsed_seconds: 120,
            elements_created: ElementsCount {
                characters: 5,
                scenes: 10,
                foreshadowings: 3,
                world_rules: 4,
                plot_points: 8,
            },
            error_message: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: PipelineCompleteEvent = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.elements_created.characters, 5);
        assert_eq!(deserialized.total_elapsed_seconds, 120);
    }
}

impl<R: tauri::Runtime> ProgressEmitter<R> {
    pub fn new(app_handle: tauri::AppHandle<R>) -> Self {
        Self { app_handle }
    }

    pub fn emit_progress(&self, event: PipelineProgressEvent) {
        let _ = self.app_handle.emit("pipeline-progress", event);
    }

    pub fn emit_complete(&self, event: PipelineCompleteEvent) {
        let _ = self.app_handle.emit("pipeline-complete", event);
    }

    pub fn emit_llm_sub_progress(&self, event: LlmSubProgress) {
        let _ = self.app_handle.emit("llm-sub-progress", event);
    }
}
