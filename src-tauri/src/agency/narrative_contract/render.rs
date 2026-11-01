//! 场简报渲染：writer 最后看到的一页执行指令（不是资产转储）。

use super::types::{ChapterContract, SceneContract};

/// 一页场简报。放在 writer prompt 的变动区末尾、末句锚点之前。
pub fn render_scene_brief(c: &SceneContract) -> String {
    let mut lines = vec![];
    lines.push(format!(
        "【本场契约｜第 {} 场 / 共 {} 场】",
        c.index + 1,
        c.total.max(1)
    ));
    lines.push(format!("场前状态：{}", c.entry_state));
    lines.push(format!("本场目标：{}", c.goal));
    lines.push(format!("阻力：{}", c.opposition));
    lines.push(format!("阻力目标：{}", c.opposition_want));
    lines.push(format!("障碍：{}", c.obstacle));
    lines.push(format!("手段：{}", c.tactic));
    lines.push(format!("转折：{}", c.turn));
    lines.push(format!(
        "必须结果：{}{}",
        c.outcome,
        if c.irreversible {
            "（不可逆）"
        } else {
            ""
        }
    ));
    lines.push(format!(
        "状态变化：{} 从「{}」到「{}」",
        c.state_delta.axis.as_zh(),
        c.state_delta.from,
        c.state_delta.to
    ));
    lines.push(format!(
        "情感链：事件={}；评估={}；情绪={}→{}；行动={}；代价={}",
        c.emotion.trigger_event,
        c.emotion.appraisal,
        c.emotion.from_emotion,
        c.emotion.to_emotion,
        c.emotion.action,
        c.emotion.cost
    ));
    if !c.must_not.is_empty() {
        lines.push(format!("禁止：{}", c.must_not.join("；")));
    }
    lines.push(format!("结尾钩子：{}", c.hook_out));
    lines.push(format!("字数：{}", c.word_budget));
    lines.join("\n")
}

/// 章契约（≤400 字）。放在稳定资产末尾、场简报之前。
pub fn render_chapter_contract(c: &ChapterContract) -> String {
    let mut lines = vec!["【本章契约】".to_string()];
    lines.push(format!("本章戏剧问题：{}", c.dramatic_question));
    lines.push(format!("主角想要：{}", c.want));
    lines.push(format!("反对者：{}", c.opposition));
    lines.push(format!("赌注：{}", c.stakes));
    lines.push(format!("本章转折：{}", c.turning_point));
    lines.push(format!("不可逆结果：{}", c.irreversible_outcome));
    lines.push(format!("进度目标：{}", c.target_progress));
    lines.push(format!("本场数：{}", c.scenes.len()));
    lines.join("\n")
}

/// writer prompt 最末的四行锚定摘要（防止契约被末句锚点压过）。
pub fn render_scene_tail(c: &SceneContract) -> String {
    format!(
        "【场契约摘要】目标：{}｜阻力：{}｜必须结果：{}｜状态变化：{}",
        c.goal.chars().take(40).collect::<String>(),
        c.opposition.chars().take(20).collect::<String>(),
        c.outcome.chars().take(40).collect::<String>(),
        format!(
            "{} 从「{}」到「{}」",
            c.state_delta.axis.as_zh(),
            c.state_delta.from.chars().take(20).collect::<String>(),
            c.state_delta.to.chars().take(30).collect::<String>()
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::{
        beat_card::ChangeKind,
        narrative_contract::types::{ChapterContract, EmotionChain, SceneContract, StateDelta},
    };

    fn contract() -> SceneContract {
        SceneContract {
            index: 0,
            total: 1,
            pov: "沈砚".into(),
            place_time: "王府".into(),
            entry_state: "手里只有半本名册".into(),
            goal: "让王爷承认名册来历".into(),
            opposition: "王爷".into(),
            opposition_want: "把沈砚引到侧厅".into(),
            obstacle: "王爷把名册扔进火盆".into(),
            tactic: "先敬酒再当众问军饷".into(),
            turn: "王爷烧名册".into(),
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
                appraisal: "证据被毁".into(),
                from_emotion: "恐惧".into(),
                to_emotion: "愤怒".into(),
                action: "抢火盆".into(),
                cost: "烫伤".into(),
            },
            information_gap: None,
            promise_op: None,
            hook_out: "证人被带走前说母亲也签过".into(),
            word_budget: 1500,
            must_not: vec!["禁止只复述末句".into()],
        }
    }

    #[test]
    fn brief_contains_all_executable_fields() {
        let out = render_scene_brief(&contract());
        assert!(out.contains("本场目标：让王爷承认名册来历"));
        assert!(out.contains("阻力：王爷"));
        assert!(out.contains("必须结果：名册烧毁一半"));
        assert!(out.contains("不可逆"));
        assert!(out.contains("情感链：事件=名册被烧"));
        assert!(out.contains("代价=烫伤"));
    }

    #[test]
    fn chapter_brief_keeps_dramatic_question_and_outcome() {
        let cc = ChapterContract {
            chapter_number: 7,
            dramatic_question: "沈砚能否在散席前拿到名册？".into(),
            protagonist: "沈砚".into(),
            want: "拿到名册".into(),
            opposition: "王爷".into(),
            stakes: "失去证人".into(),
            turning_point: "王爷烧名册".into(),
            irreversible_outcome: "证人被带走".into(),
            target_progress: "下一页：密诏败露".into(),
            scenes: vec![contract()],
        };
        let out = render_chapter_contract(&cc);
        assert!(out.contains("沈砚能否在散席前拿到名册"));
        assert!(out.contains("赌注：失去证人"));
        assert!(out.contains("不可逆结果：证人被带走"));
        assert!(out.contains("本场数：1"));
    }

    #[test]
    fn tail_keeps_goal_and_delta() {
        let out = render_scene_tail(&contract());
        assert!(out.contains("目标：让王爷承认名册来历"));
        assert!(out.contains("阻力：王爷"));
        assert!(out.contains("关系"));
    }
}
