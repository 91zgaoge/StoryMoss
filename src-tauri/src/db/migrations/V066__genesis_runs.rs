use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        66
    }

    fn description(&self) -> &'static str {
        "genesis runs"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let genesis_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='genesis_runs'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if genesis_tables.is_empty() {
            conn.execute(
                "CREATE TABLE genesis_runs (
                id TEXT PRIMARY KEY,
                story_id TEXT,
                session_id TEXT NOT NULL,
                premise TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                current_step TEXT,
                current_step_number INTEGER NOT NULL DEFAULT 0,
                total_steps INTEGER NOT NULL DEFAULT 7,
                steps_json TEXT NOT NULL DEFAULT '{}',
                error_message TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_genesis_runs_story ON genesis_runs(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_genesis_runs_session ON genesis_runs(session_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_genesis_runs_status ON genesis_runs(status)",
                [],
            )?;
        }
        Ok(())
    }
}
