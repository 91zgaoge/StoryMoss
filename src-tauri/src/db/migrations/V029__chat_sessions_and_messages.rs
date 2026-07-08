use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        29
    }

    fn description(&self) -> &'static str {
        "chat sessions and messages"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let chat_session_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='chat_sessions'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if chat_session_tables.is_empty() {
            conn.execute(
                "CREATE TABLE chat_sessions (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                title TEXT NOT NULL,
                context TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_chat_sessions_story ON chat_sessions(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE chat_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_chat_messages_session ON chat_messages(session_id)",
                [],
            )?;
        }
        Ok(())
    }
}
