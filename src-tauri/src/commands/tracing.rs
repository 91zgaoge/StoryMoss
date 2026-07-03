//! Tracing / 生成链路可观测性命令

use tauri::State;

use crate::{
    error::AppError,
    tracing::{GenerationTrace, TraceStore},
};

#[tauri::command(rename_all = "snake_case")]
pub fn get_generation_trace(
    trace_id: String,
    store: State<'_, TraceStore>,
) -> Result<GenerationTrace, AppError> {
    store.get_trace(&trace_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn list_recent_generation_traces(
    limit: Option<usize>,
    store: State<'_, TraceStore>,
) -> Result<Vec<GenerationTrace>, AppError> {
    store.list_recent(limit.unwrap_or(20))
}
