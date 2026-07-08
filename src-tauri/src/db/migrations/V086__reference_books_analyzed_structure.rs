use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        86
    }

    fn description(&self) -> &'static str {
        "reference books analyzed structure"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let rb_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(reference_books)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !rb_cols.contains(&"analyzed_structure_json".to_string()) {
            conn.execute(
                "ALTER TABLE reference_books ADD COLUMN analyzed_structure_json TEXT",
                [],
            )?;
        }
        Ok(())
    }
}
