use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        28
    }

    fn description(&self) -> &'static str {
        "scene structure fields"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let scene_columns_m25: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_columns_m25.iter().any(|c| c == "execution_stage") {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN execution_stage TEXT DEFAULT 'drafting'",
                [],
            )?;
        }
        if !scene_columns_m25.iter().any(|c| c == "outline_content") {
            conn.execute("ALTER TABLE scenes ADD COLUMN outline_content TEXT", [])?;
        }
        if !scene_columns_m25.iter().any(|c| c == "draft_content") {
            conn.execute("ALTER TABLE scenes ADD COLUMN draft_content TEXT", [])?;
        }
        Ok(())
    }
}
