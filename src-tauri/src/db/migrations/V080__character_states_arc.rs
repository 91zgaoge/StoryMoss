use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        80
    }

    fn description(&self) -> &'static str {
        "character states arc"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let cs_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(character_states)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !cs_cols.contains(&"state_transitions_json".to_string()) {
            conn.execute(
                "ALTER TABLE character_states ADD COLUMN state_transitions_json TEXT DEFAULT '[]'",
                [],
            )?;
        }
        if !cs_cols.contains(&"arc_type".to_string()) {
            conn.execute(
                "ALTER TABLE character_states ADD COLUMN arc_type TEXT DEFAULT 'positive'",
                [],
            )?;
        }
        Ok(())
    }
}
