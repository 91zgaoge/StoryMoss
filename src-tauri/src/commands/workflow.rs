//! Workflow commands

use tauri::{AppHandle, Emitter};

use crate::error::AppError;

// ===== 通用 Workflow 引擎命令 — 仅保留前端查询命令 =====

/// 列出所有已注册的工作流（包括从文件加载的）
#[tauri::command(rename_all = "snake_case")]
pub fn list_workflows(
    loader: tauri::State<'_, crate::workflow::WorkflowLoader>,
) -> Result<Vec<crate::workflow::LoadedWorkflow>, AppError> {
    Ok(loader.list_workflows())
}

/// 手动重新加载所有工作流文件
#[tauri::command(rename_all = "snake_case")]
pub fn reload_workflows(
    loader: tauri::State<'_, crate::workflow::WorkflowLoader>,
) -> Result<usize, AppError> {
    loader.reload_all().map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn evolve_capabilities(app_handle: AppHandle) -> Result<Vec<(String, String)>, AppError> {
    log::info!("[evolve_capabilities] 手动触发能力进化分析");
    let llm = crate::llm::LlmService::new(app_handle.clone());
    let engine = crate::capabilities::evolution::CapabilityEvolutionEngine::new(llm, &app_handle);
    let improvements = engine.evolve_capability_descriptions().await?;
    let _ = app_handle.emit(
        "capabilities-evolved",
        serde_json::json!({
            "improvements": improvements,
            "auto_triggered": false,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }),
    );
    log::info!(
        "[evolve_capabilities] 进化完成，生成 {} 条改进建议",
        improvements.len()
    );
    Ok(improvements)
}
