use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::{agency::models::ResourceLease, db::DbPool, error::AppError};

/// 资源租约：写库提交点的互斥锁。
/// 拿不到租约时返回 `resource_busy`（ValidationFailed + field），调用方等
/// owner 结束/过期后重试，不直接改库。
pub struct LeaseManager {
    pool: DbPool,
}

impl LeaseManager {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn acquire(
        &self,
        story_id: &str,
        resource: &str,
        owner_run_id: Option<&str>,
        owner_task_id: Option<&str>,
        ttl_secs: u64,
    ) -> Result<ResourceLease, AppError> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(AppError::from)?;

        let existing: Option<(String, String, Option<String>, Option<String>)> = tx
            .query_row(
                "SELECT id, expires_at, owner_run_id, owner_task_id
                 FROM story_resource_leases WHERE story_id = ?1 AND resource = ?2",
                params![story_id, resource],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(AppError::from)?;

        let now = chrono::Utc::now();
        if let Some((old_id, expires_at, owner_run, owner_task)) = existing {
            let expired = chrono::DateTime::parse_from_rfc3339(&expires_at)
                .map(|dt| dt.with_timezone(&chrono::Utc) <= now)
                .unwrap_or(true);
            if !expired {
                let owner = owner_task
                    .as_deref()
                    .or(owner_run.as_deref())
                    .unwrap_or("unknown");
                return Err(AppError::validation_failed(
                    format!(
                        "资源「{}」被任务 {} 占用至 {}；等它结束或过期后重试",
                        resource, owner, expires_at
                    ),
                    Some("resource_busy"),
                ));
            }
            tx.execute(
                "DELETE FROM story_resource_leases WHERE id = ?1",
                params![old_id],
            )
            .map_err(AppError::from)?;
        }

        let lease = ResourceLease {
            id: uuid::Uuid::new_v4().to_string(),
            story_id: story_id.to_string(),
            resource: resource.to_string(),
            owner_run_id: owner_run_id.map(|s| s.to_string()),
            owner_task_id: owner_task_id.map(|s| s.to_string()),
            acquired_at: now.to_rfc3339(),
            heartbeat_at: now.to_rfc3339(),
            expires_at: (now + chrono::Duration::seconds(ttl_secs as i64)).to_rfc3339(),
        };
        tx.execute(
            "INSERT INTO story_resource_leases
             (id, story_id, resource, owner_run_id, owner_task_id, acquired_at, heartbeat_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                lease.id,
                lease.story_id,
                lease.resource,
                lease.owner_run_id,
                lease.owner_task_id,
                lease.acquired_at,
                lease.heartbeat_at,
                lease.expires_at,
            ],
        )
        .map_err(AppError::from)?;
        tx.commit().map_err(AppError::from)?;
        Ok(lease)
    }

    pub fn heartbeat(&self, lease_id: &str, extend_secs: u64) -> Result<usize, AppError> {
        let now = chrono::Utc::now();
        let expires = (now + chrono::Duration::seconds(extend_secs as i64)).to_rfc3339();
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "UPDATE story_resource_leases SET heartbeat_at = ?2, expires_at = ?3 WHERE id = ?1",
            params![lease_id, now.to_rfc3339(), expires],
        )
        .map_err(AppError::from)
    }

    pub fn release(&self, lease_id: &str) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "DELETE FROM story_resource_leases WHERE id = ?1",
            params![lease_id],
        )
        .map_err(AppError::from)
    }

    pub fn release_resource(&self, story_id: &str, resource: &str) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "DELETE FROM story_resource_leases WHERE story_id = ?1 AND resource = ?2",
            params![story_id, resource],
        )
        .map_err(AppError::from)
    }

    pub fn reap_expired(&self) -> Result<usize, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.execute(
            "DELETE FROM story_resource_leases WHERE datetime(expires_at) < datetime('now')",
            [],
        )
        .map_err(AppError::from)
    }

    pub fn get(&self, lease_id: &str) -> Result<Option<ResourceLease>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        conn.query_row(
            "SELECT id, story_id, resource, owner_run_id, owner_task_id,
                    acquired_at, heartbeat_at, expires_at
             FROM story_resource_leases WHERE id = ?1",
            params![lease_id],
            |row| {
                Ok(ResourceLease {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    resource: row.get(2)?,
                    owner_run_id: row.get(3)?,
                    owner_task_id: row.get(4)?,
                    acquired_at: row.get(5)?,
                    heartbeat_at: row.get(6)?,
                    expires_at: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(AppError::from)
    }

    pub fn list_for_story(&self, story_id: &str) -> Result<Vec<ResourceLease>, AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {e}")))?;
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, resource, owner_run_id, owner_task_id,
                        acquired_at, heartbeat_at, expires_at
                 FROM story_resource_leases WHERE story_id = ?1 ORDER BY acquired_at DESC",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map(params![story_id], |row| {
                Ok(ResourceLease {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    resource: row.get(2)?,
                    owner_run_id: row.get(3)?,
                    owner_task_id: row.get(4)?,
                    acquired_at: row.get(5)?,
                    heartbeat_at: row.get(6)?,
                    expires_at: row.get(7)?,
                })
            })
            .map_err(AppError::from)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    fn manager() -> LeaseManager {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        LeaseManager::new(pool)
    }

    #[test]
    fn test_acquire_and_release() {
        let mgr = manager();
        let lease = mgr
            .acquire(
                "story-1",
                "chapter_sequence",
                Some("run-1"),
                Some("task-1"),
                300,
            )
            .unwrap();
        assert_eq!(lease.resource, "chapter_sequence");
        assert_eq!(mgr.list_for_story("story-1").unwrap().len(), 1);
        assert_eq!(mgr.release(&lease.id).unwrap(), 1);
        assert!(mgr.list_for_story("story-1").unwrap().is_empty());
    }

    #[test]
    fn test_conflict_returns_owner() {
        let mgr = manager();
        mgr.acquire(
            "story-1",
            "assets:world",
            Some("run-1"),
            Some("task-1"),
            300,
        )
        .unwrap();
        let err = mgr
            .acquire(
                "story-1",
                "assets:world",
                Some("run-2"),
                Some("task-2"),
                300,
            )
            .unwrap_err();
        assert_eq!(err.code(), "VALIDATION_FAILED");
        assert!(err.to_string().contains("task-1"));
    }

    #[test]
    fn test_expired_can_be_reacquired() {
        let mgr = manager();
        let lease = mgr
            .acquire("story-1", "gate:12", Some("run-1"), Some("task-1"), 0)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let reacquired = mgr
            .acquire("story-1", "gate:12", Some("run-2"), Some("task-2"), 300)
            .unwrap();
        assert_ne!(lease.id, reacquired.id);
    }

    #[test]
    fn test_heartbeat_and_reap() {
        let mgr = manager();
        let lease = mgr
            .acquire("story-1", "draft:12", Some("run-1"), Some("task-1"), 300)
            .unwrap();
        mgr.heartbeat(&lease.id, 600).unwrap();
        assert!(mgr.get(&lease.id).unwrap().is_some());
        let conn = mgr.pool.get().unwrap();
        conn.execute(
            "UPDATE story_resource_leases SET expires_at = datetime('now', '-1 seconds') WHERE id = ?1",
            params![lease.id],
        )
        .unwrap();
        assert_eq!(mgr.reap_expired().unwrap(), 1);
    }
}
