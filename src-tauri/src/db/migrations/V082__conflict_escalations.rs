use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        82
    }

    fn description(&self) -> &'static str {
        "conflict escalations"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let ce_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='conflict_escalations'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if ce_tables.is_empty() {
            conn.execute(
                "CREATE TABLE conflict_escalations (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL REFERENCES stories(id),
                conflict_type TEXT NOT NULL,
                party_a_ids TEXT NOT NULL DEFAULT '[]',
                party_b_ids TEXT NOT NULL DEFAULT '[]',
                intensity_timeline_json TEXT NOT NULL DEFAULT '[]',
                current_intensity REAL NOT NULL DEFAULT 0.0,
                is_escalated INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_conflict_escalations_story ON conflict_escalations(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_conflict_escalations_type ON conflict_escalations(conflict_type)",
                [],
            )?;
        }
        Ok(())
    }
}
