//! Chapter commands

use tauri::{AppHandle, State};

use crate::{
    db::{ChapterRepository, CreateChapterRequest, DbPool, SceneRepository, SceneUpdate},
    error::AppError,
    story_system::chapter_service::ChapterService,
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
pub fn get_story_chapters_paged(
    story_id: String,
    limit: i64,
    offset: i64,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::Chapter>, AppError> {
    crate::db::ChapterRepository::new(pool.inner().clone())
        .get_by_story_paged(&story_id, limit, offset)
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
pub async fn update_chapter(
    id: String,
    title: Option<String>,
    outline: Option<String>,
    content: Option<String>,
    word_count: Option<i32>,
    pool: State<'_, DbPool>,
    app: AppHandle,
    automation_service: tauri::State<'_, crate::automation::service::AutomationService>,
    vector_store: State<'_, std::sync::Arc<dyn crate::ports::VectorStore>>,
) -> Result<(), AppError> {
    let pool = pool.inner().clone();
    let automation_service = automation_service.inner().clone();
    let vector_store = vector_store.inner().clone();

    // Phase 1: 章元数据走 ChapterRepository，内容走 SceneRepository（Scene 为真相源）
    let title_for_update = title.clone();
    let content_for_update = content.clone();
    let word_count_for_update = word_count.or_else(|| content.as_ref().map(|c| c.len() as i32));
    let chapter_info = tokio::task::spawn_blocking({
        let pool = pool.clone();
        let id = id.clone();
        move || {
            let chapter_repo = ChapterRepository::new(pool.clone());
            let scene_repo = SceneRepository::new(pool.clone());

            let info = chapter_repo.get_by_id(&id).ok().flatten();

            // 1. 更新章级元数据（不含 content）
            chapter_repo
                .update(&id, title_for_update, outline, word_count_for_update)
                .map_err(AppError::from)?;

            // 2. 如果提供了内容，写入关联的 Scene
            if let Some(ref c) = content_for_update {
                if let Ok(scenes) = scene_repo.get_by_chapter(&id) {
                    if let Some(scene) = scenes.first() {
                        scene_repo.update(
                            &scene.id,
                            &SceneUpdate {
                                content: Some(c.clone()),
                                ..Default::default()
                            },
                        )?;
                    }
                }
            }

            Ok::<_, AppError>(info)
        }
    })
    .await
    .map_err(|e| AppError::Internal {
        message: format!("spawn_blocking panicked: {}", e),
    })??;

    let story_id_opt = chapter_info.as_ref().map(|c| c.story_id.clone());
    let chapter_number = chapter_info.map(|c| c.chapter_number).unwrap_or(0);

    if let Some(ref story_id) = story_id_opt {
        let service = ChapterService::new(pool, app, vector_store);
        service.on_chapter_updated(
            &id,
            story_id,
            chapter_number,
            title,
            word_count,
            &automation_service,
        );
    }

    Ok(())
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
    automation_service: tauri::State<'_, crate::automation::service::AutomationService>,
    vector_store: State<'_, std::sync::Arc<dyn crate::ports::VectorStore>>,
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

    // 委托领域服务处理后续业务编排
    let service = ChapterService::new(pool, app, vector_store.inner().clone());
    service.on_chapter_created(&chapter, title, automation_service.inner());

    Ok(chapter)
}

/// Phase 4: 聚合章内所有 Scene 的内容为纯正文（无分隔符），
/// 供幕前编辑器加载使用。
#[tauri::command(rename_all = "snake_case")]
pub fn get_chapter_aggregated_content(
    chapter_id: String,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let repo = ChapterRepository::new(pool.inner().clone());
    repo.get_content(&chapter_id).map_err(AppError::from)
}
