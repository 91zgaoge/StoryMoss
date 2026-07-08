use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        51
    }

    fn description(&self) -> &'static str {
        "review issues"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let review_issue_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='review_issues'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if review_issue_tables.is_empty() {
            conn.execute(
                "CREATE TABLE review_issues (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                scene_id TEXT,
                chapter_number INTEGER NOT NULL,
                severity TEXT NOT NULL,
                category TEXT NOT NULL,
                location TEXT,
                description TEXT NOT NULL,
                evidence TEXT,
                fix_hint TEXT,
                blocking INTEGER NOT NULL DEFAULT 0,
                resolved INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_review_issues_story ON review_issues(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_review_issues_severity ON review_issues(story_id, severity)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_review_issues_blocking ON review_issues(story_id, blocking)",
                [],
            )?;
        }
        Ok(())
    }
}
