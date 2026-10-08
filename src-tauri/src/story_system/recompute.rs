//! 编辑旧章后的物料失效与重算（v0.64.11）。
//!
//! 背景（真机问答落点）：改第 9 章正文只会重算**第 9 章自己**的物料；从旧第 9
//! 章 推出来的跨章物料不会失效——第 10 章以后的章节摘要、分层摘要/全书纲要
//! （`refresh_summaries` 只补缺失段，已存在的段永不重算）、每 10 章连续性快照，
//! 以及各类按章追加的记忆行。此前连「哪里过期」都没有记录。
//!
//! 本模块提供三段式：
//! 1. [`mark_stale`]：内容保存时记一笔（确定性、零 LLM、单次 upsert）；
//! 2. [`list_stale`]：运行维护页展示「哪一类物料从第几章起过期」；
//! 3. [`recompute_from`]：一键重算——章节摘要按当前正文重算（无 LLM 时回退截断，
//!    并把回退原因记进质量债），分层摘要/纲要删旧重建（仅在能调用模型时删，避免
//!    无模型时把已有摘要删空），连续性快照按当前数据重写。

use rusqlite::params;

use crate::{db::DbPool, llm::LlmService};

/// 物料类别（也是 `story_material_staleness.kind` 的取值）。
pub const KIND_CHAPTER_SUMMARY: &str = "chapter_summary";
pub const KIND_SEGMENT_SUMMARY: &str = "segment_summary";
pub const KIND_CHECKPOINT: &str = "checkpoint";
pub const ALL_KINDS: &[&str] = &[KIND_CHAPTER_SUMMARY, KIND_SEGMENT_SUMMARY, KIND_CHECKPOINT];

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct StaleMaterial {
    pub kind: String,
    pub from_chapter: i32,
    pub reason: Option<String>,
    pub updated_at: String,
}

impl StaleMaterial {
    /// 中文说明（运行维护页直接显示）。
    pub fn label(&self) -> String {
        let what = match self.kind.as_str() {
            KIND_CHAPTER_SUMMARY => "章节摘要（第 N 章及以后）",
            KIND_SEGMENT_SUMMARY => "分层摘要与全书纲要",
            KIND_CHECKPOINT => "连续性快照",
            other => other,
        };
        format!("{}：自第 {} 章起失效", what, self.from_chapter)
    }
}

/// 有段被删时，全书纲要必须一起重建（段变了纲要就过期）。
fn deleted_needs_book(dirty: &std::collections::BTreeSet<i32>) -> bool {
    !dirty.is_empty()
}

/// 段摘要刷新至少要覆盖到「被删段」的章号，否则 `refresh_summaries` 的
/// `completed_segment_count` 会看不到它们。
fn segment_refresh_floor(dirty: &std::collections::BTreeSet<i32>, step: i32) -> i32 {
    dirty
        .iter()
        .next_back()
        .map(|index| (index + 1) * step)
        .unwrap_or(0)
}

/// 记一笔失效标记：同一故事同一类取**最早**失效章（编辑第 9 章后又编辑第 12
/// 章， 仍从第 9 章起算）。
pub fn mark_stale(
    pool: &DbPool,
    story_id: &str,
    from_chapter: i32,
    reason: &str,
) -> Result<(), String> {
    if story_id.trim().is_empty() || from_chapter <= 0 {
        return Ok(());
    }
    let conn = pool.get().map_err(|e| e.to_string())?;
    let now = chrono::Local::now().to_rfc3339();
    for kind in ALL_KINDS {
        conn.execute(
            "INSERT INTO story_material_staleness \
             (id, story_id, kind, from_chapter, reason, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6) \
             ON CONFLICT(story_id, kind) DO UPDATE SET \
              from_chapter = MIN(story_material_staleness.from_chapter, excluded.from_chapter), \
              reason = excluded.reason, \
              updated_at = excluded.updated_at",
            params![
                uuid::Uuid::new_v4().to_string(),
                story_id,
                kind,
                from_chapter,
                reason,
                now
            ],
        )
        .map_err(|e| format!("记录物料失效失败: {e}"))?;
    }
    Ok(())
}

pub fn list_stale(pool: &DbPool, story_id: &str) -> Result<Vec<StaleMaterial>, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT kind, from_chapter, reason, updated_at FROM story_material_staleness \
             WHERE story_id = ?1 ORDER BY from_chapter ASC, kind ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([story_id], |r| {
            Ok(StaleMaterial {
                kind: r.get(0)?,
                from_chapter: r.get(1)?,
                reason: r.get(2).ok().flatten(),
                updated_at: r.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// 清掉指定类别的失效标记（重算完成后调用）。
fn clear_stale_kinds(
    pool: &DbPool,
    story_id: &str,
    kinds: &[&str],
) -> Result<usize, rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let mut cleared = 0;
    for kind in kinds {
        cleared += conn.execute(
            "DELETE FROM story_material_staleness WHERE story_id = ?1 AND kind = ?2",
            params![story_id, kind],
        )?;
    }
    Ok(cleared)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct RecomputeReport {
    pub from_chapter: i32,
    /// 按当前正文重算的章节摘要条数
    pub chapter_summaries: usize,
    /// 指纹未变、跳过的章节数（v0.64.12）
    pub summaries_unchanged: usize,
    /// 摘要回退原因（质量债已同步记录）
    pub summary_debts: Vec<String>,
    /// 删除并重建的分层摘要条数（含全书纲要）
    pub segment_summaries_deleted: usize,
    pub book_summary_deleted: bool,
    /// 重写的连续性快照条数
    pub checkpoints_rewritten: usize,
    /// 无可用模型：分层摘要未删未重建，待下次提交时后台补齐
    pub segment_pending_llm: bool,
    /// 正文里已找不到双方同时出现的关系行（交作者复核，不自动删）
    pub unsupported_relations: Vec<String>,
}

/// 重算第 `from_chapter` 章及以后的物料。
///
/// 顺序即依赖顺序：先章节摘要（段摘要的输入），再分层摘要/纲要，最后快照
/// （快照读当前状态与摘要）。
pub async fn recompute_from(
    pool: &DbPool,
    story_id: &str,
    from_chapter: i32,
    llm: Option<&LlmService>,
) -> Result<RecomputeReport, String> {
    let from_chapter = from_chapter.max(1);
    recompute_scoped(pool, story_id, from_chapter, from_chapter, None, llm).await
}

/// 章节 commit 之后的**自动**重算（v0.64.12）。
///
/// 与手动重算的差别：
/// - 章节摘要从 `committed_chapter + 1` 起扫（刚提交那一章的摘要已在 commit 里
///   重算并写好指纹）；
/// - 段摘要/快照/关系审计从 `committed_chapter` 起（该章所属段需要重建）；
/// - 加 `max_summary_chapters` 上限，超出部分重新标记失效，等下一次触发续算。
pub async fn recompute_after_commit(
    pool: &DbPool,
    story_id: &str,
    committed_chapter: i32,
    llm: Option<&LlmService>,
    max_summary_chapters: Option<usize>,
) -> Result<RecomputeReport, String> {
    let material_from = committed_chapter.max(1);
    recompute_scoped(
        pool,
        story_id,
        material_from + 1,
        material_from,
        max_summary_chapters,
        llm,
    )
    .await
}

/// 重算内核：`summary_from` 起扫章节摘要，`material_from`
/// 起处理段摘要/快照/审计。
///
/// **指纹感知**（v0.64.12）：只对「正文指纹与上次摘要不一致」的章做摘要（无 LLM
/// 时 回退截断也算一次），未变的章直接跳过——长书从第 9 章重算不再等于 90
/// 次调用。
async fn recompute_scoped(
    pool: &DbPool,
    story_id: &str,
    summary_from: i32,
    material_from: i32,
    max_summary_chapters: Option<usize>,
    llm: Option<&LlmService>,
) -> Result<RecomputeReport, String> {
    let from_chapter = material_from.max(1);
    let summary_from = summary_from.max(1);
    let mut report = RecomputeReport {
        from_chapter,
        ..Default::default()
    };

    // ── 1) 章节摘要：指纹变了才重算（无 LLM 走截断回退并记债）──
    let (mut chapters, stored_hashes, story_max_chapter): (
        Vec<(i32, String)>,
        std::collections::HashMap<i32, String>,
        i32,
    ) = {
        let conn = pool.get().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT c.chapter_number, COALESCE(s.content, '') \
                 FROM chapters c LEFT JOIN scenes s ON s.chapter_id = c.id \
                 WHERE c.story_id = ?1 AND c.chapter_number >= ?2 \
                 ORDER BY c.chapter_number ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![story_id, summary_from], |r| {
                Ok((r.get::<_, i32>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let chapters: Vec<(i32, String)> = rows.filter_map(Result::ok).collect();

        let hashes: Vec<(i32, String)> = {
            let mut stmt = conn
                .prepare(
                    "SELECT chapter_number, COALESCE(summary_source_hash, '') \
                     FROM scene_commits WHERE story_id = ?1",
                )
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([story_id], |r| {
                    Ok((r.get::<_, i32>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?;
            rows.filter_map(Result::ok).collect()
        };

        let max_chapter: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(chapter_number), 0) FROM chapters WHERE story_id = ?1",
                [story_id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        (chapters, hashes.into_iter().collect(), max_chapter)
    };

    // 上限：超出部分重新标记失效（下次触发续算），本次只处理前 N 章
    let mut deferred_from: Option<i32> = None;
    if let Some(cap) = max_summary_chapters {
        if chapters.len() > cap {
            let pending = chapters[cap].0;
            chapters.truncate(cap);
            deferred_from = Some(pending);
        }
    }

    // 段摘要失效范围：本次真正改过摘要的段（精准，不再按章号一刀切）
    let mut dirty_segments: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    let step = crate::story_system::segment_summary::SEGMENT_SIZE;
    for (chapter_number, content) in &chapters {
        if content.trim().is_empty() {
            continue;
        }
        let fingerprint = crate::story_system::chapter_summary::content_fingerprint(content);
        if stored_hashes
            .get(chapter_number)
            .map(|h| !h.is_empty() && h == &fingerprint)
            .unwrap_or(false)
        {
            report.summaries_unchanged += 1;
            continue;
        }
        let (summary, quality) =
            crate::story_system::chapter_summary::summarize_chapter_with_quality(
                Some(pool),
                content,
                *chapter_number,
                llm,
            )
            .await;
        if let Some(detail) = quality.debt_detail(*chapter_number) {
            let _ = crate::story_system::quality_debt::record_debt(
                pool,
                story_id,
                None,
                Some(*chapter_number),
                "chapter_summary",
                "warning",
                &detail,
            );
            report.summary_debts.push(detail);
        }
        if summary.trim().is_empty() {
            continue;
        }
        let updated = {
            let conn = pool.get().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE scene_commits SET summary_text = ?3, summary_source_hash = ?4 \
                 WHERE story_id = ?1 AND chapter_number = ?2",
                params![story_id, chapter_number, summary, fingerprint],
            )
            .map_err(|e| format!("更新第{chapter_number}章摘要失败: {e}"))?
        };
        if updated > 0 {
            report.chapter_summaries += 1;
            dirty_segments.insert((chapter_number - 1) / step);
        }
    }

    // ── 2) 分层摘要 / 全书纲要：只删「摘要变过」的段并重建（仅在有模型时删）──
    let mut segments_to_delete: std::collections::BTreeSet<i32> = dirty_segments.clone();
    // 手工重算时若整段没有摘要（历史缺失），也纳入重建范围
    if segments_to_delete.is_empty() && llm.is_some() {
        segments_to_delete.insert((from_chapter - 1) / step);
    }
    if llm.is_some() {
        let (deleted_segments, deleted_book) = {
            let conn = pool.get().map_err(|e| e.to_string())?;
            let mut seg = 0usize;
            for index in &segments_to_delete {
                seg += conn
                    .execute(
                        "DELETE FROM story_segment_summaries \
                         WHERE story_id = ?1 AND level = 'segment' AND segment_index = ?2",
                        params![story_id, index],
                    )
                    .map_err(|e| format!("删除失效段摘要失败: {e}"))?;
            }
            let book = if deleted_needs_book(&segments_to_delete) {
                conn.execute(
                    "DELETE FROM story_segment_summaries \
                     WHERE story_id = ?1 AND level = 'book'",
                    params![story_id],
                )
                .map_err(|e| format!("删除失效全书纲要失败: {e}"))?
            } else {
                0
            };
            (seg, book)
        };
        report.segment_summaries_deleted = deleted_segments;
        report.book_summary_deleted = deleted_book > 0;

        // 交给既有刷新（只补缺失段 → 刚删掉的会重建），失败路径已入质量债
        let max_chapter = story_max_chapter.max(segment_refresh_floor(&segments_to_delete, step));
        if let Some(llm) = llm {
            let seg_report = crate::story_system::segment_summary::refresh_summaries(
                pool,
                llm,
                story_id,
                max_chapter,
            )
            .await;
            for detail in seg_report.debt_details() {
                let _ = crate::story_system::quality_debt::record_debt(
                    pool,
                    story_id,
                    None,
                    Some(max_chapter.max(1)),
                    "segment_summary",
                    "warning",
                    &detail,
                );
            }
            report.segment_pending_llm = !seg_report.llm_failed.is_empty();
        }
    } else {
        report.segment_pending_llm = true;
    }

    // ── 3) 连续性快照：删旧 + 按当前数据重写（确定性，无需模型）──
    {
        let conn = pool.get().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM story_checkpoints WHERE story_id = ?1 AND chapter_number >= ?2",
            params![story_id, from_chapter],
        )
        .map_err(|e| format!("删除失效快照失败: {e}"))?;
    }
    let max_chapter = story_max_chapter;
    let step = crate::story_system::segment_summary::SEGMENT_SIZE;
    let mut boundary = ((from_chapter + step - 1) / step) * step;
    while boundary <= max_chapter {
        if crate::story_system::checkpoint::write_checkpoint(pool, story_id, boundary).is_ok() {
            report.checkpoints_rewritten += 1;
        }
        boundary += step;
    }

    // ── 3.5) 关系支撑审计：正文改动后，把「正文里已找不到双方同时出现」的手工
    // 关系行记成质量债（保守判定，只报告不删除——可能是作者手改过的设定）──
    match crate::story_system::relation_retract::audit_unsupported_relations(pool, story_id) {
        Ok(rows) => {
            for (a, b, ty) in rows {
                let detail = format!(
                    "关系「{a} → {b}（{ty}）」的双方在当前正文里已不再同时出现，请复核是否已失效"
                );
                let _ = crate::story_system::quality_debt::record_debt(
                    pool,
                    story_id,
                    None,
                    Some(from_chapter),
                    "relation_grounding",
                    "info",
                    &detail,
                );
                report.unsupported_relations.push(detail);
            }
        }
        Err(e) => log::warn!("[recompute] 关系支撑审计失败（非阻塞）: {e}"),
    }

    // ── 4) 清失效标记：章节摘要/快照总是清；分层摘要仅在确实重建后清 ──
    let mut cleared: Vec<&str> = vec![KIND_CHECKPOINT];
    if llm.is_some() && !report.segment_pending_llm {
        cleared.push(KIND_SEGMENT_SUMMARY);
    }
    // 被上限截断时保留 chapter_summary 标记，并把起点抬到未处理的章
    match deferred_from {
        Some(pending) => {
            let _ = mark_stale(pool, story_id, pending, "重算分批：余下章节待下一轮");
            let _ = clear_stale_kinds(pool, story_id, &[KIND_CHAPTER_SUMMARY]);
            let _ = mark_stale(pool, story_id, pending, "重算分批：余下章节待下一轮");
        }
        None => cleared.push(KIND_CHAPTER_SUMMARY),
    }
    let _ = clear_stale_kinds(pool, story_id, &cleared);

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed_story(pool: &DbPool) -> String {
        crate::db::StoryRepository::new(pool.clone())
            .create(crate::db::CreateStoryRequest {
                title: "物料重算".into(),
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

    fn seed_chapter_with_content(pool: &DbPool, story_id: &str, n: i32, body: &str) -> String {
        let conn = pool.get().unwrap();
        let now = chrono::Local::now().to_rfc3339();
        let chapter_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![chapter_id, story_id, n, format!("第{n}章"), now],
        )
        .unwrap();
        let scene_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, content, chapter_id, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                scene_id,
                story_id,
                n,
                format!("第{n}章"),
                body,
                chapter_id,
                now
            ],
        )
        .unwrap();
        // 已提交章（重算只更新既有 commit 行，不新建）
        conn.execute(
            "INSERT INTO scene_commits (id, story_id, chapter_number, status, summary_text, \
             created_at) VALUES (?1, ?2, ?3, 'accepted', '', ?4)",
            params![uuid::Uuid::new_v4().to_string(), story_id, n, now],
        )
        .unwrap();
        chapter_id
    }

    #[test]
    fn mark_stale_keeps_earliest_chapter_per_kind() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);

        mark_stale(&pool, &story_id, 12, "编辑第12章").unwrap();
        mark_stale(&pool, &story_id, 9, "编辑第9章").unwrap();
        let stale = list_stale(&pool, &story_id).unwrap();
        assert_eq!(stale.len(), ALL_KINDS.len(), "{stale:?}");
        for s in &stale {
            assert_eq!(s.from_chapter, 9, "必须取最早失效章：{s:?}");
            assert_eq!(s.reason.as_deref(), Some("编辑第9章"));
        }
        assert!(stale[0].label().contains("第 9 章"));

        // 无效输入不落行
        mark_stale(&pool, &story_id, 0, "空章号").unwrap();
        assert_eq!(list_stale(&pool, &story_id).unwrap().len(), ALL_KINDS.len());
    }

    #[tokio::test]
    async fn recompute_rewrites_chapter_summaries_and_clears_flags() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        for n in 1..=3 {
            let body = format!("第{n}章的正文内容，用于摘要重算的样本句子。");
            seed_chapter_with_content(&pool, &story_id, n, &body);
        }
        mark_stale(&pool, &story_id, 2, "编辑第2章").unwrap();

        // 无 LLM：章节摘要走截断回退（并记债），分层摘要保留待模型
        let report = recompute_from(&pool, &story_id, 2, None).await.unwrap();
        assert_eq!(report.chapter_summaries, 2, "第2、3章都应重算");
        assert!(!report.summary_debts.is_empty(), "回退必须给出说明");
        assert!(report.segment_pending_llm);

        let conn = pool.get().unwrap();
        let s3: String = conn
            .query_row(
                "SELECT summary_text FROM scene_commits WHERE story_id = ?1 AND chapter_number = 3",
                [&story_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(s3.contains("第3章的正文内容"), "摘要应按当前正文重算: {s3}");

        // 章节摘要与快照的失效标记被清；分层摘要保留（尚未重建）
        let stale = list_stale(&pool, &story_id).unwrap();
        assert!(
            stale.iter().all(|s| s.kind == KIND_SEGMENT_SUMMARY),
            "只应剩分层摘要待模型：{stale:?}"
        );

        // 章摘要回退原因已入质量债（运行维护页可见）
        let debts =
            crate::story_system::quality_debt::list_debts(&pool, &story_id, Some("open"), 50);
        assert!(
            debts.iter().any(|d| d.source == "chapter_summary"),
            "debts={debts:?}"
        );
    }

    #[tokio::test]
    async fn recompute_deletes_stale_segments_only_when_llm_available() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_chapter_with_content(&pool, &story_id, 2, "第2章正文。");
        {
            let conn = pool.get().unwrap();
            let now = chrono::Local::now().to_rfc3339();
            conn.execute(
                "INSERT INTO story_segment_summaries (id, story_id, level, segment_index, \
                 start_chapter, end_chapter, summary, source, created_at, updated_at) \
                 VALUES (?1, ?2, 'segment', 0, 1, 10, '旧段摘要', 'llm', ?3, ?3)",
                params![uuid::Uuid::new_v4().to_string(), story_id, now],
            )
            .unwrap();
        }
        // 无 LLM：不得删除已有段摘要（宁可留着过期，也不能删空）
        let report = recompute_from(&pool, &story_id, 2, None).await.unwrap();
        assert_eq!(report.segment_summaries_deleted, 0);
        let kept: i64 = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM story_segment_summaries WHERE story_id = ?1",
                [&story_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, 1, "无模型时不得删除段摘要");
    }
    /// v0.64.12 真机契约：指纹未变的章不重算（长书「从第 N 章重算」不再等于
    /// 一次全量重写），只有真改过的章才重算并标记所属段。
    #[tokio::test]
    async fn recompute_skips_chapters_whose_prose_unchanged() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        for n in 1..=3 {
            seed_chapter_with_content(&pool, &story_id, n, &format!("第{n}章正文。"));
        }
        // 第一次重算：写下指纹
        let first = recompute_from(&pool, &story_id, 1, None).await.unwrap();
        assert_eq!(first.chapter_summaries, 3, "首轮应全部重算");
        assert_eq!(first.summaries_unchanged, 0);

        // 只改第 2 章正文
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE scenes SET content = '第2章正文（改过）。' WHERE story_id = ?1 \
             AND sequence_number = 2",
            [&story_id],
        )
        .unwrap();
        drop(conn);

        let second = recompute_from(&pool, &story_id, 1, None).await.unwrap();
        assert_eq!(second.chapter_summaries, 1, "只有第2章应重算");
        assert_eq!(second.summaries_unchanged, 2, "第1、3章指纹未变应跳过");
        let s2: String = pool
            .get()
            .unwrap()
            .query_row(
                "SELECT summary_text FROM scene_commits WHERE story_id = ?1 AND chapter_number = 2",
                [&story_id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(s2.contains("改过"), "第2章摘要应按新正文重算: {s2}");
    }

    /// v0.64.12：章数上限——超出部分保留标记并从下一章续算
    #[tokio::test]
    async fn recompute_cap_defers_rest_and_keeps_flag() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        for n in 1..=5 {
            seed_chapter_with_content(&pool, &story_id, n, &format!("第{n}章正文。"));
        }
        mark_stale(&pool, &story_id, 1, "编辑第1章").unwrap();
        let report = recompute_scoped(&pool, &story_id, 1, 1, Some(2), None)
            .await
            .unwrap();
        assert_eq!(report.chapter_summaries, 2, "只处理前 2 章");
        let stale = list_stale(&pool, &story_id).unwrap();
        let chapter_flag = stale
            .iter()
            .find(|s| s.kind == KIND_CHAPTER_SUMMARY)
            .expect("必须保留章节摘要失效标记");
        assert_eq!(chapter_flag.from_chapter, 3, "起点抬到未处理的第3章");

        // 续算：处理完剩下的
        let again = recompute_scoped(&pool, &story_id, 3, 3, Some(12), None)
            .await
            .unwrap();
        assert_eq!(again.chapter_summaries, 3);
        assert!(
            list_stale(&pool, &story_id)
                .unwrap()
                .iter()
                .all(|s| s.kind != KIND_CHAPTER_SUMMARY),
            "续算完成后章节摘要标记应清掉"
        );
    }

    /// v0.64.12：自动触发的判定——有失效且起点不晚于刚提交的章
    #[test]
    fn should_auto_recompute_only_when_stale_at_or_before_committed() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        assert_eq!(should_auto_recompute(&pool, &story_id, 9), None);

        mark_stale(&pool, &story_id, 9, "编辑第9章").unwrap();
        assert_eq!(
            should_auto_recompute(&pool, &story_id, 8),
            None,
            "早于失效点不触发"
        );
        assert_eq!(should_auto_recompute(&pool, &story_id, 9), Some(9));
        assert_eq!(should_auto_recompute(&pool, &story_id, 13), Some(9));

        // 并发去重：同一故事只能持有一个在跑名额
        assert!(try_acquire_inflight(&story_id));
        assert!(!try_acquire_inflight(&story_id));
        release_inflight(&story_id);
        assert!(try_acquire_inflight(&story_id));
        release_inflight(&story_id);
    }
}

// ==================== v0.64.12：自动后台重算 ====================

/// 单次自动重算最多处理多少章的章节摘要（超出部分重新标记，等下一轮续算）。
pub const AUTO_RECOMPUTE_MAX_CHAPTERS: usize = 12;

/// 正在自动重算的故事（并发去重：同一故事同时只跑一个）。
fn inflight() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static INFLIGHT: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    INFLIGHT.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

fn try_acquire_inflight(story_id: &str) -> bool {
    let mut guard = match inflight().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    guard.insert(story_id.to_string())
}

fn release_inflight(story_id: &str) {
    let mut guard = match inflight().lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    guard.remove(story_id);
}

/// 是否值得自动重算：有失效标记，且标记起点 ≤ 刚提交的章（否则说明失效是
/// 由后面某一章的编辑引起的，等那一章提交时再算）。返回应使用的材料起点。
pub fn should_auto_recompute(pool: &DbPool, story_id: &str, committed_chapter: i32) -> Option<i32> {
    let stale = list_stale(pool, story_id).ok()?;
    let from = stale.iter().map(|s| s.from_chapter).min()?;
    if from <= committed_chapter.max(1) {
        Some(from)
    } else {
        None
    }
}

/// 章节提交防抖之后调用：有失效物料就后台自动重算（无需作者点按钮）。
///
/// 约束：①同一故事并发去重；②受全局后台模型闸门约束（拿不到就放弃，标记留着
/// 下次再算）；③单轮章数上限，超出部分保留标记续算；④开关
/// `auto_recompute_after_edit`（默认开）可关。
pub fn spawn_auto_recompute_if_stale(
    app: tauri::AppHandle,
    pool: DbPool,
    story_id: String,
    committed_chapter: i32,
) {
    use tauri::Manager;
    // 开关（默认开）：关掉则完全不消耗后台模型
    if let Ok(dir) = app.path().app_data_dir() {
        if let Ok(cfg) = crate::config::AppConfig::load(&dir) {
            if !cfg.auto_recompute_after_edit {
                return;
            }
        }
    }

    tauri::async_runtime::spawn(async move {
        let pool_check = pool.clone();
        let sid_check = story_id.clone();
        let target = tokio::task::spawn_blocking(move || {
            should_auto_recompute(&pool_check, &sid_check, committed_chapter)
        })
        .await
        .ok()
        .flatten();
        let Some(from_chapter) = target else {
            return;
        };
        if !try_acquire_inflight(&story_id) {
            log::info!("[recompute] 自动重算已在运行，跳过 story={story_id}");
            return;
        }

        let permit = crate::concurrency::BACKGROUND_LLM_SEMAPHORE.acquire().await;
        if permit.is_err() {
            log::warn!("[recompute] 后台闸门不可用，跳过自动重算 story={story_id}");
            release_inflight(&story_id);
            return;
        }
        let _permit = permit.unwrap();

        let llm = LlmService::new(app);
        let result = recompute_after_commit(
            &pool,
            &story_id,
            committed_chapter,
            Some(&llm),
            Some(AUTO_RECOMPUTE_MAX_CHAPTERS),
        )
        .await;
        match result {
            Ok(report) => log::warn!(
                "[recompute] 自动重算完成（自第{}章，触发章 {}）：章节摘要 {} 条重算 / {} 条未变、段摘要删 {}、快照重写 {}、待模型 {}",
                report.from_chapter,
                committed_chapter,
                report.chapter_summaries,
                report.summaries_unchanged,
                report.segment_summaries_deleted,
                report.checkpoints_rewritten,
                report.segment_pending_llm
            ),
            Err(e) => {
                log::warn!("[recompute] 自动重算失败 story={story_id}: {e}");
                let _ = crate::story_system::quality_debt::record_debt(
                    &pool,
                    &story_id,
                    None,
                    Some(committed_chapter),
                    "auto_recompute",
                    "warning",
                    &format!("编辑后自动重算失败（可在运行维护页手动重算）：{e}"),
                );
            }
        }
        release_inflight(&story_id);
        let _ = from_chapter;
    });
}
