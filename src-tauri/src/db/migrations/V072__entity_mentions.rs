use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        72
    }

    fn description(&self) -> &'static str {
        "entity mentions"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let mention_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='entity_mentions'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if mention_tables.is_empty() {
            conn.execute(
                "CREATE TABLE entity_mentions (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                scene_id TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                start_pos INTEGER NOT NULL,
                end_pos INTEGER NOT NULL,
                mention_text TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 1.0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                FOREIGN KEY (entity_id) REFERENCES kg_entities(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_mentions_entity ON entity_mentions(entity_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_mentions_scene ON entity_mentions(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_mentions_story ON entity_mentions(story_id)",
                [],
            )?;
        }
        Ok(())
    }
}
