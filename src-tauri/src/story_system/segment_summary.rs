#![allow(dead_code)]
//! 分层记忆金字塔：段摘要 + 全书纲要（P1-B，v0.61.0）。
//!
//! 章摘要（`chapter_summary`）→ 段摘要（每 10 章一条）→
//! 全书纲要（由段摘要再压缩）。 目的是给长篇一个**确定性的远期纲要**：`>50`
//! 章时续写上下文不再只靠 向量检索概率召回，而是「近章摘要 + 段摘要 +
//! 全书纲要」三层可见。
//!
//! 生成时机：章节 commit 成功后由 `spawn_refresh_after_commit` 后台补齐
//! （每 10 章一次段摘要；每次新增段摘要后重算全书纲要）。LLM 失败仅告警，
//! 不写半成品、不阻塞主流程。

use std::collections::HashMap;

use chrono::Local;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{db::DbPool, llm::LlmService, router::TaskType};

/// 每段覆盖的章节数
pub const SEGMENT_SIZE: i32 = 10;
/// 段摘要的后台调用标签（复用静默名单中的 background-summary）
pub const SUMMARY_LABEL: &str = "background-summary";
/// 生成全书纲要所需的最少段数
pub const BOOK_SUMMARY_MIN_SEGMENTS: usize = 3;
/// 段摘要输入上限（章摘要条数与字数）
const SEGMENT_INPUT_CHARS: usize = 6000;
const BOOK_INPUT_CHARS: usize = 4000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SegmentSummary {
    pub id: String,
    pub story_id: String,
    pub level: String,
    pub segment_index: i32,
    pub start_chapter: Option<i32>,
    pub end_chapter: Option<i32>,
    pub summary: String,
    pub source: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 已写满的段数量：第 10 章写完 → 1 段；第 23 章 → 2 段。
pub fn completed_segment_count(chapter_number: i32) -> i32 {
    if chapter_number <= 0 {
        0
    } else {
        chapter_number / SEGMENT_SIZE
    }
}

/// 第 `index` 段（0 基）覆盖的章节区间。
pub fn segment_bounds(index: i32) -> (i32, i32) {
    (
        index * SEGMENT_SIZE + 1,
        index * SEGMENT_SIZE + SEGMENT_SIZE,
    )
}

/// 读取全部段摘要（按段号升序）。
pub fn load_segment_summaries(pool: &DbPool, story_id: &str) -> Vec<SegmentSummary> {
    load_by_level(pool, story_id, "segment")
}

/// 读取全书纲要（没有则 None）。
pub fn load_book_summary(pool: &DbPool, story_id: &str) -> Option<SegmentSummary> {
    load_by_level(pool, story_id, "book").into_iter().next()
}

fn load_by_level(pool: &DbPool, story_id: &str, level: &str) -> Vec<SegmentSummary> {
    let Ok(conn) = pool.get() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(
        "SELECT id, story_id, level, segment_index, start_chapter, end_chapter, summary, source, \
         created_at, updated_at FROM story_segment_summaries \
         WHERE story_id = ?1 AND level = ?2 ORDER BY segment_index ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id, level], |row| {
        Ok(SegmentSummary {
            id: row.get(0)?,
            story_id: row.get(1)?,
            level: row.get(2)?,
            segment_index: row.get(3)?,
            start_chapter: row.get(4)?,
            end_chapter: row.get(5)?,
            summary: row.get(6)?,
            source: row.get(7)?,
            created_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    });
    rows.map(|r| r.flatten().collect()).unwrap_or_default()
}

fn upsert_summary(
    pool: &DbPool,
    story_id: &str,
    level: &str,
    segment_index: i32,
    start_chapter: Option<i32>,
    end_chapter: Option<i32>,
    summary: &str,
) -> Result<(), rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO story_segment_summaries \
         (id, story_id, level, segment_index, start_chapter, end_chapter, summary, source, \
          created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'llm', ?8, ?8) \
         ON CONFLICT(story_id, level, segment_index) DO UPDATE SET \
          summary = excluded.summary, start_chapter = excluded.start_chapter, \
          end_chapter = excluded.end_chapter, source = excluded.source, updated_at = excluded.updated_at",
        params![
            uuid::Uuid::new_v4().to_string(),
            story_id,
            level,
            segment_index,
            start_chapter,
            end_chapter,
            summary,
            now
        ],
    )?;
    Ok(())
}

/// 收集区间内的章节摘要（来自 scene_commits 最新一条非空 summary_text）。
/// 返回 [[start, end]] 区间内按章号升序的 (chapter, summary)。
pub fn collect_chapter_summaries(
    pool: &DbPool,
    story_id: &str,
    start_chapter: i32,
    end_chapter: i32,
) -> Vec<(i32, String)> {
    let Ok(conn) = pool.get() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(
        "SELECT chapter_number, summary_text FROM scene_commits \
         WHERE story_id = ?1 AND chapter_number >= ?2 AND chapter_number <= ?3 \
           AND summary_text IS NOT NULL AND TRIM(summary_text) <> '' \
         ORDER BY chapter_number ASC, created_at DESC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id, start_chapter, end_chapter], |row| {
        Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?))
    });
    let mut seen: HashMap<i32, String> = HashMap::new();
    if let Ok(rows) = rows {
        for (chapter, summary) in rows.flatten() {
            // 每章取最新一条（ORDER BY created_at DESC 保证首见最新）
            seen.entry(chapter).or_insert(summary);
        }
    }
    let mut out: Vec<(i32, String)> = seen.into_iter().collect();
    out.sort_by_key(|(chapter, _)| *chapter);
    out
}

fn build_segment_prompt(
    pool: &DbPool,
    start_chapter: i32,
    end_chapter: i32,
    chapters: &[(i32, String)],
) -> String {
    let body = chapters
        .iter()
        .map(|(chapter, summary)| format!("【第{chapter}章】{summary}"))
        .collect::<Vec<_>>()
        .join("\n");
    let body: String = body.chars().take(SEGMENT_INPUT_CHARS).collect();
    let mut vars = HashMap::new();
    vars.insert("start_chapter".to_string(), start_chapter.to_string());
    vars.insert("end_chapter".to_string(), end_chapter.to_string());
    vars.insert("chapter_summaries".to_string(), body.clone());
    crate::prompts::registry::resolve_prompt_with_vars(pool, "segment_summary", &vars)
        .ok()
        .or_else(|| {
            crate::prompts::registry::resolve_prompt_default_with_vars("segment_summary", &vars)
        })
        .unwrap_or_else(|| {
            format!(
                "你是长篇小说编辑。下面是第{start_chapter}-{end_chapter}章的逐章摘要。\
                 请把它们压缩成 200-300 字的**段摘要**，用于长篇的一致性参照：\
                 保留主线推进、人物状态与关系的关键变化、物品归属、尚未回收的伏笔与承诺；\
                 合并重复信息，不要逐章罗列，不要评价，不要 JSON，只输出一段正文。\n\n{body}"
            )
        })
}

fn build_book_prompt(pool: &DbPool, segments: &[SegmentSummary]) -> String {
    let body = segments
        .iter()
        .map(|segment| {
            format!(
                "【第{}-{}章】{}",
                segment.start_chapter.unwrap_or(0),
                segment.end_chapter.unwrap_or(0),
                segment.summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let body: String = body.chars().take(BOOK_INPUT_CHARS).collect();
    let mut vars = HashMap::new();
    vars.insert("segment_summaries".to_string(), body.clone());
    crate::prompts::registry::resolve_prompt_with_vars(pool, "book_summary", &vars)
        .ok()
        .or_else(|| {
            crate::prompts::registry::resolve_prompt_default_with_vars("book_summary", &vars)
        })
        .unwrap_or_else(|| {
            format!(
                "你是长篇小说编辑。下面是本书各段的段摘要。请压缩成 300-400 字的**全书纲要**：\
                 主线走向、主要人物当前处境与关系、关键物品归属、仍在悬置的伏笔与承诺。\
                 不要评价，不要 JSON，只输出一段正文。\n\n{body}"
            )
        })
}

async fn call_summary_llm(llm: &LlmService, prompt: String, max_tokens: i32) -> Option<String> {
    match llm
        .generate_for_task(
            TaskType::Summarization,
            prompt,
            Some(max_tokens),
            Some(0.3),
            Some(SUMMARY_LABEL),
        )
        .await
    {
        Ok(response) => super::chapter_summary::parse_summary_response(&response.content),
        Err(e) => {
            log::warn!("[segment_summary] LLM 失败（跳过本轮）: {}", e);
            None
        }
    }
}

/// 补齐缺失的段摘要，并在段数足够时重算全书纲要。返回本轮写入的条数。
///
/// 判定规则（幂等）：第 `chapter_number` 章提交后，凡 `[1, chapter_number]` 内
/// 已写满且尚无摘要的段都会被生成。LLM 或数据不足时跳过该段。
pub async fn refresh_summaries(
    pool: &DbPool,
    llm: &LlmService,
    story_id: &str,
    chapter_number: i32,
) -> usize {
    let completed = completed_segment_count(chapter_number);
    if completed <= 0 {
        return 0;
    }
    let existing: Vec<i32> = load_segment_summaries(pool, story_id)
        .into_iter()
        .map(|s| s.segment_index)
        .collect();
    let mut written = 0usize;

    for index in 0..completed {
        if existing.contains(&index) {
            continue;
        }
        let (start, end) = segment_bounds(index);
        let chapters = collect_chapter_summaries(pool, story_id, start, end);
        // 数据不足（少于一半章节有摘要）时跳过，等待后续章节补齐后再生成
        let expected = (end - start + 1) as usize;
        if chapters.len() * 2 < expected {
            log::info!(
                "[segment_summary] 第{}段数据不足（{}/{}），跳过",
                index,
                chapters.len(),
                expected
            );
            continue;
        }
        let prompt = build_segment_prompt(pool, start, end, &chapters);
        let Some(summary) = call_summary_llm(llm, prompt, 600).await else {
            continue;
        };
        if let Err(e) = upsert_summary(
            pool,
            story_id,
            "segment",
            index,
            Some(start),
            Some(end),
            &summary,
        ) {
            log::warn!("[segment_summary] 写入段摘要失败: {}", e);
            continue;
        }
        written += 1;
    }

    // 全书纲要：段数达到门槛时重算（段摘要变化后需要刷新）
    let segments = load_segment_summaries(pool, story_id);
    if segments.len() >= BOOK_SUMMARY_MIN_SEGMENTS {
        let prompt = build_book_prompt(pool, &segments);
        if let Some(summary) = call_summary_llm(llm, prompt, 800).await {
            if let Err(e) = upsert_summary(pool, story_id, "book", 0, None, None, &summary) {
                log::warn!("[segment_summary] 写入全书纲要失败: {}", e);
            } else {
                written += 1;
            }
        }
    }
    written
}

/// 渲染续写注入用的「故事纲要」块（P1-C）。
///
/// - 有全书纲要时总是注入（任何长度）；
/// - 超过长篇阈值（50 章）时再带上最近 3 条段摘要。
/// 内容按时间从旧到新排列。没有可用摘要时返回 None（不产生空块）。
pub fn render_story_so_far_block(
    pool: &DbPool,
    story_id: &str,
    chapter_number: i32,
) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    if chapter_number > crate::memory::orchestrator::LONG_BOOK_THRESHOLD {
        let segments = load_segment_summaries(pool, story_id);
        let tail = segments
            .iter()
            .rev()
            .take(3)
            .rev()
            .map(|segment| {
                format!(
                    "第{}-{}章：{}",
                    segment.start_chapter.unwrap_or(0),
                    segment.end_chapter.unwrap_or(0),
                    segment.summary
                )
            })
            .collect::<Vec<_>>();
        lines.extend(tail);
    }
    if let Some(book) = load_book_summary(pool, story_id) {
        lines.push(format!("全书纲要：{}", book.summary));
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "【故事纲要（前情提要，仅供一致性参照，禁止直接复述）】\n{}",
        lines.join("\n")
    ))
}

/// 章节 commit 成功后的后台入口（受全局后台 LLM 闸门约束）。
pub fn spawn_refresh_after_commit(
    app: tauri::AppHandle,
    pool: DbPool,
    story_id: String,
    chapter_number: i32,
) {
    tauri::async_runtime::spawn(async move {
        let bg_permit = crate::concurrency::BACKGROUND_LLM_SEMAPHORE.acquire().await;
        if bg_permit.is_err() {
            log::warn!("[segment_summary] 获取后台闸门失败，跳过分层摘要刷新");
            return;
        }
        let _bg_permit = bg_permit.unwrap();
        let llm = LlmService::new(app);
        let written = refresh_summaries(&pool, &llm, &story_id, chapter_number).await;
        // P3-C：段边界（每 SEGMENT_SIZE 章）写一份连续性快照，支撑回溯与恢复。
        if chapter_number % SEGMENT_SIZE == 0 {
            if let Err(e) =
                crate::story_system::checkpoint::write_checkpoint(&pool, &story_id, chapter_number)
            {
                log::warn!("[checkpoint] 写入检查点失败（非阻塞）: {}", e);
            }
        }
        if written > 0 {
            log::info!(
                "[segment_summary] 分层摘要刷新完成：{} 条（story_id={}）",
                written,
                story_id
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed_story(pool: &DbPool) -> String {
        let story_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '金字塔', ?2, ?2)",
            params![&story_id, &now],
        )
        .unwrap();
        story_id
    }

    fn seed_commit(pool: &DbPool, story_id: &str, chapter: i32, summary: &str) {
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scene_commits (id, story_id, chapter_number, status, summary_text, created_at) \
             VALUES (?1, ?2, ?3, 'accepted', ?4, ?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                story_id,
                chapter,
                summary,
                &now
            ],
        )
        .unwrap();
    }

    #[test]
    fn segment_math_covers_expected_ranges() {
        assert_eq!(completed_segment_count(0), 0);
        assert_eq!(completed_segment_count(9), 0);
        assert_eq!(completed_segment_count(10), 1);
        assert_eq!(completed_segment_count(23), 2);
        assert_eq!(segment_bounds(0), (1, 10));
        assert_eq!(segment_bounds(2), (21, 30));
    }

    #[test]
    fn collect_chapter_summaries_filters_range_and_orders() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        // scene_commits 对 (story_id, chapter_number) 有唯一约束，每章一行
        seed_commit(&pool, &story_id, 3, "第三章摘要");
        seed_commit(&pool, &story_id, 1, "第一章摘要");
        seed_commit(&pool, &story_id, 12, "范围外");

        let got = collect_chapter_summaries(&pool, &story_id, 1, 10);
        assert_eq!(got.len(), 2, "只收区间内的章: {got:?}");
        assert_eq!(got[0].0, 1);
        assert_eq!(got[1].0, 3);
        assert_eq!(got[1].1, "第三章摘要");
    }

    #[test]
    fn upsert_segment_summary_is_idempotent_per_index() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        upsert_summary(&pool, &story_id, "segment", 0, Some(1), Some(10), "第一版").unwrap();
        upsert_summary(&pool, &story_id, "segment", 0, Some(1), Some(10), "第二版").unwrap();
        let segments = load_segment_summaries(&pool, &story_id);
        assert_eq!(segments.len(), 1, "同段号应 upsert");
        assert_eq!(segments[0].summary, "第二版");
        assert_eq!(segments[0].start_chapter, Some(1));
    }

    #[test]
    fn book_summary_roundtrip() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        assert!(load_book_summary(&pool, &story_id).is_none());
        upsert_summary(&pool, &story_id, "book", 0, None, None, "全书纲要").unwrap();
        let book = load_book_summary(&pool, &story_id).expect("应有全书纲要");
        assert_eq!(book.summary, "全书纲要");
        assert_eq!(book.level, "book");
    }

    #[test]
    fn story_so_far_block_includes_segments_only_for_long_books() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        // 没有摘要时不产生空块
        assert!(render_story_so_far_block(&pool, &story_id, 3).is_none());

        upsert_summary(&pool, &story_id, "book", 0, None, None, "全书纲要内容").unwrap();
        for index in 0..3 {
            let (start, end) = segment_bounds(index);
            upsert_summary(
                &pool,
                &story_id,
                "segment",
                index,
                Some(start),
                Some(end),
                &format!("第{}段摘要", index + 1),
            )
            .unwrap();
        }

        // 中篇：只带全书纲要，不带段摘要
        let medium = render_story_so_far_block(&pool, &story_id, 30).expect("应有块");
        assert!(medium.contains("全书纲要内容"));
        assert!(!medium.contains("第1段摘要"), "中篇不注入段摘要: {medium}");

        // 长篇：段摘要 + 全书纲要，按从旧到新
        let long = render_story_so_far_block(&pool, &story_id, 60).expect("应有块");
        assert!(long.contains("第1-10章：第1段摘要"), "{long}");
        assert!(long.contains("第21-30章：第3段摘要"), "{long}");
        assert!(long.contains("全书纲要内容"), "{long}");
        let pos_first = long.find("第1段摘要").unwrap();
        let pos_third = long.find("第3段摘要").unwrap();
        assert!(pos_first < pos_third, "段摘要应按时间从旧到新");
    }
}
