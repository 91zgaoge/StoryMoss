use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        32
    }

    fn description(&self) -> &'static str {
        "scene style blend override"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let scene_columns_m31: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_columns_m31
            .iter()
            .any(|c| c == "style_blend_override")
        {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN style_blend_override TEXT",
                [],
            )?;
        }
        Ok(())
    }
}
