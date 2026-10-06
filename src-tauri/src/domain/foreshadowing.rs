#![allow(dead_code)]
//! 统一伏笔 / 线索 / 回报模型
//!
//! 被 creative_engine / narrative / commands 等模块共享，避免 narrative
//! 直接依赖 creative_engine，同时作为 story_system::ForeshadowingService
//! 的输入输出契约。

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

/// 伏笔 / 线索 / 回报领域错误。
///
///  deliberately kept inside `domain` so the foreshadowing model does not
///  depend on the application-level `error` module.
#[derive(Debug, Clone)]
pub enum ForeshadowingError {
    InvalidStatus(String),
    InvalidScope(String),
    InvalidImportance(String),
    NotFound { id: String },
    Internal(String),
}

impl std::fmt::Display for ForeshadowingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForeshadowingError::InvalidStatus(s) => write!(f, "未知伏笔状态: {}", s),
            ForeshadowingError::InvalidScope(s) => write!(f, "未知作用域类型: {}", s),
            ForeshadowingError::InvalidImportance(s) => write!(f, "无效重要性: {}", s),
            ForeshadowingError::NotFound { id } => write!(f, "伏笔未找到: {}", id),
            ForeshadowingError::Internal(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for ForeshadowingError {}

/// 伏笔状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForeshadowingStatus {
    /// 已设置，未回收
    Setup,
    /// 已回收
    Payoff,
    /// 已放弃
    Abandoned,
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

impl std::str::FromStr for ForeshadowingStatus {
    type Err = ForeshadowingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "setup" => Ok(ForeshadowingStatus::Setup),
            "payoff" => Ok(ForeshadowingStatus::Payoff),
            "abandoned" => Ok(ForeshadowingStatus::Abandoned),
            _ => Err(ForeshadowingError::InvalidStatus(s.to_string())),
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
    /// LitSeg: 叙事事件关联（从 narrative_threads.foreshadow 合并）
    pub setup_event_id: Option<String>,
    pub payoff_event_id: Option<String>,
    pub risk_signals_score: Option<f32>,
    pub status: ForeshadowingStatus,
    pub importance: i32, // 1-10
    pub created_at: String,
    pub resolved_at: Option<String>,
    /// v0.62.0 P2-C：计划回收窗口（场景序号区间，与 payoff_scene_id
    /// 的「实际回收」分离）
    pub target_start_scene: Option<i32>,
    pub target_end_scene: Option<i32>,
    /// v0.62.0 P2-C：埋设时的原文证据句（证据锚定）
    pub evidence: Option<String>,
}

impl ForeshadowingRecord {
    /// 判断该伏笔是否已解决（已回收或已放弃）。
    pub fn is_resolved(&self) -> bool {
        matches!(
            self.status,
            ForeshadowingStatus::Payoff | ForeshadowingStatus::Abandoned
        )
    }

    /// 判断该伏笔是否已逾期。
    ///
    /// 仅对 `Setup` 状态的伏笔生效。由于 `ForeshadowingRecord` 不携带场景序号，
    /// 此处使用创建时间与 `now` 的时间差，按重要性分级判断：
    /// - 重要性 8-10：超过 5 天视为逾期
    /// - 重要性 5-7：超过 10 天视为逾期
    /// - 重要性 1-4：超过 15 天视为逾期
    ///
    /// 若 `created_at` 无法解析，保守返回 `false`。
    pub fn is_overdue(&self, now: DateTime<Utc>) -> bool {
        if !matches!(self.status, ForeshadowingStatus::Setup) {
            return false;
        }
        let Ok(created_at) = self.created_at.parse::<DateTime<Utc>>() else {
            return false;
        };
        if created_at > now {
            return false;
        }
        let threshold_days = match self.importance {
            8..=10 => 5,
            5..=7 => 10,
            _ => 15,
        };
        let threshold = TimeDelta::days(threshold_days);
        now - created_at > threshold
    }
}

/// 伏笔作用域类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeType {
    /// 全故事级伏笔
    Story,
    /// 故事弧级伏笔
    Arc,
    /// 单场景级伏笔
    Scene,
}

impl std::fmt::Display for ScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeType::Story => write!(f, "story"),
            ScopeType::Arc => write!(f, "arc"),
            ScopeType::Scene => write!(f, "scene"),
        }
    }
}

impl std::str::FromStr for ScopeType {
    type Err = ForeshadowingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "story" => Ok(ScopeType::Story),
            "arc" => Ok(ScopeType::Arc),
            "scene" => Ok(ScopeType::Scene),
            _ => Err(ForeshadowingError::InvalidScope(s.to_string())),
        }
    }
}

/// 账本状态（比 DB 层更丰富的状态机）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayoffStatus {
    /// 已设置，尚未有进一步暗示
    Setup,
    /// 已有暗示/呼应
    Hinted,
    /// 临近回收窗口
    PendingPayoff,
    /// 已回收
    PaidOff,
    /// 已放弃/失效
    Failed,
    /// 已逾期
    Overdue,
}

impl std::fmt::Display for PayoffStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayoffStatus::Setup => write!(f, "setup"),
            PayoffStatus::Hinted => write!(f, "hinted"),
            PayoffStatus::PendingPayoff => write!(f, "pending_payoff"),
            PayoffStatus::PaidOff => write!(f, "paid_off"),
            PayoffStatus::Failed => write!(f, "failed"),
            PayoffStatus::Overdue => write!(f, "overdue"),
        }
    }
}

impl From<ForeshadowingStatus> for PayoffStatus {
    fn from(status: ForeshadowingStatus) -> Self {
        match status {
            ForeshadowingStatus::Setup => PayoffStatus::Setup,
            ForeshadowingStatus::Payoff => PayoffStatus::PaidOff,
            ForeshadowingStatus::Abandoned => PayoffStatus::Failed,
        }
    }
}

/// 紧急程度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrgencyLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for UrgencyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UrgencyLevel::Low => write!(f, "low"),
            UrgencyLevel::Medium => write!(f, "medium"),
            UrgencyLevel::High => write!(f, "high"),
            UrgencyLevel::Critical => write!(f, "critical"),
        }
    }
}

/// 回收时机推荐
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoffRecommendation {
    pub foreshadowing_id: String,
    pub ledger_key: String,
    pub title: String,
    pub recommended_scene: i32,
    pub urgency: UrgencyLevel,
    pub reason: String,
    pub importance: i32,
}

/// 伏笔账本条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoffLedgerItem {
    pub id: String,
    pub ledger_key: String,
    pub title: String,
    pub summary: String,
    pub scope_type: ScopeType,
    pub current_status: PayoffStatus,
    pub target_start_scene: Option<i32>,
    pub target_end_scene: Option<i32>,
    pub first_seen_scene: Option<i32>,
    pub last_touched_scene: Option<i32>,
    pub confidence: f32,
    pub risk_signals: Vec<String>,
    pub importance: i32,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

/// 回报（payoff）引用
#[derive(Debug, Clone)]
pub struct Payoff {
    pub foreshadowing_id: String,
    pub content: String,
    pub importance: i32,
    pub setup_scene_id: Option<String>,
    pub payoff_scene_id: Option<String>,
    pub status: ForeshadowingStatus,
}

/// 叙事线索（thread）摘要
#[derive(Debug, Clone)]
pub struct Thread {
    pub id: String,
    pub story_id: String,
    pub thread_type: ThreadType,
    pub content: String,
    pub status: ForeshadowingStatus,
    pub setup_scene_id: Option<String>,
    pub payoff_scene_id: Option<String>,
    pub importance: i32,
}

/// 线索类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadType {
    Foreshadow,
    CharacterArc,
    ConflictEscalation,
}

/// 伏笔查询端口，供 narrative 等模块在不依赖 creative_engine 的情况下读取伏笔。
pub trait ForeshadowingProvider: Send + Sync {
    fn list_by_story(&self, story_id: &str)
        -> Result<Vec<ForeshadowingRecord>, ForeshadowingError>;
    fn get_by_id(&self, id: &str) -> Result<Option<ForeshadowingRecord>, ForeshadowingError>;
    fn get_unresolved(
        &self,
        story_id: &str,
    ) -> Result<Vec<ForeshadowingRecord>, ForeshadowingError>;
    fn get_overdue(
        &self,
        story_id: &str,
        current_scene_number: i32,
    ) -> Result<Vec<ForeshadowingRecord>, ForeshadowingError>;
    fn get_writing_hints(
        &self,
        story_id: &str,
        limit: usize,
    ) -> Result<Vec<String>, ForeshadowingError>;
    fn detect_payoffs(&self, story_id: &str) -> Result<Vec<Payoff>, ForeshadowingError>;
    fn recommend_payoffs(
        &self,
        story_id: &str,
        current_scene_number: i32,
    ) -> Result<Vec<PayoffRecommendation>, ForeshadowingError>;
    fn get_ledger(&self, story_id: &str) -> Result<Vec<PayoffLedgerItem>, ForeshadowingError>;

    /// 兼容旧称，等价于 `list_by_story`。
    fn get_all(&self, story_id: &str) -> Result<Vec<ForeshadowingRecord>, ForeshadowingError> {
        self.list_by_story(story_id)
    }
}

/// 伏笔服务端口（读写），作为单一真源契约。
pub trait ForeshadowingService: ForeshadowingProvider + Send + Sync {
    fn create(
        &self,
        story_id: &str,
        content: &str,
        setup_scene_id: Option<&str>,
        importance: i32,
    ) -> Result<String, ForeshadowingError>;
    fn mark_payoff(
        &self,
        foreshadowing_id: &str,
        payoff_scene_id: Option<&str>,
    ) -> Result<(), ForeshadowingError>;
    fn abandon(&self, foreshadowing_id: &str) -> Result<(), ForeshadowingError>;
    fn update(
        &self,
        foreshadowing_id: &str,
        content: &str,
        importance: i32,
        setup_scene_id: Option<&str>,
    ) -> Result<(), ForeshadowingError>;
    fn delete(&self, foreshadowing_id: &str) -> Result<(), ForeshadowingError>;
    fn update_ledger_fields(
        &self,
        foreshadowing_id: &str,
        target_start_scene: Option<i32>,
        target_end_scene: Option<i32>,
        risk_signals: Option<Vec<String>>,
        scope_type: Option<ScopeType>,
        ledger_key: Option<String>,
        setup_event_id: Option<String>,
        payoff_event_id: Option<String>,
        risk_signals_score: Option<f32>,
    ) -> Result<(), ForeshadowingError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record_with(status: ForeshadowingStatus, created_at: DateTime<Utc>) -> ForeshadowingRecord {
        ForeshadowingRecord {
            id: "fs-1".to_string(),
            story_id: "story-1".to_string(),
            content: "一把神秘的钥匙".to_string(),
            setup_scene_id: None,
            payoff_scene_id: None,
            setup_event_id: None,
            payoff_event_id: None,
            risk_signals_score: None,
            status,
            importance: 5,
            created_at: created_at.to_rfc3339(),
            resolved_at: None,
            target_start_scene: None,
            target_end_scene: None,
            evidence: None,
        }
    }

    #[test]
    fn payoff_and_abandoned_are_resolved() {
        let now = Utc::now();
        assert!(record_with(ForeshadowingStatus::Payoff, now).is_resolved());
        assert!(record_with(ForeshadowingStatus::Abandoned, now).is_resolved());
    }

    #[test]
    fn setup_is_not_resolved() {
        let now = Utc::now();
        assert!(!record_with(ForeshadowingStatus::Setup, now).is_resolved());
    }

    #[test]
    fn setup_becomes_overdue_after_threshold() {
        let now = Utc::now();
        let just_setup = record_with(ForeshadowingStatus::Setup, now - TimeDelta::days(9));
        assert!(!just_setup.is_overdue(now));

        let overdue = record_with(ForeshadowingStatus::Setup, now - TimeDelta::days(11));
        assert!(overdue.is_overdue(now));
    }

    #[test]
    fn resolved_records_are_never_overdue() {
        let now = Utc::now();
        let old_payoff = record_with(ForeshadowingStatus::Payoff, now - TimeDelta::days(100));
        assert!(!old_payoff.is_overdue(now));

        let old_abandoned = record_with(ForeshadowingStatus::Abandoned, now - TimeDelta::days(100));
        assert!(!old_abandoned.is_overdue(now));
    }

    #[test]
    fn overdue_threshold_respects_importance() {
        let now = Utc::now();
        let mut high = record_with(ForeshadowingStatus::Setup, now - TimeDelta::days(6));
        high.importance = 9;
        assert!(high.is_overdue(now));

        let mut low = record_with(ForeshadowingStatus::Setup, now - TimeDelta::days(6));
        low.importance = 3;
        assert!(!low.is_overdue(now));

        low.created_at = (now - TimeDelta::days(16)).to_rfc3339();
        assert!(low.is_overdue(now));
    }

    #[test]
    fn future_created_at_is_not_overdue() {
        let now = Utc::now();
        let future = record_with(ForeshadowingStatus::Setup, now + TimeDelta::days(1));
        assert!(!future.is_overdue(now));
    }

    #[test]
    fn unparseable_created_at_is_not_overdue() {
        let now = Utc::now();
        let mut record = record_with(ForeshadowingStatus::Setup, now);
        record.created_at = "not-a-date".to_string();
        assert!(!record.is_overdue(now));
    }

    #[test]
    fn status_round_trip_from_str() {
        assert_eq!(
            "setup".parse::<ForeshadowingStatus>().unwrap(),
            ForeshadowingStatus::Setup
        );
        assert_eq!(
            "payoff".parse::<ForeshadowingStatus>().unwrap(),
            ForeshadowingStatus::Payoff
        );
        assert_eq!(
            "abandoned".parse::<ForeshadowingStatus>().unwrap(),
            ForeshadowingStatus::Abandoned
        );
        assert!("unknown".parse::<ForeshadowingStatus>().is_err());
    }

    #[test]
    fn scope_type_round_trip_from_str() {
        assert_eq!("story".parse::<ScopeType>().unwrap(), ScopeType::Story);
        assert_eq!("arc".parse::<ScopeType>().unwrap(), ScopeType::Arc);
        assert_eq!("scene".parse::<ScopeType>().unwrap(), ScopeType::Scene);
        assert!("unknown".parse::<ScopeType>().is_err());
    }

    #[test]
    fn foreshadowing_to_payoff_status_mapping() {
        assert_eq!(
            PayoffStatus::from(ForeshadowingStatus::Setup),
            PayoffStatus::Setup
        );
        assert_eq!(
            PayoffStatus::from(ForeshadowingStatus::Payoff),
            PayoffStatus::PaidOff
        );
        assert_eq!(
            PayoffStatus::from(ForeshadowingStatus::Abandoned),
            PayoffStatus::Failed
        );
    }

    #[test]
    fn urgency_level_display() {
        assert_eq!(UrgencyLevel::Critical.to_string(), "critical");
        assert_eq!(UrgencyLevel::Low.to_string(), "low");
    }
}
