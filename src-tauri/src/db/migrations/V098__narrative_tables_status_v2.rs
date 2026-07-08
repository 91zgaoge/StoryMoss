use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        98
    }

    fn description(&self) -> &'static str {
        "narrative tables status v2"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        for table in [
            "narrative_characters",
            "narrative_scenes",
            "narrative_world_buildings",
        ] {
            let cols: Vec<String> = conn
                .prepare(&format!("PRAGMA table_info({})", table))?
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if !cols.contains(&"status".to_string()) {
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
