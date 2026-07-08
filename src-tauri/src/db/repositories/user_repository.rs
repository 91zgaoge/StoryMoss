use super::*;

// ==================== UserRepository ====================

pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_user(
        &self,
        email: Option<String>,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<User, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO users (id, email, display_name, avatar_url, is_local_user, created_at, \
             updated_at) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            params![&id, email, display_name, avatar_url, now.to_rfc3339()],
        )?;

        Ok(User {
            id,
            email,
            display_name,
            avatar_url,
            is_local_user: false,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn find_by_oauth(
        &self,
        provider: &str,
        provider_account_id: &str,
    ) -> Result<Option<User>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT u.id, u.email, u.display_name, u.avatar_url, u.is_local_user, u.created_at, \
             u.updated_at
             FROM users u
             JOIN oauth_accounts oa ON u.id = oa.user_id
             WHERE oa.provider = ?1 AND oa.provider_account_id = ?2",
        )?;

        let user = stmt
            .query_row([provider, provider_account_id], |row| {
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    display_name: row.get(2)?,
                    avatar_url: row.get(3)?,
                    is_local_user: row.get::<_, i32>(4)? != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(user)
    }

    pub fn create_oauth_account(
        &self,
        user_id: &str,
        provider: &str,
        provider_account_id: &str,
        access_token: Option<String>,
        refresh_token: Option<String>,
        expires_at: Option<chrono::DateTime<Local>>,
    ) -> Result<OAuthAccount, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO oauth_accounts (id, user_id, provider, provider_account_id, \
             access_token, refresh_token, expires_at, created_at, updated_at) VALUES (?1, ?2, ?3, \
             ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                &id,
                user_id,
                provider,
                provider_account_id,
                access_token,
                refresh_token,
                expires_at.map(|d| d.to_rfc3339()),
                now.to_rfc3339()
            ],
        )?;

        Ok(OAuthAccount {
            id,
            user_id: user_id.to_string(),
            provider: provider.to_string(),
            provider_account_id: provider_account_id.to_string(),
            access_token,
            refresh_token,
            expires_at,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn create_session(
        &self,
        user_id: &str,
        token: &str,
        expires_at: chrono::DateTime<Local>,
    ) -> Result<Session, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO sessions (id, user_id, token, expires_at, created_at) VALUES (?1, ?2, \
             ?3, ?4, ?5)",
            params![
                &id,
                user_id,
                token,
                expires_at.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(Session {
            id,
            user_id: user_id.to_string(),
            token: token.to_string(),
            expires_at,
            created_at: now,
        })
    }

    pub fn delete_session(&self, token: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM sessions WHERE token = ?1", [token])?;
        Ok(count)
    }

    pub fn to_user_info(&self, user: &User) -> UserInfo {
        UserInfo {
            id: user.id.clone(),
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            avatar_url: user.avatar_url.clone(),
        }
    }
}
