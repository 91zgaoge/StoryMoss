//! Task System Tauri IPC Commands

use super::{models::*, service::TaskService};
use crate::error::AppError;

#[tauri::command]
pub async fn create_task(
    name: String,
    description: Option<String>,
    task_type: String,
    schedule_type: String,
    cron_pattern: Option<String>,
    payload: Option<String>,
    enabled: Option<bool>,
    max_retries: Option<i32>,
    heartbeat_timeout_seconds: Option<i32>,
    service: tauri::State<'_, TaskService>,
) -> Result<Task, AppError> {
    let req = CreateTaskRequest {
        name,
        description,
        task_type,
        schedule_type,
        cron_pattern,
        payload,
        enabled,
        max_retries,
        heartbeat_timeout_seconds,
    };
    service.create_task(req)
}

#[tauri::command]
pub async fn update_task(
    id: String,
    name: Option<String>,
    description: Option<String>,
    enabled: Option<bool>,
    cron_pattern: Option<String>,
    max_retries: Option<i32>,
    heartbeat_timeout_seconds: Option<i32>,
    service: tauri::State<'_, TaskService>,
) -> Result<Task, AppError> {
    let req = UpdateTaskRequest {
        name,
        description,
        enabled,
        cron_pattern,
        max_retries,
        heartbeat_timeout_seconds,
    };
    service.update_task(&id, req)
}

#[tauri::command]
pub async fn delete_task(
    id: String,
    service: tauri::State<'_, TaskService>,
) -> Result<(), AppError> {
    service.delete_task(&id)
}

#[tauri::command]
pub async fn list_tasks(
    status_filter: Option<String>,
    service: tauri::State<'_, TaskService>,
) -> Result<Vec<Task>, AppError> {
    service.list_tasks(status_filter)
}

#[tauri::command]
pub async fn trigger_task(
    id: String,
    service: tauri::State<'_, TaskService>,
) -> Result<(), AppError> {
    service.trigger_task(&id)
}

#[tauri::command]
pub async fn cancel_task(
    id: String,
    service: tauri::State<'_, TaskService>,
) -> Result<(), AppError> {
    service.cancel_task(&id)
}

#[tauri::command]
pub async fn get_task_logs(
    task_id: String,
    service: tauri::State<'_, TaskService>,
) -> Result<Vec<TaskLog>, AppError> {
    service.get_task_logs(&task_id)
}

/// 便捷命令：创建 AI 生成任务
#[tauri::command]
pub async fn run_ai_generation_task(
    story_id: String,
    input: String,
    chapter_number: Option<u32>,
    agent_type: Option<String>,
    mode: Option<String>,
    service: tauri::State<'_, TaskService>,
) -> Result<Task, AppError> {
    let agent_type_str = agent_type.unwrap_or_else(|| "writer".to_string());
    let mode_str = mode.unwrap_or_else(|| "full".to_string());

    let payload = serde_json::json!({
        "story_id": story_id,
        "chapter_number": chapter_number.unwrap_or(1),
        "input": input,
        "agent_type": agent_type_str,
        "mode": mode_str,
    });

    let req = CreateTaskRequest {
        name: format!("AI 生成: {}", input.chars().take(20).collect::<String>()),
        description: Some(format!("Agent: {}", agent_type_str)),
        task_type: "ai_generation".to_string(),
        schedule_type: "once".to_string(),
        cron_pattern: None,
        payload: Some(payload.to_string()),
        enabled: Some(true),
        max_retries: Some(1),
        heartbeat_timeout_seconds: Some(600),
    };

    service.create_task(req)
}

/// 便捷命令：创建 Pipeline 审校任务
#[tauri::command]
pub async fn run_pipeline_task(
    story_id: String,
    draft_id: String,
    operation: String,
    user_prompt: Option<String>,
    review_focus: Option<String>,
    chapter_number: Option<i32>,
    chapter_title: Option<String>,
    service: tauri::State<'_, TaskService>,
) -> Result<Task, AppError> {
    let mut payload = serde_json::json!({
        "story_id": story_id,
        "draft_id": draft_id,
        "operation": operation,
    });

    if let Some(p) = user_prompt {
        payload["user_prompt"] = serde_json::json!(p);
    }
    if let Some(f) = review_focus {
        payload["review_focus"] = serde_json::json!(f);
    }
    if let Some(n) = chapter_number {
        payload["chapter_number"] = serde_json::json!(n);
    }
    if let Some(t) = chapter_title {
        payload["chapter_title"] = serde_json::json!(t);
    }

    let req = CreateTaskRequest {
        name: format!("Pipeline {}", operation),
        description: Some(format!("story: {}, draft: {}", story_id, draft_id)),
        task_type: "pipeline_review".to_string(),
        schedule_type: "once".to_string(),
        cron_pattern: None,
        payload: Some(payload.to_string()),
        enabled: Some(true),
        max_retries: Some(1),
        heartbeat_timeout_seconds: Some(600),
    };

    service.create_task(req)
}
