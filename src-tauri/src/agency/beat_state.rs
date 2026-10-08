//! 拍级状态网。设计：docs/plans/2026-08-15-continue-quality-closure-design.md
//! §7

use crate::{
    agency::{
        beat_card::{CastMember, SceneBeatCard},
        continue_director::DirectorLock,
    },
    creative_engine::expansion::debt::QuotaItem,
};

#[derive(Debug, Clone)]
pub struct OpenThread {
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct BeatState {
    pub present: Vec<String>,
    pub locations: Vec<(String, String)>,
    pub threads: Vec<OpenThread>,
    /// 角色表内、本拍未在场的已登记名。探针用来拦场外开篇。
    pub offshot: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BeatProbe {
    pub named_cast: usize,
    pub gaps: Vec<String>,
}

impl BeatState {
    pub fn render_full(&self) -> String {
        let loc = self
            .locations
            .iter()
            .map(|(n, l)| format!("{n}={l}"))
            .collect::<Vec<_>>()
            .join("；");
        let mut lines = vec![
            "【本拍状态网】".into(),
            format!("在场：{}", self.present.join("、")),
            format!("地点：{loc}"),
        ];
        if self.threads.is_empty() {
            lines.push("未决：承接当前冲突，不得原地复述末句。".into());
        } else {
            let t = self
                .threads
                .iter()
                .enumerate()
                .map(|(i, th)| format!("{}. {}", i + 1, th.text))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(format!("未决：{t}"));
        }
        lines.push("在场者可以不出声。禁止写成他们不在场。禁止把同一人的别名写成另一个人。".into());
        lines.join("\n")
    }

    pub fn render_tail_summary(&self) -> String {
        let thread = self
            .threads
            .first()
            .map(|t| t.text.chars().take(40).collect::<String>())
            .unwrap_or_default();
        format!(
            "【状态摘要】在场：{}｜未决：{}",
            self.present.join("、"),
            thread
        )
    }
}

pub fn compile_beat_state(
    present: &[String],
    location: Option<&str>,
    next_node: &str,
    overdue: &[String],
    tail: &str,
    progress_lines: &[String],
) -> BeatState {
    let loc = location.unwrap_or("").trim();
    let locations = if loc.is_empty() {
        vec![]
    } else {
        present
            .iter()
            .map(|n| (n.clone(), loc.to_string()))
            .collect()
    };
    let mut threads: Vec<OpenThread> = Vec::new();
    fn push(threads: &mut Vec<OpenThread>, raw: &str) {
        let t: String = raw.chars().take(80).collect();
        let t = t.trim().to_string();
        if t.is_empty() {
            return;
        }
        if threads.iter().any(|x| x.text == t) {
            return;
        }
        if threads.len() < 5 {
            threads.push(OpenThread { text: t });
        }
    }
    push(&mut threads, next_node);
    for o in overdue {
        push(&mut threads, o);
    }
    for p in progress_lines {
        if threads.len() >= 5 {
            break;
        }
        if p.contains("进度：") {
            push(&mut threads, p.trim());
        }
    }
    const SIGNALS: &[&str] = &["必须", "之前", "否则", "子时", "七日"];
    for sent in tail.split(['。', '！', '？', '\n']) {
        if threads.len() >= 5 {
            break;
        }
        if SIGNALS.iter().any(|s| sent.contains(s)) {
            push(&mut threads, sent);
        }
    }
    BeatState {
        present: present.to_vec(),
        locations,
        threads,
        offshot: Vec::new(),
    }
}

pub fn probe_increment(
    increment: &str,
    card: &SceneBeatCard,
    state: &BeatState,
    quota: &[QuotaItem],
    lock: Option<&DirectorLock>,
) -> BeatProbe {
    probe_increment_ex(increment, card, state, quota, lock, "", "novel")
}

pub fn probe_increment_ex(
    increment: &str,
    card: &SceneBeatCard,
    state: &BeatState,
    quota: &[QuotaItem],
    lock: Option<&DirectorLock>,
    prior_tail: &str,
    story_format: &str,
) -> BeatProbe {
    let mut probe = probe_increment_core(increment, card, state, quota, lock);
    if increment_is_tail_recap(increment, prior_tail) {
        let tokens = change_delta_tokens(&card.change_delta.summary);
        if !tokens.is_empty() && tokens.iter().all(|t| !increment.contains(t.as_str())) {
            probe.gaps.push(format!(
                "本拍未兑现必须改变：{}",
                card.change_delta.kind.as_zh()
            ));
        }
    }
    // v0.64.10：卡要求升级/结账时，不得把上一次交锋换个说法再演一遍
    if card.conflict_move.stage.requires_advance()
        && conflict_repeats_prior(increment, prior_tail, &card.conflict_move.parties)
    {
        probe
            .gaps
            .push("冲突原地复述：本拍必须推进或结账，不得重演上一次交锋".into());
    }
    if story_format == "short_drama" {
        let has_heading = increment.contains("内景") || increment.contains("外景");
        if !has_heading {
            probe.gaps.push("短剧增量缺少场次标头（内景或外景）".into());
        }
    }
    probe
}

/// 增量里是否有句子与「前文」高度相似（同一场对峙换个说法重写）。
///
/// 只比涉及冲突当事人或对抗动词的句子；相似度用字符 bigram Jaccard，
/// 阈值 0.62（真机重演段落实测 0.6+，正常续写在 0.2 以下）。
pub fn conflict_repeats_prior(increment: &str, prior_tail: &str, parties: &[String]) -> bool {
    if increment.trim().is_empty() || prior_tail.trim().is_empty() {
        return false;
    }
    const CONFRONT_VERBS: &[&str] = &[
        "对峙", "对撞", "逼", "拦", "抓住", "扣住", "按住", "盯着", "迎上", "拔", "出手", "动手",
        "挡住", "横在", "顶", "质问", "冷笑", "退后", "上前",
    ];
    let relevant = |sent: &str| -> bool {
        parties
            .iter()
            .any(|p| !p.is_empty() && sent.contains(p.as_str()))
            || CONFRONT_VERBS.iter().any(|v| sent.contains(v))
    };
    let sentences = |text: &str| -> Vec<String> {
        text.split(['。', '！', '？', '\n'])
            .map(|s| s.trim().to_string())
            .filter(|s| s.chars().count() >= 12 && relevant(s))
            .collect()
    };
    let inc = sentences(increment);
    if inc.is_empty() {
        return false;
    }
    let prior = sentences(prior_tail);
    if prior.is_empty() {
        return false;
    }
    inc.iter().any(|a| {
        prior
            .iter()
            .any(|b| crate::utils::text::TextUtils::char_bigram_similarity(a, b) >= 0.62)
    })
}

/// 该级要求升级/结账时，增量是否写出了可见代价或不可逆结果。
pub fn conflict_outcome_landed(increment: &str) -> bool {
    const OUTCOMES: &[&str] = &[
        "失去",
        "丢掉",
        "失了",
        "让出",
        "交出",
        "押上",
        "抵押",
        "撕破",
        "决裂",
        "翻脸",
        "离场",
        "离开",
        "退走",
        "出城",
        "逐出",
        "赶出",
        "擒",
        "押下",
        "关进",
        "锁上",
        "公开",
        "摊开",
        "亮出",
        "认输",
        "让步",
        "跪下",
        "卸下",
        "夺走",
        "拿走",
        "带走",
        "拔刀",
        "见血",
        "死了",
        "断气",
        "落定",
        "收网",
        "尘埃落定",
        "无可挽回",
    ];
    OUTCOMES.iter().any(|m| increment.contains(m))
}

fn probe_increment_core(
    increment: &str,
    card: &SceneBeatCard,
    state: &BeatState,
    quota: &[QuotaItem],
    lock: Option<&DirectorLock>,
) -> BeatProbe {
    let cast_names: Vec<String> = card.cast.iter().map(|c| c.name.clone()).collect();
    let matched = crate::agency::continue_assets::match_character_names(&cast_names, increment);
    let named_cast = matched.len();
    let mut gaps = Vec::new();
    if quota.contains(&QuotaItem::ConflictEscalation) {
        let living_parties: Vec<&String> = card
            .conflict_move
            .parties
            .iter()
            .filter(|p| !card.dead.iter().any(|d| d == *p))
            .collect();
        let one_living = living_parties
            .iter()
            .any(|p| matched.iter().any(|n| n == *p));
        let verb = crate::agency::continue_assets::has_conflict_verb(increment);
        if !living_parties.is_empty() && !one_living && !verb {
            gaps.push("未落实冲突加压".into());
        }
    }
    // v0.64.10：卡已要求升级/结账时，「又一次同席对峙」不算推进——必须有
    // 可见代价或不可逆结果（否则重试；重试后仍在则入质量债）。
    if card.conflict_move.stage.requires_advance() {
        let living_parties: Vec<&String> = card
            .conflict_move
            .parties
            .iter()
            .filter(|p| !card.dead.iter().any(|d| d == *p))
            .collect();
        let involved = living_parties
            .iter()
            .any(|p| matched.iter().any(|n| n == *p));
        if involved && !conflict_outcome_landed(increment) {
            gaps.push("冲突未升级：本拍必须写出可见代价或不可逆结果，不得只再对峙一次".into());
        }
    }
    if quota.contains(&QuotaItem::NewScene) {
        let stayed = card
            .setting_location
            .as_deref()
            .map(|loc| increment.contains(loc))
            .unwrap_or(false);
        let moved = ["离开", "潜入", "进入", "前往"]
            .iter()
            .any(|v| increment.contains(v));
        if stayed || !moved {
            gaps.push("未离开当前场景".into());
        }
    }
    if quota.contains(&QuotaItem::CharacterMove) {
        let silent: Vec<&CastMember> = card
            .cast
            .iter()
            .filter(|c| c.purpose.contains("沉寂") || c.purpose.contains("入场"))
            .collect();
        if silent.iter().any(|c| !matched.iter().any(|n| n == &c.name)) {
            gaps.push("沉寂角色未入场".into());
        }
    }
    if !quota.contains(&QuotaItem::NewScene) && !state.offshot.is_empty() {
        let opening: String = increment.chars().take(80).collect();
        let off = crate::agency::continue_assets::match_character_names(&state.offshot, &opening);
        let on = crate::agency::continue_assets::match_character_names(&state.present, &opening);
        if !off.is_empty() && on.is_empty() {
            gaps.push("增量以场外角色开篇".into());
        }
    }
    if crate::agency::continue_assets::increment_replays_completed_deaths(increment, &card.dead) {
        gaps.push("重演已完成的死亡或行刺".into());
    }
    if let Some(lock) = lock {
        gaps.extend(crate::agency::continue_director::subject_split_gaps(
            increment, lock,
        ));
        gaps.extend(crate::agency::continue_director::kin_inversion_gaps(
            increment, lock,
        ));
        gaps.extend(crate::agency::continue_director::dead_acting_gaps(
            increment, lock,
        ));
    }
    BeatProbe { named_cast, gaps }
}

fn compact_ws(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn increment_is_tail_recap(increment: &str, prior_tail: &str) -> bool {
    if increment.chars().count() < 20 || prior_tail.trim().is_empty() {
        return false;
    }
    let inc = compact_ws(increment);
    let tail = compact_ws(prior_tail);
    !inc.is_empty() && tail.contains(&inc)
}

fn change_delta_tokens(summary: &str) -> Vec<String> {
    const SKIP: &[&str] = &["必须", "本拍", "不得", "只靠", "对话", "过渡", "禁止", "与"];
    summary
        .split(|c: char| c.is_whitespace() || "，。；：、·—－-「」\"\"''（）()【】".contains(c))
        .map(str::trim)
        .filter(|s| s.chars().count() >= 2 && !SKIP.contains(s))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::beat_card::{
        CastMember, ChangeDelta, ChangeKind, ConflictMove, EmotionBeat, SceneBeatCard,
    };

    #[test]
    fn beat_state_includes_next_node_and_overdue() {
        let st = compile_beat_state(
            &["沈砚".into(), "白芷".into()],
            Some("钟楼"),
            "子时前破金煞",
            &["五阵未破".into()],
            "沈砚握着罗盘。必须在子时前动手，否则龙脉裂口。",
            &["进度：灵堂托梦".to_string()],
        );
        assert!(st.present.contains(&"沈砚".into()));
        assert!(st.locations.iter().any(|(n, l)| n == "沈砚" && l == "钟楼"));
        assert!(st
            .threads
            .iter()
            .any(|t| t.text.contains("金煞") || t.text.contains("子时")));
        assert!(st.threads.iter().any(|t| t.text.contains("五阵")));
        assert!(st.threads.len() <= 5);
        let full = st.render_full();
        assert!(full.contains("【本拍状态网】"));
        assert!(full.contains("未决"));
    }

    #[test]
    fn probe_reports_missing_cast_and_unshifted_location() {
        let card = SceneBeatCard {
            cast: vec![
                CastMember {
                    name: "阿岩".into(),
                    purpose: "末段已在场".into(),
                },
                CastMember {
                    name: "林雪".into(),
                    purpose: "末段已在场".into(),
                },
            ],
            conflict_move: ConflictMove {
                action: "加压".into(),
                parties: vec!["阿岩".into(), "林雪".into()],
                stage: crate::agency::beat_card::default_conflict_stage(),
            },
            emotion_beat: EmotionBeat {
                summary: "怒".into(),
            },
            next_outline_node: "夜宴破裂".into(),
            expansion_quota: vec![QuotaItem::NewScene, QuotaItem::ConflictEscalation],
            expansion_quota_text: None,
            setting_location: Some("雨巷".into()),
            open_review_issues: vec![],
            dead: vec![],
            change_delta: ChangeDelta::default(),
        };
        let state = BeatState {
            present: vec!["阿岩".into(), "林雪".into()],
            locations: vec![("阿岩".into(), "雨巷".into())],
            threads: vec![],
            offshot: vec![],
        };
        let probe = probe_increment(
            "他叹了口气，继续喝茶。",
            &card,
            &state,
            &[QuotaItem::NewScene, QuotaItem::ConflictEscalation],
            None,
        );
        assert!(!probe.gaps.is_empty());
        assert!(probe.gaps.join("").contains("在场") || probe.named_cast < 2);
    }

    #[test]
    fn probe_rejects_offshot_pov_opening() {
        let card = SceneBeatCard {
            cast: vec![
                CastMember {
                    name: "苏亦铁".into(),
                    purpose: "末段已在场".into(),
                },
                CastMember {
                    name: "曹元佩".into(),
                    purpose: "末段已在场".into(),
                },
            ],
            conflict_move: ConflictMove {
                action: "加压".into(),
                parties: vec!["苏亦铁".into(), "曹元佩".into()],
                stage: crate::agency::beat_card::default_conflict_stage(),
            },
            emotion_beat: EmotionBeat {
                summary: "惊".into(),
            },
            next_outline_node: "留在大堂".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("镇北王府大堂".into()),
            open_review_issues: vec![],
            dead: vec![],
            change_delta: ChangeDelta::default(),
        };
        let state = BeatState {
            present: vec!["苏亦铁".into(), "曹元佩".into()],
            locations: vec![("苏亦铁".into(), "镇北王府大堂".into())],
            threads: vec![],
            offshot: vec!["费迪南三世".into()],
        };
        let probe = probe_increment(
            "费迪南三世在都城宫殿里批阅奏折，烟火节的税单堆满御案。",
            &card,
            &state,
            &[],
            None,
        );
        assert!(
            probe.gaps.iter().any(|g| g.contains("场外")),
            "须拦截场外开篇 gaps={:?}",
            probe.gaps
        );
    }

    #[test]
    fn probe_rejects_replay_of_completed_stab() {
        let card = SceneBeatCard {
            cast: vec![
                CastMember {
                    name: "苏亦铁".into(),
                    purpose: "末段已在场".into(),
                },
                CastMember {
                    name: "景亲王".into(),
                    purpose: "末段已在场".into(),
                },
            ],
            conflict_move: ConflictMove {
                action: "加压".into(),
                parties: vec!["苏亦铁".into(), "景亲王".into()],
                stage: crate::agency::beat_card::default_conflict_stage(),
            },
            emotion_beat: EmotionBeat {
                summary: "悲愤".into(),
            },
            next_outline_node: "当众驳斥谋反".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("镇北王府大堂".into()),
            open_review_issues: vec![],
            dead: vec!["苏会山".into(), "明成公主".into()],
            change_delta: ChangeDelta::default(),
        };
        let state = BeatState {
            present: vec!["苏亦铁".into(), "景亲王".into()],
            locations: vec![("苏亦铁".into(), "镇北王府大堂".into())],
            threads: vec![],
            offshot: vec![],
        };
        let rewind = "明成公主将短刃狠狠刺入了苏会山的胸口。苏会山头脸崩裂。";
        let probe = probe_increment(rewind, &card, &state, &[], None);
        assert!(
            probe.gaps.iter().any(|g| g.contains("重演")),
            "须拦截重演刺杀 gaps={:?}",
            probe.gaps
        );
        let forward = "苏亦铁扑向父亲的尸体。景亲王的护卫大喊谋反。曹元佩僵在座上。";
        let ok = probe_increment(forward, &card, &state, &[], None);
        assert!(
            !ok.gaps.iter().any(|g| g.contains("重演")),
            "点名尸体不得算重演 gaps={:?}",
            ok.gaps
        );
    }

    #[test]
    fn probe_does_not_gap_silent_present() {
        let card = SceneBeatCard {
            cast: vec![
                CastMember {
                    name: "苏亦铁".into(),
                    purpose: "可沉默".into(),
                },
                CastMember {
                    name: "曹元佩".into(),
                    purpose: "可沉默".into(),
                },
            ],
            conflict_move: ConflictMove {
                action: "加压".into(),
                parties: vec!["苏亦铁".into()],
                stage: crate::agency::beat_card::default_conflict_stage(),
            },
            emotion_beat: EmotionBeat {
                summary: "悲".into(),
            },
            next_outline_node: String::new(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("大堂".into()),
            open_review_issues: vec![],
            dead: vec!["苏会山".into()],
            change_delta: ChangeDelta::default(),
        };
        let state = BeatState {
            present: vec!["苏亦铁".into(), "曹元佩".into()],
            locations: vec![],
            threads: vec![],
            offshot: vec![],
        };
        let probe = probe_increment(
            "苏亦铁扑向父亲的尸体，指尖触到冰冷的骨骼。",
            &card,
            &state,
            &[],
            None,
        );
        assert!(
            !probe.gaps.iter().any(|g| g.contains("丢掉已在场者")),
            "gaps={:?}",
            probe.gaps
        );
    }

    fn recap_card() -> SceneBeatCard {
        SceneBeatCard {
            cast: vec![CastMember {
                name: "阿岩".into(),
                purpose: "在场".into(),
            }],
            conflict_move: ConflictMove {
                action: "加压".into(),
                parties: vec!["阿岩".into()],
                stage: crate::agency::beat_card::default_conflict_stage(),
            },
            emotion_beat: EmotionBeat {
                summary: "怒".into(),
            },
            next_outline_node: "夜宴破裂".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("雨巷".into()),
            open_review_issues: vec![],
            dead: vec![],
            change_delta: ChangeDelta {
                kind: ChangeKind::Information,
                summary: "夜宴破裂".into(),
            },
        }
    }

    #[test]
    fn probe_gaps_when_increment_is_tail_recap() {
        let card = recap_card();
        let state = BeatState {
            present: vec!["阿岩".into()],
            locations: vec![],
            threads: vec![],
            offshot: vec![],
        };
        let tail =
            "阿岩站在雨里一动不动。顾长夜冷笑。众人不敢出声。阿岩站在雨里一动不动。顾长夜冷笑。众人不敢出声。";
        let increment = "阿岩站在雨里一动不动。顾长夜冷笑。众人不敢出声。";
        let probe = probe_increment_ex(increment, &card, &state, &[], None, tail, "novel");
        assert!(
            probe
                .gaps
                .iter()
                .any(|g| g.contains("本拍未兑现必须改变：信息")),
            "复述近文且未出现改变项词须 gap gaps={:?}",
            probe.gaps
        );
    }

    #[test]
    fn probe_does_not_gap_literary_aside_when_not_recap() {
        let card = recap_card();
        let state = BeatState {
            present: vec!["阿岩".into()],
            locations: vec![],
            threads: vec![],
            offshot: vec![],
        };
        let tail = "阿岩站在雨里。顾长夜冷笑。";
        let increment = "雨丝斜织，青石上的灯影一颤，像有人把旧账翻到了这一页。";
        let probe = probe_increment_ex(increment, &card, &state, &[], None, tail, "novel");
        assert!(
            !probe.gaps.iter().any(|g| g.contains("未兑现必须改变")),
            "非复述旁白不得当原地踏步 gaps={:?}",
            probe.gaps
        );
    }

    #[test]
    fn probe_short_drama_requires_scene_heading() {
        let card = recap_card();
        let state = BeatState::default();
        let bare = probe_increment_ex(
            "阿岩推门。顾长夜冷笑。酒盏翻倒。",
            &card,
            &state,
            &[],
            None,
            "",
            "short_drama",
        );
        assert!(
            bare.gaps.iter().any(|g| g.contains("场次标头")),
            "gaps={:?}",
            bare.gaps
        );
        let headed = probe_increment_ex(
            "1. 内景・雨巷酒肆・夜\n阿岩推门。顾长夜冷笑。",
            &card,
            &state,
            &[],
            None,
            "",
            "short_drama",
        );
        assert!(
            !headed.gaps.iter().any(|g| g.contains("场次标头")),
            "gaps={:?}",
            headed.gaps
        );
    }

    fn staged_card(stage: crate::agency::beat_card::ConflictStage) -> SceneBeatCard {
        SceneBeatCard {
            cast: vec![CastMember {
                name: "阿岩".into(),
                purpose: "末段已在场".into(),
            }],
            conflict_move: ConflictMove {
                action: crate::agency::beat_card::conflict_line(stage, "阿岩", Some("林雪"), ""),
                parties: vec!["阿岩".into(), "林雪".into()],
                stage,
            },
            emotion_beat: EmotionBeat {
                summary: "怒".into(),
            },
            next_outline_node: "夜宴破裂".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("夜宴厅".into()),
            open_review_issues: vec![],
            dead: vec![],
            change_delta: ChangeDelta {
                kind: ChangeKind::Risk,
                summary: "加压".into(),
            },
        }
    }

    /// v0.64.10：卡要求升级时，「又一次同席对峙」不算推进 → 探针出缺口
    #[test]
    fn escalate_stage_flags_confrontation_without_outcome() {
        let card = staged_card(crate::agency::beat_card::ConflictStage::Escalate);
        let state = BeatState {
            present: vec!["阿岩".into(), "林雪".into()],
            locations: vec![],
            threads: vec![],
            offshot: vec![],
        };
        let increment =
            "阿岩与林雪在廊下再次对峙，两人谁也不肯先开口。阿岩攥紧了拳头，林雪冷冷看着他。";
        let probe = probe_increment_ex(increment, &card, &state, &[], None, "", "novel");
        assert!(
            probe.gaps.iter().any(|g| g.contains("冲突未升级")),
            "gaps={:?}",
            probe.gaps
        );

        // 写出可见代价/不可逆结果 → 不再报
        let landed = "阿岩把铜印按进火里，失去右手虎口；林雪当场撕破脸，宣布断交退走。";
        let probe = probe_increment_ex(landed, &card, &state, &[], None, "", "novel");
        assert!(
            !probe.gaps.iter().any(|g| g.contains("冲突未升级")),
            "gaps={:?}",
            probe.gaps
        );
    }

    /// 加压级不要求代价（首拍就是摊牌，别把正常开场判成缺口）
    #[test]
    fn press_stage_does_not_demand_outcome() {
        let card = staged_card(crate::agency::beat_card::ConflictStage::Press);
        let state = BeatState {
            present: vec!["阿岩".into(), "林雪".into()],
            locations: vec![],
            threads: vec![],
            offshot: vec![],
        };
        let probe = probe_increment_ex(
            "阿岩与林雪在廊下正面对峙，两人谁也不肯先开口。",
            &card,
            &state,
            &[],
            None,
            "",
            "novel",
        );
        assert!(
            !probe.gaps.iter().any(|g| g.contains("冲突未升级")),
            "gaps={:?}",
            probe.gaps
        );
    }

    /// v0.64.10：卡要求升级时，把上一次交锋换个说法再演 → 判「冲突原地复述」
    #[test]
    fn conflict_repeat_against_prior_tail_is_flagged() {
        let prior = "明成公主往前迈了一步，苏亦铁转过身，两人的目光在穿堂里撞上。\
                     苏亦铁按住棺沿，指节发白。";
        let repeat = "明成公主往前迈了一步，苏亦铁转过身，两人目光在穿堂中撞上。";
        assert!(conflict_repeats_prior(
            repeat,
            prior,
            &["明成公主".to_string(), "苏亦铁".to_string()]
        ));

        let card = staged_card(crate::agency::beat_card::ConflictStage::Settle);
        let state = BeatState {
            present: vec!["阿岩".into(), "林雪".into()],
            locations: vec![],
            threads: vec![],
            offshot: vec![],
        };
        // 探针按卡上的当事人判定（阿岩/林雪）
        let prior_own = "阿岩往前迈了一步，林雪转过身，两人的目光在穿堂里撞上。\
                         林雪按住棺沿，指节发白。";
        let repeat_own = "阿岩往前迈了一步，林雪转过身，两人目光在穿堂中撞上。";
        let probe = probe_increment_ex(repeat_own, &card, &state, &[], None, prior_own, "novel");
        assert!(
            probe.gaps.iter().any(|g| g.contains("冲突原地复述")),
            "gaps={:?}",
            probe.gaps
        );

        // 全新的推进不会误报
        let fresh = "苏福贵把名册塞进棺缝，曹元佩扣死棺盖，船队顺水下滩，两人再无话。";
        assert!(!conflict_repeats_prior(
            fresh,
            prior,
            &["明成公主".to_string(), "苏亦铁".to_string()]
        ));
    }
}
