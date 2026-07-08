use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        88
    }

    fn description(&self) -> &'static str {
        "stories genre profile id"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let story_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(stories)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !story_cols.contains(&"genre_profile_id".to_string()) {
            conn.execute("ALTER TABLE stories ADD COLUMN genre_profile_id TEXT", [])?;
        }
        Ok(())
    }
}
