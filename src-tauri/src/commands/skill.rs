//! Skill commands

use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::{
    commands::EmitSync, db::DbPool, error::AppError, router::TaskType, skills::SkillInfo,
    SKILL_MANAGER,
};

fn task_type_for_category(category: &crate::skills::SkillCategory) -> TaskType {
    match category {
        crate::skills::SkillCategory::Analysis | crate::skills::SkillCategory::Plot => {
            TaskType::Analysis
        }
        crate::skills::SkillCategory::Style => TaskType::Editing,
        crate::skills::SkillCategory::Export => TaskType::Summarization,
        crate::skills::SkillCategory::WorldBuilding | crate::skills::SkillCategory::Character => {
            TaskType::WorldBuilding
        }
        crate::skills::SkillCategory::Writing
        | crate::skills::SkillCategory::Integration
        | crate::skills::SkillCategory::Custom => TaskType::CreativeWriting,
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_skills() -> Result<Vec<SkillInfo>, AppError> {
    let skills = SKILL_MANAGER
        .get()
        .ok_or(AppError::internal("Skills not initialized"))?
        .lock()
        .map_err(|e| crate::error::AppError::from(e).to_string())?
        .get_all_skills();
    Ok(skills.into_iter().map(SkillInfo::from).collect())
}

#[tauri::command(rename_all = "snake_case")]
pub fn import_skill(path: String, app: AppHandle) -> Result<SkillInfo, AppError> {
    SKILL_MANAGER
        .get()
        .ok_or(AppError::internal("Skills not initialized"))?
        .lock()
        .map_err(|e| crate::error::AppError::from(e).to_string())?
        .import_skill(std::path::Path::new(&path))
        .map_err(AppError::from)
        .map(SkillInfo::from)
        .emit_sync(&app, None, "skills")
}

#[tauri::command(rename_all = "snake_case")]
pub fn enable_skill(skill_id: String, app: AppHandle) -> Result<(), AppError> {
    SKILL_MANAGER
        .get()
        .ok_or(AppError::internal("Skills not initialized"))?
        .lock()
        .map_err(|e| crate::error::AppError::from(e).to_string())?
        .enable_skill(&skill_id)
        .map_err(AppError::from)
        .emit_sync(&app, None, "skills")
}

#[tauri::command(rename_all = "snake_case")]
pub fn disable_skill(skill_id: String, app: AppHandle) -> Result<(), AppError> {
    SKILL_MANAGER
        .get()
        .ok_or(AppError::internal("Skills not initialized"))?
        .lock()
        .map_err(|e| crate::error::AppError::from(e).to_string())?
        .disable_skill(&skill_id)
        .map_err(AppError::from)
        .emit_sync(&app, None, "skills")
}

#[tauri::command(rename_all = "snake_case")]
pub fn uninstall_skill(skill_id: String, app: AppHandle) -> Result<(), AppError> {
    SKILL_MANAGER
        .get()
        .ok_or(AppError::internal("Skills not initialized"))?
        .lock()
        .map_err(|e| crate::error::AppError::from(e).to_string())?
        .uninstall_skill(&skill_id)
        .map_err(AppError::from)
        .emit_sync(&app, None, "skills")
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_skill(skill_id: String) -> Result<SkillInfo, AppError> {
    let skill = SKILL_MANAGER
        .get()
        .ok_or(AppError::internal("Skills not initialized"))?
        .lock()
        .map_err(|e| crate::error::AppError::from(e).to_string())?
        .get_skill(&skill_id);
    skill
        .map(SkillInfo::from)
        .ok_or_else(|| AppError::not_found("Skill", &skill_id))
}

#[tauri::command(rename_all = "snake_case")]
pub fn update_skill(
    skill_id: String,
    manifest: crate::skills::SkillManifest,
    app: AppHandle,
) -> Result<(), AppError> {
    SKILL_MANAGER
        .get()
        .ok_or(AppError::internal("Skills not initialized"))?
        .lock()
        .map_err(|e| crate::error::AppError::from(e).to_string())?
        .update_skill(&skill_id, manifest)
        .map_err(AppError::from)
        .emit_sync(&app, None, "skills")
}

#[tauri::command(rename_all = "snake_case")]
pub async fn execute_skill(
    skill_id: String,
    params: HashMap<String, serde_json::Value>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, AppError> {
    let mut params = params;
    let story_id = params
        .remove("story_id")
        .and_then(|v| v.as_str().map(|s| s.to_string()));

    // Build context from database if story_id is provided
    let context = if let Some(story_id) = story_id {
        match app_handle.try_state::<DbPool>() {
            Some(pool_state) => {
                let pool = pool_state.inner().clone();
                let builder = crate::creative_engine::StoryContextBuilder::new(pool);
                match builder.build_quick(&story_id).await {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        log::warn!(
                            "[execute_skill] StoryContextBuilder failed: {}, using minimal context",
                            e
                        );
                        crate::domain::agent_context::AgentContext::minimal(story_id, String::new())
                    }
                }
            }
            None => {
                log::warn!("[execute_skill] DbPool not available, using minimal context");
                crate::domain::agent_context::AgentContext::minimal(story_id, String::new())
            }
        }
    } else {
        crate::domain::agent_context::AgentContext::default()
    };

    // Execute skill
    let manager = {
        let guard = SKILL_MANAGER
            .get()
            .ok_or(AppError::internal("Skills not initialized"))?
            .lock()
            .map_err(|e| crate::error::AppError::from(e).to_string())?;
        guard.clone()
    };

    let skill_category = manager.get_skill(&skill_id).map(|s| s.manifest.category);

    let result = manager.execute_skill(&skill_id, &context, params).await?;

    if !result.success {
        return Err(AppError::internal(
            result.error.unwrap_or("Skill execution failed".to_string()),
        ));
    }

    // If LLM was already called (PromptRuntime with llm_service), return content
    // directly
    if let Some(content) = result.data.get("content").and_then(|v| v.as_str()) {
        return Ok(serde_json::json!({
            "success": true,
            "content": content,
            "model": result.data.get("model").and_then(|v| v.as_str()).unwrap_or(""),
            "tokens_used": result.data.get("tokens_used").and_then(|v| v.as_i64()).unwrap_or(0),
            "execution_time_ms": result.execution_time_ms,
        }));
    }

    // Fallback: skill returned prompts but no LLM result, call LLM manually
    let system_prompt = result
        .data
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let user_prompt = result
        .data
        .get("user_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if system_prompt.is_empty() && user_prompt.is_empty() {
        return Err(AppError::internal("Skill did not produce a valid prompt"));
    }

    let llm_service = crate::llm::LlmService::new(app_handle);
    let full_prompt = if system_prompt.is_empty() {
        user_prompt.to_string()
    } else {
        format!(
            "[系统指令]\n{}\n\n[用户请求]\n{}",
            system_prompt, user_prompt
        )
    };

    let max_tokens = result
        .data
        .get("max_tokens")
        .and_then(|v| v.as_i64())
        .map(|v| v.clamp(1, 16384) as i32)
        .unwrap_or(2000);
    let temperature = result
        .data
        .get("temperature")
        .and_then(|v| v.as_f64())
        .map(|v| v.clamp(0.0, 2.0) as f32)
        .unwrap_or(0.7);

    let task_type = skill_category
        .as_ref()
        .map(task_type_for_category)
        .unwrap_or(TaskType::CreativeWriting);
    let response = llm_service
        .generate_for_task(
            task_type,
            full_prompt,
            Some(max_tokens),
            Some(temperature),
            Some(&skill_id),
        )
        .await?;

    Ok(serde_json::json!({
        "success": true,
        "content": response.content,
        "model": response.model,
        "tokens_used": response.tokens_used,
        "execution_time_ms": result.execution_time_ms,
    }))
}

/// 使用 text_formatter skill 对文本进行智能排版
#[tauri::command(rename_all = "snake_case")]
pub async fn format_text(content: String, app: AppHandle) -> Result<String, AppError> {
    let result = execute_skill(
        "builtin.text_formatter".to_string(),
        {
            let mut p = HashMap::new();
            p.insert("content".to_string(), serde_json::Value::String(content));
            p
        },
        app,
    )
    .await?;

    result
        .get("content")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::internal("LLM returned empty content"))
}
