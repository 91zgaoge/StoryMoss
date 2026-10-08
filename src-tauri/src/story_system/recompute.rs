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
    let mut report = RecomputeReport {
        from_chapter,
        ..Default::default()
    };

    // ── 1) 章节摘要：按当前正文重算（无 LLM 走截断回退并记债）──
    let chapters: Vec<(i32, String)> = {
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
            .query_map(params![story_id, from_chapter], |r| {
                Ok((r.get::<_, i32>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(Result::ok).collect()
    };
    for (chapter_number, content) in &chapters {
        if content.trim().is_empty() {
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
                "UPDATE scene_commits SET summary_text = ?3 \
                 WHERE story_id = ?1 AND chapter_number = ?2",
                params![story_id, chapter_number, summary],
            )
            .map_err(|e| format!("更新第{chapter_number}章摘要失败: {e}"))?
        };
        if updated > 0 {
            report.chapter_summaries += 1;
        }
    }

    // ── 2) 分层摘要 / 全书纲要：删旧重建（仅在能调用模型时删，避免删空）──
    let containing_segment =
        (from_chapter - 1) / crate::story_system::segment_summary::SEGMENT_SIZE;
    if llm.is_some() {
        let (deleted_segments, deleted_book) = {
            let conn = pool.get().map_err(|e| e.to_string())?;
            let seg = conn
                .execute(
                    "DELETE FROM story_segment_summaries \
                     WHERE story_id = ?1 AND level = 'segment' AND segment_index >= ?2",
                    params![story_id, containing_segment],
                )
                .map_err(|e| format!("删除失效段摘要失败: {e}"))?;
            let book = conn
                .execute(
                    "DELETE FROM story_segment_summaries \
                     WHERE story_id = ?1 AND level = 'book'",
                    params![story_id],
                )
                .map_err(|e| format!("删除失效全书纲要失败: {e}"))?;
            (seg, book)
        };
        report.segment_summaries_deleted = deleted_segments;
        report.book_summary_deleted = deleted_book > 0;

        // 交给既有刷新（只补缺失段 → 刚删掉的会重建），失败路径已入质量债
        let max_chapter = chapters.last().map(|(n, _)| *n).unwrap_or(0);
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
    let max_chapter = chapters.last().map(|(n, _)| *n).unwrap_or(0);
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
    let mut cleared: Vec<&str> = vec![KIND_CHAPTER_SUMMARY, KIND_CHECKPOINT];
    if llm.is_some() && !report.segment_pending_llm {
        cleared.push(KIND_SEGMENT_SUMMARY);
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
}
