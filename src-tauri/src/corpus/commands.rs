use tauri::{AppHandle, State};

use crate::{
    corpus::{
        models::{
            CorpusDetail, CorpusDistillation, CorpusMedia, CorpusPromotionDraft, CorpusSummary,
            CorpusUpdate,
        },
        service::CorpusService,
    },
    db::DbPool,
    error::AppError,
};

#[tauri::command(rename_all = "snake_case")]
pub fn corpus_list(
    story_id: String,
    kind: Option<String>,
    status: Option<String>,
    query: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
    pool: State<'_, DbPool>,
) -> Result<Vec<CorpusSummary>, AppError> {
    CorpusService::new(pool.inner().clone()).list(
        &story_id,
        kind.as_deref(),
        status.as_deref(),
        query.as_deref(),
        limit.unwrap_or(200),
        offset.unwrap_or(0),
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn corpus_get(id: String, pool: State<'_, DbPool>) -> Result<CorpusDetail, AppError> {
    CorpusService::new(pool.inner().clone()).get(&id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn corpus_create(
    story_id: String,
    kind: String,
    title: String,
    content: Option<String>,
    summary: Option<String>,
    source_url: Option<String>,
    source_name: Option<String>,
    tags: Option<Vec<String>>,
    pool: State<'_, DbPool>,
) -> Result<CorpusDetail, AppError> {
    CorpusService::new(pool.inner().clone()).create(
        &story_id,
        &kind,
        &title,
        content.as_deref(),
        summary.as_deref(),
        source_url.as_deref(),
        source_name.as_deref(),
        tags,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn corpus_update(
    id: String,
    updates: CorpusUpdate,
    pool: State<'_, DbPool>,
) -> Result<CorpusDetail, AppError> {
    CorpusService::new(pool.inner().clone()).update(&id, &updates)
}

#[tauri::command(rename_all = "snake_case")]
pub fn corpus_delete(
    id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    CorpusService::new(pool.inner().clone()).delete(&id, &app_handle)
}

#[tauri::command(rename_all = "snake_case")]
pub fn corpus_import_file(
    story_id: String,
    kind: String,
    file_path: String,
    title: Option<String>,
    tags: Option<Vec<String>>,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<CorpusDetail, AppError> {
    CorpusService::new(pool.inner().clone()).import_file(
        &story_id,
        &kind,
        &file_path,
        title.as_deref(),
        tags,
        &app_handle,
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn corpus_read_media(id: String, pool: State<'_, DbPool>) -> Result<CorpusMedia, AppError> {
    CorpusService::new(pool.inner().clone()).read_media(&id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn corpus_distill(
    id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<CorpusDistillation, AppError> {
    CorpusService::new(pool.inner().clone())
        .distill(&id, &app_handle)
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn corpus_promote(
    id: String,
    target_type: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<CorpusPromotionDraft, AppError> {
    CorpusService::new(pool.inner().clone())
        .promote(&id, &target_type, &app_handle)
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub fn corpus_link(
    corpus_id: String,
    target_type: String,
    target_id: String,
    note: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    CorpusService::new(pool.inner().clone()).link(
        &corpus_id,
        &target_type,
        &target_id,
        note.as_deref(),
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn corpus_unlink(link_id: String, pool: State<'_, DbPool>) -> Result<(), AppError> {
    CorpusService::new(pool.inner().clone()).unlink(&link_id)
}
