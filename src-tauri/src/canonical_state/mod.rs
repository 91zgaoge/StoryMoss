//! Canonical State - 规范状态系统
//!
//! 提供统一的故事状态快照，整合分散在各模块中的状态信息，
//! 为 AI 续写提供准确的"当前处于故事哪个阶段"、"有哪些伏笔需要兑现"等上下文。

pub mod manager;

pub use manager::CanonicalStateManager;
use serde::{Deserialize, Serialize};

/// 规范状态快照 - 故事的完整当前状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalStateSnapshot {
    pub story_id: String,
    pub story_context: StoryContext,
    pub character_states: Vec<CharacterStateSnapshot>,
    pub world_facts: Vec<WorldFact>,
    pub timeline: Vec<TimelineEvent>,
    pub narrative_phase: NarrativePhase,
    pub generated_at: String, // ISO 8601
}

/// 故事上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryContext {
    pub current_scene_id: Option<String>,
    pub active_conflicts: Vec<Conflict>,
    pub pending_payoffs: Vec<PayoffRef>,
    pub overdue_payoffs: Vec<PayoffRef>,
}

/// 角色状态快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterStateSnapshot {
    pub character_id: String,
    pub name: String,
    pub current_location: Option<String>,
    pub current_emotion: Option<String>,
    pub active_goal: Option<String>,
    pub secrets_known: Vec<String>,
    pub secrets_unknown: Vec<String>,
    pub arc_progress: f32,
}

/// 世界观事实
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldFact {
    pub fact_type: String, // "rule" | "culture" | "history" | "setting"
    pub content: String,
    pub importance: i32,
}

/// 时间线事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub sequence_number: i32,
    pub scene_id: Option<String>,
    pub event_summary: String,
    pub timestamp: Option<String>,
}

/// 活跃冲突
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub conflict_type: String,
    pub parties: Vec<String>,
    pub stakes: String,
}

/// 待回收伏笔引用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoffRef {
    pub foreshadowing_id: String,
    pub content: String,
    pub importance: i32,
    pub setup_scene_id: Option<String>,
}

/// 叙事阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NarrativePhase {
    Setup,
    Rising,
    ConflictActive,
    Climax,
    Falling,
    Resolution,
}

impl std::fmt::Display for NarrativePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            NarrativePhase::Setup => "铺垫期",
            NarrativePhase::Rising => "上升期",
            NarrativePhase::ConflictActive => "冲突激化期",
            NarrativePhase::Climax => "高潮期",
            NarrativePhase::Falling => "回落期",
            NarrativePhase::Resolution => "收尾期",
        };
        write!(f, "{}", s)
    }
}

impl NarrativePhase {
    /// 返回该叙事阶段对 Writer Agent 的指导语
    pub fn writer_guidance(&self) -> &'static str {
        match self {
            NarrativePhase::Setup => {
                "当前叙事阶段：铺垫期。请专注于建立世界观、介绍角色、埋下伏笔，保持节奏舒缓，\
                 为后续冲突做铺垫。"
            }
            NarrativePhase::Rising => {
                "当前叙事阶段：上升期。请逐步升级冲突，增加紧张感，推动角色面对更大的挑战，\
                 保持情节推进动力。"
            }
            NarrativePhase::ConflictActive => {
                "当前叙事阶段：冲突激化期。冲突已达到临界点，请加快节奏，让矛盾集中爆发，\
                 优先处理逾期伏笔的回收。"
            }
            NarrativePhase::Climax => {
                "当前叙事阶段：高潮期。请保持紧张节奏，加快冲突升级，将所有线索汇聚到关键时刻，\
                 制造强烈的情感冲击。"
            }
            NarrativePhase::Falling => {
                "当前叙事阶段：回落期。高潮已过，请开始平息冲突，展示事件后果，为最终收尾做铺垫。"
            }
            NarrativePhase::Resolution => {
                "当前叙事阶段：收尾期。请解决剩余悬念，回收所有伏笔，给读者一个满意的结局，\
                 保持情感余韵。"
            }
        }
    }
}

#[cfg(test)]
mod tests;
