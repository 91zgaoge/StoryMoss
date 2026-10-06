#![allow(dead_code)]
//! 质量债台账（P3-A，v0.63.0）。
//!
//! 背景：质检 fail-open（`salvage_failed_gate` / 异常降级放行）是「保产出」的
//! 正确取舍，但此前的降级是**静默**的——问题既不进任务记录也不进任何队列。
//! 本模块把「未解决的问题」显式记账（含建议回收窗口），让降级可见、可追踪、
//! 可在后续版本批量处理（配合 `auto_revise`），而不是消失。
//!
//! 记账是幂等的：同 (story, chapter, detail) 只保留一条，重复出现只刷新时间。

use chrono::Local;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QualityDebt {
    pub id: String,
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_number: Option<i32>,
    pub source: String,
    pub severity: String,
    pub detail: String,
    pub suggested_window: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 建议回收窗口：距今越近的问题越建议尽快处理。
pub fn suggested_window(chapter_number: Option<i32>, severity: &str) -> String {
    let base = chapter_number.unwrap_or(0);
    match severity {
        "critical" => format!("第 {} 章前（尽快）", base + 1),
        "warning" => format!("第 {}-{} 章窗口", base + 2, base + 5),
        _ => format!("第 {}-{} 章窗口", base + 5, base + 10),
    }
}

/// 记录一条质量债（幂等 upsert）。
pub fn record_debt(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
    chapter_number: Option<i32>,
    source: &str,
    severity: &str,
    detail: &str,
) -> Result<(), rusqlite::Error> {
    let detail = detail.trim();
    if detail.is_empty() {
        return Ok(());
    }
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    let window = suggested_window(chapter_number, severity);
    conn.execute(
        "INSERT INTO quality_debts \
         (id, story_id, scene_id, chapter_number, source, severity, detail, suggested_window, \
          status, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'open', ?9, ?9) \
         ON CONFLICT(story_id, chapter_number, detail) DO UPDATE SET \
          severity = excluded.severity, suggested_window = excluded.suggested_window, \
          scene_id = excluded.scene_id, updated_at = excluded.updated_at \
         WHERE quality_debts.status = 'open'",
        params![
            uuid::Uuid::new_v4().to_string(),
            story_id,
            scene_id,
            chapter_number,
            source,
            severity,
            detail,
            window,
            now
        ],
    )?;
    Ok(())
}

/// 列出质量债（按状态过滤；默认 open）。
pub fn list_debts(
    pool: &DbPool,
    story_id: &str,
    status: Option<&str>,
    limit: i64,
) -> Vec<QualityDebt> {
    let Ok(conn) = pool.get() else {
        return Vec::new();
    };
    let status = status.unwrap_or("open");
    let mut stmt = match conn.prepare(
        "SELECT id, story_id, scene_id, chapter_number, source, severity, detail, \
                suggested_window, status, created_at, updated_at \
         FROM quality_debts WHERE story_id = ?1 AND status = ?2 \
         ORDER BY created_at DESC LIMIT ?3",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id, status, limit], |row| {
        Ok(QualityDebt {
            id: row.get(0)?,
            story_id: row.get(1)?,
            scene_id: row.get(2)?,
            chapter_number: row.get(3)?,
            source: row.get(4)?,
            severity: row.get(5)?,
            detail: row.get(6)?,
            suggested_window: row.get(7)?,
            status: row.get(8)?,
            created_at: row.get(9)?,
            updated_at: row.get(10)?,
        })
    });
    rows.map(|r| r.flatten().collect()).unwrap_or_default()
}

/// 结清 / 忽略一条质量债。
pub fn resolve_debt(pool: &DbPool, debt_id: &str, status: &str) -> Result<usize, rusqlite::Error> {
    let status = match status {
        "resolved" | "dismissed" => status,
        _ => "resolved",
    };
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "UPDATE quality_debts SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status, now, debt_id],
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
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '债', ?2, ?2)",
            params![&story_id, &now],
        )
        .unwrap();
        story_id
    }

    #[test]
    fn record_is_idempotent_and_lists_open_debts() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        record_debt(
            &pool,
            &story_id,
            Some("scene-1"),
            Some(3),
            "editor_qc",
            "warning",
            "第3章：场景目标未兑现",
        )
        .unwrap();
        record_debt(
            &pool,
            &story_id,
            Some("scene-1"),
            Some(3),
            "editor_qc",
            "critical",
            "第3章：场景目标未兑现",
        )
        .unwrap();
        record_debt(
            &pool,
            &story_id,
            None,
            Some(4),
            "probe",
            "info",
            "第4章：未落实冲突加压",
        )
        .unwrap();

        let debts = list_debts(&pool, &story_id, None, 50);
        assert_eq!(debts.len(), 2, "同章同问题应 upsert: {debts:?}");
        let updated = debts.iter().find(|d| d.chapter_number == Some(3)).unwrap();
        assert_eq!(updated.severity, "critical", "重复出现应升级而非重复记账");
    }

    #[test]
    fn resolve_moves_out_of_open_list() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        record_debt(
            &pool,
            &story_id,
            None,
            Some(2),
            "editor_qc",
            "warning",
            "问题A",
        )
        .unwrap();
        let debt = list_debts(&pool, &story_id, None, 10).remove(0);

        assert_eq!(resolve_debt(&pool, &debt.id, "resolved").unwrap(), 1);
        assert!(list_debts(&pool, &story_id, None, 10).is_empty());
        let resolved = list_debts(&pool, &story_id, Some("resolved"), 10);
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn suggested_window_scales_with_severity() {
        assert!(suggested_window(Some(3), "critical").contains("第 4 章前"));
        assert!(suggested_window(Some(3), "warning").contains("第 5-8 章"));
        assert!(suggested_window(Some(3), "info").contains("第 8-13 章"));
    }

    #[test]
    fn blank_detail_is_ignored() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        record_debt(
            &pool,
            &story_id,
            None,
            Some(1),
            "editor_qc",
            "warning",
            "   ",
        )
        .unwrap();
        assert!(list_debts(&pool, &story_id, None, 10).is_empty());
    }
}
