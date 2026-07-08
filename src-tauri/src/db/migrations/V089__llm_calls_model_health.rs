use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        89
    }

    fn description(&self) -> &'static str {
        "llm calls model health"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(llm_calls)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !columns.iter().any(|c| c == "task_type") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN task_type TEXT", [])?;
        }
        if !columns.iter().any(|c| c == "quality_score") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN quality_score REAL", [])?;
        }
        if !columns.iter().any(|c| c == "latency_ms") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN latency_ms INTEGER", [])?;
        }
        if !columns.iter().any(|c| c == "route_decision") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN route_decision TEXT", [])?;
        }
        if !columns.iter().any(|c| c == "audit_feedback") {
            conn.execute("ALTER TABLE llm_calls ADD COLUMN audit_feedback TEXT", [])?;
        }
        Ok(())
    }
}
