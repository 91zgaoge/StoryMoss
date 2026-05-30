//! 叙事线索追踪模型 — LitSeg 叙事感知分段与检索增强 (E1)
//!
//! 基于 LitSeg 论文：追踪人物弧光、伏笔/回报线、冲突升级等叙事线索，
//! 确保分段在叙事边界而非语义边界。

use serde::{Deserialize, Serialize};

// Re-export from existing db models
use crate::db::ConflictType;

/// 伏笔状态 — 与 db models 中的 ForeshadowingStatus 对齐（避免重复定义）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForeshadowStatus {
    Setup,     // 已埋设
    Payoff,    // 已回收
    Abandoned, // 已放弃
    Pending,   // 待处理
    Hinted,    // 暗示阶段
    PaidOff,   // 已回收（PayoffLedger 用）
    Failed,    // 失败（PayoffLedger 用）
    Overdue,   // 超期未回收（PayoffLedger 用）
}

impl Default for ForeshadowStatus {
    fn default() -> Self {
        ForeshadowStatus::Setup
    }
}

/// 弧光类型——角色内在转变的方向
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArcType {
    /// 正向弧光——角色从负面到正面（如恐惧→勇敢）
    Positive,
    /// 负向弧光——角色从正面到负面（如英雄→堕落）
    Negative,
    /// 扁平弧光——角色不变，世界改变
    Flat,
}

impl Default for ArcType {
    fn default() -> Self {
        ArcType::Positive
    }
}

/// 叙事线索——跨场景连续推进的叙事元素
#[derive(Debug, Clone)]
pub enum NarrativeThread {
    CharacterArc(CharacterArcThread),
    Foreshadow(ForeshadowThread),
    ConflictEscalation(ConflictEscalationThread),
}

/// 角色内在状态转换节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub chapter_number: i32,
    pub scene_id: Option<String>,
    pub from_state: String,
    pub to_state: String,
    /// 触发该转换的事件 ID（指向 NarrativeEvent）
    pub trigger_event_id: Option<String>,
    pub intensity: f32,
}

/// 人物弧光线程——角色内在转变的连续追踪
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterArcThread {
    pub id: String,
    pub story_id: String,
    pub character_id: String,
    /// 弧光类型（positive/negative/flat）
    pub arc_type: ArcType,
    /// 起点状态（角色初始内在状态）
    pub start_state: String,
    /// 当前状态（角色当前内在状态）
    pub current_state: String,
    /// 终点状态（角色弧光完成态，如已知）
    pub end_state: Option<String>,
    /// 状态转换节点列表（按时间顺序）
    pub state_transitions: Vec<StateTransition>,
    /// 当前进度（0.0-1.0）
    pub progress: f32,
}

/// 伏笔线程——与 PayoffLedger 联动，自动追踪
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeshadowThread {
    pub id: String,
    pub story_id: String,
    /// 埋设事件 ID（指向 NarrativeEvent）
    pub setup_event_id: Option<String>,
    /// 回收事件 ID（指向 NarrativeEvent）
    pub payoff_event_id: Option<String>,
    /// 伏笔内容描述
    pub content: String,
    /// 状态（与 PayoffLedger 对齐）
    pub status: ForeshadowStatus,
    /// 埋设场景编号
    pub setup_chapter: i32,
    /// 预期回收场景编号（如已知）
    pub target_chapter: Option<i32>,
    /// 实际回收场景编号
    pub payoff_chapter: Option<i32>,
    /// 风险信号强度（0.0-1.0，越高越可能成为烂尾伏笔）
    pub risk_signals: f32,
}

/// 冲突升级线程——矛盾从潜伏到爆发的演进追踪
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictEscalationThread {
    pub id: String,
    pub story_id: String,
    /// 冲突类型（与 Scene.conflict_type 对齐）
    pub conflict_type: ConflictType,
    /// 冲突方 A 的角色 ID
    pub party_a_ids: Vec<String>,
    /// 冲突方 B 的角色 ID
    pub party_b_ids: Vec<String>,
    /// 冲突强度演进（按章节记录）
    pub intensity_timeline: Vec<IntensityRecord>,
    /// 当前强度（0.0-1.0）
    pub current_intensity: f32,
    /// 是否已爆发
    pub is_escalated: bool,
}

/// 冲突强度时间线记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntensityRecord {
    pub chapter_number: i32,
    pub scene_id: Option<String>,
    pub intensity: f32,
    pub description: String,
}
