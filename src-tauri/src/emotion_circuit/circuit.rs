//! 情感压力场与下一拍选择（emotion-first planning）。
//!
//! 先选最高压的「角色-关系-债务」组合，再让这个组合决定本拍事件的情感
//! 读法；情节节点只是承载情感转折的容器。

use super::{
    appraisal::{
        action_tendency, appraise_event, dialogue_policy, emotion_from_appraisal,
        sensitivities_from, ActionTendency, EventKind,
    },
    carriers::select_carriers,
    types::{
        CharacterEmotionState, EmotionBeat, EmotionContract, EmotionDebt, EmotionTension,
        ReaderTarget,
    },
    vector::{EmotionKind, EmotionVector},
};

#[derive(Debug, Clone, Default)]
pub struct CastEmotionProfile {
    pub name: String,
    pub wound: Option<String>,
    pub need: Option<String>,
    pub core: Option<String>,
    pub defense: Option<String>,
}

impl CastEmotionProfile {
    pub fn from_parts(
        name: &str,
        wound: Option<&str>,
        need: Option<&str>,
        core: Option<&str>,
    ) -> Self {
        Self {
            name: name.to_string(),
            wound: wound.map(str::to_string),
            need: need.map(str::to_string),
            core: core.map(str::to_string),
            defense: None,
        }
    }

    fn wound(&self) -> &str {
        self.wound.as_deref().unwrap_or("")
    }

    fn need(&self) -> &str {
        self.need.as_deref().unwrap_or("")
    }

    fn core(&self) -> &str {
        self.core.as_deref().unwrap_or("")
    }

    fn defense(&self) -> &str {
        self.defense.as_deref().unwrap_or("")
    }
}

pub struct BeatSelectionInput<'a> {
    pub chapter_number: i32,
    pub contract: Option<&'a EmotionContract>,
    pub cast: &'a [CastEmotionProfile],
    pub tensions: &'a [EmotionTension],
    pub debts: &'a [EmotionDebt],
    pub states: &'a [CharacterEmotionState],
    pub candidates: &'a [String],
    pub location: Option<&'a str>,
    pub peak_used: u32,
}

#[derive(Debug, Clone)]
pub struct EmotionPlan {
    pub beat: EmotionBeat,
    pub tension: Option<EmotionTension>,
    pub debt: Option<EmotionDebt>,
    pub pressure: f32,
    pub event_kind: EventKind,
    pub plot_hint: String,
}

/// 选择本拍情感回路。纯函数、给定输入确定。
pub fn select_next_beat(input: &BeatSelectionInput<'_>) -> EmotionPlan {
    let tension = select_tension(input.tensions, input.chapter_number);
    let debt = select_debt(input.debts, input.chapter_number);
    let source_name = tension
        .as_ref()
        .map(|t| t.source_name.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| input.cast.first().map(|c| c.name.clone()))
        .unwrap_or_else(|| "主角".into());
    let target_name = tension
        .as_ref()
        .map(|t| t.target_name.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            input
                .cast
                .iter()
                .find(|c| c.name != source_name)
                .map(|c| c.name.clone())
        })
        .unwrap_or_else(|| "对方".into());
    let profile = input
        .cast
        .iter()
        .find(|c| c.name == source_name)
        .cloned()
        .unwrap_or_default();
    let from = latest_state(input.states, &source_name)
        .map(|s| s.vector.clone())
        .filter(|v| !v.is_zero())
        .unwrap_or_else(|| initial_vector(&profile));
    let event_kind = tension
        .as_ref()
        .map(|t| map_tension_type(&t.tension_type))
        .or_else(|| debt.as_ref().map(|d| map_debt_kind(&d.kind)))
        .unwrap_or(EventKind::Threat);
    let sensitivities = sensitivities_from(profile.wound(), profile.need(), profile.core());
    let appraisal = appraise_event(event_kind, Some(&target_name), &sensitivities);
    let mut to = emotion_from_appraisal(event_kind, &appraisal, &sensitivities);
    if from.distance(&to) < 0.10 {
        let alternative = emotion_from_appraisal(
            EventKind::SecretReveal,
            &appraise_event(EventKind::SecretReveal, Some(&target_name), &sensitivities),
            &sensitivities,
        );
        if from.distance(&alternative) > from.distance(&to) {
            to = alternative;
        }
    }
    let (tendency, action_reason) = action_tendency(&to, profile.defense());
    let dialogue = dialogue_policy(tendency, Vec::new());
    let pressure = pressure_of(
        tension.as_ref(),
        debt.as_ref(),
        &from,
        &to,
        input.chapter_number,
    );
    let public = is_public_place(input.location);
    let action_scene = matches!(
        event_kind,
        EventKind::Threat
            | EventKind::Betrayal
            | EventKind::Humiliation
            | EventKind::Sacrifice
            | EventKind::Reunion
    );
    let carriers = select_carriers(&to, profile.defense(), public, action_scene);
    let scale = choose_scale(input.contract, input.peak_used, pressure);
    let trigger_event = input
        .candidates
        .iter()
        .map(|s| s.trim())
        .find(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format!(
                "{} 与 {} 之间积压的{}逼近临界点",
                source_name,
                target_name,
                tension
                    .as_ref()
                    .map(|t| t.tension_type.as_str())
                    .unwrap_or("情感张力")
            )
        });
    let cost = cost_for(event_kind, &tension, &source_name, &target_name);
    let reader_target = reader_target_for(&to, &from, &source_name, &tension);
    let state_changes = vec![format!(
        "{}：{} → {}",
        source_name,
        from.render_compact(3),
        to.render_compact(3)
    )];
    let beat = EmotionBeat {
        from,
        to,
        trigger_event: trigger_event.clone(),
        appraisal,
        action_tendency: tendency,
        action_reason,
        dialogue_policy: Some(dialogue),
        cost,
        state_changes,
        carriers,
        reader_target,
        scale,
        event_kind,
        source_name,
        target_name,
        tension_id: tension.as_ref().map(|t| t.id.clone()),
        debt_id: debt.as_ref().map(|d| d.id.clone()),
    };
    EmotionPlan {
        beat,
        tension,
        debt,
        pressure,
        event_kind,
        plot_hint: trigger_event,
    }
}

fn select_tension(tensions: &[EmotionTension], chapter: i32) -> Option<EmotionTension> {
    tensions
        .iter()
        .filter(|t| t.is_open())
        .max_by(|a, b| {
            tension_score(a, chapter)
                .partial_cmp(&tension_score(b, chapter))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

fn tension_score(t: &EmotionTension, chapter: i32) -> f32 {
    let due = if t.is_due(chapter) { 0.30 } else { 0.0 };
    let stale = if t
        .last_escalated_chapter
        .map(|c| chapter - c >= 3)
        .unwrap_or(true)
    {
        0.10
    } else {
        0.0
    };
    t.pressure.clamp(0.0, 1.0) + due + stale + (t.accumulated_beats as f32 * 0.01).min(0.15)
}

fn select_debt(debts: &[EmotionDebt], chapter: i32) -> Option<EmotionDebt> {
    debts
        .iter()
        .filter(|d| d.is_open())
        .max_by(|a, b| {
            let sa = a.pressure + if a.is_due(chapter) { 0.35 } else { 0.0 };
            let sb = b.pressure + if b.is_due(chapter) { 0.35 } else { 0.0 };
            sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

fn latest_state<'a>(
    states: &'a [CharacterEmotionState],
    name: &str,
) -> Option<&'a CharacterEmotionState> {
    states
        .iter()
        .filter(|s| s.character_name == name || s.character_id == name)
        .max_by(|a, b| {
            let ca = (a.chapter_number.unwrap_or(0), a.beat_index.unwrap_or(0));
            let cb = (b.chapter_number.unwrap_or(0), b.beat_index.unwrap_or(0));
            ca.cmp(&cb)
        })
}

fn initial_vector(profile: &CastEmotionProfile) -> EmotionVector {
    let core = profile.core();
    let need = profile.need();
    let wound = profile.wound();
    let mut pairs: Vec<(&str, f32)> = Vec::new();
    if core.contains('怒') || core.contains('恨') || core.contains('暴') {
        pairs.push(("anger", 0.5));
        pairs.push(("shame", 0.2));
    } else if core.contains('悲') || core.contains('抑') || core.contains('忍') {
        pairs.push(("sadness", 0.45));
        pairs.push(("love", 0.25));
    } else if core.contains('怕') || core.contains("不安") {
        pairs.push(("fear", 0.5));
        pairs.push(("shame", 0.2));
    } else if core.contains('冷') || core.contains("漠") {
        pairs.push(("shame", 0.35));
        pairs.push(("disgust", 0.2));
    }
    if need.contains("认可") || need.contains("看见") {
        pairs.push(("shame", 0.2));
        pairs.push(("hope", 0.2));
    }
    if need.contains("爱") || need.contains("归属") || need.contains("留下") {
        pairs.push(("love", 0.25));
        pairs.push(("trust", 0.15));
    }
    if wound.contains("背叛") || wound.contains("欺骗") {
        pairs.push(("anger", 0.2));
        pairs.push(("trust", 0.05));
    }
    if pairs.is_empty() {
        pairs.push(("trust", 0.3));
        pairs.push(("hope", 0.2));
    }
    EmotionVector::from_pairs(&pairs)
}

fn map_tension_type(tension: &str) -> EventKind {
    let t = tension.to_string();
    if t.contains("欺骗") || t.contains("谎言") || t.contains("隐瞒") {
        EventKind::SecretReveal
    } else if t.contains("复仇") || t.contains("报复") {
        EventKind::Betrayal
    } else if t.contains("毁灭") || t.contains("对抗") || t.contains("恨") {
        EventKind::Threat
    } else if t.contains("执念") || t.contains("痴迷") {
        EventKind::Rejection
    } else if t.contains("嫉妒") || t.contains("妒") {
        EventKind::Humiliation
    } else if t.contains("愧疚") || t.contains("内疚") {
        EventKind::Reunion
    } else {
        EventKind::Threat
    }
}

fn map_debt_kind(kind: &str) -> EventKind {
    match kind {
        "promise" => EventKind::Reunion,
        "rift" => EventKind::SecretReveal,
        "sacrifice" => EventKind::Sacrifice,
        "secret" => EventKind::SecretReveal,
        "unpaid_kindness" => EventKind::Help,
        _ => EventKind::Threat,
    }
}

fn pressure_of(
    tension: Option<&EmotionTension>,
    debt: Option<&EmotionDebt>,
    from: &EmotionVector,
    to: &EmotionVector,
    chapter: i32,
) -> f32 {
    let t = tension.map(|t| t.pressure.clamp(0.0, 1.0)).unwrap_or(0.0);
    let d = debt
        .map(|d| d.pressure + if d.is_due(chapter) { 0.2 } else { 0.0 })
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let delta = from.distance(to).clamp(0.0, 1.0);
    (0.45 * t + 0.25 * d + 0.30 * delta).clamp(0.0, 1.0)
}

fn choose_scale(contract: Option<&EmotionContract>, peak_used: u32, pressure: f32) -> f32 {
    let budget = contract.map(|c| c.peak_budget()).unwrap_or(1);
    if pressure >= 0.80 && peak_used < budget {
        1.2
    } else if pressure >= 0.45 {
        0.8
    } else {
        0.4
    }
}

fn is_public_place(location: Option<&str>) -> bool {
    let Some(loc) = location else {
        return false;
    };
    ["宴", "殿", "堂", "街", "市", "朝", "广场", "店", "院", "厅"]
        .iter()
        .any(|k| loc.contains(k))
}

fn cost_for(
    event: EventKind,
    tension: &Option<EmotionTension>,
    source: &str,
    target: &str,
) -> String {
    let t = tension
        .as_ref()
        .map(|t| t.tension_type.as_str())
        .unwrap_or("");
    if t.contains("欺骗") || matches!(event, EventKind::SecretReveal) {
        return format!("真相出口后，{source} 失去体面，{target} 失去对 {source} 的信任");
    }
    if t.contains("对抗") || matches!(event, EventKind::Threat) {
        return format!("{source} 与 {target} 必须有人失去筹码或当众退让");
    }
    if t.contains("执念") || matches!(event, EventKind::Rejection) {
        return format!("{source} 要承认自己不被选择，失去最后的自尊");
    }
    if t.contains("毁灭") {
        return format!("{source} 要为自己的冲动付出不可逆的代价");
    }
    if t.contains("复仇") || matches!(event, EventKind::Betrayal) {
        return format!("{source} 要暴露自己最不愿被看见的伤");
    }
    if t.contains("嫉妒") || matches!(event, EventKind::Humiliation) {
        return format!("{source} 要承认自己不如 {target}，或毁掉 {target}");
    }
    if t.contains("愧疚") || matches!(event, EventKind::Reunion) {
        return format!("{source} 要承认自己当年对 {target} 做了什么");
    }
    if matches!(event, EventKind::Sacrifice) {
        return format!("{source} 要替 {target} 承担一个无法收回的代价");
    }
    format!("{source} 必须为一个具体选择付出可见代价")
}

fn reader_target_for(
    to: &EmotionVector,
    from: &EmotionVector,
    source: &str,
    tension: &Option<EmotionTension>,
) -> ReaderTarget {
    let (dominant, _) = to.dominant();
    let (primary, secondary) = match dominant {
        EmotionKind::Shame => ("难堪", Some("心疼")),
        EmotionKind::Anger => ("紧张", Some("替角色不平")),
        EmotionKind::Fear => ("不安", Some("警觉")),
        EmotionKind::Sadness => ("心疼", Some("失落")),
        EmotionKind::Love => ("温柔", Some("不舍")),
        EmotionKind::Guilt => ("复杂同情", Some("不安")),
        EmotionKind::Hope => ("希望", Some("忐忑")),
        EmotionKind::Pride => ("扬眉", Some("敬意")),
        EmotionKind::Disgust => ("反感", Some("快意")),
        EmotionKind::Surprise => ("错愕", Some("好奇")),
        EmotionKind::Joy => ("释然", Some("暖意")),
        EmotionKind::Trust => ("安心", Some("牵挂")),
    };
    let why = format!(
        "因为 {} 带着{}进入本拍{}，且必须为一个具体选择付出代价",
        source,
        from.render_compact(2),
        tension
            .as_ref()
            .map(|t| format!("（{}）", t.tension_type))
            .unwrap_or_default()
    );
    ReaderTarget {
        primary: primary.into(),
        secondary: secondary.map(str::to_string),
        why,
        restraint: "不要替读者哭，不要解释主题；把情绪留给动作、台词和留白。".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emotion_circuit::types::{EmotionContract, EmotionTension};

    fn profile(name: &str) -> CastEmotionProfile {
        CastEmotionProfile::from_parts(
            name,
            Some("童年被师父背叛"),
            Some("渴望被认可"),
            Some("表面冷漠"),
        )
    }

    fn tension() -> EmotionTension {
        EmotionTension {
            id: "t1".into(),
            story_id: "s1".into(),
            source_character_id: "c1".into(),
            target_character_id: Some("c2".into()),
            source_name: "林雪".into(),
            target_name: "阿岩".into(),
            tension_type: "未揭穿的欺骗".into(),
            bond: Some("感激".into()),
            reverse_bond: Some("戒备".into()),
            pressure: 0.82,
            accumulated_beats: 3,
            due_chapter: Some(5),
            status: "open".into(),
            last_escalated_chapter: Some(2),
            updated_at: String::new(),
        }
    }

    #[test]
    fn selects_highest_pressure_tension_and_creates_causal_chain() {
        let cast = vec![profile("林雪"), profile("阿岩")];
        let tensions = vec![tension()];
        let candidates = vec!["阿岩当众替林雪承认那封信".to_string()];
        let input = BeatSelectionInput {
            chapter_number: 5,
            contract: None,
            cast: &cast,
            tensions: &tensions,
            debts: &[],
            states: &[],
            candidates: &candidates,
            location: Some("夜宴厅"),
            peak_used: 0,
        };
        let plan = select_next_beat(&input);
        assert_eq!(plan.beat.source_name, "林雪");
        assert_eq!(plan.beat.target_name, "阿岩");
        assert!(plan.beat.from.distance(&plan.beat.to) >= 0.10);
        assert_eq!(plan.event_kind, EventKind::SecretReveal);
        assert!(!plan.beat.cost.is_empty());
        assert!(!plan.beat.carriers.is_empty());
        assert!(plan.beat.scale >= 0.8);
        assert_eq!(plan.plot_hint, "阿岩当众替林雪承认那封信");
    }

    #[test]
    fn scale_respects_peak_budget() {
        let cast = vec![profile("林雪")];
        let contract = EmotionContract {
            peak_budget: 1,
            ..Default::default()
        };
        let input = BeatSelectionInput {
            chapter_number: 5,
            contract: Some(&contract),
            cast: &cast,
            tensions: &[tension()],
            debts: &[],
            states: &[],
            candidates: &[],
            location: None,
            peak_used: 1,
        };
        let plan = select_next_beat(&input);
        assert!(plan.beat.scale < 1.2);
    }

    #[test]
    fn no_tension_still_produces_non_empty_beat() {
        let cast = vec![profile("林雪")];
        let input = BeatSelectionInput {
            chapter_number: 1,
            contract: None,
            cast: &cast,
            tensions: &[],
            debts: &[],
            states: &[],
            candidates: &[],
            location: None,
            peak_used: 0,
        };
        let plan = select_next_beat(&input);
        assert!(!plan.beat.trigger_event.is_empty());
        assert!(!plan.beat.cost.is_empty());
        assert!(!plan.beat.reader_target.primary.is_empty());
    }

    #[test]
    fn due_debt_raises_pressure() {
        let cast = vec![profile("林雪")];
        let debts = vec![EmotionDebt {
            id: "d1".into(),
            story_id: "s1".into(),
            kind: "promise".into(),
            source_ref: None,
            description: "答应过带她离开".into(),
            pressure: 0.7,
            due_chapter: Some(5),
            status: "open".into(),
            created_at: String::new(),
        }];
        let input = BeatSelectionInput {
            chapter_number: 5,
            contract: None,
            cast: &cast,
            tensions: &[],
            debts: &debts,
            states: &[],
            candidates: &[],
            location: None,
            peak_used: 0,
        };
        let plan = select_next_beat(&input);
        assert!(plan.pressure > 0.3);
        assert_eq!(plan.event_kind, EventKind::Reunion);
    }
}
