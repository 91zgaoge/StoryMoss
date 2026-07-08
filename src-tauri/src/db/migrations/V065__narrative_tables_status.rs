use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        65
    }

    fn description(&self) -> &'static str {
        "narrative tables status"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        for table in [
            "narrative_characters",
            "narrative_scenes",
            "narrative_world_buildings",
        ] {
            let columns: Vec<String> = conn
                .prepare(&format!("PRAGMA table_info({})", table))?
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if !columns.iter().any(|c| c == "status") {
                conn.execute(
                    &format!(
                        "ALTER TABLE {} ADD COLUMN status TEXT NOT NULL DEFAULT 'active'",
                        table
                    ),
                    [],
                )?;
            }
        }
        Ok(())
    }
}
