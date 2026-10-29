//! Canonical state IPC commands
//!
//! 幕后的执行面板（ExecutionPanel）依赖 `narrative_phase` 与
//! `story_context.overdue_payoffs` 决定阶段徽章与主操作按钮。
//! 命令曾经在 refactor 回滚（b4448325）中只留下 `handlers.rs` 的段落注释，
//! 前端仍在调用 → 每次挂载静默降级为「铺垫阶段」。契约由
//! `tests/ipc_registry_contract_test.rs` 与 `verify-ipc-manifest.py` 共同看住。

use tauri::State;

use crate::{
    canonical_state::{CanonicalStateManager, CanonicalStateSnapshot},
    db::DbPool,
    error::AppError,
};

/// 实时聚合故事状态快照。
///
/// 内部全是同步 rusqlite 查询，必须在 blocking 线程池执行，
/// 否则会在连接池等待时占死 tokio worker。
pub fn load_canonical_state(
    pool: DbPool,
    story_id: &str,
) -> Result<CanonicalStateSnapshot, AppError> {
    CanonicalStateManager::new(pool).get_snapshot_sync(story_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_canonical_state(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<CanonicalStateSnapshot, AppError> {
    let pool = pool.inner().clone();
    tokio::task::spawn_blocking(move || load_canonical_state(pool, &story_id))
        .await
        .map_err(|e| AppError::Internal {
            message: format!("canonical state 聚合任务失败: {}", e),
        })?
}
