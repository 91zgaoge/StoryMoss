//! Story System commands

use tauri::{AppHandle, State};

use crate::{db::DbPool, error::AppError};

// ==================== Story System Commands ====================

#[tauri::command(rename_all = "snake_case")]
pub fn create_master_setting(
    story_id: String,
    genre: String,
    core_tone: String,
    pacing_strategy: String,
    anti_patterns: Vec<String>,
    world_rules: Vec<String>,
    pool: State<'_, DbPool>,
    app: AppHandle,
) -> Result<crate::db::StoryContract, AppError> {
    let pool = pool.inner().clone();
    let engine = crate::story_system::StorySystemEngine::new(pool);
    let result = engine
        .create_master_setting(
            &story_id,
            &genre,
            &core_tone,
            &pacing_strategy,
            &anti_patterns,
            &world_rules,
        )
        .map_err(AppError::from)?;
    let _ =
        crate::state_sync::StateSync::emit_data_refresh(&app, Some(&story_id), "storyContracts");
    Ok(result)
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_chapter_contract(
    story_id: String,
    chapter_number: i32,
    goal: String,
    must_cover_nodes: Vec<String>,
    forbidden_zones: Vec<String>,
    time_anchor: Option<String>,
    chapter_span: Option<String>,
    pool: State<'_, DbPool>,
    app: AppHandle,
) -> Result<crate::db::StoryContract, AppError> {
    let pool = pool.inner().clone();
    let engine = crate::story_system::StorySystemEngine::new(pool);
    let result = engine
        .create_chapter_contract(
            &story_id,
            chapter_number,
            &goal,
            &must_cover_nodes,
            &forbidden_zones,
            time_anchor.as_deref(),
            chapter_span.as_deref(),
        )
        .map_err(AppError::from)?;
    let _ =
        crate::state_sync::StateSync::emit_data_refresh(&app, Some(&story_id), "storyContracts");
    Ok(result)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_contract_tree(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<crate::story_system::ContractTree, AppError> {
    let pool = pool.inner().clone();
    let engine = crate::story_system::StorySystemEngine::new(pool);
    engine.get_contract_tree(&story_id).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_runtime_contract(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
) -> Result<crate::story_system::RuntimeContract, AppError> {
    let pool = pool.inner().clone();
    let engine = crate::story_system::StorySystemEngine::new(pool);
    engine
        .get_runtime_contract(&story_id, chapter_number)
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn init_chapter_commit(
    story_id: String,
    scene_id: Option<String>,
    chapter_id: Option<String>,
    chapter_number: i32,
    pool: State<'_, DbPool>,
    app: AppHandle,
) -> Result<crate::db::SceneCommit, AppError> {
    let pool = pool.inner().clone();
    let service = crate::story_system::SceneCommitService::new(pool);
    let result = service
        .init_commit(
            &story_id,
            scene_id.as_deref(),
            chapter_id.as_deref(),
            chapter_number,
        )
        .map_err(AppError::from)?;
    let _ = crate::state_sync::StateSync::emit_data_refresh(&app, Some(&story_id), "sceneCommits");
    Ok(result)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_chapter_commits(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::SceneCommit>, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::SceneCommitRepository::new(pool);
    repo.get_by_story(&story_id).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_genesis_runs(
    limit: Option<i64>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::GenesisRun>, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::GenesisRunRepository::new(pool);
    repo.list_all(limit.unwrap_or(100)).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_genesis_run(
    id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<crate::db::GenesisRun>, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::GenesisRunRepository::new(pool);
    repo.get_by_id(&id).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_latest_style_snapshot(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<crate::db::models::StyleSnapshot>, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::StyleSnapshotRepository::new(pool);
    repo.get_latest_by_story(&story_id).map_err(AppError::from)
}
