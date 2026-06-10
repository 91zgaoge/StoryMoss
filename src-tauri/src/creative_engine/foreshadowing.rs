//! Foreshadowing Tracker - 伏笔追踪系统
//!
//! 追踪故事中的伏笔（setup）和回收（payoff），
//! 在写作时提醒作者回收未解伏笔。

use chrono::Local;
use rusqlite::params;
use uuid::Uuid;

use crate::db::DbPool;

/// 伏笔状态
#[derive(Debug, Clone)]
pub enum ForeshadowingStatus {
    Setup,     // 已设置，未回收
    Payoff,    // 已回收
    Abandoned, // 已放弃
}

impl std::fmt::Display for ForeshadowingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForeshadowingStatus::Setup => write!(f, "setup"),
            ForeshadowingStatus::Payoff => write!(f, "payoff"),
            ForeshadowingStatus::Abandoned => write!(f, "abandoned"),
        }
    }
}

/// 伏笔记录
#[derive(Debug, Clone)]
pub struct ForeshadowingRecord {
    pub id: String,
    pub story_id: String,
    pub content: String,
    pub setup_scene_id: Option<String>,
    pub payoff_scene_id: Option<String>,
    // LitSeg: 叙事事件关联（从 narrative_threads.foreshadow 合并）
    pub setup_event_id: Option<String>,
    pub payoff_event_id: Option<String>,
    pub risk_signals_score: Option<f32>,
    pub status: ForeshadowingStatus,
    pub importance: i32, // 1-10
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// 伏笔追踪器
pub struct ForeshadowingTracker {
    pool: DbPool,
}

impl ForeshadowingTracker {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 添加新伏笔
    pub fn add_foreshadowing(
        &self,
        story_id: &str,
        content: &str,
        setup_scene_id: Option<&str>,
        importance: i32,
    ) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;

        conn.execute(
            "INSERT INTO foreshadowing_tracker (id, story_id, content, setup_scene_id, status, \
             importance, created_at)
             VALUES (?1, ?2, ?3, ?4, 'setup', ?5, ?6)",
            params![
                &id,
                story_id,
                content,
                setup_scene_id,
                importance.clamp(1, 10),
                now
            ],
        )
        .map_err(|e| format!("插入伏笔失败: {}", e))?;

        Ok(id)
    }

    /// 标记伏笔为已回收
    pub fn mark_payoff(
        &self,
        foreshadowing_id: &str,
        payoff_scene_id: Option<&str>,
    ) -> Result<(), String> {
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;

        conn.execute(
            "UPDATE foreshadowing_tracker SET status = 'payoff', payoff_scene_id = ?2, \
             resolved_at = ?3 WHERE id = ?1",
            params![foreshadowing_id, payoff_scene_id, now],
        )
        .map_err(|e| format!("更新伏笔状态失败: {}", e))?;

        Ok(())
    }

    /// 放弃伏笔
    pub fn abandon(&self, foreshadowing_id: &str) -> Result<(), String> {
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;

        conn.execute(
            "UPDATE foreshadowing_tracker SET status = 'abandoned', resolved_at = ?2 WHERE id = ?1",
            params![foreshadowing_id, now],
        )
        .map_err(|e| format!("放弃伏笔失败: {}", e))?;

        Ok(())
    }

    /// 获取故事中未回收的伏笔
    pub fn get_unresolved(&self, story_id: &str) -> Result<Vec<ForeshadowingRecord>, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, content, setup_scene_id, payoff_scene_id, status, \
                 importance, created_at, resolved_at
             FROM foreshadowing_tracker WHERE story_id = ?1 AND status = 'setup'
             ORDER BY importance DESC, created_at ASC",
            )
            .map_err(|e| format!("准备查询失败: {}", e))?;

        let records = stmt
            .query_map([story_id], |row| {
                let status_str: String = row.get(5)?;
                let status = match status_str.as_str() {
                    "setup" => ForeshadowingStatus::Setup,
                    "payoff" => ForeshadowingStatus::Payoff,
                    "abandoned" => ForeshadowingStatus::Abandoned,
                    _ => ForeshadowingStatus::Setup,
                };

                Ok(ForeshadowingRecord {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    content: row.get(2)?,
                    setup_scene_id: row.get(3)?,
                    payoff_scene_id: row.get(4)?,
                    status,
                    importance: row.get(6)?,
                    created_at: row.get(7)?,
                    setup_event_id: None,
                    payoff_event_id: None,
                    risk_signals_score: None,
                    resolved_at: row.get(8)?,
                })
            })
            .map_err(|e| format!("查询失败: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("映射失败: {}", e))?;

        Ok(records)
    }

    /// 获取所有伏笔（用于幕后看板）
    pub fn get_all(&self, story_id: &str) -> Result<Vec<ForeshadowingRecord>, String> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, content, setup_scene_id, payoff_scene_id, status, \
                 importance, created_at, resolved_at
             FROM foreshadowing_tracker WHERE story_id = ?1
             ORDER BY importance DESC, created_at ASC",
            )
            .map_err(|e| format!("准备查询失败: {}", e))?;

        let records = stmt
            .query_map([story_id], |row| {
                let status_str: String = row.get(5)?;
                let status = match status_str.as_str() {
                    "setup" => ForeshadowingStatus::Setup,
                    "payoff" => ForeshadowingStatus::Payoff,
                    "abandoned" => ForeshadowingStatus::Abandoned,
                    _ => ForeshadowingStatus::Setup,
                };

                Ok(ForeshadowingRecord {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    content: row.get(2)?,
                    setup_scene_id: row.get(3)?,
                    payoff_scene_id: row.get(4)?,
                    status,
                    importance: row.get(6)?,
                    created_at: row.get(7)?,
                    setup_event_id: None,
                    payoff_event_id: None,
                    risk_signals_score: None,
                    resolved_at: row.get(8)?,
                })
            })
            .map_err(|e| format!("查询失败: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("映射失败: {}", e))?;

        Ok(records)
    }

    /// 获取写作时的轻量提示文本
    pub fn get_writing_hints(&self, story_id: &str, limit: usize) -> Result<Vec<String>, String> {
        let unresolved = self.get_unresolved(story_id)?;
        let hints: Vec<String> = unresolved
            .into_iter()
            .take(limit)
            .map(|r| {
                let importance_marker = match r.importance {
                    8..=10 => "【关键】",
                    5..=7 => "【重要】",
                    _ => "【次要】",
                };
                format!("{} 未回收伏笔: {}", importance_marker, r.content)
            })
            .collect();
        Ok(hints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_foreshadowing_status_display() {
        assert_eq!(ForeshadowingStatus::Setup.to_string(), "setup");
        assert_eq!(ForeshadowingStatus::Payoff.to_string(), "payoff");
    }
}
