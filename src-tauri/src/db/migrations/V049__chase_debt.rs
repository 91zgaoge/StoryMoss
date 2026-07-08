use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        49
    }

    fn description(&self) -> &'static str {
        "chase debt"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let chase_debt_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='chase_debt'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if chase_debt_tables.is_empty() {
            conn.execute(
                "CREATE TABLE chase_debt (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                story_id TEXT NOT NULL,
                debt_type TEXT NOT NULL,
                original_amount REAL NOT NULL DEFAULT 1.0,
                current_amount REAL NOT NULL DEFAULT 1.0,
                interest_rate REAL NOT NULL DEFAULT 0.1,
                source_chapter INTEGER NOT NULL,
                due_chapter INTEGER NOT NULL,
                override_contract_id INTEGER,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT NOT NULL,
                FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_chase_debt_story ON chase_debt(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_chase_debt_status ON chase_debt(story_id, status)",
                [],
            )?;
        }
        Ok(())
    }
}
