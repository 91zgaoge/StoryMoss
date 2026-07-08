use rusqlite::Connection;

use crate::db::migrations::RustMigration;

/// v0.26.30 hotfix: 兜底修复 inline migration → Rust migration 切换过程中可能
/// 跳过 V099，导致 characters/scenes/world_buildings/kg_entities 表缺失
/// source / is_auto_generated 列。
pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        103
    }

    fn description(&self) -> &'static str {
        "ensure source and auto generated columns"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        for table in ["characters", "scenes", "world_buildings", "kg_entities"] {
            let cols: Vec<String> = conn
                .prepare(&format!("PRAGMA table_info({})", table))?
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if !cols.contains(&"source".to_string()) {
                conn.execute(
                    &format!(
                        "ALTER TABLE {} ADD COLUMN source TEXT DEFAULT 'user_created'",
                        table
                    ),
                    [],
                )?;
            }
            if !cols.contains(&"is_auto_generated".to_string()) {
                conn.execute(
                    &format!(
                        "ALTER TABLE {} ADD COLUMN is_auto_generated INTEGER NOT NULL DEFAULT 0",
                        table
                    ),
                    [],
                )?;
            }
        }
        Ok(())
    }
}
