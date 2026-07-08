use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        56
    }

    fn description(&self) -> &'static str {
        "blueprints"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let blueprint_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='blueprints'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if blueprint_tables.is_empty() {
            conn.execute(
                "CREATE TABLE blueprints (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                chapter_number INTEGER NOT NULL,
                title TEXT,
                role TEXT,
                purpose TEXT,
                key_events TEXT,
                characters TEXT,
                suspense_hook TEXT,
                user_guidance TEXT,
                notes TEXT,
                notes_updated_at TEXT,
                knowledge_query_hint TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                UNIQUE(story_id, chapter_number)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_blueprints_story ON blueprints(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_blueprints_chapter ON blueprints(story_id, chapter_number)",
                [],
            )?;
        }
        Ok(())
    }
}
