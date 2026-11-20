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

use chrono::Local;
use futures::stream::{self, StreamExt};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    agency::{
        continue_assets::strip_editor_markup,
        coordinator::{normalize_outline, parse_lenient, SeedRelationship},
        materialize::upsert_seed_relationship,
        prose_ground::name_in_prose,
        story_brief::{
            brief_from_summaries, BriefCastState, BriefChapter, BriefTerm, BriefThread, StoryBrief,
            StoryBriefRepository,
        },
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
/// v0.99.2：逐章摘要单章采样上限（短章全文；长章头尾窗口）。
const SUMMARY_CHAPTER_CHARS: usize = 2_600;
/// 逐章摘要输出上限。
const SUMMARY_OUTPUT_TOKENS: i32 = 1_200;
/// 逐章摘要并发（本地小模型可承受 2–4 路小 prompt）。
const SUMMARY_CONCURRENCY: usize = 3;
/// 聚合 brief 的输入上限（只喂结构化摘要，不再喂原文）。
const AGGREGATE_PROMPT_CHARS: usize = 12_000;
/// 聚合 brief 输出上限（推理模型留足 reasoning 余量）。
const AGGREGATE_OUTPUT_TOKENS: i32 = 8_192;
/// 摘要缓存版本：prompt 结构变化时改这个值即可失效旧缓存。
const BRIEF_PROMPT_VERSION: &str = "v1-2026-11-19";
/// v0.99.3：摘要缓存 TTL（源章节稳定；30 天后重新摘要，避免陈旧设定漂移）。
const BRIEF_CACHE_TTL_DAYS: i64 = 30;
/// 每章在资产 prompt 的最小/最大采样窗口。
const CHAPTER_SAMPLE_MIN: usize = 800;
const CHAPTER_SAMPLE_MAX: usize = 2_600;
/// 角色卡上限（按前文出场；避免把整本书的都塞进沙盒）。
const MAX_CHARACTERS: usize = 12;
/// 前情提要逐章事实上限（与 prompt 输出章数对齐；超出只保留最近 N 章）。
const MAX_BRIEF_CHAPTERS: usize = 30;

#[derive(Debug, Default, Clone)]
pub struct TrainingAssetSummary {
    pub logline: bool,
    pub world: bool,
    pub story_outline: bool,
    pub characters: usize,
    pub relationships: usize,
    pub chapter_outlines: usize,
    /// 前情提要（逐章事实 / 未结线索 / 设定白名单）
    pub brief_chapters: usize,
    pub brief_threads: usize,
    pub brief_terms: usize,
    pub brief_written: bool,
    /// v0.99.2：逐章摘要阶段诊断。
    pub chapter_summaries: usize,
    pub summary_cached: usize,
    pub summary_fallback: usize,
    pub aggregate_ok: bool,
    pub aggregate_fallback: bool,
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
        self.brief_chapters = self.brief_chapters.max(other.brief_chapters);
        self.brief_threads = self.brief_threads.max(other.brief_threads);
        self.brief_terms = self.brief_terms.max(other.brief_terms);
        self.brief_written |= other.brief_written;
        self.chapter_summaries = self.chapter_summaries.max(other.chapter_summaries);
        self.summary_cached += other.summary_cached;
        self.summary_fallback += other.summary_fallback;
        self.aggregate_ok |= other.aggregate_ok;
        self.aggregate_fallback |= other.aggregate_fallback;
        self.warnings.extend(other.warnings);
    }
}

/// v0.99.3：资产阶段逐章进度回调（done, total）。
pub type AssetProgress = std::sync::Arc<dyn Fn(usize, usize) + Send + Sync>;

/// 训练资产重建的主入口（v0.99.2：两阶段）。
///
/// Stage 1：逐章摘要（小 prompt、并发、可缓存）——单章失败只降级该章；
/// Stage 2：聚合 brief（只喂结构化摘要，不再喂原文）——失败走确定性聚合；
/// 最后复用原有 persist 链路（scene outline_content + story_briefs）。
/// 资产重建失败不阻断训练：summary/fallback 都会写确定性 brief。
pub async fn rebuild_from_context(
    pool: &DbPool,
    llm: &LlmService,
    story_id: &str,
    book_title: &str,
    chapters: &[(usize, String)],
    progress: Option<AssetProgress>,
) -> TrainingAssetSummary {
    let mut summary = TrainingAssetSummary::default();
    if chapters.is_empty() {
        summary.llm_error = Some("没有上下文章可重建资产".into());
        return summary;
    }

    // ── Stage 1：逐章摘要 ─────────────────────────────────────
    let summaries =
        summarize_chapters(pool, llm, book_title, chapters, &mut summary, progress).await;
    if summaries.is_empty() {
        summary.llm_error = Some("逐章摘要全部失败（无可用章节事实）".into());
        return summary;
    }

    // ── Stage 2：聚合 brief ───────────────────────────────────
    let aggregate = aggregate_brief(llm, book_title, &summaries).await;
    let mut bundle = match aggregate {
        Some(bundle) => {
            summary.aggregate_ok = true;
            bundle
        }
        None => {
            summary.aggregate_fallback = true;
            summary
                .warnings
                .push("聚合 brief 不可用，改用逐章摘要确定性拼装".to_string());
            RawBundle::default()
        }
    };
    // 逐章摘要才是事实权威；聚合输出不提供/覆盖 chapter_outlines。
    bundle.chapter_outlines = Some(brief_chapters_value(&summaries));
    summary.chapter_summaries = summaries.len();
    if summary.summary_fallback > 0 {
        summary.warnings.push(format!(
            "{} 章逐章摘要走确定性兜底",
            summary.summary_fallback
        ));
    }

    let pool_c = pool.clone();
    let sid = story_id.to_string();
    let chs = chapters.to_vec();
    let persisted =
        tokio::task::spawn_blocking(move || persist_training_bundle(&pool_c, &sid, &chs, &bundle))
            .await;
    match persisted {
        Ok(result) => {
            summary.merge(result);
            summary
        }
        Err(e) => {
            log::warn!("[ContinuationTraining] 资产重建落库失败: {e}");
            summary.llm_error = Some(format!("资产重建落库失败：{e}"));
            summary
        }
    }
}

async fn summarize_chapters(
    pool: &DbPool,
    llm: &LlmService,
    book_title: &str,
    chapters: &[(usize, String)],
    summary: &mut TrainingAssetSummary,
    progress: Option<AssetProgress>,
) -> Vec<BriefChapter> {
    let llm = llm.clone();
    let book_title = book_title.to_string();
    let total = chapters.len();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let progress = progress;
    let tasks = stream::iter(chapters.iter().cloned()).map(|(number, content)| {
        let llm = llm.clone();
        let book = book_title.clone();
        let pool = pool.clone();
        let done = done.clone();
        let progress = progress.clone();
        async move {
            let result = summarize_one(&pool, &llm, &book, number, content).await;
            let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if let Some(callback) = progress.as_ref() {
                callback(n, total);
            }
            result
        }
    });
    let mut results: Vec<(BriefChapter, bool, bool)> =
        tasks.buffer_unordered(SUMMARY_CONCURRENCY).collect().await;
    results.sort_by_key(|(brief, _, _)| brief.chapter);
    let mut out = Vec::new();
    for (brief, fallback, cached) in results {
        if cached {
            summary.summary_cached += 1;
        }
        if fallback {
            summary.summary_fallback += 1;
        }
        out.push(brief);
    }
    out.truncate(MAX_BRIEF_CHAPTERS);
    out
}

/// 单章摘要：优先缓存；LLM 两次失败后走确定性事件摘要。
async fn summarize_one(
    pool: &DbPool,
    llm: &LlmService,
    book_title: &str,
    number: usize,
    content: String,
) -> (BriefChapter, bool, bool) {
    let chapter = number as i32;
    if let Some(cached) = brief_cache_read(pool, book_title, chapter) {
        return (cached, false, true);
    }
    let prompt = build_chapter_summary_prompt(book_title, number, &content);
    for attempt in 0..=1 {
        let req = if attempt == 0 {
            prompt.clone()
        } else {
            format!(
                "上一次输出为空、被截断或无法解析。这次严格只输出一个完整、合法的 JSON \
                 对象（不要 markdown、不要解释文字、不要代码围栏），字段可精简但必须闭合所有括号。\n\n{prompt}"
            )
        };
        match llm
            .generate_for_task_with_format(
                TaskType::Analysis,
                req,
                Some(SUMMARY_OUTPUT_TOKENS),
                Some(0.2),
                Some("continuation_training_chapter_summary"),
                Some(ResponseFormat::JsonObject),
            )
            .await
        {
            Ok(resp) => {
                if let Some(brief) = parse_chapter_summary(&resp.content, chapter) {
                    let _ = brief_cache_write(pool, book_title, &brief);
                    return (brief, false, false);
                }
                log::warn!(
                    "[ContinuationTraining] 第 {} 章摘要输出无法解析（第 {} 次）: raw={}",
                    number,
                    attempt + 1,
                    preview_chars(&resp.content, 180)
                );
            }
            Err(e) => {
                log::warn!(
                    "[ContinuationTraining] 第 {} 章摘要 LLM 调用失败（第 {} 次）: {}",
                    number,
                    attempt + 1,
                    e
                );
            }
        }
    }
    let summary = deterministic_chapter_summary(number, &content);
    (
        BriefChapter {
            chapter,
            summary,
            ..Default::default()
        },
        true,
        false,
    )
}

fn build_chapter_summary_prompt(book_title: &str, number: usize, content: &str) -> String {
    let text = sample_chapter_text(content, SUMMARY_CHAPTER_CHARS);
    format!(
        r#"你是小说逐章摘要师。只依据下面这一章的真实正文，输出一个 JSON 对象；不得发明人名、地名、术语或与原文冲突的设定。不确定的字段留空或 []。
{{
  "chapter": {number},
  "summary": "本章已发生的事件与结果（80-150字；只写事实，不写规划）",
  "location": "本章主要地点",
  "cast": ["本章在场人名"],
  "result": "本章结束时的状态变化",
  "open_thread": "本章留下的未决问题（没有就留空）",
  "turning_point": "本章最大的转折（没有就留空）",
  "key_terms": ["本章真实出现的术法/门派/器物/地名/组织名，最多6个"]
}}
约束：
- 只能使用正文中实际出现的人名、地名、术语；不得把书名/空简介当情节。
- 只写已经发生的事实与结果，不写下一拍规划、场景编号或方法论步骤。
- key_terms 只列正文真实出现过的名词；不确定就 []。
- 只输出 JSON，不要 markdown/解释文字/代码围栏。

小说：《{book_title}》
【第 {number} 章正文（只用于归纳，不要续写）】
{text}"#
    )
}

fn parse_chapter_summary(raw: &str, chapter: i32) -> Option<BriefChapter> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let value = parse_lenient::<Value>(trimmed).or_else(|| {
        crate::utils::json::complete_truncated_json(trimmed)
            .and_then(|fixed| serde_json::from_str::<Value>(&fixed).ok())
    })?;
    let mut briefs = parse_chapter_briefs(Some(&value));
    briefs.retain(|b| b.chapter == chapter);
    briefs.into_iter().next()
}

fn brief_chapters_value(chapters: &[BriefChapter]) -> Value {
    Value::Array(
        chapters
            .iter()
            .map(|b| {
                serde_json::json!({
                    "chapter": b.chapter,
                    "summary": b.summary,
                    "location": b.location,
                    "cast": b.cast,
                    "result": b.result,
                    "open_thread": b.open_thread,
                    "turning_point": b.turning_point,
                    "key_terms": b.key_terms,
                })
            })
            .collect(),
    )
}

fn build_brief_aggregate_prompt(book_title: &str, summaries: &[BriefChapter]) -> String {
    let mut lines = String::new();
    for b in summaries.iter().take(MAX_BRIEF_CHAPTERS) {
        lines.push_str(&format!(
            "第{}章：{}｜地点：{}｜在场：{}｜结果：{}｜未决：{}｜转折：{}｜术语：{}\n",
            b.chapter,
            b.summary,
            b.location,
            b.cast.join("、"),
            b.result,
            b.open_thread,
            b.turning_point,
            b.key_terms.join("、")
        ));
    }
    let joined = lines
        .chars()
        .take(AGGREGATE_PROMPT_CHARS)
        .collect::<String>();
    format!(
        r#"你是小说资产聚合师。下面已有逐章事实摘要，只聚合，不新增事实；不得发明人名、地名、组织名或与原文冲突的设定。
请输出一个 JSON 对象（不要 markdown、不要解释文字、不要代码围栏），字段按顺序：
{{
  "logline": "一句话故事线（<=80字）",
  "story_outline": {{"core_conflict": "当前核心冲突", "turning_points": ["已发生的转折"], "next_node": "紧接最后一章之后、尚未写过的下一推进方向"}},
  "open_threads": [{{"id": "t1", "description": "仍未解决的事件/目标/危险", "status": "open", "next_expected": "下一章最可能怎么推进"}}],
  "canon_terms": [{{"term": "前文真实出现的术法/门派/器物/地名/组织名", "type": "术法/组织/地点/器物/身份", "note": "一句话含义"}}],
  "world": {{"concept": "世界概念与冲突土壤（150-400字）", "history": "历史/势力背景（80-250字）", "rules": [{{"name": "规则名", "description": "规则说明", "importance": 8}}]}},
  "characters": [{{"name": "前文出现的人名", "background": "来历", "personality": "性格", "goals": "目标"}}],
  "relationships": [{{"source": "角色A", "target": "角色B", "relationship_type": "关系", "emotional_bond": "情感纽带", "description": "关系描述"}}]
}}
约束：
- open_threads 3-8 条；canon_terms 8-20 条；characters 只写摘要中出现过的人名。
- story_outline.next_node 不得与已发生事件重复；不得换主角/地点/委托。
- 不确定的数组用 []；不要输出大段解释。

小说：《{book_title}》
逐章摘要（只用于聚合，不要续写）：
{joined}"#
    )
}

async fn aggregate_brief(
    llm: &LlmService,
    book_title: &str,
    summaries: &[BriefChapter],
) -> Option<RawBundle> {
    if summaries.is_empty() {
        return None;
    }
    let prompt = build_brief_aggregate_prompt(book_title, summaries);
    for attempt in 0..=1 {
        let req = if attempt == 0 {
            prompt.clone()
        } else {
            format!(
                "上一次输出为空、被截断或无法解析。这次严格只输出一个完整、合法的 JSON \
                 对象（不要 markdown、不要解释文字、不要代码围栏），字段可精简但必须闭合所有括号。\n\n{prompt}"
            )
        };
        match llm
            .generate_for_task_with_format(
                TaskType::Analysis,
                req,
                Some(AGGREGATE_OUTPUT_TOKENS),
                Some(0.2),
                Some("continuation_training_brief_aggregate"),
                Some(ResponseFormat::JsonObject),
            )
            .await
        {
            Ok(resp) => {
                if let Some(bundle) = parse_bundle(&resp.content) {
                    if bundle_has_content(&bundle) {
                        return Some(bundle);
                    }
                }
                log::warn!(
                    "[ContinuationTraining] 聚合 brief 输出无法解析（第 {} 次）: raw={}",
                    attempt + 1,
                    preview_chars(&resp.content, 180)
                );
            }
            Err(e) => {
                log::warn!(
                    "[ContinuationTraining] 聚合 brief LLM 调用失败（第 {} 次）: {}",
                    attempt + 1,
                    e
                );
            }
        }
    }
    None
}

fn brief_cache_read(pool: &DbPool, book_ref: &str, chapter: i32) -> Option<BriefChapter> {
    let conn = pool.get().ok()?;
    let cutoff = (Local::now() - chrono::Duration::days(BRIEF_CACHE_TTL_DAYS.max(1))).to_rfc3339();
    let raw: Option<String> = conn
        .query_row(
            "SELECT summary_json FROM continuation_training_chapter_briefs \
             WHERE book_ref = ?1 AND chapter_index = ?2 AND prompt_version = ?3 \
             AND fetched_at >= ?4",
            rusqlite::params![book_ref, chapter, BRIEF_PROMPT_VERSION, cutoff],
            |r| r.get(0),
        )
        .optional()
        .ok()
        .flatten();
    raw.and_then(|json| serde_json::from_str::<BriefChapter>(&json).ok())
}

fn brief_cache_write(
    pool: &DbPool,
    book_ref: &str,
    brief: &BriefChapter,
) -> Result<(), rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let json = serde_json::to_string(brief)
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO continuation_training_chapter_briefs \
         (book_ref, chapter_index, prompt_version, summary_json, fetched_at) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(book_ref, chapter_index, prompt_version) DO UPDATE SET \
            summary_json = excluded.summary_json, fetched_at = excluded.fetched_at",
        rusqlite::params![book_ref, brief.chapter, BRIEF_PROMPT_VERSION, json, now],
    )?;
    Ok(())
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
请重建故事大纲、逐章事实、世界观、角色、关系、未结线索与设定白名单。只输出一个 JSON 对象（不要 markdown、不要解释文字、不要代码围栏），**严格按以下字段顺序输出**（越靠前越重要，后段被截断也不得影响前段）：
{{
  "logline": "一句话故事线（<=80字）",
  "story_outline": {{"core_conflict": "当前核心冲突", "turning_points": ["前文已经发生的转折（只写已发生的事实）"], "next_node": "紧接最后一章之后、尚未写过的下一推进方向（必须落在本拍现有角色身上）"}},
  "chapter_outlines": [{{"chapter": 97, "summary": "本章已发生的事件与结果（60-150字）", "location": "本章主要地点", "cast": ["本章在场人名"], "result": "本章结束时的状态变化", "open_thread": "本章留下的未决问题（没有就留空）"}}],
  "open_threads": [{{"id": "t1", "description": "仍未解决的事件/目标/危险", "introduced_chapter": 97, "status": "open", "next_expected": "下一章最可能怎么推进"}}],
  "canon_terms": [{{"term": "前文已出现的术法/门派/器物/地名/组织名", "type": "术法/组织/地点/器物/身份", "note": "一句话含义"}}],
  "world": {{"concept": "世界概念与冲突土壤（150-400字）", "history": "历史/势力背景（80-250字）", "rules": [{{"name": "规则名", "description": "规则说明", "importance": 8}}]}},
  "characters": [{{"name": "前文出现的人名", "background": "来历", "personality": "性格", "goals": "目标", "emotional_core": "情感内核", "emotional_trigger": "情感触发", "emotional_wound": "情感创伤", "emotional_need": "情感需求"}}],
  "relationships": [{{"source": "角色A", "target": "角色B", "relationship_type": "师徒/敌对/亲缘等", "emotional_bond": "源→目标情感纽带", "description": "关系描述"}}]
}}
约束：
- chapter_outlines 必须覆盖输入的每一章，按原书章号填写；summary/result 只写事实与结果，不写规划；open_thread 只写正文已经留下、尚未解决的事。
- open_threads 写 3-8 条，按重要性排序；每条的 description 必须能在前文中找到依据，next_expected 不得另起新主角/新地点/新委托。
- canon_terms 写 8-20 条，只列前文真实出现过的名词；不得把沙盒/模型自造的名词写进白名单。
- story_outline.next_node 不得与已发生事件重复；不得换主角、换地点或引入新委托。
- 各字段保持精简；characters 只写前文实际出现的人名，最多 8 人；不确定的数组用 []，不要输出大段解释。
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
        open_threads: None,
        canon_terms: None,
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
        || value_has_content(&bundle.open_threads)
        || value_has_content(&bundle.canon_terms)
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
    #[serde(default)]
    open_threads: Option<Value>,
    #[serde(default)]
    canon_terms: Option<Value>,
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

/// 逐章事实（v0.99.1）：在旧 `(章号, 梗概)` 基础上接收 location/cast/result/
/// open_thread，供《前情提要》渲染；旧模型只返回 summary 时也能兼容。
fn parse_chapter_briefs(value: Option<&Value>) -> Vec<BriefChapter> {
    let Some(value) = value else {
        return Vec::new();
    };
    // v0.99.2：逐章摘要 LLM 返回单个对象 `{chapter,summary,...}`；旧解析只认
    // 数组或「章号 key → 字符串」映射，会把它当空对象丢掉。
    if value
        .as_object()
        .map(|obj| {
            obj.get("chapter").is_some()
                || obj.get("index").is_some()
                || obj.get("chapter_number").is_some()
        })
        .unwrap_or(false)
    {
        return parse_chapter_briefs(Some(&Value::Array(vec![value.clone()])));
    }
    let mut out: Vec<BriefChapter> = Vec::new();
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
                .unwrap_or("");
            let Some(chapter) = chapter else {
                continue;
            };
            if let Some(b) = normalize_brief_chapter(BriefChapter {
                chapter,
                summary: summary.to_string(),
                location: item
                    .get("location")
                    .or_else(|| item.get("setting_location"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                cast: item
                    .get("cast")
                    .or_else(|| item.get("characters"))
                    .and_then(string_list)
                    .unwrap_or_default(),
                result: item
                    .get("result")
                    .or_else(|| item.get("state_change"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                open_thread: item
                    .get("open_thread")
                    .or_else(|| item.get("hook"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                turning_point: item
                    .get("turning_point")
                    .or_else(|| item.get("twist"))
                    .or_else(|| item.get("key_event"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                key_terms: item
                    .get("key_terms")
                    .or_else(|| item.get("terms"))
                    .and_then(string_list)
                    .unwrap_or_default(),
            }) {
                out.push(b);
            }
        }
    } else if let Some(obj) = value.as_object() {
        for (key, v) in obj {
            if let Some(ch) = key.trim().parse::<i32>().ok() {
                if let Some(s) = v.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                    if let Some(b) = normalize_brief_chapter(BriefChapter {
                        chapter: ch,
                        summary: s.to_string(),
                        ..Default::default()
                    }) {
                        out.push(b);
                    }
                }
            }
        }
    }
    out.sort_by_key(|b| b.chapter);
    out.dedup_by_key(|b| b.chapter);
    out
}

fn normalize_brief_chapter(mut b: BriefChapter) -> Option<BriefChapter> {
    if b.chapter <= 0 {
        return None;
    }
    if b.summary.trim().is_empty() {
        b.summary = b.result.trim().to_string();
    }
    if b.summary.trim().is_empty() && b.open_thread.trim().is_empty() {
        return None;
    }
    b.summary = b.summary.trim().to_string();
    b.location = b.location.trim().to_string();
    b.result = b.result.trim().to_string();
    b.open_thread = b.open_thread.trim().to_string();
    b.turning_point = b.turning_point.trim().to_string();
    b.cast.retain(|n| !n.trim().is_empty());
    b.key_terms
        .retain(|n| !n.trim().is_empty() && n.trim().chars().count() >= 2);
    Some(b)
}

fn parse_chapter_outlines(value: Option<&Value>) -> Vec<(i32, String)> {
    parse_chapter_briefs(value)
        .into_iter()
        .map(|b| (b.chapter, b.summary))
        .filter(|(_, s)| !s.trim().is_empty())
        .collect()
}

fn string_list(value: &Value) -> Option<Vec<String>> {
    let arr = value.as_array()?;
    let out: Vec<String> = arr
        .iter()
        .filter_map(|v| {
            v.as_str()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    v.get("name")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                })
        })
        .collect();
    (!out.is_empty()).then_some(out)
}

fn parse_threads(value: Option<&Value>) -> Vec<BriefThread> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };
    let mut out: Vec<BriefThread> = Vec::new();
    for (idx, item) in arr.iter().enumerate() {
        if let Some(s) = item.as_str().map(str::trim).filter(|s| !s.is_empty()) {
            out.push(BriefThread {
                id: format!("t{}", idx + 1),
                description: s.to_string(),
                status: "open".to_string(),
                next_expected: String::new(),
            });
            continue;
        }
        let description = item
            .get("description")
            .or_else(|| item.get("thread"))
            .or_else(|| item.get("summary"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(description) = description else {
            continue;
        };
        let id = item
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("t{}", idx + 1));
        out.push(BriefThread {
            id,
            description: description.to_string(),
            status: item
                .get("status")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("open")
                .to_string(),
            next_expected: item
                .get("next_expected")
                .or_else(|| item.get("next"))
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
                .to_string(),
        });
    }
    out.truncate(12);
    out
}

fn parse_terms(value: Option<&Value>) -> Vec<BriefTerm> {
    let Some(value) = value else {
        return Vec::new();
    };
    let Some(arr) = value.as_array() else {
        return Vec::new();
    };
    let mut out: Vec<BriefTerm> = Vec::new();
    for item in arr {
        let term = item
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                item.get("term")
                    .or_else(|| item.get("name"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            });
        let Some(term) = term else {
            continue;
        };
        if out.iter().any(|t| t.term == term) {
            continue;
        }
        out.push(BriefTerm {
            term,
            kind: item
                .get("type")
                .or_else(|| item.get("kind"))
                .or_else(|| item.get("category"))
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
                .to_string(),
            note: item
                .get("note")
                .or_else(|| item.get("description"))
                .and_then(Value::as_str)
                .map(str::trim)
                .unwrap_or("")
                .to_string(),
        });
    }
    out.truncate(24);
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

/// 逐章梗概的确定性兜底：取每章正文开头/结尾窗口，保证章纲规划与
/// 下一节点编译至少能看到「这一章发生过什么」。不冒充 LLM 摘要，报告里
/// 通过 warning 标明是兜底。
pub(crate) fn deterministic_chapter_summaries(chapters: &[(usize, String)]) -> Vec<(i32, String)> {
    chapters
        .iter()
        .map(|(number, content)| {
            (
                *number as i32,
                deterministic_chapter_summary(*number, content),
            )
        })
        .filter(|(_, summary)| !summary.trim().is_empty())
        .collect()
}

/// v0.99.2：确定性逐章摘要——不再只取首尾 70 字。切句后按「冲突/转折动词、
/// 引号对白、专名/地名/器物」打分，保留开头 1 句 + 中段得分最高 3 句 + 结尾
/// 1 句；这样第 142 章 81% 处的「邵卫江/福摩萨夜总会」能进入 brief。
pub(crate) fn deterministic_chapter_summary(_number: usize, content: &str) -> String {
    const SUMMARY_CAP: usize = 260;
    const CONFLICT_MARKERS: [&str; 24] = [
        "对峙", "逼", "拒绝", "发现", "答应", "交换", "动手", "逃", "追", "审", "藏", "夺", "算计",
        "杀", "换", "进", "退", "拦住", "按住", "抓住", "约", "见面", "合作", "警告",
    ];
    // v0.99.7：缓存原文首行常是「第136章 标题」，兜底摘要不能把它当事实。
    let plain =
        crate::utils::text::TextUtils::strip_leading_chapter_heading(&strip_editor_markup(content));
    let chars: Vec<char> = plain.chars().collect();
    if chars.len() <= 400 {
        return plain.trim().chars().take(SUMMARY_CAP).collect::<String>();
    }
    let sentences = split_sentences(&plain);
    if sentences.is_empty() {
        let head: String = plain.trim().chars().take(120).collect();
        let tail_start = chars.len().saturating_sub(120);
        let tail: String = chars[tail_start..].iter().collect();
        return format!("{head}…{tail}")
            .trim()
            .chars()
            .take(SUMMARY_CAP)
            .collect::<String>();
    }
    let score = |s: &str| -> i32 {
        let mut score = 0i32;
        if CONFLICT_MARKERS.iter().any(|m| s.contains(m)) {
            score += 4;
        }
        if s.contains('“') || s.contains('「') || s.contains('『') {
            score += 3;
        }
        if s.contains("第") || s.chars().any(|c| c.is_ascii_digit()) {
            score += 1;
        }
        let len = s.chars().count();
        if (15..=90).contains(&len) {
            score += 1;
        }
        score
    };
    let mut middle: Vec<(usize, i32)> = sentences
        .iter()
        .enumerate()
        .skip(1)
        .take(sentences.len().saturating_sub(2))
        .map(|(i, s)| (i, score(s)))
        .filter(|(_, s)| *s > 1)
        .collect();
    middle.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
    let mut picked: Vec<usize> = middle.iter().take(3).map(|(i, _)| *i).collect();
    picked.sort_unstable();
    if picked.is_empty() {
        picked.push(sentences.len() / 2);
    }
    let mut summary = String::new();
    summary.push_str(sentences.first().unwrap().trim());
    for i in picked {
        summary.push('…');
        summary.push_str(sentences[i].trim());
    }
    if sentences.len() > 1 {
        summary.push('…');
        summary.push_str(sentences.last().unwrap().trim());
    }
    summary.chars().take(SUMMARY_CAP).collect()
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for c in text.chars() {
        if matches!(c, '。' | '！' | '？' | '；' | '\n' | '\r') {
            let trimmed = current.trim();
            if trimmed.chars().count() >= 6 {
                out.push(trimmed.to_string());
            }
            current.clear();
        } else {
            current.push(c);
        }
    }
    let trimmed = current.trim();
    if trimmed.chars().count() >= 6 {
        out.push(trimmed.to_string());
    }
    out
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
    // v0.99.7：逐章事实提前构建——世界观/角色的确定性兜底都要用它。
    let brief_chapters = build_brief_chapters(chapters, bundle);

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

    // 2) 世界观（模型缺 world 时用逐章事实做确定性兜底，至少给 writer 一个
    // 可核对的场景/主线概念；规则表保持为空，不伪造术法体系）
    let world_parts = parse_world(bundle.world.as_ref())
        .or_else(|| deterministic_world_from_brief(&brief_chapters));
    if world_parts.is_some() && bundle.world.as_ref().map(|v| v.is_null()).unwrap_or(true) {
        summary
            .warnings
            .push("世界观模型未返回，已用逐章事实确定性兜底".to_string());
    }
    if let Some(world) = world_parts {
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
    } else {
        summary
            .warnings
            .push("模型未返回世界观，确定性兜底也不可用".to_string());
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
    let mut raw_characters = parse_characters(bundle.characters.as_ref());
    if raw_characters.is_empty() {
        // v0.99.7：聚合模型漏 characters（真机 v0.99.6 角色 0/关系 0），
        // 用逐章摘要的 cast 确定性兜底——名字必须在前文出现，绝不发明。
        raw_characters = deterministic_characters_from_cast(&brief_chapters, &prose);
        if !raw_characters.is_empty() {
            summary.warnings.push(format!(
                "角色模型未返回，已用逐章 cast 确定性兜底 {} 人",
                raw_characters.len()
            ));
        }
    }
    for raw in raw_characters {
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

    // 6) 逐章事实 → scenes.outline_content（供章纲生成与下一节点编译）
    // v0.99.0：模型 JSON 被截断时 chapter_outlines 往往最先丢（真机 1118
    // 资产重建 0 章梗概 → 章纲规划看不到 135–143 章的进度，续写只能围着
    // 末章结尾打转）。缺的章用原文首尾窗口确定性兜底，不冒充 LLM 摘要。
    // v0.99.1：同一份逐章事实同时进入《前情提要》，writer/planner 全量可见。
    let llm_outline_count = parse_chapter_briefs(bundle.chapter_outlines.as_ref()).len();
    if llm_outline_count == 0 {
        summary
            .warnings
            .push("模型未返回逐章梗概（已用原文首尾窗口兜底）".to_string());
    }
    {
        let scene_repo = SceneRepository::new(pool.clone());
        let scenes = scene_repo.get_by_story(story_id).unwrap_or_default();
        let by_num: HashMap<i32, BriefChapter> = brief_chapters
            .iter()
            .cloned()
            .map(|b| (b.chapter, b))
            .collect();
        for (i, scene) in scenes.iter().take(chapters.len()).enumerate() {
            let number = chapters.get(i).map(|(n, _)| *n as i32).unwrap_or(i as i32);
            let Some(b) = by_num.get(&number) else {
                continue;
            };
            let Some(summary_text) = [b.summary.trim(), b.result.trim()]
                .into_iter()
                .find(|s| !s.is_empty())
            else {
                continue;
            };
            let outline_content = format!("本章梗概：{summary_text}");
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
    }

    // 7) 《前情提要》（v0.99.1）：逐章事实 + 未结线索 + 人物当前态 + 设定白名单
    let mut threads = parse_threads(bundle.open_threads.as_ref());
    if threads.is_empty() {
        for b in &brief_chapters {
            let open = b.open_thread.trim();
            if !open.is_empty() && !threads.iter().any(|t| t.description == open) {
                threads.push(BriefThread {
                    id: format!("thread-{}", b.chapter),
                    description: open.to_string(),
                    status: "open".to_string(),
                    next_expected: String::new(),
                });
            }
        }
    }
    if threads.is_empty() {
        // 兜底：至少留一条「承接末章未决事件」的主线，避免 writer 完全没有
        // 未结线索可推进。
        if let Some(last) = brief_chapters.last() {
            threads.push(BriefThread {
                id: "main".to_string(),
                description: format!(
                    "紧接第{}章的未决事件继续推进：{}",
                    last.chapter,
                    last.summary.chars().take(120).collect::<String>()
                ),
                status: "open".to_string(),
                next_expected: "承接上一章结尾现场".to_string(),
            });
        }
    }
    if let Some(next) = bundle
        .story_outline
        .as_ref()
        .and_then(|v| v.get("next_node"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if !threads.iter().any(|t| t.description.contains(next)) {
            threads.push(BriefThread {
                id: "main-next".to_string(),
                description: format!("下一节点：{next}"),
                status: "open".to_string(),
                next_expected: next.to_string(),
            });
        }
    }
    threads.truncate(12);

    let mut terms = parse_terms(bundle.canon_terms.as_ref());
    terms.retain(|t| !t.term.trim().is_empty() && prose.contains(t.term.trim()));
    // v0.99.2：逐章摘要的 key_terms 是资产失败时最后的 grounded 白名单来源。
    for b in &brief_chapters {
        for term in &b.key_terms {
            let term = term.trim();
            if term.chars().count() < 2
                || !prose.contains(term)
                || terms.iter().any(|t| t.term == term)
            {
                continue;
            }
            terms.push(BriefTerm {
                term: term.to_string(),
                kind: "设定".to_string(),
                note: format!("第{}章正文出现", b.chapter),
            });
        }
    }
    if terms.is_empty() {
        // 兜底：角色名 + 世界观规则名（都必须是前文出现过的词）
        for name in &roster {
            if prose.contains(name.as_str()) {
                terms.push(BriefTerm {
                    term: name.clone(),
                    kind: "人物".to_string(),
                    note: "前文已出场".to_string(),
                });
            }
        }
        if let Ok(Some(world)) = WorldBuildingRepository::new(pool.clone()).get_by_story(story_id) {
            for rule in world.rules.iter().take(8) {
                if prose.contains(rule.name.as_str()) {
                    terms.push(BriefTerm {
                        term: rule.name.clone(),
                        kind: "规则".to_string(),
                        note: rule.description.clone().unwrap_or_default(),
                    });
                }
            }
        }
    }
    terms.sort_by(|a, b| a.term.cmp(&b.term));
    terms.dedup_by(|a, b| a.term == b.term);
    terms.truncate(24);

    let mut states: Vec<BriefCastState> = {
        let chars = CharacterRepository::new(pool.clone())
            .get_by_story(story_id)
            .unwrap_or_default();
        chars
            .into_iter()
            .filter(|c| c.source.as_deref() == Some("training_assets"))
            .take(12)
            .map(|c| BriefCastState {
                name: c.name,
                role: c.background.unwrap_or_default(),
                location: String::new(),
                goal: c.goals.unwrap_or_default(),
                knows: Vec::new(),
            })
            .collect()
    };
    if states.is_empty() {
        // v0.99.2：资产重建 0 角色时，用逐章摘要的 cast 做 grounded
        // 人物态兜底。
        let mut seen = HashSet::new();
        for b in &brief_chapters {
            for name in &b.cast {
                let name = name.trim();
                if name.chars().count() < 2
                    || !prose.contains(name)
                    || !seen.insert(name.to_string())
                {
                    continue;
                }
                states.push(BriefCastState {
                    name: name.to_string(),
                    role: "前文已登场".to_string(),
                    location: String::new(),
                    goal: String::new(),
                    knows: Vec::new(),
                });
            }
        }
    }
    states.sort_by(|a, b| a.name.cmp(&b.name));
    states.truncate(12);
    let cast_states = states;

    let arc_hint = bundle
        .logline
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            bundle
                .story_outline
                .as_ref()
                .and_then(|v| v.get("core_conflict"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_default();
    let mut brief = brief_from_summaries(brief_chapters, &arc_hint, threads, cast_states, terms);
    brief.source = "training_assets".to_string();
    brief.updated_at = chrono::Local::now().to_rfc3339();
    match StoryBriefRepository::new(pool.clone()).upsert(story_id, &brief) {
        Ok(_) => {
            summary.brief_written = true;
            summary.brief_chapters = brief.chapters.len();
            summary.brief_threads = brief.threads.len();
            summary.brief_terms = brief.canon_terms.len();
        }
        Err(e) => summary.warnings.push(format!("前情提要落库失败：{e}")),
    }

    // 防止资产重建把“模型没有返回”当作成功。
    if summary.characters == 0 {
        summary.warnings.push("资产重建未登记任何角色".to_string());
    }
    summary
}

/// 合并 LLM 逐章事实与确定性首尾兜底：每章必须有一行，缺的章用兜底补。
fn build_brief_chapters(chapters: &[(usize, String)], bundle: &RawBundle) -> Vec<BriefChapter> {
    let llm = parse_chapter_briefs(bundle.chapter_outlines.as_ref());
    let by_num: HashMap<i32, BriefChapter> = llm.iter().cloned().map(|b| (b.chapter, b)).collect();
    let by_idx: Vec<BriefChapter> = llm;
    let fallback = deterministic_chapter_summaries(chapters);
    let mut out: Vec<BriefChapter> = Vec::new();
    for (i, (number, _)) in chapters.iter().take(MAX_BRIEF_CHAPTERS).enumerate() {
        let chapter = *number as i32;
        let mut b = by_num
            .get(&chapter)
            .cloned()
            .or_else(|| by_idx.get(i).cloned())
            .unwrap_or_default();
        b.chapter = chapter;
        if b.summary.trim().is_empty() {
            b.summary = fallback.get(i).map(|(_, s)| s.clone()).unwrap_or_default();
        }
        if let Some(b) = normalize_brief_chapter(b) {
            out.push(b);
        }
    }
    out.sort_by_key(|b| b.chapter);
    out
}

/// v0.99.7：世界观确定性兜底。聚合模型漏 world 时，用逐章事实里的地点/
/// 主线摘要给出可核对的概念与历史；规则表保持为空（不伪造术法体系）。
fn deterministic_world_from_brief(chapters: &[BriefChapter]) -> Option<WorldParts> {
    let first = chapters.iter().find(|b| !b.summary.trim().is_empty())?;
    let mut locations: Vec<String> = Vec::new();
    for b in chapters {
        let loc = b.location.trim();
        if !loc.is_empty() && !locations.iter().any(|l| l == loc) {
            locations.push(loc.to_string());
        }
        if locations.len() >= 6 {
            break;
        }
    }
    let brief = first.summary.trim();
    let mut concept = if locations.is_empty() {
        brief.chars().take(260).collect::<String>()
    } else {
        format!(
            "前文主要场景：{}。主线：{}",
            locations.join("、"),
            brief.chars().take(200).collect::<String>()
        )
    };
    concept = concept.chars().take(400).collect();
    let last = chapters
        .iter()
        .rev()
        .find(|b| !b.summary.trim().is_empty())
        .unwrap_or(first);
    let history = Some(format!(
        "{} 章事实：{} … {}",
        chapters.len(),
        brief.chars().take(110).collect::<String>(),
        last.summary.trim().chars().take(110).collect::<String>()
    ));
    Some(WorldParts {
        concept,
        history,
        rules: Vec::new(),
        cultures: Vec::new(),
    })
}

/// v0.99.7：角色确定性兜底。只登记逐章摘要 cast 里且正文出现过的人名。
fn deterministic_characters_from_cast(chapters: &[BriefChapter], prose: &str) -> Vec<RawCharacter> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for b in chapters {
        for name in &b.cast {
            let name = name.trim();
            if name.chars().count() < 2 || !prose.contains(name) || !seen.insert(name.to_string()) {
                continue;
            }
            out.push(RawCharacter {
                name: name.to_string(),
                background: Some("前文已登场（逐章摘要确定性兜底）".to_string()),
                ..Default::default()
            });
            if out.len() >= MAX_CHARACTERS {
                return out;
            }
        }
    }
    out
}

/// v0.99.1：训练续写前保证《前情提要》存在。资产重建成功时 persist 已写入；
/// 失败时用 scenes 梗概 + 人物/世界观做确定性兜底，绝不让 writer 空手续写。
pub async fn ensure_story_brief(
    pool: &DbPool,
    story_id: &str,
    context_chapters: &[(usize, String)],
) -> StoryBrief {
    if let Ok(Some(brief)) = StoryBriefRepository::new(pool.clone()).get_by_story(story_id) {
        if !brief.is_empty() {
            return brief;
        }
    }
    let pool_c = pool.clone();
    let sid = story_id.to_string();
    let chapters = context_chapters.to_vec();
    tokio::task::spawn_blocking(move || build_and_persist_brief(&pool_c, &sid, &chapters))
        .await
        .unwrap_or_default()
}

fn build_and_persist_brief(
    pool: &DbPool,
    story_id: &str,
    chapters: &[(usize, String)],
) -> StoryBrief {
    let scene_repo = SceneRepository::new(pool.clone());
    let scenes = scene_repo.get_by_story(story_id).unwrap_or_default();
    let by_num: HashMap<i32, String> = scenes
        .iter()
        .filter_map(|s| {
            s.outline_content
                .as_deref()
                .map(|o| (s.sequence_number, strip_summary_prefix(o)))
        })
        .filter(|(_, s)| !s.is_empty())
        .collect();
    // 训练沙盒 scene.sequence_number 是 1..N，而章节号是原书 134..143；
    // 必须再按索引取同一行的梗概，否则会把已生成的 LLM 摘要丢掉。
    let by_idx: Vec<String> = scenes
        .iter()
        .take(chapters.len())
        .map(|s| {
            s.outline_content
                .as_deref()
                .map(strip_summary_prefix)
                .unwrap_or_default()
        })
        .collect();
    let fallback = deterministic_chapter_summaries(chapters);
    let mut brief_chapters: Vec<BriefChapter> = Vec::new();
    for (i, (number, _)) in chapters.iter().take(MAX_BRIEF_CHAPTERS).enumerate() {
        let chapter = *number as i32;
        let summary = by_num
            .get(&chapter)
            .cloned()
            .or_else(|| by_idx.get(i).cloned().filter(|s| !s.is_empty()))
            .or_else(|| fallback.get(i).map(|(_, s)| s.clone()))
            .unwrap_or_default();
        if summary.is_empty() {
            continue;
        }
        brief_chapters.push(BriefChapter {
            chapter,
            summary,
            ..Default::default()
        });
    }
    let arc_hint = StoryRepository::new(pool.clone())
        .get_by_id(story_id)
        .ok()
        .flatten()
        .and_then(|s| s.logline)
        .filter(|s| !s.is_empty())
        .unwrap_or_default();
    let mut terms: Vec<BriefTerm> = Vec::new();
    if let Ok(chars) = CharacterRepository::new(pool.clone()).get_by_story(story_id) {
        for c in chars
            .iter()
            .filter(|c| c.source.as_deref() == Some("training_assets"))
        {
            terms.push(BriefTerm {
                term: c.name.clone(),
                kind: "人物".to_string(),
                note: c.background.clone().unwrap_or_default(),
            });
        }
    }
    if let Ok(Some(world)) = WorldBuildingRepository::new(pool.clone()).get_by_story(story_id) {
        for rule in world.rules.iter().take(8) {
            terms.push(BriefTerm {
                term: rule.name.clone(),
                kind: "规则".to_string(),
                note: rule.description.clone().unwrap_or_default(),
            });
        }
    }
    terms.truncate(24);
    let threads = brief_chapters
        .last()
        .map(|last| {
            vec![BriefThread {
                id: "main".to_string(),
                description: format!(
                    "紧接第{}章的未决事件继续推进：{}",
                    last.chapter,
                    last.summary.chars().take(120).collect::<String>()
                ),
                status: "open".to_string(),
                next_expected: "承接上一章结尾现场".to_string(),
            }]
        })
        .unwrap_or_default();
    let cast_states: Vec<BriefCastState> = terms
        .iter()
        .filter(|t| t.kind == "人物")
        .map(|t| BriefCastState {
            name: t.term.clone(),
            role: t.note.clone(),
            ..Default::default()
        })
        .collect();
    let mut brief = brief_from_summaries(brief_chapters, &arc_hint, threads, cast_states, terms);
    brief.source = "deterministic".to_string();
    let _ = StoryBriefRepository::new(pool.clone()).upsert(story_id, &brief);
    log::info!(
        "agency: 前情提要确定性兜底 story={} chapters={} terms={}",
        story_id,
        brief.chapters.len(),
        brief.canon_terms.len()
    );
    brief
}

fn strip_summary_prefix(raw: &str) -> String {
    let text = raw.trim();
    for prefix in ["本章梗概：", "本章大纲：", "场景执行大纲：", "章纲："] {
        if let Some(rest) = text.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    text.to_string()
}

/// v0.99.1：训练资产重建失败后，通用 `ensure_story_outline` 会用「正文头
/// 600 + 尾 1800」生成一份「单章回顾 + 下一拍规划」的伪大纲。这里在续写前
/// 用《前情提要》写一份确定性全局大纲；若既有大纲已被规划文档污染，直接覆盖。
pub fn ensure_deterministic_outline(pool: &DbPool, story_id: &str, brief: &StoryBrief) -> bool {
    if brief.is_empty() {
        return false;
    }
    let repo = StoryOutlineRepository::new(pool.clone());
    let existing = repo
        .get_by_story(story_id)
        .ok()
        .flatten()
        .map(|o| o.content)
        .unwrap_or_default();
    if !existing.trim().is_empty() && !looks_like_planning_outline(&existing) {
        return false;
    }
    let content = render_deterministic_outline(brief);
    if content.trim().is_empty() {
        return false;
    }
    let result = match repo.get_by_story(story_id).ok().flatten() {
        Some(_) => repo.update(story_id, Some(&content), None).map(|_| ()),
        None => repo
            .create(
                story_id,
                &content,
                None,
                3,
                Some(brief.chapters.len() as i32),
            )
            .map(|_| ()),
    };
    match result {
        Ok(()) => {
            log::info!(
                "agency: 已写入确定性故事大纲（前情提要派生）story={}",
                story_id
            );
            true
        }
        Err(e) => {
            log::warn!(
                "agency: 确定性故事大纲写入失败 story={} err={}",
                story_id,
                e
            );
            false
        }
    }
}

pub(crate) fn looks_like_planning_outline(text: &str) -> bool {
    const MARKERS: [&str; 8] = [
        "下一拍规划",
        "建议的下一场景",
        "目标 → 冲突",
        "目标->冲突",
        "### 场景一",
        "下一场景规划",
        "我将按照要求的结构",
        "故事大纲归纳",
    ];
    MARKERS.iter().any(|m| text.contains(m))
}

fn render_deterministic_outline(brief: &StoryBrief) -> String {
    let mut out = String::new();
    if !brief.arc_summary.trim().is_empty() {
        out.push_str(&format!("【核心冲突】\n{}\n", brief.arc_summary.trim()));
    }
    if !brief.chapters.is_empty() {
        out.push_str("【已发生主线】\n");
        for c in brief.chapters.iter().take(30) {
            out.push_str(&format!(
                "- 第{}章：{}\n",
                c.chapter,
                c.summary.chars().take(120).collect::<String>()
            ));
        }
    }
    if !brief.threads.is_empty() {
        out.push_str("【未结线索】\n");
        for t in brief.threads.iter().take(8) {
            out.push_str(&format!(
                "- {}（{}）\n",
                t.description.chars().take(120).collect::<String>(),
                t.id
            ));
        }
    }
    let next = brief
        .threads
        .iter()
        .find(|t| !t.next_expected.trim().is_empty())
        .map(|t| t.next_expected.trim().to_string())
        .unwrap_or_else(|| "紧接上一章结尾的未完结事件".to_string());
    out.push_str(&format!("【下一节点】\n{next}\n"));
    out
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
    fn asset_rebuild_prompt_orders_outline_before_world_to_survive_truncation() {
        let prompt = build_asset_rebuild_prompt(
            "阴脉先生",
            &[(105, "周先生在道观与鲁汉光对峙。".to_string())],
        );
        let story = prompt.find("\"story_outline\"").expect("schema");
        let chapters = prompt.find("\"chapter_outlines\"").expect("schema");
        let world = prompt.find("\"world\"").expect("schema");
        assert!(
            story < chapters && chapters < world,
            "截断时优先保住故事大纲/逐章梗概：story={story} chapters={chapters} world={world}"
        );
    }

    #[test]
    fn missing_chapter_outlines_fall_back_to_source_head_tail() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_with_scenes(&pool);
        let chapters = vec![
            (97usize, "周先生拜神仙点空子，鲁汉光盯着他。".to_string()),
            (98usize, "女老板切料子，老神仙试探周先生。".to_string()),
        ];
        let mut bundle = parse_bundle(sample_bundle()).expect("样本 JSON 应解析");
        bundle.chapter_outlines = None;
        let summary = persist_training_bundle(&pool, &story_id, &chapters, &bundle);
        assert_eq!(summary.chapter_outlines, 2, "每章都要有梗概");
        assert!(
            summary
                .warnings
                .iter()
                .any(|w| w.contains("原文首尾窗口兜底")),
            "兜底必须显式告警: {:?}",
            summary.warnings
        );
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        assert!(scenes[0]
            .outline_content
            .as_deref()
            .unwrap_or_default()
            .contains("周先生拜神仙"));
        assert!(scenes[1]
            .outline_content
            .as_deref()
            .unwrap_or_default()
            .contains("女老板切料子"));
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

    #[test]
    fn persist_writes_story_brief_with_threads_and_terms() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_with_scenes(&pool);
        let chapters = vec![
            (97usize, "周先生拜神仙点空子，鲁汉光盯着他。".to_string()),
            (98usize, "女老板切料子，老神仙试探周先生。".to_string()),
        ];
        let raw = r#"{
            "logline":"周先生在三理教的威逼利诱中反杀。",
            "story_outline":{"core_conflict":"周先生被三理教逼入教与反制","next_node":"周先生用化偶术反控乩童"},
            "chapter_outlines":[
                {"chapter":97,"summary":"周先生拜神仙点空子。","open_thread":"鲁汉光还在等答复"},
                {"chapter":98,"summary":"老神仙试探周先生。","open_thread":""}
            ],
            "open_threads":[{"id":"t-lu","description":"鲁汉光逼周先生入教","status":"open","next_expected":"周先生给出答复"}],
            "canon_terms":[{"term":"周先生","type":"身份","note":"阴脉先生"},{"term":"镇魇反噬","type":"规则","note":"解除镇魇时反噬"}],
            "characters":[{"name":"周先生","background":"阴脉先生","goals":"破局"}]
        }"#;
        let bundle = parse_bundle(raw).expect("应解析");
        let summary = persist_training_bundle(&pool, &story_id, &chapters, &bundle);
        assert!(summary.brief_written, "必须写入前情提要");
        assert_eq!(summary.brief_chapters, 2);
        let brief = StoryBriefRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap()
            .expect("前情提要应落库");
        assert_eq!(brief.chapters.len(), 2);
        assert!(brief.chapters[0].summary.contains("周先生拜神仙"));
        assert!(brief.threads.iter().any(|t| t.id == "t-lu"));
        assert!(brief.canon_terms.iter().any(|t| t.term == "周先生"));
        // 前文未出现的「镇魇反噬」不得因模型返回就进白名单
        assert!(!brief.canon_terms.iter().any(|t| t.term == "镇魇反噬"));
    }

    #[test]
    fn brief_falls_back_to_scene_summaries_when_bundle_empty() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_with_scenes(&pool);
        let chapters = vec![
            (97usize, "周先生拜神仙点空子，鲁汉光盯着他。".to_string()),
            (98usize, "女老板切料子，老神仙试探周先生。".to_string()),
        ];
        let brief = build_and_persist_brief(&pool, &story_id, &chapters);
        assert_eq!(brief.chapters.len(), 2);
        assert!(brief.chapters[0].summary.contains("周先生"));
        assert!(brief.source.contains("deterministic"));
    }

    #[test]
    fn deterministic_outline_replaces_planning_dump() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_with_scenes(&pool);
        StoryOutlineRepository::new(pool.clone())
            .create(
                &story_id,
                "## 一、已有正文归纳\n下一拍规划\n### 场景一：走廊截人",
                None,
                3,
                None,
            )
            .unwrap();
        let brief = brief_from_summaries(
            vec![
                BriefChapter {
                    chapter: 97,
                    summary: "周先生拜神仙点空子。".into(),
                    ..Default::default()
                },
                BriefChapter {
                    chapter: 98,
                    summary: "老神仙试探周先生。".into(),
                    ..Default::default()
                },
            ],
            "周先生被三理教逼入教。",
            vec![BriefThread {
                id: "t-lu".into(),
                description: "鲁汉光逼周先生入教".into(),
                status: "open".into(),
                next_expected: "周先生给出答复".into(),
            }],
            vec![],
            vec![],
        );
        assert!(ensure_deterministic_outline(&pool, &story_id, &brief));
        let outline = StoryOutlineRepository::new(pool)
            .get_by_story(&story_id)
            .unwrap()
            .unwrap();
        assert!(outline.content.contains("【已发生主线】"));
        assert!(!outline.content.contains("下一拍规划"));
        assert!(!outline.content.contains("### 场景一"));
    }

    #[test]
    fn deterministic_summary_keeps_mid_chapter_key_setup() {
        let mut content = String::from("第142章 燕归来。杨晓雯又没做晚课，我怀疑人面斑后遗症。");
        for i in 0..16 {
            content.push_str(&format!(
                "第{i}段：院里的风把纸角吹得响，我站着看了一会儿。"
            ));
        }
        content.push_str(
            "赵开来却说：「你去一趟福摩萨夜总会，有个叫邵卫江的，一打听就知道，你去见见他。」",
        );
        content.push_str("挂了电话，我看着灯火通明的小院，思忖了一会儿才走进去。");
        let summary = deterministic_chapter_summary(142, &content);
        assert!(
            summary.contains("邵卫江") || summary.contains("福摩萨夜总会"),
            "中段关键铺垫必须进入确定性摘要: {summary}"
        );
    }

    #[test]
    fn chapter_brief_cache_roundtrip() {
        let pool = create_test_pool().unwrap();
        let brief = BriefChapter {
            chapter: 142,
            summary: "赵开来赠送福摩萨夜总会道场线索。".into(),
            location: "金城饭馆".into(),
            cast: vec!["周先生".into(), "赵开来".into()],
            result: "主角决定去福摩萨夜总会见邵卫江".into(),
            open_thread: "邵卫江身份未明".into(),
            turning_point: "赵开来提出道场交换".into(),
            key_terms: vec!["福摩萨夜总会".into(), "邵卫江".into()],
        };
        brief_cache_write(&pool, "阴脉先生", &brief).unwrap();
        let read = brief_cache_read(&pool, "阴脉先生", 142).unwrap();
        assert_eq!(read.summary, brief.summary);
        assert_eq!(read.key_terms, brief.key_terms);
        assert_eq!(read.turning_point, brief.turning_point);
        assert!(brief_cache_read(&pool, "阴脉先生", 999).is_none());
    }

    #[test]
    fn chapter_brief_cache_expires_after_ttl() {
        let pool = create_test_pool().unwrap();
        let brief = BriefChapter {
            chapter: 142,
            summary: "旧摘要".into(),
            ..Default::default()
        };
        brief_cache_write(&pool, "阴脉先生", &brief).unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE continuation_training_chapter_briefs \
             SET fetched_at = '2000-01-01T00:00:00+08:00' \
             WHERE book_ref = '阴脉先生' AND chapter_index = 142",
            [],
        )
        .unwrap();
        assert!(brief_cache_read(&pool, "阴脉先生", 142).is_none());
    }

    #[test]
    fn chapter_summary_prompt_is_single_chapter_and_has_key_terms() {
        let prompt =
            build_chapter_summary_prompt("阴脉先生", 142, "正文第一段。正文第二段。正文第三段。");
        assert!(prompt.contains("第 142 章"));
        assert!(prompt.contains("key_terms"));
        assert!(prompt.contains("turning_point"));
        assert!(prompt.contains("正文第一段"));
        assert!(!prompt.contains("第 143 章"));
    }

    #[test]
    fn aggregate_prompt_only_uses_structured_summaries() {
        let summaries = vec![
            BriefChapter {
                chapter: 142,
                summary: "赵开来赠送福摩萨夜总会道场线索。".into(),
                ..Default::default()
            },
            BriefChapter {
                chapter: 143,
                summary: "主角在医院截住小护士。".into(),
                ..Default::default()
            },
        ];
        let prompt = build_brief_aggregate_prompt("阴脉先生", &summaries);
        assert!(prompt.contains("第142章"));
        assert!(prompt.contains("福摩萨夜总会"));
        assert!(prompt.contains("open_threads"));
        assert!(prompt.contains("canon_terms"));
        assert!(!prompt.contains("正文第一段"));
    }

    #[test]
    fn brief_chapters_value_roundtrips_v0992_fields() {
        let brief = BriefChapter {
            chapter: 142,
            summary: "赵开来提出道场交换。".into(),
            cast: vec!["周先生".into()],
            result: "主角要去福摩萨夜总会".into(),
            open_thread: "邵卫江身份未明".into(),
            turning_point: "道场交换".into(),
            key_terms: vec!["福摩萨夜总会".into()],
            ..Default::default()
        };
        let value = brief_chapters_value(&[brief.clone()]);
        let parsed = parse_chapter_briefs(Some(&value));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].key_terms, brief.key_terms);
        assert_eq!(parsed[0].turning_point, brief.turning_point);
    }

    #[test]
    fn parse_single_chapter_summary_object() {
        let raw = r#"{"chapter":142,"summary":"赵开来赠送福摩萨夜总会线索。","location":"金城饭馆","cast":["周先生","赵开来"],"result":"主角决定去夜总会","open_thread":"邵卫江身份未明","turning_point":"道场交换","key_terms":["福摩萨夜总会","邵卫江"]}"#;
        let value: Value = serde_json::from_str(raw).unwrap();
        let parsed = parse_chapter_briefs(Some(&value));
        assert_eq!(parsed.len(), 1, "单个摘要对象必须解析");
        assert_eq!(parsed[0].chapter, 142);
        assert_eq!(parsed[0].summary, "赵开来赠送福摩萨夜总会线索。");
        assert_eq!(parsed[0].key_terms.len(), 2);
        assert_eq!(parsed[0].turning_point, "道场交换");
    }

    #[test]
    fn deterministic_summary_strips_chapter_heading() {
        let content = "第136章 太没有礼貌了\n\n　　本地精怪不经夸。昨天还说它懂礼貌，今天就杀上门来了。我披衣下床走到窗台，伸手探香炉，摸到一块圆硬的疙瘩。";
        let summary = deterministic_chapter_summary(136, content);
        assert!(
            !summary.contains("第136章"),
            "兜底摘要不得把章名标题当事实: {summary}"
        );
        assert!(summary.contains("本地精怪"), "正文事实必须保留: {summary}");
    }

    #[test]
    fn deterministic_world_falls_back_to_chapter_facts() {
        let chapters = vec![
            BriefChapter {
                chapter: 134,
                summary: "赵开来与周成在饭馆喝酒，透露有人借定煞阵驱使女鬼。".into(),
                location: "金城饭馆".into(),
                ..Default::default()
            },
            BriefChapter {
                chapter: 135,
                summary: "周成在路边救下大波浪卷女人，回小院遇见妙姐。".into(),
                location: "路边、小院".into(),
                ..Default::default()
            },
        ];
        let world = deterministic_world_from_brief(&chapters).expect("兜底世界观应存在");
        assert!(world.concept.contains("金城饭馆"));
        assert!(world.concept.contains("定煞阵"));
        assert!(world.rules.is_empty(), "不得伪造规则表");
    }

    #[test]
    fn deterministic_characters_only_include_grounded_cast() {
        let chapters = vec![BriefChapter {
            chapter: 135,
            cast: vec!["张宝山".into(), "我".into(), "不存在的人".into()],
            ..Default::default()
        }];
        let prose = "张宝山在电话那头嘬了口烟。";
        let chars = deterministic_characters_from_cast(&chapters, prose);
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "张宝山");
    }
}
