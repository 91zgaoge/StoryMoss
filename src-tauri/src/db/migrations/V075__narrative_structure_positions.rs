use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        75
    }

    fn description(&self) -> &'static str {
        "narrative structure positions"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let position_tables: Vec<String> = conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='narrative_structure_positions'",
        )?
        .query_map([], |row| {
            let name: String = row.get(0)?;
            Ok(name)
        })?
        .collect::<Result<Vec<_>, _>>()?;

        if position_tables.is_empty() {
            conn.execute(
                "CREATE TABLE narrative_structure_positions (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL REFERENCES stories(id),
                event_id TEXT NOT NULL REFERENCES narrative_events(id),
                act_number INTEGER NOT NULL,
                act_type TEXT NOT NULL,
                position_in_act REAL NOT NULL,
                dramatic_function TEXT NOT NULL,
                is_narrative_boundary INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id),
                FOREIGN KEY (event_id) REFERENCES narrative_events(id)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_structure_positions_story ON \
             narrative_structure_positions(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_narrative_structure_positions_boundary ON \
             narrative_structure_positions(is_narrative_boundary)",
                [],
            )?;
        }
        Ok(())
    }
}
