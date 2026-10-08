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
) -> Result<crate::domain::contracts::RuntimeContract, AppError> {
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

// ==================== v0.63.0 P3：质量债 / 时间旅行 / 待确认队列
// ====================

/// 列出质量债（默认 open；status 可传 open/resolved/dismissed）。
#[tauri::command(rename_all = "snake_case")]
pub async fn list_quality_debts(
    story_id: String,
    status: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::story_system::quality_debt::QualityDebt>, AppError> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::quality_debt::list_debts(&pool, &story_id, status.as_deref(), 200)
    })
    .await
    .map_err(|e| AppError::internal(format!("查询质量债失败: {}", e)))
}

/// 结清 / 忽略一条质量债。
#[tauri::command(rename_all = "snake_case")]
pub async fn resolve_quality_debt(
    debt_id: String,
    status: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let pool = pool.inner().clone();
    let status = status.unwrap_or_else(|| "resolved".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::quality_debt::resolve_debt(&pool, &debt_id, &status)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::internal(format!("更新质量债失败: {}", e)))?
}

/// 「截至第 N 章」的回溯视图：角色已知 / 真相揭示 / 物品归属（当前值）。
#[tauri::command(rename_all = "snake_case")]
pub async fn query_story_as_of(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
) -> Result<crate::story_system::checkpoint::AsOfView, AppError> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::checkpoint::query_as_of(&pool, &story_id, chapter_number)
    })
    .await
    .map_err(|e| AppError::internal(format!("回溯查询失败: {}", e)))
}

/// 列出待确认的规则类新增物（默认 pending）。
#[tauri::command(rename_all = "snake_case")]
pub async fn list_pending_reviews(
    story_id: String,
    status: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::story_system::pending_review::PendingReview>, AppError> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::pending_review::list_pending_reviews(
            &pool,
            &story_id,
            status.as_deref(),
        )
    })
    .await
    .map_err(|e| AppError::internal(format!("查询待确认项失败: {}", e)))
}

/// 确认 / 拒绝一条待确认项。
#[tauri::command(rename_all = "snake_case")]
pub async fn resolve_pending_review(
    review_id: String,
    status: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let pool = pool.inner().clone();
    let status = status.unwrap_or_else(|| "confirmed".to_string());
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::pending_review::resolve_pending_review(&pool, &review_id, &status)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::internal(format!("更新待确认项失败: {}", e)))?
}

// ==================== v0.64.0：文风偏好管理（运行维护页） ====================

/// 列出作者文风偏好（status 传 active/disabled，缺省列出全部）。
#[tauri::command(rename_all = "snake_case")]
pub async fn list_style_preferences(
    story_id: String,
    status: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::story_system::style_learning::StylePreference>, AppError> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::style_learning::list_preferences(
            &pool,
            &story_id,
            status.as_deref(),
            200,
        )
    })
    .await
    .map_err(|e| AppError::internal(format!("查询文风偏好失败: {}", e)))
}

/// 启用 / 停用一条文风偏好。
#[tauri::command(rename_all = "snake_case")]
pub async fn set_style_preference_status(
    preference_id: String,
    active: bool,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if active {
            crate::story_system::style_learning::reactivate_preference(&pool, &preference_id)
                .map_err(AppError::from)
        } else {
            crate::story_system::style_learning::deactivate_preference(&pool, &preference_id)
                .map_err(AppError::from)
        }
    })
    .await
    .map_err(|e| AppError::internal(format!("更新文风偏好失败: {}", e)))?
}

// ==================== v0.64.11：物料失效与重算
// ====================

/// 列出「自第 N 章起失效」的跨章物料（章节摘要 / 分层摘要与全书纲要 /
/// 连续性快照）。
#[tauri::command(rename_all = "snake_case")]
pub async fn list_stale_materials(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::story_system::recompute::StaleMaterial>, AppError> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::recompute::list_stale(&pool, &story_id).map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::internal(format!("查询物料失效失败: {}", e)))?
}

/// 重算第 `from_chapter` 章及以后的物料（章节摘要按当前正文重算；分层摘要与全书
/// 纲要删旧重建；连续性快照按当前数据重写）。`from_chapter` 缺省取最小失效章。
#[tauri::command(rename_all = "snake_case")]
pub async fn recompute_story_material(
    story_id: String,
    from_chapter: Option<i32>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::story_system::recompute::RecomputeReport, AppError> {
    let pool = pool.inner().clone();
    let from = match from_chapter {
        Some(n) if n > 0 => n,
        _ => {
            let pool_probe = pool.clone();
            let sid = story_id.clone();
            tauri::async_runtime::spawn_blocking(move || {
                crate::story_system::recompute::list_stale(&pool_probe, &sid)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|s| s.from_chapter)
                    .min()
                    .unwrap_or(1)
            })
            .await
            .map_err(|e| AppError::internal(format!("查询物料失效失败: {}", e)))?
        }
    };
    let llm = crate::llm::LlmService::new(app_handle);
    crate::story_system::recompute::recompute_from(&pool, &story_id, from, Some(&llm))
        .await
        .map_err(AppError::from)
}
