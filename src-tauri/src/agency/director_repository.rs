use rusqlite::{params, OptionalExtension};

use crate::{agency::models::DirectorAction, db::DbPool, error::AppError};

pub struct DirectorRepository {
    pool: DbPool,
}

impl DirectorRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_action(
        &self,
        story_id: &str,
        session_id: Option<&str>,
        message_id: Option<&str>,
        run_id: Option<&str>,
        action_type: &str,
        target_role: Option<&str>,
        target_id: Option<&str>,
        params_json: &str,
        status: &str,
        reason: Option<&str>,
    ) -> Result<DirectorAction, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO agency_director_actions
             (id, story_id, session_id, message_id, run_id, action_type, target_role,
              target_id, params_json, status, reason, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id,
                story_id,
                session_id,
                message_id,
                run_id,
                action_type,
                target_role,
                target_id,
                params_json,
                status,
                reason,
                now,
                now,
            ],
        )
        .map_err(AppError::from)?;
        Ok(DirectorAction {
            id,
            story_id: story_id.to_string(),
            session_id: session_id.map(|s| s.to_string()),
            message_id: message_id.map(|s| s.to_string()),
            run_id: run_id.map(|s| s.to_string()),
            action_type: action_type.to_string(),
            target_role: target_role.map(|s| s.to_string()),
            target_id: target_id.map(|s| s.to_string()),
            params_json: params_json.to_string(),
            status: status.to_string(),
            reason: reason.map(|s| s.to_string()),
            result_summary: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn get(&self, id: &str) -> Result<Option<DirectorAction>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.query_row(
            "SELECT id, story_id, session_id, message_id, run_id, action_type, target_role,
                    target_id, params_json, status, reason, result_summary, error,
                    created_at, updated_at
             FROM agency_director_actions WHERE id = ?1",
            params![id],
            map_action,
        )
        .optional()
        .map_err(AppError::from)
    }

    pub fn list_proposed(
        &self,
        story_id: &str,
        session_id: Option<&str>,
    ) -> Result<Vec<DirectorAction>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, session_id, message_id, run_id, action_type, target_role,
                        target_id, params_json, status, reason, result_summary, error,
                        created_at, updated_at
                 FROM agency_director_actions
                 WHERE story_id = ?1 AND status = 'proposed'
                   AND (?2 IS NULL OR session_id = ?2)
                 ORDER BY created_at ASC",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![story_id, session_id], map_action)
            .map_err(AppError::from)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn update_status(
        &self,
        id: &str,
        status: &str,
        result_summary: Option<&str>,
        error: Option<&str>,
    ) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "UPDATE agency_director_actions
             SET status = ?2, result_summary = ?3, error = ?4, updated_at = ?5
             WHERE id = ?1",
            params![
                id,
                status,
                result_summary,
                error,
                chrono::Utc::now().to_rfc3339()
            ],
        )
        .map_err(AppError::from)
    }
}

fn map_action(row: &rusqlite::Row<'_>) -> Result<DirectorAction, rusqlite::Error> {
    Ok(DirectorAction {
        id: row.get(0)?,
        story_id: row.get(1)?,
        session_id: row.get(2)?,
        message_id: row.get(3)?,
        run_id: row.get(4)?,
        action_type: row.get(5)?,
        target_role: row.get(6)?,
        target_id: row.get(7)?,
        params_json: row.get(8)?,
        status: row.get(9)?,
        reason: row.get(10)?,
        result_summary: row.get(11)?,
        error: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn repo() -> DirectorRepository {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        DirectorRepository::new(pool)
    }

    #[test]
    fn test_insert_list_update() {
        let repo = repo();
        let a = repo
            .insert_action(
                "story-1",
                Some("s1"),
                Some("m1"),
                None,
                "dispatch",
                Some("producer"),
                None,
                "{}",
                "proposed",
                Some("补资产"),
            )
            .unwrap();
        assert_eq!(repo.list_proposed("story-1", Some("s1")).unwrap().len(), 1);
        repo.update_status(&a.id, "done", Some("ok"), None).unwrap();
        assert!(repo
            .list_proposed("story-1", Some("s1"))
            .unwrap()
            .is_empty());
        assert_eq!(repo.get(&a.id).unwrap().unwrap().status, "done");
    }
}
