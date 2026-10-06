//! 按已有正文重写生产资产。纸面只读。
//! 设计：docs/plans/2026-08-21-asset-refresh-from-prose-design.md

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    agency::{
        beat_card::CURRENT_SCENE_OUTLINE_MARK,
        continue_assets::{strip_editor_markup, strip_outline_planning},
        materialize::concat_story_prose,
        observe::merge_current_scene_outline,
        prose_ground::{has_substantial_prose, name_in_prose},
        repository::AgencyRepository,
        tool_loop::LoopLlm,
    },
    db::{
        dto::CreateCharacterRequest,
        repositories::{
            CharacterRepository, SceneRepository, SceneUpdate, StoryOutlineRepository,
            WorldBuildingRepository,
        },
        DbPool,
    },
    error::AppError,
    memory::asset_bridge::{cap_story_outline_content, is_refinable},
    planner::{AssetRefreshDraft, PlanExecutionResult},
    router::TaskType,
};

const PROSE_BUDGET: usize = 12_000;
const DUAL_HEAD: usize = 600;
const DUAL_TAIL: usize = 1800;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetRefreshTarget {
    StoryOutline,
    Characters,
    World,
    SceneOutline,
}

const ALL_TARGETS: [AssetRefreshTarget; 4] = [
    AssetRefreshTarget::StoryOutline,
    AssetRefreshTarget::Characters,
    AssetRefreshTarget::World,
    AssetRefreshTarget::SceneOutline,
];

/// 「按正文重写设定」形状：必须能盖掉续写兜底。
/// 点名正文 +（重写/刷新），或「重新生成」且点名大纲/角色/世界观。
/// 写/生成的宾语是大纲时同样成立（「写后续的故事大纲」），不要求出现「正文」。
/// 「根据正文重新生成下一章」「按照故事大纲写后续」「写后续」不进本作业。
pub fn looks_like_asset_refresh_shape(input: &str) -> bool {
    let from_prose = input.contains("正文") || input.contains("已写章节");
    let classic = input.contains("重新写") || input.contains("重写") || input.contains("刷新");
    let regen = input.contains("重新生成") || input.contains("再生成");
    if from_prose && classic {
        return true;
    }
    if from_prose && regen {
        return names_refreshable_asset(input);
    }
    writes_named_outline_object(input)
}

fn names_refreshable_asset(input: &str) -> bool {
    input.contains("大纲")
        || input.contains("角色")
        || input.contains("人物")
        || input.contains("人设")
        || input.contains("世界观")
        || input.contains("设定")
        || input.contains("资产")
}

/// 动词宾语是大纲：`写后续的故事大纲` / `生成后续的场景大纲`。
/// `按照故事大纲写后续` 的宾语是「后续」，不是大纲。
fn writes_named_outline_object(input: &str) -> bool {
    if input.contains("写一部")
        || input.contains("写一本")
        || input.contains("写一篇")
        || input.contains("创作一部")
        || input.contains("新开一部")
    {
        return false;
    }
    const VERBS: &[&str] = &["重新生成", "再生成", "重新写", "重写", "生成", "刷新", "写"];
    for verb in VERBS {
        let mut rest = input;
        while let Some(i) = rest.find(verb) {
            let after = &rest[i + verb.len()..];
            if outline_is_verb_object(after) {
                return true;
            }
            rest = &rest[i + verb.len()..];
        }
    }
    false
}

fn outline_is_verb_object(after: &str) -> bool {
    let chunk = after
        .split(|c: char| matches!(c, '，' | '。' | '；' | '、' | ',' | ';' | '\n'))
        .next()
        .unwrap_or(after);
    let Some(o) = chunk.find("大纲") else {
        return false;
    };
    if let Some(c) = chunk.find("章") {
        if c < o {
            return false;
        }
    }
    true
}

pub fn allow_overwrite_manual(input: &str) -> bool {
    input.contains("覆盖手改") || input.contains("包括手改")
}

pub fn parse_asset_refresh_targets(input: &str) -> Vec<AssetRefreshTarget> {
    if input.contains("全部设定")
        || input.contains("所有资产")
        || input.contains("全部资产")
        || input.contains("按正文重写设定")
    {
        return ALL_TARGETS.to_vec();
    }
    let mut out = Vec::new();
    let scene = input.contains("场景大纲")
        || input.contains("本章大纲")
        || input.contains("当场大纲")
        || input.contains("当前场大纲");
    if scene {
        out.push(AssetRefreshTarget::SceneOutline);
    }
    if input.contains("故事大纲") || input.contains("整书大纲") || input.contains("书纲")
    {
        out.push(AssetRefreshTarget::StoryOutline);
    } else if input.contains("大纲") && !scene {
        out.push(AssetRefreshTarget::StoryOutline);
    }
    if input.contains("角色") || input.contains("人物卡") || input.contains("人设") {
        out.push(AssetRefreshTarget::Characters);
    }
    if input.contains("世界观") || input.contains("世界设定") {
        out.push(AssetRefreshTarget::World);
    }
    out
}

pub fn needs_outline_confirm(targets: &[AssetRefreshTarget]) -> bool {
    targets.iter().any(|t| {
        matches!(
            t,
            AssetRefreshTarget::StoryOutline | AssetRefreshTarget::SceneOutline
        )
    })
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct AssetRefreshPayload {
    #[serde(default)]
    pub story_outline: Option<String>,
    #[serde(default)]
    pub characters: Option<Vec<RefreshCharacter>>,
    #[serde(default)]
    pub world: Option<RefreshWorld>,
    #[serde(default)]
    pub scene_outline: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct RefreshCharacter {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub personality: Option<String>,
    #[serde(default)]
    pub goals: Option<String>,
    #[serde(default)]
    pub appearance: Option<String>,
    #[serde(default)]
    pub emotional_core: Option<String>,
    #[serde(default)]
    pub emotional_trigger: Option<String>,
    #[serde(default)]
    pub emotional_wound: Option<String>,
    #[serde(default)]
    pub emotional_need: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct RefreshWorld {
    #[serde(default)]
    pub concept: Option<String>,
    #[serde(default)]
    pub history: Option<String>,
}

/// 强模型常把 story_outline 做成对象（v0.30.29）；本地思考模型常丢中文键、
/// 或输出 `story_outline: …` 键值散文（v0.53.1 真机）。
pub fn parse_refresh_payload(raw: &str) -> Option<AssetRefreshPayload> {
    if let Some(p) = parse_json_refresh(raw) {
        return Some(p);
    }
    parse_labeled_refresh(raw)
}

fn parse_json_refresh(raw: &str) -> Option<AssetRefreshPayload> {
    let json = crate::narrative::extract_and_sanitize_json(raw).ok()?;
    let mut value: serde_json::Value = serde_json::from_str(&json).ok()?;
    remap_refresh_keys(&mut value);
    payload_from_value(&value)
}

fn labeled_field(key: &str) -> Option<&'static str> {
    match key.to_ascii_lowercase().as_str() {
        "story_outline" | "故事大纲" | "整书大纲" | "书纲" => Some("story_outline"),
        "scene_outline" | "场景大纲" | "本章大纲" | "当场大纲" | "当前场大纲" => {
            Some("scene_outline")
        }
        "world" | "world_building" | "世界观" | "世界设定" => Some("world"),
        _ => None,
    }
}

/// Gemma 常返回无花括号的 `story_outline:… scene_outline：…`（中英冒号都有）。
fn parse_labeled_refresh(raw: &str) -> Option<AssetRefreshPayload> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)(story_outline|scene_outline|world_building|world|故事大纲|整书大纲|书纲|场景大纲|本章大纲|当场大纲|当前场大纲|世界观|世界设定)\s*[:：]",
        )
        .expect("labeled refresh regex")
    });
    let mut story_outline = None;
    let mut scene_outline = None;
    let mut world = None;
    let caps: Vec<regex::Captures> = re.captures_iter(raw).collect();
    if caps.is_empty() {
        return None;
    }
    for (i, cap) in caps.iter().enumerate() {
        let Some(key_m) = cap.get(1) else {
            continue;
        };
        let Some(field) = labeled_field(key_m.as_str()) else {
            continue;
        };
        let full = cap.get(0).expect("regex match 0");
        let value_start = full.end();
        let value_end = caps
            .get(i + 1)
            .and_then(|n| n.get(0).map(|m| m.start()))
            .unwrap_or(raw.len());
        let text = raw[value_start..value_end].trim();
        if text.is_empty() {
            continue;
        }
        match field {
            "story_outline" => story_outline = Some(text.to_string()),
            "scene_outline" => scene_outline = Some(text.to_string()),
            "world" => {
                world = Some(RefreshWorld {
                    concept: Some(text.to_string()),
                    history: None,
                })
            }
            _ => {}
        }
    }
    let payload = AssetRefreshPayload {
        story_outline,
        characters: None,
        world,
        scene_outline,
    };
    if payload.story_outline.is_none() && payload.world.is_none() && payload.scene_outline.is_none()
    {
        return None;
    }
    Some(payload)
}

fn remap_refresh_keys(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };
    const ALIASES: &[(&str, &str)] = &[
        ("outline", "story_outline"),
        ("故事大纲", "story_outline"),
        ("整书大纲", "story_outline"),
        ("书纲", "story_outline"),
        ("场景大纲", "scene_outline"),
        ("本章大纲", "scene_outline"),
        ("世界观", "world"),
        ("世界设定", "world"),
        ("角色", "characters"),
        ("人物", "characters"),
        ("人物卡", "characters"),
    ];
    for (from, to) in ALIASES {
        if obj.contains_key(*to) {
            continue;
        }
        if let Some(val) = obj.remove(*from) {
            obj.insert((*to).into(), val);
        }
    }
}

fn payload_from_value(value: &serde_json::Value) -> Option<AssetRefreshPayload> {
    let story_outline = value
        .get("story_outline")
        .map(crate::agency::coordinator::normalize_outline)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let looks_like_outline = value.get("core_conflict").is_some()
                || value.get("turning_points").is_some()
                || value.get("three_act_structure").is_some();
            if looks_like_outline {
                let text = crate::agency::coordinator::normalize_outline(value);
                if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                }
            } else {
                None
            }
        });
    let scene_outline = value
        .get("scene_outline")
        .map(crate::agency::coordinator::normalize_outline)
        .filter(|s| !s.trim().is_empty());
    let characters = value
        .get("characters")
        .and_then(|v| serde_json::from_value::<Vec<RefreshCharacter>>(v.clone()).ok());
    let world = value.get("world").and_then(|v| match v {
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(RefreshWorld {
            concept: Some(s.clone()),
            history: None,
        }),
        other => serde_json::from_value::<RefreshWorld>(other.clone()).ok(),
    });
    let payload = AssetRefreshPayload {
        story_outline,
        characters,
        world,
        scene_outline,
    };
    if payload.story_outline.is_none()
        && payload.characters.is_none()
        && payload.world.is_none()
        && payload.scene_outline.is_none()
    {
        return None;
    }
    Some(payload)
}

/// Gemma 等思考模型常把 JSON 留在思维链，content 只剩一段归纳散文。
pub fn salvage_refresh_payload(
    raw: &str,
    targets: &[AssetRefreshTarget],
) -> Option<AssetRefreshPayload> {
    if let Some(p) = parse_labeled_refresh(raw) {
        return Some(p);
    }
    if targets.len() != 1 {
        return None;
    }
    let stripped = crate::narrative::strip_reasoning_blocks(raw);
    let text = stripped.trim();
    if text.chars().count() < 30 {
        return None;
    }
    if text.starts_with('{') || text.starts_with('[') {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    if text.contains("只输出 JSON")
        || text.contains("不要 markdown")
        || (lower.contains("json") && text.contains("输出"))
    {
        return None;
    }
    match targets[0] {
        AssetRefreshTarget::StoryOutline => Some(AssetRefreshPayload {
            story_outline: Some(text.to_string()),
            ..Default::default()
        }),
        AssetRefreshTarget::SceneOutline => Some(AssetRefreshPayload {
            scene_outline: Some(text.to_string()),
            ..Default::default()
        }),
        AssetRefreshTarget::World => Some(AssetRefreshPayload {
            world: Some(RefreshWorld {
                concept: Some(text.to_string()),
                history: None,
            }),
            ..Default::default()
        }),
        AssetRefreshTarget::Characters => None,
    }
}

fn target_label(t: AssetRefreshTarget) -> &'static str {
    match t {
        AssetRefreshTarget::StoryOutline => "故事大纲",
        AssetRefreshTarget::Characters => "角色",
        AssetRefreshTarget::World => "世界观",
        AssetRefreshTarget::SceneOutline => "场景大纲",
    }
}

fn summarize(targets: &[AssetRefreshTarget], story: Option<&str>, scene: Option<&str>) -> String {
    let names: Vec<&str> = targets.iter().copied().map(target_label).collect();
    let mut body = format!("已按正文重写{}。纸面未改。", names.join("、"));
    if let Some(s) = story.map(str::trim).filter(|s| !s.is_empty()) {
        body.push_str("\n\n【故事大纲】\n");
        body.push_str(s);
    }
    if let Some(s) = scene.map(str::trim).filter(|s| !s.is_empty()) {
        body.push_str("\n\n【场景大纲】\n");
        body.push_str(s);
    }
    body
}

fn dual_window(text: &str) -> String {
    let plain = strip_editor_markup(text);
    let chars: Vec<char> = plain.chars().collect();
    if chars.len() <= DUAL_HEAD + DUAL_TAIL {
        return plain;
    }
    let head: String = chars.iter().take(DUAL_HEAD).collect();
    let tail_start = chars.len().saturating_sub(DUAL_TAIL);
    let tail: String = chars.iter().skip(tail_start).collect();
    format!("{head}\n…\n{tail}")
}

fn one_liner(text: &str, cap: usize) -> String {
    let plain = strip_editor_markup(text);
    let line = plain.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let s: String = line.chars().take(cap).collect();
    s.trim().to_string()
}

pub(crate) fn assemble_refresh_prose(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
) -> Result<String, AppError> {
    let scenes = SceneRepository::new(pool.clone())
        .get_by_story(story_id)
        .map_err(AppError::from)?;
    let current_id = scene_id.filter(|s| !s.is_empty());
    let mut current_block = String::new();
    let mut others = String::new();
    for scene in &scenes {
        let body = scene.content.clone().unwrap_or_default();
        let outline = scene.outline_content.clone().unwrap_or_default();
        let is_current = current_id.map(|id| id == scene.id).unwrap_or(false);
        if is_current {
            current_block = format!(
                "【当前打开章节】\n{}\n",
                dual_window(if body.trim().is_empty() {
                    &outline
                } else {
                    &body
                })
            );
            continue;
        }
        let hint = if !outline.trim().is_empty() {
            one_liner(&outline, 80)
        } else {
            one_liner(&body, 80)
        };
        if hint.is_empty() {
            continue;
        }
        others.push_str(&format!("第{}场：{}\n", scene.sequence_number, hint));
    }
    if current_block.is_empty() {
        current_block = dual_window(&concat_story_prose(pool, story_id));
    }
    let mut out = format!("{current_block}\n{others}");
    if out.chars().count() > PROSE_BUDGET {
        out = current_block;
        if out.chars().count() > PROSE_BUDGET {
            out = out.chars().take(PROSE_BUDGET).collect();
        }
    }
    Ok(out)
}

fn drop_ungrounded_names(text: &str, prose: &str, names: &[String]) -> String {
    let mut out = text.to_string();
    for name in names {
        let n = name.trim();
        if n.is_empty() {
            continue;
        }
        if !name_in_prose(n, prose) && out.contains(n) {
            out = out.replace(n, "");
        }
    }
    out
}

fn candidate_names(pool: &DbPool, story_id: &str, payload: &AssetRefreshPayload) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(chars) = CharacterRepository::new(pool.clone()).get_by_story(story_id) {
        for c in chars {
            if !c.name.trim().is_empty() {
                names.push(c.name);
            }
        }
    }
    if let Some(ref incoming) = payload.characters {
        for c in incoming {
            let n = c.name.trim();
            if !n.is_empty() && !names.iter().any(|e| e == n) {
                names.push(n.to_string());
            }
        }
    }
    names
}

fn maybe_fill(
    existing: &Option<String>,
    incoming: Option<&str>,
    refinable: bool,
) -> Option<String> {
    let new = incoming
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let filled = existing
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    match new {
        Some(n) if refinable || !filled => Some(n),
        _ => None,
    }
}

pub fn persist_asset_refresh(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
    targets: &[AssetRefreshTarget],
    parsed: &AssetRefreshPayload,
    allow_overwrite_manual: bool,
) -> Result<String, AppError> {
    persist_asset_refresh_inner(
        pool,
        story_id,
        scene_id,
        targets,
        parsed,
        allow_overwrite_manual,
        false,
    )
}

/// 用户确认稿按手改落库：不再按正文删人名。
pub fn persist_confirmed_outlines(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
    story_outline: Option<&str>,
    scene_outline: Option<&str>,
    allow_overwrite_manual: bool,
) -> Result<String, AppError> {
    let mut targets = Vec::new();
    let mut parsed = AssetRefreshPayload::default();
    if let Some(s) = story_outline.map(str::trim).filter(|s| !s.is_empty()) {
        targets.push(AssetRefreshTarget::StoryOutline);
        parsed.story_outline = Some(s.to_string());
    }
    if let Some(s) = scene_outline.map(str::trim).filter(|s| !s.is_empty()) {
        targets.push(AssetRefreshTarget::SceneOutline);
        parsed.scene_outline = Some(s.to_string());
    }
    if targets.is_empty() {
        return Err(AppError::validation_failed(
            "请至少保留一段大纲再确认",
            Some("empty_confirm"),
        ));
    }
    persist_asset_refresh_inner(
        pool,
        story_id,
        scene_id,
        &targets,
        &parsed,
        allow_overwrite_manual,
        true,
    )
}

pub fn preview_refresh_outlines(
    pool: &DbPool,
    story_id: &str,
    targets: &[AssetRefreshTarget],
    parsed: &AssetRefreshPayload,
) -> (Option<String>, Option<String>) {
    let prose = concat_story_prose(pool, story_id);
    let names = candidate_names(pool, story_id, parsed);
    let story = if targets.contains(&AssetRefreshTarget::StoryOutline) {
        parsed
            .story_outline
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|raw| {
                cap_story_outline_content(&strip_outline_planning(&drop_ungrounded_names(
                    raw, &prose, &names,
                )))
            })
            .filter(|s| !s.trim().is_empty())
    } else {
        None
    };
    let scene = if targets.contains(&AssetRefreshTarget::SceneOutline) {
        parsed
            .scene_outline
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|raw| drop_ungrounded_names(raw, &prose, &names))
            .filter(|s| !s.trim().is_empty())
    } else {
        None
    };
    (story, scene)
}

fn persist_asset_refresh_inner(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
    targets: &[AssetRefreshTarget],
    parsed: &AssetRefreshPayload,
    allow_overwrite_manual: bool,
    trust_user_text: bool,
) -> Result<String, AppError> {
    let prose = concat_story_prose(pool, story_id);
    let names = candidate_names(pool, story_id, parsed);
    let mut wrote = false;
    let mut wrote_story: Option<String> = None;
    let mut wrote_scene: Option<String> = None;

    if targets.contains(&AssetRefreshTarget::StoryOutline) {
        if let Some(raw) = parsed
            .story_outline
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let grounded = if trust_user_text {
                raw.to_string()
            } else {
                drop_ungrounded_names(raw, &prose, &names)
            };
            let capped = if trust_user_text {
                cap_story_outline_content(&grounded)
            } else {
                cap_story_outline_content(&strip_outline_planning(&grounded))
            };
            if !capped.trim().is_empty() {
                let repo = StoryOutlineRepository::new(pool.clone());
                if repo
                    .get_by_story(story_id)
                    .map_err(AppError::from)?
                    .is_some()
                {
                    repo.update(story_id, Some(&capped), None)
                        .map_err(AppError::from)?;
                } else {
                    repo.create(story_id, &capped, None, 3, None)
                        .map_err(AppError::from)?;
                }
                wrote = true;
                wrote_story = Some(capped);
            }
        }
    }

    if targets.contains(&AssetRefreshTarget::Characters) {
        if persist_characters(pool, story_id, &prose, parsed, allow_overwrite_manual)? {
            wrote = true;
        }
    }

    if targets.contains(&AssetRefreshTarget::World) {
        if persist_world(pool, story_id, parsed, allow_overwrite_manual)? {
            wrote = true;
        }
    }

    if targets.contains(&AssetRefreshTarget::SceneOutline) {
        if persist_scene_outline(
            pool,
            story_id,
            scene_id,
            &prose,
            &names,
            parsed,
            trust_user_text,
        )? {
            wrote = true;
            if let Some(raw) = parsed
                .scene_outline
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                wrote_scene = Some(raw.to_string());
            }
        }
    }

    if !wrote {
        return Err(AppError::validation_failed(
            "模型未返回可落地的设定，未改动资产",
            Some("empty_refresh"),
        ));
    }
    let summary = summarize(targets, wrote_story.as_deref(), wrote_scene.as_deref());
    log::info!(
        "asset_refresh: persisted story_chars={} scene_chars={}",
        wrote_story.as_ref().map(|s| s.chars().count()).unwrap_or(0),
        wrote_scene.as_ref().map(|s| s.chars().count()).unwrap_or(0)
    );
    Ok(summary)
}

fn persist_characters(
    pool: &DbPool,
    story_id: &str,
    prose: &str,
    parsed: &AssetRefreshPayload,
    allow_overwrite_manual: bool,
) -> Result<bool, AppError> {
    let Some(incoming) = parsed.characters.as_ref() else {
        return Ok(false);
    };
    let repo = CharacterRepository::new(pool.clone());
    let existing = repo.get_by_story(story_id).map_err(AppError::from)?;
    let mut wrote = false;
    for ch in incoming {
        let name = ch.name.trim();
        if name.is_empty() || !name_in_prose(name, prose) {
            continue;
        }
        if let Some(row) = existing.iter().find(|e| e.name == name) {
            let refinable = allow_overwrite_manual || is_refinable(row.source.as_deref());
            let bg = maybe_fill(&row.background, ch.background.as_deref(), refinable);
            let personality = maybe_fill(&row.personality, ch.personality.as_deref(), refinable);
            let goals = maybe_fill(&row.goals, ch.goals.as_deref(), refinable);
            let appearance = maybe_fill(&row.appearance, ch.appearance.as_deref(), refinable);
            if bg.is_some() || personality.is_some() || goals.is_some() || appearance.is_some() {
                repo.update(
                    &row.id,
                    None,
                    bg,
                    personality,
                    goals,
                    appearance,
                    None,
                    None,
                )
                .map_err(AppError::from)?;
                wrote = true;
            }
            let core = maybe_fill(&row.emotional_core, ch.emotional_core.as_deref(), refinable);
            let trigger = maybe_fill(
                &row.emotional_trigger,
                ch.emotional_trigger.as_deref(),
                refinable,
            );
            let wound = maybe_fill(
                &row.emotional_wound,
                ch.emotional_wound.as_deref(),
                refinable,
            );
            let need = maybe_fill(&row.emotional_need, ch.emotional_need.as_deref(), refinable);
            if core.is_some() || trigger.is_some() || wound.is_some() || need.is_some() {
                repo.update_emotional(&row.id, core, trigger, wound, need)
                    .map_err(AppError::from)?;
                wrote = true;
            }
        } else {
            repo.create(CreateCharacterRequest {
                story_id: story_id.to_string(),
                name: name.to_string(),
                background: ch.background.clone(),
                personality: ch.personality.clone(),
                goals: ch.goals.clone(),
                appearance: ch.appearance.clone(),
                gender: None,
                age: None,
                source: Some("agency".into()),
                is_auto_generated: Some(true),
                emotional_core: ch.emotional_core.clone(),
                emotional_trigger: ch.emotional_trigger.clone(),
                emotional_wound: ch.emotional_wound.clone(),
                emotional_need: ch.emotional_need.clone(),
            })
            .map_err(AppError::from)?;
            wrote = true;
        }
    }
    Ok(wrote)
}

fn persist_world(
    pool: &DbPool,
    story_id: &str,
    parsed: &AssetRefreshPayload,
    allow_overwrite_manual: bool,
) -> Result<bool, AppError> {
    let Some(world) = parsed.world.as_ref() else {
        return Ok(false);
    };
    let concept = world
        .concept
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let history = world
        .history
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if concept.is_none() && history.is_none() {
        return Ok(false);
    }
    let repo = WorldBuildingRepository::new(pool.clone());
    match repo.get_by_story(story_id).map_err(AppError::from)? {
        None => {
            let c = concept.unwrap_or("");
            let wb = repo
                .create_with_source(story_id, c, Some("agency"), Some(true))
                .map_err(AppError::from)?;
            if let Some(h) = history {
                repo.update(&wb.id, None, None, Some(h), None)
                    .map_err(AppError::from)?;
            }
            Ok(true)
        }
        Some(wb) => {
            let refinable = allow_overwrite_manual || is_refinable(wb.source.as_deref());
            let concept_filled = !wb.concept.trim().is_empty();
            let write_concept = concept.filter(|_| refinable || !concept_filled);
            let hist_opt = wb.history.clone();
            let write_history = maybe_fill(&hist_opt, history, refinable);
            if write_concept.is_none() && write_history.is_none() {
                return Ok(false);
            }
            repo.update(&wb.id, write_concept, None, write_history.as_deref(), None)
                .map_err(AppError::from)?;
            Ok(true)
        }
    }
}

fn persist_scene_outline(
    pool: &DbPool,
    _story_id: &str,
    scene_id: Option<&str>,
    prose: &str,
    names: &[String],
    parsed: &AssetRefreshPayload,
    trust_user_text: bool,
) -> Result<bool, AppError> {
    let Some(raw) = parsed
        .scene_outline
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(false);
    };
    let sid = scene_id
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::validation_failed("请先打开一个章节", Some("no_scene")))?;
    let repo = SceneRepository::new(pool.clone());
    let scene = repo
        .get_by_id(sid)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::validation_failed("请先打开一个章节", Some("no_scene")))?;
    let grounded = if trust_user_text {
        raw.to_string()
    } else {
        drop_ungrounded_names(raw, prose, names)
    };
    if grounded.trim().is_empty() {
        return Ok(false);
    }
    let block = if grounded.contains(CURRENT_SCENE_OUTLINE_MARK) {
        grounded
    } else {
        format!("{CURRENT_SCENE_OUTLINE_MARK}\n{grounded}")
    };
    let merged = merge_current_scene_outline(scene.outline_content.as_deref(), &block);
    repo.update(
        sid,
        &SceneUpdate {
            outline_content: Some(merged),
            ..Default::default()
        },
    )
    .map_err(AppError::from)?;
    Ok(true)
}

fn build_refresh_prompts(
    targets: &[AssetRefreshTarget],
    prose: &str,
    user_input: &str,
) -> (String, String) {
    let mut keys = Vec::new();
    if targets.contains(&AssetRefreshTarget::StoryOutline) {
        keys.push(
            "story_outline（字符串：正文已发生的核心冲突与转折，以及正文已经指向、尚未写到的下一步。姓名必须出自正文。不要只复述当前场已写完的动作。）",
        );
    }
    if targets.contains(&AssetRefreshTarget::Characters) {
        keys.push(
            "characters（数组，每项 name 必填，可选 background/personality/goals/appearance/emotional_core/emotional_trigger/emotional_wound/emotional_need）",
        );
    }
    if targets.contains(&AssetRefreshTarget::World) {
        keys.push("world（对象：concept、history）");
    }
    if targets.contains(&AssetRefreshTarget::SceneOutline) {
        keys.push(
            "scene_outline（字符串：当前打开这一场从正文末句往后接下来怎么演。禁止复述已发生的开场或已完成动作。）",
        );
    }
    let system = format!(
        "你是小说资产编辑。只根据已有正文归纳设定，禁止发明正文未出现的人名/地名。\
只输出 JSON，不要 markdown 围栏，不要正文。只包含这些键：{}。",
        keys.join("；")
    );
    let next_hint = if user_input.contains("后续")
        || user_input.contains("接下来")
        || user_input.contains("下一")
    {
        "用户要的是后续方向，不是把已写章节再抄一遍。禁止复述开场。"
    } else {
        ""
    };
    let user = format!(
        "用户指令：{user_input}\n\n【已有正文（只读）】\n{prose}\n\n按指令重写上述设定，姓名必须能在正文中找到。\n{next_hint}"
    );
    (system, user)
}

/// Producer 一次 JSON：失败不写库。不创建会挡住续写的 Agency run。
pub async fn execute(
    pool: DbPool,
    llm: Arc<dyn LoopLlm>,
    story_id: &str,
    scene_id: Option<&str>,
    user_input: &str,
) -> Result<PlanExecutionResult, AppError> {
    let targets = parse_asset_refresh_targets(user_input);
    if targets.is_empty() {
        return Err(AppError::validation_failed(
            "请说明要按正文重写哪一类设定：故事大纲、角色、世界观，还是场景大纲。也可以说「全部设定」。",
            Some("asset_refresh_targets"),
        ));
    }

    let pool_gate = pool.clone();
    let sid = story_id.to_string();
    tokio::task::spawn_blocking(move || {
        let repo = AgencyRepository::new(pool_gate.clone());
        if repo
            .has_blocking_creative_run(&sid)
            .map_err(AppError::from)?
        {
            return Err(AppError::validation_failed(
                "正在续写中，请等当前续写结束或取消后再重写设定",
                Some("blocking_run"),
            ));
        }
        let prose = concat_story_prose(&pool_gate, &sid);
        if !has_substantial_prose(&prose) {
            return Err(AppError::validation_failed(
                "请先写一些章节正文，再按正文重写设定",
                Some("no_prose"),
            ));
        }
        Ok(())
    })
    .await
    .map_err(|e| AppError::from(format!("asset_refresh gate: {e}")))??;

    let pool_budget = pool.clone();
    let sid_b = story_id.to_string();
    let scene_b = scene_id.map(|s| s.to_string());
    let budgeted = tokio::task::spawn_blocking(move || {
        assemble_refresh_prose(&pool_budget, &sid_b, scene_b.as_deref())
    })
    .await
    .map_err(|e| AppError::from(format!("asset_refresh prose: {e}")))??;

    let (system, user) = build_refresh_prompts(&targets, &budgeted, user_input);
    let raw = llm
        .complete_json(&system, &user, TaskType::WorldBuilding, 4096)
        .await?;
    let parsed = parse_refresh_payload(&raw)
        .or_else(|| salvage_refresh_payload(&raw, &targets))
        .ok_or_else(|| {
            let preview: String = raw.chars().take(200).collect();
            log::warn!(
                "asset_refresh: JSON 解析失败 raw_chars={} preview={}",
                raw.chars().count(),
                preview.replace('\n', " ")
            );
            AppError::validation_failed("未能解析设定 JSON，未改动任何资产", Some("parse_fail"))
        })?;

    let pool_w = pool.clone();
    let sid_w = story_id.to_string();
    let scene_w = scene_id.map(|s| s.to_string());
    let targets_w = targets.clone();
    let overwrite = allow_overwrite_manual(user_input);
    let instruction = user_input.to_string();
    let (summary, draft) = tokio::task::spawn_blocking(move || {
        if needs_outline_confirm(&targets_w) {
            let (story_preview, scene_preview) =
                preview_refresh_outlines(&pool_w, &sid_w, &targets_w, &parsed);
            if story_preview.is_none() && scene_preview.is_none() {
                return Err(AppError::validation_failed(
                    "模型未返回可落地的设定，未改动资产",
                    Some("empty_refresh"),
                ));
            }
            let others: Vec<AssetRefreshTarget> = targets_w
                .iter()
                .copied()
                .filter(|t| {
                    !matches!(
                        t,
                        AssetRefreshTarget::StoryOutline | AssetRefreshTarget::SceneOutline
                    )
                })
                .collect();
            if !others.is_empty() {
                let _ = persist_asset_refresh(
                    &pool_w,
                    &sid_w,
                    scene_w.as_deref(),
                    &others,
                    &parsed,
                    overwrite,
                );
            }
            let summary = summarize(
                &targets_w,
                story_preview.as_deref(),
                scene_preview.as_deref(),
            );
            let draft = AssetRefreshDraft {
                story_id: sid_w,
                scene_id: scene_w,
                overwrite_manual: overwrite,
                instruction,
                story_outline: story_preview,
                scene_outline: scene_preview,
            };
            Ok((summary, Some(draft)))
        } else {
            let summary = persist_asset_refresh(
                &pool_w,
                &sid_w,
                scene_w.as_deref(),
                &targets_w,
                &parsed,
                overwrite,
            )?;
            Ok((summary, None))
        }
    })
    .await
    .map_err(|e| AppError::from(format!("asset_refresh persist: {e}")))??;

    Ok(PlanExecutionResult {
        success: true,
        steps_completed: 1,
        final_content: Some(summary),
        messages: vec![if draft.is_some() {
            "请确认大纲后再保存".into()
        } else {
            "设定已按正文更新".into()
        }],
        error: None,
        result_kind: Some("asset_refresh".into()),
        asset_refresh_draft: draft,
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use super::*;
    use crate::{
        agency::models::AgencyRun,
        db::{connection::create_test_pool, dto::CreateStoryRequest, StoryRepository},
    };

    fn hanxue_prose() -> String {
        "韩雪在首尔雨夜把枪口对准李明。".repeat(20)
    }

    fn seed_story(pool: &DbPool, prose: &str) -> (String, String) {
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "资产刷新".into(),
                description: None,
                genre: Some("谍战".into()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let scene_repo = SceneRepository::new(pool.clone());
        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();
        let scene = scene_repo
            .create_in_tx(&tx, &story.id, 1, Some("第一章"))
            .unwrap();
        scene_repo
            .update_in_tx(
                &tx,
                &scene.id,
                &SceneUpdate {
                    content: Some(prose.into()),
                    outline_content: Some("用户手写大纲前缀".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        tx.commit().unwrap();
        (story.id, scene.id)
    }

    fn scene_content(pool: &DbPool, scene_id: &str) -> String {
        SceneRepository::new(pool.clone())
            .get_by_id(scene_id)
            .unwrap()
            .unwrap()
            .content
            .unwrap_or_default()
    }

    #[test]
    fn parse_targets_story_outline_only() {
        let t = parse_asset_refresh_targets("将故事大纲按照现有正文重新写过");
        assert_eq!(t, vec![AssetRefreshTarget::StoryOutline]);
    }

    #[test]
    fn parse_targets_characters_and_world() {
        let t = parse_asset_refresh_targets("把角色和世界观按正文重写");
        assert!(t.contains(&AssetRefreshTarget::Characters));
        assert!(t.contains(&AssetRefreshTarget::World));
        assert!(!t.contains(&AssetRefreshTarget::StoryOutline));
    }

    #[test]
    fn parse_targets_all_assets() {
        let t = parse_asset_refresh_targets("按正文重写全部设定");
        assert_eq!(t.len(), 4);
    }

    #[test]
    fn parse_targets_empty_when_unspecified() {
        assert!(parse_asset_refresh_targets("帮我看看").is_empty());
    }

    #[test]
    fn parse_targets_regen_story_and_scene() {
        let t = parse_asset_refresh_targets("根据正文内容重新生成故事大纲和场景大纲");
        assert!(t.contains(&AssetRefreshTarget::StoryOutline));
        assert!(t.contains(&AssetRefreshTarget::SceneOutline));
    }

    #[test]
    fn parse_targets_subsequent_story_and_generate_scene() {
        let input = "根据正文内容重新写后续的故事大纲，同时生成场景大纲";
        assert!(looks_like_asset_refresh_shape(input));
        let t = parse_asset_refresh_targets(input);
        assert!(t.contains(&AssetRefreshTarget::StoryOutline));
        assert!(t.contains(&AssetRefreshTarget::SceneOutline));
    }

    #[test]
    fn looks_like_regen_outline_from_prose() {
        assert!(looks_like_asset_refresh_shape(
            "根据正文内容重新生成故事大纲和场景大纲"
        ));
        assert!(!looks_like_asset_refresh_shape("根据正文重新生成下一章"));
    }

    #[test]
    fn looks_like_write_subsequent_outlines_without_prose_keyword() {
        // 真机 2026-08-24：缺「正文/重新写」被当成续写进 Agency Append。
        let input = "写后续的故事大纲，同时生成后续的场景大纲";
        assert!(looks_like_asset_refresh_shape(input));
        let t = parse_asset_refresh_targets(input);
        assert!(t.contains(&AssetRefreshTarget::StoryOutline));
        assert!(t.contains(&AssetRefreshTarget::SceneOutline));
        assert!(!looks_like_asset_refresh_shape("写后续"));
        assert!(!looks_like_asset_refresh_shape("按照故事大纲写后续"));
        assert!(!looks_like_asset_refresh_shape("按照故事大纲继续写"));
        assert!(!looks_like_asset_refresh_shape("写一部科幻小说"));
    }

    #[test]
    fn parse_labeled_story_and_scene_from_gemma_prose() {
        // 真机 2026-08-22：无花括号，英文冒号。
        let raw = "story_outline:苏亦铁与明成公主的婚礼上，苏会山被明成公主偷袭致死，随后苏会山面容发生恐怖变化，引发了混乱。转折点在于苏亦铁对苏会山惨状的悲愤反应。 scene_outline:第一场：知启纪元八百四十七年，大奉帝国西北边陲重镇黑崎州城，大雪初晴。镇北王府正院笙乐齐鸣，红绸如霞。";
        let p = parse_refresh_payload(raw).expect("键值散文应解析");
        assert!(p.story_outline.unwrap().contains("苏亦铁"));
        assert!(p.scene_outline.unwrap().contains("第一场"));
    }

    #[test]
    fn parse_labeled_scene_outline_fullwidth_colon() {
        // 真机 2026-08-22：仅场景大纲，中文冒号。
        let raw = "scene_outline：进入王府大堂，苏会山与曹元佩等候新人。明成公主乘坐小暧轿抵达，苏亦铁轻掀轿帘，抱下公主。";
        let p = parse_refresh_payload(raw).expect("中文冒号应解析");
        assert!(p.scene_outline.unwrap().contains("王府大堂"));
        assert!(p.story_outline.is_none());
    }

    #[test]
    fn parse_refresh_payload_accepts_object_story_outline() {
        // 真机 Gemma 4：82 字量级，story_outline 常是对象不是字符串（v0.30.29
        // 同类）。
        let raw = r#"{"story_outline":{"core_conflict":"韩雪在首尔雨夜把枪口对准李明"}}"#;
        let p = parse_refresh_payload(raw).expect("对象大纲应解析");
        let outline = p.story_outline.expect("应有故事大纲");
        assert!(outline.contains("韩雪"));
        assert!(outline.contains("核心冲突"));
    }

    #[test]
    fn parse_refresh_payload_accepts_chinese_key() {
        let raw = r#"{"故事大纲":"【核心冲突】韩雪与李明在首尔对峙"}"#;
        let p = parse_refresh_payload(raw).expect("中文键应解析");
        assert!(p.story_outline.unwrap().contains("韩雪"));
    }

    #[test]
    fn parse_refresh_payload_accepts_top_level_core_conflict() {
        let raw = r#"{"core_conflict":"韩雪在首尔雨夜对峙李明","turning_points":["李明拒捕"]}"#;
        let p = parse_refresh_payload(raw).expect("DepthAssets 形大纲应解析");
        assert!(p.story_outline.unwrap().contains("韩雪"));
    }

    #[test]
    fn salvage_story_outline_prose_when_model_skips_json() {
        let raw = "韩雪在首尔雨夜把枪口对准李明。两人在巷口对峙，谁先开枪谁就会失去谈判筹码，雨把枪油冲得发亮。";
        let t = vec![AssetRefreshTarget::StoryOutline];
        let p = salvage_refresh_payload(raw, &t).expect("仅故事大纲时应 salvage 散文");
        assert!(p.story_outline.unwrap().contains("韩雪"));
    }

    #[test]
    fn salvage_rejects_short_or_instruction_echo() {
        let t = vec![AssetRefreshTarget::StoryOutline];
        assert!(salvage_refresh_payload("这不是 JSON", &t).is_none());
        assert!(salvage_refresh_payload("只输出 JSON，不要 markdown 围栏。", &t).is_none());
        assert!(salvage_refresh_payload("{\"story_outline\":\"未闭合", &t).is_none());
    }

    #[test]
    fn salvage_scene_outline_unlabeled_prose() {
        let raw = "韩雪在首尔雨夜把枪口对准李明。两人在巷口对峙，谁先开枪谁就会失去谈判筹码，雨把枪油冲得发亮。";
        let t = vec![AssetRefreshTarget::SceneOutline];
        let p = salvage_refresh_payload(raw, &t).expect("仅场景大纲时应 salvage 散文");
        assert!(p.scene_outline.unwrap().contains("韩雪"));
        assert!(p.story_outline.is_none());
    }

    #[test]
    fn persist_story_outline_does_not_touch_scene_content() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let before = scene_content(&pool, &scene_id);
        persist_asset_refresh(
            &pool,
            &story_id,
            Some(&scene_id),
            &[AssetRefreshTarget::StoryOutline],
            &AssetRefreshPayload {
                story_outline: Some("【核心冲突】韩雪与李明在首尔对峙".into()),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        assert_eq!(scene_content(&pool, &scene_id), before);
        let outline = StoryOutlineRepository::new(pool)
            .get_by_story(&story_id)
            .unwrap()
            .unwrap();
        assert!(outline.content.contains("韩雪"));
    }

    #[test]
    fn persist_summary_includes_written_outline_text() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let summary = persist_asset_refresh(
            &pool,
            &story_id,
            Some(&scene_id),
            &[
                AssetRefreshTarget::StoryOutline,
                AssetRefreshTarget::SceneOutline,
            ],
            &AssetRefreshPayload {
                story_outline: Some("【核心冲突】韩雪在首尔雨夜对峙李明".into()),
                scene_outline: Some("韩雪举枪，李明停在雨里不敢先动。".into()),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        assert!(summary.contains("已按正文重写"));
        assert!(summary.contains("纸面未改"));
        assert!(summary.contains("【故事大纲】"));
        assert!(summary.contains("韩雪在首尔雨夜对峙李明"));
        assert!(summary.contains("【场景大纲】"));
        assert!(summary.contains("举枪"));
    }

    #[test]
    fn refresh_prompt_asks_for_next_beat_not_opening_recap() {
        let (system, user) = build_refresh_prompts(
            &[
                AssetRefreshTarget::StoryOutline,
                AssetRefreshTarget::SceneOutline,
            ],
            "韩雪在首尔雨夜把枪口对准李明。",
            "根据正文内容重新写后续的故事大纲，同时生成场景大纲",
        );
        assert!(system.contains("下一步") || system.contains("接下来"));
        assert!(system.contains("禁止复述") || user.contains("禁止复述"));
        assert!(user.contains("后续"));
    }

    #[test]
    fn persist_drops_names_absent_from_prose() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        persist_asset_refresh(
            &pool,
            &story_id,
            Some(&scene_id),
            &[
                AssetRefreshTarget::StoryOutline,
                AssetRefreshTarget::Characters,
            ],
            &AssetRefreshPayload {
                story_outline: Some("【核心冲突】金敏秀与韩雪争夺核电站".into()),
                characters: Some(vec![
                    RefreshCharacter {
                        name: "金敏秀".into(),
                        background: Some("发明出来的人".into()),
                        ..Default::default()
                    },
                    RefreshCharacter {
                        name: "韩雪".into(),
                        background: Some("雨夜里的特工".into()),
                        ..Default::default()
                    },
                ]),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        let outline = StoryOutlineRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap()
            .unwrap();
        assert!(!outline.content.contains("金敏秀"), "未出场人名不得进大纲");
        let chars = CharacterRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        assert!(chars.iter().any(|c| c.name == "韩雪"));
        assert!(!chars.iter().any(|c| c.name == "金敏秀"));
        assert_eq!(scene_content(&pool, &scene_id), prose);
    }

    #[test]
    fn persist_preserves_user_created_emotional_core() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        CharacterRepository::new(pool.clone())
            .create(CreateCharacterRequest {
                story_id: story_id.clone(),
                name: "韩雪".into(),
                background: None,
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,
                source: Some("user_created".into()),
                is_auto_generated: Some(false),
                emotional_core: Some("旧核".into()),
                emotional_trigger: None,
                emotional_wound: None,
                emotional_need: None,
            })
            .unwrap();
        persist_asset_refresh(
            &pool,
            &story_id,
            Some(&scene_id),
            &[AssetRefreshTarget::Characters],
            &AssetRefreshPayload {
                characters: Some(vec![RefreshCharacter {
                    name: "韩雪".into(),
                    background: Some("雨夜特工".into()),
                    emotional_core: Some("新核不得覆盖".into()),
                    ..Default::default()
                }]),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        let row = CharacterRepository::new(pool)
            .get_by_story(&story_id)
            .unwrap()
            .into_iter()
            .find(|c| c.name == "韩雪")
            .unwrap();
        assert_eq!(row.emotional_core.as_deref(), Some("旧核"));
        assert_eq!(row.background.as_deref(), Some("雨夜特工"));
    }

    #[test]
    fn persist_refines_ingest_character() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        CharacterRepository::new(pool.clone())
            .create(CreateCharacterRequest {
                story_id: story_id.clone(),
                name: "韩雪".into(),
                background: Some("旧背景".into()),
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,
                source: Some("ingest".into()),
                is_auto_generated: Some(true),
                emotional_core: Some("旧核".into()),
                emotional_trigger: None,
                emotional_wound: None,
                emotional_need: None,
            })
            .unwrap();
        persist_asset_refresh(
            &pool,
            &story_id,
            Some(&scene_id),
            &[AssetRefreshTarget::Characters],
            &AssetRefreshPayload {
                characters: Some(vec![RefreshCharacter {
                    name: "韩雪".into(),
                    background: Some("新背景".into()),
                    emotional_core: Some("新核".into()),
                    ..Default::default()
                }]),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        let row = CharacterRepository::new(pool)
            .get_by_story(&story_id)
            .unwrap()
            .into_iter()
            .find(|c| c.name == "韩雪")
            .unwrap();
        assert_eq!(row.background.as_deref(), Some("新背景"));
        assert_eq!(row.emotional_core.as_deref(), Some("新核"));
    }

    #[test]
    fn persist_scene_outline_keeps_handwritten_prefix() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        persist_asset_refresh(
            &pool,
            &story_id,
            Some(&scene_id),
            &[AssetRefreshTarget::SceneOutline],
            &AssetRefreshPayload {
                scene_outline: Some("韩雪继续对峙，不让李明离开雨巷。".into()),
                ..Default::default()
            },
            false,
        )
        .unwrap();
        let scene = SceneRepository::new(pool.clone())
            .get_by_id(&scene_id)
            .unwrap()
            .unwrap();
        let outline = scene.outline_content.unwrap();
        assert!(outline.contains("用户手写大纲前缀"));
        assert!(outline.contains(CURRENT_SCENE_OUTLINE_MARK));
        assert!(outline.contains("韩雪"));
        assert_eq!(scene.content.as_deref(), Some(prose.as_str()));
    }

    #[test]
    fn asset_refresh_is_not_append_continue() {
        assert!(!crate::agency::persist::should_agency_append_continue(
            false, None
        ));
    }

    #[test]
    fn asset_refresh_result_kind_is_not_prose() {
        let r = PlanExecutionResult {
            success: true,
            steps_completed: 1,
            final_content: Some("已按正文重写故事大纲".into()),
            messages: vec![],
            error: None,
            result_kind: Some("asset_refresh".into()),
            asset_refresh_draft: None,
        };
        assert_eq!(r.result_kind.as_deref(), Some("asset_refresh"));
    }

    struct ScriptedLlm {
        responses: Mutex<VecDeque<String>>,
    }

    impl ScriptedLlm {
        fn json(body: &str) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(VecDeque::from([body.to_string()])),
            })
        }
    }

    #[async_trait::async_trait]
    impl LoopLlm for ScriptedLlm {
        async fn complete(
            &self,
            _s: &str,
            _u: &str,
            _t: TaskType,
            _m: i32,
        ) -> Result<String, AppError> {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| AppError::validation_failed("mock exhausted", None::<String>))
        }
    }

    #[tokio::test]
    async fn execute_writes_outline_not_prose() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let llm = ScriptedLlm::json(r#"{"story_outline":"【核心冲突】韩雪在首尔雨夜对峙李明"}"#);
        let result = execute(
            pool.clone(),
            llm,
            &story_id,
            Some(&scene_id),
            "将故事大纲按照现有正文重新写过",
        )
        .await
        .unwrap();
        assert_eq!(result.result_kind.as_deref(), Some("asset_refresh"));
        assert_eq!(scene_content(&pool, &scene_id), prose);
        assert!(
            StoryOutlineRepository::new(pool.clone())
                .get_by_story(&story_id)
                .unwrap()
                .is_none(),
            "确认前不得写库"
        );
        let draft = result.asset_refresh_draft.expect("应返回大纲草稿");
        assert!(draft.story_outline.unwrap_or_default().contains("韩雪"));
    }

    #[tokio::test]
    async fn execute_salvages_prose_outline_when_json_missing() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let llm = ScriptedLlm::json(
            "韩雪在首尔雨夜把枪口对准李明。两人在巷口对峙，谁先开枪谁就会失去谈判筹码，雨把枪油冲得发亮。",
        );
        let result = execute(
            pool.clone(),
            llm,
            &story_id,
            Some(&scene_id),
            "将故事大纲按照现有正文重新写过",
        )
        .await
        .unwrap();
        assert_eq!(result.result_kind.as_deref(), Some("asset_refresh"));
        assert!(StoryOutlineRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap()
            .is_none());
        assert!(result
            .asset_refresh_draft
            .and_then(|d| d.story_outline)
            .unwrap_or_default()
            .contains("韩雪"));
    }

    #[tokio::test]
    async fn execute_scene_outline_from_labeled_prose() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let llm = ScriptedLlm::json(
            "scene_outline：韩雪在首尔雨夜把枪口对准李明。两人在巷口对峙，谁先开枪谁就会失去谈判筹码。",
        );
        let result = execute(
            pool.clone(),
            llm,
            &story_id,
            Some(&scene_id),
            "将场景大纲按照现有正文重新写过",
        )
        .await
        .unwrap();
        assert_eq!(result.result_kind.as_deref(), Some("asset_refresh"));
        assert_eq!(scene_content(&pool, &scene_id), prose);
        let scene = SceneRepository::new(pool)
            .get_by_id(&scene_id)
            .unwrap()
            .unwrap();
        assert!(
            scene
                .outline_content
                .unwrap_or_default()
                .contains("用户手写大纲前缀"),
            "确认前不得改场景大纲"
        );
        assert!(result
            .asset_refresh_draft
            .and_then(|d| d.scene_outline)
            .unwrap_or_default()
            .contains("韩雪"));
    }

    #[tokio::test]
    async fn execute_story_and_scene_from_labeled_prose() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let llm = ScriptedLlm::json(
            "story_outline:韩雪在首尔雨夜把枪口对准李明，对峙升级。 scene_outline:韩雪举枪，李明停在雨里不敢先动。",
        );
        let result = execute(
            pool.clone(),
            llm,
            &story_id,
            Some(&scene_id),
            "根据正文内容重新生成故事大纲和场景大纲",
        )
        .await
        .unwrap();
        assert_eq!(result.result_kind.as_deref(), Some("asset_refresh"));
        assert_eq!(scene_content(&pool, &scene_id), prose);
        assert!(StoryOutlineRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap()
            .is_none());
        let scene = SceneRepository::new(pool)
            .get_by_id(&scene_id)
            .unwrap()
            .unwrap();
        assert!(
            scene
                .outline_content
                .unwrap_or_default()
                .contains("用户手写大纲前缀"),
            "确认前不得改场景大纲"
        );
        let draft = result.asset_refresh_draft.expect("应返回大纲草稿");
        assert!(draft.story_outline.unwrap_or_default().contains("韩雪"));
        assert!(draft.scene_outline.unwrap_or_default().contains("举枪"));
        let report = result.final_content.unwrap_or_default();
        assert!(report.contains("【故事大纲】"));
        assert!(report.contains("【场景大纲】"));
        assert!(report.contains("纸面未改"));
    }

    #[test]
    fn confirm_persists_user_edited_outlines_as_given() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        persist_confirmed_outlines(
            &pool,
            &story_id,
            Some(&scene_id),
            Some("【核心冲突】韩雪改成自己写的后续，金敏秀也可以留下。"),
            Some("韩雪举枪，李明停在雨里。"),
            false,
        )
        .unwrap();
        let outline = StoryOutlineRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap()
            .unwrap();
        assert!(outline.content.contains("自己写的后续"));
        assert!(outline.content.contains("金敏秀"), "确认稿按手改保留");
        let scene = SceneRepository::new(pool.clone())
            .get_by_id(&scene_id)
            .unwrap()
            .unwrap();
        assert!(scene.outline_content.unwrap_or_default().contains("举枪"));
        assert_eq!(scene_content(&pool, &scene_id), prose);
    }

    #[test]
    fn confirm_empty_outlines_writes_nothing() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let err = persist_confirmed_outlines(
            &pool,
            &story_id,
            Some(&scene_id),
            Some("  "),
            Some(""),
            false,
        )
        .unwrap_err();
        assert!(err.to_string().contains("至少保留"));
        assert!(StoryOutlineRepository::new(pool)
            .get_by_story(&story_id)
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn execute_parse_fail_writes_nothing() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let llm = ScriptedLlm::json("这不是 JSON");
        let err = execute(
            pool.clone(),
            llm,
            &story_id,
            Some(&scene_id),
            "将故事大纲按照现有正文重新写过",
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("未改动"));
        assert!(StoryOutlineRepository::new(pool)
            .get_by_story(&story_id)
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn execute_refuses_blocking_creative_run() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let repo = AgencyRepository::new(pool.clone());
        let mut cont = AgencyRun::new("cont-refresh", "续写");
        cont.story_id = Some(story_id.clone());
        cont.status = "running".into();
        repo.create_run(&cont).unwrap();
        let llm = ScriptedLlm::json(r#"{"story_outline":"不该写入"}"#);
        let err = execute(
            pool,
            llm,
            &story_id,
            Some(&scene_id),
            "将故事大纲按照现有正文重新写过",
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("正在续写"));
    }

    #[tokio::test]
    async fn execute_empty_targets_asks_which() {
        let pool = create_test_pool().unwrap();
        let prose = hanxue_prose();
        let (story_id, scene_id) = seed_story(&pool, &prose);
        let llm = ScriptedLlm::json("{}");
        let err = execute(
            pool,
            llm,
            &story_id,
            Some(&scene_id),
            "按照现有正文重新写过",
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("哪一类设定"));
    }
}
