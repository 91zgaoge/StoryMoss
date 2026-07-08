use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        79
    }

    fn description(&self) -> &'static str {
        "foreshadowing tracker events"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let fs_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(foreshadowing_tracker)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !fs_cols.contains(&"setup_event_id".to_string()) {
            conn.execute(
                "ALTER TABLE foreshadowing_tracker ADD COLUMN setup_event_id TEXT",
                [],
            )?;
        }
        if !fs_cols.contains(&"payoff_event_id".to_string()) {
            conn.execute(
                "ALTER TABLE foreshadowing_tracker ADD COLUMN payoff_event_id TEXT",
                [],
            )?;
        }
        if !fs_cols.contains(&"risk_signals_score".to_string()) {
            conn.execute(
                "ALTER TABLE foreshadowing_tracker ADD COLUMN risk_signals_score REAL DEFAULT 0.0",
                [],
            )?;
        }
        Ok(())
    }
}
