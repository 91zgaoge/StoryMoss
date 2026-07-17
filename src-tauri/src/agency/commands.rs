use tauri::{AppHandle, State};

use crate::agency::board::BlackboardService;
use crate::agency::coordinator::{cancel_agency_run, AgencyCoordinator};
use crate::agency::models::{AgencyRun, BoardItem};
use crate::agency::repository::AgencyRepository;
use crate::db::DbPool;
use crate::error::AppError;

/// 启动创世 2.0：立即返回 run_id，进度经 `agency-run-progress` 事件推送。
#[tauri::command(rename_all = "snake_case")]
pub async fn agency_start_genesis(
    premise: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
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

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_get_run(
    run_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<AgencyRun>, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        AgencyRepository::new(pool).get_run(&run_id).map_err(AppError::from)
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
        BlackboardService::new(pool).repo().list_items(&run_id, None).map_err(AppError::from)
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
        log::warn!("agency_cancel_run: run {} 不在取消注册表中（不存在或已结束）", run_id);
    }
    crate::llm::LlmService::new(app_handle).cancel_all_generations();
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || {
        let repo = AgencyRepository::new(pool);
        match repo.get_run(&run_id) {
            Ok(Some(run)) => {
                if run.status == "running" || run.status == "pending" {
                    if let Err(e) = repo.finish_run(&run_id, "cancelled", None, Some("用户取消")) {
                        log::warn!("agency_cancel_run: 标记 run {} 为 cancelled 失败: {}", run_id, e);
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
