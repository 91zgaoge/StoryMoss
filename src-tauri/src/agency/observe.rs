//! 手写/粘贴正文的三角色观察编排。
//! 设计：docs/plans/2026-08-17-prose-observe-agency-design.md

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    agency::{
        beat_card::{compile_beat_card_located, SceneBeatCard, CURRENT_SCENE_OUTLINE_MARK},
        board::BlackboardService,
        budget::{AgencyBudget, DEFAULT_RUN_TOKEN_BUDGET},
        continue_loop::{
            beat_card_asset_projections, bg_done_detail, editor_qc_done_detail,
            emit_logged_activity, project_assets_to_run, run_asset_ingest, BgExit,
        },
        coordinator::{evaluate_gate_impl, AgencyLlm, GateOutcome},
        models::{AgencyRun, AgentRole, BoardItem, BoardZone, OBSERVE_PREMISE},
        repository::AgencyRepository,
        tool_loop::LoopLlm,
        tools::ToolRegistry,
    },
    db::{DbPool, SceneRepository},
    error::AppError,
};

pub const MIN_OBSERVE_DELTA_CHARS: usize = 200;

fn inflight_stories() -> Arc<Mutex<HashSet<String>>> {
    static MAP: OnceLock<Arc<Mutex<HashSet<String>>>> = OnceLock::new();
    MAP.get_or_init(|| Arc::new(Mutex::new(HashSet::new())))
        .clone()
}

fn try_begin_inflight(story_id: &str) -> bool {
    let map = inflight_stories();
    let mut g = map.lock().unwrap_or_else(|e| e.into_inner());
    g.insert(story_id.to_string())
}

fn end_inflight(story_id: &str) {
    let map = inflight_stories();
    let mut g = map.lock().unwrap_or_else(|e| e.into_inner());
    g.remove(story_id);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostCommitWork {
    Skip,
    Observe,
    Ingest,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObserveWatermark {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub by_scene: HashMap<String, usize>,
}

impl ObserveWatermark {
    pub fn last_chars(&self, scene_id: &str) -> usize {
        self.by_scene.get(scene_id).copied().unwrap_or(0)
    }

    pub fn record(&mut self, scene_id: &str, chars: usize) {
        self.kind = "observe".into();
        self.by_scene.insert(scene_id.to_string(), chars);
    }
}

pub fn parse_watermark(result_json: Option<&str>) -> ObserveWatermark {
    let Some(raw) = result_json.map(str::trim).filter(|s| !s.is_empty()) else {
        return ObserveWatermark::default();
    };
    serde_json::from_str(raw).unwrap_or_default()
}

pub fn should_observe(last_chars: usize, current_chars: usize) -> bool {
    current_chars >= MIN_OBSERVE_DELTA_CHARS
        && current_chars.saturating_sub(last_chars) >= MIN_OBSERVE_DELTA_CHARS
}

pub fn should_spawn_ingest_on_update(content_changed: bool, should_ingest: bool) -> bool {
    should_ingest && !content_changed
}

pub fn decide_post_commit_work(
    content_chars: usize,
    last_chars: usize,
    observe_inflight: bool,
    creative_running: bool,
) -> PostCommitWork {
    if content_chars == 0 {
        return PostCommitWork::Skip;
    }
    if observe_inflight {
        return PostCommitWork::Skip;
    }
    if creative_running {
        return PostCommitWork::Ingest;
    }
    if should_observe(last_chars, content_chars) {
        PostCommitWork::Observe
    } else {
        PostCommitWork::Ingest
    }
}

pub fn observe_run_id(story_id: &str) -> String {
    format!("observe-{story_id}")
}

pub fn merge_current_scene_outline(existing: Option<&str>, block: &str) -> String {
    let block = block.trim();
    let Some(existing) = existing.map(str::trim).filter(|s| !s.is_empty()) else {
        return block.to_string();
    };
    if let Some(idx) = existing.find(CURRENT_SCENE_OUTLINE_MARK) {
        let prefix = existing[..idx].trim_end();
        if prefix.is_empty() {
            block.to_string()
        } else {
            format!("{prefix}\n\n{block}")
        }
    } else {
        format!("{existing}\n\n{block}")
    }
}

/// v0.64.9：写入/替换「章纲前缀」，保留既有节拍卡块。
///
/// 幕前 Append 路径没有装配步骤（NextChapter 才有），生成的章纲此前无处落库，
/// 于是每一拍都在无章纲状态下续写。这里把它落到 `scenes.outline_content`
/// 前缀位（卡块之后由 `merge_current_scene_outline` 常规维护）。
pub fn set_chapter_plan_prefix(existing: Option<&str>, plan: &str) -> String {
    let plan = plan.trim();
    if plan.is_empty() {
        return existing.map(str::trim).unwrap_or("").to_string();
    }
    let block = existing
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|e| e.find(CURRENT_SCENE_OUTLINE_MARK).map(|i| e[i..].trim()))
        .unwrap_or("");
    if block.is_empty() {
        plan.to_string()
    } else {
        format!("{plan}\n\n{block}")
    }
}

pub fn apply_observe_writer(
    pool: &DbPool,
    run_id: &str,
    story_id: &str,
    scene_id: &str,
    content: &str,
) -> Result<SceneBeatCard, AppError> {
    let scene_repo = SceneRepository::new(pool.clone());
    let scene = scene_repo
        .get_by_id(scene_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::from("观察：场景不存在"))?;
    let loc = scene.setting_location.clone();
    let card = compile_beat_card_located(pool, story_id, content, loc.as_deref())?;
    let outline = merge_current_scene_outline(
        scene.outline_content.as_deref(),
        &card.render_scene_outline(),
    );
    let names: Vec<String> = card
        .cast
        .iter()
        .map(|c| c.name.clone())
        .filter(|n| !n.trim().is_empty())
        .collect();
    scene_repo
        .update(
            scene_id,
            &crate::db::repositories::SceneUpdate {
                outline_content: Some(outline.clone()),
                characters_present: if names.is_empty() {
                    None
                } else {
                    Some(names.clone())
                },
                setting_location: card.setting_location.clone(),
                source: Some("agency".into()),
                ..Default::default()
            },
        )
        .map_err(AppError::from)?;
    let projections =
        beat_card_asset_projections(&names, &outline, card.setting_location.as_deref());
    project_assets_to_run(pool, run_id, story_id, &projections);
    Ok(card)
}

fn find_or_create_observe_run(pool: &DbPool, story_id: &str) -> Result<AgencyRun, AppError> {
    let repo = AgencyRepository::new(pool.clone());
    if let Some(existing) = repo.find_observe_run(story_id).map_err(AppError::from)? {
        return Ok(existing);
    }
    let mut run = AgencyRun::new(observe_run_id(story_id), OBSERVE_PREMISE);
    run.story_id = Some(story_id.to_string());
    run.status = "idle".into();
    run.phase = "observe".into();
    if let Err(e) = repo.create_run(&run) {
        if let Ok(Some(again)) = repo.find_observe_run(story_id) {
            return Ok(again);
        }
        if let Ok(Some(by_id)) = repo.get_run(&run.id) {
            return Ok(by_id);
        }
        return Err(AppError::from(format!("观察 run 创建失败: {e}")));
    }
    Ok(run)
}

pub fn lookup_post_commit_work(
    pool: &DbPool,
    story_id: &str,
    scene_id: &str,
    content_chars: usize,
) -> PostCommitWork {
    let inflight = {
        let map = inflight_stories();
        let g = map.lock().unwrap_or_else(|e| e.into_inner());
        g.contains(story_id)
    };
    let repo = AgencyRepository::new(pool.clone());
    let run = repo.find_observe_run(story_id).ok().flatten();
    let last =
        parse_watermark(run.as_ref().and_then(|r| r.result_json.as_deref())).last_chars(scene_id);
    let creative = repo.has_blocking_creative_run(story_id).unwrap_or(false);
    decide_post_commit_work(content_chars, last, inflight, creative)
}

/// 保存路径 content 变更后，与自动分章同一 30s 窗口在 Observe 分支调用。
pub fn spawn_observe_run(
    app: AppHandle,
    pool: DbPool,
    story_id: String,
    scene_id: String,
    content: String,
) {
    tauri::async_runtime::spawn(async move {
        if !try_begin_inflight(&story_id) {
            return;
        }
        run_observe(app, pool, story_id.clone(), scene_id, content).await;
        end_inflight(&story_id);
    });
}

async fn run_observe(
    app: AppHandle,
    pool: DbPool,
    story_id: String,
    scene_id: String,
    content: String,
) {
    let chars = content.chars().count();
    let setup = {
        let pool = pool.clone();
        let story_id = story_id.clone();
        tokio::task::spawn_blocking(move || find_or_create_observe_run(&pool, &story_id)).await
    };
    let Ok(Ok(run)) = setup else {
        log::warn!("agency: 观察 run 准备失败 story={story_id}");
        return;
    };
    let run_id = run.id.clone();
    let mut watermark = parse_watermark(run.result_json.as_deref());
    {
        let repo = AgencyRepository::new(pool.clone());
        let json = serde_json::to_string(&watermark).ok();
        let _ = repo.save_observe_state(&run_id, "observing", json.as_deref());
    }

    let editor = spawn_observe_editor(
        app.clone(),
        pool.clone(),
        run_id.clone(),
        story_id.clone(),
        content.clone(),
    );

    emit_logged_activity(
        &app,
        &pool,
        &run_id,
        AgentRole::Producer,
        "start",
        "资产回流",
    )
    .await;
    let producer_exit =
        run_asset_ingest(&app, &pool, &run_id, &story_id, &scene_id, &content, None).await;
    emit_logged_activity(
        &app,
        &pool,
        &run_id,
        AgentRole::Producer,
        "done",
        &bg_done_detail("资产回流", producer_exit),
    )
    .await;

    emit_logged_activity(
        &app,
        &pool,
        &run_id,
        AgentRole::LeadWriter,
        "start",
        "编译节拍",
    )
    .await;
    let writer_ok = {
        let pool = pool.clone();
        let run_id = run_id.clone();
        let story_id = story_id.clone();
        let scene_id = scene_id.clone();
        let content = content.clone();
        tokio::task::spawn_blocking(move || {
            apply_observe_writer(&pool, &run_id, &story_id, &scene_id, &content)
        })
        .await
    };
    let writer_exit = match writer_ok {
        Ok(Ok(_)) => BgExit::Success,
        Ok(Err(e)) => {
            log::warn!("agency: 观察主创编译失败 (run={run_id}): {e}");
            BgExit::Failed
        }
        Err(e) => {
            log::warn!("agency: 观察主创编译 join 失败 (run={run_id}): {e}");
            BgExit::Failed
        }
    };
    emit_logged_activity(
        &app,
        &pool,
        &run_id,
        AgentRole::LeadWriter,
        "done",
        &bg_done_detail("编译节拍", writer_exit),
    )
    .await;

    let _ = editor.await;
    watermark.record(&scene_id, chars);
    let json = serde_json::to_string(&watermark).ok();
    let repo = AgencyRepository::new(pool);
    let _ = repo.save_observe_state(&run_id, "idle", json.as_deref());
}

fn spawn_observe_editor(
    app: AppHandle,
    pool: DbPool,
    run_id: String,
    story_id: String,
    content: String,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        emit_logged_activity(
            &app,
            &pool,
            &run_id,
            AgentRole::EditorAuditor,
            "start",
            "后台审查",
        )
        .await;
        let draft = BoardItem::new(
            &run_id,
            &story_id,
            BoardZone::Draft,
            "prose",
            "observe-draft",
            content,
            "观察正文",
            AgentRole::LeadWriter,
            "active",
        );
        let deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs(300));
        let llm: Arc<dyn LoopLlm> = Arc::new(
            AgencyLlm::new(
                app.clone(),
                run_id.clone(),
                AgentRole::EditorAuditor,
                story_id.clone(),
            )
            .with_label("bg-observe-editor"),
        );
        let budget = Arc::new(AgencyBudget::new(DEFAULT_RUN_TOKEN_BUDGET));
        let board = BlackboardService::with_events(pool.clone(), &app);
        let registry = Arc::new(ToolRegistry::agency_default());
        let result = evaluate_gate_impl(
            &llm,
            &budget,
            &pool,
            &board,
            &registry,
            &run_id,
            &story_id,
            OBSERVE_PREMISE,
            &draft,
            1,
            deadline,
        )
        .await;
        match result {
            Ok((GateOutcome::Passed { .. }, _)) | Ok((GateOutcome::RevisionRequired { .. }, _)) => {
            }
            Ok((GateOutcome::Failed { reason }, _)) => {
                log::warn!("agency: 观察编辑审查未通过 (run={run_id}): {reason}");
            }
            Err(e) => {
                log::warn!("agency: 观察编辑审查异常 (run={run_id}): {e}");
            }
        }
        emit_logged_activity(
            &app,
            &pool,
            &run_id,
            AgentRole::EditorAuditor,
            "done",
            &editor_qc_done_detail(),
        )
        .await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connection::create_test_pool, dto::CreateStoryRequest, StoryRepository};

    fn seed_story_scene(pool: &DbPool) -> (String, String) {
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "观察测试".into(),
                description: None,
                genre: Some("玄幻".into()),
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
                &crate::db::repositories::SceneUpdate {
                    content: Some("阿苔走进废土。".into()),
                    outline_content: Some("用户手写大纲前缀".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        tx.commit().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, source, \
             is_auto_generated, created_at, updated_at) VALUES ('c-obs', ?1, '阿苔', '拾荒者', \
             '坚韧', '找到星环', 'agency', 1, '2026-01-01', '2026-01-01')",
            rusqlite::params![story.id],
        )
        .unwrap();
        (story.id, scene.id)
    }

    #[test]
    fn should_observe_requires_two_hundred_char_growth() {
        assert!(!should_observe(0, 199));
        assert!(should_observe(0, 200));
        assert!(!should_observe(500, 650));
        assert!(should_observe(500, 700));
    }

    #[test]
    fn watermark_is_per_scene() {
        let mut w = ObserveWatermark::default();
        w.record("s1", 800);
        w.record("s2", 100);
        assert!(!should_observe(w.last_chars("s1"), 900));
        assert!(should_observe(w.last_chars("s2"), 400));
    }

    #[test]
    fn decide_yields_to_creative_and_inflight() {
        assert_eq!(
            decide_post_commit_work(400, 0, false, true),
            PostCommitWork::Ingest
        );
        assert_eq!(
            decide_post_commit_work(400, 0, true, false),
            PostCommitWork::Skip
        );
        assert_eq!(
            decide_post_commit_work(400, 0, false, false),
            PostCommitWork::Observe
        );
        assert_eq!(
            decide_post_commit_work(50, 0, false, false),
            PostCommitWork::Ingest
        );
    }

    #[test]
    fn should_spawn_ingest_on_update_skips_content_path() {
        assert!(!should_spawn_ingest_on_update(true, true));
        assert!(should_spawn_ingest_on_update(false, true));
        assert!(!should_spawn_ingest_on_update(false, false));
    }

    #[test]
    fn merge_current_scene_outline_keeps_handwritten_prefix() {
        let block = format!("{CURRENT_SCENE_OUTLINE_MARK}\n在场：阿苔\n下一拍：禁区");
        assert_eq!(merge_current_scene_outline(None, &block), block);
        let merged = merge_current_scene_outline(Some("用户手写大纲前缀"), &block);
        assert!(merged.starts_with("用户手写大纲前缀"));
        assert!(merged.contains(CURRENT_SCENE_OUTLINE_MARK));
        let replaced = merge_current_scene_outline(
            Some(&format!("前缀\n\n{CURRENT_SCENE_OUTLINE_MARK}\n旧块")),
            &block,
        );
        assert!(replaced.starts_with("前缀"));
        assert!(!replaced.contains("旧块"));
        assert!(replaced.contains("禁区"));
    }

    #[test]
    fn apply_observe_writer_writes_outline_not_prose_or_beats() {
        let pool = create_test_pool().unwrap();
        let (story_id, scene_id) = seed_story_scene(&pool);
        let prose = format!("阿苔走进废土。{}", "风沙打在脸上。".repeat(40));
        SceneRepository::new(pool.clone())
            .update(
                &scene_id,
                &crate::db::repositories::SceneUpdate {
                    content: Some(prose.clone()),
                    ..Default::default()
                },
            )
            .unwrap();
        let run = find_or_create_observe_run(&pool, &story_id).unwrap();
        apply_observe_writer(&pool, &run.id, &story_id, &scene_id, &prose).unwrap();
        let scene = SceneRepository::new(pool.clone())
            .get_by_id(&scene_id)
            .unwrap()
            .unwrap();
        assert_eq!(scene.content.as_deref(), Some(prose.as_str()));
        let outline = scene.outline_content.unwrap_or_default();
        assert!(outline.contains("用户手写大纲前缀"));
        assert!(outline.contains(CURRENT_SCENE_OUTLINE_MARK));
        let conn = pool.get().unwrap();
        let beats = crate::creative_engine::expansion::read_beat_counters(&conn, &story_id);
        assert_eq!(beats.append_beats, 0);
        let again = find_or_create_observe_run(&pool, &story_id).unwrap();
        assert_eq!(again.id, run.id);
        assert_eq!(again.premise, OBSERVE_PREMISE);
    }

    #[test]
    fn has_blocking_creative_run_ignores_observe() {
        let pool = create_test_pool().unwrap();
        let (story_id, _) = seed_story_scene(&pool);
        let repo = AgencyRepository::new(pool.clone());
        assert!(!repo.has_blocking_creative_run(&story_id).unwrap());
        let _ = find_or_create_observe_run(&pool, &story_id).unwrap();
        repo.save_observe_state(&observe_run_id(&story_id), "observing", None)
            .unwrap();
        assert!(
            !repo.has_blocking_creative_run(&story_id).unwrap(),
            "观察 observing 不得挡住自己"
        );
        let mut cont = AgencyRun::new("cont-1", "续写");
        cont.story_id = Some(story_id.clone());
        cont.status = "running".into();
        repo.create_run(&cont).unwrap();
        assert!(repo.has_blocking_creative_run(&story_id).unwrap());
    }

    #[test]
    fn has_blocking_includes_pending_genesis() {
        let pool = create_test_pool().unwrap();
        let (story_id, _) = seed_story_scene(&pool);
        let repo = AgencyRepository::new(pool.clone());
        let mut gen = AgencyRun::new("gen-1", "创世");
        gen.story_id = Some(story_id.clone());
        gen.status = "pending".into();
        repo.create_run(&gen).unwrap();
        assert!(repo.has_blocking_creative_run(&story_id).unwrap());
    }

    /// v0.64.9：章纲写入前缀位，既有卡块保留（Append 路径没有装配步骤）。
    #[test]
    fn set_chapter_plan_prefix_keeps_card_block() {
        let block = format!("{CURRENT_SCENE_OUTLINE_MARK}\n在场：甲\n下一拍：夜宴破裂");
        let merged = set_chapter_plan_prefix(Some(&block), "本章：甲与乙决裂。");
        assert!(merged.starts_with("本章：甲与乙决裂。"), "{merged}");
        assert!(merged.contains(CURRENT_SCENE_OUTLINE_MARK), "{merged}");
        assert!(merged.contains("下一拍：夜宴破裂"), "{merged}");

        // 已有章纲 → 替换为新章纲，不重复堆叠
        let twice = set_chapter_plan_prefix(Some(&merged), "本章：新的方向。");
        assert!(twice.starts_with("本章：新的方向。"), "{twice}");
        assert_eq!(twice.matches("本章：").count(), 1, "{twice}");

        // 空计划：原样返回
        assert_eq!(set_chapter_plan_prefix(Some(&block), "  "), block);
    }
}
