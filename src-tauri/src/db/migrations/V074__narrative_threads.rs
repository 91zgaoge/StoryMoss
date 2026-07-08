use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        74
    }

    fn description(&self) -> &'static str {
        "narrative threads"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let thread_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_threads'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if thread_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_threads (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL REFERENCES stories(id),
                thread_type TEXT NOT NULL,
                target_id TEXT NOT NULL,
                thread_data TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_threads_story ON narrative_threads(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_threads_type ON narrative_threads(thread_type)",
                [],
            )?;
        }
        Ok(())
    }
}
