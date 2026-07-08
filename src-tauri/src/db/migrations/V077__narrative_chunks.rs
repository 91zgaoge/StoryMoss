use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        77
    }

    fn description(&self) -> &'static str {
        "narrative chunks"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let chunk_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_chunks'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if chunk_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_chunks (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL REFERENCES stories(id),
                chapter_range_start INTEGER NOT NULL,
                chapter_range_end INTEGER NOT NULL,
                scene_ids TEXT NOT NULL DEFAULT '[]',
                event_ids TEXT NOT NULL DEFAULT '[]',
                text TEXT NOT NULL,
                chunk_type TEXT NOT NULL,
                is_boundary_start INTEGER NOT NULL DEFAULT 0,
                is_boundary_end INTEGER NOT NULL DEFAULT 0,
                thread_ids TEXT NOT NULL DEFAULT '[]',
                vector_id TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_chunks_story ON narrative_chunks(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_chunks_type ON narrative_chunks(chunk_type)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_chunks_boundary ON \
             narrative_chunks(is_boundary_start, is_boundary_end)",
                [],
            )?;
        }
        Ok(())
    }
}
