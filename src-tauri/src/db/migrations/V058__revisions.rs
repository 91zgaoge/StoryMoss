use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        58
    }

    fn description(&self) -> &'static str {
        "revisions"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let revision_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='revisions'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if revision_tables.is_empty() {
            conn.execute(
                "CREATE TABLE revisions (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                draft_id TEXT NOT NULL,
                revision_index INTEGER NOT NULL,
                revision_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                user_prompt TEXT,
                original_content TEXT NOT NULL,
                revised_content TEXT NOT NULL,
                word_count INTEGER NOT NULL DEFAULT 0,
                change_summary TEXT,
                model_used TEXT,
                cost REAL,
                metadata TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                FOREIGN KEY (draft_id) REFERENCES drafts(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_revisions_draft ON revisions(draft_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_revisions_story ON revisions(story_id)",
                [],
            )?;
        }
        Ok(())
    }
}
