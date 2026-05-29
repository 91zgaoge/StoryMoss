//! Story commands

use tauri::{AppHandle, State};

use crate::{
    db::{CreateStoryRequest, DbPool, StoryRepository},
    error::AppError,
};

#[tauri::command(rename_all = "snake_case")]
pub fn list_stories(pool: State<'_, DbPool>) -> Result<Vec<crate::db::Story>, AppError> {
    StoryRepository::new(pool.inner().clone())
        .get_all()
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_story(
    title: String,
    description: Option<String>,
    genre: Option<String>,
    pool: State<'_, DbPool>,
    app: AppHandle,
    automation_service: tauri::State<crate::automation::service::AutomationService>,
) -> Result<crate::db::Story, AppError> {
    let story = StoryRepository::new(pool.inner().clone())
        .create(CreateStoryRequest {
            title,
            description,
            genre,
            style_dna_id: None,
        })
        .map_err(AppError::from)?;
    let _ = crate::state_sync::StateSync::emit_story_created(&app, &story.id, &story.title);
    let automation_service_clone = automation_service.inner().clone();
    let story_id_clone = story.id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = automation_service_clone
            .trigger_event(crate::automation::triggers::TriggerEvent::StoryCreated {
                story_id: story_id_clone,
            })
            .await
        {
            log::warn!(
                "[create_story] Failed to trigger story created automation: {}",
                e
            );
        }
    });
    Ok(story)
}

#[tauri::command(rename_all = "snake_case")]
pub fn update_story(
    id: String,
    title: Option<String>,
    description: Option<String>,
    genre: Option<String>,
    tone: Option<String>,
    pacing: Option<String>,
    style_dna_id: Option<String>,
    methodology_id: Option<String>,
    methodology_step: Option<i32>,
    pool: State<'_, DbPool>,
    app: AppHandle,
) -> Result<(), AppError> {
    let req = crate::db::UpdateStoryRequest {
        title: title.clone(),
        description: description.clone(),
        genre,
        tone,
        pacing,
        style_dna_id,
        methodology_id,
        methodology_step,
    };
    StoryRepository::new(pool.inner().clone())
        .update(&id, &req)
        .map_err(AppError::from)?;
    let _ = crate::state_sync::StateSync::emit_story_updated(&app, &id, title.as_deref());
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_story(id: String, pool: State<'_, DbPool>, app: AppHandle) -> Result<(), AppError> {
    StoryRepository::new(pool.inner().clone())
        .delete(&id)
        .map_err(AppError::from)?;
    let _ = crate::state_sync::StateSync::emit_story_deleted(&app, &id);
    Ok(())
}
