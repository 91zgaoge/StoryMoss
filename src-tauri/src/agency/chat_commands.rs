use std::sync::Arc;

use tauri::{AppHandle, State};

use crate::{
    agency::{
        chat::ChatService,
        chat_repository::ChatRepository,
        coordinator::AgencyCoordinator,
        crew::CrewInspector,
        director::DirectorExecutor,
        director_repository::DirectorRepository,
        lease::LeaseManager,
        models::{AgentRole, ChatMessage, ChatSession},
        scheduler::TaskScheduler,
    },
    db::DbPool,
    error::AppError,
};

fn build_scheduler(app: &AppHandle, pool: &DbPool) -> Arc<TaskScheduler> {
    let coordinator = Arc::new(AgencyCoordinator::new(app.clone(), pool.clone()));
    Arc::new(TaskScheduler::new(
        pool.clone(),
        coordinator,
        Some(app.clone()),
    ))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_chat_send(
    story_id: String,
    session_id: Option<String>,
    message: String,
    target_role: Option<String>,
    corpus_ids: Option<Vec<String>>,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<crate::agency::chat::ChatSendResult, AppError> {
    let pool = pool.inner().clone();
    let scheduler = build_scheduler(&app, &pool);
    let service = ChatService::new(pool, app, (*scheduler).clone());
    service
        .send(
            &story_id,
            session_id.as_deref(),
            &message,
            target_role.as_deref(),
            corpus_ids.unwrap_or_default(),
        )
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub fn agency_chat_sessions(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<ChatSession>, AppError> {
    ChatRepository::new(pool.inner().clone()).list_sessions(&story_id)
}

#[tauri::command(rename_all = "snake_case")]
pub fn agency_chat_messages(
    session_id: String,
    limit: Option<i32>,
    pool: State<'_, DbPool>,
) -> Result<Vec<ChatMessage>, AppError> {
    ChatRepository::new(pool.inner().clone()).list_messages(&session_id, limit.unwrap_or(200))
}

#[tauri::command(rename_all = "snake_case")]
pub fn agency_chat_new_session(
    story_id: String,
    title: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<ChatSession, AppError> {
    ChatRepository::new(pool.inner().clone()).create_session(
        &story_id,
        title.as_deref().unwrap_or("剧组会话"),
        "director",
    )
}

#[tauri::command(rename_all = "snake_case")]
pub fn agency_chat_delete_session(
    session_id: String,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    ChatRepository::new(pool.inner().clone()).delete_session(&session_id)?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn agency_crew_status(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<crate::agency::crew::CrewSnapshot, AppError> {
    CrewInspector::inspect(pool.inner(), &story_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_crew_dispatch(
    story_id: String,
    role: String,
    instruction: String,
    premise: Option<String>,
    run_key: Option<String>,
    session_id: Option<String>,
    corpus_ids: Option<Vec<String>>,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let pool = pool.inner().clone();
    let role = AgentRole::from_str(&role)
        .ok_or_else(|| AppError::validation_failed(format!("未知角色: {role}"), Some("role")))?;
    let premise = premise
        .or_else(|| {
            CrewInspector::inspect(&pool, &story_id)
                .ok()
                .map(|s| s.story_title)
        })
        .unwrap_or_else(|| instruction.clone());
    let run_key = run_key.unwrap_or_else(|| {
        session_id
            .as_ref()
            .map(|s| format!("chat:{s}"))
            .unwrap_or_else(|| "chat:manual".to_string())
    });
    let meta = serde_json::json!({
        "chat_session_id": session_id,
        "append_mode": if session_id.is_some() { "follow_up" } else { "none" },
        "premise": premise,
        "corpus_ids": corpus_ids.unwrap_or_default(),
    })
    .to_string();
    let scheduler = build_scheduler(&app, &pool);
    scheduler
        .dispatch_sub_agent(
            &story_id,
            &run_key,
            role,
            &premise,
            &instruction,
            None,
            None,
            meta,
        )
        .await
}

#[tauri::command(rename_all = "snake_case")]
pub fn agency_crew_cancel_task(
    task_id: String,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let pool = pool.inner().clone();
    let scheduler = build_scheduler(&app, &pool);
    scheduler.cancel_task(&task_id)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_crew_retry_task(
    task_id: String,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let pool = pool.inner().clone();
    let scheduler = build_scheduler(&app, &pool);
    scheduler.retry_task(&task_id).await
}

#[tauri::command(rename_all = "snake_case")]
pub fn agency_crew_release_lease(
    story_id: String,
    resource: String,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    LeaseManager::new(pool.inner().clone())
        .release_resource(&story_id, &resource)
        .map(|_| ())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn agency_director_execute(
    story_id: String,
    session_id: String,
    message_id: String,
    premise: Option<String>,
    action_ids: Option<Vec<String>>,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<Vec<String>, AppError> {
    let pool = pool.inner().clone();
    let premise = premise
        .or_else(|| {
            CrewInspector::inspect(&pool, &story_id)
                .ok()
                .map(|s| s.story_title)
        })
        .unwrap_or_else(|| String::new());
    let scheduler = build_scheduler(&app, &pool);
    DirectorExecutor::execute_confirmed(
        &pool,
        &story_id,
        &session_id,
        &message_id,
        &premise,
        &action_ids.unwrap_or_default(),
        &scheduler,
    )
    .await
}

#[tauri::command(rename_all = "snake_case")]
pub fn agency_director_reject(
    action_ids: Vec<String>,
    message_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let repo = DirectorRepository::new(pool.inner().clone());
    for id in action_ids {
        repo.update_status(&id, "rejected", None, None)?;
    }
    if let Some(mid) = message_id {
        let _ = ChatRepository::new(pool.inner().clone()).update_message_payload(
            &mid,
            "done",
            None,
            None,
            Some("[]"),
            None,
        );
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn agency_director_patrol(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<crate::agency::crew::CrewSnapshot, AppError> {
    CrewInspector::inspect(pool.inner(), &story_id)
}
