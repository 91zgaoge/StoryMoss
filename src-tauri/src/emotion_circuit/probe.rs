//! 确定性情感探针（0 LLM）。
//!
//! P0/P1 只记录、不阻断；探针必须能引用原文证据，不能只给「情感不足」。

use serde::{Deserialize, Serialize};

use super::{types::EmotionBeat, vector::EmotionVector};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeSeverity {
    Low,
    Medium,
    High,
}

impl ProbeSeverity {
    pub fn id(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionProbe {
    pub code: String,
    pub severity: ProbeSeverity,
    pub passed: bool,
    pub detail: String,
}

impl EmotionProbe {
    fn pass(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity: ProbeSeverity::Low,
            passed: true,
            detail: detail.into(),
        }
    }

    fn fail(code: &str, severity: ProbeSeverity, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            severity,
            passed: false,
            detail: detail.into(),
        }
    }
}

pub fn run_emotion_probes(text: &str, beat: &EmotionBeat) -> Vec<EmotionProbe> {
    let actual = EmotionVector::from_text(text);
    let has_action = has_any(text, beat.action_tendency.visible_markers());
    let has_cost = count_cost_markers(text) > 0;
    let has_emotion_word = !actual.is_zero();
    let mut probes = Vec::new();

    // 1) 因果：有动作或有代价；只有情绪词没有动作/代价 → 空转。
    if !has_action && !has_cost {
        probes.push(EmotionProbe::fail(
            "emotion_causality",
            ProbeSeverity::High,
            "本拍既没有可见动作/台词，也没有代价/后果；情感停在标签层",
        ));
    } else if has_emotion_word && !has_action {
        probes.push(EmotionProbe::fail(
            "emotion_causality",
            ProbeSeverity::Medium,
            "本拍有情绪词但没有行动倾向落地；读者看不到角色做了什么",
        ));
    } else {
        probes.push(EmotionProbe::pass(
            "emotion_causality",
            "情感有行动或代价支撑",
        ));
    }

    // 2) 目标达成：实际情绪分布与计划 to 的 cosine 相似度。
    if actual.is_zero() {
        probes.push(EmotionProbe::fail(
            "emotion_target",
            ProbeSeverity::Low,
            "正文未提取到明确情绪词，无法核对目标情感（低置信）",
        ));
    } else {
        let sim = 1.0 - beat.to.distance(&actual);
        if sim < 0.35 {
            probes.push(EmotionProbe::fail(
                "emotion_target",
                ProbeSeverity::Medium,
                format!(
                    "目标情感 {} 与正文实际 {} 偏离较大（相似度 {:.2}）",
                    beat.to.render_compact(2),
                    actual.render_compact(2),
                    sim
                ),
            ));
        } else if sim < 0.50 {
            probes.push(EmotionProbe::fail(
                "emotion_target",
                ProbeSeverity::Low,
                format!(
                    "目标情感 {} 与正文实际 {} 部分偏离（相似度 {:.2}）",
                    beat.to.render_compact(2),
                    actual.render_compact(2),
                    sim
                ),
            ));
        } else {
            probes.push(EmotionProbe::pass(
                "emotion_target",
                format!("目标情感达成相似度 {:.2}", sim),
            ));
        }
    }

    // 3) 情感变化：本拍是否真的移动了。
    if !actual.is_zero() {
        let moved = beat.from.distance(&actual);
        if moved < 0.10 {
            probes.push(EmotionProbe::fail(
                "emotion_delta",
                ProbeSeverity::High,
                format!(
                    "本拍情绪从 {} 到 {} 没有实质变化（位移 {:.2}）",
                    beat.from.render_compact(2),
                    actual.render_compact(2),
                    moved
                ),
            ));
        } else {
            probes.push(EmotionProbe::pass(
                "emotion_delta",
                format!("情感位移 {:.2}", moved),
            ));
        }
    }

    // 4) 声音一致：本拍明令禁止的台词是否出现。
    if let Some(ref d) = beat.dialogue_policy {
        let hit = d
            .forbidden
            .iter()
            .find(|w| !w.trim().is_empty() && text.contains(w.trim()));
        if let Some(w) = hit {
            probes.push(EmotionProbe::fail(
                "voice_consistency",
                ProbeSeverity::High,
                format!("角色说了本拍明确禁止的话：{w}"),
            ));
        } else {
            probes.push(EmotionProbe::pass(
                "voice_consistency",
                "未出现本拍台词禁忌",
            ));
        }
    }

    // 5) 张力兑现：升级/揭示/修复/破裂至少一种。
    if beat.tension_id.is_some() {
        if has_any(text, &TENSION_MARKERS) {
            probes.push(EmotionProbe::pass("tension_payoff", "张力有推进动作"));
        } else {
            probes.push(EmotionProbe::fail(
                "tension_payoff",
                ProbeSeverity::Medium,
                "本拍有选定关系张力，但正文没有升级/揭示/修复/破裂的可见动作",
            ));
        }
    }

    // 6) 过载：连续具身化 + 高情绪词密度 + 旁白解释。
    let body = count_markers(text, &BODY_MARKERS);
    let emotion_words = count_markers(text, &EMOTION_WORDS);
    if body >= 3 && emotion_words >= 6 {
        probes.push(EmotionProbe::fail(
            "melodrama",
            ProbeSeverity::Medium,
            format!(
                "情绪过载：身体反应 {body} 处、情绪词 {emotion_words} 处；每个情绪都写成身体反应",
            ),
        ));
    } else if count_markers(text, &EXPLAIN_MARKERS) > 0 {
        probes.push(EmotionProbe::fail(
            "melodrama",
            ProbeSeverity::Medium,
            "叙述替读者解释了情感（「她终于明白」类），属于煽情/主题直说",
        ));
    } else {
        probes.push(EmotionProbe::pass("melodrama", "未见明显情感过载"));
    }

    // 7) 叙述克制：旁白解释单独给一条低严重度提醒。
    let explained = count_markers(text, &EXPLAIN_MARKERS);
    if explained > 0 {
        probes.push(EmotionProbe::fail(
            "reader_restraint",
            ProbeSeverity::Low,
            format!("发现 {explained} 处替读者解释情感/主题的句子"),
        ));
    } else {
        probes.push(EmotionProbe::pass("reader_restraint", "叙述未替读者解释"));
    }

    // 8) 载体稀疏：身体反应堆叠说明载体单一。
    if body >= 4 {
        probes.push(EmotionProbe::fail(
            "carrier_sparsity",
            ProbeSeverity::Low,
            format!("身体反应 {body} 处，载体可能过度集中在具身化"),
        ));
    } else {
        probes.push(EmotionProbe::pass("carrier_sparsity", "载体未明显堆叠"));
    }

    probes
}

const TENSION_MARKERS: [&str; 18] = [
    "揭穿", "承认", "坦白", "摊牌", "公开", "决裂", "翻脸", "离开", "留下", "道歉", "和好", "让步",
    "夺走", "交出", "失去", "欠", "拒绝", "追",
];

const BODY_MARKERS: [&str; 16] = [
    "心口一紧",
    "背脊发凉",
    "喉咙发紧",
    "眼眶一热",
    "鼻子一酸",
    "手指发颤",
    "浑身发抖",
    "呼吸一滞",
    "血液",
    "心脏",
    "胸口",
    "手心",
    "眼泪",
    "泪水",
    "颤抖",
    "发冷",
];

const COST_MARKERS: [&str; 20] = [
    "失去", "丢掉", "让出", "交出", "押上", "撕破", "决裂", "翻脸", "离场", "离开", "赶出", "公开",
    "摊开", "认输", "让步", "跪下", "欠", "伤", "断", "代价",
];

const EXPLAIN_MARKERS: [&str; 10] = [
    "她终于明白",
    "他终于明白",
    "这一刻她懂了",
    "这一刻他懂了",
    "泪水说明了一切",
    "原来这就是",
    "这就是命运",
    "殊不知",
    "无人知晓",
    "命运的齿轮",
];

const EMOTION_WORDS: [&str; 32] = [
    "难过",
    "悲伤",
    "伤心",
    "痛苦",
    "愤怒",
    "生气",
    "恨",
    "害怕",
    "恐惧",
    "不安",
    "羞耻",
    "难堪",
    "愧疚",
    "内疚",
    "爱",
    "心疼",
    "舍不得",
    "希望",
    "绝望",
    "失望",
    "委屈",
    "不甘",
    "嫉妒",
    "厌恶",
    "恶心",
    "惊讶",
    "震惊",
    "欣慰",
    "释然",
    "孤独",
    "温暖",
    "心酸",
];

fn has_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|m| text.contains(m))
}

fn count_markers(text: &str, markers: &[&str]) -> usize {
    markers.iter().map(|m| text.matches(m).count()).sum()
}

fn count_cost_markers(text: &str) -> usize {
    count_markers(text, &COST_MARKERS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emotion_circuit::{
        appraisal::{ActionTendency, Appraisal, EventKind},
        carriers::select_carriers,
        types::{DialoguePolicy, ReaderTarget},
        vector::EmotionVector,
    };

    fn beat() -> EmotionBeat {
        let to = EmotionVector::from_pairs(&[("shame", 0.4), ("anger", 0.6)]);
        EmotionBeat {
            from: EmotionVector::from_pairs(&[("shame", 0.6), ("anger", 0.3)]),
            to: to.clone(),
            trigger_event: "阿岩当众替林雪承认那封信".into(),
            appraisal: Appraisal::default(),
            action_tendency: ActionTendency::Attack,
            action_reason: String::new(),
            dialogue_policy: Some(DialoguePolicy {
                objective: "让他收回那句话".into(),
                tactic: "冷嘲".into(),
                subtext: "你凭什么替我决定".into(),
                forbidden: vec!["谢谢".into()],
            }),
            cost: "她当众推开他，坐实了不识好歹".into(),
            state_changes: vec![],
            carriers: select_carriers(&to, "攻击型", true, false),
            reader_target: ReaderTarget::default(),
            scale: 0.8,
            event_kind: EventKind::SecretReveal,
            source_name: "林雪".into(),
            target_name: "阿岩".into(),
            tension_id: Some("t1".into()),
            debt_id: None,
        }
    }

    #[test]
    fn flat_emotion_text_fails_causality() {
        let text = "她很难过，也很痛苦，心里很不是滋味。";
        let probes = run_emotion_probes(text, &beat());
        let c = probes
            .iter()
            .find(|p| p.code == "emotion_causality")
            .unwrap();
        assert!(!c.passed, "{:?}", c);
    }

    #[test]
    fn action_and_cost_pass_causality() {
        let text = "林雪把伞推回去，冷笑了一声：「你凭什么替我决定。」阿岩失去了解释的机会。";
        let probes = run_emotion_probes(text, &beat());
        let c = probes
            .iter()
            .find(|p| p.code == "emotion_causality")
            .unwrap();
        assert!(c.passed, "{:?}", c);
    }

    #[test]
    fn forbidden_line_fails_voice() {
        let text = "林雪说：「谢谢。」然后转身离开。";
        let probes = run_emotion_probes(text, &beat());
        let v = probes
            .iter()
            .find(|p| p.code == "voice_consistency")
            .unwrap();
        assert!(!v.passed);
    }

    #[test]
    fn explanatory_ending_fails_melodrama() {
        let text = "她终于明白，原来这就是爱。泪水说明了一切。";
        let probes = run_emotion_probes(text, &beat());
        let m = probes.iter().find(|p| p.code == "melodrama").unwrap();
        assert!(!m.passed);
    }

    #[test]
    fn human_restrained_text_does_not_over_fire() {
        let text = "林雪把伞推回去，伞骨磕在门槛上。她说：「知道了。」阿岩没再说话，转身走了。";
        let probes = run_emotion_probes(text, &beat());
        let failed = probes.iter().filter(|p| !p.passed).count();
        assert!(failed <= 2, "probes={probes:?}");
    }
}
