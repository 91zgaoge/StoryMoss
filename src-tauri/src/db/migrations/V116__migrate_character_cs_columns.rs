use rusqlite::Connection;

use crate::db::migrations::RustMigration;

/// Backfill `characters.cs_*` dynamic state columns into `character_states`,
/// then drop the redundant columns from `characters`.
pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        116
    }

    fn description(&self) -> &'static str {
        "migrate character cs columns to character_states"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let tx = conn.transaction()?;

        // 0. Ensure `character_states` table exists (defensive: some upgrade
        //    paths may reach V116 without V014 having run).
        tx.execute(
            "CREATE TABLE IF NOT EXISTS character_states (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                character_id TEXT NOT NULL,
                current_location TEXT,
                current_emotion TEXT,
                active_goal TEXT,
                secrets_known TEXT,
                secrets_unknown TEXT,
                arc_progress REAL,
                last_updated TEXT,
                FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_character_states_story ON character_states(story_id)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_character_states_character ON character_states(character_id)",
            [],
        )?;

        // 1. Ensure `character_states` has columns that mirror the legacy
        //    `cs_*` fields.
        let cs_cols: Vec<String> = tx
            .prepare("PRAGMA table_info(character_states)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let desired_cs_columns = [
            ("location", "TEXT"),
            ("power_level", "TEXT"),
            ("physical_state", "TEXT"),
            ("mental_state", "TEXT"),
            ("key_items", "TEXT"),
            ("recent_events", "TEXT"),
            ("updated_at_chapter", "INTEGER"),
            ("cs_json", "TEXT"),
        ];
        for (col, typ) in &desired_cs_columns {
            if !cs_cols.iter().any(|c| c == *col) {
                tx.execute(
                    &format!("ALTER TABLE character_states ADD COLUMN {} {}", col, typ),
                    [],
                )?;
            }
        }

        // 2. Migrate data if the legacy columns still exist on `characters`.
        let char_cols: Vec<String> = tx
            .prepare("PRAGMA table_info(characters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if char_cols.iter().any(|c| c == "cs_location") {
            // 2a. Create `character_states` rows for characters that do not
            // have one yet.
            let mut stmt = tx.prepare(
                "SELECT c.id, c.story_id, c.cs_location, c.cs_power_level, c.cs_physical_state, \
                 c.cs_mental_state, c.cs_key_items, c.cs_recent_events, \
                 c.cs_updated_at_chapter, c.cs_json \
                 FROM characters c \
                 LEFT JOIN character_states s ON s.character_id = c.id \
                 WHERE s.id IS NULL",
            )?;
            let rows: Vec<(
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<i32>,
                Option<String>,
            )> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2).ok(),
                        row.get(3).ok(),
                        row.get(4).ok(),
                        row.get(5).ok(),
                        row.get(6).ok(),
                        row.get(7).ok(),
                        row.get(8).ok(),
                        row.get(9).ok(),
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            drop(stmt);

            for (
                character_id,
                story_id,
                cs_location,
                cs_power_level,
                cs_physical_state,
                cs_mental_state,
                cs_key_items,
                cs_recent_events,
                cs_updated_at_chapter,
                cs_json,
            ) in rows
            {
                tx.execute(
                    "INSERT INTO character_states (
                        id, story_id, character_id, location, power_level, physical_state,
                        mental_state, key_items, recent_events, updated_at_chapter, cs_json,
                        last_updated
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        story_id,
                        character_id,
                        cs_location,
                        cs_power_level,
                        cs_physical_state,
                        cs_mental_state,
                        cs_key_items,
                        cs_recent_events,
                        cs_updated_at_chapter,
                        cs_json,
                        chrono::Local::now().to_rfc3339(),
                    ],
                )?;
            }

            // 2b. For characters that already have a `character_states` row,
            // backfill     only empty legacy columns so we do not
            // clobber newer canonical data.
            tx.execute(
                "UPDATE character_states SET
                    location = COALESCE(location, (SELECT cs_location FROM characters WHERE id = character_states.character_id)),
                    power_level = COALESCE(power_level, (SELECT cs_power_level FROM characters WHERE id = character_states.character_id)),
                    physical_state = COALESCE(physical_state, (SELECT cs_physical_state FROM characters WHERE id = character_states.character_id)),
                    mental_state = COALESCE(mental_state, (SELECT cs_mental_state FROM characters WHERE id = character_states.character_id)),
                    key_items = COALESCE(key_items, (SELECT cs_key_items FROM characters WHERE id = character_states.character_id)),
                    recent_events = COALESCE(recent_events, (SELECT cs_recent_events FROM characters WHERE id = character_states.character_id)),
                    updated_at_chapter = COALESCE(updated_at_chapter, (SELECT cs_updated_at_chapter FROM characters WHERE id = character_states.character_id)),
                    cs_json = COALESCE(cs_json, (SELECT cs_json FROM characters WHERE id = character_states.character_id))
                 WHERE character_id IN (SELECT id FROM characters)",
                [],
            )?;
        }

        // 3. Drop the redundant `cs_*` columns from `characters`.
        for col in [
            "cs_location",
            "cs_power_level",
            "cs_physical_state",
            "cs_mental_state",
            "cs_key_items",
            "cs_recent_events",
            "cs_updated_at_chapter",
            "cs_json",
        ] {
            if char_cols.iter().any(|c| c == col) {
                tx.execute(&format!("ALTER TABLE characters DROP COLUMN {}", col), [])?;
            }
        }

        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_columns(conn: &Connection, table: &str) -> Vec<String> {
        conn.prepare(&format!("PRAGMA table_info({})", table))
            .unwrap()
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn test_v116_migrates_cs_columns_to_character_states() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Setup legacy schema
        conn.execute_batch(
            "CREATE TABLE characters (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                name TEXT NOT NULL,
                cs_location TEXT,
                cs_power_level TEXT,
                cs_physical_state TEXT,
                cs_mental_state TEXT,
                cs_key_items TEXT,
                cs_recent_events TEXT,
                cs_updated_at_chapter INTEGER,
                cs_json TEXT
            );
            CREATE TABLE character_states (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                character_id TEXT NOT NULL,
                current_location TEXT,
                current_emotion TEXT,
                active_goal TEXT,
                secrets_known TEXT,
                secrets_unknown TEXT,
                arc_progress REAL,
                last_updated TEXT
            );",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO characters (id, story_id, name, cs_location, cs_power_level, \
             cs_physical_state, cs_mental_state, cs_key_items, cs_recent_events, \
             cs_updated_at_chapter, cs_json)
             VALUES ('char1', 'story1', 'Hero', 'Beijing', 'S-class', 'injured', \
             'angry', 'sword', 'escaped', 5, '{\"foo\":\"bar\"}')",
            [],
        )
        .unwrap();

        // Existing character_states row with some canonical data
        conn.execute(
            "INSERT INTO character_states (id, story_id, character_id, current_location, \
             current_emotion, active_goal, secrets_known, secrets_unknown, arc_progress, last_updated)
             VALUES ('cs1', 'story1', 'char1', 'Shanghai', 'calm', 'revenge', '[]', '[]', 0.5, '2024-01-01')",
            [],
        )
        .unwrap();

        Migration.apply(&mut conn).unwrap();

        // cs_* columns removed from characters
        let char_cols = table_columns(&conn, "characters");
        for col in [
            "cs_location",
            "cs_power_level",
            "cs_physical_state",
            "cs_mental_state",
            "cs_key_items",
            "cs_recent_events",
            "cs_updated_at_chapter",
            "cs_json",
        ] {
            assert!(
                !char_cols.iter().any(|c| c == col),
                "characters 不应再包含 {}",
                col
            );
        }

        // New columns added to character_states
        let cs_cols = table_columns(&conn, "character_states");
        for col in [
            "location",
            "power_level",
            "physical_state",
            "mental_state",
            "key_items",
            "recent_events",
            "updated_at_chapter",
            "cs_json",
        ] {
            assert!(
                cs_cols.iter().any(|c| c == col),
                "character_states 应包含 {}",
                col
            );
        }

        // Existing canonical columns should not be overwritten
        let (current_location, current_emotion, active_goal): (String, String, String) = conn
            .query_row(
                "SELECT current_location, current_emotion, active_goal FROM character_states WHERE character_id = 'char1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(current_location, "Shanghai");
        assert_eq!(current_emotion, "calm");
        assert_eq!(active_goal, "revenge");

        // Legacy data migrated into new columns
        let (location, power_level, physical_state, mental_state, key_items, recent_events, updated_at_chapter, cs_json): (
            String,
            String,
            String,
            String,
            String,
            String,
            i32,
            String,
        ) = conn
            .query_row(
                "SELECT location, power_level, physical_state, mental_state, key_items, \
                 recent_events, updated_at_chapter, cs_json FROM character_states WHERE character_id = 'char1'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(location, "Beijing");
        assert_eq!(power_level, "S-class");
        assert_eq!(physical_state, "injured");
        assert_eq!(mental_state, "angry");
        assert_eq!(key_items, "sword");
        assert_eq!(recent_events, "escaped");
        assert_eq!(updated_at_chapter, 5);
        assert_eq!(cs_json, r#"{"foo":"bar"}"#);
    }

    #[test]
    fn test_v116_is_idempotent_when_columns_already_dropped() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE characters (id TEXT PRIMARY KEY, story_id TEXT NOT NULL, name TEXT NOT NULL);
             CREATE TABLE character_states (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                character_id TEXT NOT NULL,
                current_location TEXT,
                location TEXT
             );",
        )
        .unwrap();

        // Should not error even though cs_* columns are gone.
        Migration.apply(&mut conn).unwrap();

        let cs_cols = table_columns(&conn, "character_states");
        assert!(cs_cols.iter().any(|c| c == "location"));
    }
}
