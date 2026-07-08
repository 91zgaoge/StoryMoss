use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        37
    }

    fn description(&self) -> &'static str {
        "scene foreshadowing ids"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let scene_columns_m36: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_columns_m36.iter().any(|c| c == "foreshadowing_ids") {
            conn.execute("ALTER TABLE scenes ADD COLUMN foreshadowing_ids TEXT", [])?;
        }
        Ok(())
    }
}
