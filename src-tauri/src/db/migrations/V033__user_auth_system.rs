use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        33
    }

    fn description(&self) -> &'static str {
        "user auth system"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let auth_tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='users'")?
            .query_map([], |row| {
                let name: String = row.get(0)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if auth_tables.is_empty() {
            conn.execute(
                "CREATE TABLE users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE,
                display_name TEXT,
                avatar_url TEXT,
                is_local_user INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute(
                "CREATE TABLE oauth_accounts (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                provider TEXT NOT NULL,
                provider_account_id TEXT NOT NULL,
                access_token TEXT,
                refresh_token TEXT,
                expires_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(provider, provider_account_id)
            )",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_oauth_accounts_user ON oauth_accounts(user_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX idx_oauth_accounts_provider ON oauth_accounts(provider, \
             provider_account_id)",
                [],
            )?;
            conn.execute(
                "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                token TEXT NOT NULL UNIQUE,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
                [],
            )?;
            conn.execute("CREATE INDEX idx_sessions_token ON sessions(token)", [])?;
            conn.execute("CREATE INDEX idx_sessions_user ON sessions(user_id)", [])?;
        }
        Ok(())
    }
}
