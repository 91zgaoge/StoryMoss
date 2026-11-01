//! 节拍卡 → 场契约（确定性，0 LLM）。
//!
//! 输入是 `SceneBeatCard`（阵容/冲突/情感/必须改变）与当前章大纲/近文；
//! 输出是一页 writer 可直接执行的 `SceneContract`。缺具体目标或具名阻力时
//! 返回 `ContractError`，不再产通用句。

use super::types::{ChapterContract, ContractError, EmotionChain, SceneContract, StateDelta};
use crate::agency::{
    beat_card::{ChangeKind, SceneBeatCard},
    continue_assets::strip_editor_markup,
    prose_ground::is_generic_next_node,
};

/// 把本拍素材编译成一场戏。`index/total` 暂用于单拍/批量展示；
/// P0 单拍固定 0/1，批量后续按章契约投影。
pub fn compile_scene_contract(
    card: &SceneBeatCard,
    chapter_outline: &str,
    current_content: &str,
    index: usize,
    total: usize,
) -> Result<SceneContract, ContractError> {
    let plain = strip_editor_markup(current_content);
    let plain = plain.trim();
    let pov = card
        .cast
        .first()
        .map(|c| c.name.trim().to_string())
        .filter(|n| !n.is_empty())
        .ok_or(ContractError::MissingGoal)?;

    let place_time = card
        .setting_location
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "本场当前地点".to_string());

    let entry_state = if plain.chars().count() < 40 {
        "本章刚开场，尚未建立可承接的上一场结果".to_string()
    } else {
        let tail: String = plain
            .chars()
            .rev()
            .take(120)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("前文末段：{}", tail.trim())
    };

    // 目标优先用「下一节点」；通用句不得成为契约目标，回退到章纲第一条
    // 具体事件，仍不具体则报 MissingGoal。
    let raw_goal = if !card.next_outline_node.trim().is_empty()
        && !is_generic_next_node(&card.next_outline_node)
    {
        card.next_outline_node.trim().to_string()
    } else {
        first_concrete_line(chapter_outline)
            .or_else(|| {
                let summary = card.change_delta.summary.trim();
                if summary.is_empty() {
                    None
                } else {
                    Some(summary.to_string())
                }
            })
            .ok_or(ContractError::MissingGoal)?
    };
    if raw_goal.is_empty() || is_generic_goal(&raw_goal) {
        return Err(ContractError::GenericGoal(raw_goal));
    }
    let goal = format!("{pov} 必须完成：{raw_goal}");

    let opposition = card
        .conflict_move
        .parties
        .iter()
        .skip(1)
        .find(|n| !n.trim().is_empty() && *n != &pov)
        .cloned()
        .or_else(|| {
            card.cast
                .iter()
                .skip(1)
                .map(|c| c.name.clone())
                .find(|n| !n.trim().is_empty() && n != &pov)
        })
        .ok_or(ContractError::MissingOpposition)?;
    let opposition_want =
        format!("{opposition} 要保住自己的筹码，并阻止 {pov} 完成目标；他不会只靠沉默拖延");

    let obstacle = {
        let action = card.conflict_move.action.trim();
        if action.is_empty() {
            format!("{opposition} 拒绝让 {pov} 达成目标，并当场设阻")
        } else {
            action.to_string()
        }
    };
    let tactic = format!(
        "{pov} 先试探 {opposition} 的底线，再当众提出一个无法回避的要求；若被挡回，改用下一步动作逼近目标"
    );
    let turn = format!(
        "{opposition} 反将一军（{obstacle}），逼 {pov} 当场选择：继续争夺，还是保人/保证据"
    );
    let outcome = outcome_for(card.change_delta.kind, &pov, &opposition, &obstacle);
    let irreversible = !matches!(card.change_delta.kind, ChangeKind::Information);

    let from = if plain.chars().count() < 40 {
        "上一场尚未建立稳定状态".to_string()
    } else {
        let tail: String = plain
            .chars()
            .rev()
            .take(60)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        tail.trim().to_string()
    };
    let to = outcome.clone();
    let evidence_hint = axis_keywords(card.change_delta.kind).join("、");
    let state_delta = StateDelta {
        axis: card.change_delta.kind,
        from,
        to,
        evidence_hint,
    };

    let emotion = EmotionChain {
        trigger_event: turn.clone(),
        appraisal: format!("事件与 {pov} 的目标直接相关，且触碰其伤口/底线"),
        from_emotion: dominant_emotion(&card.emotion_beat.summary),
        to_emotion: "压力下的行动冲动".into(),
        action: tactic.clone(),
        cost: outcome.clone(),
    };

    let hook_out = if !card.next_outline_node.trim().is_empty()
        && !is_generic_next_node(&card.next_outline_node)
    {
        format!("本场结果引出下一步：{}", card.next_outline_node.trim())
    } else {
        format!("本场结果留下新的未决问题：{outcome}")
    };

    let mut must_not = Vec::new();
    if !card.dead.is_empty() {
        must_not.push(format!(
            "{} 已死，禁止再作为行动主体，禁止重演其死亡",
            card.dead.join("、")
        ));
    }
    must_not.push("禁止只复述末句、全员表态或写过渡场".into());
    must_not.push("禁止用「众人反对/命运/气氛」代替具名阻力".into());

    Ok(SceneContract {
        index,
        total,
        pov,
        place_time,
        entry_state,
        goal,
        opposition,
        opposition_want,
        obstacle,
        tactic,
        turn,
        outcome,
        irreversible,
        state_delta,
        emotion,
        information_gap: None,
        promise_op: None,
        hook_out,
        word_budget: 1500,
        must_not,
    })
}

/// 章契约 + 1–3 场投影（确定性）。章契约本身不新增 LLM 调用：用同一张
/// 节拍卡 + 章纲切出 1–3 场，上一场结果成为下一场前提。
pub fn compile_chapter_contract(
    card: &SceneBeatCard,
    chapter_outline: &str,
    current_content: &str,
    chapter_number: i32,
    total_scenes: usize,
) -> Result<ChapterContract, ContractError> {
    let total = total_scenes.clamp(1, 3);
    let base = compile_scene_contract(card, chapter_outline, current_content, 0, total)?;
    let segments = split_outline_segments(chapter_outline, total);
    let n = segments.len().max(1);
    let mut scenes: Vec<SceneContract> = Vec::new();
    for (i, seg) in segments.iter().enumerate() {
        let mut scene = base.clone();
        scene.index = i;
        scene.total = n;
        let seg = seg.trim();
        if !seg.is_empty() {
            scene.goal = format!("{} 必须完成：{}", base.pov, seg);
            scene.turn = format!(
                "{} 反将一军：{}，逼 {} 当场选择：继续争夺，还是保人/保证据",
                base.opposition, seg, base.pov
            );
            scene.emotion.trigger_event = seg.chars().take(60).collect();
            scene.emotion.appraisal =
                format!("事件与 {} 的目标直接相关，且触碰其伤口/底线", base.pov);
        }
        if i > 0 {
            scene.entry_state = scenes[i - 1].outcome.clone();
            scene.state_delta.from = scenes[i - 1].outcome.clone();
        }
        let outcome = outcome_for(card.change_delta.kind, &base.pov, &base.opposition, seg);
        scene.outcome = format!(
            "{}（本场结果：{}）",
            outcome,
            seg.chars().take(40).collect::<String>()
        );
        scene.state_delta.to = scene.outcome.clone();
        scene.emotion.cost = scene.outcome.clone();
        scene.hook_out = if i + 1 < segments.len() {
            format!("本场结果引出下一场：{}", segments[i + 1].trim())
        } else {
            base.hook_out.clone()
        };
        scenes.push(scene);
    }
    let last = scenes.last().cloned().unwrap_or_else(|| base.clone());
    Ok(ChapterContract {
        chapter_number,
        dramatic_question: format!(
            "{} 能否在 {} 前完成「{}」？",
            base.pov, base.place_time, base.goal
        ),
        protagonist: base.pov.clone(),
        want: base.goal.clone(),
        opposition: base.opposition.clone(),
        stakes: base.outcome.clone(),
        turning_point: base.turn.clone(),
        irreversible_outcome: last.outcome.clone(),
        target_progress: base.hook_out.clone(),
        scenes,
    })
}

fn split_outline_segments(outline: &str, total: usize) -> Vec<String> {
    if total <= 1 {
        let t = outline.trim();
        return if t.is_empty() {
            vec![]
        } else {
            vec![t.to_string()]
        };
    }
    let mut parts: Vec<String> = outline
        .split(['\n', '。', '！', '？', '；', ';'])
        .map(str::trim)
        .filter(|s| s.chars().count() >= 8)
        .map(|s| s.to_string())
        .collect();
    if parts.is_empty() {
        let t = outline.trim();
        return if t.is_empty() {
            vec![]
        } else {
            vec![t.to_string()]
        };
    }
    // 尽量均匀分成 total 段；不足时保留实际段数。
    let chunk = (parts.len() as f64 / total as f64).ceil() as usize;
    let mut out: Vec<String> = Vec::new();
    for group in parts.chunks(chunk.max(1)) {
        out.push(group.join("；"));
    }
    if out.len() > total {
        out.truncate(total);
    }
    out
}

fn first_concrete_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter(|l| !l.starts_with('#') && !l.starts_with('-') && !l.starts_with('*'))
        .find(|l| l.chars().count() >= 8)
        .map(|l| l.chars().take(160).collect())
}

fn dominant_emotion(summary: &str) -> String {
    let s = summary.trim();
    if s.is_empty() {
        "压抑/戒备".to_string()
    } else {
        s.chars().take(60).collect()
    }
}

fn is_generic_goal(goal: &str) -> bool {
    let t = goal.trim();
    t.is_empty()
        || t.starts_with("按场景结构推进")
        || t.starts_with("在硬约束内把当前冲突推进一步")
        || t.contains("目标→冲突→灾难")
        || t.contains("目标->冲突->灾难")
        || t == "推进剧情"
        || t == "写下一拍"
}

fn outcome_for(kind: ChangeKind, pov: &str, opposition: &str, obstacle: &str) -> String {
    match kind {
        ChangeKind::Relationship => {
            format!("{pov} 与 {opposition} 的关系从表面状态变为公开对立，且当场无法修复")
        }
        ChangeKind::Risk => {
            format!("{pov} 因「{obstacle}」失去一项可见筹码（证据/人证/靠山），风险升级到不可回避")
        }
        ChangeKind::Goal => format!("{pov} 的旧目标当场作废或被迫转向下一步，不能再靠观望推进"),
        ChangeKind::Information => format!("一条新信息落地：{pov} 的旧判断失效，必须据此改动作"),
        ChangeKind::Emotion => format!("{pov} 的情绪压过理性，做出一个有可见代价的行动"),
    }
}

/// 五轴各自在正文里应出现的证据关键词。供契约 verifier 做确定性核对。
pub fn axis_keywords(kind: ChangeKind) -> &'static [&'static str] {
    match kind {
        ChangeKind::Information => &[
            "发现", "得知", "揭示", "说", "告诉", "文件", "消息", "账", "证据",
        ],
        ChangeKind::Relationship => &[
            "关系", "背叛", "信任", "疏远", "决裂", "翻脸", "亲", "仇", "敌对",
        ],
        ChangeKind::Goal => &[
            "决定", "改变", "放弃", "转向", "目标", "选择", "答应", "拒绝",
        ],
        ChangeKind::Risk => &[
            "危险", "威胁", "代价", "失去", "赌注", "筹码", "暴露", "追兵",
        ],
        ChangeKind::Emotion => &["怕", "怒", "悲", "喜", "羞", "恨", "颤", "哭", "笑", "心疼"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::beat_card::{CastMember, ConflictMove, ConflictStage, EmotionBeat};

    fn card() -> SceneBeatCard {
        SceneBeatCard {
            cast: vec![
                CastMember {
                    name: "沈砚".into(),
                    purpose: "行动主体".into(),
                },
                CastMember {
                    name: "王爷".into(),
                    purpose: "阻力".into(),
                },
            ],
            conflict_move: ConflictMove {
                action: "加压：王爷与沈砚正面对峙，赌注未解".into(),
                parties: vec!["沈砚".into(), "王爷".into()],
                stage: ConflictStage::Press,
            },
            emotion_beat: EmotionBeat {
                summary: "沈砚：伤口=母亲改嫁，需求=被认可".into(),
            },
            next_outline_node: "王爷当众烧掉名册".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("王府宴席".into()),
            open_review_issues: vec![],
            dead: vec![],
            change_delta: crate::agency::beat_card::ChangeDelta {
                kind: ChangeKind::Relationship,
                summary: "加压：王爷与沈砚正面对峙".into(),
            },
        }
    }

    #[test]
    fn compiles_specific_contract() {
        let c = compile_scene_contract(&card(), "沈砚拿到半本名册。王爷烧名册。", "前文。", 0, 1)
            .unwrap();
        assert!(c.is_valid(), "{:?}", c.validate());
        assert!(c.goal.contains("王爷当众烧掉名册"));
        assert_eq!(c.opposition, "王爷");
        assert!(c.emotion.cost.contains("关系"));
    }

    #[test]
    fn generic_next_node_falls_back_to_chapter_outline() {
        let mut card = card();
        card.next_outline_node = "按场景结构推进：目标→冲突→灾难或反应→困境→决定。".into();
        let c = compile_scene_contract(
            &card,
            "沈砚在宴席上逼王爷承认名册。王爷烧名册。",
            "前文。",
            0,
            1,
        )
        .unwrap();
        assert!(c.goal.contains("沈砚在宴席上逼王爷承认名册"));
        assert!(!c.goal.contains("按场景结构推进"));
    }

    #[test]
    fn missing_opposition_errors() {
        let mut card = card();
        card.cast = vec![CastMember {
            name: "沈砚".into(),
            purpose: "行动主体".into(),
        }];
        card.conflict_move.parties = vec!["沈砚".into()];
        let err = compile_scene_contract(&card, "沈砚拿到名册", "前文。", 0, 1).unwrap_err();
        assert_eq!(err, ContractError::MissingOpposition);
    }

    #[test]
    fn generic_only_errors() {
        let mut card = card();
        card.next_outline_node = "按场景结构推进".into();
        card.change_delta.summary = String::new();
        let err = compile_scene_contract(&card, "", "", 0, 1).unwrap_err();
        assert_eq!(err, ContractError::MissingGoal);
    }

    #[test]
    fn chapter_contract_projects_one_to_three_scenes() {
        let cc = compile_chapter_contract(
            &card(),
            "沈砚拿到半本名册。王爷当众烧名册。证人被王爷的人带走。",
            "前文。",
            7,
            3,
        )
        .unwrap();
        assert!(cc.is_valid(), "{:?}", cc.validate());
        assert!(cc.scenes.len() >= 1 && cc.scenes.len() <= 3);
        assert_eq!(cc.chapter_number, 7);
        assert!(cc.dramatic_question.contains("沈砚"));
        assert!(cc.irreversible_outcome.contains("关系"));
        if cc.scenes.len() > 1 {
            assert_eq!(cc.scenes[1].entry_state, cc.scenes[0].outcome);
        }
    }

    #[test]
    fn chapter_contract_single_scene_does_not_fake_multi_act() {
        let cc = compile_chapter_contract(&card(), "沈砚逼王爷承认名册。", "前文。", 1, 3).unwrap();
        assert_eq!(cc.scenes.len(), 1);
        assert_eq!(cc.scenes[0].index, 0);
        assert_eq!(cc.scenes[0].total, 1);
    }
}
