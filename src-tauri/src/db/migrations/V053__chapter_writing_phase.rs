use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        53
    }

    fn description(&self) -> &'static str {
        "chapter writing phase"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let chapter_columns_m55: Vec<String> = conn
            .prepare("PRAGMA table_info(chapters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !chapter_columns_m55.iter().any(|c| c == "writing_phase") {
            conn.execute(
                "ALTER TABLE chapters ADD COLUMN writing_phase TEXT DEFAULT 'planning'",
                [],
            )?;
        }
        Ok(())
    }
}
