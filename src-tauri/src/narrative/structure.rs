//! 叙事结构定位模型 — LitSeg 叙事感知分段与检索增强 (E1)
//!
//! 基于 LitSeg 论文：识别叙事结构转折点——如亚里士多德的"发现/逆转"、
//! 弗莱塔格的"上升-高潮-下降"。这是 StoryForge 最大的缺失模块。

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// 叙事幕——宏观叙事结构单元
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ActType {
    /// 起——铺垫与引入（Act 1）
    #[default]
    Introduction,
    /// 承——发展与冲突升级（Act 2a）
    Development,
    /// 转——转折点/高潮前奏（Act 2b / Midpoint）
    Turn,
    /// 合——高潮与收束（Act 3）
    Resolution,
}

impl std::fmt::Display for ActType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ActType::Introduction => "起",
            ActType::Development => "承",
            ActType::Turn => "转",
            ActType::Resolution => "合",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for ActType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "introduction" | "起" => Ok(ActType::Introduction),
            "development" | "承" => Ok(ActType::Development),
            "turn" | "转" => Ok(ActType::Turn),
            "resolution" | "合" => Ok(ActType::Resolution),
            _ => Err(format!("Unknown act type: {}", s)),
        }
    }
}


/// 亚里士多德式戏剧功能
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum DramaticFunction {
    /// 开端——引入冲突
    Prologue,
    /// 发展——冲突升级
    RisingAction,
    /// 高潮——冲突顶点
    Climax,
    /// 回落——紧张缓解
    FallingAction,
    /// 结局——冲突解决
    Catastrophe,
    /// 转折点——局势逆转
    Peripeteia,
    /// 发现——真相揭露
    Anagnorisis,
    /// 过渡——连接功能
    #[default]
    Transition,
}

impl std::fmt::Display for DramaticFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            DramaticFunction::Prologue => "开端",
            DramaticFunction::RisingAction => "发展",
            DramaticFunction::Climax => "高潮",
            DramaticFunction::FallingAction => "回落",
            DramaticFunction::Catastrophe => "结局",
            DramaticFunction::Peripeteia => "逆转",
            DramaticFunction::Anagnorisis => "发现",
            DramaticFunction::Transition => "过渡",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for DramaticFunction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "prologue" | "开端" => Ok(DramaticFunction::Prologue),
            "rising_action" | "发展" => Ok(DramaticFunction::RisingAction),
            "climax" | "高潮" => Ok(DramaticFunction::Climax),
            "falling_action" | "回落" => Ok(DramaticFunction::FallingAction),
            "catastrophe" | "结局" => Ok(DramaticFunction::Catastrophe),
            "peripeteia" | "逆转" => Ok(DramaticFunction::Peripeteia),
            "anagnorisis" | "发现" => Ok(DramaticFunction::Anagnorisis),
            "transition" | "过渡" => Ok(DramaticFunction::Transition),
            _ => Err(format!("Unknown dramatic function: {}", s)),
        }
    }
}


/// 叙事结构定位——事件在宏观叙事中的位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeStructurePosition {
    pub event_id: String,
    /// 所属幕（1-5）
    pub act_number: i32,
    /// 幕类型（起/承/转/合）
    pub act_type: ActType,
    /// 在幕中的相对位置（0.0-1.0）
    pub position_in_act: f32,
    /// 戏剧功能（亚里士多德式）
    pub dramatic_function: DramaticFunction,
    /// 是否在叙事边界上——LitSeg 的核心判断
    pub is_narrative_boundary: bool,
}

/// 叙事幕级划分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Act {
    pub act_number: i32,
    pub act_type: ActType,
    /// 起始章节编号
    pub start_chapter: i32,
    /// 结束章节编号
    pub end_chapter: i32,
}

/// 完整的叙事结构——包含所有幕的划分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeStructure {
    pub story_id: String,
    pub acts: Vec<Act>,
    pub created_at: DateTime<Local>,
}
