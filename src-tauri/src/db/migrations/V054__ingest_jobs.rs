use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        54
    }

    fn description(&self) -> &'static str {
        "ingest jobs"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let ingest_jobs_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='ingest_jobs'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if ingest_jobs_tables.is_empty() {
            conn.execute(
                "CREATE TABLE ingest_jobs (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_id TEXT,
                status TEXT NOT NULL,
                error_message TEXT,
                created_at TEXT NOT NULL,
                completed_at TEXT
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_ingest_jobs_story ON ingest_jobs(story_id, created_at)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_ingest_jobs_status ON ingest_jobs(story_id, status)",
                [],
            )?;
        }
        Ok(())
    }
}
