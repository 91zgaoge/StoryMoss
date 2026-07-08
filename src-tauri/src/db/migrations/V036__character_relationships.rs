use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        36
    }

    fn description(&self) -> &'static str {
        "character relationships"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let char_columns_m35: Vec<String> = conn
            .prepare("PRAGMA table_info(characters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !char_columns_m35.iter().any(|c| c == "appearance") {
            conn.execute("ALTER TABLE characters ADD COLUMN appearance TEXT", [])?;
        }
        if !char_columns_m35.iter().any(|c| c == "gender") {
            conn.execute("ALTER TABLE characters ADD COLUMN gender TEXT", [])?;
        }
        if !char_columns_m35.iter().any(|c| c == "age") {
            conn.execute("ALTER TABLE characters ADD COLUMN age INTEGER", [])?;
        }

        let rel_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND \
             name='character_relationships'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if rel_tables.is_empty() {
            conn.execute(
                "CREATE TABLE character_relationships (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                source_character_id TEXT NOT NULL,
                target_character_id TEXT NOT NULL,
                relationship_type TEXT NOT NULL,
                description TEXT,
                dynamic TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (source_character_id) REFERENCES characters(id) ON DELETE CASCADE,
                FOREIGN KEY (target_character_id) REFERENCES characters(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_char_rel_story ON character_relationships(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_char_rel_source ON character_relationships(source_character_id)",
                [],
            )?;
        }
        Ok(())
    }
}
