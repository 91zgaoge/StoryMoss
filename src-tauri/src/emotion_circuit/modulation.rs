//! 调制器：把情感回路编译成提示词块。
//!
//! 只写本拍选中的载体；未选中的不是禁止，而是留白。

use super::types::{
    CharacterEmotionState, EmotionBeat, EmotionContract, EmotionDebt, EmotionTension,
};

/// 渲染本拍情感回路块。目标 ≤600 字；超长时保留因果链与载体，删读者目标。
pub fn render_emotion_block(contract: Option<&EmotionContract>, beat: &EmotionBeat) -> String {
    let mut lines =
        vec!["【本拍情感回路（硬约束：情感必须由事件引起，并改变下一步行动）】".to_string()];
    if let Some(c) = contract {
        if !c.core_question.trim().is_empty() {
            lines.push(format!("故事情感契约：{}", c.core_question.trim()));
        }
    }
    lines.push("本拍因果链：".into());
    lines.push(format!("  事件：{}", beat.trigger_event.trim()));
    lines.push(format!(
        "  评估：{}；目标相关性{:.2}；与目标{}；{}；确定性{:.2}；可控性{:.2}；违背底线{:.2}",
        beat.event_kind.label(),
        beat.appraisal.relevance,
        if beat.appraisal.congruence >= 0.0 {
            "一致"
        } else {
            "冲突"
        },
        beat.appraisal.agency.label(),
        beat.appraisal.certainty,
        beat.appraisal.control,
        beat.appraisal.norm_violation,
    ));
    lines.push(format!(
        "  情绪：{}（强度 {:.2}）",
        beat.from.render_delta(&beat.to, 3),
        beat.scale
    ));
    lines.push(format!(
        "  行动倾向：{}（{}）",
        beat.action_tendency.label(),
        if beat.action_reason.trim().is_empty() {
            "由本拍情绪直接推出"
        } else {
            beat.action_reason.trim()
        }
    ));
    if let Some(ref d) = beat.dialogue_policy {
        if !d.objective.trim().is_empty() {
            let forbidden = if d.forbidden.is_empty() {
                String::new()
            } else {
                format!("；绝不说={}", d.forbidden.join("、"))
            };
            lines.push(format!(
                "  台词：目标={}；手段={}；潜台词={}{}",
                d.objective.trim(),
                d.tactic.trim(),
                d.subtext.trim(),
                forbidden
            ));
        }
    }
    lines.push(format!("  代价：{}", beat.cost.trim()));
    if !beat.state_changes.is_empty() {
        lines.push(format!("  新状态：{}", beat.state_changes.join("；")));
    }
    if let Some(ref t) = tension_line(beat) {
        lines.push(t.clone());
    }
    lines.push("载体配比（只选 3 项，其余留白）：".into());
    lines.push(beat.carriers.render());
    if !beat.reader_target.primary.trim().is_empty() {
        let secondary = beat
            .reader_target
            .secondary
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("／{}", s.trim()))
            .unwrap_or_default();
        lines.push(format!(
            "读者目标：{}{}；{}",
            beat.reader_target.primary.trim(),
            secondary,
            beat.reader_target.why.trim()
        ));
        lines.push(format!("叙述克制：{}", beat.reader_target.restraint.trim()));
    }
    let mut block = lines.join("\n");
    if block.chars().count() > 700 {
        // 超长时只保留因果链与载体：读者目标是可删的修饰，因果链不可删。
        block = block
            .lines()
            .filter(|l| {
                !l.starts_with("读者目标：")
                    && !l.starts_with("叙述克制：")
                    && !l.starts_with("故事情感契约：")
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    block
}

fn tension_line(beat: &EmotionBeat) -> Option<String> {
    if beat.tension_id.is_none() {
        return None;
    }
    Some(format!(
        "关系张力：{} → {}（本拍必须让这条张力升级、揭示、修复或破裂之一）",
        beat.source_name, beat.target_name
    ))
}

/// 章纲规划用的情感回路块。没有契约/状态/张力/债务时返回空串，旧故事
/// 的 `scene_outline` 提示词逐字不变。
pub fn render_chapter_emotion_plan(
    contract: Option<&EmotionContract>,
    states: &[CharacterEmotionState],
    tensions: &[EmotionTension],
    debts: &[EmotionDebt],
    chapter: i32,
) -> String {
    if contract.is_none() && states.is_empty() && tensions.is_empty() && debts.is_empty() {
        return String::new();
    }
    let mut lines: Vec<String> = Vec::new();
    if let Some(c) = contract {
        if !c.core_question.trim().is_empty() {
            lines.push(format!("故事情感契约：{}", c.core_question.trim()));
        }
        if !c.reader_promise.trim().is_empty() {
            lines.push(format!("读者承诺：{}", c.reader_promise.trim()));
        }
    }
    let state_block = render_emotion_state_block(states, tensions);
    if !state_block.is_empty() {
        lines.push(state_block);
    }
    let due: Vec<&EmotionDebt> = debts
        .iter()
        .filter(|d| d.is_open() && d.due_chapter.map(|x| x <= chapter).unwrap_or(false))
        .take(2)
        .collect();
    if !due.is_empty() {
        let items = due
            .iter()
            .map(|d| {
                format!(
                    "{}（第{}章到期）",
                    d.description.trim(),
                    d.due_chapter.unwrap_or(chapter)
                )
            })
            .collect::<Vec<_>>()
            .join("；");
        lines.push(format!("逾期情感债务：{items}"));
    }
    lines.push(
        "本章情感转折要求：开始（谁带什么情绪/未说出口的话）→ 触发（什么事件触碰伤口/需求）→ 结束（谁的情绪/关系/目标不可逆变化）。"
            .into(),
    );
    lines.join("\n")
}

/// 渲染角色动态状态与关系张力快照。用于状态网扩展。
pub fn render_emotion_state_block(
    states: &[CharacterEmotionState],
    tensions: &[EmotionTension],
) -> String {
    let mut lines: Vec<String> = Vec::new();
    if !states.is_empty() {
        lines.push("【角色情感状态】".to_string());
        for s in states.iter().take(6) {
            let name = if s.character_name.is_empty() {
                &s.character_id
            } else {
                &s.character_name
            };
            let mut parts = vec![format!("{}：{}", name, s.vector.render_compact(3))];
            if let Some(ref g) = s.active_goal {
                if !g.trim().is_empty() {
                    parts.push(format!("目标={}", g.trim()));
                }
            }
            if let Some(ref d) = s.defense {
                if !d.trim().is_empty() {
                    parts.push(format!("防御={}", d.trim()));
                }
            }
            lines.push(format!("- {}", parts.join("；")));
        }
    }
    let open: Vec<&EmotionTension> = tensions.iter().filter(|t| t.is_open()).take(4).collect();
    if !open.is_empty() {
        lines.push("【未释放的关系张力】".to_string());
        for t in open {
            let target = if t.target_name.is_empty() {
                "对方"
            } else {
                t.target_name.as_str()
            };
            lines.push(format!(
                "- {} → {}：{}（压力 {:.2}，积压 {} 拍{}）",
                t.source_name,
                target,
                t.tension_type,
                t.pressure,
                t.accumulated_beats,
                t.due_chapter
                    .map(|d| format!("，最迟第{d}章兑现"))
                    .unwrap_or_default()
            ));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emotion_circuit::{
        types::{DialoguePolicy, EmotionBeat, ReaderTarget},
        vector::EmotionVector,
    };

    fn beat() -> EmotionBeat {
        EmotionBeat {
            from: EmotionVector::from_pairs(&[("shame", 0.6), ("anger", 0.3)]),
            to: EmotionVector::from_pairs(&[("shame", 0.4), ("anger", 0.6)]),
            trigger_event: "阿岩当众替林雪承认那封信".into(),
            appraisal: Default::default(),
            action_tendency: crate::emotion_circuit::appraisal::ActionTendency::Conceal,
            action_reason: "主导情绪=羞耻，防御=攻击型".into(),
            dialogue_policy: Some(DialoguePolicy {
                objective: "让他收回那句话".into(),
                tactic: "冷嘲".into(),
                subtext: "你凭什么替我决定".into(),
                forbidden: vec!["谢谢".into()],
            }),
            cost: "她当众推开他，坐实了不识好歹".into(),
            state_changes: vec!["林雪：羞耻→愤怒".into()],
            carriers: crate::emotion_circuit::carriers::select_carriers(
                &EmotionVector::from_pairs(&[("anger", 0.7), ("shame", 0.3)]),
                "攻击型",
                true,
                false,
            ),
            reader_target: ReaderTarget {
                primary: "难堪".into(),
                secondary: Some("心疼".into()),
                why: "因为她必须为拒绝付出代价".into(),
                restraint: "不要替读者哭".into(),
            },
            scale: 0.8,
            event_kind: crate::emotion_circuit::appraisal::EventKind::SecretReveal,
            source_name: "林雪".into(),
            target_name: "阿岩".into(),
            tension_id: Some("t1".into()),
            debt_id: None,
        }
    }

    #[test]
    fn block_carries_causal_chain_and_three_carriers() {
        let text = render_emotion_block(None, &beat());
        assert!(text.contains("事件："));
        assert!(text.contains("评估："));
        assert!(text.contains("情绪："));
        assert!(text.contains("行动倾向："));
        assert!(text.contains("代价："));
        assert!(text.contains("载体配比"));
        assert!(
            text.lines()
                .filter(|l| l.chars().next().map(|c| c.is_numeric()).unwrap_or(false))
                .count()
                <= 3
        );
    }

    #[test]
    fn block_is_bounded() {
        let text = render_emotion_block(None, &beat());
        assert!(text.chars().count() <= 800, "len={}", text.chars().count());
    }

    #[test]
    fn chapter_plan_empty_without_assets() {
        let text = render_chapter_emotion_plan(None, &[], &[], &[], 1);
        assert!(text.is_empty());
    }

    #[test]
    fn chapter_plan_renders_contract_and_due_debt() {
        let contract = EmotionContract {
            core_question: "能否被爱而不失去自我".into(),
            reader_promise: "看一个人学会为自己活".into(),
            ..Default::default()
        };
        let debts = vec![crate::emotion_circuit::types::EmotionDebt {
            description: "答应带她离开".into(),
            pressure: 0.7,
            due_chapter: Some(3),
            status: "open".into(),
            ..Default::default()
        }];
        let text = render_chapter_emotion_plan(Some(&contract), &[], &[], &debts, 3);
        assert!(text.contains("能否被爱"));
        assert!(text.contains("答应带她离开"));
        assert!(text.contains("情感转折"));
    }

    #[test]
    fn state_block_lists_open_tensions_only() {
        let states = vec![CharacterEmotionState {
            character_name: "林雪".into(),
            vector: EmotionVector::from_pairs(&[("shame", 1.0)]),
            ..Default::default()
        }];
        let tensions = vec![EmotionTension {
            source_name: "林雪".into(),
            target_name: "阿岩".into(),
            tension_type: "未揭穿的欺骗".into(),
            pressure: 0.8,
            accumulated_beats: 3,
            status: "open".into(),
            ..Default::default()
        }];
        let text = render_emotion_state_block(&states, &tensions);
        assert!(text.contains("林雪"));
        assert!(text.contains("未揭穿的欺骗"));
    }
}
