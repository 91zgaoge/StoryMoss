//! 契约核对：用同一份场契约逐字段检查正文，并给出原文证据。
//!
//! 这是 Gate v2 的结构层补充：只抓「具名阻力缺席 / 正文过短 / 契约无效」
//! 这类可确定性判定的硬失败；文风、节奏仍交给现有 grader/editor。

use super::{compile::axis_keywords, types::SceneContract};
use crate::agency::continue_assets::strip_editor_markup;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractStatus {
    Met,
    Partial,
    Miss,
}

impl ContractStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ContractStatus::Met => "met",
            ContractStatus::Partial => "partial",
            ContractStatus::Miss => "miss",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FieldResult {
    pub field: String,
    pub status: ContractStatus,
    pub evidence: String,
    pub note: String,
}

#[derive(Debug, Clone, Default)]
pub struct ContractVerification {
    pub fields: Vec<FieldResult>,
    pub hard_failures: Vec<String>,
    pub soft_issues: Vec<String>,
}

impl ContractVerification {
    pub fn passed(&self) -> bool {
        self.hard_failures.is_empty()
    }

    pub fn score(&self) -> f64 {
        if self.fields.is_empty() {
            return 0.5;
        }
        let total: f64 = self
            .fields
            .iter()
            .map(|f| match f.status {
                ContractStatus::Met => 1.0,
                ContractStatus::Partial => 0.5,
                ContractStatus::Miss => 0.0,
            })
            .sum();
        (total / self.fields.len() as f64).clamp(0.0, 1.0)
    }
}

/// 核对正文是否兑现场契约。`prose` 可为带 HTML 的草稿。
pub fn verify_contract(contract: &SceneContract, prose: &str) -> ContractVerification {
    let plain = strip_editor_markup(prose);
    let plain = plain.trim();
    let mut out = ContractVerification::default();

    if plain.chars().count() < 200 {
        out.hard_failures.push(format!(
            "契约核对：正文过短（{} 字），无法兑现本场契约",
            plain.chars().count()
        ));
    }
    for issue in contract.validate() {
        out.hard_failures.push(format!("契约无效：{issue}"));
    }

    // 具名阻力必须真正出现在正文。找不到即硬失败——这是「无戏剧冲突」最
    // 直接的可确定性证据。
    let opp = contract.opposition.trim();
    let opp_met = !opp.is_empty() && plain.contains(opp);
    out.fields.push(FieldResult {
        field: "opposition".into(),
        status: if opp_met {
            ContractStatus::Met
        } else {
            ContractStatus::Miss
        },
        evidence: snippet_around(plain, opp),
        note: if opp_met {
            "具名阻力已出场".into()
        } else {
            "正文没有出现契约点名的阻力".into()
        },
    });
    if !opp_met {
        out.hard_failures
            .push(format!("契约核对：具名阻力「{opp}」未在正文出现"));
    }

    // 目标：POV 与阻力双方都出场，且正文不是纯粹复述。
    let pov_met = !contract.pov.trim().is_empty() && plain.contains(contract.pov.trim());
    let goal_status = if pov_met && opp_met {
        ContractStatus::Met
    } else if pov_met {
        ContractStatus::Partial
    } else {
        ContractStatus::Miss
    };
    out.fields.push(FieldResult {
        field: "goal".into(),
        status: goal_status,
        evidence: snippet_around(plain, contract.pov.trim()),
        note: "目标需要 POV 与阻力同时在场".into(),
    });
    if goal_status == ContractStatus::Miss {
        out.soft_issues
            .push("目标核对：POV 未在正文出现或不在行动主体位置".into());
    }

    // 转折：至少出现一个可观察的冲突动作词。
    let turn_hit = contains_any(
        plain,
        &[
            "烧", "扔", "拒绝", "逼", "揭", "抢", "撕", "砸", "拔", "转身", "冷笑", "问", "挡",
            "拦", "跪下", "拔刀", "推", "抓", "夺",
        ],
    );
    out.fields.push(FieldResult {
        field: "turn".into(),
        status: if turn_hit {
            ContractStatus::Met
        } else {
            ContractStatus::Partial
        },
        evidence: snippet_around(plain, opp),
        note: "转折需要可见动作，不是情绪/气氛".into(),
    });

    // 状态变化：五轴证据关键词至少命中一个。
    let axis_hit = contains_any(plain, axis_keywords(contract.state_delta.axis));
    out.fields.push(FieldResult {
        field: "state_delta".into(),
        status: if axis_hit {
            ContractStatus::Met
        } else {
            ContractStatus::Partial
        },
        evidence: snippet_around(
            plain,
            contract
                .state_delta
                .evidence_hint
                .split('、')
                .next()
                .unwrap_or(""),
        ),
        note: format!("本场应改变「{}」", contract.state_delta.axis.as_zh()),
    });
    if !axis_hit {
        out.soft_issues.push(format!(
            "状态变化核对：未命中「{}」的证据关键词（{}）",
            contract.state_delta.axis.as_zh(),
            contract.state_delta.evidence_hint
        ));
    }

    // 情感：触发/行动/代价至少两段可见。
    let emotion_hit = contains_any(
        plain,
        &[
            "代价", "失去", "伤", "疼", "带走", "烧", "暴露", "哭", "颤", "怕", "怒", "恨",
        ],
    );
    out.fields.push(FieldResult {
        field: "emotion".into(),
        status: if emotion_hit {
            ContractStatus::Met
        } else {
            ContractStatus::Partial
        },
        evidence: snippet_around(plain, "代价"),
        note: "情感需要事件→行动→代价，不能只有标签".into(),
    });
    if !emotion_hit {
        out.soft_issues
            .push("情感核对：正文未出现代价/行动词，可能停在标签层".into());
    }

    // 钩子：结尾出现新问题或未决压力。
    let tail: String = plain
        .chars()
        .rev()
        .take(240)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let hook_hit = tail.contains('?')
        || tail.contains('？')
        || contains_any(
            &tail,
            &["问题", "不知道", "还没", "带走", "等到", "门外", "下一步"],
        );
    out.fields.push(FieldResult {
        field: "hook".into(),
        status: if hook_hit {
            ContractStatus::Met
        } else {
            ContractStatus::Partial
        },
        evidence: tail.chars().take(80).collect::<String>(),
        note: "场末需要新问题/新威胁/决定".into(),
    });
    if !hook_hit {
        out.soft_issues
            .push("钩子核对：结尾没有新问题或未决压力".into());
    }

    out
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| !n.is_empty() && text.contains(n))
}

fn snippet_around(text: &str, needle: &str) -> String {
    if needle.is_empty() {
        return text.chars().take(60).collect();
    }
    if let Some(pos) = text.find(needle) {
        let start = text[..pos].chars().rev().take(20).collect::<Vec<_>>();
        let start: String = start.into_iter().rev().collect();
        let from = text[..pos]
            .chars()
            .count()
            .saturating_sub(start.chars().count());
        text.chars().skip(from).take(120).collect()
    } else {
        text.chars().take(80).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::{
        beat_card::ChangeKind,
        narrative_contract::types::{EmotionChain, SceneContract, StateDelta},
    };

    fn contract() -> SceneContract {
        SceneContract {
            index: 0,
            total: 1,
            pov: "沈砚".into(),
            place_time: "王府".into(),
            entry_state: "半本名册".into(),
            goal: "沈砚必须让王爷承认名册来历".into(),
            opposition: "王爷".into(),
            opposition_want: "把沈砚引到侧厅".into(),
            obstacle: "王爷烧名册".into(),
            tactic: "当众问军饷".into(),
            turn: "王爷烧名册".into(),
            outcome: "名册烧毁一半，证人被带走".into(),
            irreversible: true,
            state_delta: StateDelta {
                axis: ChangeKind::Relationship,
                from: "表面叔侄".into(),
                to: "公开敌对".into(),
                evidence_hint: "决裂、关系".into(),
            },
            emotion: EmotionChain {
                trigger_event: "名册被烧".into(),
                appraisal: "证据被毁".into(),
                from_emotion: "恐惧".into(),
                to_emotion: "愤怒".into(),
                action: "抢火盆".into(),
                cost: "烫伤".into(),
            },
            information_gap: None,
            promise_op: None,
            hook_out: "证人说母亲也签过".into(),
            word_budget: 1500,
            must_not: vec![],
        }
    }

    #[test]
    fn passing_prose_has_no_hard_failure() {
        let prose = format!(
            "沈砚把半本名册压在掌下。王爷笑着，把另一本名册扔进火盆。\
             沈砚扑上去抢，指尖烫出一层水泡。王爷的人把证人带走。\
             证人被拖出门前说：你母亲也签过。{}",
            "她把掌心压在桌沿上，等到火舌把纸边卷起来才伸手，指节被烫得发红。\
             满桌人没有动，只有王爷的人把证人往门外拖。"
                .repeat(4)
        );
        let v = verify_contract(&contract(), &prose);
        assert!(v.passed(), "{:?}", v.hard_failures);
        assert!(v.score() >= 0.5);
    }

    #[test]
    fn missing_opposition_is_hard_failure() {
        let prose = "沈砚把半本名册压在掌下。她想起母亲改嫁那天。雨一直下。\
            她把灯芯剪短了一截，没说话。"
            .to_string();
        let v = verify_contract(&contract(), &prose);
        assert!(!v.passed());
        assert!(v.hard_failures.iter().any(|s| s.contains("具名阻力")));
    }
}
