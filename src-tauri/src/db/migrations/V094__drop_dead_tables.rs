use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        94
    }

    fn description(&self) -> &'static str {
        "drop dead tables"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "DROP TABLE IF EXISTS beat_cards;
         DROP TABLE IF EXISTS story_engines;
         DROP TABLE IF EXISTS pressure_relationships;",
        )?;
        Ok(())
    }
}
