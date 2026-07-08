use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        73
    }

    fn description(&self) -> &'static str {
        "narrative events"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let event_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_events'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if event_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_events (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL REFERENCES stories(id),
                chapter_number INTEGER NOT NULL,
                scene_id TEXT,
                event_type TEXT NOT NULL,
                intensity REAL NOT NULL DEFAULT 0.5,
                sentiment REAL NOT NULL DEFAULT 0.0,
                description TEXT NOT NULL,
                involved_character_ids TEXT NOT NULL DEFAULT '[]',
                conflict_types TEXT NOT NULL DEFAULT '[]',
                preceding_event_id TEXT,
                following_event_id TEXT,
                act_number INTEGER NOT NULL DEFAULT 1,
                position_in_act INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id),
                FOREIGN KEY (preceding_event_id) REFERENCES narrative_events(id),
                FOREIGN KEY (following_event_id) REFERENCES narrative_events(id)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_events_story ON narrative_events(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_events_chapter ON narrative_events(story_id, \
             chapter_number)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_events_type ON narrative_events(event_type)",
                [],
            )?;
        }
        Ok(())
    }
}
