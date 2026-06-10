pub mod ot;
pub mod websocket;

use chrono::{DateTime, Utc};
#[allow(unused_imports)]
pub use ot::*;
use rusqlite::params;
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
pub use websocket::*;

use crate::{db::DbPool, error::AppError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollabSession {
    pub id: String,
    pub story_id: String,
    pub chapter_id: Option<String>,
    pub participants: Vec<Participant>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    pub user_id: String,
    pub user_name: String,
    pub cursor_position: Option<CursorPosition>,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CursorPosition {
    pub line: i32,
    pub column: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditOperation {
    pub id: String,
    pub session_id: String,
    pub user_id: String,
    pub operation_type: OperationType,
    pub position: CursorPosition,
    pub content: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationType {
    Insert,
    Delete,
    Replace,
}

pub struct CollabManager {
    pool: DbPool,
}

impl CollabManager {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_session(
        &self,
        story_id: String,
        chapter_id: Option<String>,
    ) -> Result<CollabSession, AppError> {
        let session = CollabSession {
            id: uuid::Uuid::new_v4().to_string(),
            story_id: story_id.clone(),
            chapter_id: chapter_id.clone(),
            participants: Vec::new(),
            created_at: Utc::now(),
        };

        let conn = self.pool.get().map_err(AppError::from)?;
        conn.execute(
            "INSERT INTO collab_sessions (id, story_id, chapter_id, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                &session.id,
                &story_id,
                &chapter_id,
                session.created_at.to_rfc3339(),
            ],
        )
        .map_err(AppError::from)?;

        Ok(session)
    }

    pub fn get_session(&self, session_id: &str) -> Result<Option<CollabSession>, AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, chapter_id, created_at
             FROM collab_sessions WHERE id = ?1",
            )
            .map_err(AppError::from)?;

        let session_result = stmt.query_row([session_id], |row| {
            let created_at_str: String = row.get(3)?;
            Ok(CollabSession {
                id: row.get(0)?,
                story_id: row.get(1)?,
                chapter_id: row.get(2)?,
                participants: Vec::new(),
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        });

        let mut session = match session_result {
            Ok(s) => s,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(AppError::from(e)),
        };

        // Load participants
        let mut part_stmt = conn
            .prepare(
                "SELECT user_id, user_name, cursor_line, cursor_column, joined_at
             FROM collab_participants WHERE session_id = ?1",
            )
            .map_err(AppError::from)?;

        let participants = part_stmt
            .query_map([session_id], |row| {
                let line: Option<i32> = row.get(2)?;
                let column: Option<i32> = row.get(3)?;
                let joined_at_str: String = row.get(4)?;
                Ok(Participant {
                    user_id: row.get(0)?,
                    user_name: row.get(1)?,
                    cursor_position: line.map(|l| CursorPosition {
                        line: l,
                        column: column.unwrap_or(0),
                    }),
                    joined_at: DateTime::parse_from_rfc3339(&joined_at_str)
                        .map(|d| d.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .map_err(AppError::from)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;

        session.participants = participants;
        Ok(Some(session))
    }

    pub fn join_session(
        &self,
        session_id: &str,
        user_id: String,
        user_name: String,
    ) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        conn.execute(
            "INSERT OR REPLACE INTO collab_participants
             (id, session_id, user_id, user_name, cursor_line, cursor_column, joined_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                uuid::Uuid::new_v4().to_string(),
                session_id,
                &user_id,
                &user_name,
                Option::<i32>::None,
                Option::<i32>::None,
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(AppError::from)?;
        Ok(())
    }

    pub fn leave_session(&self, session_id: &str, user_id: &str) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        conn.execute(
            "DELETE FROM collab_participants WHERE session_id = ?1 AND user_id = ?2",
            [session_id, user_id],
        )
        .map_err(AppError::from)?;
        Ok(())
    }

    pub fn update_cursor(
        &self,
        session_id: &str,
        user_id: &str,
        position: CursorPosition,
    ) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        conn.execute(
            "UPDATE collab_participants
             SET cursor_line = ?1, cursor_column = ?2
             WHERE session_id = ?3 AND user_id = ?4",
            params![position.line, position.column, session_id, user_id],
        )
        .map_err(AppError::from)?;
        Ok(())
    }

    pub fn delete_session(&self, session_id: &str) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        conn.execute("DELETE FROM collab_sessions WHERE id = ?1", [session_id])
            .map_err(AppError::from)?;
        Ok(())
    }
}
