use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        78
    }

    fn description(&self) -> &'static str {
        "scene litseg fields"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let scene_cols: Vec<String> = conn
            .prepare("PRAGMA table_info(scenes)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !scene_cols.contains(&"narrative_intensity".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_intensity REAL DEFAULT 0.5",
                [],
            )?;
        }
        if !scene_cols.contains(&"narrative_sentiment".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_sentiment REAL DEFAULT 0.0",
                [],
            )?;
        }
        if !scene_cols.contains(&"narrative_event_types".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_event_types TEXT DEFAULT '[]'",
                [],
            )?;
        }
        if !scene_cols.contains(&"narrative_preceding_scene_id".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_preceding_scene_id TEXT",
                [],
            )?;
        }
        if !scene_cols.contains(&"narrative_following_scene_id".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN narrative_following_scene_id TEXT",
                [],
            )?;
        }
        if !scene_cols.contains(&"act_number".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN act_number INTEGER DEFAULT 1",
                [],
            )?;
        }
        if !scene_cols.contains(&"position_in_act".to_string()) {
            conn.execute(
                "ALTER TABLE scenes ADD COLUMN position_in_act INTEGER DEFAULT 1",
                [],
            )?;
        }
        Ok(())
    }
}
