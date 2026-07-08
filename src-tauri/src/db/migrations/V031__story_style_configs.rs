use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        31
    }

    fn description(&self) -> &'static str {
        "story style configs"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let story_style_config_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='story_style_configs'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if story_style_config_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_style_configs (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                name TEXT NOT NULL DEFAULT '默认混合',
                blend_json TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_style_configs_story ON story_style_configs(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_style_configs_active ON story_style_configs(story_id, \
             is_active)",
                [],
            )?;
        }
        Ok(())
    }
}
