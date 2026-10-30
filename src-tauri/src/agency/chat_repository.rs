use rusqlite::{params, OptionalExtension};

use crate::{
    agency::models::{ChatMessage, ChatSession},
    db::DbPool,
    error::AppError,
};

/// 剧组对话会话/消息持久化。
pub struct ChatRepository {
    pool: DbPool,
}

impl ChatRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_session(
        &self,
        story_id: &str,
        title: &str,
        target_role: &str,
    ) -> Result<ChatSession, AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        let session = ChatSession {
            id: uuid::Uuid::new_v4().to_string(),
            story_id: story_id.to_string(),
            title: title.to_string(),
            target_role: target_role.to_string(),
            summary: None,
            summary_upto_message_id: None,
            archived: false,
            created_at: now.clone(),
            updated_at: now,
        };
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "INSERT INTO agency_chat_sessions
             (id, story_id, title, target_role, summary, summary_upto_message_id, archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, NULL, NULL, 0, ?5, ?6)",
            params![
                session.id,
                session.story_id,
                session.title,
                session.target_role,
                session.created_at,
                session.updated_at
            ],
        )
        .map_err(AppError::from)?;
        Ok(session)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<ChatSession>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.query_row(
            "SELECT id, story_id, title, target_role, summary, summary_upto_message_id,
                    archived, created_at, updated_at
             FROM agency_chat_sessions WHERE id = ?1",
            params![session_id],
            map_session,
        )
        .optional()
        .map_err(AppError::from)
    }

    pub fn latest_session(&self, story_id: &str) -> Result<Option<ChatSession>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.query_row(
            "SELECT id, story_id, title, target_role, summary, summary_upto_message_id,
                    archived, created_at, updated_at
             FROM agency_chat_sessions WHERE story_id = ?1 AND archived = 0
             ORDER BY updated_at DESC LIMIT 1",
            params![story_id],
            map_session,
        )
        .optional()
        .map_err(AppError::from)
    }

    pub fn list_sessions(&self, story_id: &str) -> Result<Vec<ChatSession>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, title, target_role, summary, summary_upto_message_id,
                        archived, created_at, updated_at
                 FROM agency_chat_sessions WHERE story_id = ?1
                 ORDER BY updated_at DESC",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![story_id], map_session)
            .map_err(AppError::from)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn delete_session(&self, session_id: &str) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "DELETE FROM agency_chat_sessions WHERE id = ?1",
            params![session_id],
        )
        .map_err(AppError::from)
    }

    pub fn append_message(&self, message: &ChatMessage) -> Result<(), AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "INSERT INTO agency_chat_messages
             (id, session_id, role, content, status, run_key, task_id, task_ids_json,
              actions_json, meta_json, error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                message.id,
                message.session_id,
                message.role,
                message.content,
                message.status,
                message.run_key,
                message.task_id,
                message.task_ids_json,
                message.actions_json,
                message.meta_json,
                message.error,
                message.created_at,
            ],
        )
        .map_err(AppError::from)?;
        conn.execute(
            "UPDATE agency_chat_sessions SET updated_at = ?2 WHERE id = ?1",
            params![message.session_id, message.created_at],
        )
        .map_err(AppError::from)?;
        Ok(())
    }

    pub fn list_messages(
        &self,
        session_id: &str,
        limit: i32,
    ) -> Result<Vec<ChatMessage>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, role, content, status, run_key, task_id,
                        task_ids_json, actions_json, meta_json, error, created_at
                 FROM agency_chat_messages WHERE session_id = ?1
                 ORDER BY created_at ASC LIMIT ?2",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![session_id, limit], map_message)
            .map_err(AppError::from)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn update_message_status(
        &self,
        message_id: &str,
        status: &str,
        content: Option<&str>,
        error: Option<&str>,
    ) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "UPDATE agency_chat_messages
             SET status = ?2,
                 content = COALESCE(?3, content),
                 error = ?4
             WHERE id = ?1",
            params![message_id, status, content, error],
        )
        .map_err(AppError::from)
    }

    pub fn update_message_payload(
        &self,
        message_id: &str,
        status: &str,
        content: Option<&str>,
        task_ids_json: Option<&str>,
        actions_json: Option<&str>,
        error: Option<&str>,
    ) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "UPDATE agency_chat_messages
             SET status = ?2,
                 content = COALESCE(?3, content),
                 task_ids_json = COALESCE(?4, task_ids_json),
                 actions_json = COALESCE(?5, actions_json),
                 error = ?6
             WHERE id = ?1",
            params![
                message_id,
                status,
                content,
                task_ids_json,
                actions_json,
                error
            ],
        )
        .map_err(AppError::from)
    }

    /// 子 agent 任务完成后追加一条 follow-up 消息（role 为任务角色）。
    pub fn append_task_result(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        status: &str,
        task_id: &str,
        run_key: Option<&str>,
    ) -> Result<ChatMessage, AppError> {
        let mut message = ChatMessage::new(session_id, role, content);
        message.status = status.to_string();
        message.task_id = Some(task_id.to_string());
        message.run_key = run_key.map(|s| s.to_string());
        self.append_message(&message)?;
        Ok(message)
    }
}

fn map_session(row: &rusqlite::Row<'_>) -> Result<ChatSession, rusqlite::Error> {
    Ok(ChatSession {
        id: row.get(0)?,
        story_id: row.get(1)?,
        title: row.get(2)?,
        target_role: row.get(3)?,
        summary: row.get(4)?,
        summary_upto_message_id: row.get(5)?,
        archived: row.get::<_, i64>(6)? != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn map_message(row: &rusqlite::Row<'_>) -> Result<ChatMessage, rusqlite::Error> {
    Ok(ChatMessage {
        id: row.get(0)?,
        session_id: row.get(1)?,
        role: row.get(2)?,
        content: row.get(3)?,
        status: row.get(4)?,
        run_key: row.get(5)?,
        task_id: row.get(6)?,
        task_ids_json: row.get(7)?,
        actions_json: row.get(8)?,
        meta_json: row.get(9)?,
        error: row.get(10)?,
        created_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn repo() -> ChatRepository {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        ChatRepository::new(pool)
    }

    #[test]
    fn test_session_and_messages() {
        let repo = repo();
        let session = repo
            .create_session("story-1", "今晚的调度", "director")
            .unwrap();
        let msg = ChatMessage::new(&session.id, "user", "你好");
        repo.append_message(&msg).unwrap();
        repo.append_task_result(
            &session.id,
            "producer",
            "已补齐",
            "done",
            "t1",
            Some("chat:s"),
        )
        .unwrap();
        let messages = repo.list_messages(&session.id, 10).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].status, "done");
        assert_eq!(repo.list_sessions("story-1").unwrap().len(), 1);
        assert_eq!(
            repo.latest_session("story-1").unwrap().unwrap().id,
            session.id
        );
    }

    #[test]
    fn test_delete_session_cascades_messages() {
        let repo = repo();
        let session = repo.create_session("story-1", "t", "director").unwrap();
        repo.append_message(&ChatMessage::new(&session.id, "user", "x"))
            .unwrap();
        repo.delete_session(&session.id).unwrap();
        assert!(repo.get_session(&session.id).unwrap().is_none());
        assert!(repo.list_messages(&session.id, 10).unwrap().is_empty());
    }
}
