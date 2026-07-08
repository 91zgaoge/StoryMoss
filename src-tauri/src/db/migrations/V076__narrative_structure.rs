use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        76
    }

    fn description(&self) -> &'static str {
        "narrative structure"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let structure_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_structure'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if structure_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_structure (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL REFERENCES stories(id),
                act_number INTEGER NOT NULL,
                act_type TEXT NOT NULL,
                start_chapter INTEGER NOT NULL,
                end_chapter INTEGER NOT NULL,
                summary TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_structure_story ON narrative_structure(story_id)",
                [],
            )?;
        }
        Ok(())
    }
}
