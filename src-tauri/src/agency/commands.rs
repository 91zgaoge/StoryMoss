use tauri::{AppHandle, State};

use crate::{
    agency::{
        board::BlackboardService,
        coordinator::{cancel_agency_run, AgencyCoordinator},
        models::{AgencyRun, BoardItem},
        repository::AgencyRepository,
    },
    db::DbPool,
    error::AppError,
};

/// 启动创世 2.0：立即返回 run_id，进度经 `agency-run-progress` 事件推送。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_start_genesis(
    premise: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    crate::agency::coordinator::validate_premise(&premise)?;
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = AgencyCoordinator::new(app_handle, pool.inner().clone());
    let rid = run_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = coordinator.run_genesis(&rid, &premise).await {
            log::error!("agency genesis run {} failed: {}", rid, e);
        }
    });
    Ok(run_id)
}

/// 续写下一章：同一 story 不允许并行 run；章号 = MAX(sequence_number)+1。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_continue_chapter(
    story_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let pool = pool.inner().clone();
    // 并发护栏：同一 story 不允许并行 run
    let pool_guard = pool.clone();
    let sid_guard = story_id.clone();
    let has_running = tokio::task::spawn_blocking(move || {
        crate::agency::repository::AgencyRepository::new(pool_guard)
            .has_running_run_for_story(&sid_guard)
    })
    .await
    .map_err(|e| AppError::from(format!("guard join error: {}", e)))?
    .map_err(AppError::from)?;
    if has_running {
        return Err(AppError::validation_failed(
            "该故事已有进行中的创作任务",
            None::<String>,
        ));
    }
    // 下一章号
    let pool2 = pool.clone();
    let sid2 = story_id.clone();
    let chapter_number = tokio::task::spawn_blocking(move || -> Result<i32, AppError> {
        let conn = pool2
            .get()
            .map_err(|e| AppError::from(format!("pool: {}", e)))?;
        conn.query_row(
            "SELECT COALESCE(MAX(sequence_number), 0) + 1 FROM scenes WHERE story_id = ?1",
            rusqlite::params![sid2],
            |r| r.get(0),
        )
        .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("chapter join error: {}", e)))??;
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = AgencyCoordinator::new(app_handle, pool);
    let rid = run_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = coordinator
            .run_continue(&rid, &story_id, chapter_number)
            .await
        {
            log::error!("agency continue run {} failed: {}", rid, e);
        }
    });
    Ok(run_id)
}

/// 批量续写：并行稳态循环（gate(n-1) ∥ writer(n)），立即返回 run_id。
/// count 默认 3，钳制 1..=5；起始章号 = MAX(sequence_number)+1。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_continue_batch(
    story_id: String,
    count: Option<u32>,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let count = (count.unwrap_or(3) as usize).clamp(1, 5);
    let pool = pool.inner().clone();
    // 并发护栏：同一 story 不允许并行 run
    let pool_guard = pool.clone();
    let sid = story_id.clone();
    let has_running = tokio::task::spawn_blocking(move || {
        crate::agency::repository::AgencyRepository::new(pool_guard).has_running_run_for_story(&sid)
    })
    .await
    .map_err(|e| AppError::from(format!("guard join error: {}", e)))?
    .map_err(AppError::from)?;
    if has_running {
        return Err(AppError::validation_failed(
            "该故事已有进行中的创作任务",
            None::<String>,
        ));
    }
    // 起始章号
    let pool2 = pool.clone();
    let sid2 = story_id.clone();
    let start_chapter =
        tokio::task::spawn_blocking(move || AgencyCoordinator::next_chapter_number(&pool2, &sid2))
            .await
            .map_err(|e| AppError::from(format!("chapter join error: {}", e)))??;
    let run_id = uuid::Uuid::new_v4().to_string();
    let coordinator = AgencyCoordinator::new(app_handle, pool);
    let rid = run_id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = coordinator
            .run_continue_batch(&rid, &story_id, start_chapter, count)
            .await
        {
            log::error!("agency batch run {} failed: {}", rid, e);
        }
    });
    Ok(run_id)
}

/// 跨会话恢复：立即返回 ResumeOutcome（含 new_run_id），续写 batch 在后台执行。
/// 进度经 agency-run-progress / agency-agent-activity 事件推送；取消用 agency_cancel_run(new_run_id)。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_resume_run(
    old_run_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<crate::agency::coordinator::ResumeOutcome, AppError> {
    let coordinator = AgencyCoordinator::new(app_handle, pool.inner().clone());
    let outcome = coordinator.resume_prepare(&old_run_id).await?;
    let (new_run_id, story_id) = (outcome.new_run_id.clone(), outcome.story_id.clone());
    let outcome_ret = outcome.clone();
    tauri::async_runtime::spawn(async move {
        let start =
            match AgencyCoordinator::next_chapter_number_async(&coordinator, &story_id).await {
                Ok(n) => n,
                Err(e) => {
                    log::error!("resume batch chapter number failed: {}", e);
                    return;
                }
            };
        if let Err(e) = coordinator
            .run_continue_batch(&new_run_id, &story_id, start, 1)
            .await
        {
            log::error!("resume batch run {} failed: {}", new_run_id, e);
        }
    });
    Ok(outcome_ret)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_get_run(
    run_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<AgencyRun>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        AgencyRepository::new(pool)
            .get_run(&run_id)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("agency_get_run join error: {}", e)))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_list_board(
    run_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<BoardItem>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        BlackboardService::new(pool)
            .repo()
            .list_items(&run_id, None)
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::from(format!("agency_list_board join error: {}", e)))?
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_cancel_run(
    run_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    if !cancel_agency_run(&run_id) {
        log::warn!(
            "agency_cancel_run: run {} 不在取消注册表中（不存在或已结束）",
            run_id
        );
    }
    // 定点取消：仅取消该 run 的在途 LLM 调用，不再全局 cancel_all
    let llm = crate::llm::LlmService::new(app_handle);
    crate::agency::coordinator::cancel_requests_for_run(&llm, &run_id);
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        let repo = AgencyRepository::new(pool);
        match repo.get_run(&run_id) {
            Ok(Some(run)) => {
                if run.status == "running" || run.status == "pending" {
                    if let Err(e) = repo.finish_run(&run_id, "cancelled", None, Some("用户取消"))
                    {
                        log::warn!(
                            "agency_cancel_run: 标记 run {} 为 cancelled 失败: {}",
                            run_id,
                            e
                        );
                    }
                }
            }
            Ok(None) => log::warn!("agency_cancel_run: run {} 不存在", run_id),
            Err(e) => log::warn!("agency_cancel_run: 读取 run {} 失败: {}", run_id, e),
        }
    })
    .await
    .map_err(|e| AppError::from(format!("agency_cancel_run join error: {}", e)))?;
    Ok(())
}
