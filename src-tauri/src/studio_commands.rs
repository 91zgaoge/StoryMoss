//! Studio Commands

#![allow(unused_imports)]

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Manager, State};

use crate::{
    agents::novel_creation::{
        CharacterProfileOption, GenerationOptions, NovelCreationAgent, SceneProposal,
        WorldBuildingOption, WritingStyleOption,
    },
    config::StudioManager,
    db::{
        AgentBotConfig, AnchorType, ChangeStatus, ChangeTrack, ChangeTrackRepository, ChangeType,
        Chapter, ChapterRepo, ChapterRepository, Character, CharacterConflict,
        CharacterRelationshipRepository, CharacterRepo, CharacterRepository, CharacterState,
        CommentMessage, CommentThread, CommentThreadRepository, CommentThreadWithMessages,
        ConflictType, CreateChapterRequest, CreateCharacterRequest, CreateStoryRequest,
        CreatorType, Culture, DbPool, Entity, KnowledgeGraphRepository, LlmStudioConfig, Relation,
        Scene, SceneAnnotation, SceneAnnotationRepository, SceneRepo, SceneRepository, SceneUpdate,
        SceneVersion, SceneVersionRepository, Story, StoryOutlineRepository, StoryRepo,
        StoryRepository, StoryStyleConfigRepository, StorySummary, StorySummaryRepository,
        StudioConfig, StudioConfigRepository, StudioExportRequest, StyleDnaRepository,
        TextAnnotation, TextAnnotationRepository, ThreadStatus, UiStudioConfig, UpdateStoryRequest,
        WorldBuilding, WorldBuildingRepo, WorldBuildingRepository, WorldRule, WritingStyle,
        WritingStyleRepo, WritingStyleRepository, WritingStyleUpdate,
    },
    error::AppError,
    llm::LlmService,
    memory::{
        ingest::{IngestContent, IngestPipeline},
        retention::RetentionManager,
    },
    revision_commands::CharacterQuickView,
};

#[command(rename_all = "snake_case")]
pub async fn create_world_building(
    story_id: String,
    concept: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<WorldBuilding, AppError> {
    log::info!(
        "[story_commands] {} called: story_id={}",
        "create_world_building",
        story_id
    );
    let repo = WorldBuildingRepository::new(pool.inner().clone());
    let wb = repo.create(&story_id, &concept).map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "create_world_building", e);
        AppError::from(e)
    })?;
    let _ =
        crate::state_sync::StateSync::emit_world_building_created(&app_handle, &story_id, &wb.id);
    let _ = crate::state_sync::StateSync::emit_world_building_updated(&app_handle, &story_id);
    Ok(wb)
}

#[command(rename_all = "snake_case")]
pub async fn get_world_building(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<WorldBuilding>, AppError> {
    let repo = WorldBuildingRepository::new(pool.inner().clone());
    repo.get_by_story(&story_id).map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_world_building(
    id: String,
    concept: Option<String>,
    rules: Option<Vec<WorldRule>>,
    history: Option<String>,
    cultures: Option<Vec<Culture>>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    log::info!(
        "[story_commands] {} called: id={}",
        "update_world_building",
        id
    );
    let repo = WorldBuildingRepository::new(pool.inner().clone());
    let old_wb = repo.get_by_id(&id).ok().flatten();
    let result = repo
        .update(
            &id,
            concept.as_deref(),
            rules.as_deref(),
            history.as_deref(),
            cultures.as_deref(),
        )
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "update_world_building", e);
            AppError::from(e)
        })?;

    // OnWorldBuildingUpdate hook
    let story_id_for_sync = old_wb.as_ref().map(|wb| wb.story_id.clone()).or_else(|| {
        pool.inner().get().ok().and_then(|c| {
            c.query_row(
                "SELECT story_id FROM world_buildings WHERE id = ?",
                [&id],
                |row| row.get::<_, String>(0),
            )
            .ok()
        })
    });
    if let Some(ref story_id) = story_id_for_sync {
        let _ = crate::state_sync::StateSync::emit_world_building_updated(&app_handle, story_id);
    }
    if let Some(story_id) = story_id_for_sync.clone() {
        if let Some(manager) = crate::SKILL_MANAGER.get() {
            if let Ok(skill_manager) = manager.lock() {
                let world_building_id = id.clone();
                let skill_manager = skill_manager.clone();
                let story_id_for_hook = story_id.clone();
                tauri::async_runtime::spawn(async move {
                    let context =
                        crate::agents::AgentContext::minimal(story_id_for_hook, String::new());
                    let data = serde_json::json!({ "world_building_id": world_building_id });
                    let _ = skill_manager
                        .execute_hooks(
                            crate::skills::HookEvent::OnWorldBuildingUpdate,
                            &context,
                            data,
                        )
                        .await;
                    log::info!(
                        "Hook executed: {:?}",
                        crate::skills::HookEvent::OnWorldBuildingUpdate
                    );
                });
            }
        }

        // D1 Phase 4: 世界观敏感字段变更触发级联改写
        if let Some(ref old) = old_wb {
            let mut changed_fields = Vec::new();
            let mut before_map = serde_json::Map::new();
            let mut after_map = serde_json::Map::new();

            if let Some(ref new_val) = concept {
                if &old.concept != new_val {
                    changed_fields.push("concept".to_string());
                    before_map.insert("concept".to_string(), serde_json::json!(old.concept));
                    after_map.insert("concept".to_string(), serde_json::json!(new_val));
                }
            }
            if let Some(ref new_val) = history {
                if old.history.as_ref() != Some(new_val) {
                    changed_fields.push("history".to_string());
                    before_map.insert("history".to_string(), serde_json::json!(old.history));
                    after_map.insert("history".to_string(), serde_json::json!(new_val));
                }
            }
            if let Some(ref new_val) = rules {
                let old_rules_json = serde_json::to_string(&old.rules).unwrap_or_default();
                let new_rules_json = serde_json::to_string(new_val).unwrap_or_default();
                if old_rules_json != new_rules_json {
                    changed_fields.push("rules".to_string());
                    before_map.insert("rules".to_string(), serde_json::json!(old.rules));
                    after_map.insert("rules".to_string(), serde_json::json!(new_val));
                }
            }
            if let Some(ref new_val) = cultures {
                let old_cultures_json = serde_json::to_string(&old.cultures).unwrap_or_default();
                let new_cultures_json = serde_json::to_string(new_val).unwrap_or_default();
                if old_cultures_json != new_cultures_json {
                    changed_fields.push("cultures".to_string());
                    before_map.insert("cultures".to_string(), serde_json::json!(old.cultures));
                    after_map.insert("cultures".to_string(), serde_json::json!(new_val));
                }
            }

            if !changed_fields.is_empty() {
                before_map.insert("id".to_string(), serde_json::json!(old.id));
                after_map.insert("id".to_string(), serde_json::json!(old.id));

                let before_json = serde_json::to_string(&before_map).unwrap_or_default();
                let after_json = serde_json::to_string(&after_map).unwrap_or_default();

                let change_event = crate::creative_engine::cascade_rewriter::models::EntityChangeEvent {
                    story_id: story_id.clone(),
                    entity_id: id.clone(),
                    entity_type: "world_building".to_string(),
                    entity_name: old.concept.clone(),
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
                    name: format!("级联改写: {}", old.concept),
                    description: Some(format!(
                        "因世界观 {} 的设定变更触发的场景级联改写",
                        old.concept
                    )),
                    task_type: "cascade_rewrite".to_string(),
                    schedule_type: "once".to_string(),
                    cron_pattern: None,
                    payload: Some(payload_json),
                    enabled: Some(true),
                    max_retries: Some(3),
                    heartbeat_timeout_seconds: Some(300),
                };

                if let Some(task_service) =
                    app_handle.try_state::<crate::task_system::service::TaskService>()
                {
                    match task_service.create_task(req) {
                        Ok(task) => log::info!(
                            "[CascadeRewrite] Created task {} for world_building {}",
                            task.id,
                            id
                        ),
                        Err(e) => log::warn!(
                            "[CascadeRewrite] Failed to create task for world_building {}: {}",
                            id,
                            e
                        ),
                    }
                }
            }
        }
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn delete_world_building(
    id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    log::info!(
        "[story_commands] {} called: id={}",
        "delete_world_building",
        id
    );
    let repo = WorldBuildingRepository::new(pool.inner().clone());
    // 先查询 story_id 用于同步事件（删除后无法获取）
    let story_id_opt = pool.inner().get().ok().and_then(|c| {
        c.query_row(
            "SELECT story_id FROM world_buildings WHERE id = ?",
            [&id],
            |row| row.get::<_, String>(0),
        )
        .ok()
    });
    let result = repo.delete(&id).map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "delete_world_building", e);
        e.to_string()
    })?;
    if let Some(ref story_id) = story_id_opt {
        let _ =
            crate::state_sync::StateSync::emit_world_building_deleted(&app_handle, story_id, &id);
        let _ = crate::state_sync::StateSync::emit_world_building_updated(&app_handle, story_id);
    }
    Ok(result)
}

// ==================== 文字风格命令 ====================

#[command(rename_all = "snake_case")]
pub async fn create_writing_style(
    story_id: String,
    name: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<WritingStyle, AppError> {
    let repo = WritingStyleRepository::new(pool.inner().clone());
    let result = repo
        .create(&story_id, name.as_deref())
        .map_err(AppError::from)?;
    let _ = crate::state_sync::StateSync::emit_data_refresh(
        &app_handle,
        Some(&story_id),
        "writingStyles",
    );
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn get_writing_style(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<WritingStyle>, AppError> {
    let repo = WritingStyleRepository::new(pool.inner().clone());
    repo.get_by_story(&story_id).map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_writing_style(
    id: String,
    updates: WritingStyleUpdate,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = WritingStyleRepository::new(pool.inner().clone());
    let count = repo.update(&id, &updates).map_err(AppError::from)?;

    // P2-15 修复: 查询 story_id 并发射同步事件
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM writing_styles WHERE id = ?1",
        [&id],
        |row| row.get(0),
    );
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "writingStyle",
        );
    }
    Ok(count)
}

// ==================== 工作室配置命令 ====================

#[command(rename_all = "snake_case")]
pub async fn create_studio_config(
    story_id: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<StudioConfig, AppError> {
    let app_dir = app_handle.path().app_data_dir().map_err(AppError::from)?;
    let manager = StudioManager::new(pool.inner().clone(), &app_dir);
    let result = manager
        .create_default_studio(&story_id, "")
        .map_err(AppError::from)?;
    let _ = crate::state_sync::StateSync::emit_data_refresh(
        &app_handle,
        Some(&story_id),
        "studioConfig",
    );
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn get_studio_config(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<StudioConfig>, AppError> {
    let repo = StudioConfigRepository::new(pool.inner().clone());
    repo.get_by_story(&story_id).map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_studio_config(
    id: String,
    pen_name: Option<String>,
    llm_config: Option<LlmStudioConfig>,
    ui_config: Option<UiStudioConfig>,
    agent_bots: Option<Vec<AgentBotConfig>>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = StudioConfigRepository::new(pool.inner().clone());
    let result = repo
        .update(
            &id,
            pen_name.as_deref(),
            llm_config.as_ref(),
            ui_config.as_ref(),
            agent_bots.as_deref(),
        )
        .map_err(AppError::from)?;
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM studio_configs WHERE id = ?1",
        [&id],
        |row| row.get(0),
    );
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "studioConfig",
        );
    }
    Ok(result)
}

// ==================== 导入/导出命令 ====================

#[command(rename_all = "snake_case")]
pub async fn export_studio(
    request: StudioExportRequest,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<Vec<u8>, AppError> {
    let app_dir = app_handle.path().app_data_dir().map_err(AppError::from)?;
    let manager = StudioManager::new(pool.inner().clone(), &app_dir);
    manager.export_studio(&request).map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn import_studio(
    data: Vec<u8>,
    options: crate::config::studio_manager::ImportOptions,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<Story, AppError> {
    let app_dir = app_handle.path().app_data_dir().map_err(AppError::from)?;
    let manager = StudioManager::new(pool.inner().clone(), &app_dir);
    manager
        .import_studio(&data, &options)
        .map_err(AppError::from)
}

// ==================== 知识图谱命令 ====================
// EVENT_REQUIRED: 所有 KG 变更命令在成功执行后必须发射 sync-event，
// 确保前后台知识图谱视图自动刷新。新增/修改命令时请在 code review 中确认。

#[command(rename_all = "snake_case")]
pub async fn create_entity(
    story_id: String,
    name: String,
    entity_type: String,
    attributes: serde_json::Value,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<Entity, AppError> {
    log::info!(
        "[story_commands] {} called: story_id={}, name={}, entity_type={}",
        "create_entity",
        story_id,
        name,
        entity_type
    );
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    // EVENT_REQUIRED
    let result = repo
        .create_entity(&story_id, &name, &entity_type, &attributes, None)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_entity", e);
            AppError::from(e)
        })?;
    let _ = crate::state_sync::StateSync::emit_data_refresh(
        &app_handle,
        Some(&story_id),
        "knowledgeGraph",
    );
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn update_entity(
    entity_id: String,
    name: Option<String>,
    attributes: Option<serde_json::Value>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<Entity, AppError> {
    use std::collections::HashMap;

    use crate::embeddings::{embed_entity_async, EntityEmbeddingRequest};

    log::info!(
        "[story_commands] {} called: entity_id={}",
        "update_entity",
        entity_id
    );
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let existing = repo
        .get_entity_by_id(&entity_id)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "update_entity", e);
            AppError::from(e)
        })?
        .ok_or("Entity not found")?;

    let new_name = name.as_deref().unwrap_or(&existing.name);
    let new_attrs = attributes.as_ref().unwrap_or(&existing.attributes);

    // Auto-regenerate embedding when attributes or name changes
    let embedding = if name.is_some() || attributes.is_some() {
        let attrs_map: HashMap<String, serde_json::Value> = match new_attrs {
            serde_json::Value::Object(map) => {
                map.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
            }
            _ => HashMap::new(),
        };
        let request = EntityEmbeddingRequest {
            entity_id: entity_id.clone(),
            name: new_name.to_string(),
            description: new_attrs
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            entity_type: existing.entity_type.to_string(),
            attributes: attrs_map,
        };
        embed_entity_async(request).await.ok()
    } else {
        existing.embedding
    };

    // EVENT_REQUIRED
    let result = repo
        .update_entity(&entity_id, Some(new_name), Some(new_attrs), embedding)
        .map_err(AppError::from)?;

    let story_id_for_sync = pool.inner().get().ok().and_then(|c| {
        c.query_row(
            "SELECT story_id FROM entities WHERE id = ?",
            [&entity_id],
            |row| row.get::<_, String>(0),
        )
        .ok()
    });
    if let Some(ref story_id) = story_id_for_sync {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(story_id),
            "knowledgeGraph",
        );
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn get_story_entities(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<Entity>, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    repo.get_entities_by_story(&story_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn create_relation(
    story_id: String,
    source_id: String,
    target_id: String,
    relation_type: String,
    strength: f32,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<Relation, AppError> {
    // EVENT_REQUIRED
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let result = repo
        .create_relation(&story_id, &source_id, &target_id, &relation_type, strength)
        .map_err(AppError::from)?;
    let _ = crate::state_sync::StateSync::emit_data_refresh(
        &app_handle,
        Some(&story_id),
        "knowledgeGraph",
    );
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn get_entity_relations(
    entity_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<Relation>, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    repo.get_relations_by_entity(&entity_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_ingest_jobs(
    story_id: String,
    limit: i32,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::memory::ingest::IngestJob>, AppError> {
    crate::memory::ingest::IngestPipeline::get_recent_jobs(&story_id, limit, pool.inner())
        .map_err(AppError::from)
}

// ==================== 场景批注命令 ====================

#[command(rename_all = "snake_case")]
pub async fn compress_content(
    story_id: String,
    content: String,
    target_ratio: Option<f32>,
    app_handle: AppHandle,
) -> Result<crate::agents::AgentResult, AppError> {
    use std::collections::HashMap;

    use crate::agents::{
        commands::ExecuteAgentRequest,
        service::{AgentService, AgentTask, AgentType},
    };

    log::info!(
        "[story_commands] {} called: story_id={}",
        "compress_content",
        story_id
    );
    let parameters = target_ratio.map(|r| {
        let mut map = HashMap::new();
        map.insert("target_ratio".to_string(), serde_json::json!(r));
        map
    });

    let request = ExecuteAgentRequest {
        agent_type: AgentType::MemoryCompressor,
        story_id: story_id.clone(),
        chapter_number: None,
        input: content.clone(),
        parameters: parameters.clone(),
    };

    let context = crate::agents::commands::build_agent_context(&app_handle, &request).await?;
    let task = AgentTask {
        id: uuid::Uuid::new_v4().to_string(),
        agent_type: AgentType::MemoryCompressor,
        context,
        input: content,
        parameters: parameters.unwrap_or_default(),
        tier: None,
    };

    let service = AgentService::new(app_handle);
    service.execute_task(task).await.map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "compress_content", e);
        e
    })
}

#[command(rename_all = "snake_case")]
pub async fn compress_scene(
    scene_id: String,
    target_ratio: Option<f32>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::agents::AgentResult, AppError> {
    use std::collections::HashMap;

    use crate::agents::{
        commands::ExecuteAgentRequest,
        service::{AgentService, AgentTask, AgentType},
    };

    log::info!(
        "[story_commands] {} called: scene_id={}",
        "compress_scene",
        scene_id
    );
    let scene_repo = SceneRepository::new(pool.inner().clone());
    let scene = scene_repo
        .get_by_id(&scene_id)
        .map_err(|e| {
            log::error!(
                "[story_commands] {} scene lookup failed: {}",
                "compress_scene",
                e
            );
            AppError::from(e)
        })?
        .ok_or("Scene not found")?;

    let content = scene.content.unwrap_or_default();
    if content.trim().is_empty() {
        return Err(AppError::internal("Scene has no content to compress"));
    }

    let parameters = target_ratio.map(|r| {
        let mut map = HashMap::new();
        map.insert("target_ratio".to_string(), serde_json::json!(r));
        map
    });

    let request = ExecuteAgentRequest {
        agent_type: AgentType::MemoryCompressor,
        story_id: scene.story_id.clone(),
        chapter_number: Some(scene.sequence_number.max(0) as u32),
        input: content.clone(),
        parameters: parameters.clone(),
    };

    let context = crate::agents::commands::build_agent_context(&app_handle, &request).await?;
    let task = AgentTask {
        id: uuid::Uuid::new_v4().to_string(),
        agent_type: AgentType::MemoryCompressor,
        context,
        input: content,
        parameters: parameters.unwrap_or_default(),
        tier: None,
    };

    let service = AgentService::new(app_handle);
    service.execute_task(task).await.map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "compress_scene", e);
        e
    })
}

// ==================== 知识蒸馏命令 ====================

#[command(rename_all = "snake_case")]
pub async fn distill_story_knowledge(
    story_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<StorySummary, AppError> {
    use crate::agents::{
        commands::ExecuteAgentRequest,
        service::{AgentService, AgentTask, AgentType},
    };

    log::info!(
        "[story_commands] {} called: story_id={}",
        "distill_story_knowledge",
        story_id
    );
    let kg_repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let entities = kg_repo.get_entities_by_story(&story_id).map_err(|e| {
        log::error!(
            "[story_commands] {} entity query failed: {}",
            "distill_story_knowledge",
            e
        );
        AppError::from(e)
    })?;
    let relations = kg_repo.get_relations_by_story(&story_id).map_err(|e| {
        log::error!(
            "[story_commands] {} relation query failed: {}",
            "distill_story_knowledge",
            e
        );
        AppError::from(e)
    })?;

    use std::collections::HashMap;
    let entity_names: HashMap<&str, &str> = entities
        .iter()
        .map(|e| (e.id.as_str(), e.name.as_str()))
        .collect();

    let kg_input = serde_json::json!({
        "entities": entities.iter().map(|e| {
            serde_json::json!({
                "name": e.name,
                "type": e.entity_type,
                "attributes": e.attributes,
            })
        }).collect::<Vec<_>>(),
        "relations": relations.iter().map(|r| {
            serde_json::json!({
                "source": entity_names.get(r.source_id.as_str()).unwrap_or(&r.source_id.as_str()),
                "target": entity_names.get(r.target_id.as_str()).unwrap_or(&r.target_id.as_str()),
                "type": r.relation_type,
                "strength": r.strength,
            })
        }).collect::<Vec<_>>(),
    });

    let request = ExecuteAgentRequest {
        agent_type: AgentType::KnowledgeDistiller,
        story_id: story_id.clone(),
        chapter_number: None,
        input: kg_input.to_string(),
        parameters: None,
    };

    let context = crate::agents::commands::build_agent_context(&app_handle, &request).await?;
    let task = AgentTask {
        id: uuid::Uuid::new_v4().to_string(),
        agent_type: AgentType::KnowledgeDistiller,
        context,
        input: kg_input.to_string(),
        parameters: std::collections::HashMap::new(),
        tier: None,
    };

    let service = AgentService::new(app_handle);
    let result = service.execute_task(task).await.map_err(|e| {
        log::error!(
            "[story_commands] {} LLM task failed: {}",
            "distill_story_knowledge",
            e
        );
        e
    })?;
    log::info!(
        "[story_commands] {} LLM task completed",
        "distill_story_knowledge"
    );

    let summary_repo = StorySummaryRepository::new(pool.inner().clone());
    // 如果已存在同类型摘要，则更新；否则创建
    let summary = match summary_repo.get_summary_by_type(&story_id, "knowledge_distillation") {
        Ok(Some(existing)) => {
            summary_repo
                .update_summary(&existing.id, &result.content)
                .map_err(AppError::from)?;
            StorySummary {
                content: result.content,
                updated_at: chrono::Local::now(),
                ..existing
            }
        }
        _ => summary_repo
            .create_summary(&story_id, "knowledge_distillation", &result.content)
            .map_err(AppError::from)?,
    };

    Ok(summary)
}

#[command(rename_all = "snake_case")]
pub async fn get_story_summaries(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<StorySummary>, AppError> {
    let repo = StorySummaryRepository::new(pool.inner().clone());
    repo.get_summaries_by_story(&story_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_story_summary(
    summary_id: String,
    content: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = StorySummaryRepository::new(pool.inner().clone());
    let result = repo
        .update_summary(&summary_id, &content)
        .map_err(AppError::from)?;
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM story_summaries WHERE id = ?1",
        [&summary_id],
        |row| row.get(0),
    );
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "storySummaries",
        );
    }
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn delete_story_summary(
    summary_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = StorySummaryRepository::new(pool.inner().clone());
    // 先查询 story_id 用于同步事件（删除后无法获取）
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM story_summaries WHERE id = ?1",
        [&summary_id],
        |row| row.get(0),
    );
    let result = repo.delete_summary(&summary_id).map_err(AppError::from)?;
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "storySummaries",
        );
    }
    Ok(result)
}

#[derive(Debug, serde::Serialize)]
pub struct StoryGraph {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
}

#[command(rename_all = "snake_case")]
pub async fn get_story_graph(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<StoryGraph, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let entities = repo
        .get_entities_by_story(&story_id)
        .map_err(AppError::from)?;
    let relations = repo
        .get_relations_by_story(&story_id)
        .map_err(AppError::from)?;
    Ok(StoryGraph {
        entities,
        relations,
    })
}

#[command(rename_all = "snake_case")]
pub async fn get_retention_report(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<crate::memory::retention::RetentionReport, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let entities = repo
        .get_entities_by_story(&story_id)
        .map_err(AppError::from)?;

    let manager = RetentionManager::new();
    Ok(manager.generate_retention_report(&entities))
}

#[command(rename_all = "snake_case")]
pub async fn archive_forgotten_entities(
    story_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::memory::retention::ArchiveResult, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let entities = repo
        .get_entities_by_story(&story_id)
        .map_err(AppError::from)?;

    let manager = RetentionManager::new();
    let forgotten = manager.get_forgotten_entities(&entities);

    let mut archived = Vec::new();
    for (entity, _) in &forgotten {
        repo.archive_entity(&entity.id).map_err(AppError::from)?;
        archived.push(entity.name.clone());
    }

    let result = crate::memory::retention::ArchiveResult {
        archived_count: archived.len(),
        archived_entities: archived,
        story_id: story_id.clone(),
    };
    let _ = crate::state_sync::StateSync::emit_data_refresh(
        &app_handle,
        Some(&story_id),
        "knowledgeGraph",
    );
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn restore_archived_entity(
    entity_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<Entity, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    repo.restore_entity(&entity_id).map_err(AppError::from)?;

    let entity = repo
        .get_entity_by_id(&entity_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("Entity", &entity_id))?;
    let _ = crate::state_sync::StateSync::emit_data_refresh(
        &app_handle,
        Some(&entity.story_id),
        "knowledgeGraph",
    );
    Ok(entity)
}

#[command(rename_all = "snake_case")]
pub async fn get_archived_entities(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<Entity>, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    repo.get_archived_entities(&story_id)
        .map_err(AppError::from)
}

// ==================== 小说创建向导命令 ====================

#[command(rename_all = "snake_case")]
pub fn get_character_by_name(
    story_id: String,
    name: String,
    pool: State<'_, DbPool>,
) -> Result<Option<CharacterQuickView>, AppError> {
    let conn = pool.get().map_err(AppError::from)?;

    // 1. Find character by name in story
    let character: Option<(String, String, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT id, name, appearance, personality FROM characters WHERE story_id = ?1 AND \
             name = ?2",
            [&story_id, &name],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .ok();

    let (char_id, char_name, appearance, personality) = match character {
        Some(c) => c,
        None => return Ok(None),
    };

    // 2. Truncate appearance to 60 chars
    let appearance_text = appearance.unwrap_or_default();
    let has_more = appearance_text.chars().count() > 60;
    let appearance_summary =
        appearance_text.chars().take(60).collect::<String>() + if has_more { "..." } else { "" };

    // 3. Build status tags from personality keywords
    let mut status_tags = Vec::new();
    if let Some(ref p) = personality {
        let keywords = [
            "冷静", "冲动", "善良", "邪恶", "勇敢", "懦弱", "聪明", "愚蠢", "忠诚", "背叛", "温柔",
            "残暴", "乐观", "悲观",
        ];
        for kw in &keywords {
            if p.contains(kw) {
                status_tags.push(kw.to_string());
            }
        }
    }

    // 4. Find last seen scene (using sequence_number as proxy for chapter)
    let last_seen_chapter: i32 = conn
        .query_row(
            "SELECT MAX(s.sequence_number) FROM scene_characters sc
         JOIN scenes s ON sc.scene_id = s.id
         WHERE sc.character_id = ?1",
            [&char_id],
            |row| row.get(0),
        )
        .unwrap_or(1);

    Ok(Some(CharacterQuickView {
        id: char_id,
        name: char_name,
        appearance_summary,
        status_tags,
        last_seen_chapter: last_seen_chapter.max(1),
    }))
}

// ==================== Story System: Projection Health Check
// ====================

#[command(rename_all = "snake_case")]
pub fn check_projection_health(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
) -> Result<crate::story_system::ProjectionHealthReport, AppError> {
    let engine = crate::story_system::StorySystemEngine::new(pool.inner().clone());
    engine
        .check_projection_health(&story_id, chapter_number)
        .map_err(AppError::internal)
}

// ==================== Analytics: Writing Statistics ====================

#[command(rename_all = "snake_case")]
pub fn get_writing_analytics(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<crate::analytics::WritingAnalytics, AppError> {
    let repo = SceneRepository::new(pool.inner().clone());
    let scenes = repo.get_by_story(&story_id).map_err(AppError::from)?;
    let engine = crate::analytics::AnalyticsEngine::new();
    Ok(engine.analyze_writing_data(&story_id, &scenes))
}

// ==================== 角色状态命令 ====================
