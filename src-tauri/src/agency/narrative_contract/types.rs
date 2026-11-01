//! 叙事契约（Narrative Contract）：把「节拍卡 + 当前状态」编译成一场戏的
//! 可执行、可验证契约。设计见
//! docs/plans/2026-11-02-narrative-contract-design.md §4–§5。
//!
//! P0 阶段只做**确定性编译**：不新增 LLM 调用。章契约生成（LLM）在 P1。

use serde::{Deserialize, Serialize};

use crate::agency::beat_card::ChangeKind;

/// 契约编译失败：宁可报资产不足，也不产通用句。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractError {
    MissingGoal,
    MissingOpposition,
    GenericGoal(String),
    EmptyProse,
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractError::MissingGoal => write!(f, "本场缺少具体目标"),
            ContractError::MissingOpposition => write!(f, "本场缺少具名阻力"),
            ContractError::GenericGoal(goal) => {
                write!(f, "本场目标仍是通用模板: {goal}")
            }
            ContractError::EmptyProse => write!(f, "正文为空，无法编译场前状态"),
        }
    }
}

/// 五轴状态变化。每场至少改变一轴；from != to。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateDelta {
    pub axis: ChangeKind,
    pub from: String,
    pub to: String,
    /// 正文里应出现的证据关键词（供确定性核对）。
    pub evidence_hint: String,
}

/// 最小情感因果链。完整 OCC/载体/读者面板仍由 emotion_circuit 提供；
/// 这里只要求「事件 → 评估 → 情绪 → 行动 → 代价」五段可执行。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmotionChain {
    pub trigger_event: String,
    pub appraisal: String,
    pub from_emotion: String,
    pub to_emotion: String,
    pub action: String,
    pub cost: String,
}

/// 一场戏的契约。字段都是 writer 可直接执行的句子，而不是资产转储。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SceneContract {
    pub index: usize,
    pub total: usize,

    pub pov: String,
    pub place_time: String,
    pub entry_state: String,

    pub goal: String,
    pub opposition: String,
    pub opposition_want: String,
    pub obstacle: String,
    pub tactic: String,
    pub turn: String,
    pub outcome: String,
    pub irreversible: bool,

    pub state_delta: StateDelta,
    pub emotion: EmotionChain,
    pub information_gap: Option<String>,
    pub promise_op: Option<String>,
    pub hook_out: String,
    pub word_budget: usize,
    pub must_not: Vec<String>,
}

impl SceneContract {
    /// 契约本身的最小校验：具体性、具名阻力、状态变化、情感五段。
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if self.goal.trim().is_empty() {
            issues.push("goal 为空".into());
        }
        if is_generic_sentence(&self.goal) {
            issues.push(format!("goal 仍是通用句：{}", self.goal));
        }
        if self.opposition.trim().is_empty() {
            issues.push("opposition 为空".into());
        }
        if self.turn.trim().is_empty() {
            issues.push("turn 为空".into());
        }
        if self.outcome.trim().is_empty() {
            issues.push("outcome 为空".into());
        }
        if self.state_delta.from.trim() == self.state_delta.to.trim() {
            issues.push("state_delta.from == to".into());
        }
        if self.emotion.trigger_event.trim().is_empty() {
            issues.push("emotion.trigger_event 为空".into());
        }
        if self.emotion.action.trim().is_empty() {
            issues.push("emotion.action 为空".into());
        }
        if self.emotion.cost.trim().is_empty() {
            issues.push("emotion.cost 为空".into());
        }
        issues
    }

    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

fn is_generic_sentence(s: &str) -> bool {
    let t = s.trim();
    t.is_empty()
        || t.starts_with("按场景结构推进")
        || t.starts_with("在硬约束内把当前冲突推进一步")
        || t.contains("目标→冲突→灾难")
        || t.contains("目标->冲突->灾难")
        || t == "推进剧情"
        || t == "写下一拍"
}

/// 章契约：本章的戏剧问题、赌注、转折与 1–3 场投影。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChapterContract {
    pub chapter_number: i32,
    pub dramatic_question: String,
    pub protagonist: String,
    pub want: String,
    pub opposition: String,
    pub stakes: String,
    pub turning_point: String,
    pub irreversible_outcome: String,
    pub target_progress: String,
    pub scenes: Vec<SceneContract>,
}

impl ChapterContract {
    pub fn validate(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if self.dramatic_question.trim().is_empty() {
            issues.push("dramatic_question 为空".into());
        }
        if self.want.trim().is_empty() {
            issues.push("want 为空".into());
        }
        if self.opposition.trim().is_empty() {
            issues.push("opposition 为空".into());
        }
        if self.stakes.trim().is_empty() {
            issues.push("stakes 为空".into());
        }
        if self.irreversible_outcome.trim().is_empty() {
            issues.push("irreversible_outcome 为空".into());
        }
        if self.scenes.is_empty() {
            issues.push("scenes 为空".into());
        }
        for scene in &self.scenes {
            issues.extend(scene.validate());
        }
        issues
    }

    pub fn is_valid(&self) -> bool {
        self.validate().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> SceneContract {
        SceneContract {
            index: 0,
            total: 1,
            pov: "沈砚".into(),
            place_time: "王府宴席·夜".into(),
            entry_state: "沈砚手里只有半本名册".into(),
            goal: "沈砚要在散席前让王爷承认名册来历".into(),
            opposition: "王爷".into(),
            opposition_want: "把沈砚引到侧厅".into(),
            obstacle: "王爷不接话，把名册扔进火盆".into(),
            tactic: "沈砚先敬酒再当众问军饷".into(),
            turn: "王爷烧名册，逼沈砚选救名册还是救证人".into(),
            outcome: "名册烧毁一半，证人被带走".into(),
            irreversible: true,
            state_delta: StateDelta {
                axis: ChangeKind::Relationship,
                from: "表面叔侄".into(),
                to: "公开敌对".into(),
                evidence_hint: "决裂".into(),
            },
            emotion: EmotionChain {
                trigger_event: "名册被烧".into(),
                appraisal: "证据被毁，需求受阻".into(),
                from_emotion: "恐惧".into(),
                to_emotion: "愤怒".into(),
                action: "徒手抢火盆".into(),
                cost: "烫伤且证人被带走".into(),
            },
            information_gap: None,
            promise_op: None,
            hook_out: "证人被带走前说母亲也签过".into(),
            word_budget: 1500,
            must_not: vec![],
        }
    }

    #[test]
    fn contract_valid() {
        assert!(contract().is_valid());
    }

    #[test]
    fn generic_goal_is_rejected() {
        let mut c = contract();
        c.goal = "按场景结构推进：目标→冲突→灾难".into();
        assert!(!c.is_valid());
    }
}
