use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        48
    }

    fn description(&self) -> &'static str {
        "chapter reading power"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let reading_power_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND \
             name='chapter_reading_power'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if reading_power_tables.is_empty() {
            conn.execute(
                "CREATE TABLE chapter_reading_power (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                scene_id TEXT,
                chapter_number INTEGER NOT NULL,
                hook_type TEXT,
                hook_strength TEXT DEFAULT 'medium',
                coolpoint_patterns_json TEXT,
                micropayoffs_json TEXT,
                hard_violations_json TEXT,
                soft_suggestions_json TEXT,
                is_transition INTEGER NOT NULL DEFAULT 0,
                override_count INTEGER NOT NULL DEFAULT 0,
                debt_balance REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_reading_power_story ON chapter_reading_power(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE UNIQUE INDEX idx_reading_power_chapter ON chapter_reading_power(story_id, \
             chapter_number)",
                [],
            )?;
        }
        Ok(())
    }
}
