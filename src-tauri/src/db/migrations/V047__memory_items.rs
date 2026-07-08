use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        47
    }

    fn description(&self) -> &'static str {
        "memory items"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let memory_item_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='memory_items'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if memory_item_tables.is_empty() {
            conn.execute(
                "CREATE TABLE memory_items (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                category TEXT NOT NULL,
                subject TEXT,
                field TEXT,
                value TEXT,
                source_chapter INTEGER,
                confidence REAL NOT NULL DEFAULT 1.0,
                status TEXT NOT NULL DEFAULT 'active',
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_memory_items_story ON memory_items(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_memory_items_category ON memory_items(story_id, category)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_memory_items_status ON memory_items(story_id, status)",
                [],
            )?;
        }
        Ok(())
    }
}
