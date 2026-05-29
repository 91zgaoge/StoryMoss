//! Audit commands

use tauri::State;

use crate::{db::DbPool, error::AppError};

// ==================== Story Structure Audit Commands ====================

#[tauri::command(rename_all = "snake_case")]
pub async fn audit_story(
    pool: State<'_, DbPool>,
    story_id: String,
    app_handle: tauri::AppHandle,
) -> Result<crate::narrative::audit::StoryAnalysisReport, AppError> {
    let pool = pool.inner().clone();
    let llm_service = crate::llm::LlmService::new(app_handle);
    let auditor = crate::narrative::audit::StoryStructureAuditor::new(pool, llm_service);
    auditor
        .analyze(&story_id)
        .await
        .map_err(|e| AppError::internal(e.to_string()))
}
