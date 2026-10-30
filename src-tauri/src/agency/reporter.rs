use std::sync::Arc;

use tauri::{AppHandle, Emitter};

use crate::{
    agency::{
        models::{AgencyAgentTask, AgentRole},
        task_repository::TaskRepository,
        tool_loop::LoopLlm,
    },
    db::DbPool,
    error::AppError,
    router::TaskType,
};

pub const EVENT_CREW_UPDATE: &str = "agency-crew-update";

/// 任务心跳/结束报告器。心跳 fire-and-forget；结束必须 await 落库。
#[derive(Clone)]
pub struct TaskReporter {
    pool: DbPool,
    task_id: String,
    story_id: String,
    run_id: String,
    role: AgentRole,
    app: Option<AppHandle>,
}

impl TaskReporter {
    pub fn new(
        pool: DbPool,
        task_id: impl Into<String>,
        story_id: impl Into<String>,
        run_id: impl Into<String>,
        role: AgentRole,
        app: Option<AppHandle>,
    ) -> Self {
        Self {
            pool,
            task_id: task_id.into(),
            story_id: story_id.into(),
            run_id: run_id.into(),
            role,
            app,
        }
    }

    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    pub async fn start(&self) -> Result<(), AppError> {
        let pool = self.pool.clone();
        let id = self.task_id.clone();
        tokio::task::spawn_blocking(move || {
            TaskRepository::new(pool).start(&id).map_err(AppError::from)
        })
        .await
        .map_err(|e| AppError::from(format!("start join error: {e}")))??;
        self.emit_update().await;
        Ok(())
    }

    pub fn heartbeat(&self, phase: &str, last_tool: Option<&str>) {
        let pool = self.pool.clone();
        let id = self.task_id.clone();
        let phase = phase.to_string();
        let tool = last_tool.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            let _ = TaskRepository::new(pool).heartbeat(&id, &phase, 0, tool.as_deref());
        });
    }

    pub async fn finish(
        &self,
        status: &str,
        result_summary: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), AppError> {
        let pool = self.pool.clone();
        let id = self.task_id.clone();
        let status_s = status.to_string();
        let summary = result_summary.map(|s| s.to_string());
        let err_s = error.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            TaskRepository::new(pool)
                .finish(&id, &status_s, summary.as_deref(), err_s.as_deref())
                .map_err(AppError::from)
        })
        .await
        .map_err(|e| AppError::from(format!("finish join error: {e}")))??;
        self.emit_update().await;
        Ok(())
    }

    async fn emit_update(&self) {
        let Some(app) = &self.app else {
            return;
        };
        let pool = self.pool.clone();
        let id = self.task_id.clone();
        let task = tokio::task::spawn_blocking(move || TaskRepository::new(pool).get_task(&id))
            .await
            .ok()
            .and_then(|r| r.ok())
            .flatten();
        if let Some(task) = task {
            let _ = app.emit(
                EVENT_CREW_UPDATE,
                serde_json::json!({
                    "story_id": task.story_id,
                    "task": task,
                }),
            );
        }
    }

    pub fn story_id(&self) -> &str {
        &self.story_id
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn role(&self) -> AgentRole {
        self.role
    }
}

/// 给 ToolLoop 用的 LLM 包装：每次调用前后写心跳。
pub struct ReportingLlm {
    inner: Arc<dyn LoopLlm>,
    reporter: TaskReporter,
}

impl ReportingLlm {
    pub fn new(inner: Arc<dyn LoopLlm>, reporter: TaskReporter) -> Self {
        Self { inner, reporter }
    }
}

#[async_trait::async_trait]
impl LoopLlm for ReportingLlm {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        self.reporter.heartbeat("llm", Some("llm"));
        self.inner
            .complete(system_prompt, user_prompt, task, max_tokens)
            .await
    }

    async fn complete_metered(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<(String, i32, f64), AppError> {
        self.reporter.heartbeat("llm", Some("llm"));
        self.inner
            .complete_metered(system_prompt, user_prompt, task, max_tokens)
            .await
    }

    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        self.reporter.heartbeat("llm_json", Some("llm_json"));
        self.inner
            .complete_json(system_prompt, user_prompt, task, max_tokens)
            .await
    }

    async fn complete_turn_metered(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
        tools: Option<&[crate::llm::adapter::ToolSpec]>,
    ) -> Result<(crate::agency::tool_loop::LlmTurn, i32, f64), AppError> {
        self.reporter.heartbeat("llm_turn", Some("llm_turn"));
        self.inner
            .complete_turn_metered(system_prompt, user_prompt, task, max_tokens, tools)
            .await
    }
}

/// 便捷构造：任务行 + reporter。
pub fn create_reporter(
    pool: DbPool,
    task: &AgencyAgentTask,
    app: Option<AppHandle>,
) -> Option<TaskReporter> {
    Some(TaskReporter::new(
        pool,
        task.id.clone(),
        task.story_id.clone(),
        task.run_id.clone(),
        task.role,
        app,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    struct FixedLlm;

    #[async_trait::async_trait]
    impl LoopLlm for FixedLlm {
        async fn complete(
            &self,
            _s: &str,
            _u: &str,
            _t: TaskType,
            _m: i32,
        ) -> Result<String, AppError> {
            Ok("输出".to_string())
        }
    }

    #[tokio::test]
    async fn test_reporting_llm_heartbeats_and_finish() {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        let td = crate::agency::models::AgencyAgentTask::new(
            "chat:s1",
            "story-1",
            AgentRole::Producer,
            "补齐世界观",
            None,
            "{}",
        );
        TaskRepository::new(pool.clone()).create_task(&td).unwrap();
        let reporter = TaskReporter::new(
            pool.clone(),
            &td.id,
            "story-1",
            "chat:s1",
            AgentRole::Producer,
            None,
        );
        reporter.start().await.unwrap();
        let llm = ReportingLlm::new(Arc::new(FixedLlm), reporter.clone());
        let out = llm
            .complete("s", "u", TaskType::Analysis, 100)
            .await
            .unwrap();
        assert_eq!(out, "输出");
        reporter
            .finish("completed", Some("ok"), None)
            .await
            .unwrap();
        let loaded = TaskRepository::new(pool).get_task(&td.id).unwrap().unwrap();
        assert_eq!(loaded.status, "completed");
        assert_eq!(loaded.turn_count, 1);
        assert_eq!(loaded.result_summary.as_deref(), Some("ok"));
    }
}
