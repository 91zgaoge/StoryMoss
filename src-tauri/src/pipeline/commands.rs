use tauri::{command, AppHandle, Manager, State};

use super::{types::*, PipelineOrchestrator, PostProcessRunWithSteps};
use crate::{
    db::{DbPool, DraftRepository, PipelineReviewRepository},
    error::AppError,
    llm::LlmService,
    subscription::SubscriptionService,
};

fn check_pipeline_feature_access(app_handle: &AppHandle, feature_id: &str) -> Result<(), AppError> {
    let pool = app_handle.state::<DbPool>();
    let app_dir = app_handle.path().app_data_dir().unwrap_or_default();
    let machine_id_path = app_dir.join(".machine_id");
    let user_id = if machine_id_path.exists() {
        std::fs::read_to_string(&machine_id_path)
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        "local".to_string()
    };
    let subscription = SubscriptionService::new(pool.inner().clone());
    if !subscription.has_feature_access(&user_id, feature_id)? {
        return Err(AppError::subscription_required(
            feature_id,
            format!("{} 功能需要 Pro 订阅，请升级以继续使用", feature_id),
        ));
    }
    Ok(())
}

// ==================== Pipeline Task Tracking ====================

struct PipelineTaskCallbacks {
    task_id: String,
    pool: DbPool,
}

impl super::types::PipelineCallbacks for PipelineTaskCallbacks {
    fn log(&self, message: &str) {
        let repo = crate::task_system::repository::TaskRepository::new(self.pool.clone());
        let _ = repo.create_log(&self.task_id, "info", message);
    }

    fn progress(&self, phase: &str, percent: f32) {
        let repo = crate::task_system::repository::TaskRepository::new(self.pool.clone());
        let progress = (percent * 100.0) as i32;
        let _ = repo.update_status(
            &self.task_id,
            &crate::task_system::models::TaskStatus::Running,
            Some(progress),
            None,
            None,
        );
    }

    fn on_chunk(&self, _chunk: &str) {}
}

fn create_pipeline_tracking_task(
    task_service: &crate::task_system::service::TaskService,
    pool: &DbPool,
    operation: &str,
    story_id: &str,
    draft_id: &str,
    payload: serde_json::Value,
) -> Result<String, AppError> {
    let req = crate::task_system::models::CreateTaskRequest {
        name: format!("Pipeline {}", operation),
        description: Some(format!("story: {}, draft: {}", story_id, draft_id)),
        task_type: "pipeline_review".to_string(),
        schedule_type: "once".to_string(),
        cron_pattern: None,
        payload: Some(payload.to_string()),
        enabled: Some(false),
        max_retries: Some(0),
        heartbeat_timeout_seconds: Some(600),
    };

    let task = task_service.create_task(req)?;
    let repo = crate::task_system::repository::TaskRepository::new(pool.clone());
    let _ = repo.update_status(
        &task.id,
        &crate::task_system::models::TaskStatus::Running,
        Some(0),
        None,
        None,
    );
    let _ = repo.update_last_run(&task.id);
    Ok(task.id)
}

fn finalize_pipeline_task_success(pool: &DbPool, task_id: &str, result_json: String) {
    if task_id.is_empty() {
        return;
    }
    let repo = crate::task_system::repository::TaskRepository::new(pool.clone());
    let _ = repo.update_status(
        task_id,
        &crate::task_system::models::TaskStatus::Completed,
        Some(100),
        Some(result_json),
        None,
    );
}

fn finalize_pipeline_task_failed(pool: &DbPool, task_id: &str, error: &str) {
    if task_id.is_empty() {
        return;
    }
    let repo = crate::task_system::repository::TaskRepository::new(pool.clone());
    let _ = repo.update_status(
        task_id,
        &crate::task_system::models::TaskStatus::Failed,
        None,
        None,
        Some(error.to_string()),
    );
}

// ==================== Commands ====================

/// 执行 AI 修稿
#[command(rename_all = "snake_case")]
pub async fn run_refine(
    story_id: String,
    draft_id: String,
    user_prompt: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    task_service: State<'_, crate::task_system::service::TaskService>,
) -> Result<RefineResult, AppError> {
    check_pipeline_feature_access(&app_handle, "pipeline_refine")?;

    let payload = serde_json::json!({
        "operation": "refine",
        "story_id": &story_id,
        "draft_id": &draft_id,
        "user_prompt": user_prompt,
    });
    let task_id = create_pipeline_tracking_task(
        &task_service,
        pool.inner(),
        "refine",
        &story_id,
        &draft_id,
        payload,
    )
    .unwrap_or_else(|e| {
        log::warn!("[pipeline] Failed to create tracking task: {}", e);
        String::new()
    });

    let config = PipelineConfig::default();
    let llm_service = LlmService::new(app_handle);
    let silent = super::types::SilentCallbacks;
    let pipeline_callbacks = PipelineTaskCallbacks {
        task_id: task_id.clone(),
        pool: pool.inner().clone(),
    };
    let callbacks: &dyn PipelineCallbacks = if task_id.is_empty() {
        &silent
    } else {
        &pipeline_callbacks
    };

    let result = super::refine_draft(
        &story_id,
        &draft_id,
        user_prompt.as_deref(),
        &config,
        pool.inner(),
        &llm_service,
        callbacks,
    )
    .await;

    match &result {
        Ok(refine_result) => {
            if let Ok(json) = serde_json::to_string(refine_result) {
                finalize_pipeline_task_success(pool.inner(), &task_id, json);
            }
        }
        Err(e) => {
            finalize_pipeline_task_failed(pool.inner(), &task_id, &e.to_string());
        }
    }
    result.map_err(|e| AppError::internal(e.to_string()))
}

/// 执行 AI 审稿
#[command(rename_all = "snake_case")]
pub async fn run_review(
    story_id: String,
    draft_id: String,
    review_focus: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    task_service: State<'_, crate::task_system::service::TaskService>,
) -> Result<ReviewResult, AppError> {
    check_pipeline_feature_access(&app_handle, "pipeline_review")?;

    let payload = serde_json::json!({
        "operation": "review",
        "story_id": &story_id,
        "draft_id": &draft_id,
        "review_focus": review_focus,
    });
    let task_id = create_pipeline_tracking_task(
        &task_service,
        pool.inner(),
        "review",
        &story_id,
        &draft_id,
        payload,
    )
    .unwrap_or_else(|e| {
        log::warn!("[pipeline] Failed to create tracking task: {}", e);
        String::new()
    });

    let config = PipelineConfig::default();
    let llm_service = LlmService::new(app_handle);
    let silent = super::types::SilentCallbacks;
    let pipeline_callbacks = PipelineTaskCallbacks {
        task_id: task_id.clone(),
        pool: pool.inner().clone(),
    };
    let callbacks: &dyn PipelineCallbacks = if task_id.is_empty() {
        &silent
    } else {
        &pipeline_callbacks
    };

    let result = super::review_draft(
        &story_id,
        &draft_id,
        review_focus.as_deref(),
        &config,
        pool.inner(),
        &llm_service,
        callbacks,
    )
    .await;

    match &result {
        Ok(review_result) => {
            if let Ok(json) = serde_json::to_string(review_result) {
                finalize_pipeline_task_success(pool.inner(), &task_id, json);
            }
        }
        Err(e) => {
            finalize_pipeline_task_failed(pool.inner(), &task_id, &e.to_string());
        }
    }
    result.map_err(|e| AppError::internal(e.to_string()))
}

/// 执行定稿与后处理
#[command(rename_all = "snake_case")]
pub async fn run_finalize(
    story_id: String,
    draft_id: String,
    chapter_number: i32,
    chapter_title: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    task_service: State<'_, crate::task_system::service::TaskService>,
) -> Result<PipelineResult, AppError> {
    check_pipeline_feature_access(&app_handle, "pipeline_finalize")?;

    let payload = serde_json::json!({
        "operation": "finalize",
        "story_id": &story_id,
        "draft_id": &draft_id,
        "chapter_number": chapter_number,
        "chapter_title": chapter_title,
    });
    let task_id = create_pipeline_tracking_task(
        &task_service,
        pool.inner(),
        "finalize",
        &story_id,
        &draft_id,
        payload,
    )
    .unwrap_or_else(|e| {
        log::warn!("[pipeline] Failed to create tracking task: {}", e);
        String::new()
    });

    let config = PipelineConfig::default();
    let silent = super::types::SilentCallbacks;
    let pipeline_callbacks = PipelineTaskCallbacks {
        task_id: task_id.clone(),
        pool: pool.inner().clone(),
    };
    let callbacks: &dyn PipelineCallbacks = if task_id.is_empty() {
        &silent
    } else {
        &pipeline_callbacks
    };
    let chapter_info = ChapterInfo {
        chapter_number,
        title: chapter_title,
    };

    let post_process_run_id = super::finalize_draft(
        &story_id,
        &draft_id,
        &chapter_info,
        &config,
        pool.inner(),
        &app_handle,
        callbacks,
    )
    .await;

    let app_result = match post_process_run_id {
        Ok(id) => Ok(PipelineResult {
            draft_id: draft_id.clone(),
            chapter_number,
            refined_draft_id: None,
            review_id: None,
            finalized_draft_id: Some(draft_id),
            post_process_run_id: if id.is_empty() { None } else { Some(id) },
            success: true,
            message: "定稿完成".to_string(),
        }),
        Err(e) => Err(AppError::internal(e.to_string())),
    };

    match &app_result {
        Ok(pipeline_result) => {
            if let Ok(json) = serde_json::to_string(pipeline_result) {
                finalize_pipeline_task_success(pool.inner(), &task_id, json);
            }
        }
        Err(e) => {
            finalize_pipeline_task_failed(pool.inner(), &task_id, &e.to_string());
        }
    }
    app_result
}

/// 修复定稿后处理 — 当后处理失败时重跑
#[command(rename_all = "snake_case")]
pub async fn repair_finalize(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    task_service: State<'_, crate::task_system::service::TaskService>,
) -> Result<PipelineResult, AppError> {
    let orchestrator = PipelineOrchestrator::new(pool.inner().clone());

    let draft = orchestrator
        .get_finalized_draft(&story_id, chapter_number)?
        .ok_or_else(|| AppError::internal("未找到已定稿的草稿"))?;

    let payload = serde_json::json!({
        "operation": "finalize",
        "story_id": &story_id,
        "draft_id": &draft.id,
        "chapter_number": chapter_number,
    });
    let task_id = create_pipeline_tracking_task(
        &task_service,
        pool.inner(),
        "repair_finalize",
        &story_id,
        &draft.id,
        payload,
    )
    .unwrap_or_else(|e| {
        log::warn!("[pipeline] Failed to create tracking task: {}", e);
        String::new()
    });

    let config = PipelineConfig::default();
    let silent = super::types::SilentCallbacks;
    let pipeline_callbacks = PipelineTaskCallbacks {
        task_id: task_id.clone(),
        pool: pool.inner().clone(),
    };
    let callbacks: &dyn PipelineCallbacks = if task_id.is_empty() {
        &silent
    } else {
        &pipeline_callbacks
    };
    let chapter_info = ChapterInfo {
        chapter_number,
        title: None,
    };

    let post_process_run_id = super::finalize_draft(
        &story_id,
        &draft.id,
        &chapter_info,
        &config,
        pool.inner(),
        &app_handle,
        callbacks,
    )
    .await;

    let app_result = match post_process_run_id {
        Ok(id) => Ok(PipelineResult {
            draft_id: draft.id.clone(),
            chapter_number,
            refined_draft_id: None,
            review_id: None,
            finalized_draft_id: Some(draft.id),
            post_process_run_id: if id.is_empty() { None } else { Some(id) },
            success: true,
            message: "后处理修复完成".to_string(),
        }),
        Err(e) => Err(AppError::internal(e.to_string())),
    };

    match &app_result {
        Ok(pipeline_result) => {
            if let Ok(json) = serde_json::to_string(pipeline_result) {
                finalize_pipeline_task_success(pool.inner(), &task_id, json);
            }
        }
        Err(e) => {
            finalize_pipeline_task_failed(pool.inner(), &task_id, &e.to_string());
        }
    }
    app_result
}

/// 获取后处理运行状态（含步骤详情）
#[command(rename_all = "snake_case")]
pub async fn get_post_process_status(
    run_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<PostProcessRunWithSteps>, AppError> {
    let orchestrator = PipelineOrchestrator::new(pool.inner().clone());
    orchestrator.get_post_process_status(&run_id)
}

/// 获取管线编排器状态 — 指定章节当前活跃草稿
#[command(rename_all = "snake_case")]
pub async fn get_pipeline_active_draft(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
) -> Result<Option<crate::db::Draft>, AppError> {
    let orchestrator = PipelineOrchestrator::new(pool.inner().clone());
    orchestrator.get_active_draft(&story_id, chapter_number)
}

/// 合并修稿（用户接受修稿结果）
#[command(rename_all = "snake_case")]
pub async fn merge_revision(
    revision_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let orchestrator = PipelineOrchestrator::new(pool.inner().clone());
    orchestrator.merge_revision(&revision_id)
}

/// 获取草稿的修稿历史
#[command(rename_all = "snake_case")]
pub async fn get_draft_revision_history(
    draft_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::Revision>, AppError> {
    let orchestrator = PipelineOrchestrator::new(pool.inner().clone());
    orchestrator.get_draft_revision_history(&draft_id)
}

/// 获取草稿的审稿历史
#[command(rename_all = "snake_case")]
pub async fn get_draft_review_history(
    draft_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::PipelineReview>, AppError> {
    let orchestrator = PipelineOrchestrator::new(pool.inner().clone());
    orchestrator.get_draft_review_history(&draft_id)
}

/// 获取故事章节的草稿列表
#[command(rename_all = "snake_case")]
pub async fn get_story_chapter_drafts(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::Draft>, AppError> {
    let repo = DraftRepository::new(pool.inner().clone());
    repo.get_by_story_chapter(&story_id, chapter_number)
        .map_err(AppError::from)
}

/// 获取草稿的最新审稿报告
#[command(rename_all = "snake_case")]
pub async fn get_latest_pipeline_review(
    draft_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<crate::db::PipelineReview>, AppError> {
    let repo = PipelineReviewRepository::new(pool.inner().clone());
    repo.get_latest_by_draft(&draft_id).map_err(AppError::from)
}
