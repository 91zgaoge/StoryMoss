//! Intent commands

use tauri::{AppHandle, State};

use crate::{db::DbPool, error::AppError, LearningPoint, RecordFeedbackRequest};

// Intent Parser Command
#[tauri::command(rename_all = "snake_case")]
pub async fn parse_intent(
    pool: State<'_, DbPool>,
    user_input: String,
    app_handle: AppHandle,
) -> Result<crate::intent::Intent, AppError> {
    let _pool = pool;
    let parser = crate::intent::IntentParser::new(app_handle);
    parser.parse(&user_input).await.map_err(AppError::from)
}

// Intent Executor Command
#[tauri::command(rename_all = "snake_case")]
pub async fn execute_intent(
    pool: State<'_, DbPool>,
    intent: crate::intent::Intent,
    story_id: String,
    app_handle: AppHandle,
) -> Result<crate::intent::IntentExecutionResult, AppError> {
    let _pool = pool;
    let executor = crate::intent::IntentExecutor::new(app_handle);
    executor
        .execute(intent, story_id)
        .await
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn record_feedback(
    pool: State<'_, DbPool>,
    request: RecordFeedbackRequest,
    app: AppHandle,
) -> Result<Vec<LearningPoint>, AppError> {
    let pool = pool.inner().clone();
    let recorder = crate::creative_engine::adaptive::FeedbackRecorder::new(pool.clone());
    let result = match request.feedback_type.as_str() {
        "accept" => recorder.record_accept(
            &request.story_id,
            &request.original_ai_text,
            request.agent_type.as_deref(),
        ),
        "reject" => recorder.record_reject(
            &request.story_id,
            &request.original_ai_text,
            request.agent_type.as_deref(),
        ),
        "modify" => recorder.record_modify(
            &request.story_id,
            &request.original_ai_text,
            request.final_text.as_deref().unwrap_or(""),
            request.agent_type.as_deref(),
        ),
        _ => Err(AppError::validation_failed(
            "Unknown feedback type",
            None::<String>,
        )),
    };

    if result.is_err() {
        return Err(result.err().unwrap());
    }
    let miner = crate::creative_engine::adaptive::PreferenceMiner::new(pool.clone());
    let learnings = match miner.mine(&request.story_id) {
        Ok(prefs) => prefs
            .into_iter()
            .filter(|p| p.confidence >= 0.5)
            .take(3)
            .map(|p| LearningPoint {
                category: p.preference_type,
                observation: format!(
                    "{}: {} (置信度{:.0}%)",
                    p.preference_key,
                    p.preference_value,
                    p.confidence * 100.0
                ),
                impact: p.reasoning,
            })
            .collect(),
        Err(e) => {
            log::warn!("[record_feedback] Preference mining failed: {}", e);
            vec![]
        }
    };

    // 异步触发偏好挖掘保存，让自适应学习系统形成闭环
    let story_id = request.story_id.clone();
    tauri::async_runtime::spawn(async move {
        let engine = crate::creative_engine::adaptive::AdaptiveLearningEngine::new(pool);
        match engine.mine_preferences(&story_id) {
            Ok(prefs) if !prefs.is_empty() => {
                log::info!(
                    "[Adaptive] Mined {} preferences for story {}",
                    prefs.len(),
                    story_id
                );
            }
            Ok(_) => {}
            Err(e) => log::warn!("[Adaptive] Preference mining failed: {}", e),
        }
    });

    let _ = crate::state_sync::StateSync::emit_data_refresh(
        &app,
        Some(&request.story_id),
        "learningPoints",
    );
    Ok(learnings)
}
