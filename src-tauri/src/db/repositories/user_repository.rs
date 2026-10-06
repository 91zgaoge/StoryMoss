use super::*;

// ==================== UserRepository ====================

pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // v0.34 起登录改走 server 中转（upsert_server_user），本地 OAuth
    // 建号路径暂不被 命令引用，保留供后续复用。
    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    /// 按 id 查询用户
    pub fn find_by_id(&self, id: &str) -> Result<User, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.query_row(
            "SELECT id, email, display_name, avatar_url, is_local_user, created_at, updated_at \
             FROM users WHERE id = ?1",
            [id],
            |row| {
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
            },
        )
    }

    /// 写入/更新 server 登录用户（id 为 server UUID）
    pub fn upsert_server_user(
        &self,
        id: &str,
        email: Option<String>,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<User, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO users (id, email, display_name, avatar_url, is_local_user, created_at, \
             updated_at) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5) ON CONFLICT(id) DO UPDATE SET \
             email=?2, display_name=?3, avatar_url=?4, updated_at=?5",
            params![id, email, display_name, avatar_url, now],
        )?;
        drop(conn);
        self.find_by_id(id)
    }

    /// 取最新未过期 session 及其用户（get_current_user 用）
    pub fn find_latest_valid_session(&self) -> Option<(User, String)> {
        let conn = self.pool.get().ok()?;
        let now = Local::now().to_rfc3339();
        conn.query_row(
            "SELECT u.id, u.email, u.display_name, u.avatar_url, u.is_local_user, u.created_at, \
             u.updated_at, s.token FROM sessions s JOIN users u ON u.id = s.user_id WHERE \
             s.expires_at > ?1 ORDER BY s.created_at DESC LIMIT 1",
            params![now],
            |row| {
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok((
                    User {
                        id: row.get(0)?,
                        email: row.get(1)?,
                        display_name: row.get(2)?,
                        avatar_url: row.get(3)?,
                        is_local_user: row.get::<_, i32>(4)? != 0,
                        created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                        updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    },
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .optional()
        .ok()
        .flatten()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn repo() -> UserRepository {
        UserRepository::new(create_test_pool().unwrap())
    }

    #[test]
    fn upsert_server_user_creates_then_updates_same_id() {
        let repo = repo();
        let user = repo
            .upsert_server_user(
                "srv-u1",
                Some("a@example.com".to_string()),
                Some("Alice".to_string()),
                None,
            )
            .unwrap();
        assert_eq!(user.id, "srv-u1");
        assert_eq!(user.email.as_deref(), Some("a@example.com"));
        assert_eq!(user.display_name.as_deref(), Some("Alice"));
        assert!(!user.is_local_user);

        // 同 id 再次 upsert：更新资料而不是报错/新建
        let updated = repo
            .upsert_server_user(
                "srv-u1",
                Some("b@example.com".to_string()),
                Some("Alice2".to_string()),
                Some("https://example.com/a.png".to_string()),
            )
            .unwrap();
        assert_eq!(updated.id, "srv-u1");
        assert_eq!(updated.email.as_deref(), Some("b@example.com"));
        assert_eq!(updated.display_name.as_deref(), Some("Alice2"));
        assert_eq!(
            updated.avatar_url.as_deref(),
            Some("https://example.com/a.png")
        );
    }

    #[test]
    fn find_latest_valid_session_returns_user_and_token() {
        let repo = repo();
        let user = repo
            .upsert_server_user("srv-u2", None, Some("Bob".to_string()), None)
            .unwrap();
        let expires_at = Local::now() + chrono::Duration::days(7);
        repo.create_session(&user.id, "token-valid", expires_at)
            .unwrap();

        let (found_user, token) = repo
            .find_latest_valid_session()
            .expect("should find session");
        assert_eq!(found_user.id, "srv-u2");
        assert_eq!(found_user.display_name.as_deref(), Some("Bob"));
        assert_eq!(token, "token-valid");
    }

    #[test]
    fn find_latest_valid_session_returns_none_when_expired() {
        let repo = repo();
        let user = repo.upsert_server_user("srv-u3", None, None, None).unwrap();
        let expires_at = Local::now() - chrono::Duration::days(1);
        repo.create_session(&user.id, "token-expired", expires_at)
            .unwrap();

        assert!(repo.find_latest_valid_session().is_none());
    }
}
