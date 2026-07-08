use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        93
    }

    fn description(&self) -> &'static str {
        "prompt overrides"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS prompt_overrides (
            prompt_id           TEXT PRIMARY KEY,
            overridden_content  TEXT NOT NULL,
            updated_at          INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_prompt_overrides_updated
            ON prompt_overrides(updated_at);
        ",
        )?;
        Ok(())
    }
}
