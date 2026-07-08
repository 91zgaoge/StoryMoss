use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        63
    }

    fn description(&self) -> &'static str {
        "ai usage quota offline grace"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        // 注：配额系统已移除 (A1)，此迁移保留以兼容旧数据库，但跳过无表的情况
        let quota_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='ai_usage_quota'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !quota_tables.is_empty() {
            let quota_v3_columns: Vec<String> = conn
                .prepare("PRAGMA table_info(ai_usage_quota)")?
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if !quota_v3_columns.iter().any(|c| c == "offline_grace_used") {
                conn.execute(
                    "ALTER TABLE ai_usage_quota ADD COLUMN offline_grace_used INTEGER NOT NULL \
                 DEFAULT 0",
                    [],
                )?;
            }
        }
        Ok(())
    }
}
