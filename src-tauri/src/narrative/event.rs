//! 叙事事件模型 — LitSeg 叙事感知分段与检索增强 (E1)
//!
//! 基于 LitSeg 论文：从文本中提取"有效事件"——推动情节发展的关键节点，
//! 而非简单的语义分割点。每个事件携带强度、情感、因果链等信息。

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

// Re-export ConflictType from db models
use crate::db::ConflictType;

/// 叙事事件类型——推动情节发展的关键节点
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum EventType {
    /// 开场/引入——引入角色或世界观
    Introduction,
    /// 转折点——局势质变
    TurningPoint,
    /// 高潮——冲突顶点
    Climax,
    /// 回落——紧张缓解
    Resolution,
    /// 揭示——真相揭露
    Revelation,
    /// 冲突爆发——矛盾升级
    ConflictEruption,
    /// 角色弧光节点——内在改变
    CharacterArc,
    /// 伏笔埋设——为后续回报做铺垫
    ForeshadowSetup,
    /// 伏笔回收——伏笔得到回报
    ForeshadowPayoff,
    /// 过渡——连接功能
    #[default]
    Transition,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EventType::Introduction => "开场/引入",
            EventType::TurningPoint => "转折点",
            EventType::Climax => "高潮",
            EventType::Resolution => "回落",
            EventType::Revelation => "揭示",
            EventType::ConflictEruption => "冲突爆发",
            EventType::CharacterArc => "角色弧光",
            EventType::ForeshadowSetup => "伏笔埋设",
            EventType::ForeshadowPayoff => "伏笔回收",
            EventType::Transition => "过渡",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for EventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "introduction" | "开场/引入" => Ok(EventType::Introduction),
            "turning_point" | "转折点" => Ok(EventType::TurningPoint),
            "climax" | "高潮" => Ok(EventType::Climax),
            "resolution" | "回落" => Ok(EventType::Resolution),
            "revelation" | "揭示" => Ok(EventType::Revelation),
            "conflict_eruption" | "冲突爆发" => Ok(EventType::ConflictEruption),
            "character_arc" | "角色弧光" => Ok(EventType::CharacterArc),
            "foreshadow_setup" | "伏笔埋设" => Ok(EventType::ForeshadowSetup),
            "foreshadow_payoff" | "伏笔回收" => Ok(EventType::ForeshadowPayoff),
            "transition" | "过渡" => Ok(EventType::Transition),
            _ => Err(format!("Unknown event type: {}", s)),
        }
    }
}

/// 叙事事件——推动情节的关键节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeEvent {
    pub id: String,
    pub story_id: String,
    pub chapter_number: i32,
    pub scene_id: Option<String>,
    /// 事件类型
    pub event_type: EventType,
    /// 事件强度（0.0-1.0），越高表示对情节推动越大
    pub intensity: f32,
    /// 情感极性（-1.0 = 负面, 0.0 = 中性, +1.0 = 正面）
    pub sentiment: f32,
    /// 事件描述（自然语言摘要）
    pub description: String,
    /// 涉及的角色 ID
    pub involved_character_ids: Vec<String>,
    /// 涉及的冲突类型
    pub conflict_types: Vec<ConflictType>,
    /// 前置事件（因果链）
    pub preceding_event_id: Option<String>,
    /// 后续事件
    pub following_event_id: Option<String>,
    /// 所属叙事幕（act_number 1-5）
    pub act_number: i32,
    /// 在幕中的位置（1=start, 2=mid, 3=end）
    pub position_in_act: i32,
    /// 时间戳
    pub created_at: DateTime<Local>,
}

impl Default for NarrativeEvent {
    fn default() -> Self {
        Self {
            id: String::new(),
            story_id: String::new(),
            chapter_number: 0,
            scene_id: None,
            event_type: EventType::default(),
            intensity: 0.5,
            sentiment: 0.0,
            description: String::new(),
            involved_character_ids: Vec::new(),
            conflict_types: Vec::new(),
            preceding_event_id: None,
            following_event_id: None,
            act_number: 1,
            position_in_act: 1,
            created_at: Local::now(),
        }
    }
}
