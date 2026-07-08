use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        46
    }

    fn description(&self) -> &'static str {
        "scene commits"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let scene_commit_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='scene_commits'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if scene_commit_tables.is_empty() {
            conn.execute(
                "CREATE TABLE scene_commits (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                scene_id TEXT,
                chapter_id TEXT REFERENCES chapters(id) ON DELETE SET NULL,
                chapter_number INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                outline_snapshot_json TEXT,
                review_result_json TEXT,
                fulfillment_result_json TEXT,
                accepted_events_json TEXT,
                state_deltas_json TEXT,
                entity_deltas_json TEXT,
                summary_text TEXT,
                dominant_strand TEXT,
                projection_status_json TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
                FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE SET NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_commits_story ON scene_commits(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_scene_commits_scene ON scene_commits(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE UNIQUE INDEX idx_scene_commits_number ON scene_commits(story_id, \
             chapter_number)",
                [],
            )?;
        }
        Ok(())
    }
}
