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

/// 一章的情感弧：1–3 个 act，每个 act 复用单拍的 `EmotionBeat`。
#[derive(Debug, Clone)]
pub struct ChapterEmotionPlan {
    pub chapter_number: i32,
    pub contract: Option<EmotionContract>,
    pub beats: Vec<EmotionBeat>,
    pub tension: Option<EmotionTension>,
    pub debt: Option<EmotionDebt>,
    pub plot_nodes: Vec<String>,
    pub chapter_from: EmotionVector,
    pub chapter_to: EmotionVector,
}

impl ChapterEmotionPlan {
    pub fn peak_count(&self) -> usize {
        self.beats.iter().filter(|b| b.scale >= 1.15).count()
    }

    pub fn is_empty(&self) -> bool {
        self.beats.is_empty()
    }
}

pub struct ChapterPlanInput<'a> {
    pub chapter_number: i32,
    pub contract: Option<&'a EmotionContract>,
    pub cast: &'a [CastEmotionProfile],
    pub tensions: &'a [EmotionTension],
    pub debts: &'a [EmotionDebt],
    pub states: &'a [CharacterEmotionState],
    pub chapter_outline: &'a str,
    pub next_node: &'a str,
    pub location: Option<&'a str>,
    pub peak_used: u32,
    pub previous_plan: Option<&'a ChapterEmotionPlan>,
}

/// 批量续写的章节级情感弧。无契约时返回 None（旧路径不变）。
pub fn plan_chapter_emotion(input: &ChapterPlanInput<'_>) -> Option<ChapterEmotionPlan> {
    let contract = input.contract?;
    let events = parse_key_events(input.chapter_outline, input.next_node);
    let act_count = match events.len() {
        0 => 1,
        1 => 2,
        _ => 3,
    };
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
    let sensitivities = sensitivities_from(profile.wound(), profile.need(), profile.core());
    let mut from = input
        .previous_plan
        .and_then(|p| p.beats.last())
        .map(|b| b.to.clone())
        .or_else(|| latest_state(input.states, &source_name).map(|s| s.vector.clone()))
        .filter(|v| !v.is_zero())
        .unwrap_or_else(|| initial_vector(&profile));
    let public = is_public_place(input.location);
    let budget = contract.peak_budget();
    let mut peaks_used = input.peak_used;
    let base_pressure = tension.as_ref().map(|t| t.pressure).unwrap_or(0.45);
    let mut beats: Vec<EmotionBeat> = Vec::new();

    for i in 0..act_count {
        let event_text = events.get(i).cloned().unwrap_or_else(|| {
            if i == 0 {
                let n = input.next_node.trim();
                if n.is_empty() {
                    "本拍必须付出一个可见代价".to_string()
                } else {
                    n.to_string()
                }
            } else {
                let last = events.last().cloned().unwrap_or_default();
                if last.is_empty() {
                    "上一事件的后果显现".to_string()
                } else {
                    format!("{last}的后果显现")
                }
            }
        });
        let event_kind = event_kind_from_text(&event_text).unwrap_or_else(|| {
            if i == 0 {
                map_tension_type(
                    tension
                        .as_ref()
                        .map(|t| t.tension_type.as_str())
                        .unwrap_or(""),
                )
            } else if i + 1 == act_count {
                EventKind::Reunion
            } else {
                EventKind::Threat
            }
        });
        let appraisal = appraise_event(event_kind, Some(&target_name), &sensitivities);
        let mut to = emotion_from_appraisal(event_kind, &appraisal, &sensitivities);
        if from.distance(&to) < 0.10 {
            let stronger = if matches!(event_kind, EventKind::SecretReveal | EventKind::Betrayal) {
                EventKind::Threat
            } else {
                EventKind::SecretReveal
            };
            let alt_appraisal = appraise_event(stronger, Some(&target_name), &sensitivities);
            let alt = emotion_from_appraisal(stronger, &alt_appraisal, &sensitivities);
            if from.distance(&alt) > from.distance(&to) {
                to = alt;
            }
        }
        let (tendency, action_reason) = action_tendency(&to, profile.defense());
        let dialogue = dialogue_policy(tendency, Vec::new());
        let action_scene = matches!(
            event_kind,
            EventKind::Threat
                | EventKind::Betrayal
                | EventKind::Humiliation
                | EventKind::Sacrifice
                | EventKind::Reunion
        );
        let carriers = select_carriers(&to, profile.defense(), public, action_scene);
        let scale = if act_count > 1
            && i == act_count / 2
            && peaks_used < budget
            && base_pressure >= 0.45
        {
            peaks_used += 1;
            1.2
        } else if base_pressure >= 0.45 {
            0.8
        } else {
            0.4
        };
        let cost = cost_for(event_kind, &tension, &source_name, &target_name);
        let reader_target = reader_target_for(&to, &from, &source_name, &tension);
        let state_change = format!(
            "{}：{} → {}",
            source_name,
            from.render_compact(3),
            to.render_compact(3)
        );
        beats.push(EmotionBeat {
            from: from.clone(),
            to: to.clone(),
            trigger_event: event_text,
            appraisal,
            action_tendency: tendency,
            action_reason,
            dialogue_policy: Some(dialogue),
            cost,
            state_changes: vec![state_change],
            carriers,
            reader_target,
            scale,
            event_kind,
            source_name: source_name.clone(),
            target_name: target_name.clone(),
            tension_id: tension.as_ref().map(|t| t.id.clone()),
            debt_id: debt.as_ref().map(|d| d.id.clone()),
        });
        from = to;
    }

    if beats.is_empty() {
        return None;
    }
    let chapter_from = beats.first()?.from.clone();
    let chapter_to = beats.last()?.to.clone();
    if chapter_from.distance(&chapter_to) < 0.15 {
        return None;
    }
    Some(ChapterEmotionPlan {
        chapter_number: input.chapter_number,
        contract: Some(contract.clone()),
        beats,
        tension,
        debt,
        plot_nodes: events,
        chapter_from,
        chapter_to,
    })
}

fn parse_key_events(outline: &str, next_node: &str) -> Vec<String> {
    let mut events: Vec<String> = Vec::new();
    let mut in_key = false;
    for line in outline.lines() {
        let t = line.trim();
        if t.contains("关键事件") {
            in_key = true;
            continue;
        }
        if !in_key {
            continue;
        }
        if t.is_empty() {
            continue;
        }
        if t.starts_with("**") || t.starts_with("###") {
            break;
        }
        let cleaned = t
            .trim_start_matches(|c: char| {
                c.is_ascii_digit() || matches!(c, '.' | '、' | '-' | '*' | '（' | '(' | ')' | '）')
            })
            .trim();
        if cleaned.chars().count() >= 4 {
            events.push(cleaned.chars().take(120).collect());
        }
    }
    if events.is_empty() {
        for line in outline.lines() {
            let t = line
                .trim()
                .trim_start_matches(|c: char| {
                    c.is_ascii_digit() || matches!(c, '.' | '、' | '-' | '*' | '（' | '(')
                })
                .trim();
            if t.chars().count() >= 8 {
                events.push(t.chars().take(120).collect());
            }
            if events.len() >= 3 {
                break;
            }
        }
    }
    if events.is_empty() && !next_node.trim().is_empty() {
        events.push(next_node.trim().to_string());
    }
    events.truncate(4);
    events
}

fn event_kind_from_text(text: &str) -> Option<EventKind> {
    let t = text.to_string();
    if t.contains("背叛") || t.contains("出卖") {
        Some(EventKind::Betrayal)
    } else if t.contains("真相")
        || t.contains("揭露")
        || t.contains("承认")
        || t.contains("发现")
        || t.contains("揭穿")
    {
        Some(EventKind::SecretReveal)
    } else if t.contains("失去") || t.contains("死") || t.contains("离开") || t.contains("分离")
    {
        Some(EventKind::Loss)
    } else if t.contains("威胁") || t.contains("追杀") || t.contains("对峙") || t.contains("逼")
    {
        Some(EventKind::Threat)
    } else if t.contains("羞辱") || t.contains("当众") || t.contains("难堪") || t.contains("拒绝")
    {
        Some(EventKind::Humiliation)
    } else if t.contains("重逢") || t.contains("回来") || t.contains("见面") || t.contains("和好")
    {
        Some(EventKind::Reunion)
    } else if t.contains("帮助") || t.contains("救") || t.contains("护") || t.contains("替") {
        Some(EventKind::Help)
    } else if t.contains("牺牲") || t.contains("挡") {
        Some(EventKind::Sacrifice)
    } else if t.contains("成功") || t.contains("赢") || t.contains("达成") {
        Some(EventKind::Achievement)
    } else {
        None
    }
}

/// 把整章文本按幕切分，用于批量探针；优先按段落比例，退化时按字符边界。
pub fn split_chapter_into_acts(text: &str, n: usize) -> Vec<String> {
    let n = n.max(1);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if n == 1 || chars.len() < 600 {
        return vec![trimmed.to_string()];
    }
    let ratios: Vec<f32> = match n {
        2 => vec![0.55],
        _ => vec![0.30, 0.65],
    };
    let mut boundaries: Vec<usize> = Vec::new();
    for ratio in ratios {
        let mut idx = ((chars.len() as f32) * ratio) as usize;
        while idx > 0
            && idx < chars.len()
            && !chars[idx].is_whitespace()
            && !matches!(chars[idx - 1], '。' | '！' | '？' | '；' | '\n')
        {
            idx += 1;
        }
        if idx > 0 && idx < chars.len() && !boundaries.contains(&idx) {
            boundaries.push(idx);
        }
        if boundaries.len() + 1 >= n {
            break;
        }
    }
    if boundaries.is_empty() {
        return vec![trimmed.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut prev = 0usize;
    for idx in boundaries {
        let seg: String = chars[prev..idx].iter().collect();
        if !seg.trim().is_empty() {
            out.push(seg.trim().to_string());
        }
        prev = idx;
    }
    let tail: String = chars[prev..].iter().collect();
    if !tail.trim().is_empty() {
        out.push(tail.trim().to_string());
    }
    if out.is_empty() {
        vec![trimmed.to_string()]
    } else {
        out
    }
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

    fn chapter_contract() -> EmotionContract {
        EmotionContract {
            story_id: "s1".into(),
            core_question: "能否被爱而不失去自我".into(),
            reader_promise: "看两个人如何学会靠近".into(),
            peak_budget: 2,
            ..Default::default()
        }
    }

    #[test]
    fn chapter_plan_requires_contract() {
        let cast = vec![profile("林雪"), profile("阿岩")];
        let input = ChapterPlanInput {
            chapter_number: 1,
            contract: None,
            cast: &cast,
            tensions: &[tension()],
            debts: &[],
            states: &[],
            chapter_outline: "关键事件：阿岩当众替林雪承认那封信",
            next_node: "夜宴破裂",
            location: None,
            peak_used: 0,
            previous_plan: None,
        };
        assert!(plan_chapter_emotion(&input).is_none());
    }

    #[test]
    fn chapter_plan_one_event_creates_two_acts() {
        let cast = vec![profile("林雪"), profile("阿岩")];
        let contract = chapter_contract();
        let input = ChapterPlanInput {
            chapter_number: 5,
            contract: Some(&contract),
            cast: &cast,
            tensions: &[tension()],
            debts: &[],
            states: &[],
            chapter_outline: "关键事件：阿岩当众替林雪承认那封信",
            next_node: "夜宴破裂",
            location: Some("夜宴厅"),
            peak_used: 0,
            previous_plan: None,
        };
        let plan = plan_chapter_emotion(&input).expect("plan");
        assert_eq!(plan.beats.len(), 2);
        assert!(plan.chapter_from.distance(&plan.chapter_to) >= 0.15);
        assert!(plan.peak_count() <= 2);
    }

    #[test]
    fn chapter_plan_two_events_creates_three_acts() {
        let cast = vec![profile("林雪"), profile("阿岩")];
        let contract = chapter_contract();
        let input = ChapterPlanInput {
            chapter_number: 5,
            contract: Some(&contract),
            cast: &cast,
            tensions: &[tension()],
            debts: &[],
            states: &[],
            chapter_outline:
                "关键事件：\n1. 阿岩当众替林雪承认那封信\n2. 林雪拒绝并揭穿他的隐瞒\n3. 两人决裂",
            next_node: "决裂",
            location: Some("夜宴厅"),
            peak_used: 0,
            previous_plan: None,
        };
        let plan = plan_chapter_emotion(&input).expect("plan");
        assert_eq!(plan.beats.len(), 3);
        assert!(plan.peak_count() <= 2);
    }

    #[test]
    fn chapter_plan_uses_previous_plan_as_from() {
        let cast = vec![profile("林雪"), profile("阿岩")];
        let contract = chapter_contract();
        let first = plan_chapter_emotion(&ChapterPlanInput {
            chapter_number: 5,
            contract: Some(&contract),
            cast: &cast,
            tensions: &[tension()],
            debts: &[],
            states: &[],
            chapter_outline: "关键事件：阿岩当众替林雪承认那封信",
            next_node: "夜宴破裂",
            location: None,
            peak_used: 0,
            previous_plan: None,
        })
        .expect("first plan");
        let second = plan_chapter_emotion(&ChapterPlanInput {
            chapter_number: 6,
            contract: Some(&contract),
            cast: &cast,
            tensions: &[tension()],
            debts: &[],
            states: &[],
            chapter_outline: "关键事件：林雪离开夜宴",
            next_node: "雨夜追人",
            location: None,
            peak_used: 0,
            previous_plan: Some(&first),
        })
        .expect("second plan");
        assert_eq!(second.chapter_from, first.chapter_to);
    }

    #[test]
    fn split_short_text_stays_one_act() {
        let text = "他推开门。她没有回头。".to_string();
        let acts = split_chapter_into_acts(&text, 3);
        assert_eq!(acts.len(), 1);
    }

    #[test]
    fn split_long_text_into_three_acts() {
        let text = format!(
            "{}\n\n{}\n\n{}",
            "第一幕。".repeat(80),
            "第二幕。".repeat(80),
            "第三幕。".repeat(80)
        );
        let acts = split_chapter_into_acts(&text, 3);
        assert_eq!(acts.len(), 3);
        assert!(acts.iter().all(|a| !a.trim().is_empty()));
    }
}
