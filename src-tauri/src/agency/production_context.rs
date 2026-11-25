//! v0.99.23：续写训练能力的生产复用层。
//!
//! 训练沙盒此前独享四条能力：原著主线投影、原文语体样本/量化基线、
//! 前情提要（StoryBrief）与直连编审 gate。普通创世/续写只拿到部分能力，
//! 所以训练出来的质量改进无法进入真实创作路径。
//!
//! 本模块把训练能力映射到生产数据，不依赖真实后续章（truth chapters）：
//!
//! - **主线投影（StoryPlan）**：从故事大纲 `【下一节点】`、CHAPTER 合同
//!   `must_cover_nodes/goal`、卷纲 `goal/climax`、Blueprint 逐章目的派生逐章
//!   硬任务；与训练 `MainlineProjection` 同一数据结构，只是来源/文案不同。
//! - **语体样本/基线**：从前文 scene 正文抽取；无前文时退到 StyleDNA 的
//!   六维指标（句长/对白/比喻/视角）作量化基线，或 corpus 固定语料。
//! - **StoryBrief**：从最近 10 章前文确定性构建逐章事实/人物态/设定白名单；
//!   已有训练 brief 时直接复用。
//!
//! 所有函数都是确定性 DB 读取/统计，不调用 LLM；生成路径只在同一 run 内
//! 缓存一次，避免每章重复扫描前文。

use crate::{
    agency::{
        mainline::{self, MainlineBeat, MainlineProjection, MainlineSource},
        story_brief::{
            self, BriefCastState, BriefChapter, BriefTerm, BriefThread, StoryBrief,
            StoryBriefRepository,
        },
        style_probe::{self, StyleProfile},
        training_assets::deterministic_chapter_summary,
    },
    creative_engine::write_time_bundle::WriteTimeBundle,
    db::{
        models::Scene, DbPool, SceneRepository, StoryContractRepository, StoryOutlineRepository,
        StoryRepository, StyleDnaRepository,
    },
    domain::contracts::ChapterContract,
    story_system::volume_plan::{self, VolumePlan},
};

/// 确定性 brief 覆盖的前文章数（与训练 10 章上下文一致）。
pub const PRODUCTION_BRIEF_CHAPTERS: usize = 10;
/// 语体样本覆盖的前文章数。
pub const PRODUCTION_STYLE_CHAPTERS: usize = 10;

/// 生产路径的静态能力：前文语体样本/基线 + 确定性前情提要。
#[derive(Debug, Clone, Default)]
pub struct ProductionStatic {
    pub style_exemplar: Option<String>,
    pub style_baseline: Option<StyleProfile>,
    pub story_brief: Option<StoryBrief>,
    pub style_source: String,
    pub brief_source: String,
    /// v0.99.24：从训练报告提升的通用规则（active，按累计次数排序）。
    pub promoted_rule_block: Option<String>,
    pub promoted_rule_count: usize,
}

/// 生产主线投影的纯输入（便于单测，不读 DB）。
#[derive(Debug, Clone, Default)]
pub struct StoryPlanInputs {
    pub outline: String,
    pub contracts: Vec<ChapterContract>,
    pub blueprints: Vec<crate::db::Blueprint>,
    pub volumes: Vec<VolumePlan>,
    pub characters: Vec<String>,
    pub locations: Vec<String>,
    /// 第一人称主角权威姓名（不进入主线硬核对/关键要素，避免第一人称
    /// 正文只有「我」时被判主线缺失）。
    pub protagonist: String,
    /// 前文最后一段正文（用于 key_turn/voice_cues 语感样本）。
    pub voice_source: Option<(i32, String)>,
    pub core_conflict: String,
    pub turning_points: String,
    pub next_node: String,
}

/// 从故事计划构建逐章主线投影（StoryPlan）。
pub fn from_story_plan(
    inputs: &StoryPlanInputs,
    start_chapter: i32,
    count: usize,
) -> MainlineProjection {
    let count = count.max(1);
    let mut beats: Vec<MainlineBeat> = Vec::new();
    for offset in 0..count {
        let chapter = start_chapter + offset as i32;
        if let Some(beat) = beat_from_inputs(inputs, chapter, start_chapter) {
            beats.push(beat);
        }
    }
    let mut arc_parts: Vec<String> = Vec::new();
    if !inputs.core_conflict.trim().is_empty() {
        arc_parts.push(format!("核心冲突：{}", inputs.core_conflict.trim()));
    }
    if !inputs.turning_points.trim().is_empty() {
        arc_parts.push(format!("已定转折：{}", inputs.turning_points.trim()));
    }
    for plan in &inputs.volumes {
        if !plan.goal.trim().is_empty() {
            arc_parts.push(format!(
                "第{}卷目标：{}（卷末：{}）",
                plan.volume_number,
                plan.goal.trim(),
                plan.climax.trim()
            ));
        }
    }
    let arc: String = arc_parts.join("；").chars().take(400).collect();
    MainlineProjection {
        arc,
        beats,
        source: MainlineSource::StoryPlan,
    }
}

fn beat_from_inputs(
    inputs: &StoryPlanInputs,
    chapter: i32,
    start_chapter: i32,
) -> Option<MainlineBeat> {
    if let Some(bp) = inputs
        .blueprints
        .iter()
        .find(|bp| bp.chapter_number == chapter)
    {
        let goal = first_nonempty([
            bp.purpose.as_deref(),
            bp.key_events.as_deref(),
            bp.suspense_hook.as_deref(),
        ])?;
        let names = names_in_text(
            &[
                goal.as_str(),
                bp.title.as_deref().unwrap_or(""),
                bp.notes.as_deref().unwrap_or(""),
            ]
            .join("\n"),
            &inputs.characters,
            &inputs.protagonist,
        );
        let location = find_location(&goal, &inputs.locations);
        let key_terms = join_terms(&names, &location, &[]);
        let must_terms = names.iter().take(2).cloned().collect::<Vec<_>>();
        return Some(MainlineBeat {
            chapter_number: chapter,
            title: bp.title.clone().unwrap_or_default(),
            goal: goal.chars().take(200).collect(),
            key_turn: bp
                .suspense_hook
                .clone()
                .or(bp.key_events.clone())
                .unwrap_or_default()
                .chars()
                .take(80)
                .collect(),
            voice_cues: voice_cues_for(inputs, &names),
            cast: names.iter().take(6).cloned().collect(),
            location,
            key_terms,
            must_terms,
        });
    }
    if let Some(cc) = inputs
        .contracts
        .iter()
        .find(|cc| cc.chapter_number == chapter)
    {
        let goal = cc.chapter_directive.goal.trim();
        if goal.is_empty() {
            return None;
        }
        let names = names_in_text(goal, &inputs.characters, &inputs.protagonist);
        let location = find_location(goal, &inputs.locations);
        let mut hard: Vec<String> = cc
            .chapter_directive
            .must_cover_nodes
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| s.chars().count() >= 2 && s.trim() != inputs.protagonist.trim())
            .collect();
        hard.truncate(4);
        if !location.is_empty() && !hard.iter().any(|t| t == &location) {
            hard.push(location.clone());
        }
        hard.truncate(4);
        let key_terms = join_terms(&names, &location, &hard);
        return Some(MainlineBeat {
            chapter_number: chapter,
            title: cc
                .chapter_directive
                .must_cover_nodes
                .first()
                .cloned()
                .unwrap_or_default(),
            goal: goal.chars().take(200).collect(),
            key_turn: cc
                .chapter_directive
                .must_cover_nodes
                .join("；")
                .chars()
                .take(80)
                .collect(),
            voice_cues: voice_cues_for(inputs, &names),
            cast: names.iter().take(6).cloned().collect(),
            location,
            key_terms,
            must_terms: hard,
        });
    }
    // 第一章（当前续写起点）优先用故事大纲的下一节点。
    if chapter == start_chapter && !inputs.next_node.trim().is_empty() {
        let names = names_in_text(&inputs.next_node, &inputs.characters, &inputs.protagonist);
        let location = find_location(&inputs.next_node, &inputs.locations);
        let key_terms = join_terms(&names, &location, &[]);
        // 大纲下一节点只有「全书级方向」，人物名册来自角色表，不构成该章
        // 硬性要求；gate 不据此判缺失（避免第一人称主角姓名误判）。
        let must_terms: Vec<String> = Vec::new();
        return Some(MainlineBeat {
            chapter_number: chapter,
            title: String::new(),
            goal: inputs.next_node.trim().chars().take(200).collect(),
            // 故事大纲的转折点是全书级，不是本章级；下一节点 beat 不把
            // 卷末/后期转折误当成第一章必须兑现的机制。
            key_turn: String::new(),
            voice_cues: voice_cues_for(inputs, &names),
            cast: names.iter().take(6).cloned().collect(),
            location,
            key_terms,
            must_terms,
        });
    }
    if let Some(plan) = inputs.volumes.iter().find(|p| p.covers(chapter)) {
        let goal = plan.goal.trim();
        if goal.is_empty() && plan.climax.trim().is_empty() {
            return None;
        }
        let names = names_in_text(
            &[goal.to_string(), plan.climax.clone()].join("\n"),
            &inputs.characters,
            &inputs.protagonist,
        );
        let location = find_location(
            &[goal.to_string(), plan.climax.clone()].join("\n"),
            &inputs.locations,
        );
        let mut goal_text = String::new();
        if !goal.is_empty() {
            goal_text.push_str(&format!("按本卷目标推进：{goal}"));
        }
        if !plan.climax.trim().is_empty() {
            if !goal_text.is_empty() {
                goal_text.push_str("；");
            }
            goal_text.push_str(&format!("本卷末必须发生：{}", plan.climax.trim()));
        }
        let key_terms = join_terms(&names, &location, &[]);
        return Some(MainlineBeat {
            chapter_number: chapter,
            title: String::new(),
            goal: goal_text.chars().take(200).collect(),
            // 卷末高潮是整卷级目标，不是当前章必须兑现的「本章转折」；
            // 若写成 key_turn，会让每卷前几章都被卷末高潮钉住。
            key_turn: String::new(),
            voice_cues: voice_cues_for(inputs, &names),
            cast: names.iter().take(6).cloned().collect(),
            location,
            key_terms,
            // 卷纲是卷级目标，人物名册同样不是该章的硬性要求。
            must_terms: Vec::new(),
        });
    }
    None
}

fn first_nonempty<const N: usize>(values: [Option<&str>; N]) -> Option<String> {
    values
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(str::to_string)
}

fn names_in_text(text: &str, characters: &[String], protagonist: &str) -> Vec<String> {
    let protagonist = protagonist.trim();
    let mut names: Vec<String> = characters
        .iter()
        .filter(|name| {
            let name = name.trim();
            name.chars().count() >= 2
                && text.contains(name)
                && (protagonist.is_empty() || name != protagonist)
        })
        .map(|name| name.trim().to_string())
        .collect();
    names.sort_by(|a, b| {
        text.matches(a.as_str())
            .count()
            .cmp(&text.matches(b.as_str()).count())
            .then(a.cmp(b))
    });
    names.dedup();
    names.truncate(6);
    names
}

fn find_location(text: &str, locations: &[String]) -> String {
    locations
        .iter()
        .filter(|location| location.trim().chars().count() >= 2 && text.contains(location.trim()))
        .max_by_key(|location| text.matches(location.trim()).count())
        .map(|location| location.trim().to_string())
        .unwrap_or_default()
}

fn join_terms(names: &[String], location: &str, extra: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for term in names
        .iter()
        .chain(std::iter::once(&location.to_string()))
        .chain(extra.iter())
    {
        let term = term.trim();
        if term.chars().count() < 2 {
            continue;
        }
        if !out.iter().any(|existing| existing == term) {
            out.push(term.to_string());
        }
        if out.len() >= 6 {
            break;
        }
    }
    out
}

fn voice_cues_for(inputs: &StoryPlanInputs, names: &[String]) -> Vec<String> {
    let Some((_, prose)) = inputs.voice_source.as_ref() else {
        return Vec::new();
    };
    let plain = crate::export::content::content_to_plain_text(prose);
    mainline::extract_voice_cues(&plain, names)
}

/// 从 DB 构建生产主线投影。返回 None 表示故事还没有可用的计划层。
pub fn build_mainline(
    pool: &DbPool,
    story_id: &str,
    start_chapter: i32,
    count: usize,
) -> Option<MainlineProjection> {
    build_mainline_inner(pool, story_id, start_chapter, count, None)
}

/// 带「下一拍/章纲」提示的版本：writer/章纲路径拿到的 `next_outline_node`
/// 比故事大纲摘要里的通用「下一节点」更具体，优先用它作为首章主线。
pub fn build_mainline_with_hint(
    pool: &DbPool,
    story_id: &str,
    start_chapter: i32,
    count: usize,
    hint: Option<&str>,
) -> Option<MainlineProjection> {
    build_mainline_inner(pool, story_id, start_chapter, count, hint)
}

fn build_mainline_inner(
    pool: &DbPool,
    story_id: &str,
    start_chapter: i32,
    count: usize,
    hint: Option<&str>,
) -> Option<MainlineProjection> {
    let outline = StoryOutlineRepository::new(pool.clone())
        .get_by_story(story_id)
        .ok()
        .flatten()
        .map(|o| o.content)
        .unwrap_or_default();
    let sections = parse_outline_sections(&outline);
    let mut core_conflict = section_text(&sections, "核心冲突");
    if core_conflict.is_empty() {
        core_conflict =
            prefixed_line(&outline, &["核心冲突：", "核心冲突:", "核心冲突"]).unwrap_or_default();
    }
    let mut turning_points = section_text(&sections, "转折点");
    if turning_points.is_empty() {
        turning_points =
            prefixed_line(&outline, &["关键转折点：", "转折点：", "转折点"]).unwrap_or_default();
    }
    let mut next_node = section_text(&sections, "下一节点");
    if next_node.is_empty() {
        next_node = prefixed_line(
            &outline,
            &[
                "下一节点：",
                "下一节点:",
                "下一节点",
                "整体推进方向：",
                "整体推进方向:",
                "整体推进方向",
                "推进方向：",
            ],
        )
        .unwrap_or_default();
    }
    if let Some(hint) = hint.map(str::trim).filter(|h| !h.is_empty()) {
        next_node = hint.to_string();
    }
    if core_conflict.is_empty() && turning_points.is_empty() && next_node.is_empty() {
        // 没有任何故事计划层时不生成投影；避免用空大纲造出假主线。
        return None;
    }
    let contracts: Vec<ChapterContract> = StoryContractRepository::new(pool.clone())
        .get_by_story(story_id)
        .ok()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.contract_type == "CHAPTER")
        .filter_map(|c| serde_json::from_str::<ChapterContract>(&c.contract_json).ok())
        .collect();
    let blueprints = crate::db::BlueprintRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default();
    let volumes = volume_plan::list_volume_plans(pool, story_id).unwrap_or_default();
    let characters = crate::db::CharacterRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.name)
        .collect::<Vec<_>>();
    let scenes = SceneRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default();
    let mut locations: Vec<String> = scenes
        .iter()
        .filter_map(|s| s.setting_location.clone())
        .filter(|s| !s.trim().is_empty())
        .collect();
    locations.sort();
    locations.dedup();
    let voice_source = scenes
        .iter()
        .filter(|s| {
            s.content
                .as_deref()
                .map(|c| c.trim().chars().count() > 80)
                .unwrap_or(false)
        })
        .max_by_key(|s| s.sequence_number)
        .map(|s| (s.sequence_number, s.content.clone().unwrap_or_default()));
    let protagonist = StoryBriefRepository::new(pool.clone())
        .get_by_story(story_id)
        .ok()
        .flatten()
        .map(|b| b.protagonist)
        .filter(|p| !p.is_empty())
        .or_else(|| {
            voice_source.as_ref().and_then(|(_, prose)| {
                let plain = crate::export::content::content_to_plain_text(prose);
                story_brief::detect_protagonist_name(&plain, &characters)
            })
        })
        .unwrap_or_default();
    // 没有显式「下一节点」时，用核心冲突作为首章主线锚点：它比空投影更
    // 能阻止模型另起一套故事，也不会把卷末转折误当成第一章任务。
    let next_node = if next_node.trim().is_empty() {
        core_conflict.clone()
    } else {
        next_node
    };
    let inputs = StoryPlanInputs {
        outline,
        contracts,
        blueprints,
        volumes,
        characters,
        locations,
        protagonist,
        voice_source,
        core_conflict,
        turning_points,
        next_node,
    };
    let projection = from_story_plan(&inputs, start_chapter, count);
    if projection.is_empty() {
        return None;
    }
    Some(projection)
}

fn parse_outline_sections(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut current: Option<String> = None;
    let mut buf = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // 章节标记行：`【核心冲突】` / `【关键转折点】` / `【下一节点】`，
        // 也可能在同一行后直接跟正文。
        if line.starts_with('【') {
            if let Some(end) = line.find('】') {
                let key = line[..end + 3].to_string();
                let rest = line[end + 3..].trim();
                if let Some(prev) = current.take() {
                    if !buf.trim().is_empty() {
                        out.push((prev, buf.trim().to_string()));
                    }
                }
                current = Some(key);
                buf.clear();
                if !rest.is_empty() {
                    buf.push_str(rest);
                    buf.push('\n');
                }
                continue;
            }
        }
        if let Some(_) = current.as_ref() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if let Some(prev) = current {
        if !buf.trim().is_empty() {
            out.push((prev, buf.trim().to_string()));
        }
    }
    out
}

fn section_text(sections: &[(String, String)], needle: &str) -> String {
    sections
        .iter()
        .find(|(key, _)| key.contains(needle))
        .map(|(_, body)| body.trim().to_string())
        .unwrap_or_default()
}

/// 兼容旧格式的「核心冲突：…」「下一节点：…」单行前缀。
fn prefixed_line(text: &str, prefixes: &[&str]) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();
        for prefix in prefixes {
            if let Some(rest) = line.strip_prefix(prefix) {
                let rest = rest.trim().trim_start_matches(['：', ':']);
                if !rest.is_empty() {
                    return Some(rest.to_string());
                }
            }
            // 一行里可能串写多个字段（旧大纲常见「核心冲突：…转折点：…
            // 推进方向：…」），从任意位置取前缀后的正文。
            if let Some(pos) = line.find(prefix) {
                let rest = line[pos + prefix.len()..]
                    .trim()
                    .trim_start_matches(['：', ':']);
                if !rest.is_empty() {
                    return Some(rest.to_string());
                }
            }
            if line == *prefix {
                // `核心冲突：` 单独一行时取下一行。
                if let Some(next_line) = text
                    .lines()
                    .skip_while(|l| l.trim() != line)
                    .nth(1)
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                {
                    return Some(next_line.to_string());
                }
            }
        }
    }
    None
}

/// 从前文 scene 构建生产静态能力（语体 + 前情提要）。
pub fn build_static(pool: &DbPool, story_id: &str) -> ProductionStatic {
    let scenes = SceneRepository::new(pool.clone())
        .get_by_story(story_id)
        .unwrap_or_default();
    let latest = scenes
        .iter()
        .max_by_key(|s| s.sequence_number)
        .map(|s| s.sequence_number)
        .unwrap_or(0);
    let style = build_style(pool, story_id, &scenes);
    let story_brief = build_brief(pool, story_id, latest, &scenes);
    let brief_source = if story_brief.is_some() {
        "story_briefs_or_scenes".to_string()
    } else {
        "none".to_string()
    };
    let (promoted_rule_block, promoted_rule_count) =
        crate::agency::training_rules::load_prompt_block(pool);
    ProductionStatic {
        style_exemplar: style.0,
        style_baseline: style.1,
        story_brief,
        style_source: style.2,
        brief_source,
        promoted_rule_block,
        promoted_rule_count,
    }
}

fn build_style(
    pool: &DbPool,
    story_id: &str,
    scenes: &[Scene],
) -> (Option<String>, Option<StyleProfile>, String) {
    let mut context: Vec<(usize, String)> = scenes
        .iter()
        .filter(|s| {
            s.content
                .as_deref()
                .map(|c| c.trim().chars().count() >= 80)
                .unwrap_or(false)
        })
        .map(|s| {
            (
                s.sequence_number as usize,
                s.content.clone().unwrap_or_default(),
            )
        })
        .collect();
    context.sort_by_key(|(n, _)| *n);
    if context.len() > PRODUCTION_STYLE_CHAPTERS {
        context = context.split_off(context.len() - PRODUCTION_STYLE_CHAPTERS);
    }
    if !context.is_empty() {
        let exemplar = style_probe::build_exemplar(&context, false);
        let joined = context
            .iter()
            .map(|(_, c)| crate::export::content::content_to_plain_text(c))
            .collect::<Vec<_>>()
            .join("\n");
        let baseline = style_probe::profile(&joined);
        if !exemplar.is_empty() && baseline.chars > 0 {
            return (Some(exemplar), Some(baseline), "story_scenes".to_string());
        }
    }
    // 无前文正文：尝试 corpus 固定语料。
    if let Some(block) = crate::corpus::pinned_context_block(pool, story_id) {
        let chars = block.chars().count();
        if chars >= 200 {
            let context = vec![(0usize, block.clone())];
            let exemplar = style_probe::build_exemplar(&context, false);
            let baseline = style_probe::profile(&block);
            if !exemplar.is_empty() && baseline.chars > 0 {
                return (Some(exemplar), Some(baseline), "corpus".to_string());
            }
        }
    }
    // 无前文且无 corpus：用故事 StyleDNA 的六维指标作量化基线。
    let story = StoryRepository::new(pool.clone())
        .get_by_id(story_id)
        .ok()
        .flatten();
    let Some(dna_id) = story.and_then(|s| s.style_dna_id) else {
        return (None, None, "none".to_string());
    };
    let Some(dna_row) = StyleDnaRepository::new(pool.clone())
        .get_by_id(&dna_id)
        .ok()
        .flatten()
    else {
        return (None, None, "style_dna_missing".to_string());
    };
    let Some(dna) = serde_json::from_str::<crate::domain::style::StyleDNA>(&dna_row.dna_json).ok()
    else {
        return (None, None, "style_dna_invalid".to_string());
    };
    let baseline = style_probe::from_style_dna(&dna);
    let extension = dna.to_prompt_extension();
    let block = if let Some(profile) = baseline.as_ref() {
        format!(
            "【StyleDNA 语体基线（作者选定风格；按此校准句长/对白/比喻/视角，禁止改成冷硬侦探腔）】\n{}\n{}",
            style_probe::baseline_line(profile),
            extension
        )
    } else {
        format!(
            "【StyleDNA 语体基线（作者选定风格；按此校准句长/对白/比喻/视角，禁止改成冷硬侦探腔）】\n{}",
            extension
        )
    };
    (Some(block), baseline, "style_dna".to_string())
}

fn build_brief(
    pool: &DbPool,
    story_id: &str,
    latest_chapter: i32,
    scenes: &[Scene],
) -> Option<StoryBrief> {
    let repo = StoryBriefRepository::new(pool.clone());
    let existing = repo
        .get_by_story(story_id)
        .ok()
        .flatten()
        .filter(|b| !b.is_empty());
    if let Some(brief) = existing.clone() {
        if brief.end_chapter >= latest_chapter {
            return Some(brief);
        }
    }
    if scenes.is_empty() {
        return existing;
    }
    let mut ordered = scenes
        .iter()
        .filter(|s| {
            s.content
                .as_deref()
                .map(|c| !c.trim().is_empty())
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    ordered.sort_by_key(|s| s.sequence_number);
    if ordered.len() > PRODUCTION_BRIEF_CHAPTERS {
        ordered = ordered.split_off(ordered.len() - PRODUCTION_BRIEF_CHAPTERS);
    }
    let joined_plain = ordered
        .iter()
        .map(|s| crate::export::content::content_to_plain_text(s.content.as_deref().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n");
    let mut summaries: Vec<BriefChapter> = Vec::new();
    for scene in &ordered {
        let plain = crate::utils::text::TextUtils::strip_leading_chapter_heading(
            &crate::export::content::content_to_plain_text(scene.content.as_deref().unwrap_or("")),
        );
        if plain.trim().is_empty() {
            continue;
        }
        let cast: Vec<String> = scene
            .characters_present
            .iter()
            .map(|n| n.trim().to_string())
            .filter(|n| n.chars().count() >= 2 && !story_brief::is_protagonist_alias(n))
            .collect();
        let location = scene.setting_location.clone().unwrap_or_default();
        let mut key_terms = cast.clone();
        if !location.is_empty() && !key_terms.iter().any(|t| t == &location) {
            key_terms.push(location.clone());
        }
        key_terms.truncate(6);
        summaries.push(BriefChapter {
            chapter: scene.sequence_number,
            summary: deterministic_chapter_summary(scene.sequence_number as usize, &plain),
            location,
            cast: cast.into_iter().take(4).collect(),
            result: plain
                .trim()
                .chars()
                .rev()
                .take(120)
                .collect::<String>()
                .chars()
                .rev()
                .collect(),
            open_thread: String::new(),
            turning_point: turning_point_of(&plain),
            key_terms,
        });
    }
    if summaries.is_empty() {
        return existing;
    }
    let bundle =
        WriteTimeBundle::load_sync(pool, story_id, latest_chapter.max(1), None, None, None).ok();
    let mut threads: Vec<BriefThread> = Vec::new();
    if let Some(bundle) = bundle.as_ref() {
        for (status, items) in [
            ("pending", &bundle.pending_foreshadowings),
            ("overdue", &bundle.overdue_foreshadowings),
        ] {
            for item in items.iter().take(4) {
                let description: String = item.trim().chars().take(120).collect();
                if description.is_empty() {
                    continue;
                }
                threads.push(BriefThread {
                    id: format!("foreshadow-{}", threads.len() + 1),
                    description,
                    status: status.to_string(),
                    next_expected: String::new(),
                });
            }
        }
    }
    let mut cast_states: Vec<BriefCastState> = Vec::new();
    let mut canon_terms: Vec<BriefTerm> = Vec::new();
    if let Some(bundle) = bundle.as_ref() {
        for c in bundle.core_characters.iter().take(8) {
            if c.name.trim().chars().count() < 2 || story_brief::is_protagonist_alias(&c.name) {
                continue;
            }
            cast_states.push(BriefCastState {
                name: c.name.trim().to_string(),
                role: c.identity.clone().unwrap_or_default(),
                location: c.location.clone().unwrap_or_default(),
                goal: String::new(),
                knows: Vec::new(),
            });
            if joined_plain.contains(c.name.trim()) {
                canon_terms.push(BriefTerm {
                    term: c.name.trim().to_string(),
                    kind: "人物".to_string(),
                    note: c.identity.clone().unwrap_or_default(),
                });
            }
        }
    }
    for location in ordered.iter().filter_map(|s| s.setting_location.as_deref()) {
        if location.trim().chars().count() >= 2
            && !canon_terms.iter().any(|t| t.term == location.trim())
        {
            canon_terms.push(BriefTerm {
                term: location.trim().to_string(),
                kind: "地点".to_string(),
                note: String::new(),
            });
        }
        if canon_terms.len() >= 12 {
            break;
        }
    }
    canon_terms.truncate(12);
    let mut brief =
        story_brief::brief_from_summaries(summaries, "", threads, cast_states, canon_terms);
    let roster: Vec<String> = ordered
        .iter()
        .flat_map(|s| s.characters_present.iter().cloned())
        .collect();
    if let Some(name) = story_brief::detect_protagonist_name(&joined_plain, &roster) {
        brief.protagonist = name;
    }
    brief.source = "deterministic_scenes".to_string();
    // 落库让下一次 run 直接复用；失败不影响生成。
    let _ = repo.upsert(story_id, &brief);
    Some(brief)
}

fn turning_point_of(plain: &str) -> String {
    const MARKERS: [&str; 16] = [
        "却", "但", "忽然", "突然", "当场", "反手", "结果", "发现", "逼", "动手", "翻脸", "拦住",
        "抓住", "拒绝", "答应", "警告",
    ];
    for sentence in plain.split(['。', '！', '？', '\n']) {
        let sentence = sentence.trim();
        if sentence.chars().count() < 8 {
            continue;
        }
        if MARKERS.iter().any(|m| sentence.contains(m)) {
            return sentence.chars().take(80).collect();
        }
    }
    String::new()
}

pub fn production_brief_source(brief: Option<&StoryBrief>) -> String {
    brief
        .map(|b| format!("{} 章 / {}", b.chapters.len(), b.source))
        .unwrap_or_else(|| "无前情提要".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::{
            create_test_pool, dto::CreateStoryRequest, SceneRepository, SceneUpdate,
            StoryRepository,
        },
        domain::contracts::{ChapterContract, ChapterDirective},
    };

    fn contract(chapter: i32, goal: &str, nodes: &[&str]) -> ChapterContract {
        ChapterContract {
            schema_version: "story-system/v1".into(),
            contract_type: "CHAPTER".into(),
            generator_version: "test".into(),
            chapter_number: chapter,
            chapter_directive: ChapterDirective {
                goal: goal.into(),
                must_cover_nodes: nodes.iter().map(|s| (*s).to_string()).collect(),
                forbidden_zones: vec![],
                time_anchor: None,
                chapter_span: None,
            },
        }
    }

    fn volume(start: i32, end: i32, goal: &str, climax: &str) -> VolumePlan {
        VolumePlan {
            schema_version: "story-system/v1".into(),
            contract_type: "VOLUME".into(),
            generator_version: "test".into(),
            volume_number: 1,
            start_chapter: start,
            end_chapter: end,
            goal: goal.into(),
            climax: climax.into(),
            foreshadowing_due: vec![],
            new_thread_budget: 2,
            scale_estimate: String::new(),
        }
    }

    fn seed_story(pool: &DbPool) -> String {
        StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "生产复用".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap()
            .id
    }

    fn seed_scene(pool: &DbPool, story_id: &str, n: i32, content: &str, cast: &[&str]) -> String {
        let repo = SceneRepository::new(pool.clone());
        let scene = repo.create(story_id, n, Some(&format!("第{n}章"))).unwrap();
        repo.update(
            &scene.id,
            &SceneUpdate {
                content: Some(content.to_string()),
                characters_present: Some(cast.iter().map(|s| (*s).to_string()).collect()),
                setting_location: Some("福摩萨夜总会".into()),
                ..Default::default()
            },
        )
        .unwrap();
        scene.id
    }

    #[test]
    fn plan_projection_prefers_chapter_contracts_then_volume() {
        let inputs = StoryPlanInputs {
            outline: "【核心冲突】周成与邵卫江争夺福摩萨\n【下一节点】去福摩萨夜总会找邵卫江"
                .into(),
            contracts: vec![contract(
                101,
                "周成去福摩萨夜总会冲撞邵卫江，汪必成被迫站队。",
                &["邵卫江", "福摩萨夜总会"],
            )],
            blueprints: vec![],
            volumes: vec![volume(101, 120, "站稳福摩萨", "夺回戏院")],
            characters: vec!["周成".into(), "邵卫江".into(), "汪必成".into()],
            locations: vec!["福摩萨夜总会".into()],
            protagonist: "周成".into(),
            voice_source: Some((100, "邵卫江道：“法不可亵，术不可显。”".into())),
            core_conflict: "周成与邵卫江争夺福摩萨".into(),
            turning_points: "玉佩易主".into(),
            next_node: "去福摩萨夜总会找邵卫江".into(),
        };
        let p = from_story_plan(&inputs, 101, 2);
        assert_eq!(p.source, MainlineSource::StoryPlan);
        assert_eq!(p.beats.len(), 2);
        assert!(p.beats[0].goal.contains("冲撞邵卫江"));
        assert!(p.beats[0].must_terms.iter().any(|t| t == "邵卫江"));
        assert!(p.beats[0].key_terms.iter().any(|t| t == "福摩萨夜总会"));
        assert!(p.beats[1].goal.contains("按本卷目标推进"));
        assert!(!p.beats[1].goal.contains("去福摩萨夜总会找邵卫江"));
        let block = p.render_beat_block(101).unwrap();
        assert!(block.contains("本章故事主线任务"), "{block}");
        assert!(!block.contains("本章原著主线任务"), "{block}");
        let (gate, missing) = p.render_gate_block(101, "我在医院走廊里等。").unwrap();
        assert!(gate.contains("本章故事主线任务"));
        assert_eq!(missing.len(), 2);
    }

    #[test]
    fn plan_projection_uses_next_node_only_for_first_chapter() {
        let inputs = StoryPlanInputs {
            outline: "【核心冲突】周成与邵卫江争夺福摩萨\n【下一节点】去福摩萨夜总会找邵卫江"
                .into(),
            contracts: vec![],
            blueprints: vec![],
            volumes: vec![volume(101, 120, "站稳福摩萨", "夺回戏院")],
            characters: vec!["周成".into(), "邵卫江".into()],
            locations: vec!["福摩萨夜总会".into()],
            protagonist: "周成".into(),
            voice_source: None,
            core_conflict: "周成与邵卫江争夺福摩萨".into(),
            turning_points: String::new(),
            next_node: "去福摩萨夜总会找邵卫江".into(),
        };
        let p = from_story_plan(&inputs, 101, 2);
        assert_eq!(p.beats[0].goal, "去福摩萨夜总会找邵卫江");
        assert!(p.beats[1].goal.contains("按本卷目标推进"));
    }

    #[test]
    fn production_mainline_reads_next_node_from_db_outline() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        StoryOutlineRepository::new(pool.clone())
            .create(
                &story_id,
                "【核心冲突】周成与邵卫江争夺福摩萨\n【下一节点】去福摩萨夜总会找邵卫江",
                None,
                3,
                None,
            )
            .unwrap();
        let p = build_mainline(&pool, &story_id, 101, 1).expect("应生成生产主线");
        assert_eq!(p.source, MainlineSource::StoryPlan);
        assert_eq!(p.beats[0].chapter_number, 101);
        assert!(p.beats[0].goal.contains("福摩萨"));
    }

    #[test]
    fn production_static_builds_style_and_brief_from_prior_scenes() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let prose = "<p>第101章 夜闯福摩萨</p><p>我蹲在夜总会门口，叼着半根烟：“咋整？甭跟老子扯那些没用的。”</p><p>邵卫江坐在吧台后头，把一只茶盏推到我面前，慢条斯理地说：“法不可亵，术不可显。你要问的，今晚只能问一句。”</p><p>我心里琢磨，这事儿得整明白，不能光靠瞎扯。汪必成在门厅里踱来踱去，鞋底磨得青砖发亮。</p>";
        let prose2 = "<p>第102章 包厢逼问</p><p>我掀开珠帘走进包厢，邵卫江把手按在桌面上：“你昨天问了一句，今天还想问几句？”</p><p>汪必成在吧台边上低着头，连茶都不敢端。风从门厅灌进来，珠帘影子一直晃到墙根。</p>";
        let prose3 = "<p>第103章 夺戏院</p><p>我站在戏院台阶上，邵卫江从门厅里走出来，脸色沉得像夜里的雨。</p><p>他说：“福摩萨的账，今天该清了。”我看着他，心里咯噔一下，但还是把烟头摁灭在台阶上。</p>";
        seed_scene(&pool, &story_id, 101, prose, &["邵卫江", "汪必成"]);
        seed_scene(&pool, &story_id, 102, prose2, &["邵卫江", "汪必成"]);
        seed_scene(&pool, &story_id, 103, prose3, &["邵卫江", "汪必成"]);
        let static_ = build_static(&pool, &story_id);
        assert_eq!(static_.style_source, "story_scenes");
        assert!(static_
            .style_exemplar
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false));
        assert!(static_
            .style_baseline
            .map(|p| p.avg_sentence_len > 0.0)
            .unwrap_or(false));
        let brief = static_.story_brief.expect("应生成前情提要");
        assert_eq!(brief.start_chapter, 101);
        assert_eq!(brief.end_chapter, 103);
        assert_eq!(brief.context_chapters, 3);
        assert!(brief.chapters.iter().any(|c| c.location.contains("福摩萨")));
        assert!(brief
            .chapters
            .iter()
            .any(|c| c.cast.iter().any(|n| n == "邵卫江")));
        assert!(brief.canon_terms.iter().any(|t| t.term == "福摩萨夜总会"));
    }
}
