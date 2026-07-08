use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        55
    }

    fn description(&self) -> &'static str {
        "feature usage logs"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let feature_usage_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='feature_usage_logs'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if feature_usage_tables.is_empty() {
            conn.execute(
                "CREATE TABLE feature_usage_logs (
                id TEXT PRIMARY KEY,
                feature_id TEXT NOT NULL,
                action TEXT NOT NULL,
                story_id TEXT,
                metadata TEXT,
                created_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_feature_usage_feature ON feature_usage_logs(feature_id, \
             created_at)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_feature_usage_story ON feature_usage_logs(story_id, created_at)",
                [],
            )?;
        }

        // ==================== Pipeline 管线体系（基于 Vela
        // 学习借鉴）====================
        Ok(())
    }
}
