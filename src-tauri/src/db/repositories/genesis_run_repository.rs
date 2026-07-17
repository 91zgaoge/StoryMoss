use super::*;

// ==================== GenesisRun Repository (W2-B9) ====================

pub struct GenesisRunRepository {
    pool: DbPool,
}

// Task 7: 创世写路径已切到 agency，部分方法暂失消费者（读路径仪表盘仍在用）；Task 8 处理。
#[allow(dead_code)]
impl GenesisRunRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        id: &str,
        session_id: &str,
        premise: &str,
        total_steps: i32,
    ) -> Result<GenesisRun, rusqlite::Error> {
        let now = Local::now();
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO genesis_runs (id, session_id, premise, status, total_steps, steps_json, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                session_id,
                premise,
                "pending",
                total_steps,
                "{}",
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;
        Ok(GenesisRun {
            id: id.to_string(),
            story_id: None,
            session_id: session_id.to_string(),
            premise: premise.to_string(),
            status: "pending".to_string(),
            current_step: None,
            current_step_number: 0,
            total_steps,
            steps_json: "{}".to_string(),
            error_message: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_step(
        &self,
        id: &str,
        step_name: &str,
        step_number: i32,
        status: &str,
        steps_json: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET current_step = ?2, current_step_number = ?3, status = ?4, \
             steps_json = ?5, updated_at = ?6 WHERE id = ?1",
            params![id, step_name, step_number, status, steps_json, now],
        )
    }

    pub fn complete(&self, id: &str, story_id: Option<&str>) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET status = 'completed', story_id = ?2, updated_at = ?3 WHERE \
             id = ?1",
            params![id, story_id, now],
        )
    }

    /// v0.26.19 P0-4: 在概念生成完成、首章尚未结束时，记录 story_id
    /// 并切换到中间状态。 与 `complete` 区别：不标记
    /// completed，允许后台阶段继续更新同一记录。
    pub fn set_story_id_and_status(
        &self,
        id: &str,
        story_id: &str,
        status: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET story_id = ?2, status = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, story_id, status, now],
        )
    }

    /// v0.26.19 P0-4: 更新 steps_json（用于记录步骤执行明细或累计的错误列表）。
    pub fn update_steps_json(&self, id: &str, steps_json: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET steps_json = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, steps_json, now],
        )
    }

    pub fn fail(&self, id: &str, error_message: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET status = 'failed', error_message = ?2, updated_at = ?3 WHERE \
             id = ?1",
            params![id, error_message, now],
        )
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<GenesisRun>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, session_id, premise, status, current_step, current_step_number, \
             total_steps, steps_json, error_message, created_at, updated_at FROM genesis_runs \
             WHERE id = ?1",
        )?;
        let run = stmt
            .query_row([id], |row| {
                let created_str: String = row.get(10)?;
                let updated_str: String = row.get(11)?;
                Ok(GenesisRun {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    session_id: row.get(2)?,
                    premise: row.get(3)?,
                    status: row.get(4)?,
                    current_step: row.get(5)?,
                    current_step_number: row.get(6)?,
                    total_steps: row.get(7)?,
                    steps_json: row.get(8)?,
                    error_message: row.get(9)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;
        Ok(run)
    }

    pub fn list_all(&self, limit: i64) -> Result<Vec<GenesisRun>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, session_id, premise, status, current_step, current_step_number, \
             total_steps, steps_json, error_message, created_at, updated_at FROM genesis_runs \
             ORDER BY created_at DESC LIMIT ?1",
        )?;
        let runs = stmt
            .query_map([limit], |row| {
                let created_str: String = row.get(10)?;
                let updated_str: String = row.get(11)?;
                Ok(GenesisRun {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    session_id: row.get(2)?,
                    premise: row.get(3)?,
                    status: row.get(4)?,
                    current_step: row.get(5)?,
                    current_step_number: row.get(6)?,
                    total_steps: row.get(7)?,
                    steps_json: row.get(8)?,
                    error_message: row.get(9)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(runs)
    }
}
