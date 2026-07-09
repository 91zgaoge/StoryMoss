//! Pipeline Review Task Executor
//!
//! 将 Pipeline 修稿/审稿/定稿任务接入 Task System，支持后台执行和进度追踪。

use std::sync::Arc;

use tauri::AppHandle;

use super::types::*;
use crate::{
    db::DbPool,
    llm::LlmService,
    ports::VectorStore,
    task_system::{
        executor::{TaskExecutionContext, TaskExecutor},
        models::*,
    },
};

pub struct PipelineReviewExecutor {
    pool: DbPool,
    app_handle: AppHandle,
    vector_store: Arc<dyn VectorStore>,
}

impl PipelineReviewExecutor {
    pub fn new(pool: DbPool, app_handle: AppHandle, vector_store: Arc<dyn VectorStore>) -> Self {
        Self {
            pool,
            app_handle,
            vector_store,
        }
    }
}

struct TaskPipelineCallbacks {
    task_id: String,
    pool: DbPool,
    app_handle: AppHandle,
}

impl super::types::PipelineCallbacks for TaskPipelineCallbacks {
    fn log(&self, message: &str) {
        let ctx = TaskExecutionContext::new(
            self.task_id.clone(),
            self.pool.clone(),
            self.app_handle.clone(),
        );
        ctx.log("info", message);
    }

    fn progress(&self, phase: &str, percent: f32) {
        let ctx = TaskExecutionContext::new(
            self.task_id.clone(),
            self.pool.clone(),
            self.app_handle.clone(),
        );
        let progress = (percent * 100.0) as i32;
        let message = format!("Pipeline {} 进度 {}%", phase, progress);
        ctx.update_progress(phase, progress, &message);
    }

    fn on_chunk(&self, _chunk: &str) {}
}

fn task_ok(result_json: Option<String>) -> TaskResult {
    TaskResult {
        success: true,
        result_json,
        error_message: None,
    }
}

fn task_err(error_message: impl Into<String>) -> TaskResult {
    TaskResult {
        success: false,
        result_json: None,
        error_message: Some(error_message.into()),
    }
}

/// Parsed payload for pipeline operations.
#[derive(Debug)]
struct PipelinePayload {
    operation: String,
    story_id: String,
    draft_id: String,
    user_prompt: Option<String>,
    review_focus: Option<String>,
    chapter_number: i32,
    chapter_title: Option<String>,
    scene_id: Option<String>,
}

fn parse_pipeline_payload(task: &Task) -> Result<PipelinePayload, String> {
    let payload: serde_json::Value = match task.payload.as_deref() {
        Some(p) => serde_json::from_str(p).unwrap_or_else(|_| serde_json::json!({})),
        None => serde_json::json!({}),
    };

    let story_id = payload
        .get("story_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let draft_id = payload
        .get("draft_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    if story_id.is_empty() || draft_id.is_empty() {
        return Err("Missing story_id or draft_id in payload".to_string());
    }

    Ok(PipelinePayload {
        operation: payload
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("review")
            .to_string(),
        story_id,
        draft_id,
        user_prompt: payload
            .get("user_prompt")
            .and_then(|v| v.as_str())
            .map(String::from),
        review_focus: payload
            .get("review_focus")
            .and_then(|v| v.as_str())
            .map(String::from),
        chapter_number: payload
            .get("chapter_number")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32,
        chapter_title: payload
            .get("chapter_title")
            .and_then(|v| v.as_str())
            .map(String::from),
        scene_id: payload
            .get("scene_id")
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

#[async_trait::async_trait]
impl TaskExecutor for PipelineReviewExecutor {
    fn can_handle(&self, task_type: &TaskType) -> bool {
        *task_type == TaskType::PipelineReview
    }

    async fn execute(&self, task: &Task) -> Result<TaskResult, Box<dyn std::error::Error>> {
        let ctx =
            TaskExecutionContext::new(task.id.clone(), self.pool.clone(), self.app_handle.clone());

        let payload = match parse_pipeline_payload(task) {
            Ok(p) => p,
            Err(e) => return Ok(task_err(e)),
        };

        let config = PipelineConfig::default();
        let llm_service = LlmService::new(self.app_handle.clone());
        let callbacks = TaskPipelineCallbacks {
            task_id: task.id.clone(),
            pool: self.pool.clone(),
            app_handle: self.app_handle.clone(),
        };

        let result = match payload.operation.as_str() {
            "refine" => {
                ctx.update_progress("refine", 5, "开始修稿...");
                match super::refine_draft(
                    &payload.story_id,
                    &payload.draft_id,
                    payload.user_prompt.as_deref(),
                    &config,
                    &self.pool,
                    &llm_service,
                    &callbacks,
                )
                .await
                {
                    Ok(result) => {
                        let json = serde_json::to_string(&result)?;
                        Ok(task_ok(Some(json)))
                    }
                    Err(e) => Ok(task_err(e.to_string())),
                }
            }
            "review" => {
                ctx.update_progress("review", 5, "开始审稿...");
                match super::review_draft(
                    &payload.story_id,
                    &payload.draft_id,
                    payload.review_focus.as_deref(),
                    &config,
                    &self.pool,
                    &llm_service,
                    &callbacks,
                )
                .await
                {
                    Ok(result) => {
                        let json = serde_json::to_string(&result)?;
                        Ok(task_ok(Some(json)))
                    }
                    Err(e) => Ok(task_err(e.to_string())),
                }
            }
            "finalize" => {
                let chapter_info = ChapterInfo {
                    chapter_number: payload.chapter_number,
                    title: payload.chapter_title.clone(),
                };
                ctx.update_progress("finalize", 5, "开始定稿...");
                match super::finalize_draft(
                    &payload.story_id,
                    &payload.draft_id,
                    &chapter_info,
                    &config,
                    &self.pool,
                    &self.app_handle,
                    &callbacks,
                    self.vector_store.as_ref(),
                    payload.scene_id.as_deref(),
                )
                .await
                {
                    Ok(post_process_run_id) => {
                        let json = serde_json::to_string(&serde_json::json!({
                            "post_process_run_id": post_process_run_id,
                        }))?;
                        Ok(task_ok(Some(json)))
                    }
                    Err(e) => Ok(task_err(e.to_string())),
                }
            }
            _ => Ok(task_err(format!(
                "Unknown pipeline operation: {}",
                payload.operation
            ))),
        };

        ctx.heartbeat();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_system::models::TaskType;

    #[test]
    fn test_parse_pipeline_payload_basic() {
        let task = Task {
            id: "t1".to_string(),
            name: "test".to_string(),
            description: None,
            task_type: TaskType::PipelineReview,
            schedule_type: crate::task_system::models::ScheduleType::Once,
            cron_pattern: None,
            payload: Some(
                r#"{"story_id": "s1", "draft_id": "d1", "operation": "review"}"#.to_string(),
            ),
            status: crate::task_system::models::TaskStatus::Pending,
            progress: 0,
            result: None,
            error_message: None,
            max_retries: 3,
            retry_count: 0,
            enabled: true,
            last_run_at: None,
            next_run_at: None,
            last_heartbeat_at: None,
            heartbeat_timeout_seconds: 300,
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };

        let payload = parse_pipeline_payload(&task).unwrap();
        assert_eq!(payload.story_id, "s1");
        assert_eq!(payload.draft_id, "d1");
        assert_eq!(payload.operation, "review");
        assert_eq!(payload.review_focus, None);
        assert_eq!(payload.chapter_number, 1);
        assert_eq!(payload.chapter_title, None);
        assert_eq!(payload.scene_id, None);
    }

    #[test]
    fn test_parse_pipeline_payload_refine() {
        let task = Task {
            id: "t2".to_string(),
            name: "test".to_string(),
            description: None,
            task_type: TaskType::PipelineReview,
            schedule_type: crate::task_system::models::ScheduleType::Once,
            cron_pattern: None,
            payload: Some(r#"{"story_id": "s2", "draft_id": "d2", "operation": "refine", "user_prompt": "请缩短"}"#.to_string()),
            status: crate::task_system::models::TaskStatus::Pending,
            progress: 0,
            result: None,
            error_message: None,
            max_retries: 3,
            retry_count: 0,
            enabled: true,
            last_run_at: None,
            next_run_at: None,
            last_heartbeat_at: None,
            heartbeat_timeout_seconds: 300,
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };

        let payload = parse_pipeline_payload(&task).unwrap();
        assert_eq!(payload.operation, "refine");
        assert_eq!(payload.user_prompt, Some("请缩短".to_string()));
    }

    #[test]
    fn test_parse_pipeline_payload_finalize() {
        let task = Task {
            id: "t3".to_string(),
            name: "test".to_string(),
            description: None,
            task_type: TaskType::PipelineReview,
            schedule_type: crate::task_system::models::ScheduleType::Once,
            cron_pattern: None,
            payload: Some(r#"{"story_id": "s3", "draft_id": "d3", "operation": "finalize", "chapter_number": 5, "chapter_title": "第五章"}"#.to_string()),
            status: crate::task_system::models::TaskStatus::Pending,
            progress: 0,
            result: None,
            error_message: None,
            max_retries: 3,
            retry_count: 0,
            enabled: true,
            last_run_at: None,
            next_run_at: None,
            last_heartbeat_at: None,
            heartbeat_timeout_seconds: 300,
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };

        let payload = parse_pipeline_payload(&task).unwrap();
        assert_eq!(payload.operation, "finalize");
        assert_eq!(payload.chapter_number, 5);
        assert_eq!(payload.chapter_title, Some("第五章".to_string()));
        assert_eq!(payload.scene_id, None);
    }

    #[test]
    fn test_parse_pipeline_payload_missing_story_id() {
        let task = Task {
            id: "t4".to_string(),
            name: "test".to_string(),
            description: None,
            task_type: TaskType::PipelineReview,
            schedule_type: crate::task_system::models::ScheduleType::Once,
            cron_pattern: None,
            payload: Some(r#"{"draft_id": "d4"}"#.to_string()),
            status: crate::task_system::models::TaskStatus::Pending,
            progress: 0,
            result: None,
            error_message: None,
            max_retries: 3,
            retry_count: 0,
            enabled: true,
            last_run_at: None,
            next_run_at: None,
            last_heartbeat_at: None,
            heartbeat_timeout_seconds: 300,
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };

        let result = parse_pipeline_payload(&task);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("story_id"));
    }

    #[test]
    fn test_parse_pipeline_payload_missing_draft_id() {
        let task = Task {
            id: "t5".to_string(),
            name: "test".to_string(),
            description: None,
            task_type: TaskType::PipelineReview,
            schedule_type: crate::task_system::models::ScheduleType::Once,
            cron_pattern: None,
            payload: Some(r#"{"story_id": "s5"}"#.to_string()),
            status: crate::task_system::models::TaskStatus::Pending,
            progress: 0,
            result: None,
            error_message: None,
            max_retries: 3,
            retry_count: 0,
            enabled: true,
            last_run_at: None,
            next_run_at: None,
            last_heartbeat_at: None,
            heartbeat_timeout_seconds: 300,
            created_at: "2024-01-01".to_string(),
            updated_at: "2024-01-01".to_string(),
        };

        let result = parse_pipeline_payload(&task);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("draft_id"));
    }

    #[test]
    fn test_task_ok() {
        let result = task_ok(Some("{}".to_string()));
        assert!(result.success);
        assert_eq!(result.result_json, Some("{}".to_string()));
        assert_eq!(result.error_message, None);
    }

    #[test]
    fn test_task_err() {
        let result = task_err("Something went wrong");
        assert!(!result.success);
        assert_eq!(result.result_json, None);
        assert_eq!(
            result.error_message,
            Some("Something went wrong".to_string())
        );
    }
}
