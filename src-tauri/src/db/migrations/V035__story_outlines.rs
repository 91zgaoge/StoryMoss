use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        35
    }

    fn description(&self) -> &'static str {
        "story outlines"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let outline_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='story_outlines'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if outline_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_outlines (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL UNIQUE,
                content TEXT NOT NULL,
                structure_json TEXT,
                act_count INTEGER DEFAULT 3,
                total_scenes_estimate INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_outlines_story ON story_outlines(story_id)",
                [],
            )?;
        }
        Ok(())
    }
}
