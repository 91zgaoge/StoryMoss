//! Pipeline Review Task Executor
//!
//! 将 Pipeline 修稿/审稿/定稿任务接入 Task System，支持后台执行和进度追踪。

use tauri::AppHandle;

use super::types::*;
use crate::{
    db::DbPool,
    llm::LlmService,
    task_system::{
        executor::{TaskExecutionContext, TaskExecutor},
        models::*,
    },
};

pub struct PipelineReviewExecutor {
    pool: DbPool,
    app_handle: AppHandle,
}

impl PipelineReviewExecutor {
    pub fn new(pool: DbPool, app_handle: AppHandle) -> Self {
        Self { pool, app_handle }
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

#[async_trait::async_trait]
impl TaskExecutor for PipelineReviewExecutor {
    fn can_handle(&self, task_type: &TaskType) -> bool {
        *task_type == TaskType::PipelineReview
    }

    async fn execute(&self, task: &Task) -> Result<TaskResult, Box<dyn std::error::Error>> {
        let ctx =
            TaskExecutionContext::new(task.id.clone(), self.pool.clone(), self.app_handle.clone());

        let payload: serde_json::Value = match task.payload.as_deref() {
            Some(p) => serde_json::from_str(p).unwrap_or_else(|_| serde_json::json!({})),
            None => serde_json::json!({}),
        };

        let operation = payload
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("review");

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
            return Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some("Missing story_id or draft_id in payload".to_string()),
            });
        }

        let config = PipelineConfig::default();
        let llm_service = LlmService::new(self.app_handle.clone());
        let callbacks = TaskPipelineCallbacks {
            task_id: task.id.clone(),
            pool: self.pool.clone(),
            app_handle: self.app_handle.clone(),
        };

        let result = match operation {
            "refine" => {
                let user_prompt = payload.get("user_prompt").and_then(|v| v.as_str());
                ctx.update_progress("refine", 5, "开始修稿...");
                match super::refine_draft(
                    &story_id,
                    &draft_id,
                    user_prompt,
                    &config,
                    &self.pool,
                    &llm_service,
                    &callbacks,
                )
                .await
                {
                    Ok(result) => {
                        let json = serde_json::to_string(&result)?;
                        Ok(TaskResult {
                            success: true,
                            result_json: Some(json),
                            error_message: None,
                        })
                    }
                    Err(e) => Ok(TaskResult {
                        success: false,
                        result_json: None,
                        error_message: Some(e.to_string()),
                    }),
                }
            }
            "review" => {
                let review_focus = payload.get("review_focus").and_then(|v| v.as_str());
                ctx.update_progress("review", 5, "开始审稿...");
                match super::review_draft(
                    &story_id,
                    &draft_id,
                    review_focus,
                    &config,
                    &self.pool,
                    &llm_service,
                    &callbacks,
                )
                .await
                {
                    Ok(result) => {
                        let json = serde_json::to_string(&result)?;
                        Ok(TaskResult {
                            success: true,
                            result_json: Some(json),
                            error_message: None,
                        })
                    }
                    Err(e) => Ok(TaskResult {
                        success: false,
                        result_json: None,
                        error_message: Some(e.to_string()),
                    }),
                }
            }
            "finalize" => {
                let chapter_number = payload
                    .get("chapter_number")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1) as i32;
                let chapter_title = payload
                    .get("chapter_title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let chapter_info = ChapterInfo {
                    chapter_number,
                    title: chapter_title,
                };
                ctx.update_progress("finalize", 5, "开始定稿...");
                match super::finalize_draft(
                    &story_id,
                    &draft_id,
                    &chapter_info,
                    &config,
                    &self.pool,
                    &self.app_handle,
                    &callbacks,
                )
                .await
                {
                    Ok(post_process_run_id) => {
                        let json = serde_json::to_string(&serde_json::json!({
                            "post_process_run_id": post_process_run_id,
                        }))?;
                        Ok(TaskResult {
                            success: true,
                            result_json: Some(json),
                            error_message: None,
                        })
                    }
                    Err(e) => Ok(TaskResult {
                        success: false,
                        result_json: None,
                        error_message: Some(e.to_string()),
                    }),
                }
            }
            _ => Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some(format!("Unknown pipeline operation: {}", operation)),
            }),
        };

        ctx.heartbeat();
        result
    }
}
