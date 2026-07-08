use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        62
    }

    fn description(&self) -> &'static str {
        "llm calls"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let llm_call_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='llm_calls'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if llm_call_tables.is_empty() {
            conn.execute(
                "CREATE TABLE llm_calls (
                id TEXT PRIMARY KEY,
                story_id TEXT,
                draft_id TEXT,
                revision_id TEXT,
                model_id TEXT NOT NULL,
                model_name TEXT,
                purpose TEXT NOT NULL,
                prompt_tokens INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                success INTEGER NOT NULL DEFAULT 1,
                error_message TEXT,
                prompt_preview TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_llm_calls_time ON llm_calls(created_at)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_llm_calls_story ON llm_calls(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_llm_calls_purpose ON llm_calls(purpose)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_llm_calls_model ON llm_calls(model_id)",
                [],
            )?;
        }
        Ok(())
    }
}
