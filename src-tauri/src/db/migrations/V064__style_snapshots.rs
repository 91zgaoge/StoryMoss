use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        64
    }

    fn description(&self) -> &'static str {
        "style snapshots"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let snapshot_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='style_snapshots'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if snapshot_tables.is_empty() {
            conn.execute(
                "CREATE TABLE style_snapshots (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                chapter_number INTEGER,
                scene_number INTEGER,
                sentence_length REAL NOT NULL,
                dialogue_ratio REAL NOT NULL,
                metaphor_density REAL NOT NULL,
                inner_monologue_ratio REAL NOT NULL,
                emotion_density REAL NOT NULL,
                rhythm_score REAL NOT NULL,
                computed_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_style_snapshots_story ON style_snapshots(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_style_snapshots_story_chapter ON style_snapshots(story_id, \
             chapter_number)",
                [],
            )?;
        }
        Ok(())
    }
}
