//! 功能使用度量遥测模块
//!
//! 不联网，只写本地 SQLite。用于评估各功能模块的使用情况，
//! 为功能去留决策提供数据支撑。

use chrono::Local;
use serde::Serialize;

use crate::db::DbPool;

/// 记录一次功能使用事件
pub fn log_feature_usage(
    pool: &DbPool,
    feature_id: &str,
    action: &str,
    story_id: Option<&str>,
    metadata: Option<&str>,
) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[Telemetry] Failed to get connection: {}", e);
            return;
        }
    };

    let id = uuid::Uuid::new_v4().to_string();
    let now = Local::now().to_rfc3339();

    if let Err(e) = conn.execute(
        "INSERT INTO feature_usage_logs (id, feature_id, action, story_id, metadata, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, feature_id, action, story_id, metadata, now],
    ) {
        log::warn!("[Telemetry] Failed to log feature usage: {}", e);
    }
}

/// 查询指定功能最近 N 天的使用次数
#[derive(Debug, Clone, Serialize)]
pub struct FeatureUsageStat {
    pub feature_id: String,
    pub action: String,
    pub count: i64,
}

pub fn get_feature_usage_stats(
    pool: &DbPool,
    days: i32,
) -> Result<Vec<FeatureUsageStat>, rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let since = (Local::now() - chrono::Duration::days(days as i64)).to_rfc3339();

    let mut stmt = conn.prepare(
        "SELECT feature_id, action, COUNT(*) as cnt
         FROM feature_usage_logs
         WHERE created_at >= ?1
         GROUP BY feature_id, action
         ORDER BY feature_id, action",
    )?;

    let stats = stmt
        .query_map([since], |row| {
            Ok(FeatureUsageStat {
                feature_id: row.get(0)?,
                action: row.get(1)?,
                count: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(stats)
}

/// 便捷封装：自动获取全局 pool 并记录
#[macro_export]
macro_rules! log_feature {
    ($feature_id:expr, $action:expr) => {
        if let Some(pool) = $crate::get_pool() {
            $crate::telemetry::log_feature_usage(&pool, $feature_id, $action, None, None);
        }
    };
    ($feature_id:expr, $action:expr, $story_id:expr) => {
        if let Some(pool) = $crate::get_pool() {
            $crate::telemetry::log_feature_usage(&pool, $feature_id, $action, Some($story_id), None);
        }
    };
}
