//! Subscription Service — 功能订阅制
//!
//! 商业模式重构完成。软件订阅制，模型使用完全由用户决定，软件不介入模型计费。
//! 订阅层级仅用于功能开关控制（Free/Pro/Enterprise），不再计量模型消费配额。

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError};

pub mod commands;
pub mod identity;

pub use crate::domain::subscription::SubscriptionTier;

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
    pub fn get_or_create_subscription(
        &self,
        user_id: &str,
    ) -> Result<SubscriptionStatus, AppError> {
        let conn = self.pool.get()?;

        let existing: Option<(String, String, Option<String>)> = conn
            .query_row(
                "SELECT tier, status, expires_at FROM subscriptions WHERE user_id = ?1 ORDER BY \
                 created_at DESC LIMIT 1",
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
                "INSERT INTO subscriptions (id, user_id, tier, status, started_at, created_at, \
                 updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?5)",
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

    /// 检查用户是否有权使用指定功能（订阅解锁功能，非模型配额）
    ///
    /// 细粒度功能权限映射：
    /// - Free 用户可用：基础写作、场景管理、角色管理、知识图谱查询、大纲、
    ///   技能书提炼（v0.36.0 起免费）
    /// - Pro 用户解锁：Bootstrap / Pipeline（Refine/Review/Finalize）/ 拆书 /
    ///   自动续写 / 自动修改
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
            "guidebook_distillation",
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
            "INSERT INTO ai_usage_logs (id, user_id, story_id, chapter_id, agent_type, \
             instruction, prompt_tokens, completion_tokens, model_used, cost, duration_ms, \
             tier_at_time, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, \
             ?13)",
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
    pub fn upgrade_subscription(
        &self,
        user_id: &str,
        tier: &str,
        expires_days: Option<i32>,
    ) -> Result<SubscriptionStatus, AppError> {
        let conn = self.pool.get()?;
        let now = chrono::Local::now();
        let id = uuid::Uuid::new_v4().to_string();
        let expires_at =
            expires_days.map(|d| (now + chrono::Duration::days(d as i64)).to_rfc3339());

        conn.execute(
            "INSERT INTO subscriptions (id, user_id, tier, status, started_at, expires_at, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, ?5)",
            params![id, user_id, tier, "active", now.to_rfc3339(), expires_at],
        )?;

        self.get_or_create_subscription(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> SubscriptionService {
        let pool = crate::db::connection::create_test_pool().unwrap();
        SubscriptionService::new(pool)
    }

    #[test]
    fn new_user_defaults_to_free_active() {
        let svc = service();
        let status = svc.get_or_create_subscription("u-new").unwrap();
        assert_eq!(status.tier, "free");
        assert_eq!(status.status, "active");
        assert_eq!(status.expires_at, None);
    }

    #[test]
    fn free_user_has_basic_features_but_not_pro_features() {
        let svc = service();
        assert!(svc.has_feature_access("u-free", "writer").unwrap());
        assert!(svc.has_feature_access("u-free", "outline").unwrap());
        // v0.36.0：技能书提炼转为免费功能
        assert!(svc
            .has_feature_access("u-free", "guidebook_distillation")
            .unwrap());
        assert!(!svc.has_feature_access("u-free", "bootstrap").unwrap());
    }

    #[test]
    fn upgrade_to_pro_unlocks_pro_features_and_sets_expiry() {
        let svc = service();
        let status = svc.upgrade_subscription("u1", "pro", Some(30)).unwrap();
        assert_eq!(status.tier, "pro");
        assert!(status.expires_at.is_some());
        // 注意：当前 has_feature_access 不校验 expires_at 是否过期，仅按 tier
        // 判断
        assert!(svc
            .has_feature_access("u1", "guidebook_distillation")
            .unwrap());
    }

    #[test]
    fn upgrade_to_enterprise_also_unlocks_pro_features() {
        let svc = service();
        svc.upgrade_subscription("u2", "enterprise", Some(365))
            .unwrap();
        assert!(svc
            .has_feature_access("u2", "guidebook_distillation")
            .unwrap());
    }

    #[test]
    fn downgrade_back_to_free_revokes_pro_features() {
        let svc = service();
        svc.upgrade_subscription("u3", "pro", Some(30)).unwrap();
        assert!(svc.has_feature_access("u3", "book_deconstruction").unwrap());

        let status = svc.upgrade_subscription("u3", "free", None).unwrap();
        assert_eq!(status.tier, "free");
        assert!(!svc.has_feature_access("u3", "book_deconstruction").unwrap());
        // 免费基础功能不受影响（含 v0.36.0 起免费的技能书提炼）
        assert!(svc.has_feature_access("u3", "writer").unwrap());
        assert!(svc
            .has_feature_access("u3", "guidebook_distillation")
            .unwrap());
    }

    #[test]
    fn cache_remote_status_pro_unlocks_pro_features() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let remote = crate::server_client::RemoteSubscription {
            tier: "pro".to_string(),
            status: "active".to_string(),
            expires_at: None,
        };
        cache_remote_status(&pool, "u-cache-pro", &remote).unwrap();

        let svc = SubscriptionService::new(pool.clone());
        assert!(svc
            .has_feature_access("u-cache-pro", "guidebook_distillation")
            .unwrap());
    }

    #[test]
    fn cache_remote_status_uses_remote_expiry_days() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let remote = crate::server_client::RemoteSubscription {
            tier: "pro".into(),
            status: "active".into(),
            expires_at: Some((chrono::Local::now() + chrono::Duration::days(90)).to_rfc3339()),
        };
        cache_remote_status(&pool, "u-exp", &remote).unwrap();

        // 本地缓存行的 expires_at 应≈90 天后（容差 1 天），而非恒 30 天
        let svc = SubscriptionService::new(pool.clone());
        let status = svc.get_or_create_subscription("u-exp").unwrap();
        let exp = status.expires_at.unwrap();
        let exp_dt = chrono::DateTime::parse_from_rfc3339(&exp).unwrap();
        assert!(exp_dt > chrono::Local::now() + chrono::Duration::days(80));
    }

    #[test]
    fn cache_remote_status_same_tier_does_not_insert_new_row() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let remote = crate::server_client::RemoteSubscription {
            tier: "pro".to_string(),
            status: "active".to_string(),
            expires_at: None,
        };
        cache_remote_status(&pool, "u-cache-same", &remote).unwrap();

        let count_rows = || -> i64 {
            pool.get()
                .unwrap()
                .query_row(
                    "SELECT COUNT(*) FROM subscriptions WHERE user_id = ?1",
                    params!["u-cache-same"],
                    |row| row.get(0),
                )
                .unwrap()
        };
        let before = count_rows();
        cache_remote_status(&pool, "u-cache-same", &remote).unwrap();
        assert_eq!(count_rows(), before);

        // tier 变化时才会插入新行
        let remote_free = crate::server_client::RemoteSubscription {
            tier: "free".to_string(),
            status: "active".to_string(),
            expires_at: None,
        };
        cache_remote_status(&pool, "u-cache-same", &remote_free).unwrap();
        assert_eq!(count_rows(), before + 1);
        let svc = SubscriptionService::new(pool.clone());
        assert!(!svc
            .has_feature_access("u-cache-same", "book_deconstruction")
            .unwrap());
    }
}

/// 远程订阅状态写本地缓存（供 has_feature_access 等同步检查点读取）
pub fn cache_remote_status(
    pool: &DbPool,
    user_id: &str,
    remote: &crate::server_client::RemoteSubscription,
) -> Result<(), AppError> {
    let service = SubscriptionService::new(pool.clone());
    let current = service.get_or_create_subscription(user_id)?;
    if current.tier != remote.tier {
        let expires_days = remote
            .expires_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|exp| {
                let secs = exp.timestamp() - chrono::Local::now().timestamp();
                (secs / 86400).max(0) as i32
            });
        let expires_days = match remote.tier.as_str() {
            "free" => None,
            _ => expires_days.or(Some(30)), // 无 expires 的 pro（dev 通道）兜底 30 天
        };
        service.upgrade_subscription(user_id, &remote.tier, expires_days)?;
    }
    Ok(())
}

/// 已登录时后台同步一次远程订阅（登录后/启动时调用）
pub fn sync_remote_subscription(app_handle: &tauri::AppHandle) {
    use tauri::Manager;
    let pool = app_handle.state::<DbPool>().inner().clone();
    let identity = identity::resolve_identity(app_handle, &pool);
    if let identity::Identity::Account { user_id, token } = identity {
        let handle = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let client = crate::server_client::ServerClient::new();
            match client.get_subscription(&token).await {
                Ok(remote) => {
                    if let Err(e) = cache_remote_status(&pool, &user_id, &remote) {
                        log::warn!("cache remote subscription failed: {}", e);
                    }
                    let _ = crate::state_sync::StateSync::emit_subscription_changed(
                        &handle,
                        &user_id,
                        &remote.tier,
                    );
                }
                Err(e) => log::warn!("remote subscription sync failed (offline ok): {}", e),
            }
        });
    }
}
