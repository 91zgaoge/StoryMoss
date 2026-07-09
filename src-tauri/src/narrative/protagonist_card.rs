//! Genesis 开篇人物卡：合并、渲染、规则探针（零 LLM）。
//!
//! v0.26.45：强制落地姓名 + 欲望/阻力，专治「人空」与「戏空」。

use crate::domain::StoryMetaElement;

/// 从 OpeningSkeleton 抽出的提示（避免本模块依赖 genesis 造成循环引用）。
#[derive(Debug, Clone, Default)]
pub struct SkeletonHints {
    pub name: Option<String>,
    pub goal: Option<String>,
    pub obstacle: Option<String>,
    pub dramatic_goal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtagonistCard {
    pub name: String,
    pub desire: Option<String>,
    pub wound: Option<String>,
    pub obstacle: Option<String>,
    pub scene_goal: Option<String>,
    /// `"skeleton"` | `"concept"` | `"mixed"`
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtagonistProbeResult {
    pub name_hit: bool,
    pub generic_label_hit: bool,
    /// 无 desire/scene_goal 时为 true（不计入失败）
    pub desire_hit: bool,
    /// 无 obstacle 时为 true（不计入失败）
    pub obstacle_hit: bool,
}

fn is_generic_name(s: &str) -> bool {
    let t = s.trim();
    t.is_empty() || matches!(t, "主角" | "男主" | "女主")
}

fn nonempty_opt(s: Option<&str>) -> Option<String> {
    s.map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
}

fn first_nonempty(candidates: &[Option<&str>]) -> Option<String> {
    for c in candidates {
        if let Some(v) = nonempty_opt(*c) {
            return Some(v);
        }
    }
    None
}

/// 合并骨架 ∪ 概念 → 人物卡；无有效姓名则返回 None。
pub fn merge_protagonist_card(
    meta: &StoryMetaElement,
    skeleton: Option<&SkeletonHints>,
) -> Option<ProtagonistCard> {
    let sk_name = skeleton.and_then(|s| s.name.as_deref());
    let meta_name = meta.protagonist_name.as_deref();

    let name = [sk_name, meta_name]
        .into_iter()
        .flatten()
        .find(|n| !is_generic_name(n))
        .map(|n| n.trim().to_string())?;

    let sk_goal = skeleton.and_then(|s| s.goal.as_deref());
    let sk_obstacle = skeleton.and_then(|s| s.obstacle.as_deref());
    let sk_dramatic = skeleton.and_then(|s| s.dramatic_goal.as_deref());

    let desire = first_nonempty(&[
        sk_goal,
        meta.protagonist_desire.as_deref(),
        meta.survival_stakes.as_deref(),
    ]);
    let obstacle = first_nonempty(&[
        sk_obstacle,
        meta.survival_stakes.as_deref(),
        meta.core_conflict.as_deref(),
    ]);
    let scene_goal = first_nonempty(&[sk_dramatic, meta.core_conflict.as_deref()]);
    let wound = nonempty_opt(meta.protagonist_wound.as_deref());

    let from_sk = skeleton.is_some()
        && (sk_name.map(|n| !is_generic_name(n)).unwrap_or(false)
            || sk_goal.map(|g| !g.trim().is_empty()).unwrap_or(false)
            || sk_obstacle.map(|o| !o.trim().is_empty()).unwrap_or(false));
    let from_meta = meta
        .protagonist_name
        .as_deref()
        .map(|n| !is_generic_name(n))
        == Some(true)
        || meta
            .protagonist_desire
            .as_ref()
            .is_some_and(|d| !d.trim().is_empty())
        || meta
            .core_conflict
            .as_ref()
            .is_some_and(|c| !c.trim().is_empty());

    let source = match (from_sk, from_meta) {
        (true, true) => "mixed",
        (true, false) => "skeleton",
        _ => "concept",
    };

    Some(ProtagonistCard {
        name,
        desire,
        wound,
        obstacle,
        scene_goal,
        source,
    })
}

/// 渲染 Critical 人物卡短段（约 100–160 字）。
pub fn render_protagonist_card(card: &ProtagonistCard) -> String {
    let desire_line = card
        .desire
        .as_ref()
        .or(card.scene_goal.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("（未指定——请从用户前提推断一个具体目标）");
    let obstacle_line = card
        .obstacle
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("（未指定——请设置一个具体阻力）");

    let mut out = String::new();
    out.push_str("【开篇人物卡·必须落地】\n");
    out.push_str(&format!("姓名：{}\n", card.name));
    out.push_str(&format!("本场欲望/目标：{}\n", desire_line));
    out.push_str(&format!("本场阻力：{}\n", obstacle_line));
    if let Some(w) = card.wound.as_ref().filter(|w| !w.trim().is_empty()) {
        out.push_str(&format!("旧伤/软肋：{}\n", w));
    }
    out.push_str(
        "纪律：\n\
1. 开场前 3 段内必须出现姓名，禁止用「主角」指代\n\
2. 开场必须让读者明白：主角此刻要什么，以及什么在拦他（用行动/选择体现，禁止空喊口号）",
    );
    out
}

const STOP_CHARS: &[char] = &[
    '的', '了', '在', '与', '和', '是', '被', '把', '就', '都', '也', '而', '及', '或', '对', '向',
    '从', '到', '为', '以', '一', '个',
];

/// 从短语抽出用于子串匹配的内容词（≥2 字，去停用字）。
fn content_tokens(phrase: &str) -> Vec<String> {
    let cleaned: String = phrase
        .chars()
        .filter(|c| !c.is_whitespace() && !STOP_CHARS.contains(c))
        .collect();
    if cleaned.chars().count() < 2 {
        return Vec::new();
    }
    // 整词优先；再拆 2-gram
    let mut tokens = vec![cleaned.clone()];
    let chars: Vec<char> = cleaned.chars().collect();
    if chars.len() >= 4 {
        for w in chars.windows(2) {
            tokens.push(w.iter().collect());
        }
    }
    tokens
}

fn phrase_signal_hit(content: &str, phrase: Option<&str>) -> bool {
    let Some(p) = phrase.map(str::trim).filter(|s| !s.is_empty()) else {
        return true; // 无字段 → 不计入失败
    };
    let tokens = content_tokens(p);
    if tokens.is_empty() {
        return true; // 过短 → 跳过
    }
    // 整句子串或任一内容词命中
    if content.contains(p) {
        return true;
    }
    tokens
        .iter()
        .any(|t| t.chars().count() >= 2 && content.contains(t.as_str()))
}

pub fn probe_protagonist_card(content: &str, card: &ProtagonistCard) -> ProtagonistProbeResult {
    let name_hit = content.contains(card.name.trim());
    let generic_label_hit =
        content.contains("主角") || content.contains("男主") || content.contains("女主");
    let desire_phrase = card.desire.as_deref().or(card.scene_goal.as_deref());
    let desire_hit = phrase_signal_hit(content, desire_phrase);
    let obstacle_hit = phrase_signal_hit(content, card.obstacle.as_deref());
    ProtagonistProbeResult {
        name_hit,
        generic_label_hit,
        desire_hit,
        obstacle_hit,
    }
}

/// 软重试：真名未中，或欲/阻字段齐全却双 miss。
pub fn should_soft_retry_protagonist_card(
    probe: &ProtagonistProbeResult,
    card: &ProtagonistCard,
) -> bool {
    if !probe.name_hit {
        return true;
    }
    let has_desire = card.desire.as_ref().is_some_and(|d| !d.trim().is_empty())
        || card
            .scene_goal
            .as_ref()
            .is_some_and(|g| !g.trim().is_empty());
    let has_obstacle = card.obstacle.as_ref().is_some_and(|o| !o.trim().is_empty());
    has_desire && has_obstacle && !probe.desire_hit && !probe.obstacle_hit
}

/// 软重试时追加的短指令。
pub fn anti_empty_retry_directive(card: &ProtagonistCard) -> String {
    let desire = card
        .desire
        .as_ref()
        .or(card.scene_goal.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("具体目标");
    let obstacle = card
        .obstacle
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("具体阻力");
    format!(
        "\n\n【绝对禁止 — 上一版人物/冲突空洞，本次必须严格遵守】\n\
- 必须使用姓名「{}」，禁止用「主角」「男主」「女主」指代\n\
- 开场用行动体现要「{}」、被「{}」所阻，让读者立刻知道目标与阻力\n\
- 禁止空喊口号或只写氛围不写选择",
        card.name, desire, obstacle
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ElementSource, StoryMetaElement};

    fn meta_with(
        name: Option<&str>,
        desire: Option<&str>,
        wound: Option<&str>,
        conflict: Option<&str>,
        stakes: Option<&str>,
    ) -> StoryMetaElement {
        StoryMetaElement {
            id: String::new(),
            title: "荒星".into(),
            description: "末世求生".into(),
            genre: "末世生存".into(),
            genre_profile_ids: vec![],
            tone: "暗黑".into(),
            pacing: "快节奏".into(),
            themes: vec![],
            target_length: "长篇".into(),
            author: None,
            protagonist_name: name.map(|s| s.to_string()),
            protagonist_desire: desire.map(|s| s.to_string()),
            protagonist_wound: wound.map(|s| s.to_string()),
            core_conflict: conflict.map(|s| s.to_string()),
            world_one_liner: None,
            survival_stakes: stakes.map(|s| s.to_string()),
            source: ElementSource::Generated,
            source_ref_id: None,
        }
    }

    #[test]
    fn merge_prefers_skeleton_name_over_concept() {
        let meta = meta_with(Some("张三"), None, None, None, None);
        let sk = SkeletonHints {
            name: Some("林深".into()),
            ..Default::default()
        };
        let card = merge_protagonist_card(&meta, Some(&sk)).unwrap();
        assert_eq!(card.name, "林深");
    }

    #[test]
    fn merge_filters_generic_protagonist_label() {
        let meta = meta_with(Some("林深"), None, None, None, None);
        let sk = SkeletonHints {
            name: Some("主角".into()),
            ..Default::default()
        };
        let card = merge_protagonist_card(&meta, Some(&sk)).unwrap();
        assert_eq!(card.name, "林深");

        let meta_generic = meta_with(Some("男主"), None, None, None, None);
        let sk_generic = SkeletonHints {
            name: Some("主角".into()),
            ..Default::default()
        };
        assert!(merge_protagonist_card(&meta_generic, Some(&sk_generic)).is_none());
    }

    #[test]
    fn merge_fills_desire_obstacle_scene_goal_from_skeleton() {
        let meta = meta_with(Some("林深"), None, None, None, None);
        let sk = SkeletonHints {
            name: Some("林深".into()),
            goal: Some("找到净水".into()),
            obstacle: Some("辐射尘暴".into()),
            dramatic_goal: Some("在废墟中找到净水".into()),
        };
        let card = merge_protagonist_card(&meta, Some(&sk)).unwrap();
        assert_eq!(card.desire.as_deref(), Some("找到净水"));
        assert_eq!(card.obstacle.as_deref(), Some("辐射尘暴"));
        assert_eq!(card.scene_goal.as_deref(), Some("在废墟中找到净水"));
    }

    #[test]
    fn render_omits_empty_optional_lines() {
        let card = ProtagonistCard {
            name: "林深".into(),
            desire: Some("找到净水".into()),
            wound: None,
            obstacle: Some("辐射尘暴".into()),
            scene_goal: None,
            source: "skeleton",
        };
        let text = render_protagonist_card(&card);
        assert!(!text.contains("旧伤"));
        assert!(text.contains("本场欲望"));
        assert!(text.contains("本场阻力"));
        assert!(text.contains("林深"));
        assert!(text.contains("禁止用「主角」"));
        assert!(text.contains("要什么"));
    }

    #[test]
    fn probe_detects_name_desire_obstacle_signals() {
        let card = ProtagonistCard {
            name: "林深".into(),
            desire: Some("找到净水".into()),
            wound: None,
            obstacle: Some("辐射尘暴".into()),
            scene_goal: None,
            source: "skeleton",
        };
        let good = "林深推开铁门，必须找到净水，外面辐射尘暴正逼近。";
        let probe = probe_protagonist_card(good, &card);
        assert!(probe.name_hit);
        assert!(probe.desire_hit);
        assert!(probe.obstacle_hit);
        assert!(!probe.generic_label_hit);

        let bad = "主角站在废墟里发呆，什么也没做。";
        let probe2 = probe_protagonist_card(bad, &card);
        assert!(!probe2.name_hit);
        assert!(probe2.generic_label_hit);
    }

    #[test]
    fn soft_retry_trigger_when_goal_and_obstacle_both_miss() {
        let card = ProtagonistCard {
            name: "林深".into(),
            desire: Some("找到净水".into()),
            wound: None,
            obstacle: Some("辐射尘暴".into()),
            scene_goal: None,
            source: "skeleton",
        };
        let name_only = ProtagonistProbeResult {
            name_hit: true,
            generic_label_hit: false,
            desire_hit: false,
            obstacle_hit: false,
        };
        assert!(should_soft_retry_protagonist_card(&name_only, &card));

        let desire_ok = ProtagonistProbeResult {
            name_hit: true,
            generic_label_hit: false,
            desire_hit: true,
            obstacle_hit: false,
        };
        assert!(!should_soft_retry_protagonist_card(&desire_ok, &card));

        let no_name = ProtagonistProbeResult {
            name_hit: false,
            generic_label_hit: true,
            desire_hit: true,
            obstacle_hit: true,
        };
        assert!(should_soft_retry_protagonist_card(&no_name, &card));
    }
}
