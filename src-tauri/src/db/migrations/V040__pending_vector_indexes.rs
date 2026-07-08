use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        40
    }

    fn description(&self) -> &'static str {
        "pending vector indexes"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let pending_vector_tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND \
             name='pending_vector_indexes'",
            )?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if pending_vector_tables.is_empty() {
            conn.execute(
                "CREATE TABLE pending_vector_indexes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chapter_id TEXT NOT NULL UNIQUE,
                created_at INTEGER NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_pending_vector_chapter ON pending_vector_indexes(chapter_id)",
                [],
            )?;
        }
        Ok(())
    }
}
