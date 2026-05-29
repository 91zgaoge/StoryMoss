//! AI Generation Task Executor
//!
//! 将 Agent 生成任务接入 Task System，支持后台执行和进度追踪。

use std::collections::HashMap;

use tauri::AppHandle;

use super::{
    orchestrator::{AgentOrchestrator, GenerationMode, WorkflowConfig},
    service::{AgentService, AgentTask, AgentType},
};
use crate::{
    db::DbPool,
    task_system::{
        executor::{TaskExecutionContext, TaskExecutor},
        models::*,
    },
};

pub struct AiGenerationExecutor {
    pool: DbPool,
    app_handle: AppHandle,
}

impl AiGenerationExecutor {
    pub fn new(pool: DbPool, app_handle: AppHandle) -> Self {
        Self { pool, app_handle }
    }
}

#[async_trait::async_trait]
impl TaskExecutor for AiGenerationExecutor {
    fn can_handle(&self, task_type: &TaskType) -> bool {
        *task_type == TaskType::AiGeneration
    }

    async fn execute(&self, task: &Task) -> Result<TaskResult, Box<dyn std::error::Error>> {
        let ctx =
            TaskExecutionContext::new(task.id.clone(), self.pool.clone(), self.app_handle.clone());

        let payload: serde_json::Value = match task.payload.as_deref() {
            Some(p) => serde_json::from_str(p).unwrap_or_else(|_| serde_json::json!({})),
            None => serde_json::json!({}),
        };

        let story_id = payload
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if story_id.is_empty() {
            return Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some("Missing story_id in payload".to_string()),
            });
        }

        let chapter_number = payload
            .get("chapter_number")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        let input = payload
            .get("input")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        let agent_type: AgentType = payload
            .get("agent_type")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or(AgentType::Writer);

        let mode = match payload.get("mode").and_then(|v| v.as_str()) {
            Some("fast") => GenerationMode::Fast,
            _ => GenerationMode::Full,
        };

        let parameters: HashMap<String, serde_json::Value> = payload
            .get("parameters")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        ctx.update_progress("build_context", 10, "构建 Agent 上下文...");
        ctx.heartbeat();

        let request = super::commands::ExecuteAgentRequest {
            agent_type,
            story_id: story_id.clone(),
            chapter_number: Some(chapter_number),
            input: input.clone(),
            parameters: Some(parameters.clone()),
        };

        let context = match super::commands::build_agent_context(&self.app_handle, &request).await {
            Ok(ctx) => ctx,
            Err(e) => {
                ctx.log("error", &format!("构建上下文失败: {}", e));
                return Ok(TaskResult {
                    success: false,
                    result_json: None,
                    error_message: Some(format!("构建上下文失败: {}", e)),
                });
            }
        };

        ctx.update_progress("generate", 30, "执行 AI 生成...");
        ctx.heartbeat();

        let service = AgentService::new(self.app_handle.clone());
        let config = WorkflowConfig::default();
        let orchestrator = AgentOrchestrator::new(service, config, self.app_handle.clone());

        let agent_task = AgentTask {
            id: task.id.clone(),
            agent_type,
            context,
            input,
            parameters,
            tier: None,
        };

        if ctx.is_cancelled() {
            return Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some("任务已取消".to_string()),
            });
        }

        match orchestrator.generate(agent_task, mode).await {
            Ok(result) => {
                let result_json = serde_json::to_string(&serde_json::json!({
                    "content": result.final_content,
                    "score": result.final_score,
                    "was_rewritten": result.was_rewritten,
                    "rewrite_count": result.rewrite_count,
                    "steps": result.steps.len(),
                }))?;

                ctx.update_progress("complete", 100, "生成完成");
                Ok(TaskResult {
                    success: true,
                    result_json: Some(result_json),
                    error_message: None,
                })
            }
            Err(e) => {
                ctx.log("error", &format!("生成失败: {}", e));
                Ok(TaskResult {
                    success: false,
                    result_json: None,
                    error_message: Some(format!("生成失败: {}", e)),
                })
            }
        }
    }
}
