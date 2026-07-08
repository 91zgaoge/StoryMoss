use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        87
    }

    fn description(&self) -> &'static str {
        "genre profiles typical structure"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let genre_profile_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(genre_profiles)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !genre_profile_cols.contains(&"typical_structure_json".to_string()) {
            conn.execute(
                "ALTER TABLE genre_profiles ADD COLUMN typical_structure_json TEXT",
                [],
            )?;
        }
        Ok(())
    }
}
