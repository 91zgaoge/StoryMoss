//! Workspace IPC 命令

use std::collections::HashMap;

use tauri::{AppHandle, State};

use crate::{db::DbPool, error::AppError, workspace::WorkspaceService};

#[tauri::command(rename_all = "snake_case")]
pub async fn get_workspace_files(
    story_id: String,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<HashMap<String, String>, AppError> {
    let svc = WorkspaceService::new(&app, pool.inner().clone())?;
    svc.get_all_files(&story_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn sync_workspace_memory(
    story_id: String,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let svc = WorkspaceService::new(&app, pool.inner().clone())?;
    svc.sync_memory(&story_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_workspace_file(
    story_id: String,
    filename: String,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let svc = WorkspaceService::new(&app, pool.inner().clone())?;
    svc.get_file(&story_id, &filename).await
}
