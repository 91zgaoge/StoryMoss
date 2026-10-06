#![allow(dead_code)]
//! 成本账本与计费盲区哨兵（P2-D，v0.62.0）。
//!
//! 数据来源是既有的 `llm_calls` 表（每次 LLM 调用一行，含 token 与成功标记）。
//! 本模块提供：
//! - **按故事聚合**：调用次数 / token / 失败数 / 零 token 次数 / 时间范围；
//! - **阈值提示**：累计 token 超过 `DEFAULT_STORY_TOKEN_WARN`
//!   时给出提示（非硬停）；
//! - **零增量计费盲区检测**：连续 `ZERO_DELTA_STREAK_ALERT` 次调用记账为 0
//!   token 时告警——说明网关或模型没有回报用量，任何预算上限都不会触发
//!   （对齐外部项目 ainovel-cli 的 BudgetSentinel 经验）。
//!
//! 明确不做：不在此处硬停 LLM。硬性熔断由 agency run 预算与后台闸门负责；
//! 这里是「可见性与早警」，避免静默超支。

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;

/// 单故事累计 token 提示阈值（约合 300+ 章节的写作量级；仅提示不熔断）
pub const DEFAULT_STORY_TOKEN_WARN: u64 = 500_000;
/// 连续零 token 记账达到该次数即判定为计费盲区
pub const ZERO_DELTA_STREAK_ALERT: usize = 5;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StoryCostSummary {
    pub story_id: String,
    pub total_calls: u64,
    pub total_tokens: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub failed_calls: u64,
    /// 记账为 0 token 的调用次数（模型/网关未回报用量的信号）
    pub zero_token_calls: u64,
    pub first_call_at: Option<String>,
    pub last_call_at: Option<String>,
    /// 累计 token 是否已超过提示阈值
    pub budget_warning: bool,
    pub warn_threshold: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CostAnomaly {
    pub kind: String,
    pub detail: String,
}

/// 按故事聚合 llm_calls。
pub fn summarize_story_cost(pool: &DbPool, story_id: &str) -> StoryCostSummary {
    let mut summary = StoryCostSummary {
        story_id: story_id.to_string(),
        warn_threshold: DEFAULT_STORY_TOKEN_WARN,
        ..Default::default()
    };
    let Ok(conn) = pool.get() else {
        return summary;
    };
    let row = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(total_tokens), 0), COALESCE(SUM(prompt_tokens), 0), \
                COALESCE(SUM(completion_tokens), 0), \
                COALESCE(SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END), 0), \
                COALESCE(SUM(CASE WHEN total_tokens = 0 THEN 1 ELSE 0 END), 0), \
                MIN(created_at), MAX(created_at) \
         FROM llm_calls WHERE story_id = ?1",
        params![story_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        },
    );
    match row {
        Ok((calls, tokens, prompt, completion, failed, zero, first, last)) => {
            summary.total_calls = calls.max(0) as u64;
            summary.total_tokens = tokens.max(0) as u64;
            summary.prompt_tokens = prompt.max(0) as u64;
            summary.completion_tokens = completion.max(0) as u64;
            summary.failed_calls = failed.max(0) as u64;
            summary.zero_token_calls = zero.max(0) as u64;
            summary.first_call_at = first;
            summary.last_call_at = last;
            summary.budget_warning =
                summary.total_tokens >= DEFAULT_STORY_TOKEN_WARN && summary.total_tokens > 0;
        }
        Err(e) => {
            log::warn!("[cost] 聚合 llm_calls 失败（story={}）: {}", story_id, e);
        }
    }
    summary
}

/// 计费盲区检测：最近 `lookback` 条调用里，从最新往回数连续零 token 的条数
/// 是否达到阈值。返回异常列表（空 = 正常）。
pub fn detect_cost_anomalies(pool: &DbPool, story_id: &str, lookback: usize) -> Vec<CostAnomaly> {
    let mut anomalies = Vec::new();
    let Ok(conn) = pool.get() else {
        return anomalies;
    };
    let mut stmt = match conn.prepare(
        "SELECT total_tokens FROM llm_calls WHERE story_id = ?1 \
         ORDER BY created_at DESC LIMIT ?2",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return anomalies,
    };
    let rows = stmt.query_map(params![story_id, lookback as i64], |row| {
        row.get::<_, i64>(0)
    });
    let Ok(rows) = rows else {
        return anomalies;
    };
    let tokens: Vec<i64> = rows.flatten().collect();
    if tokens.len() < ZERO_DELTA_STREAK_ALERT {
        return anomalies;
    }
    let streak = tokens.iter().take_while(|t| **t <= 0).count();
    if streak >= ZERO_DELTA_STREAK_ALERT {
        anomalies.push(CostAnomaly {
            kind: "zero_token_streak".to_string(),
            detail: format!(
                "最近连续 {streak} 次调用记账为 0 token——网关/模型未回报用量，\
                 任何 token 预算上限都不会触发，请检查模型配置或网关日志"
            ),
        });
    }
    anomalies
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed_story(pool: &DbPool) -> String {
        let story_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '成本', ?2, ?2)",
            params![&story_id, &now],
        )
        .unwrap();
        story_id
    }

    fn seed_call(pool: &DbPool, story_id: &str, tokens: i64, success: bool, minute: i64) {
        let conn = pool.get().unwrap();
        let created = (chrono::Local::now() + chrono::Duration::minutes(minute)).to_rfc3339();
        conn.execute(
            "INSERT INTO llm_calls \
             (id, story_id, model_id, purpose, prompt_tokens, completion_tokens, total_tokens, \
              duration_ms, success, created_at) \
             VALUES (?1, ?2, 'test-model', 'test', ?3, 0, ?3, 10, ?4, ?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                story_id,
                tokens,
                if success { 1 } else { 0 },
                created
            ],
        )
        .unwrap();
    }

    #[test]
    fn summary_aggregates_tokens_failures_and_range() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_call(&pool, &story_id, 1000, true, 0);
        seed_call(&pool, &story_id, 2000, true, 1);
        seed_call(&pool, &story_id, 0, false, 2);

        let summary = summarize_story_cost(&pool, &story_id);
        assert_eq!(summary.total_calls, 3);
        assert_eq!(summary.total_tokens, 3000);
        assert_eq!(summary.failed_calls, 1);
        assert_eq!(summary.zero_token_calls, 1);
        assert!(summary.first_call_at.is_some() && summary.last_call_at.is_some());
        assert!(!summary.budget_warning, "未超阈值不应告警");
    }

    #[test]
    fn budget_warning_triggers_above_threshold() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_call(
            &pool,
            &story_id,
            (DEFAULT_STORY_TOKEN_WARN + 1) as i64,
            true,
            0,
        );
        let summary = summarize_story_cost(&pool, &story_id);
        assert!(summary.budget_warning);
        assert_eq!(summary.warn_threshold, DEFAULT_STORY_TOKEN_WARN);
    }

    #[test]
    fn zero_token_streak_is_flagged_but_normal_usage_is_not() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        // 正常用量：不告警
        for i in 0..6 {
            seed_call(&pool, &story_id, 500, true, i);
        }
        assert!(detect_cost_anomalies(&pool, &story_id, 20).is_empty());

        // 最近连续 5 次零记账：告警
        for i in 6..11 {
            seed_call(&pool, &story_id, 0, true, i);
        }
        let anomalies = detect_cost_anomalies(&pool, &story_id, 20);
        assert_eq!(anomalies.len(), 1, "{anomalies:?}");
        assert_eq!(anomalies[0].kind, "zero_token_streak");
        assert!(anomalies[0].detail.contains("不会触发"));
    }

    #[test]
    fn unknown_story_yields_empty_summary() {
        let pool = create_test_pool().unwrap();
        let summary = summarize_story_cost(&pool, "missing-story");
        assert_eq!(summary.total_calls, 0);
        assert!(detect_cost_anomalies(&pool, "missing-story", 20).is_empty());
    }
}
