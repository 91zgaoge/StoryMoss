#![allow(dead_code)]
//! 新增物审批队列（P3-B，v0.63.0）。
//!
//! 对应外部经验（oh-story「新增物三级」）：写手/分析自动新增的**规则类**资产
//! （世界观硬规则、金手指规则等）不应直接成为硬约束，而应先进入待确认队列，
//! 由作者确认后才被当作规则使用。本模块只负责队列的落库与状态迁移；
//! 界面在后续版本接入（命令已就绪）。
//!
//! 入库幂等：同 (story, kind, subject) 只保留一条，重复只刷新 detail 与时间。

use chrono::Local;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;

/// 触发审批的重要性下限（规则类实体）
pub const RULE_REVIEW_MIN_IMPORTANCE: i32 = 7;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingReview {
    pub id: String,
    pub story_id: String,
    pub kind: String,
    pub subject: String,
    pub detail: Option<String>,
    pub source: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 记入待确认队列（幂等 upsert；已确认/拒绝的不再回到 pending）。
pub fn note_pending_review(
    pool: &DbPool,
    story_id: &str,
    kind: &str,
    subject: &str,
    detail: &str,
) -> Result<(), rusqlite::Error> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Ok(());
    }
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO pending_reviews \
         (id, story_id, kind, subject, detail, source, status, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'ingest', 'pending', ?6, ?6) \
         ON CONFLICT(story_id, kind, subject) DO UPDATE SET \
          detail = excluded.detail, updated_at = excluded.updated_at \
         WHERE pending_reviews.status = 'pending'",
        params![
            uuid::Uuid::new_v4().to_string(),
            story_id,
            kind,
            subject,
            if detail.trim().is_empty() {
                None
            } else {
                Some(detail.trim())
            },
            now
        ],
    )?;
    Ok(())
}

pub fn list_pending_reviews(
    pool: &DbPool,
    story_id: &str,
    status: Option<&str>,
) -> Vec<PendingReview> {
    let Ok(conn) = pool.get() else {
        return Vec::new();
    };
    let status = status.unwrap_or("pending");
    let mut stmt = match conn.prepare(
        "SELECT id, story_id, kind, subject, detail, source, status, created_at, updated_at \
         FROM pending_reviews WHERE story_id = ?1 AND status = ?2 ORDER BY created_at DESC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id, status], |row| {
        Ok(PendingReview {
            id: row.get(0)?,
            story_id: row.get(1)?,
            kind: row.get(2)?,
            subject: row.get(3)?,
            detail: row.get(4)?,
            source: row.get(5)?,
            status: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    });
    rows.map(|r| r.flatten().collect()).unwrap_or_default()
}

/// 确认 / 拒绝一条待确认项。
pub fn resolve_pending_review(
    pool: &DbPool,
    review_id: &str,
    status: &str,
) -> Result<usize, rusqlite::Error> {
    let status = match status {
        "confirmed" | "rejected" => status,
        _ => "confirmed",
    };
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "UPDATE pending_reviews SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status, now, review_id],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed_story(pool: &DbPool) -> String {
        let story_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '审批', ?2, ?2)",
            params![&story_id, &now],
        )
        .unwrap();
        story_id
    }

    #[test]
    fn note_is_idempotent_and_resolve_removes_from_pending() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        note_pending_review(&pool, &story_id, "world_rule", "灵气复苏三阶段", "初版描述").unwrap();
        note_pending_review(&pool, &story_id, "world_rule", "灵气复苏三阶段", "更新描述").unwrap();

        let pending = list_pending_reviews(&pool, &story_id, None);
        assert_eq!(pending.len(), 1, "同 subject 应 upsert");
        assert_eq!(pending[0].detail.as_deref(), Some("更新描述"));

        assert_eq!(
            resolve_pending_review(&pool, &pending[0].id, "confirmed").unwrap(),
            1
        );
        assert!(list_pending_reviews(&pool, &story_id, None).is_empty());
        // 已确认的不再被重复入队覆盖
        note_pending_review(&pool, &story_id, "world_rule", "灵气复苏三阶段", "又变了").unwrap();
        let confirmed = list_pending_reviews(&pool, &story_id, Some("confirmed"));
        assert_eq!(confirmed.len(), 1);
        assert!(list_pending_reviews(&pool, &story_id, None).is_empty());
    }

    #[test]
    fn blank_subject_is_ignored() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        note_pending_review(&pool, &story_id, "world_rule", "  ", "x").unwrap();
        assert!(list_pending_reviews(&pool, &story_id, None).is_empty());
    }
}
