use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        34
    }

    fn description(&self) -> &'static str {
        "subscription real user id"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let sub_columns: Vec<String> = conn
            .prepare("PRAGMA table_info(subscriptions)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if !sub_columns.iter().any(|c| c == "real_user_id") {
            conn.execute(
                "ALTER TABLE subscriptions ADD COLUMN real_user_id TEXT REFERENCES users(id)",
                [],
            )?;
        }
        Ok(())
    }
}
