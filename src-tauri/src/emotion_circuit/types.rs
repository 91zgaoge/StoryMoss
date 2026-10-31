//! 情感回路的持久化与提示词数据结构。

use serde::{Deserialize, Serialize};

use super::{
    appraisal::{ActionTendency, Appraisal, EventKind},
    carriers::CarrierPlan,
    vector::EmotionVector,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmotionContract {
    pub story_id: String,
    pub core_question: String,
    pub reader_promise: String,
    pub palette: Vec<String>,
    pub ending_residue: String,
    pub forbidden: Vec<String>,
    pub peak_budget: u8,
    pub version: i64,
}

impl EmotionContract {
    pub fn peak_budget(&self) -> u32 {
        if self.peak_budget == 0 {
            1
        } else {
            self.peak_budget as u32
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CharacterEmotionState {
    pub id: String,
    pub story_id: String,
    pub character_id: String,
    #[serde(default)]
    pub character_name: String,
    pub scene_id: Option<String>,
    pub chapter_number: Option<i32>,
    pub beat_index: Option<i32>,
    pub vector: EmotionVector,
    pub intensity: f32,
    pub active_goal: Option<String>,
    pub defense: Option<String>,
    pub cause_event: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmotionTension {
    pub id: String,
    pub story_id: String,
    pub source_character_id: String,
    pub target_character_id: Option<String>,
    #[serde(default)]
    pub source_name: String,
    #[serde(default)]
    pub target_name: String,
    pub tension_type: String,
    pub bond: Option<String>,
    pub reverse_bond: Option<String>,
    pub pressure: f32,
    pub accumulated_beats: u32,
    pub due_chapter: Option<i32>,
    pub status: String,
    pub last_escalated_chapter: Option<i32>,
    pub updated_at: String,
}

impl EmotionTension {
    pub fn is_open(&self) -> bool {
        !matches!(
            self.status.as_str(),
            "paid" | "resolved" | "closed" | "dormant"
        )
    }

    pub fn is_due(&self, chapter: i32) -> bool {
        self.is_open() && self.due_chapter.map(|d| d <= chapter).unwrap_or(false)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmotionDebt {
    pub id: String,
    pub story_id: String,
    pub kind: String,
    pub source_ref: Option<String>,
    pub description: String,
    pub pressure: f32,
    pub due_chapter: Option<i32>,
    pub status: String,
    pub created_at: String,
}

impl EmotionDebt {
    pub fn is_open(&self) -> bool {
        matches!(self.status.as_str(), "open" | "deferred")
    }

    pub fn is_due(&self, chapter: i32) -> bool {
        self.is_open() && self.due_chapter.map(|d| d <= chapter).unwrap_or(false)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DialoguePolicy {
    pub objective: String,
    pub tactic: String,
    pub subtext: String,
    pub forbidden: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReaderTarget {
    pub primary: String,
    pub secondary: Option<String>,
    pub why: String,
    pub restraint: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmotionBeat {
    pub from: EmotionVector,
    pub to: EmotionVector,
    pub trigger_event: String,
    pub appraisal: Appraisal,
    pub action_tendency: ActionTendency,
    #[serde(default)]
    pub action_reason: String,
    pub dialogue_policy: Option<DialoguePolicy>,
    pub cost: String,
    #[serde(default)]
    pub state_changes: Vec<String>,
    pub carriers: CarrierPlan,
    pub reader_target: ReaderTarget,
    pub scale: f32,
    #[serde(default)]
    pub event_kind: EventKind,
    #[serde(default)]
    pub source_name: String,
    #[serde(default)]
    pub target_name: String,
    #[serde(default)]
    pub tension_id: Option<String>,
    #[serde(default)]
    pub debt_id: Option<String>,
}

impl Default for EventKind {
    fn default() -> Self {
        Self::Neutral
    }
}

impl Default for ActionTendency {
    fn default() -> Self {
        Self::Approach
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SceneEmotionBeat {
    pub id: String,
    pub story_id: String,
    pub scene_id: String,
    pub beat_index: i32,
    pub from: EmotionVector,
    pub to: EmotionVector,
    pub trigger_event: String,
    pub appraisal: Appraisal,
    pub action_tendency: ActionTendency,
    pub dialogue_policy: Option<DialoguePolicy>,
    pub cost: String,
    pub carriers: CarrierPlan,
    pub reader_target: ReaderTarget,
    pub scale: f32,
    #[serde(default)]
    pub event_kind: EventKind,
    #[serde(default)]
    pub source_name: String,
    #[serde(default)]
    pub target_name: String,
    #[serde(default)]
    pub tension_id: Option<String>,
    #[serde(default)]
    pub debt_id: Option<String>,
    pub created_at: String,
}

impl SceneEmotionBeat {
    /// 还原为探针/提示词使用的计划对象。
    pub fn to_emotion_beat(&self) -> EmotionBeat {
        EmotionBeat {
            from: self.from.clone(),
            to: self.to.clone(),
            trigger_event: self.trigger_event.clone(),
            appraisal: self.appraisal.clone(),
            action_tendency: self.action_tendency,
            action_reason: String::new(),
            dialogue_policy: self.dialogue_policy.clone(),
            cost: self.cost.clone(),
            state_changes: Vec::new(),
            carriers: self.carriers.clone(),
            reader_target: self.reader_target.clone(),
            scale: self.scale,
            event_kind: self.event_kind,
            source_name: self.source_name.clone(),
            target_name: self.target_name.clone(),
            tension_id: self.tension_id.clone(),
            debt_id: self.debt_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReaderResonanceReview {
    pub id: String,
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_number: Option<i32>,
    pub persona: String,
    pub scores: super::reader::ReaderScores,
    pub verdict: String,
    pub evidence: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmotionProbeLog {
    pub id: String,
    pub story_id: String,
    pub scene_id: Option<String>,
    pub beat_index: Option<i32>,
    pub probe: String,
    pub severity: String,
    pub passed: bool,
    pub detail: String,
    pub created_at: String,
}
