use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        85
    }

    fn description(&self) -> &'static str {
        "reference scenes litseg fields"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let rs_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(reference_scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !rs_cols.contains(&"narrative_intensity".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN narrative_intensity REAL",
                [],
            )?;
        }
        if !rs_cols.contains(&"narrative_sentiment".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN narrative_sentiment REAL",
                [],
            )?;
        }
        if !rs_cols.contains(&"narrative_event_types".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN narrative_event_types TEXT DEFAULT '[]'",
                [],
            )?;
        }
        if !rs_cols.contains(&"act_number".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN act_number INTEGER DEFAULT 1",
                [],
            )?;
        }
        if !rs_cols.contains(&"position_in_act".to_string()) {
            conn.execute(
                "ALTER TABLE reference_scenes ADD COLUMN position_in_act REAL DEFAULT 0.0",
                [],
            )?;
        }
        Ok(())
    }
}
