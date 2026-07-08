use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        61
    }

    fn description(&self) -> &'static str {
        "character dynamic state fields"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let character_columns_m63: Vec<String> = conn
            .prepare("PRAGMA table_info(characters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !character_columns_m63.iter().any(|c| c == "cs_location") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_location TEXT", [])?;
        }
        if !character_columns_m63.iter().any(|c| c == "cs_power_level") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_power_level TEXT", [])?;
        }
        if !character_columns_m63
            .iter()
            .any(|c| c == "cs_physical_state")
        {
            conn.execute(
                "ALTER TABLE characters ADD COLUMN cs_physical_state TEXT",
                [],
            )?;
        }
        if !character_columns_m63.iter().any(|c| c == "cs_mental_state") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_mental_state TEXT", [])?;
        }
        if !character_columns_m63.iter().any(|c| c == "cs_key_items") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_key_items TEXT", [])?;
        }
        if !character_columns_m63
            .iter()
            .any(|c| c == "cs_recent_events")
        {
            conn.execute(
                "ALTER TABLE characters ADD COLUMN cs_recent_events TEXT",
                [],
            )?;
        }
        if !character_columns_m63
            .iter()
            .any(|c| c == "cs_updated_at_chapter")
        {
            conn.execute(
                "ALTER TABLE characters ADD COLUMN cs_updated_at_chapter INTEGER",
                [],
            )?;
        }
        if !character_columns_m63.iter().any(|c| c == "cs_json") {
            conn.execute("ALTER TABLE characters ADD COLUMN cs_json TEXT", [])?;
        }
        Ok(())
    }
}
