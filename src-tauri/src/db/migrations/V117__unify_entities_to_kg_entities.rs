use rusqlite::Connection;

use crate::db::migrations::RustMigration;

/// Unify entity storage so that `kg_entities` is the single source of truth for
/// characters.
///
/// Up:
///   1. Copy every row from `characters` into `kg_entities` with `entity_type =
///      'Character'`, preserving the original `id` so that foreign-key
///      consumers continue to work.
///   2. Map legacy character columns (`background`, `personality`, `goals`,
///      `appearance`, `gender`, `age`, `dynamic_traits`) into the JSON
///      `attributes` object.
///   3. Backfill `memory_items.kg_entity_id` for rows whose `subject` matches a
///      migrated character `name` within the same story.
///   4. Create a read-only compatibility view `v_characters` that exposes the
///      old `characters` column layout by reading from `kg_entities`.
///
/// Down:
///   1. Drop the `v_characters` view.
///   2. Remove the character rows that were copied into `kg_entities` so the
///      database returns to the pre-migration state.  (The legacy `characters`
///      table is intentionally left untouched by `up`, so no data restoration
///      is required there.)
pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        117
    }

    fn description(&self) -> &'static str {
        "unify entities to kg_entities"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let tx = conn.transaction()?;

        // Ensure required tables exist (defensive for partial upgrade paths).
        tx.execute(
            "CREATE TABLE IF NOT EXISTS kg_entities (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                name TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                attributes TEXT,
                embedding BLOB,
                first_seen TEXT NOT NULL,
                last_updated TEXT NOT NULL,
                confidence_score REAL,
                access_count INTEGER DEFAULT 0,
                last_accessed TEXT,
                is_archived INTEGER DEFAULT 0,
                archived_at TEXT,
                source TEXT DEFAULT 'user_created',
                is_auto_generated INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_kg_entities_story ON kg_entities(story_id)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_kg_entities_type ON kg_entities(entity_type)",
            [],
        )?;

        // Only migrate if the legacy `characters` table still exists.  If a
        // previous run already dropped it, there is nothing to copy.
        let char_table_exists: bool = tx
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='characters'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if char_table_exists {
            // Discover the actual columns on `characters` because the set of
            // columns has changed across earlier migrations.
            let mut stmt = tx.prepare(
                "SELECT
                    id,
                    story_id,
                    name,
                    background,
                    personality,
                    goals,
                    appearance,
                    gender,
                    age,
                    dynamic_traits,
                    source,
                    is_auto_generated,
                    created_at,
                    updated_at
                 FROM characters",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(legacy_row(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3).ok(),
                    row.get(4).ok(),
                    row.get(5).ok(),
                    row.get(6).ok(),
                    row.get(7).ok(),
                    row.get(8).ok(),
                    row.get(9).ok(),
                    row.get(10).ok(),
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                ))
            })?;

            for row in rows {
                let row = row?;
                let attributes = build_attributes(&row);
                let source = row.source.as_deref().unwrap_or("user_created");
                let is_auto_generated = row.is_auto_generated.unwrap_or(false) as i32;

                tx.execute(
                    "INSERT OR IGNORE INTO kg_entities (
                        id, story_id, name, entity_type, attributes,
                        first_seen, last_updated, access_count, is_archived,
                        source, is_auto_generated
                    ) VALUES (?1, ?2, ?3, 'Character', ?4, ?5, ?6, 0, 0, ?7, ?8)",
                    rusqlite::params![
                        &row.id,
                        &row.story_id,
                        &row.name,
                        attributes.to_string(),
                        &row.created_at,
                        &row.updated_at,
                        source,
                        is_auto_generated,
                    ],
                )?;
            }
            drop(stmt);

            // Backfill memory_items.kg_entity_id by subject-name match within
            // the same story.
            tx.execute(
                "UPDATE memory_items
                 SET kg_entity_id = (
                     SELECT e.id
                     FROM kg_entities e
                     WHERE e.story_id = memory_items.story_id
                       AND e.name = memory_items.subject
                       AND e.entity_type = 'Character'
                       AND e.is_archived = 0
                     LIMIT 1
                 )
                 WHERE kg_entity_id IS NULL
                   AND subject IS NOT NULL
                   AND category IN ('entity', 'character_state')",
                [],
            )?;
        }

        // Create (or recreate) the compatibility view.
        tx.execute("DROP VIEW IF EXISTS v_characters", [])?;
        tx.execute(
            "CREATE VIEW v_characters AS
             SELECT
                 id,
                 story_id,
                 name,
                 json_extract(attributes, '$.background') AS background,
                 json_extract(attributes, '$.personality') AS personality,
                 json_extract(attributes, '$.goals') AS goals,
                 json_extract(attributes, '$.appearance') AS appearance,
                 json_extract(attributes, '$.gender') AS gender,
                 json_extract(attributes, '$.age') AS age,
                 json_extract(attributes, '$.dynamic_traits') AS dynamic_traits,
                 source,
                 is_auto_generated,
                 first_seen AS created_at,
                 last_updated AS updated_at
             FROM kg_entities
             WHERE entity_type = 'Character' AND is_archived = 0",
            [],
        )?;

        tx.commit()?;
        Ok(())
    }
}

impl Migration {
    /// Reverses the migration: drops the compatibility view and removes the
    /// character rows that were copied into `kg_entities`.
    ///
    /// Note: the legacy `characters` table is left untouched by both `apply`
    /// and `down`, so callers that still read from `characters` continue to
    /// work after rollback.
    pub fn down(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let tx = conn.transaction()?;
        tx.execute("DROP VIEW IF EXISTS v_characters", [])?;
        tx.execute(
            "DELETE FROM kg_entities
             WHERE entity_type = 'Character'
               AND id IN (SELECT id FROM characters)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }
}

struct LegacyCharacterRow {
    id: String,
    story_id: String,
    name: String,
    background: Option<String>,
    personality: Option<String>,
    goals: Option<String>,
    appearance: Option<String>,
    gender: Option<String>,
    age: Option<i32>,
    dynamic_traits: Option<String>,
    source: Option<String>,
    is_auto_generated: Option<bool>,
    created_at: String,
    updated_at: String,
}

fn legacy_row(
    id: String,
    story_id: String,
    name: String,
    background: Option<String>,
    personality: Option<String>,
    goals: Option<String>,
    appearance: Option<String>,
    gender: Option<String>,
    age: Option<i32>,
    dynamic_traits: Option<String>,
    source: Option<String>,
    is_auto_generated: Option<i32>,
    created_at: String,
    updated_at: String,
) -> LegacyCharacterRow {
    LegacyCharacterRow {
        id,
        story_id,
        name,
        background,
        personality,
        goals,
        appearance,
        gender,
        age,
        dynamic_traits,
        source,
        is_auto_generated: is_auto_generated.map(|v| v != 0),
        created_at,
        updated_at,
    }
}

fn build_attributes(row: &LegacyCharacterRow) -> serde_json::Value {
    use serde_json::json;

    let dynamic_traits: serde_json::Value = row
        .dynamic_traits
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| json!([]));

    let mut attrs = serde_json::Map::new();
    attrs.insert("background".to_string(), json!(row.background));
    attrs.insert("personality".to_string(), json!(row.personality));
    attrs.insert("goals".to_string(), json!(row.goals));
    attrs.insert("appearance".to_string(), json!(row.appearance));
    attrs.insert("gender".to_string(), json!(row.gender));
    attrs.insert("age".to_string(), json!(row.age));
    attrs.insert("dynamic_traits".to_string(), dynamic_traits);
    serde_json::Value::Object(attrs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_schema(conn: &mut Connection) {
        conn.execute_batch(
            "CREATE TABLE stories (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE characters (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                name TEXT NOT NULL,
                background TEXT,
                personality TEXT,
                goals TEXT,
                appearance TEXT,
                gender TEXT,
                age INTEGER,
                dynamic_traits TEXT,
                source TEXT DEFAULT 'user_created',
                is_auto_generated INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE kg_entities (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                name TEXT NOT NULL,
                entity_type TEXT NOT NULL,
                attributes TEXT,
                embedding BLOB,
                first_seen TEXT NOT NULL,
                last_updated TEXT NOT NULL,
                confidence_score REAL,
                access_count INTEGER DEFAULT 0,
                last_accessed TEXT,
                is_archived INTEGER DEFAULT 0,
                archived_at TEXT,
                source TEXT DEFAULT 'user_created',
                is_auto_generated INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE memory_items (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                category TEXT NOT NULL,
                subject TEXT,
                field TEXT,
                value TEXT,
                source_chapter INTEGER,
                confidence REAL NOT NULL DEFAULT 1.0,
                status TEXT NOT NULL DEFAULT 'active',
                updated_at TEXT NOT NULL,
                kg_entity_id TEXT
            );
            INSERT INTO stories (id, title, created_at, updated_at)
            VALUES ('story1', 'Test Story', '2024-01-01T00:00:00+08:00', '2024-01-01T00:00:00+08:00');
            INSERT INTO characters (id, story_id, name, background, personality, goals,
                appearance, gender, age, dynamic_traits, source, is_auto_generated,
                created_at, updated_at)
            VALUES ('char1', 'story1', 'Alice',
                'A traveler', 'Brave', 'Find the stone',
                'Tall', 'female', 28,
                '[{\"trait\":\"brave\",\"confidence\":0.9}]',
                'user_created', 0,
                '2024-01-01T00:00:00+08:00', '2024-01-02T00:00:00+08:00');
            INSERT INTO memory_items (id, story_id, category, subject, value, updated_at)
            VALUES ('mi1', 'story1', 'character_state', 'Alice', 'healthy', '2024-01-01T00:00:00+08:00');",
        )
        .unwrap();
    }

    #[test]
    fn test_v117_migrates_characters_to_kg_entities_and_backfills_memory() {
        let mut conn = Connection::open_in_memory().unwrap();
        setup_schema(&mut conn);

        Migration.apply(&mut conn).unwrap();

        // Character copied to kg_entities with the same id.
        let (name, entity_type, attrs_json): (String, String, String) = conn
            .query_row(
                "SELECT name, entity_type, attributes FROM kg_entities WHERE id = 'char1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "Alice");
        assert_eq!(entity_type, "Character");
        let attrs: serde_json::Value = serde_json::from_str(&attrs_json).unwrap();
        assert_eq!(attrs["background"].as_str(), Some("A traveler"));
        assert_eq!(attrs["personality"].as_str(), Some("Brave"));
        assert_eq!(attrs["goals"].as_str(), Some("Find the stone"));
        assert_eq!(attrs["appearance"].as_str(), Some("Tall"));
        assert_eq!(attrs["gender"].as_str(), Some("female"));
        assert_eq!(attrs["age"].as_i64(), Some(28));
        assert!(attrs["dynamic_traits"].is_array());

        // Compatibility view exposes old column layout.
        let (view_name, view_age, view_traits): (String, i32, String) = conn
            .query_row(
                "SELECT name, age, dynamic_traits FROM v_characters WHERE id = 'char1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(view_name, "Alice");
        assert_eq!(view_age, 28);
        let traits: serde_json::Value = serde_json::from_str(&view_traits).unwrap();
        assert_eq!(traits[0]["trait"].as_str(), Some("brave"));

        // memory_items.kg_entity_id backfilled by subject name match.
        let kg_entity_id: Option<String> = conn
            .query_row(
                "SELECT kg_entity_id FROM memory_items WHERE id = 'mi1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(kg_entity_id.as_deref(), Some("char1"));
    }

    #[test]
    fn test_v117_down_is_reversible() {
        let mut conn = Connection::open_in_memory().unwrap();
        setup_schema(&mut conn);

        Migration.apply(&mut conn).unwrap();
        assert!(
            conn.query_row(
                "SELECT 1 FROM sqlite_master WHERE type='view' AND name='v_characters'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false),
            "v_characters should exist after apply"
        );

        Migration.down(&mut conn).unwrap();

        let view_exists: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='view' AND name='v_characters'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(!view_exists, "v_characters should be dropped after down");

        let kg_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_entities WHERE entity_type = 'Character'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            kg_count, 0,
            "character rows should be removed from kg_entities after down"
        );

        // Legacy table data is preserved.
        let char_name: String = conn
            .query_row(
                "SELECT name FROM characters WHERE id = 'char1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(char_name, "Alice");
    }
}
