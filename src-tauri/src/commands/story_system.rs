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

// ==================== 章纲重规划（P0-2，v0.67.0） ====================

/// 预览「受影响下游章纲」的重规划：按当前正文与资产重生成章纲对照
/// （不落库），作者在返回的新旧对照里逐条勾选后再调
/// `apply_chapter_outline_replans`。每轮上限 `max_chapters`（默认 3）章，
/// 超出部分保留失效标记下轮继续。
#[tauri::command(rename_all = "snake_case")]
pub async fn preview_chapter_outline_replans(
    story_id: String,
    from_chapter: Option<i32>,
    max_chapters: Option<usize>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::agency::outline_replan::PreviewReport, AppError> {
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
                    .find(|s| s.kind == crate::story_system::recompute::KIND_CHAPTER_OUTLINE)
                    .map(|s| s.from_chapter)
                    .unwrap_or(1)
            })
            .await
            .map_err(|e| AppError::internal(format!("查询章纲失效失败: {}", e)))?
        }
    };
    let coordinator = crate::agency::coordinator::AgencyCoordinator::new(app_handle, pool.clone());
    crate::agency::outline_replan::preview(
        &coordinator,
        &pool,
        &story_id,
        from,
        max_chapters.unwrap_or(crate::agency::outline_replan::REPLAN_MAX_CHAPTERS),
    )
    .await
}

/// 落库作者勾选的章纲重规划条目（只写勾选项；手写/手调章纲默认不勾选）。
#[tauri::command(rename_all = "snake_case")]
pub async fn apply_chapter_outline_replans(
    story_id: String,
    items: Vec<crate::agency::outline_replan::ApplyOutlineItem>,
    pool: State<'_, DbPool>,
) -> Result<crate::agency::outline_replan::ApplyReport, AppError> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::agency::outline_replan::apply(&pool, &story_id, &items)
    })
    .await
    .map_err(|e| AppError::internal(format!("章纲重规划落库失败: {}", e)))?
}

/// 忽略某类失效提示（当前用于「章纲待重规划」——作者决定不处理时清掉标记）。
#[tauri::command(rename_all = "snake_case")]
pub async fn dismiss_stale_material(
    story_id: String,
    kind: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::recompute::clear_stale_kind(&pool, &story_id, &kind)
    })
    .await
    .map_err(|e| AppError::internal(format!("清除失效标记失败: {}", e)))?
    .map_err(AppError::from)
}

// ==================== 卷纲（P1-3，v0.70.0） ====================

/// 卷纲总览：全部卷纲 + 最新进度 + 状态（no_plan / in_volume / past_end）。
#[tauri::command(rename_all = "snake_case")]
pub async fn volume_plan_overview(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<crate::story_system::volume_plan::VolumeOverview, AppError> {
    let pool = pool.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::volume_plan::volume_overview(&pool, &story_id).map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::internal(format!("查询卷纲失败: {}", e)))?
}

/// 生成一卷卷纲（Producer 档一次 JSON 调用，成功后落库）。卷号/起始章缺省取
/// 总览里的「下一个卷」；由作者在运行维护页显式触发（不自动改写计划）。
#[tauri::command(rename_all = "snake_case")]
pub async fn generate_volume_plan(
    story_id: String,
    volume_number: Option<i32>,
    start_chapter: Option<i32>,
    instruction: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::story_system::volume_plan::VolumePlan, AppError> {
    let pool = pool.inner().clone();
    let (volume_number, start_chapter) = match (volume_number, start_chapter) {
        (Some(v), Some(s)) if v >= 1 && s >= 1 => (v, s),
        _ => {
            let pool_probe = pool.clone();
            let sid = story_id.clone();
            tauri::async_runtime::spawn_blocking(move || -> Result<(i32, i32), AppError> {
                let overview = crate::story_system::volume_plan::volume_overview(&pool_probe, &sid)
                    .map_err(AppError::from)?;
                Ok((
                    overview.next_volume_number.unwrap_or(1),
                    overview.next_start_chapter.unwrap_or(1),
                ))
            })
            .await
            .map_err(|e| AppError::internal(format!("查询卷纲状态失败: {}", e)))??
        }
    };
    let coordinator = crate::agency::coordinator::AgencyCoordinator::new(app_handle, pool.clone());
    let budget = std::sync::Arc::new(crate::agency::budget::AgencyBudget::new(
        crate::agency::budget::DEFAULT_RUN_TOKEN_BUDGET,
    ));
    let run_id = format!("volume-plan-{}", uuid::Uuid::new_v4());
    let plan = coordinator
        .generate_volume_plan(
            &run_id,
            &story_id,
            &budget,
            volume_number,
            start_chapter,
            instruction.as_deref(),
        )
        .await?;
    let plan_save = plan.clone();
    let pool_save = pool.clone();
    let sid = story_id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::story_system::volume_plan::save_volume_plan(&pool_save, &sid, &plan_save)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::internal(format!("卷纲落库 join error: {}", e)))??;
    Ok(plan)
}
