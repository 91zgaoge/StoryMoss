use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        60
    }

    fn description(&self) -> &'static str {
        "post process runs"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let post_process_run_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='post_process_runs'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if post_process_run_tables.is_empty() {
            conn.execute(
                "CREATE TABLE post_process_runs (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                chapter_number INTEGER NOT NULL,
                source_label TEXT NOT NULL,
                scope TEXT,
                status TEXT NOT NULL DEFAULT 'running',
                started_at TEXT NOT NULL,
                completed_at TEXT,
                error_message TEXT,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_post_process_runs_story ON post_process_runs(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_post_process_runs_chapter ON post_process_runs(story_id, \
             chapter_number)",
                [],
            )?;
        }

        let post_process_step_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='post_process_steps'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if post_process_step_tables.is_empty() {
            conn.execute(
                "CREATE TABLE post_process_steps (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL,
                step_key TEXT NOT NULL,
                step_label TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                critical INTEGER NOT NULL DEFAULT 0,
                log_output TEXT,
                error_message TEXT,
                started_at TEXT,
                completed_at TEXT,
                FOREIGN KEY (run_id) REFERENCES post_process_runs(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_post_process_steps_run ON post_process_steps(run_id)",
                [],
            )?;
        }
        Ok(())
    }
}
