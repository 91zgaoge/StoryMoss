use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        42
    }

    fn description(&self) -> &'static str {
        "scene characters"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let scene_characters_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='scene_characters'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if scene_characters_tables.is_empty() {
            conn.execute(
                "CREATE TABLE scene_characters (
                id TEXT PRIMARY KEY,
                scene_id TEXT NOT NULL,
                character_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE,
                UNIQUE(scene_id, character_id)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_characters_scene ON scene_characters(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_characters_character ON scene_characters(character_id)",
                [],
            )?;
        }
        Ok(())
    }
}
