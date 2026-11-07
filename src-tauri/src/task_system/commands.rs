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

// ═══════════════ v0.88.0: 续写训练报告查询 ═══════════════

#[derive(serde::Serialize)]
pub struct TrainingReportSummary {
    pub id: String,
    pub book_title: String,
    pub story_id: String,
    pub start_chapter: i64,
    pub context_chapters: i64,
    pub generated_chapter: Option<i64>,
    pub scores_json: Option<String>,
    pub overall: Option<f64>,
    pub created_at: String,
}

/// 训练报告列表（可按书名过滤；最多 50 条）
#[tauri::command(rename_all = "snake_case")]
pub async fn training_list_reports(
    pool: tauri::State<'_, crate::db::DbPool>,
    book_title: Option<String>,
) -> Result<Vec<TrainingReportSummary>, crate::error::AppError> {
    let conn = pool
        .inner()
        .get()
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    let mut stmt = conn
        .prepare(
            "SELECT id, book_title, story_id, start_chapter, context_chapters, \
             generated_chapter, scores_json, created_at \
             FROM continuation_training_reports \
             WHERE (?1 IS NULL OR book_title = ?1) \
             ORDER BY created_at DESC LIMIT 50",
        )
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    let rows = stmt
        .query_map(rusqlite::params![book_title.as_deref()], |r| {
            let scores_json: Option<String> = r.get(6)?;
            let overall = scores_json.as_deref().and_then(|s| {
                serde_json::from_str::<serde_json::Value>(s)
                    .ok()
                    .and_then(|v| v.get("overall").and_then(|o| o.as_f64()))
            });
            Ok(TrainingReportSummary {
                id: r.get(0)?,
                book_title: r.get(1)?,
                story_id: r.get(2)?,
                start_chapter: r.get(3)?,
                context_chapters: r.get(4)?,
                generated_chapter: r.get(5)?,
                scores_json,
                overall,
                created_at: r.get(7)?,
            })
        })
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| crate::error::AppError::internal(e.to_string()))?);
    }
    Ok(out)
}

#[derive(serde::Serialize)]
pub struct TrainingReportDetail {
    pub id: String,
    pub book_title: String,
    pub story_id: String,
    pub run_id: Option<String>,
    pub start_chapter: i64,
    pub generated_chapter: Option<i64>,
    pub truth_content: Option<String>,
    pub scores_json: Option<String>,
    pub report_md: String,
    pub improvement_md: Option<String>,
    pub created_at: String,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn training_get_report(
    pool: tauri::State<'_, crate::db::DbPool>,
    report_id: String,
) -> Result<TrainingReportDetail, crate::error::AppError> {
    let conn = pool
        .inner()
        .get()
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    conn.query_row(
        "SELECT id, book_title, story_id, run_id, start_chapter, generated_chapter, \
         truth_content, scores_json, report_md, improvement_md, created_at \
         FROM continuation_training_reports WHERE id = ?1",
        rusqlite::params![report_id],
        |r| {
            Ok(TrainingReportDetail {
                id: r.get(0)?,
                book_title: r.get(1)?,
                story_id: r.get(2)?,
                run_id: r.get(3)?,
                start_chapter: r.get(4)?,
                generated_chapter: r.get(5)?,
                truth_content: r.get(6)?,
                scores_json: r.get(7)?,
                report_md: r.get(8)?,
                improvement_md: r.get(9)?,
                created_at: r.get(10)?,
            })
        },
    )
    .map_err(|e| crate::error::AppError::internal(format!("报告不存在: {e}")))
}
