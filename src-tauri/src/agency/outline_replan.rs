//! P0-2（v0.67.0）：纲层失效与局部重规划。
//!
//! 缘起：docs/audits/2026-10-14-sherpa-benchmark-analysis.md。Sherpa 的层级
//! 规划器「识别变化、追踪下游影响、仅重规划受影响的部分」——StoryMoss 的失效
//! 重算（`story_system::recompute`）覆盖物料层（章摘要/段摘要/快照），但下游
//! **章纲**（计划层）不在失效范围内：改了第 3 章，第 4 章以后的章纲不受影响。
//!
//! 本模块补上计划层的一环：
//! - `mark_outline_stale_for_edit`：编辑旧章时（确有下游章纲才）标记
//!   `chapter_outline` 失效（from = 编辑章 + 1）；
//! - `preview`：按当前正文重生成受影响章纲的**预览**（每轮上限 N 章， LLM
//!   调用，不落库）；
//! - `apply`：只落库作者勾选确认的那些条目（复用 `set_chapter_plan_prefix`
//!   保留【当前场大纲】卡块；与「报告不改写、作者裁决」的既有哲学一致，
//!   手写/手调章纲天然受保护——默认不勾选）。
//!
//! 纪律：`chapter_outline` 刻意不入 `recompute::ALL_KINDS`（它不是物料、
//! 不参与后台自动重算；`should_auto_recompute` 已过滤），重规划必须由作者
//! 在运行维护页显式触发。

use std::sync::Arc;

use crate::{
    agency::{
        budget::{AgencyBudget, DEFAULT_RUN_TOKEN_BUDGET},
        coordinator::AgencyCoordinator,
    },
    db::{
        repositories::{SceneRepository, SceneUpdate},
        DbPool,
    },
    error::AppError,
    story_system::recompute::{self, KIND_CHAPTER_OUTLINE},
};

/// 每轮预览最多重生成多少章（LLM 成本约束；超出部分下轮继续）。
pub const REPLAN_MAX_CHAPTERS: usize = 3;

/// 有章纲的下游章（候选）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OutlineCandidate {
    pub chapter_number: i32,
    pub scene_id: String,
    pub plan: String,
}

/// 预览条目：一章的新旧章纲对照；`new_plan` 为空时 `skipped` 说明原因。
#[derive(Debug, Clone, serde::Serialize)]
pub struct OutlineReplanItem {
    pub chapter_number: i32,
    pub scene_id: String,
    pub old_plan: String,
    pub new_plan: Option<String>,
    pub skipped: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PreviewReport {
    /// 自 `from_chapter` 起「有章纲」的候选总数（可能大于 items 长度）。
    pub total: usize,
    pub items: Vec<OutlineReplanItem>,
}

/// 作者确认后要落库的单条。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ApplyOutlineItem {
    pub scene_id: String,
    pub chapter_number: i32,
    pub plan: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyReport {
    pub applied: usize,
    pub remaining: usize,
}

/// 「自 from_chapter 起、已有章纲」的章（按章号升序）。章纲判定复用
/// `continue_assets::chapter_plan_from_scene_outline`（只剥节拍卡块，手写大纲
/// 不误伤）。
pub fn chapter_outline_candidates(
    pool: &DbPool,
    story_id: &str,
    from_chapter: i32,
) -> Result<Vec<OutlineCandidate>, String> {
    let scenes = SceneRepository::new(pool.clone())
        .get_by_story(story_id)
        .map_err(|e| format!("查询场景失败: {e}"))?;
    let mut out: Vec<OutlineCandidate> = scenes
        .into_iter()
        .filter(|s| s.sequence_number >= from_chapter)
        .filter_map(|s| {
            let plan = crate::agency::continue_assets::chapter_plan_from_scene_outline(
                s.outline_content.as_deref().unwrap_or(""),
            );
            let plan = plan.trim().to_string();
            if plan.is_empty() {
                None
            } else {
                Some(OutlineCandidate {
                    chapter_number: s.sequence_number,
                    scene_id: s.id,
                    plan,
                })
            }
        })
        .collect();
    out.sort_by_key(|c| c.chapter_number);
    Ok(out)
}

/// 编辑第 `edited_chapter` 章后标记下游章纲失效。**确有下游章纲**才标记
/// （没有下游计划的章节改正文不产生无意义提示）；新章首次落笔由调用方
/// （`had_content_before`）把关。返回 `Some(起始章)` 表示已标记。
pub fn mark_outline_stale_for_edit(
    pool: &DbPool,
    story_id: &str,
    edited_chapter: i32,
) -> Result<Option<i32>, String> {
    let from = edited_chapter + 1;
    let candidates = chapter_outline_candidates(pool, story_id, from)?;
    if candidates.is_empty() {
        return Ok(None);
    }
    recompute::mark_stale_kind(
        pool,
        story_id,
        KIND_CHAPTER_OUTLINE,
        from,
        &format!(
            "第{}章正文被编辑，下游 {} 章章纲可能过期（待按当前正文重规划）",
            edited_chapter,
            candidates.len()
        ),
    )?;
    Ok(Some(from))
}

/// 按当前正文重生成受影响章纲的预览（每轮 ≤ `max_chapters` 章；不落库）。
/// 生成失败或产出过短的章计入 `skipped`，不阻断其余章。
pub async fn preview(
    coordinator: &AgencyCoordinator,
    pool: &DbPool,
    story_id: &str,
    from_chapter: i32,
    max_chapters: usize,
) -> Result<PreviewReport, AppError> {
    let candidates =
        chapter_outline_candidates(pool, story_id, from_chapter).map_err(AppError::from)?;
    let total = candidates.len();
    if total == 0 {
        return Ok(PreviewReport {
            total: 0,
            items: Vec::new(),
        });
    }
    let has_outline = {
        let pool = pool.clone();
        let sid = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            crate::db::repositories::StoryOutlineRepository::new(pool.clone())
                .get_by_story(&sid)
                .ok()
                .flatten()
                .map(|o| !o.content.trim().is_empty())
                .unwrap_or(false)
        })
        .await
        .map_err(|e| AppError::internal(format!("story outline join error: {e}")))?
    };
    let title = coordinator
        .story_title(story_id)
        .await
        .unwrap_or_else(|| "未命名".to_string());
    let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
    let run_id = format!("replan-outline-{}", uuid::Uuid::new_v4());
    let take = max_chapters.max(1);
    let mut items = Vec::new();
    for candidate in candidates.into_iter().take(take) {
        let premise = format!(
            "重规划《{}》第{}章章纲（按当前正文与既有资产刷新）",
            title, candidate.chapter_number
        );
        let new_plan = if has_outline {
            coordinator
                .generate_chapter_outline(
                    &run_id,
                    story_id,
                    &premise,
                    &budget,
                    candidate.chapter_number,
                    "【故事大纲】",
                    "",
                )
                .await
        } else {
            String::new()
        };
        let new_plan = new_plan.trim().to_string();
        let (new_plan, skipped) = if new_plan.is_empty() {
            (
                None,
                Some(if has_outline {
                    "生成失败或产出过短，本轮跳过".to_string()
                } else {
                    "故事尚无大纲，无法重规划".to_string()
                }),
            )
        } else {
            (Some(new_plan), None)
        };
        items.push(OutlineReplanItem {
            chapter_number: candidate.chapter_number,
            scene_id: candidate.scene_id,
            old_plan: candidate.plan,
            new_plan,
            skipped,
        });
    }
    Ok(PreviewReport { total, items })
}

/// 落库作者勾选的章纲：`set_chapter_plan_prefix`（保留【当前场大纲】卡块）→
/// 更新 `scenes.outline_content`。校验失败（场景不存在/章号不符/
/// 计划为空）的条目 静默跳过。
///
/// 失效标记推进：作者确认到第 M 章后，把标记前移到「第 M+1 章起」（清理后重标，
/// 因为标记取 MIN 不能自行后移）；后面已无「有章纲」的候选才清除标记。
pub fn apply(
    pool: &DbPool,
    story_id: &str,
    items: &[ApplyOutlineItem],
) -> Result<ApplyReport, AppError> {
    let repo = SceneRepository::new(pool.clone());
    let mut applied = 0usize;
    let mut confirmed_upto: Option<i32> = None;
    for item in items {
        let plan = item.plan.trim();
        if plan.is_empty() {
            continue;
        }
        let scene = match repo.get_by_id(&item.scene_id) {
            Ok(Some(scene)) => scene,
            Ok(None) => continue,
            Err(e) => {
                log::warn!("[outline_replan] 读取场景失败 {}: {}", item.scene_id, e);
                continue;
            }
        };
        if scene.story_id != story_id || scene.sequence_number != item.chapter_number {
            log::warn!(
                "[outline_replan] 场景与章号不符，跳过：{}（期望第{}章）",
                item.scene_id,
                item.chapter_number
            );
            continue;
        }
        let merged =
            crate::agency::observe::set_chapter_plan_prefix(scene.outline_content.as_deref(), plan);
        if let Err(e) = repo.update(
            &item.scene_id,
            &SceneUpdate {
                outline_content: Some(merged),
                ..Default::default()
            },
        ) {
            log::warn!("[outline_replan] 章纲落库失败 {}: {}", item.scene_id, e);
            continue;
        }
        applied += 1;
        confirmed_upto = Some(
            confirmed_upto
                .map(|n| n.max(item.chapter_number))
                .unwrap_or(item.chapter_number),
        );
    }

    let stale_from = recompute::list_stale(pool, story_id)
        .unwrap_or_default()
        .into_iter()
        .find(|s| s.kind == KIND_CHAPTER_OUTLINE)
        .map(|s| s.from_chapter);
    let Some(stale_from) = stale_from else {
        return Ok(ApplyReport {
            applied,
            remaining: 0,
        });
    };
    let (scan_from, advance_to) = match confirmed_upto {
        Some(n) => (n + 1, Some(n + 1)),
        None => (stale_from, None),
    };
    let remaining = chapter_outline_candidates(pool, story_id, scan_from)
        .map_err(AppError::from)?
        .len();
    if remaining == 0 {
        if let Err(e) = recompute::clear_stale_kind(pool, story_id, KIND_CHAPTER_OUTLINE) {
            log::warn!("[outline_replan] 清除章纲失效标记失败（非阻塞）: {e}");
        }
    } else if let Some(from) = advance_to {
        // 标记取 MIN 不会自行后移：先清再标，把起点推进到第 from 章。
        if let Err(e) = recompute::clear_stale_kind(pool, story_id, KIND_CHAPTER_OUTLINE) {
            log::warn!("[outline_replan] 推进章纲失效标记（清理）失败: {e}");
        }
        if let Err(e) = recompute::mark_stale_kind(
            pool,
            story_id,
            KIND_CHAPTER_OUTLINE,
            from,
            &format!("已重规划至第 {} 章，其余待续", from - 1),
        ) {
            log::warn!("[outline_replan] 推进章纲失效标记失败: {e}");
        }
    }
    Ok(ApplyReport { applied, remaining })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{agency::tests::MockLlm, db::create_test_pool};

    fn seed_story(pool: &DbPool) -> String {
        let story = crate::db::repositories::StoryRepository::new(pool.clone())
            .create(crate::db::dto::CreateStoryRequest {
                title: "重规划".into(),
                description: Some("前提".into()),
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, is_auto_generated, created_at, updated_at)
             VALUES ('c1', ?1, '阿苔', '拾荒者', '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES ('o1', ?1, '核心冲突：寻找星环。三幕：起因-发展-高潮。', NULL, 3, NULL, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        story.id
    }

    fn seed_scene(pool: &DbPool, story_id: &str, n: i32, plan: Option<&str>) -> String {
        let repo = SceneRepository::new(pool.clone());
        let scene = repo.create(story_id, n, Some(&format!("第{n}章"))).unwrap();
        if let Some(plan) = plan {
            repo.update(
                &scene.id,
                &SceneUpdate {
                    outline_content: Some(format!(
                        "{plan}\n\n【当前场大纲】\n在场：阿苔\n冲突：加压：对峙"
                    )),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        scene.id
    }

    #[test]
    fn mark_outline_stale_only_when_downstream_plans_exist() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_scene(&pool, &story_id, 3, Some("本章：阿苔追查断灯绳。"));
        // 无下游章纲（只有第 3 章自己）→ 编辑第 3 章不标记。
        assert_eq!(
            mark_outline_stale_for_edit(&pool, &story_id, 3).unwrap(),
            None
        );
        assert!(recompute::list_stale(&pool, &story_id)
            .unwrap()
            .iter()
            .all(|s| s.kind != KIND_CHAPTER_OUTLINE));
        // 第 4、5 章都有章纲 → 编辑第 3 章标记 from=4，reason 含 2 章。
        seed_scene(&pool, &story_id, 4, Some("本章：阿苔渡口取玉佩。"));
        seed_scene(&pool, &story_id, 5, None);
        let from = mark_outline_stale_for_edit(&pool, &story_id, 3).unwrap();
        assert_eq!(from, Some(4));
        let row = recompute::list_stale(&pool, &story_id)
            .unwrap()
            .into_iter()
            .find(|s| s.kind == KIND_CHAPTER_OUTLINE)
            .expect("应有章纲失效行");
        assert_eq!(row.from_chapter, 4);
        assert!(row.label().contains("章纲"), "维护页中文标签");
        assert!(row.reason.unwrap_or_default().contains("1 章"));
        // MIN 合并：再编辑第 2 章，起点仍取最早（3）。
        assert_eq!(
            mark_outline_stale_for_edit(&pool, &story_id, 2).unwrap(),
            Some(3)
        );
        let row = recompute::list_stale(&pool, &story_id)
            .unwrap()
            .into_iter()
            .find(|s| s.kind == KIND_CHAPTER_OUTLINE)
            .unwrap();
        assert_eq!(row.from_chapter, 3);
    }

    #[test]
    fn outline_kind_is_excluded_from_auto_recompute() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        seed_scene(&pool, &story_id, 3, Some("本章：甲。"));
        seed_scene(&pool, &story_id, 4, Some("本章：乙。"));
        mark_outline_stale_for_edit(&pool, &story_id, 3).unwrap();
        // 只有 chapter_outline 失效时，自动重算不得被触发（否则永久空转）。
        assert_eq!(recompute::should_auto_recompute(&pool, &story_id, 10), None);
        // 物料类失效仍照常触发。
        recompute::mark_stale(&pool, &story_id, 3, "编辑第3章").unwrap();
        assert_eq!(
            recompute::should_auto_recompute(&pool, &story_id, 10),
            Some(3)
        );
    }

    #[tokio::test]
    async fn preview_and_apply_round_trip() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let ch4 = seed_scene(&pool, &story_id, 4, Some("本章：旧纲（甲与乙对峙）。"));
        let ch5 = seed_scene(&pool, &story_id, 5, Some("本章：旧纲（丙登场）。"));
        mark_outline_stale_for_edit(&pool, &story_id, 3).unwrap();

        // generate_chapter_outline 会丢弃 <50 字的产出，测试脚本须给足长度。
        let plan5 = "本章大纲：阿苔在渡口交还玉佩并追问断灯绳的来处，苏会山只留下一句警告便退走；\
             阿苔带着玉佩回到集市，发现曹元佩的人已经在等她，本章以她被迫交出一样贴身之物收束。";
        let plan4 = "本章大纲：阿苔顺着一枚铜钱追查断灯绳来处，在集市与曹元佩正面对峙；\
             她必须当着众人承认自己藏了信物，付出被逐出渡口的代价，换来苏会山留下的一条线索。";
        let mock = MockLlm::scripted(vec![plan4, plan5]);
        let coordinator = AgencyCoordinator::for_test(pool.clone(), mock.clone());

        let report = preview(&coordinator, &pool, &story_id, 4, REPLAN_MAX_CHAPTERS)
            .await
            .unwrap();
        assert_eq!(report.total, 2);
        assert_eq!(report.items.len(), 2);
        assert_eq!(report.items[0].chapter_number, 4);
        assert_eq!(report.items[0].old_plan, "本章：旧纲（甲与乙对峙）。");
        assert_eq!(report.items[0].new_plan.as_deref(), Some(plan4));
        assert!(report.items[0].skipped.is_none());
        // 预览不落库。
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        let ch4_row = scenes.iter().find(|s| s.id == ch4).unwrap();
        assert!(ch4_row
            .outline_content
            .as_deref()
            .unwrap_or("")
            .contains("旧纲（甲与乙对峙）"));

        // 只应用第 4 章 → 第 5 章仍未重规划，标记保留。
        let apply_items = vec![ApplyOutlineItem {
            scene_id: ch4.clone(),
            chapter_number: 4,
            plan: report.items[0].new_plan.clone().unwrap(),
        }];
        let apply_report = apply(&pool, &story_id, &apply_items).unwrap();
        assert_eq!(apply_report.applied, 1);
        assert_eq!(apply_report.remaining, 1, "第 5 章仍未处理");
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        let ch4_row = scenes.iter().find(|s| s.id == ch4).unwrap();
        assert!(ch4_row
            .outline_content
            .as_deref()
            .unwrap_or("")
            .contains("追查断灯绳来处"));
        assert!(
            ch4_row
                .outline_content
                .as_deref()
                .unwrap_or("")
                .contains("【当前场大纲】"),
            "卡块必须保留"
        );
        assert!(recompute::list_stale(&pool, &story_id)
            .unwrap()
            .iter()
            .any(|s| s.kind == KIND_CHAPTER_OUTLINE));

        // 应用第 5 章 → 全部完成，标记清除。
        let apply_items = vec![ApplyOutlineItem {
            scene_id: ch5.clone(),
            chapter_number: 5,
            plan: plan5.to_string(),
        }];
        let apply_report = apply(&pool, &story_id, &apply_items).unwrap();
        assert_eq!(apply_report.applied, 1);
        assert_eq!(apply_report.remaining, 0);
        assert!(recompute::list_stale(&pool, &story_id)
            .unwrap()
            .iter()
            .all(|s| s.kind != KIND_CHAPTER_OUTLINE));
    }

    #[test]
    fn apply_skips_mismatched_scene_and_empty_plan() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        let ch4 = seed_scene(&pool, &story_id, 4, Some("本章：旧纲。"));
        let report = apply(
            &pool,
            &story_id,
            &[
                ApplyOutlineItem {
                    scene_id: ch4.clone(),
                    chapter_number: 9, // 章号不符
                    plan: "本章：新纲。".into(),
                },
                ApplyOutlineItem {
                    scene_id: ch4.clone(),
                    chapter_number: 4,
                    plan: "   ".into(), // 空计划
                },
                ApplyOutlineItem {
                    scene_id: "missing".into(),
                    chapter_number: 4,
                    plan: "本章：新纲。".into(),
                },
            ],
        )
        .unwrap();
        assert_eq!(report.applied, 0);
        let scenes = SceneRepository::new(pool.clone())
            .get_by_story(&story_id)
            .unwrap();
        assert!(scenes[0]
            .outline_content
            .as_deref()
            .unwrap_or("")
            .contains("旧纲"));
    }
}
