use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        52
    }

    fn description(&self) -> &'static str {
        "genre profiles"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let genre_profile_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='genre_profiles'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if genre_profile_tables.is_empty() {
            conn.execute(
                "CREATE TABLE genre_profiles (
                id TEXT PRIMARY KEY,
                genre_name TEXT NOT NULL UNIQUE,
                canonical_name TEXT NOT NULL,
                aliases_json TEXT,
                core_tone TEXT,
                pacing_strategy TEXT,
                anti_patterns_json TEXT,
                reference_tables_json TEXT,
                is_builtin INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_genre_profiles_canonical ON genre_profiles(canonical_name)",
                [],
            )?;
        }
        Ok(())
    }
}
