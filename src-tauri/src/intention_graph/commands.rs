//! SING 意图图 IPC 命令
//!
//! 暴露给前端的 Tauri 命令，用于查询意图图状态、诊断信息和执行历史。

use tauri::{AppHandle, Manager};

use crate::error::AppError;

use super::{
    graph::IntentionGraphRepository,
    models::ExecutionGraph,
};

/// 查询意图图诊断信息
///
/// 复用 setup 阶段注册的 IntentionGraphRepository（共享预热缓存）。
#[tauri::command(rename_all = "snake_case")]
pub async fn get_intention_graph_diagnostics(
    app_handle: AppHandle,
) -> Result<IntentionGraphDiagnostics, AppError> {
    let repo = get_repo(&app_handle)?;

    let stats = repo.get_statistics()?;
    let recent_graphs = repo.get_recent_executions(10)?;

    Ok(IntentionGraphDiagnostics {
        intention_count: stats.intention_count,
        asset_count: stats.asset_count,
        edge_count: stats.intention_asset_edge_count + stats.asset_asset_edge_count,
        recent_executions: recent_graphs.into_iter().map(|g| ExecutionSummary {
            id: g.id,
            request_id: g.request_id,
            user_input: g.user_input,
            status: format!("{:?}", g.status),
            created_at: g.created_at.to_rfc3339(),
        }).collect(),
    })
}

/// 查询最近的执行图详情
#[tauri::command(rename_all = "snake_case")]
pub async fn get_execution_graph_detail(
    app_handle: AppHandle,
    graph_id: String,
) -> Result<Option<ExecutionGraph>, AppError> {
    let repo = get_repo(&app_handle)?;
    repo.get_execution_graph(&graph_id)
}

/// 意图图诊断信息响应
#[derive(Debug, Clone, serde::Serialize)]
pub struct IntentionGraphDiagnostics {
    pub intention_count: i64,
    pub asset_count: i64,
    pub edge_count: i64,
    pub recent_executions: Vec<ExecutionSummary>,
}

/// 执行摘要
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExecutionSummary {
    pub id: String,
    pub request_id: String,
    pub user_input: String,
    pub status: String,
    pub created_at: String,
}

/// 获取共享的 IntentionGraphRepository（setup 阶段注册）
fn get_repo(app_handle: &AppHandle) -> Result<IntentionGraphRepository, AppError> {
    app_handle
        .try_state::<IntentionGraphRepository>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| AppError::internal("IntentionGraphRepository not initialized".to_string()))
}
