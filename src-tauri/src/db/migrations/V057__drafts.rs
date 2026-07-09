use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        57
    }

    fn description(&self) -> &'static str {
        "drafts"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let draft_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='drafts'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if draft_tables.is_empty() {
            conn.execute(
                "CREATE TABLE drafts (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                chapter_number INTEGER NOT NULL,
                scene_id TEXT,
                version INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL DEFAULT 'draft',
                source TEXT NOT NULL DEFAULT 'write',
                content TEXT NOT NULL DEFAULT '',
                word_count INTEGER NOT NULL DEFAULT 0,
                model_used TEXT,
                cost REAL,
                metadata TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                UNIQUE(story_id, chapter_number, version)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_drafts_story_chapter ON drafts(story_id, chapter_number)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_drafts_story_scene ON drafts(story_id, scene_id)",
                [],
            )?;
            conn.execute("CREATE INDEX idx_drafts_status ON drafts(status)", [])?;
            conn.execute(
                "CREATE INDEX idx_drafts_finalized ON drafts(story_id, chapter_number, status)",
                [],
            )?;
        }
        Ok(())
    }
}
