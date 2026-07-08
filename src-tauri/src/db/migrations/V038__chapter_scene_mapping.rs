use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        38
    }

    fn description(&self) -> &'static str {
        "chapter scene mapping"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let chapter_columns_m37: Vec<String> = conn
            .prepare("PRAGMA table_info(chapters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !chapter_columns_m37.iter().any(|c| c == "scene_id") {
            conn.execute(
                "ALTER TABLE chapters ADD COLUMN scene_id TEXT REFERENCES scenes(id) ON DELETE \
             SET NULL",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_chapters_scene ON chapters(scene_id)",
                [],
            )?;
        }

        let scene_columns_m37: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_columns_m37.iter().any(|c| c == "chapter_id") {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN chapter_id TEXT REFERENCES chapters(id) ON DELETE \
             SET NULL",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_scenes_chapter ON scenes(chapter_id)",
                [],
            )?;
        }
        Ok(())
    }
}
