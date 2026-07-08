use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        81
    }

    fn description(&self) -> &'static str {
        "story outlines analyzed structure"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let so_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(story_outlines)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !so_cols.contains(&"analyzed_structure_json".to_string()) {
            conn.execute(
                "ALTER TABLE story_outlines ADD COLUMN analyzed_structure_json TEXT",
                [],
            )?;
        }
        Ok(())
    }
}
