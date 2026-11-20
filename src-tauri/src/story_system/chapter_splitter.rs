//! 自动划分章节（v0.26.57）
//!
//! 设置驱动：
//! - `chapter_split_mode =
//!   "word_count"`：当前章字数超过阈值时，在段落边界切出下一章
//! - `chapter_split_mode = "plot"`：在情节/场景转换点切章（启发式，无额外 LLM）
//!
//! 字数单位：中文「字」（`TextUtils::chinese_word_count`）。
//! 字数上限留空 → 使用 `DEFAULT_CHAPTER_SPLIT_MAX_CHARS`（3000）。
//!
//! 触发：场景内容保存后 30s 空闲（与 auto_commit
//! 同窗口）。任意位置的章均可切分——此前仅限最新章，续写持续写入非最新章
//! 时该章会无限增长（真实数据出现单章 19,492 字）。切中间章时新章插入其后，
//! 同一事务内将后续章 `chapter_number` 与关联 `scenes.sequence_number` 重排
//! +1， 失败整体回滚，不留半切分状态。
//!
//! 单次触发循环切分：一次触发内反复切分指定章及其溢出新章，
//! 直到溢出章字数 ≤ 阈值或找不到切分点为止——粘贴恢复的大段正文（如
//! 9 万字）可在一次触发内分章完成。安全上限 `MAX_SPLIT_ITERATIONS`（50），
//! 且单轮无进展（新溢出章字数不小于上一轮）时中断并告警。
//!
//! 并发与数据契约：
//! - 章号在事务内读取（I-1），以事务内值为准做重排；另有进程内 per-story split
//!   互斥（`story_split_lock`，沿用 `memory::asset_bridge::STORY_LOCKS`
//!   惯例）做第二道保险，同一 story 同时只跑一个
//!   split，拿锁失败直接跳过（下个防抖窗口再来）。
//! - 合约跟随内容（I-2）：切中间章时，`chapter_number > N` 的 CHAPTER
//!   合约随其章一并顺延 +1；新溢出章 N+1 必无合约，标题回退 `第{N+1}章`。
//! - 多场景守卫（I-3）：章关联 scene 数 > 1 时告警并跳过（不截断、
//!   不新建）——章内容是全场景拼接，截断只写首场景会造成正文重复。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use rusqlite::{params, OptionalExtension};
use tauri::AppHandle;

use crate::{
    config::{AppConfig, DEFAULT_CHAPTER_SPLIT_MAX_CHARS},
    db::{
        chapter_split::{plan_split, split_chapter_in_tx, ChapterSplitMode},
        ChapterRepository, DbPool,
    },
    state_sync::StateSync,
    utils::text::TextUtils,
};

/// 进程内 per-story split 互斥注册表（沿用
/// `memory::asset_bridge::STORY_LOCKS` 惯例）。章号已在事务内读取
/// （I-1），本锁是第二道保险：本应用是单进程桌面应用，进程内锁即可
/// 保证同一 story 的 split 串行执行，拿锁失败直接跳过（下个防抖窗口再来）。
static STORY_SPLIT_LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();

fn story_split_lock(story_id: &str) -> Arc<Mutex<()>> {
    let registry = STORY_SPLIT_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = registry.lock().unwrap_or_else(|p| p.into_inner());
    map.entry(story_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

/// 解析有效字数上限：`None` / `0` / 负数 → 自动默认 3000 字。
pub fn resolve_max_chars(configured: Option<i32>) -> usize {
    match configured {
        Some(n) if n > 0 => n as usize,
        _ => DEFAULT_CHAPTER_SPLIT_MAX_CHARS,
    }
}

/// 单次触发内循环切分的安全上限。
const MAX_SPLIT_ITERATIONS: usize = 50;

/// 一次切分的结果（供事件发射与返回值使用）。
struct SplitOutcome {
    /// 被截断的原章 id
    old_chapter_id: String,
    /// 原章第一个场景 id（用于 sceneUpdated 事件）
    scene_id: String,
    /// 新切出的章 id
    new_chapter_id: String,
    /// 新章标题
    new_chapter_title: Option<String>,
    /// 本次切分中被顺延重排（chapter_number +1）的旧章 id
    renumbered_chapter_ids: Vec<String>,
}

/// 对指定章（任意位置，不限最新章）执行一次切分；无需/不可切分时返回
/// `Ok(None)`。
///
/// 重排 + 插入新章 + 截断全部在一个事务内完成（见 `split_chapter_in_tx`）。
/// 章号在**事务内**读取（I-1），以事务内读到的值为准做重排——事务外
/// 预读会在并发 split 时拿到过期章号导致静默错序（另有 per-story split
/// 互斥做第二道保险，见 `maybe_split_latest_chapter`）。
/// 不发射事件（便于单测）；事件由调用方按 `SplitOutcome` 补发。
fn split_latest_chapter_once(
    pool: &DbPool,
    story_id: &str,
    chapter_id: &str,
    mode: ChapterSplitMode,
    max_chars: usize,
) -> Result<Option<SplitOutcome>, String> {
    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 章必须属于本故事；任意位置均可切分。事务内读章号（I-1）。
    let chapter_number: Option<i32> = tx
        .query_row(
            "SELECT chapter_number FROM chapters WHERE id = ?1 AND story_id = ?2",
            params![chapter_id, story_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(chapter_number) = chapter_number else {
        return Ok(None);
    };

    let content: String = {
        let mut stmt = tx
            .prepare(
                "SELECT COALESCE(content, '') FROM scenes WHERE chapter_id = ?1 \
                 ORDER BY sequence_number",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([chapter_id], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        let mut parts = Vec::new();
        for row in rows {
            parts.push(row.map_err(|e| e.to_string())?);
        }
        parts.concat()
    };
    // 无需切分：事务未写任何数据，drop 即回滚
    let Some(plan) = plan_split(&content, mode, max_chars) else {
        return Ok(None);
    };
    let scene_id: Option<String> = tx
        .query_row(
            "SELECT id FROM scenes WHERE chapter_id = ?1 ORDER BY sequence_number LIMIT 1",
            [chapter_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(scene_id) = scene_id else {
        return Ok(None);
    };

    // None = 多场景章被守卫跳过（不截断、不新建；守卫内已告警）
    let Some((new_chapter_id, renumbered_chapter_ids, new_title)) =
        split_chapter_in_tx(&tx, story_id, chapter_id, chapter_number, &plan, &scene_id)
            .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    tx.commit().map_err(|e| e.to_string())?;

    log::info!(
        "[ChapterSplitter] split chapter {} → new {} (mode={:?}, max_chars={}, renumbered={})",
        chapter_id,
        new_chapter_id,
        mode,
        max_chars,
        renumbered_chapter_ids.len()
    );

    Ok(Some(SplitOutcome {
        old_chapter_id: chapter_id.to_string(),
        scene_id,
        new_chapter_id,
        new_chapter_title: Some(new_title),
        renumbered_chapter_ids,
    }))
}

/// 循环切分指定章及其溢出新章，直到溢出章字数 ≤ 阈值或找不到切分点。
///
/// 每轮对上一刀切出的溢出章再切（溢出章号 = 上一刀新章号，其后的章继续
/// 顺延）。安全上限 `MAX_SPLIT_ITERATIONS`；单轮无进展（新溢出章字数不小于
/// 上一轮）时中断并告警。返回各次切分结果（按切分顺序）。
fn split_latest_until_within_threshold(
    pool: &DbPool,
    story_id: &str,
    chapter_id: &str,
    mode: ChapterSplitMode,
    max_chars: usize,
) -> Result<Vec<SplitOutcome>, String> {
    let chapter_repo = ChapterRepository::new(pool.clone());
    let mut outcomes = Vec::new();
    let mut current_id = chapter_id.to_string();
    let mut prev_len: Option<usize> = None;

    for _ in 0..MAX_SPLIT_ITERATIONS {
        let content = chapter_repo
            .get_content(&current_id)
            .map_err(|e| e.to_string())?;
        let len = TextUtils::chinese_word_count(&content);

        // 安全网：新溢出章字数不小于上一轮 → 无进展，防死循环
        if let Some(prev) = prev_len {
            if len >= prev {
                log::warn!(
                    "[ChapterSplitter] split loop made no progress (chapter {} len {} >= prev {}), stop",
                    current_id,
                    len,
                    prev
                );
                break;
            }
        }
        if len <= max_chars {
            break;
        }
        prev_len = Some(len);

        let Some(outcome) =
            split_latest_chapter_once(pool, story_id, &current_id, mode, max_chars)?
        else {
            break;
        };
        current_id = outcome.new_chapter_id.clone();
        outcomes.push(outcome);
    }

    if outcomes.len() >= MAX_SPLIT_ITERATIONS {
        log::warn!(
            "[ChapterSplitter] split loop reached max iterations ({}), stop",
            MAX_SPLIT_ITERATIONS
        );
    }

    Ok(outcomes)
}

/// 对指定章执行自动划分（若需要）。章可为故事任意位置（不限最新章）；
/// 切中间章时新章插入其后，后续章号在事务内顺延重排。
///
/// 单次触发内循环切分：每次切出的溢出新章继续检查并切分，
/// 直到溢出章字数 ≤ 阈值或找不到切分点（安全上限 `MAX_SPLIT_ITERATIONS`，
/// 单轮无进展时中断）。每次切分各自发射
/// chapterUpdated/sceneUpdated/chapterCreated 事件；被顺延重排的旧章
/// 逐个补发 chapterUpdated，并在有重排时追加一次 story 级
/// `DataRefresh("chapters")`，避免前端缓存的章号/列表过期。
///
/// 返回 `Ok(Some(last_new_chapter_id))`：多次切分时返回**最后一次**切出的章
/// id（当前唯一调用方 scene_service 仅用于日志）；`Ok(None)` 表示无需切分
/// 或同 story 已有 split 进行中（per-story 互斥跳过，I-1）。
pub fn maybe_split_latest_chapter(
    pool: &DbPool,
    app_handle: &AppHandle,
    story_id: &str,
    chapter_id: &str,
    config: &AppConfig,
) -> Result<Option<String>, String> {
    let mode = ChapterSplitMode::parse(&config.chapter_split_mode);
    let max_chars = resolve_max_chars(config.chapter_split_max_chars);

    // I-1 保险：进程内 per-story split 互斥（沿用 memory::asset_bridge
    // STORY_LOCKS 惯例）。章号已在事务内读取，本锁防并发 split 任务交错；
    // 拿锁失败直接跳过，下个 30s 防抖窗口再来。
    let lock = story_split_lock(story_id);
    let Ok(_split_guard) = lock.try_lock() else {
        log::info!(
            "[ChapterSplitter] story {} 已有 split 进行中，本次跳过（下个防抖窗口再试）",
            story_id
        );
        return Ok(None);
    };

    let outcomes =
        split_latest_until_within_threshold(pool, story_id, chapter_id, mode, max_chars)?;

    let mut any_renumbered = false;
    for outcome in &outcomes {
        let _ =
            StateSync::emit_chapter_updated(app_handle, &outcome.old_chapter_id, None, story_id);
        let _ = StateSync::emit_scene_updated(app_handle, story_id, &outcome.scene_id, None, true);
        let _ = StateSync::emit_chapter_created(
            app_handle,
            story_id,
            &outcome.new_chapter_id,
            outcome.new_chapter_title.as_deref(),
            Some(outcome.old_chapter_id.as_str()),
        );
        for renumbered_id in &outcome.renumbered_chapter_ids {
            StateSync::emit_chapter_updated(app_handle, renumbered_id, None, story_id);
            any_renumbered = true;
        }
    }
    if any_renumbered {
        StateSync::emit_data_refresh(app_handle, Some(story_id), "chapters");
    }

    Ok(outcomes.last().map(|o| o.new_chapter_id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chinese_repeat(ch: char, n: usize) -> String {
        std::iter::repeat(ch).take(n).collect()
    }

    #[test]
    fn resolve_max_chars_empty_uses_default() {
        assert_eq!(resolve_max_chars(None), DEFAULT_CHAPTER_SPLIT_MAX_CHARS);
        assert_eq!(resolve_max_chars(Some(0)), DEFAULT_CHAPTER_SPLIT_MAX_CHARS);
        assert_eq!(resolve_max_chars(Some(-1)), DEFAULT_CHAPTER_SPLIT_MAX_CHARS);
        assert_eq!(resolve_max_chars(Some(2500)), 2500);
    }

    // ==================== 循环切分（DB 集成） ====================

    use crate::db::{
        create_test_pool, CreateChapterRequest, CreateStoryRequest, SceneRepository,
        StoryContractRepository, StoryRepository,
    };

    /// 种一个故事 + 第一章（内容写入其场景），返回 (story_id, chapter_id)。
    fn seed_story_with_chapter(pool: &DbPool, content: &str) -> (String, String) {
        let story_repo = StoryRepository::new(pool.clone());
        let story = story_repo
            .create(CreateStoryRequest {
                title: "循环分章测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let chapter_repo = ChapterRepository::new(pool.clone());
        let chapter = chapter_repo
            .create(CreateChapterRequest {
                story_id: story.id.clone(),
                chapter_number: 1,
                title: Some("第一章".to_string()),
                outline: None,
                content: Some(content.to_string()),
            })
            .unwrap();
        (story.id, chapter.id)
    }

    #[test]
    fn bulk_content_splits_repeatedly_in_one_trigger() {
        let pool = create_test_pool().unwrap();
        // 5 段 × 1800 字 = 9000 字，段落边界保证 word_count 模式每轮可切
        let para = chinese_repeat('文', 1800);
        let content = [para.as_str(); 5].join("\n\n");
        let (story_id, chapter_id) = seed_story_with_chapter(&pool, &content);

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &chapter_id,
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();

        assert!(
            outcomes.len() >= 2,
            "9000 字应一次触发切出多章, got {}",
            outcomes.len()
        );

        let chapter_repo = ChapterRepository::new(pool.clone());
        let chapters = chapter_repo.get_by_story(&story_id).unwrap();
        assert_eq!(chapters.len(), 1 + outcomes.len());

        // 每章字数都在阈值内（最后一章是剩余部分）
        for ch in &chapters {
            let c = chapter_repo.get_content(&ch.id).unwrap();
            assert!(
                TextUtils::chinese_word_count(&c) <= 3000,
                "chapter {} exceeds threshold: {}",
                ch.chapter_number,
                TextUtils::chinese_word_count(&c)
            );
        }

        // 章号连续 1..=N
        let mut numbers: Vec<i32> = chapters.iter().map(|c| c.chapter_number).collect();
        numbers.sort_unstable();
        assert_eq!(numbers, (1..=chapters.len() as i32).collect::<Vec<_>>());
    }

    #[test]
    fn boundary_less_content_terminates_without_runaway() {
        let pool = create_test_pool().unwrap();
        // 无段落 / 句末边界：word_count 回退到阈值处硬切，每轮新章必缩短，
        // 循环必然终止且章数有界
        let content = chinese_repeat('密', 9000);
        let (story_id, chapter_id) = seed_story_with_chapter(&pool, &content);

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &chapter_id,
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();

        // 9000 / 3000 → 2 次切分、3 章，远小于安全上限
        assert!(
            outcomes.len() <= 3,
            "boundary-less content should terminate quickly, got {}",
            outcomes.len()
        );
        let chapter_repo = ChapterRepository::new(pool.clone());
        let chapters = chapter_repo.get_by_story(&story_id).unwrap();
        assert_eq!(chapters.len(), 1 + outcomes.len());
        assert!((chapters.len() as usize) < MAX_SPLIT_ITERATIONS);
    }

    #[test]
    fn unsplittable_content_exits_loop_without_new_chapter() {
        let pool = create_test_pool().unwrap();
        // 开头即段落边界 → keep 为 0 字，plan_split 拒绝 → 无切分、循环退出
        let content = format!("\n\n{}", chinese_repeat('孤', 4000));
        let (story_id, chapter_id) = seed_story_with_chapter(&pool, &content);

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &chapter_id,
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();

        assert!(outcomes.is_empty());
        let chapter_repo = ChapterRepository::new(pool.clone());
        assert_eq!(chapter_repo.get_by_story(&story_id).unwrap().len(), 1);
    }

    #[test]
    fn within_threshold_content_loop_is_noop() {
        let pool = create_test_pool().unwrap();
        let content = chinese_repeat('短', 1000);
        let (story_id, chapter_id) = seed_story_with_chapter(&pool, &content);

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &chapter_id,
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();

        assert!(outcomes.is_empty());
        let chapter_repo = ChapterRepository::new(pool.clone());
        assert_eq!(chapter_repo.get_by_story(&story_id).unwrap().len(), 1);
    }

    // ==================== 合约跟随内容（I-2） ====================

    fn chapter_contract(
        chapter_number: i32,
        goal: &str,
    ) -> crate::domain::contracts::ChapterContract {
        crate::domain::contracts::ChapterContract {
            schema_version: "1".to_string(),
            contract_type: "CHAPTER".to_string(),
            generator_version: "test".to_string(),
            chapter_number,
            chapter_directive: crate::domain::contracts::ChapterDirective {
                goal: goal.to_string(),
                must_cover_nodes: vec![],
                forbidden_zones: vec![],
                time_anchor: None,
                chapter_span: None,
            },
        }
    }

    /// 该 story 全部 CHAPTER 合约解析后的 chapter_number（升序）。
    fn chapter_contract_numbers(pool: &DbPool, story_id: &str) -> Vec<i32> {
        let repo = StoryContractRepository::new(pool.clone());
        let mut numbers: Vec<i32> = repo
            .get_by_story(story_id)
            .unwrap()
            .iter()
            .filter(|c| c.contract_type == "CHAPTER")
            .filter_map(|c| {
                serde_json::from_str::<crate::domain::contracts::ChapterContract>(&c.contract_json)
                    .ok()
            })
            .map(|c| c.chapter_number)
            .collect();
        numbers.sort_unstable();
        numbers
    }

    /// 合约跟随内容：循环切分最新章时，`chapter_number > N` 的合约随内容
    /// 顺延 +1，新溢出章必无合约、标题回退 `第{N}章`。回归 v0.33.x bug：
    /// 一次切分出的章节全部同名——现在所有新章回退默认命名，互不相同。
    #[test]
    fn split_shifts_contracts_with_content_and_falls_back_titles() {
        let pool = create_test_pool().unwrap();
        // 3 段 × 1800 字 → 切出第 2、3 章
        let para = chinese_repeat('文', 1800);
        let content = [para.as_str(); 3].join("\n\n");
        let (story_id, chapter_id) = seed_story_with_chapter(&pool, &content);

        // 仅第 2 章有合约
        let contract_repo = StoryContractRepository::new(pool.clone());
        contract_repo
            .create(
                &story_id,
                "CHAPTER",
                &serde_json::to_string(&chapter_contract(2, "潜入敌营救出同伴")).unwrap(),
            )
            .unwrap();

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &chapter_id,
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();
        assert_eq!(outcomes.len(), 2, "5400 字应切出 2 章");

        // 合约跟随内容：切第 1 章时合约 2→3，切第 2 章时再 3→4
        assert_eq!(chapter_contract_numbers(&pool, &story_id), vec![4]);

        let chapter_repo = ChapterRepository::new(pool.clone());
        let chapters = chapter_repo.get_by_story(&story_id).unwrap();
        let title_of = |n: i32| {
            chapters
                .iter()
                .find(|c| c.chapter_number == n)
                .and_then(|c| c.title.clone())
                .unwrap_or_default()
        };
        // 新溢出章无合约 → 回退默认命名
        assert_eq!(title_of(2), "第二章");
        assert_eq!(title_of(3), "第三章");
        // 三章标题互不相同
        let mut titles: Vec<String> = chapters.iter().filter_map(|c| c.title.clone()).collect();
        titles.sort();
        titles.dedup();
        assert_eq!(titles.len(), chapters.len());
    }

    /// 中间章切分：> N 的合约随其章顺延（ch3 合约 → ch4），== N 的合约
    /// 不动（ch2 合约留在 ch2）；新溢出章 ch3 无合约，标题回退 `第3章`。
    #[test]
    fn middle_chapter_split_shifts_contracts_with_content() {
        let pool = create_test_pool().unwrap();
        let ch2_content = format!(
            "{}\n\n{}",
            chinese_repeat('中', 1800),
            chinese_repeat('间', 1800)
        );
        let (story_id, ids) =
            seed_story_with_chapters(&pool, &["第一章内容", &ch2_content, "第三章内容"]);
        let ch2_id = ids[1].clone();
        let ch3_id = ids[2].clone();

        let contract_repo = StoryContractRepository::new(pool.clone());
        for (n, goal) in [(2, "第二章合约"), (3, "第三章合约")] {
            contract_repo
                .create(
                    &story_id,
                    "CHAPTER",
                    &serde_json::to_string(&chapter_contract(n, goal)).unwrap(),
                )
                .unwrap();
        }

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &ch2_id,
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();
        assert_eq!(outcomes.len(), 1, "3600 字应切 1 刀");
        assert_eq!(
            outcomes[0].new_chapter_title.as_deref(),
            Some("第三章"),
            "新溢出章 N+1 无合约，标题应回退默认命名"
        );

        // 合约跟随内容：ch2 合约（== N）不动，ch3 合约（> N）顺延为 4
        assert_eq!(chapter_contract_numbers(&pool, &story_id), vec![2, 4]);

        // 旧 ch3 的内容随章号顺延到 ch4，未被篡改
        let chapter_repo = ChapterRepository::new(pool.clone());
        let chapters = chapter_repo.get_by_story(&story_id).unwrap();
        let old3 = chapters.iter().find(|c| c.id == ch3_id).unwrap();
        assert_eq!(old3.chapter_number, 4);
        assert!(chapter_repo
            .get_content(&ch3_id)
            .unwrap()
            .contains("第三章内容"));
    }

    // ==================== 多场景章守卫（I-3） ====================

    /// 章关联 scene 数 > 1 时跳过切分：不截断、不新建，两场景内容原样保留。
    #[test]
    fn multi_scene_chapter_is_skipped_without_split() {
        let pool = create_test_pool().unwrap();
        let (story_id, chapter_id) = seed_story_with_chapter(&pool, &chinese_repeat('首', 2000));
        // 给该章追加第二个场景（合计 4000 字 > 阈值 3000）
        {
            let conn = pool.get().unwrap();
            let now = chrono::Local::now().to_rfc3339();
            conn.execute(
                "INSERT INTO scenes (id, story_id, sequence_number, title, content, \
                 characters_present, character_conflicts, execution_stage, chapter_id, \
                 created_at, updated_at) \
                 VALUES ('sc-extra', ?1, 100, '续', ?2, '[]', '[]', 'drafting', ?3, ?4, ?4)",
                params![&story_id, chinese_repeat('续', 2000), &chapter_id, now],
            )
            .unwrap();
        }

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &chapter_id,
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();
        assert!(outcomes.is_empty(), "多场景章应跳过切分");

        // 章数不变、两场景内容均未截断
        let chapter_repo = ChapterRepository::new(pool.clone());
        assert_eq!(chapter_repo.get_by_story(&story_id).unwrap().len(), 1);
        let conn = pool.get().unwrap();
        let contents: Vec<String> = conn
            .prepare(
                "SELECT COALESCE(content, '') FROM scenes WHERE chapter_id = ?1 \
                 ORDER BY sequence_number",
            )
            .unwrap()
            .query_map([&chapter_id], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            contents,
            vec![chinese_repeat('首', 2000), chinese_repeat('续', 2000)]
        );
    }

    // ==================== per-story split 互斥（I-1） ====================

    #[test]
    fn per_story_split_lock_is_exclusive_per_story() {
        let lock_a = story_split_lock("lock-test-a");
        let guard = lock_a.try_lock().expect("首个 split 应拿到锁");
        assert!(
            lock_a.try_lock().is_err(),
            "同 story 并发 split 应被互斥（拿锁失败跳过）"
        );
        // 同一注册表多次取锁返回同一实例
        let lock_a2 = story_split_lock("lock-test-a");
        assert!(lock_a2.try_lock().is_err());
        // 不同 story 互不影响
        let lock_b = story_split_lock("lock-test-b");
        assert!(lock_b.try_lock().is_ok(), "不同 story 的 split 互不影响");
        drop(guard);
        assert!(lock_a.try_lock().is_ok(), "释放后可再次拿锁");
    }

    // ==================== 中间章切分 + 后续章重排 ====================

    /// 种一个故事 + 若干章（内容写入各章场景），返回 (story_id, chapter_ids)。
    fn seed_story_with_chapters(pool: &DbPool, contents: &[&str]) -> (String, Vec<String>) {
        let story_repo = StoryRepository::new(pool.clone());
        let story = story_repo
            .create(CreateStoryRequest {
                title: "中间章切分测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let chapter_repo = ChapterRepository::new(pool.clone());
        let mut ids = Vec::new();
        for (i, content) in contents.iter().enumerate() {
            let chapter = chapter_repo
                .create(CreateChapterRequest {
                    story_id: story.id.clone(),
                    chapter_number: (i + 1) as i32,
                    title: Some(format!("第{}章", i + 1)),
                    outline: None,
                    content: Some(content.to_string()),
                })
                .unwrap();
            ids.push(chapter.id);
        }
        (story.id, ids)
    }

    /// 3 章故事中切中间的超长 ch2：新章插入 ch2 之后，旧 ch3 顺延为 ch4，
    /// chapters.chapter_number 与 scenes.sequence_number 同步重排。
    #[test]
    fn middle_chapter_split_renumbers_following_chapters_and_scenes() {
        let pool = create_test_pool().unwrap();
        // ch2：2 段 × 1800 字 = 3600 字，超阈值可切 1 刀
        let ch2_content = format!(
            "{}\n\n{}",
            chinese_repeat('中', 1800),
            chinese_repeat('间', 1800)
        );
        let (story_id, ids) =
            seed_story_with_chapters(&pool, &["第一章内容", &ch2_content, "第三章内容"]);
        let (ch1_id, ch2_id, ch3_id) = (ids[0].clone(), ids[1].clone(), ids[2].clone());

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &ch2_id,
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();
        assert_eq!(
            outcomes.len(),
            1,
            "3600 字应切 1 刀, got {}",
            outcomes.len()
        );
        assert_eq!(
            outcomes[0].renumbered_chapter_ids,
            vec![ch3_id.clone()],
            "旧第3章应被重排"
        );

        let chapter_repo = ChapterRepository::new(pool.clone());
        let chapters = chapter_repo.get_by_story(&story_id).unwrap();
        assert_eq!(chapters.len(), 4);
        let number_of = |id: &str| chapters.iter().find(|c| c.id == id).unwrap().chapter_number;
        assert_eq!(number_of(&ch1_id), 1);
        assert_eq!(number_of(&ch2_id), 2);
        assert_eq!(number_of(&outcomes[0].new_chapter_id), 3);
        assert_eq!(number_of(&ch3_id), 4, "旧第3章应顺延为第4章");

        // ch2 截断 ≤ 3000，新章含溢出
        let c2 = chapter_repo.get_content(&ch2_id).unwrap();
        assert!(
            TextUtils::chinese_word_count(&c2) <= 3000,
            "ch2 截断后仍超阈值: {}",
            TextUtils::chinese_word_count(&c2)
        );
        let c_new = chapter_repo
            .get_content(&outcomes[0].new_chapter_id)
            .unwrap();
        assert!(c_new.contains('间'), "新章应包含溢出内容");

        // scenes.sequence_number 与章号同步重排
        let scene_repo = SceneRepository::new(pool.clone());
        let scenes = scene_repo.get_by_story(&story_id).unwrap();
        let seq_of = |cid: &str| {
            scenes
                .iter()
                .find(|s| s.chapter_id.as_deref() == Some(cid))
                .unwrap()
                .sequence_number
        };
        assert_eq!(seq_of(&ch2_id), 2);
        assert_eq!(seq_of(&outcomes[0].new_chapter_id), 3);
        assert_eq!(
            seq_of(&ch3_id),
            4,
            "旧第3章的场景 sequence_number 应顺延为 4"
        );
        let title_of = |id: &str| {
            chapters
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .title
                .clone()
                .unwrap_or_default()
        };
        assert_eq!(title_of(&ch3_id), "第四章", "派生标题须跟随新章号");
        assert_eq!(title_of(&ch1_id), "第1章");
        assert_eq!(title_of(&ch2_id), "第2章");
    }

    /// 中间章 ch1 有 9000+ 字：一次触发循环切成多章，旧后续章顺延到队尾。
    #[test]
    fn middle_chapter_loop_split_pushes_following_chapters_to_tail() {
        let pool = create_test_pool().unwrap();
        let para = chinese_repeat('文', 1800);
        let ch1_content = [para.as_str(); 5].join("\n\n"); // 9000 字
        let (story_id, ids) = seed_story_with_chapters(&pool, &[&ch1_content, "短章"]);
        let ch2_id = ids[1].clone();

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &ids[0],
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();
        assert!(
            outcomes.len() >= 2,
            "9000 字应一次触发切出多章, got {}",
            outcomes.len()
        );

        let chapter_repo = ChapterRepository::new(pool.clone());
        let chapters = chapter_repo.get_by_story(&story_id).unwrap();
        assert_eq!(chapters.len(), 2 + outcomes.len());

        // 旧 ch2 顺延到队尾
        let old2 = chapters.iter().find(|c| c.id == ch2_id).unwrap();
        assert_eq!(
            old2.chapter_number,
            chapters.len() as i32,
            "旧第2章应顺延到队尾"
        );
        let expected_tail_title = format!(
            "第{}章",
            crate::utils::chapter_title::chinese_numeral(chapters.len() as u32)
        );
        assert_eq!(
            old2.title.as_deref(),
            Some(expected_tail_title.as_str()),
            "队尾章派生标题须跟随新章号"
        );

        // 每章字数都在阈值内
        for ch in &chapters {
            let c = chapter_repo.get_content(&ch.id).unwrap();
            assert!(
                TextUtils::chinese_word_count(&c) <= 3000,
                "chapter {} exceeds threshold: {}",
                ch.chapter_number,
                TextUtils::chinese_word_count(&c)
            );
        }

        // 章号连续 1..=N，场景号同步连续
        let mut numbers: Vec<i32> = chapters.iter().map(|c| c.chapter_number).collect();
        numbers.sort_unstable();
        assert_eq!(numbers, (1..=chapters.len() as i32).collect::<Vec<_>>());
        let scene_repo = SceneRepository::new(pool.clone());
        let mut seqs: Vec<i32> = scene_repo
            .get_by_story(&story_id)
            .unwrap()
            .iter()
            .map(|s| s.sequence_number)
            .collect();
        seqs.sort_unstable();
        assert_eq!(seqs, (1..=chapters.len() as i32).collect::<Vec<_>>());
    }

    /// 回归：最新章切分行为不变（无后续章可重排）。
    #[test]
    fn latest_chapter_split_behavior_unchanged() {
        let pool = create_test_pool().unwrap();
        let para = chinese_repeat('文', 1800);
        let content = [para.as_str(); 2].join("\n\n"); // 3600 字 → 切 1 刀
        let (story_id, chapter_id) = seed_story_with_chapter(&pool, &content);

        let outcomes = split_latest_until_within_threshold(
            &pool,
            &story_id,
            &chapter_id,
            ChapterSplitMode::WordCount,
            3000,
        )
        .unwrap();
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].renumbered_chapter_ids.is_empty());

        let chapter_repo = ChapterRepository::new(pool.clone());
        let chapters = chapter_repo.get_by_story(&story_id).unwrap();
        assert_eq!(chapters.len(), 2);
        let mut numbers: Vec<i32> = chapters.iter().map(|c| c.chapter_number).collect();
        numbers.sort_unstable();
        assert_eq!(numbers, vec![1, 2]);
    }
}
