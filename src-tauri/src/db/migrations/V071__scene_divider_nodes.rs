use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        71
    }

    fn description(&self) -> &'static str {
        "scene divider nodes"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let divider_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='scene_divider_nodes'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if divider_tables.is_empty() {
            conn.execute(
                "CREATE TABLE scene_divider_nodes (
                id TEXT PRIMARY KEY,
                chapter_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                scene_id TEXT NOT NULL,
                label TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE,
                FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                UNIQUE(chapter_id, position)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_divider_chapter ON scene_divider_nodes(chapter_id)",
                [],
            )?;
        }
        Ok(())
    }
}
