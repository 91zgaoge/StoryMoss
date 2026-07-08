use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        90
    }

    fn description(&self) -> &'static str {
        "text annotations metadata severity"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let ta_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(text_annotations)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !ta_columns.iter().any(|c| c == "metadata") {
            conn.execute("ALTER TABLE text_annotations ADD COLUMN metadata TEXT", [])?;
        }
        if !ta_columns.iter().any(|c| c == "severity") {
            conn.execute(
                "ALTER TABLE text_annotations ADD COLUMN severity TEXT NOT NULL DEFAULT 'medium'",
                [],
            )?;
        }
        Ok(())
    }
}
