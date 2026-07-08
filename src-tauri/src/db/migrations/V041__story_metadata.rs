use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        41
    }

    fn description(&self) -> &'static str {
        "story metadata"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let story_metadata_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='story_metadata'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if story_metadata_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_metadata (
                story_id TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (story_id, key),
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_metadata_story ON story_metadata(story_id)",
                [],
            )?;
        }
        Ok(())
    }
}
