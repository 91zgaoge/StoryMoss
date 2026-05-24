//! Tauri 命令

use crate::db::{
    // Pool
    DbPool,
    // Models
    Scene, Story, Character, Chapter, WorldBuilding, WritingStyle, StudioConfig,
    SceneUpdate, WritingStyleUpdate, WorldRule, Culture,
    SceneVersion, CreatorType, SceneAnnotation, TextAnnotation,
    ChangeTrack, ChangeType, ChangeStatus, CommentThread, CommentMessage,
    CommentThreadWithMessages, AnchorType, ThreadStatus, ConflictType, CharacterConflict,
    CreateStoryRequest, CreateCharacterRequest, CreateChapterRequest, UpdateStoryRequest,
    LlmStudioConfig, UiStudioConfig, AgentBotConfig, Entity, Relation, StorySummary,
    CharacterState, StudioExportRequest,
    // Repositories
    SceneRepository, StoryRepository, CharacterRepository, ChapterRepository,
    WorldBuildingRepository, WritingStyleRepository, StudioConfigRepository,
    KnowledgeGraphRepository, SceneAnnotationRepository, TextAnnotationRepository,
    StorySummaryRepository, SceneVersionRepository, ChangeTrackRepository,
    CommentThreadRepository, StyleDnaRepository, StoryStyleConfigRepository,
    StoryOutlineRepository, CharacterRelationshipRepository,
    // Traits
    SceneRepo, StoryRepo, CharacterRepo, ChapterRepo, WorldBuildingRepo, WritingStyleRepo,
};
use crate::error::AppError;
use crate::config::StudioManager;
use crate::memory::retention::RetentionManager;
use crate::memory::ingest::{IngestPipeline, IngestContent};
use crate::agents::novel_creation::{NovelCreationAgent, WorldBuildingOption, CharacterProfileOption, WritingStyleOption, SceneProposal, GenerationOptions};
use crate::llm::LlmService;
use serde::{Serialize, Deserialize};
use tauri::{command, AppHandle, Manager, State};


// ==================== 场景命令 ====================

#[command(rename_all = "snake_case")]
pub async fn create_scene(
    story_id: String,
    sequence_number: i32,
    title: Option<String>,
    dramatic_goal: Option<String>,
    external_pressure: Option<String>,
    conflict_type: Option<String>,
    characters_present: Option<Vec<String>>,
    setting_location: Option<String>,
    setting_time: Option<String>,
    setting_atmosphere: Option<String>,
    content: Option<String>,
    confidence_score: Option<f32>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    automation_service: State<'_, crate::automation::service::AutomationService>,
) -> Result<Scene, AppError> {
    log::info!("[story_commands] {} called: story_id={}", "create_scene", story_id);
    let repo = SceneRepository::new(pool.inner().clone());
    let scene = repo.create(&story_id, sequence_number, title.as_deref())
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_scene", e);
            AppError::from(e)

        })?;
    
    // W2-F3: 提前记录 setting 字段是否变更（后续 move 后无法使用）
    let has_setting_changes = setting_location.is_some()
        || setting_time.is_some()
        || setting_atmosphere.is_some();

    // 如果提供了额外字段，立即更新场景
    let has_extra = dramatic_goal.is_some()
        || external_pressure.is_some()
        || conflict_type.is_some()
        || characters_present.is_some()
        || has_setting_changes
        || content.is_some()
        || confidence_score.is_some();
    if has_extra {
        let _ = repo.update(
            &scene.id,
            &SceneUpdate {
                title: None,
                content,
                dramatic_goal: dramatic_goal.clone(),
                external_pressure: external_pressure.clone(),
                conflict_type: conflict_type.and_then(|c| c.parse().ok()),
                characters_present,
                character_conflicts: None,
                setting_location,
                setting_time,
                setting_atmosphere,
                previous_scene_id: None,
                next_scene_id: None,
                confidence_score,
                ..Default::default()
            },
        );
        // P1-9 修复: 额外字段更新后发射 scene_updated，确保前端缓存刷新
        let _ = crate::state_sync::StateSync::emit_scene_updated(
            &app_handle, &story_id, &scene.id, scene.title.as_deref(),
        );
        // W2-F3: setting 字段变更同步触发 world_building 更新
        if has_setting_changes {
            let _ = crate::state_sync::StateSync::emit_world_building_updated(&app_handle, &story_id);
        }
    }

    // OnSceneCreate hook
    if let Some(manager) = crate::SKILL_MANAGER.get() {
        if let Ok(skill_manager) = manager.lock() {
            let story_id = scene.story_id.clone();
            let scene_id = scene.id.clone();
            let scene_title = scene.title.clone();
            let skill_manager = skill_manager.clone();
            tauri::async_runtime::spawn(async move {
                let context = crate::agents::AgentContext::minimal(story_id, String::new());
                let data = serde_json::json!({ "scene_id": scene_id, "scene_title": scene_title });
                let _ = skill_manager.execute_hooks(crate::skills::HookEvent::OnSceneCreate, &context, data).await;
                log::info!("Hook executed: {:?}", crate::skills::HookEvent::OnSceneCreate);
            });
        }
    }

    let _ = crate::state_sync::StateSync::emit_scene_created(&app_handle, &story_id, &scene.id, scene.title.as_deref());
    let _ = automation_service.trigger_event(
        crate::automation::triggers::TriggerEvent::SceneCreated {
            story_id: story_id.clone(),
            scene_id: scene.id.clone(),
        }
    ).await;

    Ok(scene)
}

/// 业务逻辑层：获取故事的所有场景（可被 mock 测试）
pub fn get_story_scenes_core(repo: &dyn crate::db::traits::SceneRepo, story_id: &str) -> Result<Vec<Scene>, AppError> {
    repo.get_by_story(story_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_story_scenes(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<Scene>, AppError> {
    let repo = SceneRepository::new(pool.inner().clone());
    get_story_scenes_core(&repo, &story_id)
}

#[command(rename_all = "snake_case")]
pub async fn get_scene(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<Scene>, AppError> {
    let repo = SceneRepository::new(pool.inner().clone());
    repo.get_by_id(&scene_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_scene(
    scene_id: String,
    updates: SceneUpdate,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    automation_service: State<'_, crate::automation::service::AutomationService>,
) -> Result<usize, AppError> {
    log::info!("[story_commands] {} called: scene_id={}", "update_scene", scene_id);
    let repo = SceneRepository::new(pool.inner().clone());
    // 获取 story_id 用于同步事件（P0-3 修复: 避免 unwrap_or_default 导致空字符串）
    let story_id_opt = repo.get_by_id(&scene_id).ok().flatten().map(|s| s.story_id);
    let result = repo.update(&scene_id, &updates)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "update_scene", e);
            AppError::from(e)

        })?;

    // 自动 Ingest：当场景内容或关键元数据被更新时，后台分析并更新知识图谱
    let should_ingest = updates.content.is_some()
        || updates.title.is_some()
        || updates.dramatic_goal.is_some()
        || updates.external_pressure.is_some()
        || updates.conflict_type.is_some()
        || updates.outline_content.is_some()
        || updates.draft_content.is_some()
        || updates.setting_location.is_some()
        || updates.setting_time.is_some()
        || updates.setting_atmosphere.is_some();
    if should_ingest {
        let pool_clone = pool.inner().clone();
        let scene_id_clone = scene_id.clone();
        let app_handle_clone = app_handle.clone();

        tauri::async_runtime::spawn(async move {
            // 获取场景信息以确定 story_id
            let scene_repo = SceneRepository::new(pool_clone.clone());
            if let Ok(Some(scene)) = scene_repo.get_by_id(&scene_id_clone) {
                let story_id = scene.story_id;
                let content = scene.content.unwrap_or_default();
                if content.len() > 50 {
                    let content_for_vector = content.clone();
                    let app_handle_for_sync = app_handle_clone.clone();
                    let llm_service = LlmService::new(app_handle_clone.clone());
                    let ingest_result = {
                        let pipeline = IngestPipeline::new(llm_service)
                            .with_pool(pool_clone.clone())
                            .with_app_handle(app_handle_clone.clone());
                        let ingest_content = IngestContent {
                            text: content,
                            source: format!("scene:{}", scene_id_clone),
                            story_id: story_id.clone(),
                            scene_id: Some(scene_id_clone.clone()),
                        };
                        match pipeline.ingest(&ingest_content).await {
                            Ok(result) => Some(result),
                            Err(e) => {
                                log::warn!("[AutoIngest] Scene {}: ingest failed: {}", scene_id_clone, e);
                                None
                            }
                        }
                    };

                    if let Some(ingest_result) = ingest_result {
                        let kg_repo = KnowledgeGraphRepository::new(pool_clone.clone());
                        // 批量保存实体（保留 Ingest Pipeline 分配的 ID，冲突时更新）
                        let saved_entities = kg_repo.save_entities_batch(&ingest_result.entities).unwrap_or(0);
                        let saved_relations = kg_repo.save_relations_batch(&ingest_result.relations).unwrap_or(0);

                        // D1 Phase 4: 提取实体引用索引（entity_mentions）
                        let mention_repo = crate::creative_engine::cascade_rewriter::EntityMentionRepository::new(pool_clone.clone());
                        let _ = mention_repo.delete_by_scene(&scene_id_clone);
                        let content_for_search = content_for_vector.clone();
                        let now = chrono::Utc::now().to_rfc3339();
                        for entity in &ingest_result.entities {
                            let entity_name = &entity.name;
                            let mut start = 0usize;
                            while let Some(pos) = content_for_search[start..].find(entity_name) {
                                let absolute_pos = start + pos;
                                let end_pos = absolute_pos + entity_name.len();
                                let mention = crate::creative_engine::cascade_rewriter::models::EntityMention {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    story_id: story_id.clone(),
                                    scene_id: scene_id_clone.clone(),
                                    entity_id: entity.id.clone(),
                                    entity_type: entity.entity_type.to_string(),
                                    start_pos: absolute_pos as i32,
                                    end_pos: end_pos as i32,
                                    mention_text: entity_name.clone(),
                                    confidence: 1.0,
                                    created_at: now.clone(),
                                    updated_at: now.clone(),
                                };
                                if let Err(e) = mention_repo.create(&mention) {
                                    log::warn!("[AutoIngest] Failed to create entity mention for {} in scene {}: {}", entity_name, scene_id_clone, e);
                                }
                                start = end_pos;
                            }
                        }

                        log::info!(
                            "[AutoIngest] Scene {}: {} entities, {} relations saved to KG",
                            scene_id_clone,
                            saved_entities,
                            saved_relations
                        );
                        let _ = crate::state_sync::StateSync::emit_ingestion_completed(
                            &app_handle_for_sync, &story_id, "scene"
                        );
                        let _ = crate::state_sync::StateSync::emit_data_refresh(
                            &app_handle_for_sync, Some(&story_id), "knowledgeGraph"
                        );
                        if let Some(store) = crate::VECTOR_STORE.get() {
                            match crate::embeddings::embed_text_async(content_for_vector.clone()).await {
                                Ok(embedding) => {
                                    let record = crate::vector::VectorRecord {
                                        id: format!("scene:{}", scene_id_clone),
                                        story_id: story_id.clone(),
                                        chapter_id: scene.chapter_id.clone().unwrap_or_default(),
                                        chapter_number: scene.sequence_number,
                                        text: content_for_vector,
                                        record_type: "scene".to_string(),
                                        embedding,
                                    };
                                    match store.add_record(record).await {
                                        Ok(_) => log::info!("[AutoIngest] Scene {} indexed to vector store", scene_id_clone),
                                        Err(e) => log::warn!("[AutoIngest] Failed to index scene {}: {}", scene_id_clone, e),
                                    }
                                }
                                Err(e) => {
                                    log::warn!("[AutoIngest] Failed to generate embedding for scene {}: {}", scene_id_clone, e);
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    if let Some(ref story_id) = story_id_opt {
        // W2-F3: setting 字段变更同步触发 world_building 更新
        if updates.setting_location.is_some() || updates.setting_time.is_some() || updates.setting_atmosphere.is_some() {
            let _ = crate::state_sync::StateSync::emit_world_building_updated(&app_handle, story_id);
        }
        let _ = crate::state_sync::StateSync::emit_scene_updated(&app_handle, story_id, &scene_id, updates.title.as_deref());
        let word_count = updates.content.as_ref().map(|c| c.split_whitespace().count()).unwrap_or(0);
        let _ = automation_service.trigger_event(
            crate::automation::triggers::TriggerEvent::SceneContentUpdated {
                story_id: story_id.clone(),
                scene_id: scene_id.clone(),
                word_count,
            }
        ).await;
    }
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn delete_scene(
    scene_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    log::info!("[story_commands] {} called: scene_id={}", "delete_scene", scene_id);
    let repo = SceneRepository::new(pool.inner().clone());
    let story_id = repo.get_by_id(&scene_id).ok().flatten().map(|s| s.story_id);
    let result = repo.delete(&scene_id)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "delete_scene", e);
            AppError::from(e)

        })?;
    if let Some(story_id) = story_id {
        // W2-F3: 场景删除后同步触发 world_building 更新（清理无引用规则）
        let _ = crate::state_sync::StateSync::emit_world_building_updated(&app_handle, &story_id);
        let _ = crate::state_sync::StateSync::emit_scene_deleted(&app_handle, &story_id, &scene_id);
    }
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn reorder_scenes(
    story_id: String,
    scene_ids: Vec<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let repo = SceneRepository::new(pool.inner().clone());
    
    for (index, scene_id) in scene_ids.iter().enumerate() {
        repo.update_sequence(scene_id, (index + 1) as i32)
            .map_err(AppError::from)?;
    }
    
    let _ = crate::state_sync::StateSync::emit_scene_updated(&app_handle, &story_id, &scene_ids.first().cloned().unwrap_or_default(), None);
    Ok(())
}

// ==================== 世界观命令 ====================

#[command(rename_all = "snake_case")]
pub async fn create_world_building(
    story_id: String,
    concept: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<WorldBuilding, AppError> {
    log::info!("[story_commands] {} called: story_id={}", "create_world_building", story_id);
    let repo = WorldBuildingRepository::new(pool.inner().clone());
    let wb = repo.create(&story_id, &concept)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_world_building", e);
            AppError::from(e)

        })?;
    let _ = crate::state_sync::StateSync::emit_world_building_created(&app_handle, &story_id, &wb.id);
    let _ = crate::state_sync::StateSync::emit_world_building_updated(&app_handle, &story_id);
    Ok(wb)
}

#[command(rename_all = "snake_case")]
pub async fn get_world_building(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<WorldBuilding>, AppError> {
    let repo = WorldBuildingRepository::new(pool.inner().clone());
    repo.get_by_story(&story_id)
        .map_err(AppError::from)
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
    log::info!("[story_commands] {} called: id={}", "update_world_building", id);
    let repo = WorldBuildingRepository::new(pool.inner().clone());
    let old_wb = repo.get_by_id(&id).ok().flatten();
    let result = repo.update(&id, concept.as_deref(), rules.as_deref(), history.as_deref(), cultures.as_deref())
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "update_world_building", e);
            AppError::from(e)

        })?;

    // OnWorldBuildingUpdate hook
    let story_id_for_sync = old_wb.as_ref().map(|wb| wb.story_id.clone())
        .or_else(|| pool.inner().get().ok().and_then(|c| {
            c.query_row("SELECT story_id FROM world_buildings WHERE id = ?", [&id], |row| {
                row.get::<_, String>(0)
            }).ok()
        }));
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
                    let context = crate::agents::AgentContext::minimal(story_id_for_hook, String::new());
                    let data = serde_json::json!({ "world_building_id": world_building_id });
                    let _ = skill_manager.execute_hooks(crate::skills::HookEvent::OnWorldBuildingUpdate, &context, data).await;
                    log::info!("Hook executed: {:?}", crate::skills::HookEvent::OnWorldBuildingUpdate);
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

                let payload = crate::creative_engine::cascade_rewriter::models::CascadeTaskPayload {
                    story_id: story_id.clone(),
                    change_events: vec![change_event],
                };

                let payload_json = serde_json::to_string(&payload).unwrap_or_default();

                let req = crate::task_system::models::CreateTaskRequest {
                    name: format!("级联改写: {}", old.concept),
                    description: Some(format!("因世界观 {} 的设定变更触发的场景级联改写", old.concept)),
                    task_type: "cascade_rewrite".to_string(),
                    schedule_type: "once".to_string(),
                    cron_pattern: None,
                    payload: Some(payload_json),
                    enabled: Some(true),
                    max_retries: Some(3),
                    heartbeat_timeout_seconds: Some(300),
                };

                if let Some(task_service) = app_handle.try_state::<crate::task_system::service::TaskService>() {
                    match task_service.create_task(req) {
                        Ok(task) => log::info!("[CascadeRewrite] Created task {} for world_building {}", task.id, id),
                        Err(e) => log::warn!("[CascadeRewrite] Failed to create task for world_building {}: {}", id, e),
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
    log::info!("[story_commands] {} called: id={}", "delete_world_building", id);
    let repo = WorldBuildingRepository::new(pool.inner().clone());
    // 先查询 story_id 用于同步事件（删除后无法获取）
    let story_id_opt = pool.inner().get().ok().and_then(|c| {
        c.query_row("SELECT story_id FROM world_buildings WHERE id = ?", [&id], |row| {
            row.get::<_, String>(0)
        }).ok()
    });
    let result = repo.delete(&id).map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "delete_world_building", e);
        e.to_string()
    })?;
    if let Some(ref story_id) = story_id_opt {
        let _ = crate::state_sync::StateSync::emit_world_building_deleted(&app_handle, story_id, &id);
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
) -> Result<WritingStyle, AppError> {
    let repo = WritingStyleRepository::new(pool.inner().clone());
    repo.create(&story_id, name.as_deref())
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_writing_style(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<WritingStyle>, AppError> {
    let repo = WritingStyleRepository::new(pool.inner().clone());
    repo.get_by_story(&story_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_writing_style(
    id: String,
    updates: WritingStyleUpdate,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = WritingStyleRepository::new(pool.inner().clone());
    let count = repo.update(&id, &updates)
        .map_err(AppError::from)?;
    
    // P2-15 修复: 查询 story_id 并发射同步事件
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM writing_styles WHERE id = ?1",
        [&id],
        |row| row.get(0)
    );
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "writingStyle");
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
    let app_dir = app_handle.path().app_data_dir()
        .map_err(AppError::from)?;
    let manager = StudioManager::new(pool.inner().clone(), &app_dir);
    manager.create_default_studio(&story_id, "")
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_studio_config(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<StudioConfig>, AppError> {
    let repo = StudioConfigRepository::new(pool.inner().clone());
    repo.get_by_story(&story_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_studio_config(
    id: String,
    pen_name: Option<String>,
    llm_config: Option<LlmStudioConfig>,
    ui_config: Option<UiStudioConfig>,
    agent_bots: Option<Vec<AgentBotConfig>>,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = StudioConfigRepository::new(pool.inner().clone());
    repo.update(&id, pen_name.as_deref(), llm_config.as_ref(), ui_config.as_ref(), agent_bots.as_deref())
        .map_err(AppError::from)
}

// ==================== 导入/导出命令 ====================

#[command(rename_all = "snake_case")]
pub async fn export_studio(
    request: StudioExportRequest,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<Vec<u8>, AppError> {
    let app_dir = app_handle.path().app_data_dir()
        .map_err(AppError::from)?;
    let manager = StudioManager::new(pool.inner().clone(), &app_dir);
    manager.export_studio(&request)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn import_studio(
    data: Vec<u8>,
    options: crate::config::studio_manager::ImportOptions,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<Story, AppError> {
    let app_dir = app_handle.path().app_data_dir()
        .map_err(AppError::from)?;
    let manager = StudioManager::new(pool.inner().clone(), &app_dir);
    manager.import_studio(&data, &options)
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
    log::info!("[story_commands] {} called: story_id={}, name={}, entity_type={}", "create_entity", story_id, name, entity_type);
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    // EVENT_REQUIRED
    let result = repo.create_entity(&story_id, &name, &entity_type, &attributes, None)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_entity", e);
            AppError::from(e)

        })?;
    let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "knowledgeGraph");
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
    use crate::embeddings::{embed_entity_async, EntityEmbeddingRequest};
    use std::collections::HashMap;

    log::info!("[story_commands] {} called: entity_id={}", "update_entity", entity_id);
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let existing = repo.get_entity_by_id(&entity_id)
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
            serde_json::Value::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            _ => HashMap::new(),
        };
        let request = EntityEmbeddingRequest {
            entity_id: entity_id.clone(),
            name: new_name.to_string(),
            description: new_attrs.get("description").and_then(|v| v.as_str()).map(|s| s.to_string()),
            entity_type: existing.entity_type.to_string(),
            attributes: attrs_map,
        };
        embed_entity_async(request).await.ok()
    } else {
        existing.embedding
    };

    // EVENT_REQUIRED
    let result = repo.update_entity(&entity_id, Some(new_name), Some(new_attrs), embedding)
        .map_err(AppError::from)?;

    let story_id_for_sync = pool.inner().get().ok().and_then(|c| {
        c.query_row("SELECT story_id FROM entities WHERE id = ?", [&entity_id], |row| {
            row.get::<_, String>(0)
        }).ok()
    });
    if let Some(ref story_id) = story_id_for_sync {
        let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(story_id), "knowledgeGraph");
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
    let result = repo.create_relation(&story_id, &source_id, &target_id, &relation_type, strength)
        .map_err(AppError::from)?;
    let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "knowledgeGraph");
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
pub async fn create_scene_annotation(
    scene_id: String,
    story_id: String,
    content: String,
    annotation_type: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<SceneAnnotation, AppError> {
    log::info!("[story_commands] {} called: scene_id={}", "create_scene_annotation", scene_id);
    let repo = SceneAnnotationRepository::new(pool.inner().clone());
    let annotation = repo.create_annotation(&scene_id, &story_id, &content, &annotation_type)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_scene_annotation", e);
            AppError::from(e)
        })?;
    let _ = crate::state_sync::StateSync::emit_annotation_created(&app_handle, &story_id, &annotation.id, &scene_id);
    Ok(annotation)
}

#[command(rename_all = "snake_case")]
pub async fn get_scene_annotations(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<SceneAnnotation>, AppError> {
    let repo = SceneAnnotationRepository::new(pool.inner().clone());
    repo.get_annotations_by_scene(&scene_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_story_unresolved_annotations(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<SceneAnnotation>, AppError> {
    let repo = SceneAnnotationRepository::new(pool.inner().clone());
    repo.get_unresolved_annotations_by_story(&story_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_scene_annotation(
    annotation_id: String,
    content: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = SceneAnnotationRepository::new(pool.inner().clone());
    repo.update_annotation(&annotation_id, &content)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn resolve_scene_annotation(
    annotation_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = SceneAnnotationRepository::new(pool.inner().clone());
    // 先查询 story_id 和 scene_id 用于同步事件
    let meta_opt = pool.inner().get().ok().and_then(|c| {
        c.query_row(
            "SELECT story_id, scene_id FROM scene_annotations WHERE id = ?",
            [&annotation_id],
            |row| {
                let story_id: String = row.get(0)?;
                let scene_id: String = row.get(1)?;
                Ok((story_id, scene_id))
            },
        )
        .ok()
    });
    let result = repo.resolve_annotation(&annotation_id).map_err(AppError::from)?;
    if let Some((story_id, scene_id)) = meta_opt {
        let _ = crate::state_sync::StateSync::emit_annotation_resolved(
            &app_handle, &story_id, &annotation_id, &scene_id);
    }
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn unresolve_scene_annotation(
    annotation_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = SceneAnnotationRepository::new(pool.inner().clone());
    repo.unresolve_annotation(&annotation_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn delete_scene_annotation(
    annotation_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = SceneAnnotationRepository::new(pool.inner().clone());
    repo.delete_annotation(&annotation_id)
        .map_err(AppError::from)
}

// ==================== 文本内联批注命令 ====================

#[command(rename_all = "snake_case")]
pub async fn create_text_annotation(
    story_id: String,
    scene_id: Option<String>,
    chapter_id: Option<String>,
    content: String,
    annotation_type: String,
    from_pos: i32,
    to_pos: i32,
    pool: State<'_, DbPool>,
) -> Result<TextAnnotation, AppError> {
    log::info!("[story_commands] {} called: story_id={}, scene_id={:?}, chapter_id={:?}", "create_text_annotation", story_id, scene_id, chapter_id);
    let repo = TextAnnotationRepository::new(pool.inner().clone());
    repo.create_annotation(&story_id, scene_id.as_deref(), chapter_id.as_deref(), &content, &annotation_type, from_pos, to_pos)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_text_annotation", e);
            AppError::from(e)

        })
}

#[command(rename_all = "snake_case")]
pub async fn get_text_annotations_by_chapter(
    chapter_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<TextAnnotation>, AppError> {
    let repo = TextAnnotationRepository::new(pool.inner().clone());
    repo.get_annotations_by_chapter(&chapter_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_text_annotations_by_scene(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<TextAnnotation>, AppError> {
    let repo = TextAnnotationRepository::new(pool.inner().clone());
    repo.get_annotations_by_scene(&scene_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_text_annotation(
    annotation_id: String,
    content: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = TextAnnotationRepository::new(pool.inner().clone());
    repo.update_annotation(&annotation_id, &content)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn resolve_text_annotation(
    annotation_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = TextAnnotationRepository::new(pool.inner().clone());
    repo.resolve_annotation(&annotation_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn unresolve_text_annotation(
    annotation_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = TextAnnotationRepository::new(pool.inner().clone());
    repo.unresolve_annotation(&annotation_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn delete_text_annotation(
    annotation_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = TextAnnotationRepository::new(pool.inner().clone());
    repo.delete_annotation(&annotation_id)
        .map_err(AppError::from)
}

// ==================== 古典评点家命令 ====================

#[command(rename_all = "snake_case")]
pub async fn generate_paragraph_commentaries(
    story_id: String,
    story_title: String,
    genre: String,
    text: String,
    app_handle: AppHandle,
) -> Result<String, AppError> {
    use crate::agents::AgentContext;
    use crate::agents::commentator::CommentatorAgent;

    log::info!("[story_commands] {} called: story_id={}", "generate_paragraph_commentaries", story_id);
    let pool = app_handle.state::<crate::db::DbPool>();
    let builder = crate::creative_engine::StoryContextBuilder::new(pool.inner().clone());
    let mut context = match builder.build_quick(&story_id) {
        Ok(ctx) => ctx,
        Err(e) => {
            log::warn!("[story_commands] StoryContextBuilder failed: {}, falling back to minimal", e);
            AgentContext::minimal(story_id.clone(), String::new())
        }
    };
    // 覆盖从数据库读取的标题/题材（调用方可能传入覆盖值）
    context.story_title = story_title;
    context.genre = genre;

    let llm_service = LlmService::new(app_handle);
    let agent = CommentatorAgent::new(llm_service);
    let commentaries = agent.comment_on_text(&context, &text).await
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "generate_paragraph_commentaries", e);
            AppError::from(e)

        })?;

    serde_json::to_string(&commentaries).map_err(|e| {
        log::error!("[story_commands] {} serialization failed: {}", "generate_paragraph_commentaries", e);
        AppError::from(e)
    })
}

// ==================== 记忆压缩命令 ====================

#[command(rename_all = "snake_case")]
pub async fn compress_content(
    story_id: String,
    content: String,
    target_ratio: Option<f32>,
    app_handle: AppHandle,
) -> Result<crate::agents::AgentResult, AppError> {
    use crate::agents::service::{AgentService, AgentTask, AgentType};
    use crate::agents::commands::ExecuteAgentRequest;
    use std::collections::HashMap;

    log::info!("[story_commands] {} called: story_id={}", "compress_content", story_id);
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
    use crate::agents::service::{AgentService, AgentTask, AgentType};
    use crate::agents::commands::ExecuteAgentRequest;
    use std::collections::HashMap;

    log::info!("[story_commands] {} called: scene_id={}", "compress_scene", scene_id);
    let scene_repo = SceneRepository::new(pool.inner().clone());
    let scene = scene_repo.get_by_id(&scene_id)
        .map_err(|e| {
            log::error!("[story_commands] {} scene lookup failed: {}", "compress_scene", e);
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
    use crate::agents::service::{AgentService, AgentTask, AgentType};
    use crate::agents::commands::ExecuteAgentRequest;

    log::info!("[story_commands] {} called: story_id={}", "distill_story_knowledge", story_id);
    let kg_repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let entities = kg_repo.get_entities_by_story(&story_id)
        .map_err(|e| {
            log::error!("[story_commands] {} entity query failed: {}", "distill_story_knowledge", e);
            AppError::from(e)

        })?;
    let relations = kg_repo.get_relations_by_story(&story_id)
        .map_err(|e| {
            log::error!("[story_commands] {} relation query failed: {}", "distill_story_knowledge", e);
            AppError::from(e)

        })?;

    use std::collections::HashMap;
    let entity_names: HashMap<&str, &str> = entities.iter()
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
        log::error!("[story_commands] {} LLM task failed: {}", "distill_story_knowledge", e);
        e
    })?;
    log::info!("[story_commands] {} LLM task completed", "distill_story_knowledge");

    let summary_repo = StorySummaryRepository::new(pool.inner().clone());
    // 如果已存在同类型摘要，则更新；否则创建
    let summary = match summary_repo.get_summary_by_type(&story_id, "knowledge_distillation") {
        Ok(Some(existing)) => {
            summary_repo.update_summary(&existing.id, &result.content)
                .map_err(AppError::from)?;
            StorySummary {
                content: result.content,
                updated_at: chrono::Local::now(),
                ..existing
            }
        }
        _ => {
            summary_repo.create_summary(&story_id, "knowledge_distillation", &result.content)
                .map_err(AppError::from)?
        }
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
) -> Result<usize, AppError> {
    let repo = StorySummaryRepository::new(pool.inner().clone());
    repo.update_summary(&summary_id, &content)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn delete_story_summary(
    summary_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = StorySummaryRepository::new(pool.inner().clone());
    repo.delete_summary(&summary_id)
        .map_err(AppError::from)
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
    let entities = repo.get_entities_by_story(&story_id)
        .map_err(AppError::from)?;
    let relations = repo.get_relations_by_story(&story_id)
        .map_err(AppError::from)?;
    Ok(StoryGraph { entities, relations })
}

#[command(rename_all = "snake_case")]
pub async fn get_retention_report(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<crate::memory::retention::RetentionReport, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let entities = repo.get_entities_by_story(&story_id)
        .map_err(AppError::from)?;
    
    let manager = RetentionManager::new();
    Ok(manager.generate_retention_report(&entities))
}

#[command(rename_all = "snake_case")]
pub async fn archive_forgotten_entities(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<crate::memory::retention::ArchiveResult, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    let entities = repo.get_entities_by_story(&story_id)
        .map_err(AppError::from)?;
    
    let manager = RetentionManager::new();
    let forgotten = manager.get_forgotten_entities(&entities);
    
    let mut archived = Vec::new();
    for (entity, _) in &forgotten {
        repo.archive_entity(&entity.id)
            .map_err(AppError::from)?;
        archived.push(entity.name.clone());
    }
    
    Ok(crate::memory::retention::ArchiveResult {
        archived_count: archived.len(),
        archived_entities: archived,
        story_id,
    })
}

#[command(rename_all = "snake_case")]
pub async fn restore_archived_entity(
    entity_id: String,
    pool: State<'_, DbPool>,
) -> Result<Entity, AppError> {
    let repo = KnowledgeGraphRepository::new(pool.inner().clone());
    repo.restore_entity(&entity_id)
        .map_err(AppError::from)?;
    
    repo.get_entity_by_id(&entity_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("Entity", &entity_id))
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
pub async fn generate_world_building_options(
    user_input: String,
    app_handle: AppHandle,
) -> Result<Vec<WorldBuildingOption>, AppError> {
    let llm_service = LlmService::new(app_handle);
    let agent = NovelCreationAgent::new(llm_service);
    let options = GenerationOptions::default();
    
    agent.generate_world_building_options(&user_input, &options)
        .await
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn generate_character_profiles(
    world_building: WorldBuildingOption,
    app_handle: AppHandle,
) -> Result<Vec<Vec<CharacterProfileOption>>, AppError> {
    let llm_service = LlmService::new(app_handle);
    let agent = NovelCreationAgent::new(llm_service);
    let options = GenerationOptions::default();
    
    agent.generate_character_profiles(&world_building, &options)
        .await
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn generate_writing_styles(
    genre: String,
    world_building: WorldBuildingOption,
    app_handle: AppHandle,
) -> Result<Vec<WritingStyleOption>, AppError> {
    let llm_service = LlmService::new(app_handle);
    let agent = NovelCreationAgent::new(llm_service);
    let options = GenerationOptions::default();
    
    agent.generate_writing_styles(&genre, &world_building, &options)
        .await
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn generate_first_scene(
    world_building: WorldBuildingOption,
    characters: Vec<CharacterProfileOption>,
    writing_style: WritingStyleOption,
    app_handle: AppHandle,
) -> Result<SceneProposal, AppError> {
    let llm_service = LlmService::new(app_handle);
    let agent = NovelCreationAgent::new(llm_service);
    
    agent.generate_first_scene(&world_building, &characters, &writing_style)
        .await
        .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WizardCreationResult {
    pub story: Story,
    pub world_building: WorldBuilding,
    pub writing_style: WritingStyle,
    pub first_scene: Scene,
    pub characters: Vec<Character>,
    pub ingested_entities: usize,
    pub ingested_relations: usize,
}

/// 在事务中持久化故事核心要素（Story + WorldBuilding + Characters + WritingStyle + Scene）
/// 供 create_story_with_wizard 和 CreationWorkflowEngine 复用
fn persist_wizard_elements_in_tx(
    tx: &rusqlite::Transaction,
    pool: DbPool,
    title: &str,
    description: Option<&str>,
    genre: Option<&str>,
    world_building: &WorldBuildingOption,
    characters: &[CharacterProfileOption],
    writing_style: &WritingStyleOption,
    first_scene: &SceneProposal,
) -> Result<(Story, WorldBuilding, Vec<Character>, WritingStyle, Scene), AppError> {
    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo.create_in_tx(
        tx,
        CreateStoryRequest {
            title: title.to_string(),
            description: description.map(|s| s.to_string()),
            genre: genre.map(|s| s.to_string()),
            style_dna_id: None,
        },
    )?;
    let story_id = story.id.clone();

    let wb_repo = WorldBuildingRepository::new(pool.clone());
    let wb = wb_repo.create_in_tx(tx, &story_id, &world_building.concept)?;
    wb_repo.update_in_tx(
        tx,
        &wb.id,
        Some(&world_building.concept),
        Some(&world_building.rules),
        Some(&world_building.history),
        Some(&world_building.cultures),
    )?;

    let char_repo = CharacterRepository::new(pool.clone());
    let mut created_chars = Vec::new();
    for char_opt in characters {
        let background = format!("{}", char_opt.background);
        let char = char_repo.create_in_tx(
            tx,
            CreateCharacterRequest {
                story_id: story_id.clone(),
                name: char_opt.name.clone(),
                background: Some(background),
                personality: Some(char_opt.personality.clone()),
                goals: Some(char_opt.goals.clone()),
                appearance: None,
                gender: None,
                age: None,
            },
        )?;
        created_chars.push(char);
    }

    let ws_repo = WritingStyleRepository::new(pool.clone());
    let ws = ws_repo.create_in_tx(tx, &story_id, Some(&writing_style.name))?;
    let ws_update = WritingStyleUpdate {
        name: Some(writing_style.name.clone()),
        description: Some(writing_style.description.clone()),
        tone: Some(writing_style.tone.clone()),
        pacing: Some(writing_style.pacing.clone()),
        vocabulary_level: Some(writing_style.vocabulary_level.clone()),
        sentence_structure: Some(writing_style.sentence_structure.clone()),
        custom_rules: Some(vec![]),
    };
    ws_repo.update_in_tx(tx, &ws.id, &ws_update)?;

    let scene_repo = SceneRepository::new(pool.clone());
    let scene = scene_repo.create_in_tx(tx, &story_id, 1, Some(&first_scene.title))?;

    let conflict_type = first_scene.conflict_type.parse().ok();
    let char_ids: Vec<String> = created_chars.iter().map(|c| c.id.clone()).collect();
    let scene_update = SceneUpdate {
        title: Some(first_scene.title.clone()),
        dramatic_goal: Some(first_scene.dramatic_goal.clone()),
        external_pressure: Some(first_scene.external_pressure.clone()),
        conflict_type,
        characters_present: Some(char_ids),
        character_conflicts: Some(vec![]),
        content: Some(first_scene.content.clone()),
        setting_location: Some(first_scene.setting_location.clone()),
        setting_time: Some(first_scene.setting_time.clone()),
        setting_atmosphere: Some(first_scene.setting_atmosphere.clone()),
        previous_scene_id: None,
        next_scene_id: None,
        confidence_score: Some(0.8),
        ..Default::default()
    };
    scene_repo.update_in_tx(tx, &scene.id, &scene_update)?;

    Ok((story, wb, created_chars, ws, scene))
}

#[command(rename_all = "snake_case")]
pub async fn create_story_with_wizard(
    title: String,
    description: Option<String>,
    genre: Option<String>,
    world_building: WorldBuildingOption,
    characters: Vec<CharacterProfileOption>,
    writing_style: WritingStyleOption,
    first_scene: SceneProposal,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    automation_service: State<'_, crate::automation::service::AutomationService>,
) -> Result<WizardCreationResult, AppError> {
    log::info!("[story_commands] {} called: title={}", "create_story_with_wizard", title);
    let pool_ref = pool.inner().clone();

    // === 事务 1: 核心要素持久化 ===
    let (story, created_chars, scene) = {
        let mut conn = pool_ref.get().map_err(|e| {
            log::error!("[story_commands] Failed to get DB connection: {}", e);
            AppError::from(rusqlite::Error::InvalidParameterName(e.to_string()))
        })?;
        let tx = conn.transaction().map_err(AppError::from)?;

        let (story, _wb, created_chars, _ws, scene) = persist_wizard_elements_in_tx(
            &tx,
            pool_ref.clone(),
            &title,
            description.as_deref(),
            genre.as_deref(),
            &world_building,
            &characters,
            &writing_style,
            &first_scene,
        ).map_err(|e| {
            log::error!("[story_commands] {} element persistence failed: {}", "create_story_with_wizard", e);
            e
        })?;

        tx.commit().map_err(AppError::from)?;
        (story, created_chars, scene)
    };
    let story_id = story.id.clone();
    log::info!("[story_commands] {} steps 1-5 committed: story_id={}", "create_story_with_wizard", story_id);

    // === 步骤 6: 异步 Ingest ===
    let ingest_text = format!(
        "世界观：{}\n\n历史背景：{}\n\n角色设定：\n{}\n\n文字风格：{}\n\n首个场景：{}\n\n{}",
        world_building.concept,
        &world_building.history,
        characters.iter().map(|c| format!("- {}：{}，目标：{}", c.name, c.personality, c.goals)).collect::<Vec<_>>().join("\n"),
        writing_style.name,
        first_scene.title,
        first_scene.content
    );

    let llm_service = LlmService::new(app_handle.clone());
    let pipeline = IngestPipeline::new(llm_service)
        .with_pool(pool.inner().clone())
        .with_app_handle(app_handle.clone());
    let ingest_content = IngestContent {
        text: ingest_text,
        source: format!("novel_creation_wizard:{}", story_id),
        story_id: story_id.clone(),
        scene_id: Some(scene.id.clone()),
    };

    let ingest_result = pipeline.ingest(&ingest_content).await
        .map_err(|e| {
            log::error!("[story_commands] {} ingest failed: {}", "create_story_with_wizard", e);
            AppError::from(e)
        })?;
    log::info!("[story_commands] {} step 6 completed: {} entities, {} relations", "create_story_with_wizard", ingest_result.entities.len(), ingest_result.relations.len());

    // === 事务 2: 保存 KG 数据 ===
    let (saved_entities, saved_relations) = {
        let mut conn2 = pool_ref.get().map_err(|e| {
            AppError::from(rusqlite::Error::InvalidParameterName(e.to_string()))
        })?;
        let tx2 = conn2.transaction().map_err(AppError::from)?;
        let kg_repo = KnowledgeGraphRepository::new(pool_ref.clone());

        let mut saved_entities = 0usize;
        for entity in &ingest_result.entities {
            kg_repo.create_entity_in_tx(
                &tx2, &story_id, &entity.name, &entity.entity_type.to_string(), &entity.attributes, entity.embedding.clone())
                .map_err(|e| {
                    log::error!("[story_commands] {} KG entity save failed: {}", "create_story_with_wizard", e);
                    AppError::from(e)
                })?;
            saved_entities += 1;
        }

        let entity_name_to_id: std::collections::HashMap<String, String> = ingest_result.entities
            .iter()
            .map(|e| (e.name.clone(), e.id.clone()))
            .collect();

        let mut saved_relations = 0usize;
        for relation in &ingest_result.relations {
            if let (Some(source_id), Some(target_id)) = (entity_name_to_id.get(&relation.source_id), entity_name_to_id.get(&relation.target_id)) {
                kg_repo.create_relation_in_tx(&tx2, &story_id, source_id, target_id, &relation.relation_type.to_string(), relation.strength)
                    .map_err(|e| {
                        log::error!("[story_commands] {} KG relation save failed: {}", "create_story_with_wizard", e);
                        AppError::from(e)
                    })?;
                saved_relations += 1;
            }
        }
        tx2.commit().map_err(AppError::from)?;
        (saved_entities, saved_relations)
    };

    // 重新获取完整数据（因为 update 返回的是 usize）
    let wb_repo = WorldBuildingRepository::new(pool_ref.clone());
    let final_wb = wb_repo.get_by_story(&story_id)
        .map_err(|e| {
            log::error!("[story_commands] {} final WB query failed: {}", "create_story_with_wizard", e);
            AppError::from(e)
        })?
        .ok_or_else(|| AppError::from("World building not found".to_string()))?;

    let ws_repo = WritingStyleRepository::new(pool_ref.clone());
    let final_ws = ws_repo.get_by_story(&story_id)
        .map_err(|e| {
            log::error!("[story_commands] {} final WS query failed: {}", "create_story_with_wizard", e);
            AppError::from(e)
        })?
        .ok_or_else(|| AppError::from("Writing style not found".to_string()))?;

    let scene_repo = SceneRepository::new(pool_ref.clone());
    let final_scene = scene_repo.get_by_id(&scene.id)
        .map_err(|e| {
            log::error!("[story_commands] {} final scene query failed: {}", "create_story_with_wizard", e);
            AppError::from(e)
        })?
        .ok_or_else(|| AppError::from("Scene not found".to_string()))?;

    log::info!("[story_commands] {} completed successfully", "create_story_with_wizard");

    // P0-2 修复: 发射同步事件，确保幕后界面自动刷新新创建的内容
    let _ = crate::state_sync::StateSync::emit_story_created(&app_handle, &story_id, &story.title);
    let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "all");

    // 触发自动化事件：故事创建完成
    if let Err(e) = automation_service.trigger_event(crate::automation::triggers::TriggerEvent::StoryCreated {
        story_id: story_id.clone()
    }).await {
        log::warn!("[story_commands] Failed to trigger story created automation: {}", e);
    }

    // 触发自动化事件：角色创建完成（为每个创建的角色）
    for character in &created_chars {
        if let Err(e) = automation_service.trigger_event(crate::automation::triggers::TriggerEvent::CharacterCreated {
            story_id: story_id.clone(),
            character_id: character.id.clone()
        }).await {
            log::warn!("[story_commands] Failed to trigger character created automation for {}: {}", character.id, e);
        }
    }

    Ok(WizardCreationResult {
        story,
        world_building: final_wb,
        writing_style: final_ws,
        first_scene: final_scene,
        characters: created_chars,
        ingested_entities: saved_entities,
        ingested_relations: saved_relations,
    })}


// ==================== 场景版本命令 ====================

use crate::versions::service::{SceneVersionService, VersionChainNode, VersionDiff, VersionStats};

#[command(rename_all = "snake_case")]
pub async fn get_scene_versions(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<SceneVersion>, AppError> {
    let repo = SceneVersionRepository::new(pool.inner().clone());
    repo.get_versions(&scene_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_scene_version(
    version_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<SceneVersion>, AppError> {
    let repo = SceneVersionRepository::new(pool.inner().clone());
    repo.get_version(&version_id)
        .map_err(AppError::from)
}

/// 为指定场景创建版本快照，并自动生成 ChangeTrack diff
fn create_version_snapshot(
    pool: &DbPool,
    scene_id: &str,
    change_summary: &str,
    created_by: &str,
) -> Result<Option<SceneVersion>, AppError> {

    let scene_repo = crate::db::repositories_v3::SceneRepository::new(pool.clone());
    let version_repo = SceneVersionRepository::new(pool.clone());
    let track_repo = ChangeTrackRepository::new(pool.clone());
    
    let scene = match scene_repo.get_by_id(scene_id) {
        Ok(Some(s)) => s,
        Ok(None) => return Ok(None),
        Err(e) => return Err(AppError::from(e)),
    };
    
    // 获取上一版本内容用于 diff
    let prev_content = version_repo.get_versions(scene_id)
        .map_err(AppError::from)?
        .into_iter()
        .next()
        .and_then(|v| v.content);
    
    let creator = match created_by {
        "user" => CreatorType::User,
        "ai" => CreatorType::Ai,
        _ => CreatorType::System,
    };
    
    let version = version_repo.create_version(&scene, change_summary, creator, None, None)
        .map_err(AppError::from)?;
    
    // 基于 diff 生成 ChangeTrack
    let current_content = scene.content.as_deref().unwrap_or("");
    if let Some(old) = prev_content {
        let tracks = diff_to_change_tracks(scene_id, created_by, &old, current_content);
        for mut track in tracks {
            track.version_id = Some(version.id.clone());
            let _ = track_repo.create(&track);
        }
    }
    
    Ok(Some(version))
}

#[command(rename_all = "snake_case")]
pub async fn create_scene_version(
    scene_id: String,
    change_summary: String,
    created_by: String,
    confidence_score: Option<f32>,
    pool: State<'_, DbPool>,
) -> Result<SceneVersion, AppError> {

    let scene_repo = crate::db::repositories_v3::SceneRepository::new(pool.inner().clone());
    let version_repo = SceneVersionRepository::new(pool.inner().clone());
    let track_repo = ChangeTrackRepository::new(pool.inner().clone());
    
    let scene = scene_repo.get_by_id(&scene_id)
        .map_err(AppError::from)?
        .ok_or("Scene not found")?;
    
    // 获取上一版本内容用于 diff
    let prev_content = version_repo.get_versions(&scene_id)
        .map_err(AppError::from)?
        .into_iter()
        .next()
        .and_then(|v| v.content);
    
    let creator = match created_by.as_str() {
        "user" => CreatorType::User,
        "ai" => CreatorType::Ai,
        _ => CreatorType::System,
    };
    
    let version = version_repo.create_version(&scene, &change_summary, creator, None, confidence_score)
        .map_err(AppError::from)?;
    
    // 基于 diff 生成 ChangeTrack
    let current_content = scene.content.as_deref().unwrap_or("");
    if let Some(old) = prev_content {
        let tracks = diff_to_change_tracks(&scene_id, &created_by, &old, current_content);
        for mut track in tracks {
            track.version_id = Some(version.id.clone());
            let _ = track_repo.create(&track);
        }
    }
    
    Ok(version)
}

/// 将两段文本的差异转换为 ChangeTrack 列表（简单字符级 diff）
fn diff_to_change_tracks(
    scene_id: &str,
    author_id: &str,
    old: &str,
    new: &str,
) -> Vec<crate::db::models_v3::ChangeTrack> {
    
    if old == new {
        return vec![];
    }
    
    // 找公共前缀
    let mut prefix = 0;
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    while prefix < old_chars.len() && prefix < new_chars.len() && old_chars[prefix] == new_chars[prefix] {
        prefix += 1;
    }
    
    // 找公共后缀
    let mut suffix = 0;
    while suffix < old_chars.len() - prefix && suffix < new_chars.len() - prefix
        && old_chars[old_chars.len() - 1 - suffix] == new_chars[new_chars.len() - 1 - suffix]
    {
        suffix += 1;
    }
    
    let old_mid_start = prefix;
    let old_mid_end = old_chars.len() - suffix;
    let new_mid_start = prefix;
    let new_mid_end = new_chars.len() - suffix;
    
    let mut tracks = Vec::new();
    
    // 删除的部分
    if old_mid_start < old_mid_end {
        let deleted: String = old_chars[old_mid_start..old_mid_end].iter().collect();
        tracks.push(ChangeTrack::new(
            Some(scene_id.to_string()),
            None,
            author_id.to_string(),
            ChangeType::Delete,
            old_mid_start as i32,
            old_mid_end as i32,
            Some(deleted),
        ));
    }
    
    // 插入的部分
    if new_mid_start < new_mid_end {
        let inserted: String = new_chars[new_mid_start..new_mid_end].iter().collect();
        tracks.push(ChangeTrack::new(
            Some(scene_id.to_string()),
            None,
            author_id.to_string(),
            ChangeType::Insert,
            new_mid_start as i32,
            new_mid_end as i32,
            Some(inserted),
        ));
    }
    
    tracks
}

#[command(rename_all = "snake_case")]
pub async fn compare_scene_versions(
    from_version_id: String,
    to_version_id: String,
    pool: State<'_, DbPool>,
) -> Result<VersionDiff, AppError> {
    let service = SceneVersionService::new(pool.inner().clone());
    service.compare_versions(&from_version_id, &to_version_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_scene_version_chain(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<VersionChainNode>, AppError> {
    let service = SceneVersionService::new(pool.inner().clone());
    service.get_version_chain(&scene_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_version_change_tracks(
    version_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::models_v3::ChangeTrack>, AppError> {
    let repo = crate::db::repositories_v3::ChangeTrackRepository::new(pool.inner().clone());
    repo.get_by_version(&version_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn restore_scene_version(
    scene_id: String,
    version_id: String,
    restored_by: String,
    pool: State<'_, DbPool>,
) -> Result<SceneVersion, AppError> {
    let service = SceneVersionService::new(pool.inner().clone());
    let result = service.restore_version(&scene_id, &version_id, &restored_by)
        .map_err(AppError::from)?;
    Ok(result.new_version)
}

#[command(rename_all = "snake_case")]
pub async fn get_scene_version_stats(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<VersionStats, AppError> {
    let service = SceneVersionService::new(pool.inner().clone());
    service.get_version_stats(&scene_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn delete_scene_version(
    version_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = SceneVersionRepository::new(pool.inner().clone());
    repo.delete_version(&version_id)
        .map_err(AppError::from)
}


// ==================== 变更追踪命令 (修订模式) ====================

#[command(rename_all = "snake_case")]
pub async fn track_change(
    scene_id: Option<String>,
    chapter_id: Option<String>,
    change_type: String,
    from_pos: i32,
    to_pos: i32,
    content: Option<String>,
    author_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<crate::db::models_v3::ChangeTrack, AppError> {

    log::info!("[story_commands] {} called: scene_id={:?}, chapter_id={:?}", "track_change", scene_id, chapter_id);
    let ct = match change_type.as_str() {
        "Delete" => ChangeType::Delete,
        "Format" => ChangeType::Format,
        _ => ChangeType::Insert,
    };

    let track = ChangeTrack::new(
        scene_id,
        chapter_id,
        author_id.unwrap_or_else(|| "user".to_string()),
        ct,
        from_pos,
        to_pos,
        content,
    );

    let repo = ChangeTrackRepository::new(pool.inner().clone());
    repo.create(&track)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "track_change", e);
            AppError::from(e)

        })
}

#[command(rename_all = "snake_case")]
pub async fn accept_change(
    change_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {

    log::info!("[story_commands] {} called: change_id={}", "accept_change", change_id);
    let repo = ChangeTrackRepository::new(pool.inner().clone());
    let result = repo.update_status(&change_id, ChangeStatus::Accepted)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "accept_change", e);
            AppError::from(e)

        })?;
    
    // 自动创建版本快照
    if let Ok(Some(track)) = repo.get_by_id(&change_id) {
        if let Some(scene_id) = track.scene_id {
            let _ = create_version_snapshot(pool.inner(), &scene_id, "接受变更", "system");
        }
    }
    
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn reject_change(
    change_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {

    log::info!("[story_commands] {} called: change_id={}", "reject_change", change_id);
    let repo = ChangeTrackRepository::new(pool.inner().clone());
    let result = repo.update_status(&change_id, ChangeStatus::Rejected)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "reject_change", e);
            AppError::from(e)

        })?;
    
    // 自动创建版本快照
    if let Ok(Some(track)) = repo.get_by_id(&change_id) {
        if let Some(scene_id) = track.scene_id {
            let _ = create_version_snapshot(pool.inner(), &scene_id, "拒绝变更", "system");
        }
    }
    
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn get_pending_changes(
    scene_id: Option<String>,
    chapter_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::models_v3::ChangeTrack>, AppError> {

    let repo = ChangeTrackRepository::new(pool.inner().clone());
    if let Some(sid) = scene_id {
        repo.get_pending_by_scene(&sid)
            .map_err(AppError::from)
    } else if let Some(cid) = chapter_id {
        repo.get_pending_by_chapter(&cid)
            .map_err(AppError::from)
    } else {
        Err(AppError::validation_failed("Either scene_id or chapter_id must be provided", None::<String>))
    }
}

#[command(rename_all = "snake_case")]
pub async fn accept_all_changes(
    scene_id: Option<String>,
    chapter_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {

    let repo = ChangeTrackRepository::new(pool.inner().clone());
    let result = if let Some(sid) = scene_id.clone() {
        repo.accept_all_by_scene(&sid)
            .map_err(AppError::from)?
    } else if let Some(cid) = chapter_id {
        repo.accept_all_by_chapter(&cid)
            .map_err(AppError::from)?
    } else {
        return Err(AppError::validation_failed("Either scene_id or chapter_id must be provided", None::<String>));
    };
    
    // 自动创建版本快照（仅场景级变更）
    if let Some(sid) = scene_id {
        let _ = create_version_snapshot(pool.inner(), &sid, "全部接受变更", "system");
    }
    
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn reject_all_changes(
    scene_id: Option<String>,
    chapter_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {

    let repo = ChangeTrackRepository::new(pool.inner().clone());
    let result = if let Some(sid) = scene_id.clone() {
        repo.reject_all_by_scene(&sid)
            .map_err(AppError::from)?
    } else if let Some(cid) = chapter_id {
        repo.reject_all_by_chapter(&cid)
            .map_err(AppError::from)?
    } else {
        return Err(AppError::validation_failed("Either scene_id or chapter_id must be provided", None::<String>));
    };
    
    // 自动创建版本快照（仅场景级变更）
    if let Some(sid) = scene_id {
        let _ = create_version_snapshot(pool.inner(), &sid, "全部拒绝变更", "system");
    }
    
    Ok(result)
}


// ==================== 评论线程命令 (修订模式) ====================

#[command(rename_all = "snake_case")]
pub async fn create_comment_thread(
    version_id: Option<String>,
    anchor_type: String,
    scene_id: Option<String>,
    chapter_id: Option<String>,
    from_pos: Option<i32>,
    to_pos: Option<i32>,
    selected_text: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<crate::db::models_v3::CommentThread, AppError> {

    log::info!("[story_commands] {} called: scene_id={:?}, chapter_id={:?}", "create_comment_thread", scene_id, chapter_id);
    let at = match anchor_type.as_str() {
        "SceneLevel" => AnchorType::SceneLevel,
        _ => AnchorType::TextRange,
    };

    let thread = CommentThread::new(
        version_id,
        at,
        scene_id,
        chapter_id,
        from_pos,
        to_pos,
        selected_text,
    );

    let repo = CommentThreadRepository::new(pool.inner().clone());
    repo.create_thread(&thread)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_comment_thread", e);
            AppError::from(e)

        })
}

#[command(rename_all = "snake_case")]
pub async fn add_comment_message(
    thread_id: String,
    content: String,
    author_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<crate::db::models_v3::CommentMessage, AppError> {
    use chrono::Local;
    use uuid::Uuid;

    log::info!("[story_commands] {} called: thread_id={}", "add_comment_message", thread_id);
    let message = CommentMessage {
        id: Uuid::new_v4().to_string(),
        thread_id,
        author_id: author_id.unwrap_or_else(|| "user".to_string()),
        author_name: None,
        content,
        created_at: Local::now(),
    };

    let repo = CommentThreadRepository::new(pool.inner().clone());
    repo.add_message(&message)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "add_comment_message", e);
            AppError::from(e)

        })
}

#[command(rename_all = "snake_case")]
pub async fn get_comment_threads(
    scene_id: Option<String>,
    chapter_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::models_v3::CommentThreadWithMessages>, AppError> {

    let repo = CommentThreadRepository::new(pool.inner().clone());
    if let Some(sid) = scene_id {
        repo.get_threads_by_scene(&sid)
            .map_err(AppError::from)
    } else if let Some(cid) = chapter_id {
        repo.get_threads_by_chapter(&cid)
            .map_err(AppError::from)
    } else {
        Err(AppError::validation_failed("Either scene_id or chapter_id must be provided", None::<String>))
    }
}

#[command(rename_all = "snake_case")]
pub async fn resolve_comment_thread(
    thread_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {

    let repo = CommentThreadRepository::new(pool.inner().clone());
    repo.resolve_thread(&thread_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn reopen_comment_thread(
    thread_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {

    let repo = CommentThreadRepository::new(pool.inner().clone());
    repo.reopen_thread(&thread_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn delete_comment_thread(
    thread_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {

    let repo = CommentThreadRepository::new(pool.inner().clone());
    repo.delete_thread(&thread_id)
        .map_err(AppError::from)
}


#[command(rename_all = "snake_case")]
pub async fn run_creation_workflow(
    story_id: String,
    mode: String, // "ai_only" | "ai_first" | "human_first"
    initial_input: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    use crate::creative_engine::workflow::{CreationWorkflowEngine, CreationMode};
    use crate::agents::service::AgentService;

    log::info!("[story_commands] {} called: story_id={}, mode={}", "run_creation_workflow", story_id, mode);
    let mode = match mode.as_str() {
        "ai_only" => CreationMode::AiOnly,
        "human_first" | "human_draft_ai_polish" => CreationMode::HumanDraftAiPolish,
        _ => CreationMode::AiDraftHumanEdit,
    };

    let agent_service = AgentService::new(app_handle.clone());
    let engine = CreationWorkflowEngine::new(agent_service, pool.inner().clone());
    let config = CreationWorkflowEngine::create_standard_workflow(&story_id, mode);

    match engine.execute_full_workflow(&config, &initial_input).await {
        Ok(result) => {
            log::info!("[story_commands] {} completed: success={}", "run_creation_workflow", result.success);
            // 查询刚创建的 Scene ID，供前端直接跳转
            let scene_repo = SceneRepository::new(pool.inner().clone());
            let scene_id = scene_repo.get_by_story(&story_id)
                .ok()
                .and_then(|scenes| scenes.into_iter().last())
                .map(|s| s.id);

            // AiOnly 模式：spawn 后台 enrich 任务，从正文生成完整 World/Character/Style
            if mode == CreationMode::AiOnly {
                let pool_clone = pool.inner().clone();
                let story_id_clone = story_id.clone();
                let app_handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    log::info!("[story_commands] Spawning background enrich for story_id={}", story_id_clone);
                    let agent_service = AgentService::new(app_handle_clone);
                    let engine = CreationWorkflowEngine::new(agent_service, pool_clone);
                    if let Err(e) = engine.enrich_story_elements(&story_id_clone).await {
                        log::warn!("[story_commands] Background enrich failed: {}", e);
                    }
                });
            }

            let json = serde_json::json!({
                "success": result.success,
                "current_phase": result.current_phase,
                "completed_phases": result.completed_phases,
                "output_preview": result.output.as_ref().map(|o| o.chars().take(500).collect::<String>()),
                "quality_report": result.quality_report,
                "scene_id": scene_id,
                "error": result.error,
            });
            Ok(json)
        }
        Err(e) => {
            log::error!("[story_commands] {} failed: {}", "run_creation_workflow", e);
            Err(e)
        }
    }
}

// ==================== StyleDNA 命令 ====================

#[tauri::command]
pub async fn list_style_dnas(pool: State<'_, DbPool>) -> Result<Vec<serde_json::Value>, AppError> {
    let repo = StyleDnaRepository::new(pool.inner().clone());
    match repo.get_all() {
        Ok(dnas) => {
            let result: Vec<serde_json::Value> = dnas.into_iter().map(|d| {
                serde_json::json!({
                    "id": d.id,
                    "name": d.name,
                    "author": d.author,
                    "is_builtin": d.is_builtin,
                    "is_user_created": d.is_user_created,
                })
            }).collect();
            Ok(result)
        }
        Err(e) => Err(AppError::from(e)),
    }
}

#[tauri::command]
pub async fn set_story_style_dna(
    story_id: String,
    style_dna_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let repo = StoryRepository::new(pool.inner().clone());
    let req = UpdateStoryRequest {
        title: None,
        description: None,
        genre: None,
        tone: None,
        pacing: None,
        style_dna_id,
        methodology_id: None,
        methodology_step: None,
    };
    match repo.update(&story_id, &req) {
        Ok(_) => Ok(()),
        Err(e) => Err(AppError::from(e)),
    }
}

/// 从文本样例使用 LLM 分析生成 StyleDNA
#[tauri::command]
pub async fn analyze_style_sample(
    text: String,
    name: Option<String>,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    use crate::creative_engine::style::StyleAnalyzer;
    use crate::llm::service::LlmService;

    let llm_service = LlmService::new(app_handle);
    let dna_name = name.unwrap_or_else(|| "自定义风格".to_string());

    let dna = StyleAnalyzer::analyze_with_llm(&text, &dna_name, &llm_service).await?;
    let dna_json = serde_json::to_string(&dna)
        .map_err(|e| format!("序列化 StyleDNA 失败: {}", e))?;

    let repo = StyleDnaRepository::new(pool.inner().clone());
    let record = repo.create(&dna_name, dna.meta.author.as_deref(), &dna_json, false)
        .map_err(|e| format!("保存 StyleDNA 失败: {}", e))?;

    Ok(serde_json::json!({
        "id": record.id,
        "name": record.name,
        "author": record.author,
        "is_builtin": record.is_builtin,
        "is_user_created": record.is_user_created,
    }))
}

// ==================== 伏笔追踪命令 ====================

#[command(rename_all = "snake_case")]
pub async fn get_story_foreshadowings(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<serde_json::Value>, AppError> {
    use crate::creative_engine::foreshadowing::ForeshadowingTracker;
    let tracker = ForeshadowingTracker::new(pool.inner().clone());
    let records = tracker.get_all(&story_id)
        .map_err(AppError::from)?;
    let result: Vec<serde_json::Value> = records.into_iter().map(|r| {
        serde_json::json!({
            "id": r.id,
            "story_id": r.story_id,
            "content": r.content,
            "setup_scene_id": r.setup_scene_id,
            "payoff_scene_id": r.payoff_scene_id,
            "status": r.status.to_string(),
            "importance": r.importance,
            "created_at": r.created_at,
            "resolved_at": r.resolved_at,
        })
    }).collect();
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn create_foreshadowing(
    story_id: String,
    content: String,
    setup_scene_id: Option<String>,
    importance: i32,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<String, AppError> {
    use crate::creative_engine::foreshadowing::ForeshadowingTracker;
    log::info!("[story_commands] {} called: story_id={}", "create_foreshadowing", story_id);
    let tracker = ForeshadowingTracker::new(pool.inner().clone());
    let result = tracker.add_foreshadowing(&story_id, &content, setup_scene_id.as_deref(), importance)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_foreshadowing", e);
            AppError::from(e)

        });
    if result.is_ok() {
        let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "foreshadowings");
    }
    result
}

#[command(rename_all = "snake_case")]
pub async fn update_foreshadowing_status(
    id: String,
    status: String,
    payoff_scene_id: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    use crate::creative_engine::foreshadowing::ForeshadowingTracker;
    let tracker = ForeshadowingTracker::new(pool.inner().clone());
    let result = match status.as_str() {
        "payoff" => tracker.mark_payoff(&id, payoff_scene_id.as_deref()),
        "abandoned" => tracker.abandon(&id),
        _ => Err(format!("无效状态: {}", status)),
    }.map_err(AppError::from);
    
    if result.is_ok() {
        // 查询 story_id 以发射同步事件
        let conn = pool.inner().get().map_err(AppError::from)?;
        let story_id: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT story_id FROM foreshadowing_tracker WHERE id = ?1",
            [&id],
            |row| row.get(0)
        );
        if let Ok(story_id) = story_id {
            let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "foreshadowings");
        }
    }
    result
}

// ==================== Payoff Ledger 命令 ====================

#[command(rename_all = "snake_case")]
pub async fn get_payoff_ledger(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<serde_json::Value>, AppError> {
    use crate::creative_engine::payoff_ledger::PayoffLedger;
    let ledger = PayoffLedger::new(pool.inner().clone());
    let items = ledger.get_ledger(&story_id)
        .map_err(AppError::from)?;

    let result: Vec<serde_json::Value> = items.into_iter().map(|item| {
        serde_json::json!({
            "id": item.id,
            "ledger_key": item.ledger_key,
            "title": item.title,
            "summary": item.summary,
            "scope_type": item.scope_type.to_string(),
            "current_status": item.current_status.to_string(),
            "target_start_scene": item.target_start_scene,
            "target_end_scene": item.target_end_scene,
            "first_seen_scene": item.first_seen_scene,
            "last_touched_scene": item.last_touched_scene,
            "confidence": item.confidence,
            "risk_signals": item.risk_signals,
            "importance": item.importance,
            "created_at": item.created_at,
            "resolved_at": item.resolved_at,
        })
    }).collect();

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn detect_overdue_payoffs(
    story_id: String,
    current_scene_number: i32,
    pool: State<'_, DbPool>,
) -> Result<Vec<serde_json::Value>, AppError> {
    use crate::creative_engine::payoff_ledger::PayoffLedger;
    let ledger = PayoffLedger::new(pool.inner().clone());
    let items = ledger.detect_overdue(&story_id, current_scene_number)
        .map_err(AppError::from)?;

    let result: Vec<serde_json::Value> = items.into_iter().map(|item| {
        serde_json::json!({
            "id": item.id,
            "ledger_key": item.ledger_key,
            "title": item.title,
            "summary": item.summary,
            "scope_type": item.scope_type.to_string(),
            "current_status": item.current_status.to_string(),
            "target_start_scene": item.target_start_scene,
            "target_end_scene": item.target_end_scene,
            "first_seen_scene": item.first_seen_scene,
            "last_touched_scene": item.last_touched_scene,
            "confidence": item.confidence,
            "risk_signals": item.risk_signals,
            "importance": item.importance,
            "created_at": item.created_at,
            "resolved_at": item.resolved_at,
        })
    }).collect();

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn recommend_payoff_timing(
    story_id: String,
    current_scene_number: i32,
    pool: State<'_, DbPool>,
) -> Result<Vec<serde_json::Value>, AppError> {
    use crate::creative_engine::payoff_ledger::PayoffLedger;
    let ledger = PayoffLedger::new(pool.inner().clone());
    let recs = ledger.recommend_payoff_timing(&story_id, current_scene_number)
        .map_err(AppError::from)?;

    let result: Vec<serde_json::Value> = recs.into_iter().map(|rec| {
        serde_json::json!({
            "foreshadowing_id": rec.foreshadowing_id,
            "ledger_key": rec.ledger_key,
            "title": rec.title,
            "recommended_scene": rec.recommended_scene,
            "urgency": rec.urgency.to_string(),
            "reason": rec.reason,
            "importance": rec.importance,
        })
    }).collect();

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn update_payoff_ledger_fields(
    foreshadowing_id: String,
    target_start_scene: Option<i32>,
    target_end_scene: Option<i32>,
    risk_signals: Option<Vec<String>>,
    scope_type: Option<String>,
    ledger_key: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    use crate::creative_engine::payoff_ledger::PayoffLedger;
    let ledger = PayoffLedger::new(pool.inner().clone());
    let scope = scope_type.as_deref().and_then(|s| s.parse().ok());
    let result = ledger.update_ledger_fields(
        &foreshadowing_id,
        target_start_scene,
        target_end_scene,
        risk_signals,
        scope,
        ledger_key,
    ).map_err(AppError::from);
    
    if result.is_ok() {
        // 查询 story_id 以发射同步事件
        let conn = pool.inner().get().map_err(AppError::from)?;
        let story_id: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT story_id FROM foreshadowing_tracker WHERE id = ?1",
            [&foreshadowing_id],
            |row| row.get(0)
        );
        if let Ok(story_id) = story_id {
            let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "foreshadowings");
        }
    }
    result
}

// ==================== 结构化大纲命令 ====================

#[command(rename_all = "snake_case")]
pub async fn generate_scene_outline(
    scene_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::agents::AgentResult, AppError> {
    use crate::agents::service::{AgentService, AgentTask, AgentType};
    use crate::agents::commands::ExecuteAgentRequest;
    use std::collections::HashMap;

    log::info!("[story_commands] {} called: scene_id={}", "generate_scene_outline", scene_id);
    let scene_repo = SceneRepository::new(pool.inner().clone());
    let scene = scene_repo.get_by_id(&scene_id)
        .map_err(|e| {
            log::error!("[story_commands] {} scene lookup failed: {}", "generate_scene_outline", e);
            AppError::from(e)

        })?
        .ok_or("Scene not found")?;

    // 构建输入：场景规划信息
    let mut input_parts = Vec::new();
    input_parts.push(format!("场景标题: {}", scene.title.as_deref().unwrap_or("未命名")));
    if let Some(ref goal) = scene.dramatic_goal {
        input_parts.push(format!("戏剧目标: {}", goal));
    }
    if let Some(ref pressure) = scene.external_pressure {
        input_parts.push(format!("外部压迫: {}", pressure));
    }
    if let Some(ref location) = scene.setting_location {
        input_parts.push(format!("场景地点: {}", location));
    }
    if let Some(ref time) = scene.setting_time {
        input_parts.push(format!("场景时间: {}", time));
    }
    if !scene.characters_present.is_empty() {
        input_parts.push(format!("出场角色: {}", scene.characters_present.join(", ")));
    }

    let input = input_parts.join("\n");

    let request = ExecuteAgentRequest {
        agent_type: AgentType::OutlinePlanner,
        story_id: scene.story_id.clone(),
        chapter_number: Some(scene.sequence_number.max(0) as u32),
        input: input.clone(),
        parameters: None,
    };

    let context = crate::agents::commands::build_agent_context(&app_handle, &request).await?;
    let task = AgentTask {
        id: uuid::Uuid::new_v4().to_string(),
        agent_type: AgentType::OutlinePlanner,
        context,
        input,
        parameters: HashMap::new(),
        tier: None,
    };

    let service = AgentService::new(app_handle);
    let result = service.execute_task(task).await.map_err(|e| {
        log::error!("[story_commands] {} LLM task failed: {}", "generate_scene_outline", e);
        e
    })?;
    log::info!("[story_commands] {} completed successfully", "generate_scene_outline");

    // 保存大纲到数据库
    let _ = scene_repo.update(&scene_id, &crate::db::repositories_v3::SceneUpdate {
        outline_content: Some(result.content.clone()),
        execution_stage: Some("outline".to_string()),
        ..Default::default()
    });

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn generate_scene_draft(
    scene_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::agents::AgentResult, AppError> {
    use crate::agents::service::{AgentService, AgentTask, AgentType};
    use crate::agents::commands::ExecuteAgentRequest;
    use std::collections::HashMap;

    log::info!("[story_commands] {} called: scene_id={}", "generate_scene_draft", scene_id);
    let scene_repo = SceneRepository::new(pool.inner().clone());
    let scene = scene_repo.get_by_id(&scene_id)
        .map_err(|e| {
            log::error!("[story_commands] {} scene lookup failed: {}", "generate_scene_draft", e);
            AppError::from(e)

        })?
        .ok_or("Scene not found")?;

    // 优先使用 outline_content，否则使用 dramatic_goal 等信息
    let outline = scene.outline_content.as_ref()
        .ok_or("场景还没有大纲，请先生成大纲")?;

    let mut input_parts = Vec::new();
    input_parts.push(format!("场景标题: {}", scene.title.as_deref().unwrap_or("未命名")));
    input_parts.push(format!("大纲:\n{}", outline));
    if let Some(ref goal) = scene.dramatic_goal {
        input_parts.push(format!("戏剧目标: {}", goal));
    }
    if let Some(ref pressure) = scene.external_pressure {
        input_parts.push(format!("外部压迫: {}", pressure));
    }
    if let Some(ref location) = scene.setting_location {
        input_parts.push(format!("场景地点: {}", location));
    }
    if let Some(ref time) = scene.setting_time {
        input_parts.push(format!("场景时间: {}", time));
    }
    if let Some(ref atmosphere) = scene.setting_atmosphere {
        input_parts.push(format!("场景氛围: {}", atmosphere));
    }
    if !scene.characters_present.is_empty() {
        input_parts.push(format!("出场角色: {}", scene.characters_present.join(", ")));
    }

    let input = input_parts.join("\n");

    let request = ExecuteAgentRequest {
        agent_type: AgentType::Writer,
        story_id: scene.story_id.clone(),
        chapter_number: Some(scene.sequence_number.max(0) as u32),
        input: input.clone(),
        parameters: None,
    };

    let context = crate::agents::commands::build_agent_context(&app_handle, &request).await?;
    let task = AgentTask {
        id: uuid::Uuid::new_v4().to_string(),
        agent_type: AgentType::Writer,
        context,
        input,
        parameters: HashMap::new(),
        tier: None,
    };

    let service = AgentService::new(app_handle.clone());
    let orchestrator = crate::agents::orchestrator::AgentOrchestrator::with_default_config(
        service,
        app_handle.clone(),
    );
    let workflow_result = orchestrator.generate(task, crate::agents::orchestrator::GenerationMode::Full).await.map_err(|e| {
        log::error!("[story_commands] {} LLM task failed: {}", "generate_scene_draft", e);
        e
    })?;
    log::info!("[story_commands] {} completed successfully", "generate_scene_draft");

    // 保存草稿到数据库
    let _ = scene_repo.update(&scene_id, &crate::db::repositories_v3::SceneUpdate {
        draft_content: Some(workflow_result.final_content.clone()),
        execution_stage: Some("drafting".to_string()),
        ..Default::default()
    });

    let result = crate::agents::AgentResult {
        content: workflow_result.final_content,
        score: Some(workflow_result.final_score),
        suggestions: workflow_result.steps.iter().flat_map(|s| s.suggestions.clone()).collect(),
        request_id: None,
    };

    Ok(result)
}


// ==================== 风格混合命令 ====================

#[command(rename_all = "snake_case")]
pub async fn get_story_style_blend(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<serde_json::Value>, AppError> {
    use crate::creative_engine::style::blend::StyleBlendConfig;

    let repo = StoryStyleConfigRepository::new(pool.inner().clone());
    
    // 优先返回 active 配置
    if let Ok(Some(config)) = repo.get_active_by_story(&story_id) {
        let blend: StyleBlendConfig = serde_json::from_str(&config.blend_json)
            .map_err(|e| format!("解析混合配置失败: {}", e))?;
        return Ok(Some(serde_json::json!({
            "id": config.id,
            "story_id": config.story_id,
            "name": config.name,
            "blend": blend,
            "is_active": config.is_active,
        })));
    }
    
    Ok(None)
}

#[command(rename_all = "snake_case")]
pub async fn set_story_style_blend(
    story_id: String,
    name: String,
    blend_json: String,
    pool: State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    use crate::creative_engine::style::blend::StyleBlendConfig;

    log::info!("[story_commands] {} called: story_id={}, name={}", "set_story_style_blend", story_id, name);
    // 验证 JSON 格式
    let blend: StyleBlendConfig = serde_json::from_str(&blend_json)
        .map_err(|e| format!("混合配置格式错误: {}", e))?;
    
    // 验证权重合理性
    if let Err(errors) = blend.validate() {
        return Err(AppError::validation_failed(format!("验证失败: {}", errors.join("; ")), None::<String>));
    }

    let repo = StoryStyleConfigRepository::new(pool.inner().clone());
    
    // 查找是否已有同名配置
    let existing = repo.get_all_by_story(&story_id)
        .map_err(AppError::from)?;
    
    if let Some(existing_config) = existing.iter().find(|c| c.name == name) {
        // 更新现有配置
        repo.update(&existing_config.id, Some(&name), Some(&blend_json))
            .map_err(|e| {
                log::error!("[story_commands] {} update failed: {}", "set_story_style_blend", e);
                AppError::from(e)

            })?;
        repo.set_active(&story_id, &existing_config.id)
            .map_err(|e| {
                log::error!("[story_commands] {} set_active failed: {}", "set_story_style_blend", e);
                AppError::from(e)

            })?;
        
        Ok(serde_json::json!({
            "id": existing_config.id,
            "story_id": story_id,
            "name": name,
            "blend": blend,
            "is_active": true,
            "updated": true,
        }))
    } else {
        // 创建新配置并设为 active
        let config = repo.create(&story_id, &name, &blend_json)
            .map_err(|e| {
                log::error!("[story_commands] {} create failed: {}", "set_story_style_blend", e);
                AppError::from(e)

            })?;
        repo.set_active(&story_id, &config.id)
            .map_err(|e| {
                log::error!("[story_commands] {} set_active failed: {}", "set_story_style_blend", e);
                AppError::from(e)

            })?;
        
        log::info!("[story_commands] {} created new config", "set_story_style_blend");
        Ok(serde_json::json!({
            "id": config.id,
            "story_id": story_id,
            "name": name,
            "blend": blend,
            "is_active": true,
            "created": true,
        }))
    }
}

#[command(rename_all = "snake_case")]
pub async fn update_scene_style_blend(
    scene_id: String,
    blend_override: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    use crate::creative_engine::style::blend::StyleBlendConfig;

    log::info!("[story_commands] {} called: scene_id={}", "update_scene_style_blend", scene_id);
    // 验证 JSON 格式（如果提供了）
    if let Some(ref json) = blend_override {
        let _: StyleBlendConfig = serde_json::from_str(json)
            .map_err(|e| format!("混合配置格式错误: {}", e))?;
    }

    let repo = SceneRepository::new(pool.inner().clone());
    let updates = SceneUpdate {
        style_blend_override: blend_override,
        ..Default::default()
    };
    repo.update(&scene_id, &updates)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "update_scene_style_blend", e);
            AppError::from(e)

        })?;
    
    log::info!("[story_commands] {} completed successfully", "update_scene_style_blend");
    Ok(())
}

#[command(rename_all = "snake_case")]
pub async fn check_style_drift(
    text: String,
    story_id: String,
    scene_number: Option<i32>,
    pool: State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    use crate::creative_engine::style::blend::StyleBlendConfig;
    use crate::creative_engine::style::dna::StyleDNA;
    use crate::creative_engine::style::StyleDriftChecker;

    // 1. 获取风格混合配置（scene override → story active）
    let blend = if let Some(n) = scene_number {
        let scene_repo = SceneRepository::new(pool.inner().clone());
        if let Ok(Some(scene)) = scene_repo.get_by_story(&story_id)
            .map(|scenes| scenes.into_iter().find(|s| s.sequence_number == n)) 
        {
            if let Some(ref override_json) = scene.style_blend_override {
                serde_json::from_str::<StyleBlendConfig>(override_json).ok()
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    
    let blend = blend.or_else(|| {
        let repo = StoryStyleConfigRepository::new(pool.inner().clone());
        repo.get_active_by_story(&story_id).ok().flatten()
            .and_then(|c| serde_json::from_str::<StyleBlendConfig>(&c.blend_json).ok())
    });

    let blend = blend.ok_or("未找到风格混合配置")?;

    // 2. 加载所有涉及的 DNA
    let dna_repo = StyleDnaRepository::new(pool.inner().clone());
    let mut dnas = Vec::new();
    for comp in &blend.components {
        if let Ok(Some(db_dna)) = dna_repo.get_by_id(&comp.dna_id) {
            if let Ok(dna) = serde_json::from_str::<StyleDNA>(&db_dna.dna_json) {
                dnas.push(dna);
            }
        }
    }

    if dnas.is_empty() {
        return Err(AppError::internal("无法加载风格 DNA 数据"));
    }

    // 3. 运行自检
    let result = StyleDriftChecker::check(&text, &blend, &dnas);

    Ok(serde_json::json!({
        "passed": result.passed,
        "overall_score": result.overall_score,
        "checks": result.checks.iter().map(|c| {
            serde_json::json!({
                "dimension": &c.dimension,
                "target_min": c.target_min,
                "target_max": c.target_max,
                "actual_value": c.actual_value,
                "score": c.score,
                "passed": c.passed,
                "suggestion": &c.suggestion,
            })
        }).collect::<Vec<_>>(),
    }))
}


// ==================== 创世引擎命令 ====================

#[command(rename_all = "snake_case")]
pub async fn get_story_outline(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<serde_json::Value>, AppError> {
    let repo = StoryOutlineRepository::new(pool.inner().clone());
    let outline = repo.get_by_story(&story_id).map_err(AppError::from)?;

    Ok(outline.map(|o| serde_json::json!({
        "id": o.id,
        "story_id": o.story_id,
        "content": o.content,
        "structure_json": o.structure_json,
        "act_count": o.act_count,
        "total_scenes_estimate": o.total_scenes_estimate,
        "created_at": o.created_at,
        "updated_at": o.updated_at,
    })))
}

#[command(rename_all = "snake_case")]
pub async fn update_story_outline(
    story_id: String,
    content: String,
    structure_json: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let repo = StoryOutlineRepository::new(pool.inner().clone());
    repo.update(&story_id, Some(&content), structure_json.as_deref())
        .map_err(AppError::from)?;
    let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "storyOutlines");
    Ok(())
}

#[command(rename_all = "snake_case")]
pub async fn create_character_relationship(
    story_id: String,
    source_character_id: String,
    target_character_id: String,
    relationship_type: String,
    description: Option<String>,
    dynamic: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, AppError> {
    log::info!("[story_commands] {} called: story_id={}", "create_character_relationship", story_id);
    let repo = CharacterRelationshipRepository::new(pool.inner().clone());
    let relationship = repo.create(
        &story_id,
        &source_character_id,
        &target_character_id,
        &relationship_type,
        description.as_deref(),
        dynamic.as_deref(),
    ).map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "create_character_relationship", e);
        e.to_string()
    })?;

    // P0-3 修复: 发射同步事件，确保幕后界面自动刷新
    let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "characterRelationships");

    Ok(serde_json::json!({
        "id": relationship.id,
        "story_id": relationship.story_id,
        "source_character_id": relationship.source_character_id,
        "target_character_id": relationship.target_character_id,
        "target_character_name": relationship.target_character_name,
        "relationship_type": relationship.relationship_type,
        "description": relationship.description,
        "dynamic": relationship.dynamic,
        "created_at": relationship.created_at,
    }))
}

#[command(rename_all = "snake_case")]
pub async fn update_character_relationship(
    relationship_id: String,
    relationship_type: Option<String>,
    description: Option<String>,
    dynamic: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    log::info!("[story_commands] {} called: relationship_id={}", "update_character_relationship", relationship_id);
    let repo = CharacterRelationshipRepository::new(pool.inner().clone());

    // D1 Phase 4: 查询旧关系数据用于级联改写对比
    let old_relationship = repo.get_by_id(&relationship_id).ok().flatten();

    repo.update(
        &relationship_id,
        relationship_type.as_deref(),
        description.as_deref(),
        dynamic.as_deref(),
    ).map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "update_character_relationship", e);
        e.to_string()
    })?;

    // P0-3 修复: 查询 story_id 并发射同步事件
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM character_relationships WHERE id = ?1",
        [&relationship_id],
        |row| row.get(0)
    );
    if let Ok(ref story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(story_id), "characterRelationships");

        // D1 Phase 4: 关系敏感字段变更触发级联改写
        if let Some(ref old) = old_relationship {
            let mut changed_fields = Vec::new();
            let mut before_map = serde_json::Map::new();
            let mut after_map = serde_json::Map::new();

            if let Some(ref new_val) = relationship_type {
                if old.relationship_type != *new_val {
                    changed_fields.push("relationship_type".to_string());
                    before_map.insert("relationship_type".to_string(), serde_json::json!(old.relationship_type));
                    after_map.insert("relationship_type".to_string(), serde_json::json!(new_val));
                }
            }
            if let Some(ref new_val) = description {
                if old.description.as_ref() != Some(new_val) {
                    changed_fields.push("description".to_string());
                    before_map.insert("description".to_string(), serde_json::json!(old.description));
                    after_map.insert("description".to_string(), serde_json::json!(new_val));
                }
            }
            if let Some(ref new_val) = dynamic {
                if old.dynamic.as_ref() != Some(new_val) {
                    changed_fields.push("dynamic".to_string());
                    before_map.insert("dynamic".to_string(), serde_json::json!(old.dynamic));
                    after_map.insert("dynamic".to_string(), serde_json::json!(new_val));
                }
            }

            if !changed_fields.is_empty() {
                let char_repo = CharacterRepository::new(pool.inner().clone());
                let source_name = char_repo.get_by_id(&old.source_character_id).ok().flatten().map(|c| c.name).unwrap_or_default();
                let target_name = char_repo.get_by_id(&old.target_character_id).ok().flatten().map(|c| c.name).unwrap_or_default();

                let mut change_events = Vec::new();
                for (char_id, char_name, other_name) in [
                    (&old.source_character_id, source_name.clone(), target_name.clone()),
                    (&old.target_character_id, target_name.clone(), source_name.clone()),
                ] {
                    if char_name.is_empty() { continue; }

                    let mut char_before = before_map.clone();
                    let mut char_after = after_map.clone();
                    char_before.insert("name".to_string(), serde_json::json!(&char_name));
                    char_before.insert("relationship_with".to_string(), serde_json::json!(&other_name));
                    char_after.insert("name".to_string(), serde_json::json!(&char_name));
                    char_after.insert("relationship_with".to_string(), serde_json::json!(&other_name));

                    let change_event = crate::creative_engine::cascade_rewriter::models::EntityChangeEvent {
                        story_id: story_id.clone(),
                        entity_id: char_id.clone(),
                        entity_type: "character".to_string(),
                        entity_name: char_name,
                        change_type: crate::creative_engine::cascade_rewriter::models::ChangeType::RelationModified,
                        before_json: serde_json::to_string(&char_before).unwrap_or_default(),
                        after_json: serde_json::to_string(&char_after).unwrap_or_default(),
                        changed_fields: changed_fields.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    };
                    change_events.push(change_event);
                }

                if !change_events.is_empty() {
                    let payload = crate::creative_engine::cascade_rewriter::models::CascadeTaskPayload {
                        story_id: story_id.clone(),
                        change_events,
                    };
                    let payload_json = serde_json::to_string(&payload).unwrap_or_default();

                    let req = crate::task_system::models::CreateTaskRequest {
                        name: format!("级联改写: {} 与 {} 的关系", source_name, target_name),
                        description: Some("因角色关系变更触发的场景级联改写".to_string()),
                        task_type: "cascade_rewrite".to_string(),
                        schedule_type: "once".to_string(),
                        cron_pattern: None,
                        payload: Some(payload_json),
                        enabled: Some(true),
                        max_retries: Some(3),
                        heartbeat_timeout_seconds: Some(300),
                    };

                    if let Some(task_service) = app_handle.try_state::<crate::task_system::service::TaskService>() {
                        match task_service.create_task(req) {
                            Ok(task) => log::info!("[CascadeRewrite] Created task {} for relationship {}", task.id, relationship_id),
                            Err(e) => log::warn!("[CascadeRewrite] Failed to create task for relationship {}: {}", relationship_id, e),
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[command(rename_all = "snake_case")]
pub async fn delete_character_relationship(
    relationship_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    log::info!("[story_commands] {} called: relationship_id={}", "delete_character_relationship", relationship_id);

    // 先查询 story_id 用于同步事件（删除后无法获取）
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM character_relationships WHERE id = ?1",
        [&relationship_id],
        |row| row.get(0)
    );

    let repo = CharacterRelationshipRepository::new(pool.inner().clone());
    repo.delete(&relationship_id).map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "delete_character_relationship", e);
        e.to_string()
    })?;

    // P0-3 修复: 发射同步事件
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "characterRelationships");
    }

    Ok(())
}

#[command(rename_all = "snake_case")]
pub async fn get_character_relationships(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let repo = CharacterRelationshipRepository::new(pool.inner().clone());
    let relationships = repo.get_by_story(&story_id).map_err(AppError::from)?;

    Ok(relationships.into_iter().map(|r| serde_json::json!({
        "id": r.id,
        "story_id": r.story_id,
        "source_character_id": r.source_character_id,
        "target_character_id": r.target_character_id,
        "target_character_name": r.target_character_name,
        "relationship_type": r.relationship_type,
        "description": r.description,
        "dynamic": r.dynamic,
        "created_at": r.created_at,
    })).collect())
}

// ==================== Character Quick View ====================

#[derive(Debug, Clone, Serialize)]
pub struct CharacterQuickView {
    pub id: String,
    pub name: String,
    pub appearance_summary: String,
    pub status_tags: Vec<String>,
    pub last_seen_chapter: i32,
}

#[command(rename_all = "snake_case")]
pub fn get_character_by_name(
    story_id: String,
    name: String,
    pool: State<'_, DbPool>,
) -> Result<Option<CharacterQuickView>, AppError> {
    let conn = pool.get().map_err(AppError::from)?;

    // 1. Find character by name in story
    let character: Option<(String, String, Option<String>, Option<String>)> = conn.query_row(
        "SELECT id, name, appearance, personality FROM characters WHERE story_id = ?1 AND name = ?2",
        [&story_id, &name],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?, row.get::<_, Option<String>>(3)?))
    ).ok();

    let (char_id, char_name, appearance, personality) = match character {
        Some(c) => c,
        None => return Ok(None),
    };

    // 2. Truncate appearance to 60 chars
    let appearance_text = appearance.unwrap_or_default();
    let has_more = appearance_text.chars().count() > 60;
    let appearance_summary = appearance_text
        .chars()
        .take(60)
        .collect::<String>()
        + if has_more { "..." } else { "" };

    // 3. Build status tags from personality keywords
    let mut status_tags = Vec::new();
    if let Some(ref p) = personality {
        let keywords = ["冷静", "冲动", "善良", "邪恶", "勇敢", "懦弱", "聪明", "愚蠢", "忠诚", "背叛", "温柔", "残暴", "乐观", "悲观"];
        for kw in &keywords {
            if p.contains(kw) {
                status_tags.push(kw.to_string());
            }
        }
    }

    // 4. Find last seen scene (using sequence_number as proxy for chapter)
    let last_seen_chapter: i32 = conn.query_row(
        "SELECT MAX(s.sequence_number) FROM scene_characters sc
         JOIN scenes s ON sc.scene_id = s.id
         WHERE sc.character_id = ?1",
        [&char_id],
        |row| row.get(0)
    ).unwrap_or(1);

    Ok(Some(CharacterQuickView {
        id: char_id,
        name: char_name,
        appearance_summary,
        status_tags,
        last_seen_chapter: last_seen_chapter.max(1),
    }))
}

// ==================== Story System: Projection Health Check ====================

#[command(rename_all = "snake_case")]
pub fn check_projection_health(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
) -> Result<crate::story_system::ProjectionHealthReport, AppError> {
    let engine = crate::story_system::StorySystemEngine::new(pool.inner().clone());
    engine.check_projection_health(&story_id, chapter_number)
        .map_err(AppError::internal)
}

// ==================== Analytics: Writing Statistics ====================

#[command(rename_all = "snake_case")]
pub fn get_writing_analytics(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<crate::analytics::WritingAnalytics, AppError> {
    let repo = SceneRepository::new(pool.inner().clone());
    let scenes = repo.get_by_story(&story_id)
        .map_err(AppError::from)?;
    let engine = crate::analytics::AnalyticsEngine::new();
    Ok(engine.analyze_writing_data(&story_id, &scenes))
}

// ==================== 角色状态命令 ====================

#[command(rename_all = "snake_case")]
pub async fn update_character_state(
    character_id: String,
    state: CharacterState,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = CharacterRepository::new(pool.inner().clone());
    repo.update_character_state(&character_id, &state).map_err(AppError::from)
}

// ==================== Cascade Rewriter 命令 ====================

/// 手动触发级联改写任务
#[command(rename_all = "snake_case")]
pub async fn trigger_cascade_rewrite(
    story_id: String,
    entity_id: String,
    entity_type: String,
    before_json: String,
    after_json: String,
    changed_fields: Vec<String>,
    _pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<String, AppError> {
    use crate::creative_engine::cascade_rewriter::models::{CascadeTaskPayload, ChangeType, EntityChangeEvent};
    use crate::task_system::models::CreateTaskRequest;
    use crate::task_system::service::TaskService;

    let change_event = EntityChangeEvent {
        story_id: story_id.clone(),
        entity_id: entity_id.clone(),
        entity_type: entity_type.clone(),
        entity_name: entity_id.clone(), // TODO: resolve entity name from KG
        change_type: ChangeType::AttributeModified,
        before_json,
        after_json,
        changed_fields,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let payload = CascadeTaskPayload {
        story_id: story_id.clone(),
        change_events: vec![change_event],
    };

    let payload_json = serde_json::to_string(&payload).map_err(|e| AppError::internal(format!("序列化失败: {}", e)))?;

    let req = CreateTaskRequest {
        name: format!("级联改写: {}", entity_id),
        description: Some(format!("因 {} 变更触发的场景级联改写", entity_type)),
        task_type: "cascade_rewrite".to_string(),
        schedule_type: "once".to_string(),
        cron_pattern: None,
        payload: Some(payload_json),
        enabled: Some(true),
        max_retries: Some(1),
        heartbeat_timeout_seconds: Some(300),
    };

    let task_service: State<TaskService> = app_handle.state();
    let task = task_service.create_task(req)?;

    Ok(task.id)
}

// ==================== Trait-based 业务逻辑测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSceneRepo {
        scenes: Vec<Scene>,
    }

    impl SceneRepo for MockSceneRepo {
        fn create(&self, _story_id: &str, _sequence_number: i32, _title: Option<&str>) -> Result<Scene, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName("mock create not implemented".to_string()))
        }
        fn get_by_id(&self, _id: &str) -> Result<Option<Scene>, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName("mock get_by_id not implemented".to_string()))
        }
        fn get_by_story(&self, _story_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
            Ok(self.scenes.clone())
        }
        fn get_by_chapter(&self, _chapter_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName("mock get_by_chapter not implemented".to_string()))
        }
        fn update(&self, _id: &str, _updates: &crate::db::SceneUpdate) -> Result<usize, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName("mock update not implemented".to_string()))
        }
        fn delete(&self, _id: &str) -> Result<usize, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName("mock delete not implemented".to_string()))
        }
        fn update_sequence(&self, _id: &str, _new_sequence: i32) -> Result<usize, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName("mock update_sequence not implemented".to_string()))
        }
    }

    fn make_test_scene(id: &str, story_id: &str, sequence: i32, title: &str) -> Scene {
        Scene {
            id: id.to_string(),
            story_id: story_id.to_string(),
            sequence_number: sequence,
            title: Some(title.to_string()),
            dramatic_goal: None,
            external_pressure: None,
            conflict_type: None,
            characters_present: vec![],
            character_conflicts: vec![],
            content: None,
            setting_location: None,
            setting_time: None,
            setting_atmosphere: None,
            previous_scene_id: None,
            next_scene_id: None,
            execution_stage: None,
            outline_content: None,
            draft_content: None,
            model_used: None,
            cost: None,
            created_at: chrono::Local::now(),
            updated_at: chrono::Local::now(),
            confidence_score: None,
            style_blend_override: None,
            foreshadowing_ids: None,
            chapter_id: None,
        }
    }

    #[test]
    fn test_get_story_scenes_core_returns_scenes() {
        let scenes = vec![
            make_test_scene("s1", "story-1", 1, "Scene One"),
            make_test_scene("s2", "story-1", 2, "Scene Two"),
        ];
        let mock = MockSceneRepo { scenes: scenes.clone() };
        let result = get_story_scenes_core(&mock, "story-1").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "s1");
        assert_eq!(result[1].id, "s2");
    }

    #[test]
    fn test_get_story_scenes_core_empty_when_no_scenes() {
        let mock = MockSceneRepo { scenes: vec![] };
        let result = get_story_scenes_core(&mock, "story-empty").unwrap();
        assert!(result.is_empty());
    }
}
