//! Cascade Rewrite 任务执行器

use tauri::AppHandle;

use super::{
    models::{CascadeTaskPayload, RewriteStatus},
    rewrite_engine::RewriteEngine,
};
use crate::{
    db::DbPool,
    task_system::{
        executor::TaskExecutor,
        models::{Task, TaskResult},
    },
};

pub struct CascadeRewriteExecutor {
    pool: DbPool,
    app_handle: AppHandle,
}

impl CascadeRewriteExecutor {
    pub fn new(pool: DbPool, app_handle: AppHandle) -> Self {
        Self { pool, app_handle }
    }
}

#[async_trait::async_trait]
impl TaskExecutor for CascadeRewriteExecutor {
    async fn execute(&self, task: &Task) -> Result<TaskResult, Box<dyn std::error::Error>> {
        log::info!("[CascadeRewriteExecutor] Task {} started", task.id);

        let payload: CascadeTaskPayload = match task.payload.as_ref() {
            Some(p) => serde_json::from_str(p)?,
            None => {
                return Ok(TaskResult {
                    success: false,
                    result_json: None,
                    error_message: Some("Missing payload".to_string()),
                });
            }
        };

        let engine = RewriteEngine::new(self.pool.clone(), self.app_handle.clone());
        let result = engine.execute(&payload).await?;

        let success = result.status != RewriteStatus::Failed;
        let result_json = serde_json::to_string(&result).ok();

        log::info!(
            "[CascadeRewriteExecutor] Task {} completed with status: {:?}",
            task.id,
            result.status
        );

        Ok(TaskResult {
            success,
            result_json,
            error_message: if success {
                None
            } else {
                Some("Rewrite engine failed".to_string())
            },
        })
    }

    fn can_handle(&self, task_type: &crate::task_system::models::TaskType) -> bool {
        *task_type == crate::task_system::models::TaskType::CascadeRewrite
    }
}
