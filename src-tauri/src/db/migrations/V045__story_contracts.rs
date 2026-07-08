use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        45
    }

    fn description(&self) -> &'static str {
        "story contracts"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let story_contract_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='story_contracts'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if story_contract_tables.is_empty() {
            conn.execute(
                "CREATE TABLE story_contracts (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                contract_type TEXT NOT NULL,
                contract_json TEXT NOT NULL,
                version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_contracts_story ON story_contracts(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_story_contracts_type ON story_contracts(story_id, contract_type)",
                [],
            )?;
        }
        Ok(())
    }
}
