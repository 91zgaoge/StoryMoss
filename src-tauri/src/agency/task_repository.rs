use rusqlite::{params, OptionalExtension};

use crate::{
    agency::models::{AgencyAgentTask, AgentRole},
    db::DbPool,
};

/// 子 agent 任务注册表：任务状态、心跳、轮次、父子关系。
pub struct TaskRepository {
    pool: DbPool,
}

impl TaskRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_task(&self, task: &AgencyAgentTask) -> Result<(), rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "INSERT INTO agency_agent_tasks
             (id, run_id, story_id, role, parent_task_id, task, status, phase,
              turn_count, tool_calls, last_tool, heartbeat_at, started_at, finished_at,
              error, result_summary, meta_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                task.id,
                task.run_id,
                task.story_id,
                task.role.as_str(),
                task.parent_task_id,
                task.task,
                task.status,
                task.phase,
                task.turn_count,
                task.tool_calls,
                task.last_tool,
                task.heartbeat_at,
                task.started_at,
                task.finished_at,
                task.error,
                task.result_summary,
                task.meta_json,
                task.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_task(&self, task_id: &str) -> Result<Option<AgencyAgentTask>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.query_row(
            "SELECT id, run_id, story_id, role, parent_task_id, task, status, phase,
                    turn_count, tool_calls, last_tool, heartbeat_at, started_at, finished_at,
                    error, result_summary, meta_json, created_at
             FROM agency_agent_tasks WHERE id = ?1",
            params![task_id],
            map_task,
        )
        .optional()
    }

    pub fn list_for_story(
        &self,
        story_id: &str,
        limit: i32,
    ) -> Result<Vec<AgencyAgentTask>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, story_id, role, parent_task_id, task, status, phase,
                    turn_count, tool_calls, last_tool, heartbeat_at, started_at, finished_at,
                    error, result_summary, meta_json, created_at
             FROM agency_agent_tasks WHERE story_id = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![story_id, limit], map_task)?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    pub fn list_for_run(&self, run_id: &str) -> Result<Vec<AgencyAgentTask>, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        let mut stmt = conn.prepare(
            "SELECT id, run_id, story_id, role, parent_task_id, task, status, phase,
                    turn_count, tool_calls, last_tool, heartbeat_at, started_at, finished_at,
                    error, result_summary, meta_json, created_at
             FROM agency_agent_tasks WHERE run_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![run_id], map_task)?;
        rows.collect::<Result<Vec<_>, _>>()
    }

    pub fn start(&self, task_id: &str) -> Result<usize, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_agent_tasks
             SET status = 'running', started_at = ?2, heartbeat_at = ?2
             WHERE id = ?1 AND status IN ('queued', 'waiting')",
            params![task_id, now],
        )
    }

    pub fn set_status(&self, task_id: &str, status: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_agent_tasks SET status = ?2 WHERE id = ?1",
            params![task_id, status],
        )
    }

    /// 心跳：每轮 LLM 调用 +1 turn，tool_calls 用增量（P0 由 ReportingLlm 传
    /// 0）。
    pub fn heartbeat(
        &self,
        task_id: &str,
        phase: &str,
        tool_calls_delta: i32,
        last_tool: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_agent_tasks
             SET phase = ?2,
                 turn_count = turn_count + 1,
                 tool_calls = tool_calls + ?3,
                 last_tool = COALESCE(?4, last_tool),
                 heartbeat_at = ?5
             WHERE id = ?1",
            params![task_id, phase, tool_calls_delta, last_tool, now],
        )
    }

    pub fn finish(
        &self,
        task_id: &str,
        status: &str,
        result_summary: Option<&str>,
        error: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.pool.get().map_err(pool_err)?;
        conn.execute(
            "UPDATE agency_agent_tasks
             SET status = ?2, finished_at = ?3, heartbeat_at = ?3,
                 result_summary = ?4, error = ?5
             WHERE id = ?1",
            params![task_id, status, now, result_summary, error],
        )
    }

    /// 把心跳超时的 running 任务标记 stale。heartbeat_at 存 UTC RFC3339。
    pub fn reap_stale(&self, threshold_secs: u64) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(pool_err)?;
        let modifier = format!("-{} seconds", threshold_secs);
        conn.execute(
            "UPDATE agency_agent_tasks
             SET status = 'stale', finished_at = COALESCE(finished_at, ?2)
             WHERE status = 'running'
               AND heartbeat_at IS NOT NULL
               AND datetime(heartbeat_at) < datetime('now', ?1)",
            params![modifier, chrono::Utc::now().to_rfc3339()],
        )
    }
}

fn pool_err(e: r2d2::Error) -> rusqlite::Error {
    rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
        Some(format!("pool: {e}")),
    )
}

fn map_task(row: &rusqlite::Row<'_>) -> Result<AgencyAgentTask, rusqlite::Error> {
    let role_str: String = row.get(3)?;
    Ok(AgencyAgentTask {
        id: row.get(0)?,
        run_id: row.get(1)?,
        story_id: row.get(2)?,
        role: AgentRole::from_str(&role_str).unwrap_or(AgentRole::Producer),
        parent_task_id: row.get(4)?,
        task: row.get(5)?,
        status: row.get(6)?,
        phase: row.get(7)?,
        turn_count: row.get(8)?,
        tool_calls: row.get(9)?,
        last_tool: row.get(10)?,
        heartbeat_at: row.get(11)?,
        started_at: row.get(12)?,
        finished_at: row.get(13)?,
        error: row.get(14)?,
        result_summary: row.get(15)?,
        meta_json: row.get(16)?,
        created_at: row.get(17)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn repo() -> TaskRepository {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        TaskRepository::new(pool)
    }

    fn task() -> AgencyAgentTask {
        AgencyAgentTask::new(
            "run-1",
            "story-1",
            AgentRole::Producer,
            "补齐世界观",
            None,
            "{}",
        )
    }

    #[test]
    fn test_create_get_and_list() {
        let repo = repo();
        let t = task();
        repo.create_task(&t).unwrap();
        let loaded = repo.get_task(&t.id).unwrap().unwrap();
        assert_eq!(loaded.role, AgentRole::Producer);
        assert_eq!(loaded.status, "queued");
        assert_eq!(repo.list_for_story("story-1", 10).unwrap().len(), 1);
    }

    #[test]
    fn test_start_heartbeat_finish() {
        let repo = repo();
        let t = task();
        repo.create_task(&t).unwrap();
        repo.start(&t.id).unwrap();
        repo.heartbeat(&t.id, "llm", 0, Some("llm")).unwrap();
        repo.heartbeat(&t.id, "llm:done", 0, Some("llm")).unwrap();
        repo.finish(&t.id, "completed", Some("ok"), None).unwrap();
        let loaded = repo.get_task(&t.id).unwrap().unwrap();
        assert_eq!(loaded.status, "completed");
        assert_eq!(loaded.turn_count, 2);
        assert_eq!(loaded.result_summary.as_deref(), Some("ok"));
        assert!(loaded.finished_at.is_some());
    }

    #[test]
    fn test_reap_stale_only_running() {
        let repo = repo();
        let t = task();
        repo.create_task(&t).unwrap();
        repo.start(&t.id).unwrap();
        let conn = repo.pool.get().unwrap();
        conn.execute(
            "UPDATE agency_agent_tasks SET heartbeat_at = datetime('now', '-301 seconds') WHERE id = ?1",
            params![t.id],
        )
        .unwrap();
        assert_eq!(repo.reap_stale(300).unwrap(), 1);
        assert_eq!(repo.get_task(&t.id).unwrap().unwrap().status, "stale");
    }
}
