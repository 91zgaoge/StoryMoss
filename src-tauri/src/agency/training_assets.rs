//! v0.94.0：续写训练的资产重建阶段。
//!
//! 训练沙盒此前只有灌入的章节正文，没有世界观/故事大纲/角色/关系，导致
//! writer 在「创作资产区域为空」时拒绝写作或另起新故事。本模块在续写前
//! 从**上下文章**（不含末位真实基准章）重建：
//! - 世界观：concept / history / rules
//! - 故事大纲：核心冲突 / 已发生关键转折 / 下一节点
//! - 角色卡：姓名/背景/性格/目标 + 情感四字段
//! - 角色关系：source/target/类型/情感纽带
//! - 逐章梗概：写入 scenes.outline_content，供章纲生成与下一节点编译使用
//!
//! 复用既有资产落库链路：`CharacterRepository` / `WorldBuildingRepository` /
//! `StoryOutlineRepository` / `SceneRepository` /
//! `materialize::upsert_seed_relationship`。 资产重建失败不阻断训练：返回摘要，
//! writer 仍走 v0.93.16 的资产缺失兜底 与硬承接锚点。

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    agency::{
        coordinator::{normalize_outline, parse_lenient, SeedRelationship},
        materialize::upsert_seed_relationship,
        prose_ground::name_in_prose,
    },
    db::{
        dto::CreateCharacterRequest,
        models::{Culture, RuleType, WorldRule},
        repositories::{
            CharacterRepository, SceneRepository, StoryOutlineRepository, StoryRepository,
            WorldBuildingRepository,
        },
        DbPool,
    },
    llm::{adapter::ResponseFormat, LlmService},
    router::TaskType,
};

/// 资产分析 prompt 的上下文预算（字符）。训练章节多时按比例截断。
const ASSET_PROMPT_CHARS: usize = 26_000;
/// 每章在资产 prompt 的最小/最大采样窗口。
const CHAPTER_SAMPLE_MIN: usize = 800;
const CHAPTER_SAMPLE_MAX: usize = 2_600;
/// 角色卡上限（按前文出场；避免把整本书的都塞进沙盒）。
const MAX_CHARACTERS: usize = 12;

#[derive(Debug, Default, Clone)]
pub struct TrainingAssetSummary {
    pub logline: bool,
    pub world: bool,
    pub story_outline: bool,
    pub characters: usize,
    pub relationships: usize,
    pub chapter_outlines: usize,
    pub warnings: Vec<String>,
    pub llm_error: Option<String>,
}

impl TrainingAssetSummary {
    fn merge(&mut self, other: TrainingAssetSummary) {
        self.logline |= other.logline;
        self.world |= other.world;
        self.story_outline |= other.story_outline;
        self.characters += other.characters;
        self.relationships += other.relationships;
        self.chapter_outlines += other.chapter_outlines;
        self.warnings.extend(other.warnings);
    }
}

/// 训练资产重建的主入口。失败只返回摘要，不阻断训练。
pub async fn rebuild_from_context(
    pool: &DbPool,
    llm: &LlmService,
    story_id: &str,
    book_title: &str,
    chapters: &[(usize, String)],
) -> TrainingAssetSummary {
    let mut summary = TrainingAssetSummary::default();
    if chapters.is_empty() {
        summary.llm_error = Some("没有上下文章可重建资产".into());
        return summary;
    }
    let prompt = build_asset_rebuild_prompt(book_title, chapters);
    let mut last_raw = String::new();
    for attempt in 0..=1 {
        let req = if attempt == 0 {
            prompt.clone()
        } else {
            format!(
                "上一次输出为空、被截断或无法解析。这次严格只输出一个完整、合法的 JSON \
                 对象（不要 markdown、不要解释、不要代码围栏），字段可精简但必须闭合所有括号。\n\n{prompt}"
            )
        };
        // v0.98.3：JSON mode + 8192 tokens。真机 deepseek-v4 在 4096 下把
        // 全部额度烧在 reasoning_content（7770 字），content 为空；JSON mode
        // 允许适配器在 content 为空时从思维链提取最终 JSON。
        let resp = match llm
            .generate_for_task_with_format(
                TaskType::Analysis,
                req,
                Some(8192),
                Some(0.2),
                Some("continuation_training_assets"),
                Some(ResponseFormat::JsonObject),
            )
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                log::warn!("[ContinuationTraining] 资产重建 LLM 调用失败: {e}");
                summary.llm_error = Some(format!("资产重建 LLM 调用失败：{e}"));
                return summary;
            }
        };
        last_raw = resp.content;
        if let Some(bundle) = parse_bundle(&last_raw) {
            let pool_c = pool.clone();
            let sid = story_id.to_string();
            let chs = chapters.to_vec();
            let persisted = tokio::task::spawn_blocking(move || {
                persist_training_bundle(&pool_c, &sid, &chs, &bundle)
            })
            .await;
            match persisted {
                Ok(result) => {
                    summary.merge(result);
                    return summary;
                }
                Err(e) => {
                    log::warn!("[ContinuationTraining] 资产重建落库失败: {e}");
                    summary.llm_error = Some(format!("资产重建落库失败：{e}"));
                    return summary;
                }
            }
        }
        log::warn!(
            "[ContinuationTraining] 资产重建输出无法解析（第 {} 次）: raw={}",
            attempt + 1,
            preview_chars(&last_raw, 240)
        );
    }
    summary.llm_error = Some(format!(
        "资产重建输出无法解析（已重试）；raw 前 240 字: {}",
        preview_chars(&last_raw, 240)
    ));
    summary
}

fn preview_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

/// 资产重建 prompt：只允许使用前文实际出现的人名/地名，禁止发明；并把
/// 末章结尾原文保留下来给 next_node 判断。
pub(crate) fn build_asset_rebuild_prompt(book_title: &str, chapters: &[(usize, String)]) -> String {
    let n = chapters.len().max(1);
    let per = (ASSET_PROMPT_CHARS / n).clamp(CHAPTER_SAMPLE_MIN, CHAPTER_SAMPLE_MAX);
    let mut blocks = String::new();
    for (idx, (number, content)) in chapters.iter().enumerate() {
        let text = sample_chapter_text(content, per);
        let tail_hint = if idx == n - 1 {
            "\n（以上为最后一章上下文的结尾窗口；续写的 next_node 必须从这之后开始）"
        } else {
            ""
        };
        blocks.push_str(&format!("【第 {number} 章】\n{text}{tail_hint}\n\n"));
    }
    format!(
        r#"你是小说资产重建师。只依据下面提供的小说前文归纳设定，不得凭空发明人名、地名、组织名或与原文冲突的设定。所有角色名、关系端点、地点名必须能在前文中找到；不确定就省略。
请重建世界观、故事大纲、角色与关系，并为每一章给出已发生事件的梗概。只输出一个 JSON 对象（不要 markdown、不要解释文字、不要代码围栏），字段如下：
{{
  "logline": "一句话故事线（<=80字）",
  "world": {{"concept": "世界概念与冲突土壤（150-400字）", "history": "历史/势力背景（80-250字）", "rules": [{{"name": "规则名", "description": "规则说明", "importance": 8}}]}},
  "story_outline": {{"core_conflict": "当前核心冲突", "turning_points": ["前文已经发生的转折（只写已发生的事实）"], "next_node": "紧接最后一章之后、尚未写过的下一推进方向（必须落在本拍现有角色身上）"}},
  "characters": [{{"name": "前文出现的人名", "background": "来历", "personality": "性格", "goals": "目标", "emotional_core": "情感内核", "emotional_trigger": "情感触发", "emotional_wound": "情感创伤", "emotional_need": "情感需求"}}],
  "relationships": [{{"source": "角色A", "target": "角色B", "relationship_type": "师徒/敌对/亲缘等", "emotional_bond": "源→目标情感纽带", "description": "关系描述"}}],
  "chapter_outlines": [{{"chapter": 97, "summary": "本章已发生的事件与结果（60-150字）"}}]
}}
约束：
- chapter_outlines 必须覆盖输入的每一章，按原书章号填写。
- story_outline.next_node 不得与已发生事件重复；不得换主角、换地点或引入新委托。
- characters 只写前文实际出现的人名，最多 12 人。
- relationships 的 source/target 必须是 characters 中的名字。

小说：《{book_title}》
前文（只用于归纳，不要续写）：
{blocks}"#
    )
}

/// 单章采样：短章全文；长章保留开场 + 结尾窗口（保留结尾用于下一节点判断）。
pub(crate) fn sample_chapter_text(content: &str, cap: usize) -> String {
    let trimmed = content.trim();
    let total = trimmed.chars().count();
    if total <= cap {
        return trimmed.to_string();
    }
    let half = cap / 2;
    let head: String = trimmed.chars().take(half).collect();
    let tail: String = trimmed.chars().skip(total.saturating_sub(half)).collect();
    format!("{head}\n…（中间省略）…\n{tail}")
}

fn parse_bundle(raw: &str) -> Option<RawBundle> {
    if let Some(bundle) = parse_lenient::<RawBundle>(raw) {
        if bundle_has_content(&bundle) {
            return Some(bundle);
        }
    }
    // v0.98.3：输出在 JSON 中途被截断时补全括号再解析（RawBundle 字段
    // 全部可选，截断在 chapter_outlines 也能保住世界/大纲/角色）。
    if let Some(fixed) = crate::utils::json::complete_truncated_json(raw) {
        if let Ok(bundle) = serde_json::from_str::<RawBundle>(&fixed) {
            if bundle_has_content(&bundle) {
                log::warn!("[ContinuationTraining] 资产重建 JSON 被截断，已补全括号并采用可用字段");
                return Some(bundle);
            }
        }
    }
    // 本地思考模型偶尔把 JSON 留在思维链、content 只剩一段归纳散文。
    // 复用现有资产刷新解析器 salvage 世界观/故事大纲/角色；关系与逐章梗概
    // 缺失时由 persist 告警，但至少不会让训练因资产空而拒绝。
    let payload = crate::agency::asset_refresh::parse_refresh_payload(raw)?;
    let characters = payload.characters.map(|items| {
        items
            .into_iter()
            .map(|c| RawCharacter {
                name: c.name,
                background: c.background,
                personality: c.personality,
                goals: c.goals,
                appearance: c.appearance,
                gender: None,
                age: None,
                emotional_core: c.emotional_core,
                emotional_trigger: c.emotional_trigger,
                emotional_wound: c.emotional_wound,
                emotional_need: c.emotional_need,
            })
            .collect::<Vec<RawCharacter>>()
    });
    let bundle = RawBundle {
        logline: None,
        world: payload.world.map(|w| {
            serde_json::json!({
                "concept": w.concept,
                "history": w.history,
            })
        }),
        story_outline: payload.story_outline.map(Value::String),
        characters: characters.and_then(|c| serde_json::to_value(c).ok()),
        relationships: None,
        chapter_outlines: None,
    };
    bundle_has_content(&bundle).then_some(bundle)
}

/// 至少有一个实质字段才算有效 bundle；避免把空对象/随机 `{}` 当成功。
fn bundle_has_content(bundle: &RawBundle) -> bool {
    fn value_has_content(value: &Option<Value>) -> bool {
        match value {
            None | Some(Value::Null) => false,
            Some(Value::String(s)) => !s.trim().is_empty(),
            Some(Value::Array(a)) => !a.is_empty(),
            Some(Value::Object(o)) => !o.is_empty(),
            Some(_) => true,
        }
    }
    !bundle.logline.as_deref().unwrap_or("").trim().is_empty()
        || value_has_content(&bundle.world)
        || value_has_content(&bundle.story_outline)
        || value_has_content(&bundle.characters)
        || value_has_content(&bundle.relationships)
        || value_has_content(&bundle.chapter_outlines)
}

/// LLM 可能返回字符串、对象或数组；字段全部以 Value 接收再由本模块归一。
#[derive(Debug, Default, Deserialize)]
struct RawBundle {
    #[serde(default)]
    logline: Option<String>,
    #[serde(default)]
    world: Option<Value>,
    #[serde(default)]
    story_outline: Option<Value>,
    #[serde(default)]
    characters: Option<Value>,
    #[serde(default)]
    relationships: Option<Value>,
    #[serde(default)]
    chapter_outlines: Option<Value>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
struct RawCharacter {
    #[serde(default)]
    name: String,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    personality: Option<String>,
    #[serde(default)]
    goals: Option<String>,
    #[serde(default)]
    appearance: Option<String>,
    #[serde(default)]
    gender: Option<String>,
    #[serde(default)]
    age: Option<i32>,
    #[serde(default)]
    emotional_core: Option<String>,
    #[serde(default)]
    emotional_trigger: Option<String>,
    #[serde(default)]
    emotional_wound: Option<String>,
    #[serde(default)]
    emotional_need: Option<String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
struct RawRelationship {
    #[serde(default)]
    source: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    relationship_type: String,
    #[serde(default)]
    emotional_bond: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    emotional_intensity: Option<f32>,
    #[serde(default)]
    reverse_emotional_bond: Option<String>,
    #[serde(default)]
    reverse_emotional_intensity: Option<f32>,
}

fn parse_characters(value: Option<&Value>) -> Vec<RawCharacter> {
    let Some(value) = value else {
        return Vec::new();
    };
    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .filter_map(|v| {
                if let Some(name) = v.as_str() {
                    let mut c = RawCharacter::default();
                    c.name = name.trim().to_string();
                    Some(c)
                } else {
                    serde_json::from_value::<RawCharacter>(v.clone()).ok()
                }
            })
            .collect();
    }
    if let Some(obj) = value.as_object() {
        return obj
            .values()
            .filter_map(|v| serde_json::from_value::<RawCharacter>(v.clone()).ok())
            .collect();
    }
    Vec::new()
}

fn parse_relationships(value: Option<&Value>) -> Vec<RawRelationship> {
    let Some(value) = value else {
        return Vec::new();
    };
    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .filter_map(|v| serde_json::from_value::<RawRelationship>(v.clone()).ok())
            .collect();
    }
    Vec::new()
}

fn parse_chapter_outlines(value: Option<&Value>) -> Vec<(i32, String)> {
    let Some(value) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(arr) = value.as_array() {
        for item in arr {
            let chapter = item
                .get("chapter")
                .or_else(|| item.get("index"))
                .or_else(|| item.get("chapter_number"))
                .and_then(value_as_i32);
            let summary = item
                .get("summary")
                .or_else(|| item.get("outline"))
                .or_else(|| item.get("content"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            if let (Some(ch), Some(s)) = (chapter, summary) {
                out.push((ch, s.to_string()));
            }
        }
    } else if let Some(obj) = value.as_object() {
        for (key, v) in obj {
            if let Some(ch) = key.trim().parse::<i32>().ok() {
                if let Some(s) = v.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                    out.push((ch, s.to_string()));
                }
            }
        }
    }
    out
}

fn value_as_i32(value: &Value) -> Option<i32> {
    match value {
        Value::Number(n) => n.as_i64().map(|v| v as i32),
        Value::String(s) => s.trim().parse::<i32>().ok(),
        _ => None,
    }
}

fn parse_world(value: Option<&Value>) -> Option<WorldParts> {
    let value = value?;
    if let Some(s) = value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        return Some(WorldParts {
            concept: s.to_string(),
            history: None,
            rules: Vec::new(),
            cultures: Vec::new(),
        });
    }
    let obj = value.as_object()?;
    let concept = obj
        .get("concept")
        .or_else(|| obj.get("name"))
        .or_else(|| obj.get("description"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let history = obj
        .get("history")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let mut rules = Vec::new();
    if let Some(arr) = obj.get("rules").and_then(Value::as_array) {
        for rule in arr {
            let Some(name) = rule
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                continue;
            };
            let description = rule
                .get("description")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let importance = rule
                .get("importance")
                .and_then(value_as_i32)
                .unwrap_or(8)
                .clamp(1, 10);
            rules.push(WorldRule {
                id: Uuid::new_v4().to_string(),
                name: name.to_string(),
                description,
                rule_type: RuleType::Custom,
                importance,
            });
        }
    }
    let mut cultures = Vec::new();
    if let Some(arr) = obj.get("cultures").and_then(Value::as_array) {
        for c in arr {
            if let Ok(culture) = serde_json::from_value::<Culture>(c.clone()) {
                if !culture.name.trim().is_empty() {
                    cultures.push(culture);
                }
            }
        }
    }
    Some(WorldParts {
        concept,
        history,
        rules,
        cultures,
    })
}

#[derive(Debug, Clone)]
struct WorldParts {
    concept: String,
    history: Option<String>,
    rules: Vec<WorldRule>,
    cultures: Vec<Culture>,
}

fn outline_text(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let mut text = normalize_outline(value).trim().to_string();
    if text.is_empty() {
        if let Some(s) = value.get("summary").and_then(Value::as_str) {
            text = s.trim().to_string();
        }
    }
    if let Some(next) = value
        .get("next_node")
        .or_else(|| value.get("next"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !text.contains(next) {
            text.push_str(&format!("\n\n【下一节点】{next}"));
        }
    }
    text
}

fn parse_scene_outline_summary(value: Option<&Value>) -> Vec<(i32, String)> {
    parse_chapter_outlines(value)
}

/// 落库：世界观/大纲/角色/关系/逐章梗概。失败条目只告警，不阻断。
pub(crate) fn persist_training_bundle(
    pool: &DbPool,
    story_id: &str,
    chapters: &[(usize, String)],
    bundle: &RawBundle,
) -> TrainingAssetSummary {
    let mut summary = TrainingAssetSummary::default();
    let prose: String = chapters
        .iter()
        .map(|(_, c)| c.as_str())
        .collect::<Vec<_>>()
        .join("\n");

    // 1) 故事线 logline
    if let Some(logline) = bundle
        .logline
        .as_deref()
        .map(str::trim)
        .filter(|s| s.chars().count() >= 10)
    {
        if StoryRepository::new(pool.clone())
            .update_logline(story_id, logline)
            .is_ok()
        {
            summary.logline = true;
        } else {
            summary.warnings.push("logline 写入失败".to_string());
        }
    }

    // 2) 世界观
    if let Some(world) = parse_world(bundle.world.as_ref()) {
        let repo = WorldBuildingRepository::new(pool.clone());
        match repo.get_by_story(story_id) {
            Ok(Some(existing)) => {
                if repo
                    .update(
                        &existing.id,
                        Some(&world.concept),
                        Some(&world.rules),
                        world.history.as_deref(),
                        Some(&world.cultures),
                    )
                    .is_ok()
                {
                    summary.world = true;
                } else {
                    summary.warnings.push("世界观更新失败".to_string());
                }
            }
            Ok(None) => match repo.create_with_source(
                story_id,
                &world.concept,
                Some("training_assets"),
                Some(true),
            ) {
                Ok(created) => {
                    if repo
                        .update(
                            &created.id,
                            None,
                            Some(&world.rules),
                            world.history.as_deref(),
                            Some(&world.cultures),
                        )
                        .is_ok()
                    {
                        summary.world = true;
                    } else {
                        summary.warnings.push("世界观规则写入失败".to_string());
                    }
                }
                Err(e) => summary.warnings.push(format!("世界观创建失败：{e}")),
            },
            Err(e) => summary.warnings.push(format!("读取世界观失败：{e}")),
        }
    }

    // 3) 故事大纲
    let outline = outline_text(bundle.story_outline.as_ref());
    if !outline.is_empty() {
        let repo = StoryOutlineRepository::new(pool.clone());
        match repo.get_by_story(story_id) {
            Ok(Some(existing)) if !existing.content.trim().is_empty() => {
                summary.story_outline = true;
            }
            Ok(_) => match repo.create(story_id, &outline, None, 3, Some(chapters.len() as i32)) {
                Ok(_) => summary.story_outline = true,
                Err(e) => summary.warnings.push(format!("故事大纲创建失败：{e}")),
            },
            Err(e) => summary.warnings.push(format!("读取故事大纲失败：{e}")),
        }
    } else {
        summary.warnings.push("模型未返回故事大纲".to_string());
    }

    // 4) 角色卡（只登记前文出现过的人名）
    let char_repo = CharacterRepository::new(pool.clone());
    let mut roster: Vec<String> = char_repo
        .get_by_story(story_id)
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.name)
        .collect();
    for raw in parse_characters(bundle.characters.as_ref()) {
        let name = raw.name.trim();
        if name.is_empty() || roster.iter().any(|n| n == name) {
            continue;
        }
        if !name_in_prose(name, &prose) {
            summary
                .warnings
                .push(format!("跳过前文未出现的人名「{name}」"));
            continue;
        }
        let req = CreateCharacterRequest {
            story_id: story_id.to_string(),
            name: name.to_string(),
            background: raw.background.filter(|s| !s.trim().is_empty()),
            personality: raw.personality.filter(|s| !s.trim().is_empty()),
            goals: raw.goals.filter(|s| !s.trim().is_empty()),
            appearance: raw.appearance.filter(|s| !s.trim().is_empty()),
            gender: raw.gender.filter(|s| !s.trim().is_empty()),
            age: raw.age,
            source: Some("training_assets".to_string()),
            is_auto_generated: Some(true),
            emotional_core: raw.emotional_core.filter(|s| !s.trim().is_empty()),
            emotional_trigger: raw.emotional_trigger.filter(|s| !s.trim().is_empty()),
            emotional_wound: raw.emotional_wound.filter(|s| !s.trim().is_empty()),
            emotional_need: raw.emotional_need.filter(|s| !s.trim().is_empty()),
        };
        match char_repo.create(req) {
            Ok(_) => {
                roster.push(name.to_string());
                summary.characters += 1;
            }
            Err(e) => summary
                .warnings
                .push(format!("角色「{name}」写入失败：{e}")),
        }
        if summary.characters >= MAX_CHARACTERS {
            break;
        }
    }

    // 5) 角色关系（source/target 必须已登记且在前文中出现）
    if let Ok(conn) = pool.get() {
        let roster_set: HashSet<&str> = roster.iter().map(|s| s.as_str()).collect();
        for rel in parse_relationships(bundle.relationships.as_ref()) {
            let source = rel.source.trim();
            let target = rel.target.trim();
            if source.is_empty()
                || target.is_empty()
                || !roster_set.contains(source)
                || !roster_set.contains(target)
            {
                continue;
            }
            if !name_in_prose(source, &prose) || !name_in_prose(target, &prose) {
                continue;
            }
            let seed = SeedRelationship {
                source: source.to_string(),
                target: target.to_string(),
                relationship_type: if rel.relationship_type.trim().is_empty() {
                    "关系".into()
                } else {
                    rel.relationship_type.trim().to_string()
                },
                emotional_bond: if rel.emotional_bond.trim().is_empty() {
                    "复杂".into()
                } else {
                    rel.emotional_bond.trim().to_string()
                },
                emotional_intensity: rel.emotional_intensity.unwrap_or(0.5).clamp(0.0, 1.0),
                reverse_emotional_bond: rel.reverse_emotional_bond.unwrap_or_default(),
                reverse_emotional_intensity: rel
                    .reverse_emotional_intensity
                    .unwrap_or(0.5)
                    .clamp(0.0, 1.0),
                description: rel.description.trim().to_string(),
            };
            summary.relationships += upsert_seed_relationship(&conn, story_id, &seed);
        }
    }

    // 6) 逐章梗概 → scenes.outline_content（供章纲生成与下一节点编译）
    let outlines = parse_scene_outline_summary(bundle.chapter_outlines.as_ref());
    if !outlines.is_empty() {
        let scene_repo = SceneRepository::new(pool.clone());
        let scenes = scene_repo.get_by_story(story_id).unwrap_or_default();
        let by_num: HashMap<i32, String> = outlines.iter().cloned().collect();
        let by_idx: Vec<String> = outlines.iter().map(|(_, s)| s.clone()).collect();
        for (i, scene) in scenes.iter().take(chapters.len()).enumerate() {
            let number = chapters.get(i).map(|(n, _)| *n as i32).unwrap_or(i as i32);
            let summary_text = by_num
                .get(&number)
                .cloned()
                .or_else(|| by_idx.get(i).cloned());
            let Some(summary_text) = summary_text.filter(|s| !s.trim().is_empty()) else {
                continue;
            };
            let outline_content = format!("本章梗概：{}", summary_text.trim());
            match scene_repo.update(
                &scene.id,
                &crate::db::repositories::SceneUpdate {
                    outline_content: Some(outline_content),
                    ..Default::default()
                },
            ) {
                Ok(_) => summary.chapter_outlines += 1,
                Err(e) => summary
                    .warnings
                    .push(format!("第{number}章梗概写入失败：{e}")),
            }
        }
    } else {
        summary.warnings.push("模型未返回逐章梗概".to_string());
    }

    // 防止资产重建把“模型没有返回”当作成功。
    if summary.characters == 0 {
        summary.warnings.push("资产重建未登记任何角色".to_string());
    }
    summary
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::db::{
        create_test_pool,
        dto::CreateStoryRequest,
        repositories::{SceneRepository, StoryOutlineRepository, WorldBuildingRepository},
    };

    fn sample_bundle() -> &'static str {
        r#"{
            "logline":"周先生在三理教的威逼利诱中反杀，并追索镇魇幕后主使。",
            "world":{"concept":"金城的江湖与三理教势力交织，术士以骨灰选灵争夺灵缘钱。","history":"三理教教主被枪毙后逃出大陆，金城分部借灵修班敛财。","rules":[{"name":"镇魇反噬","description":"桐人镇魇被解除时施术者精神先被吸纳","importance":9}]},
            "story_outline":{"core_conflict":"周先生被三理教逼入教与反制","turning_points":["周先生当众揭破鲁汉光的威逼利诱","闻路杰被送进局子"],"next_node":"周先生用化偶术反控乩童，逼公道师交出生骨灰选灵的真正主使"},
            "characters":[{"name":"周先生","background":"阴脉先生","personality":"谨慎懂行","goals":"活下去并破局","emotional_core":"谨慎","emotional_trigger":"被逼入教","emotional_wound":"妙姐失踪","emotional_need":"掌控局面"},{"name":"鲁汉光","background":"三理教大公保","personality":"圆滑强硬","goals":"逼周先生入教","emotional_core":"权力欲","emotional_trigger":"被拒绝","emotional_wound":"失去教中地位","emotional_need":"掌控"}],
            "relationships":[{"source":"周先生","target":"鲁汉光","relationship_type":"敌对","emotional_bond":"不信任","description":"鲁汉光用利益威逼周先生入教"}],
            "chapter_outlines":[{"chapter":97,"summary":"周先生拜神仙点空子，确认老神仙的抬轿规矩。"},{"chapter":98,"summary":"女老板切料子，老神仙试探周先生。"}]
        }"#
    }

    fn seed_story_with_scenes(pool: &DbPool) -> String {
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "训练资产".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let repo = SceneRepository::new(pool.clone());
        for i in 1..=2 {
            let scene = repo
                .create(&story.id, i, Some(&format!("第{}章", 96 + i)))
                .unwrap();
            repo.update(
                &scene.id,
                &crate::db::repositories::SceneUpdate {
                    content: Some(if i == 1 {
                        "周先生拜神仙点空子，鲁汉光盯着他。".into()
                    } else {
                        "女老板切料子，老神仙试探周先生。".into()
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        story.id
    }

    #[test]
    fn json_bundle_parses_and_persists_all_assets() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_with_scenes(&pool);
        let chapters = vec![
            (97usize, "周先生拜神仙点空子，鲁汉光盯着他。".to_string()),
            (98usize, "女老板切料子，老神仙试探周先生。".to_string()),
        ];
        let bundle = parse_bundle(sample_bundle()).expect("样本 JSON 应解析");
        let summary = persist_training_bundle(&pool, &story_id, &chapters, &bundle);
        assert_eq!(summary.characters, 2);
        assert_eq!(summary.relationships, 1);
        assert_eq!(summary.chapter_outlines, 2);
        assert!(summary.world);
        assert!(summary.story_outline);
        assert!(summary.logline);
        assert!(WorldBuildingRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap()
            .is_some());
        let outline = StoryOutlineRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap()
            .unwrap();
        assert!(outline.content.contains("核心冲突") || outline.content.contains("周先生"));
        assert!(outline.content.contains("下一节点"));
        let char_count: i64 = {
            let conn = pool.get().unwrap();
            conn.query_row(
                "SELECT COUNT(*) FROM characters WHERE story_id = ?1",
                params![story_id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(char_count, 2);
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        assert!(scenes[0]
            .outline_content
            .as_deref()
            .unwrap_or_default()
            .contains("拜神仙"));
        assert!(scenes[1]
            .outline_content
            .as_deref()
            .unwrap_or_default()
            .contains("切料子"));
    }

    #[test]
    fn labeled_prose_falls_back_to_refresh_payload() {
        let raw = "story_outline：周先生在三理教威逼利诱中反制，下一步逼问公道师。\
                   世界观：金城江湖与三理教灵修班交织。";
        let bundle = parse_bundle(raw).expect("键值散文应 salvage");
        assert!(bundle
            .story_outline
            .as_ref()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("周先生"));
        assert!(bundle.world.is_some());
    }

    #[test]
    fn asset_rebuild_prompt_mentions_grounded_schema_and_last_chapter_window() {
        let prompt = build_asset_rebuild_prompt(
            "阴脉先生",
            &[
                (105, "周先生在道观与鲁汉光对峙。".to_string()),
                (106, "桐人晃了晃栽在地，鲁汉光盯着他。".to_string()),
            ],
        );
        assert!(prompt.contains("小说：《阴脉先生》"));
        assert!(prompt.contains("chapter_outlines"));
        assert!(prompt.contains("【第 106 章】"));
        assert!(prompt.contains("最后一章上下文的结尾窗口"));
        assert!(prompt.contains("不得凭空发明人名"));
    }

    #[test]
    fn chapter_outlines_accept_map_and_keep_last_chapter_tail_window() {
        let value = serde_json::json!({"97": "拜神仙", "98": "切料子"});
        let parsed = parse_chapter_outlines(Some(&value));
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].0, 97);

        let long = "开篇".repeat(100) + &"结尾线索".repeat(100);
        let sampled = sample_chapter_text(&long, 200);
        assert!(sampled.contains("开篇"));
        assert!(sampled.contains("结尾线索"));
        assert!(sampled.contains("中间省略"));
    }

    #[test]
    fn parse_bundle_repairs_truncated_json() {
        // v0.98.3 真机：deepseek-v4 在 JSON 中途被截断（chapter_outlines
        // 未闭合）。
        let raw = r#"{"world":{"concept":"金城江湖","history":"三理教"},"characters":[{"name":"周先生","personality":"谨慎"}],"chapter_outlines":[{"chapter":97,"summary":"拜神仙"},{"chapter":98,"summary":"切料"#;
        let bundle = parse_bundle(raw).expect("截断 JSON 应能补全括号");
        assert!(bundle.world.is_some());
        assert!(bundle.characters.is_some());
        assert!(bundle_has_content(&bundle));
    }

    #[test]
    fn parse_bundle_rejects_empty_object() {
        // 思维链里随机提取到的 `{}` 不得被当成资产成功。
        assert!(parse_bundle("{}").is_none());
        assert!(parse_bundle("").is_none());
    }

    #[test]
    fn skips_ungrounded_names_in_relationships_and_characters() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_with_scenes(&pool);
        let chapters = vec![(97usize, "周先生拜神仙点空子，鲁汉光盯着他。".to_string())];
        let raw = r#"{
            "characters":[{"name":"陈默","personality":"坚毅"}],
            "relationships":[{"source":"陈默","target":"周先生","emotional_bond":"敌"}],
            "chapter_outlines":[{"chapter":97,"summary":"周先生拜神仙"}]
        }"#;
        let bundle = parse_bundle(raw).unwrap();
        let summary = persist_training_bundle(&pool, &story_id, &chapters, &bundle);
        assert_eq!(summary.characters, 0, "前文未出现的陈默不得登记");
        assert_eq!(summary.relationships, 0, "关系端点不在前文/角色表");
        assert!(summary
            .warnings
            .iter()
            .any(|w| w.contains("陈默") || w.contains("未登记任何角色")));
    }
}
