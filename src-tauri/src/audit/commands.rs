//! Audit System IPC Commands

use tauri::{command, AppHandle, State};

use super::{AuditReport, AuditService};
use crate::{db::DbPool, error::AppError};

/// 审计场景
#[command]
pub async fn audit_scene(
    scene_id: String,
    audit_type: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<AuditReport, AppError> {
    let service = AuditService::new(pool.inner().clone());
    service
        .audit_scene(&scene_id, &audit_type, Some(&app_handle))
        .await
}
