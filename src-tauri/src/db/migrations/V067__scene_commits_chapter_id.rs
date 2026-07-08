use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        67
    }

    fn description(&self) -> &'static str {
        "scene commits chapter id"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let scene_commit_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='scene_commits'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;
        let table_name = if scene_commit_exists {
            "scene_commits"
        } else {
            "chapter_commits"
        };

        let cc_columns_m68: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info({})", table_name))?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !cc_columns_m68.iter().any(|c| c == "chapter_id") {
            conn.execute(
                &format!(
                    "ALTER TABLE {} ADD COLUMN chapter_id TEXT REFERENCES chapters(id) ON DELETE \
                 SET NULL",
                    table_name
                ),
                [],
            )?;
            conn.execute(
                &format!(
                    "CREATE INDEX IF NOT EXISTS idx_{}_chapter ON {}(chapter_id)",
                    table_name.replace("_commits", "_commits"),
                    table_name
                ),
                [],
            )?;
        }
        Ok(())
    }
}
