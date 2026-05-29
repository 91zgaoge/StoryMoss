//! Character commands

use tauri::{AppHandle, Manager, State};

use crate::{
    db::{CharacterRepository, CreateCharacterRequest, DbPool},
    error::AppError,
    SKILL_MANAGER,
};

#[tauri::command(rename_all = "snake_case")]
pub fn get_story_characters(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::Character>, AppError> {
    CharacterRepository::new(pool.inner().clone())
        .get_by_story(&story_id)
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_character(
    story_id: String,
    name: String,
    background: Option<String>,
    personality: Option<String>,
    goals: Option<String>,
    appearance: Option<String>,
    gender: Option<String>,
    age: Option<i32>,
    app: AppHandle,
    automation_service: tauri::State<crate::automation::service::AutomationService>,
    pool: State<'_, DbPool>,
) -> Result<crate::db::Character, AppError> {
    let character = CharacterRepository::new(pool.inner().clone())
        .create(CreateCharacterRequest {
            story_id: story_id.clone(),
            name: name.clone(),
            background,
            personality,
            goals,
            appearance,
            gender,
            age,
        })
        .map_err(AppError::from)?;

    // OnCharacterCreate hook
    if let Some(manager) = SKILL_MANAGER.get() {
        if let Ok(skill_manager) = manager.lock() {
            let story_id = character.story_id.clone();
            let character_id = character.id.clone();
            let character_name = character.name.clone();
            let skill_manager = skill_manager.clone();
            tauri::async_runtime::spawn(async move {
                let context = crate::agents::AgentContext::minimal(story_id, String::new());
                let data = serde_json::json!({ "character_id": character_id, "character_name": character_name });
                let _ = skill_manager
                    .execute_hooks(crate::skills::HookEvent::OnCharacterCreate, &context, data)
                    .await;
                log::info!(
                    "Hook executed: {:?}",
                    crate::skills::HookEvent::OnCharacterCreate
                );
            });
        }
    }

    let _ = crate::state_sync::StateSync::emit_character_created(
        &app,
        &story_id,
        &character.id,
        &character.name,
    );
    let automation_service_clone = automation_service.inner().clone();
    let story_id_clone = story_id.clone();
    let character_id_clone = character.id.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = automation_service_clone
            .trigger_event(
                crate::automation::triggers::TriggerEvent::CharacterCreated {
                    story_id: story_id_clone,
                    character_id: character_id_clone,
                },
            )
            .await
        {
            log::warn!(
                "[create_character] Failed to trigger character created automation: {}",
                e
            );
        }
    });
    Ok(character)
}

#[tauri::command(rename_all = "snake_case")]
pub fn update_character(
    id: String,
    name: Option<String>,
    background: Option<String>,
    personality: Option<String>,
    goals: Option<String>,
    appearance: Option<String>,
    gender: Option<String>,
    age: Option<i32>,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let pool = pool.inner().clone();
    let repo = CharacterRepository::new(pool.clone());
    // 先查询旧角色数据，用于级联改写对比（P0-3 修复: 避免 unwrap_or_default
    // 导致空字符串）
    let old_character = repo.get_by_id(&id).ok().flatten();
    let story_id_opt = old_character.as_ref().map(|c| c.story_id.clone());
    // 保存字段副本用于 Ingest（repo.update 会 move 走 Option 值）
    let name_for_ingest = name.clone();
    let background_for_ingest = background.clone();
    let personality_for_ingest = personality.clone();
    let goals_for_ingest = goals.clone();
    let appearance_for_ingest = appearance.clone();
    repo.update(
        &id,
        name,
        background,
        personality,
        goals,
        appearance,
        gender,
        age,
    )
    .map_err(AppError::from)?;
    if let Some(story_id) = story_id_opt.clone() {
        let _ = crate::state_sync::StateSync::emit_character_updated(
            &app,
            &id,
            name_for_ingest.as_deref(),
            &story_id,
        );

        // D1 Phase 4: 角色敏感字段变更触发级联改写（在 Ingest spawn
        // 之前执行，避免变量所有权冲突）
        if let Some(ref old) = old_character {
            let mut changed_fields = Vec::new();
            let mut before_map = serde_json::Map::new();
            let mut after_map = serde_json::Map::new();

            if let Some(ref new_val) = personality_for_ingest {
                if old.personality.as_ref() != Some(new_val) {
                    changed_fields.push("personality".to_string());
                    before_map.insert(
                        "personality".to_string(),
                        serde_json::json!(old.personality),
                    );
                    after_map.insert("personality".to_string(), serde_json::json!(new_val));
                }
            }
            if let Some(ref new_val) = goals_for_ingest {
                if old.goals.as_ref() != Some(new_val) {
                    changed_fields.push("goals".to_string());
                    before_map.insert("goals".to_string(), serde_json::json!(old.goals));
                    after_map.insert("goals".to_string(), serde_json::json!(new_val));
                }
            }
            if let Some(ref new_val) = appearance_for_ingest {
                if old.appearance.as_ref() != Some(new_val) {
                    changed_fields.push("appearance".to_string());
                    before_map.insert("appearance".to_string(), serde_json::json!(old.appearance));
                    after_map.insert("appearance".to_string(), serde_json::json!(new_val));
                }
            }
            if let Some(ref new_val) = background_for_ingest {
                if old.background.as_ref() != Some(new_val) {
                    changed_fields.push("background".to_string());
                    before_map.insert("background".to_string(), serde_json::json!(old.background));
                    after_map.insert("background".to_string(), serde_json::json!(new_val));
                }
            }

            if !changed_fields.is_empty() {
                before_map.insert("name".to_string(), serde_json::json!(old.name));
                after_map.insert("name".to_string(), serde_json::json!(old.name));

                let before_json = serde_json::to_string(&before_map).unwrap_or_default();
                let after_json = serde_json::to_string(&after_map).unwrap_or_default();

                let change_event = crate::creative_engine::cascade_rewriter::models::EntityChangeEvent {
                    story_id: story_id.clone(),
                    entity_id: id.clone(),
                    entity_type: "character".to_string(),
                    entity_name: old.name.clone(),
                    change_type: crate::creative_engine::cascade_rewriter::models::ChangeType::AttributeModified,
                    before_json,
                    after_json,
                    changed_fields,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };

                let payload =
                    crate::creative_engine::cascade_rewriter::models::CascadeTaskPayload {
                        story_id: story_id.clone(),
                        change_events: vec![change_event],
                    };

                let payload_json = serde_json::to_string(&payload).unwrap_or_default();

                let req = crate::task_system::models::CreateTaskRequest {
                    name: format!("级联改写: {}", old.name),
                    description: Some(format!("因角色 {} 的设定变更触发的场景级联改写", old.name)),
                    task_type: "cascade_rewrite".to_string(),
                    schedule_type: "once".to_string(),
                    cron_pattern: None,
                    payload: Some(payload_json),
                    enabled: Some(true),
                    max_retries: Some(3),
                    heartbeat_timeout_seconds: Some(300),
                };

                if let Some(task_service) =
                    app.try_state::<crate::task_system::service::TaskService>()
                {
                    match task_service.create_task(req) {
                        Ok(task) => log::info!(
                            "[CascadeRewrite] Created task {} for character {}",
                            task.id,
                            id
                        ),
                        Err(e) => log::warn!(
                            "[CascadeRewrite] Failed to create task for character {}: {}",
                            id,
                            e
                        ),
                    }
                }
            }
        }

        // P0 修复: 角色变更触发 Ingest，更新知识图谱
        let pool_for_pipeline = pool.clone();
        let pool_for_kg = pool.clone();
        let character_id = id.clone();
        let story_id_for_ingest = story_id.clone();
        let app_handle_clone = app.clone();
        let name_for_ingest_spawn = name_for_ingest.clone();
        let background_for_ingest_spawn = background_for_ingest.clone();
        let personality_for_ingest_spawn = personality_for_ingest.clone();
        let goals_for_ingest_spawn = goals_for_ingest.clone();
        let appearance_for_ingest_spawn = appearance_for_ingest.clone();
        tauri::async_runtime::spawn(async move {
            let llm_service = crate::llm::LlmService::new(app_handle_clone.clone());
            let pipeline = crate::memory::ingest::IngestPipeline::new(llm_service)
                .with_pool(pool_for_pipeline)
                .with_app_handle(app_handle_clone.clone());
            let ingest_text = format!(
                "角色: {}\n背景: {}\n性格: {}\n目标: {}\n外貌: {}",
                name_for_ingest_spawn.as_deref().unwrap_or(""),
                background_for_ingest_spawn.as_deref().unwrap_or(""),
                personality_for_ingest_spawn.as_deref().unwrap_or(""),
                goals_for_ingest_spawn.as_deref().unwrap_or(""),
                appearance_for_ingest_spawn.as_deref().unwrap_or("")
            );
            let content = crate::memory::ingest::IngestContent {
                text: ingest_text,
                source: format!("character:{}", character_id),
                story_id: story_id_for_ingest.clone(),
                scene_id: None,
            };
            match pipeline.ingest(&content).await {
                Ok(result) => {
                    let kg_repo =
                        crate::db::repositories::KnowledgeGraphRepository::new(pool_for_kg);
                    for entity in &result.entities {
                        let _ = kg_repo.create_entity(
                            &story_id_for_ingest,
                            &entity.name,
                            &entity.entity_type.to_string(),
                            &entity.attributes,
                            entity.embedding.clone(),
                        );
                    }
                    log::info!(
                        "[AutoIngest] Character {}: {} entities saved to KG",
                        character_id,
                        result.entities.len()
                    );
                    let _ = crate::state_sync::StateSync::emit_data_refresh(
                        &app_handle_clone,
                        Some(&story_id_for_ingest),
                        "knowledgeGraph",
                    );
                }
                Err(e) => {
                    log::warn!(
                        "[AutoIngest] Character {} ingest failed: {}",
                        character_id,
                        e
                    );
                }
            }
        });
    }
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_character(
    id: String,
    app: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let repo = CharacterRepository::new(pool.inner().clone());
    // 先查询 story_id，删除后无法再获取（P0-3 修复: 避免 unwrap_or_default
    // 导致空字符串）
    let story_id_opt = repo.get_by_id(&id).ok().flatten().map(|c| c.story_id);
    repo.delete(&id).map_err(AppError::from)?;
    if let Some(story_id) = story_id_opt {
        let _ = crate::state_sync::StateSync::emit_character_deleted(&app, &id, &story_id);
    }
    Ok(())
}
