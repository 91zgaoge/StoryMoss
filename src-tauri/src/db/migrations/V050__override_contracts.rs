use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        50
    }

    fn description(&self) -> &'static str {
        "override contracts"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let override_contract_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='override_contracts'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if override_contract_tables.is_empty() {
            conn.execute(
                "CREATE TABLE override_contracts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                story_id TEXT NOT NULL,
                chapter_number INTEGER NOT NULL,
                constraint_type TEXT NOT NULL,
                constraint_id TEXT NOT NULL,
                rationale_type TEXT NOT NULL,
                rationale_text TEXT NOT NULL,
                payback_plan TEXT NOT NULL,
                due_chapter INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                fulfilled_at TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_override_contracts_story ON override_contracts(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_override_contracts_status ON override_contracts(story_id, \
             status)",
                [],
            )?;
        }
        Ok(())
    }
}
