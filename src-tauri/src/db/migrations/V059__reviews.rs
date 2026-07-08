use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        59
    }

    fn description(&self) -> &'static str {
        "reviews"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let review_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='reviews'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if review_tables.is_empty() {
            conn.execute(
                "CREATE TABLE reviews (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                draft_id TEXT NOT NULL,
                review_index INTEGER NOT NULL,
                content TEXT NOT NULL,
                dimensions TEXT,
                issues TEXT,
                overall_score REAL,
                review_focus TEXT,
                model_used TEXT,
                cost REAL,
                metadata TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                FOREIGN KEY (draft_id) REFERENCES drafts(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute("CREATE INDEX idx_reviews_draft ON reviews(draft_id)", [])?;
            conn.execute("CREATE INDEX idx_reviews_story ON reviews(story_id)", [])?;
        }
        Ok(())
    }
}
