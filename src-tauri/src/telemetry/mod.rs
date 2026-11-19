#![allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed(pool: &DbPool, feature: &str, action: &str, days_ago: i64) {
        let conn = pool.get().unwrap();
        let created = (Local::now() - chrono::Duration::days(days_ago)).to_rfc3339();
        conn.execute(
            "INSERT INTO feature_usage_logs (id, feature_id, action, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![uuid::Uuid::new_v4().to_string(), feature, action, created],
        )
        .unwrap();
    }

    #[test]
    fn stats_group_by_feature_action_and_respect_window() {
        let pool = create_test_pool().unwrap();
        seed(&pool, "story_contract", "opened", 0);
        seed(&pool, "story_contract", "opened", 1);
        seed(&pool, "story_contract", "executed", 2);
        // 窗口外（40 天前）不计入 30 天统计。
        seed(&pool, "memory_pack", "opened", 40);

        let stats = get_feature_usage_stats(&pool, 30).unwrap();
        assert_eq!(stats.len(), 2, "{stats:?}");
        let opened = stats
            .iter()
            .find(|s| s.feature_id == "story_contract" && s.action == "opened")
            .expect("story_contract/opened");
        assert_eq!(opened.count, 2);
        assert!(!stats.iter().any(|s| s.feature_id == "memory_pack"));

        let long_window = get_feature_usage_stats(&pool, 60).unwrap();
        assert!(long_window.iter().any(|s| s.feature_id == "memory_pack"));
    }
}
