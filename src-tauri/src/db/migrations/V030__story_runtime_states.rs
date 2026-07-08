use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        30
    }

    fn description(&self) -> &'static str {
        "story runtime states"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let story_state_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='story_runtime_states'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if story_state_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_runtime_states (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL UNIQUE,
                state_json TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_runtime_states_story ON story_runtime_states(story_id)",
                [],
            )?;
        }
        Ok(())
    }
}
