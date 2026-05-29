//! Chapter commands

use std::time::{Duration, Instant};

use tauri::{AppHandle, State};

use crate::{
    db::{ChapterRepository, CreateChapterRequest, DbPool, SceneRepository},
    error::AppError,
    CHAPTER_COMMIT_DEBOUNCE, CHAPTER_COMMIT_DEBOUNCE_SECONDS, SKILL_MANAGER, VECTOR_STORE,
};

#[tauri::command(rename_all = "snake_case")]
pub fn get_story_chapters(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::Chapter>, AppError> {
    crate::db::ChapterRepository::new(pool.inner().clone())
        .get_by_story(&story_id)
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_chapter(
    id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<crate::db::Chapter>, AppError> {
    crate::db::ChapterRepository::new(pool.inner().clone())
        .get_by_id(&id)
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn update_chapter(
    id: String,
    title: Option<String>,
    outline: Option<String>,
    content: Option<String>,
    word_count: Option<i32>,
    pool: State<'_, DbPool>,
    app: AppHandle,
    automation_service: tauri::State<crate::automation::service::AutomationService>,
) -> Result<(), AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::ChapterRepository::new(pool.clone());
    // 先查询 story_id 和 chapter_number（P0-3 修复: 避免 unwrap_or_default
    // 导致空字符串）
    let chapter_info = repo.get_by_id(&id).ok().flatten();
    let story_id_opt = chapter_info.as_ref().map(|c| c.story_id.clone());
    let chapter_number = chapter_info.map(|c| c.chapter_number).unwrap_or(0);
    let result = repo
        .update(&id, title.clone(), outline, content, word_count)
        .map_err(AppError::from);
    if result.is_ok() {
        let _ = crate::window::WindowManager::send_to_frontstage(
            &app,
            crate::window::FrontstageEvent::SaveStatus {
                saved: true,
                timestamp: Some(chrono::Local::now().to_rfc3339()),
            },
        );
        if let Some(ref story_id) = story_id_opt {
            let _ = crate::state_sync::StateSync::emit_chapter_updated(
                &app,
                &id,
                title.as_deref(),
                story_id,
            );
            let automation_service_clone = automation_service.inner().clone();
            let story_id_clone = story_id.clone();
            let chapter_id_clone = id.clone();
            let word_count_val = word_count.unwrap_or(0) as usize;
            tauri::async_runtime::spawn(async move {
                if let Err(e) = automation_service_clone
                    .trigger_event(
                        crate::automation::triggers::TriggerEvent::ChapterContentUpdated {
                            story_id: story_id_clone,
                            chapter_id: chapter_id_clone,
                            word_count: word_count_val,
                        },
                    )
                    .await
                {
                    log::warn!(
                        "[update_chapter] Failed to trigger chapter content updated automation: {}",
                        e
                    );
                }
            });

            // P1 修复: 章节保存后检查逾期伏笔
            let story_id_for_payoff = story_id.clone();
            let chapter_number_for_payoff = chapter_number;
            let app_handle_for_payoff = app.clone();
            let pool_for_payoff = pool.clone();
            tauri::async_runtime::spawn(async move {
                let ledger =
                    crate::creative_engine::payoff_ledger::PayoffLedger::new(pool_for_payoff);
                match ledger.detect_overdue(&story_id_for_payoff, chapter_number_for_payoff) {
                    Ok(overdue) if !overdue.is_empty() => {
                        log::info!(
                            "[PayoffLedger] {} overdue payoffs detected for story {}",
                            overdue.len(),
                            story_id_for_payoff
                        );
                        let _ = crate::state_sync::StateSync::emit_payoff_overdue(
                            &app_handle_for_payoff,
                            &story_id_for_payoff,
                            &overdue,
                        );
                    }
                    _ => {}
                }
            });
        }
        // W4-B9: 保存后自动触发 chapter commit（30s debounce），替代独立的 auto_ingest
        // apply_commit 已吸收 vector + kg ingest 功能，避免重复工作
        let chapter_id = id.clone();
        let app_handle = app.clone();
        let scheduled_time = Instant::now();
        {
            let mut debounce = CHAPTER_COMMIT_DEBOUNCE.lock().unwrap();
            debounce.insert(chapter_id.clone(), scheduled_time);
            // 清理超过 24 小时的过期条目，防止内存泄漏
            const MAX_DEBOUNCE_AGE_HOURS: u64 = 24;
            debounce.retain(|_, last_time| {
                Instant::now().duration_since(*last_time)
                    < Duration::from_secs(MAX_DEBOUNCE_AGE_HOURS * 3600)
            });
        }
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(CHAPTER_COMMIT_DEBOUNCE_SECONDS)).await;
            let should_commit = {
                let debounce = CHAPTER_COMMIT_DEBOUNCE.lock().unwrap();
                debounce
                    .get(&chapter_id)
                    .map(|t| *t == scheduled_time)
                    .unwrap_or(false)
            };
            if should_commit {
                let repo = crate::db::ChapterRepository::new(pool.clone());
                let scene_repo = crate::db::SceneRepository::new(pool.clone());
                if let Ok(Some(chapter)) = repo.get_by_id(&chapter_id) {
                    let scene_id = scene_repo
                        .get_by_chapter(&chapter_id)
                        .ok()
                        .and_then(|scenes| scenes.into_iter().next())
                        .map(|s| s.id);
                    let service = crate::story_system::SceneCommitService::new(pool);
                    let store = VECTOR_STORE.get();
                    if let Err(e) = service
                        .auto_commit(
                            &chapter.story_id,
                            scene_id.as_deref(),
                            Some(&chapter_id),
                            chapter.chapter_number,
                            chapter.content.as_deref(),
                            Some(app_handle),
                            store,
                        )
                        .await
                    {
                        log::warn!(
                            "[update_chapter] auto_commit failed for chapter {}: {}",
                            chapter_id,
                            e
                        );
                    }
                }
            } else {
                log::info!(
                    "[update_chapter] auto_commit skipped for chapter {} (debounced)",
                    chapter_id
                );
            }
        });
    }
    result.map(|_| ())
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_chapter(id: String, pool: State<'_, DbPool>, app: AppHandle) -> Result<(), AppError> {
    let repo = crate::db::ChapterRepository::new(pool.inner().clone());
    // 先查询 story_id，删除后无法再获取（P0-3 修复: 避免 unwrap_or_default
    // 导致空字符串）
    let story_id_opt = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id);
    repo.delete(&id).map_err(AppError::from)?;
    if let Some(story_id) = story_id_opt {
        let _ = crate::state_sync::StateSync::emit_chapter_deleted(&app, &id, &story_id);
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_chapter(
    story_id: String,
    chapter_number: i32,
    title: Option<String>,
    outline: Option<String>,
    content: Option<String>,
    pool: State<'_, DbPool>,
    app: AppHandle,
    automation_service: tauri::State<crate::automation::service::AutomationService>,
) -> Result<crate::db::Chapter, AppError> {
    let pool = pool.inner().clone();
    let repo = ChapterRepository::new(pool.clone());

    // 如果该 chapter_number 已存在，直接返回已有章节（幂等）
    if let Ok(chapters) = repo.get_by_story(&story_id) {
        if let Some(existing) = chapters
            .into_iter()
            .find(|c| c.chapter_number == chapter_number)
        {
            log::info!(
                "[create_chapter] Chapter {} already exists for story {}, returning existing",
                chapter_number,
                story_id
            );
            return Ok(existing);
        }
    }

    let req = CreateChapterRequest {
        story_id: story_id.clone(),
        chapter_number,
        title: title.clone(),
        outline,
        content,
    };
    let chapter = repo.create(req).map_err(AppError::from)?;

    // AfterChapterSave hook
    if let Some(manager) = SKILL_MANAGER.get() {
        if let Ok(skill_manager) = manager.lock() {
            let story_id = chapter.story_id.clone();
            let chapter_id = chapter.id.clone();
            let chapter_number = chapter.chapter_number;
            let skill_manager = skill_manager.clone();
            tauri::async_runtime::spawn(async move {
                let context = crate::agents::AgentContext::minimal(story_id, String::new());
                let data = serde_json::json!({ "chapter_id": chapter_id, "chapter_number": chapter_number });
                let _ = skill_manager
                    .execute_hooks(crate::skills::HookEvent::AfterChapterSave, &context, data)
                    .await;
                log::info!(
                    "Hook executed: {:?}",
                    crate::skills::HookEvent::AfterChapterSave
                );
            });
        }
    }

    let _ = crate::state_sync::StateSync::emit_chapter_created(
        &app,
        &story_id,
        &chapter.id,
        title.as_deref(),
    );
    // 同时发射 Scene 更新事件（chapter 创建会自动创建/关联 scene）
    {
        let scene_repo = SceneRepository::new(pool.clone());
        if let Ok(scenes) = scene_repo.get_by_chapter(&chapter.id) {
            if let Some(scene) = scenes.first() {
                let _ = crate::state_sync::StateSync::emit_scene_updated(
                    &app,
                    &story_id,
                    &scene.id,
                    title.as_deref(),
                );
            }
        }
    }

    // P1 修复: 新建章节后检查逾期伏笔
    {
        let story_id_for_payoff = story_id.clone();
        let chapter_number_for_payoff = chapter.chapter_number;
        let app_handle_for_payoff = app.clone();
        let pool_for_payoff = pool.clone();
        tauri::async_runtime::spawn(async move {
            let ledger = crate::creative_engine::payoff_ledger::PayoffLedger::new(pool_for_payoff);
            match ledger.detect_overdue(&story_id_for_payoff, chapter_number_for_payoff) {
                Ok(overdue) if !overdue.is_empty() => {
                    log::info!(
                        "[PayoffLedger] {} overdue payoffs detected for story {} on chapter \
                         creation",
                        overdue.len(),
                        story_id_for_payoff
                    );
                    let _ = crate::state_sync::StateSync::emit_payoff_overdue(
                        &app_handle_for_payoff,
                        &story_id_for_payoff,
                        &overdue,
                    );
                }
                _ => {}
            }
        });
    }

    // P0-3 修复: 触发自动化事件：章节创建完成
    let automation_service_clone = automation_service.inner().clone();
    let story_id_clone = story_id.clone();
    let chapter_id_clone = chapter.id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = automation_service_clone
            .trigger_event(crate::automation::triggers::TriggerEvent::ChapterCreated {
                story_id: story_id_clone,
                chapter_id: chapter_id_clone,
            })
            .await
        {
            log::warn!(
                "[create_chapter] Failed to trigger chapter created automation: {}",
                e
            );
        }
    });

    // W4-B9: 新建章节后自动触发 chapter commit（无需 debounce，首次创建只执行一次）
    // apply_commit 已吸收 vector + kg ingest 功能，避免重复工作
    let scene_id_for_commit = {
        let scene_repo = SceneRepository::new(pool.clone());
        scene_repo
            .get_by_chapter(&chapter.id)
            .ok()
            .and_then(|scenes| scenes.into_iter().next())
            .map(|s| s.id)
    };
    {
        let chapter_id = chapter.id.clone();
        let app_handle = app.clone();
        let story_id = chapter.story_id.clone();
        let chapter_number = chapter.chapter_number;
        let content = chapter.content.clone();
        tauri::async_runtime::spawn(async move {
            let service = crate::story_system::SceneCommitService::new(pool);
            let store = VECTOR_STORE.get();
            if let Err(e) = service
                .auto_commit(
                    &story_id,
                    scene_id_for_commit.as_deref(),
                    Some(&chapter_id),
                    chapter_number,
                    content.as_deref(),
                    Some(app_handle),
                    store,
                )
                .await
            {
                log::warn!(
                    "[create_chapter] auto_commit failed for chapter {}: {}",
                    chapter_id,
                    e
                );
            }
        });
    }
    Ok(chapter)
}
