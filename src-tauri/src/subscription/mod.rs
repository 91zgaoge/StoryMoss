//! Subscription Service — 功能订阅制
//!
//! v0.7.3: 商业模式重构完成。软件订阅制，模型使用完全由用户决定，软件不介入模型计费。
//! 订阅层级仅用于功能开关控制（Free/Pro/Enterprise），不再计量模型消费配额。

use crate::db::DbPool;
use crate::error::AppError;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

pub mod commands;

/// 订阅层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionTier {
    Free,
    Pro,
    Enterprise,
}

impl std::fmt::Display for SubscriptionTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionTier::Free => write!(f, "free"),
            SubscriptionTier::Pro => write!(f, "pro"),
            SubscriptionTier::Enterprise => write!(f, "enterprise"),
        }
    }
}

impl std::str::FromStr for SubscriptionTier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "free" => Ok(SubscriptionTier::Free),
            "pro" => Ok(SubscriptionTier::Pro),
            "enterprise" => Ok(SubscriptionTier::Enterprise),
            _ => Err(format!("Unknown subscription tier: {}", s)),
        }
    }
}

/// 订阅状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionStatus {
    pub user_id: String,
    pub tier: String,
    pub status: String,
    pub expires_at: Option<String>,
}

/// 订阅服务
pub struct SubscriptionService {
    pool: DbPool,
}

impl SubscriptionService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 获取或创建默认订阅状态
    pub fn get_or_create_subscription(&self, user_id: &str) -> Result<SubscriptionStatus, AppError> {
        let conn = self.pool.get()?;

        let existing: Option<(String, String, Option<String>)> = conn
            .query_row(
                "SELECT tier, status, expires_at FROM subscriptions WHERE user_id = ?1 ORDER BY created_at DESC LIMIT 1",
                params![user_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let (tier, status, expires_at) = if let Some((tier, status, expires)) = existing {
            (tier, status, expires)
        } else {
            let now = chrono::Local::now().to_rfc3339();
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO subscriptions (id, user_id, tier, status, started_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5)",
                params![id, user_id, "free", "active", now],
            )?;
            ("free".to_string(), "active".to_string(), None)
        };

        Ok(SubscriptionStatus {
            user_id: user_id.to_string(),
            tier,
            status,
            expires_at,
        })
    }

    /// 检查用户是否有权使用指定功能（v0.7.3：订阅解锁功能，非模型配额）
    ///
    /// 细粒度功能权限映射：
    /// - Free 用户可用：基础写作、场景管理、角色管理、知识图谱查询
    /// - Pro 用户解锁：Bootstrap / Pipeline（Refine/Review/Finalize）/ 拆书 / 自动续写 / 自动修改
    pub fn has_feature_access(&self, user_id: &str, feature_id: &str) -> Result<bool, AppError> {
        let status = self.get_or_create_subscription(user_id)?;
        let is_pro = status.tier == "pro" || status.tier == "enterprise";

        // Free 用户可用的基础功能
        let free_features = [
            "writer",
            "scene_management",
            "character_management",
            "knowledge_graph_query",
            "outline",
        ];

        if free_features.contains(&feature_id) {
            return Ok(true);
        }

        // 其余功能需要 Pro 订阅
        Ok(is_pro)
    }

    /// 记录 AI 调用日志（仅用于统计，不参与配额控制）
    pub fn log_ai_usage(
        &self,
        user_id: &str,
        story_id: Option<&str>,
        chapter_id: Option<&str>,
        agent_type: &str,
        instruction: Option<&str>,
        prompt_tokens: Option<i32>,
        completion_tokens: Option<i32>,
        model_used: Option<&str>,
        cost: Option<f64>,
        duration_ms: Option<i32>,
        tier_at_time: &str,
    ) -> Result<(), AppError> {
        let conn = self.pool.get()?;
        let id = uuid::Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO ai_usage_logs (id, user_id, story_id, chapter_id, agent_type, instruction, prompt_tokens, completion_tokens, model_used, cost, duration_ms, tier_at_time, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                id,
                user_id,
                story_id,
                chapter_id,
                agent_type,
                instruction,
                prompt_tokens,
                completion_tokens,
                model_used,
                cost,
                duration_ms,
                tier_at_time,
                chrono::Local::now().to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// 升级订阅（模拟，实际应对接支付系统）
    pub fn upgrade_subscription(&self, user_id: &str, tier: &str, expires_days: Option<i32>) -> Result<SubscriptionStatus, AppError> {
        let conn = self.pool.get()?;
        let now = chrono::Local::now();
        let id = uuid::Uuid::new_v4().to_string();
        let expires_at = expires_days.map(|d| (now + chrono::Duration::days(d as i64)).to_rfc3339());

        conn.execute(
            "INSERT INTO subscriptions (id, user_id, tier, status, started_at, expires_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, ?5)",
            params![id, user_id, tier, "active", now.to_rfc3339(), expires_at],
        )?;

        self.get_or_create_subscription(user_id)
    }
}
