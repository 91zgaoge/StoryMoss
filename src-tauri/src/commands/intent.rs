//! Intent commands

use tauri::{AppHandle, State};

use crate::{
    creative_engine::adaptive::{FeedbackEvent, FeedbackType, PreferencePairExporter},
    db::DbPool,
    error::AppError,
    LearningPoint, RecordFeedbackRequest,
};

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

    let feedback_type = match request.feedback_type.as_str() {
        "accept" => FeedbackType::Accept,
        "reject" => FeedbackType::Reject,
        "modify" => FeedbackType::Modify,
        _ => {
            return Err(AppError::validation_failed(
                "Unknown feedback type",
                None::<String>,
            ))
        }
    };

    let final_text = request
        .final_text
        .clone()
        .unwrap_or_else(|| request.original_ai_text.clone());
    let subsequent_edit_diff = request.subsequent_edit_diff.clone().or_else(|| {
        if feedback_type == FeedbackType::Modify {
            Some(final_text.clone())
        } else {
            None
        }
    });

    let event = FeedbackEvent {
        story_id: request.story_id.clone(),
        scene_id: request.scene_id.clone(),
        chapter_id: request.chapter_id.clone(),
        feedback_type,
        agent_type: request.agent_type.clone(),
        original_ai_text: request.original_ai_text.clone(),
        final_text,
        ai_score: None,
        user_satisfaction: None,
        original_prompt: request.original_prompt.clone(),
        generated_content: request
            .generated_content
            .clone()
            .or(Some(request.original_ai_text.clone())),
        subsequent_edit_diff,
    };

    let recorder = crate::creative_engine::adaptive::FeedbackRecorder::new(pool.clone());
    recorder.record(event.clone())?;

    // 导出偏好对到 `.storyforge/feedback/preference_pairs.jsonl`
    if let Err(e) = PreferencePairExporter::export(&app, &event) {
        log::warn!("[record_feedback] Preference pair export failed: {}", e);
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
