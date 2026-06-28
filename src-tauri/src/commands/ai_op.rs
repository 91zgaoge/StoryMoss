//! Ai Op commands

use tauri::{AppHandle, Emitter, State};

use crate::{
    db::{DbPool, SceneRepository, SceneUpdate},
    error::AppError,
};

#[tauri::command(rename_all = "snake_case")]
pub async fn list_ai_operations(
    pool: State<'_, DbPool>,
    story_id: String,
) -> Result<Vec<crate::db::AiOperation>, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::AiOperationRepository::new(pool);
    repo.get_by_story(&story_id).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn rollback_ai_operation(
    pool: State<'_, DbPool>,
    operation_id: String,
    app: AppHandle,
) -> Result<(), AppError> {
    let pool = pool.inner().clone();
    let op_repo = crate::db::AiOperationRepository::new(pool.clone());

    let operation = op_repo
        .get_by_id(&operation_id)
        .map_err(AppError::from)?
        .ok_or("Operation not found")?;

    // Only support rollback for chapter content operations that have
    // previous_content
    let prev_content = operation.previous_content.ok_or("此操作不支持回滚")?;

    // Phase 1: 回滚内容恢复走 SceneRepository（Scene 为真相源）
    let chapter_id = operation.chapter_id.ok_or("此操作没有关联章节")?;

    {
        let scene_repo = SceneRepository::new(pool.clone());
        if let Ok(scenes) = scene_repo.get_by_chapter(&chapter_id) {
            if let Some(scene) = scenes.first() {
                scene_repo
                    .update(
                        &scene.id,
                        &SceneUpdate {
                            content: Some(prev_content),
                            ..Default::default()
                        },
                    )
                    .map_err(AppError::from)?;
            }
        }
    }

    // Mark operation as rolled back
    op_repo
        .update_status(&operation_id, "rolled_back")
        .map_err(AppError::from)?;

    // Emit sync event
    let _ = app.emit(
        "sync-event",
        serde_json::json!({
            "event": "chapterUpdated",
            "chapter_id": chapter_id,
            "story_id": operation.story_id,
        }),
    );

    Ok(())
}
