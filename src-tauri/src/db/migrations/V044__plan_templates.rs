use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        44
    }

    fn description(&self) -> &'static str {
        "plan templates"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let plan_templates_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='plan_templates'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if plan_templates_tables.is_empty() {
            conn.execute(
                "CREATE TABLE plan_templates (
                id TEXT PRIMARY KEY,
                trigger_patterns TEXT NOT NULL,
                plan_json TEXT NOT NULL,
                success_count INTEGER NOT NULL DEFAULT 0,
                failure_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_plan_templates_patterns ON plan_templates(trigger_patterns)",
                [],
            )?;
        }

        // ==================== Story System 合同驱动体系 ====================
        Ok(())
    }
}
