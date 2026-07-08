use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        43
    }

    fn description(&self) -> &'static str {
        "scene character actions"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let scene_character_actions_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND \
             name='scene_character_actions'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if scene_character_actions_tables.is_empty() {
            conn.execute(
                "CREATE TABLE scene_character_actions (
                id TEXT PRIMARY KEY,
                scene_id TEXT NOT NULL,
                character_id TEXT NOT NULL,
                action_type TEXT,
                content TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_character_actions_scene ON \
             scene_character_actions(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_character_actions_character ON \
             scene_character_actions(character_id)",
                [],
            )?;
        }
        Ok(())
    }
}
