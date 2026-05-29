//! Scene Commands

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
    versions::service::{SceneVersionService, VersionChainNode, VersionDiff, VersionStats},
};

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
    log::info!(
        "[story_commands] {} called: story_id={}",
        "create_scene",
        story_id
    );
    let repo = SceneRepository::new(pool.inner().clone());
    let scene = repo
        .create(&story_id, sequence_number, title.as_deref())
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_scene", e);
            AppError::from(e)
        })?;

    // W2-F3: 提前记录 setting 字段是否变更（后续 move 后无法使用）
    let has_setting_changes =
        setting_location.is_some() || setting_time.is_some() || setting_atmosphere.is_some();

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
            &app_handle,
            &story_id,
            &scene.id,
            scene.title.as_deref(),
        );
        // W2-F3: setting 字段变更同步触发 world_building 更新
        if has_setting_changes {
            let _ =
                crate::state_sync::StateSync::emit_world_building_updated(&app_handle, &story_id);
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
                let _ = skill_manager
                    .execute_hooks(crate::skills::HookEvent::OnSceneCreate, &context, data)
                    .await;
                log::info!(
                    "Hook executed: {:?}",
                    crate::skills::HookEvent::OnSceneCreate
                );
            });
        }
    }

    let _ = crate::state_sync::StateSync::emit_scene_created(
        &app_handle,
        &story_id,
        &scene.id,
        scene.title.as_deref(),
    );
    let _ = automation_service
        .trigger_event(crate::automation::triggers::TriggerEvent::SceneCreated {
            story_id: story_id.clone(),
            scene_id: scene.id.clone(),
        })
        .await;

    Ok(scene)
}

/// 业务逻辑层：获取故事的所有场景（可被 mock 测试）

pub fn get_story_scenes_core(
    repo: &dyn crate::db::traits::SceneRepo,
    story_id: &str,
) -> Result<Vec<Scene>, AppError> {
    repo.get_by_story(story_id).map_err(AppError::from)
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
    repo.get_by_id(&scene_id).map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn update_scene(
    scene_id: String,
    updates: SceneUpdate,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    automation_service: State<'_, crate::automation::service::AutomationService>,
) -> Result<usize, AppError> {
    log::info!(
        "[story_commands] {} called: scene_id={}",
        "update_scene",
        scene_id
    );
    let repo = SceneRepository::new(pool.inner().clone());
    // 获取 story_id 用于同步事件（P0-3 修复: 避免 unwrap_or_default 导致空字符串）
    let story_id_opt = repo.get_by_id(&scene_id).ok().flatten().map(|s| s.story_id);
    let result = repo.update(&scene_id, &updates).map_err(|e| {
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
                                log::warn!(
                                    "[AutoIngest] Scene {}: ingest failed: {}",
                                    scene_id_clone,
                                    e
                                );
                                None
                            }
                        }
                    };

                    if let Some(ingest_result) = ingest_result {
                        let kg_repo = KnowledgeGraphRepository::new(pool_clone.clone());
                        // 批量保存实体（保留 Ingest Pipeline 分配的 ID，冲突时更新）
                        let saved_entities = kg_repo
                            .save_entities_batch(&ingest_result.entities)
                            .unwrap_or(0);
                        let saved_relations = kg_repo
                            .save_relations_batch(&ingest_result.relations)
                            .unwrap_or(0);

                        // D1 Phase 4: 提取实体引用索引（entity_mentions）
                        let mention_repo =
                            crate::creative_engine::cascade_rewriter::EntityMentionRepository::new(
                                pool_clone.clone(),
                            );
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
                                    log::warn!(
                                        "[AutoIngest] Failed to create entity mention for {} in \
                                         scene {}: {}",
                                        entity_name,
                                        scene_id_clone,
                                        e
                                    );
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
                            &app_handle_for_sync,
                            &story_id,
                            "scene",
                        );
                        let _ = crate::state_sync::StateSync::emit_data_refresh(
                            &app_handle_for_sync,
                            Some(&story_id),
                            "knowledgeGraph",
                        );
                        if let Some(store) = crate::VECTOR_STORE.get() {
                            match crate::embeddings::embed_text_async(content_for_vector.clone())
                                .await
                            {
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
                                        Ok(_) => log::info!(
                                            "[AutoIngest] Scene {} indexed to vector store",
                                            scene_id_clone
                                        ),
                                        Err(e) => log::warn!(
                                            "[AutoIngest] Failed to index scene {}: {}",
                                            scene_id_clone,
                                            e
                                        ),
                                    }
                                }
                                Err(e) => {
                                    log::warn!(
                                        "[AutoIngest] Failed to generate embedding for scene {}: \
                                         {}",
                                        scene_id_clone,
                                        e
                                    );
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
        if updates.setting_location.is_some()
            || updates.setting_time.is_some()
            || updates.setting_atmosphere.is_some()
        {
            let _ =
                crate::state_sync::StateSync::emit_world_building_updated(&app_handle, story_id);
        }
        let _ = crate::state_sync::StateSync::emit_scene_updated(
            &app_handle,
            story_id,
            &scene_id,
            updates.title.as_deref(),
        );
        let word_count = updates
            .content
            .as_ref()
            .map(|c| c.split_whitespace().count())
            .unwrap_or(0);
        let _ = automation_service
            .trigger_event(
                crate::automation::triggers::TriggerEvent::SceneContentUpdated {
                    story_id: story_id.clone(),
                    scene_id: scene_id.clone(),
                    word_count,
                },
            )
            .await;
    }
    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn delete_scene(
    scene_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    log::info!(
        "[story_commands] {} called: scene_id={}",
        "delete_scene",
        scene_id
    );
    let repo = SceneRepository::new(pool.inner().clone());
    let story_id = repo.get_by_id(&scene_id).ok().flatten().map(|s| s.story_id);
    let result = repo.delete(&scene_id).map_err(|e| {
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

    let _ = crate::state_sync::StateSync::emit_scene_updated(
        &app_handle,
        &story_id,
        &scene_ids.first().cloned().unwrap_or_default(),
        None,
    );
    Ok(())
}

// ==================== 世界观命令 ====================

#[command(rename_all = "snake_case")]
pub async fn create_scene_annotation(
    scene_id: String,
    story_id: String,
    content: String,
    annotation_type: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<SceneAnnotation, AppError> {
    log::info!(
        "[story_commands] {} called: scene_id={}",
        "create_scene_annotation",
        scene_id
    );
    let repo = SceneAnnotationRepository::new(pool.inner().clone());
    let annotation = repo
        .create_annotation(&scene_id, &story_id, &content, &annotation_type)
        .map_err(|e| {
            log::error!(
                "[story_commands] {} failed: {}",
                "create_scene_annotation",
                e
            );
            AppError::from(e)
        })?;
    let _ = crate::state_sync::StateSync::emit_annotation_created(
        &app_handle,
        &story_id,
        &annotation.id,
        &scene_id,
    );
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
    let result = repo
        .resolve_annotation(&annotation_id)
        .map_err(AppError::from)?;
    if let Some((story_id, scene_id)) = meta_opt {
        let _ = crate::state_sync::StateSync::emit_annotation_resolved(
            &app_handle,
            &story_id,
            &annotation_id,
            &scene_id,
        );
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
    log::info!(
        "[story_commands] {} called: story_id={}, scene_id={:?}, chapter_id={:?}",
        "create_text_annotation",
        story_id,
        scene_id,
        chapter_id
    );
    let repo = TextAnnotationRepository::new(pool.inner().clone());
    repo.create_annotation(
        &story_id,
        scene_id.as_deref(),
        chapter_id.as_deref(),
        &content,
        &annotation_type,
        from_pos,
        to_pos,
    )
    .map_err(|e| {
        log::error!(
            "[story_commands] {} failed: {}",
            "create_text_annotation",
            e
        );
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
    use crate::agents::{commentator::CommentatorAgent, AgentContext};

    log::info!(
        "[story_commands] {} called: story_id={}",
        "generate_paragraph_commentaries",
        story_id
    );
    let pool = app_handle.state::<crate::db::DbPool>();
    let builder = crate::creative_engine::StoryContextBuilder::new(pool.inner().clone());
    let mut context = match builder.build_quick(&story_id) {
        Ok(ctx) => ctx,
        Err(e) => {
            log::warn!(
                "[story_commands] StoryContextBuilder failed: {}, falling back to minimal",
                e
            );
            AgentContext::minimal(story_id.clone(), String::new())
        }
    };
    // 覆盖从数据库读取的标题/题材（调用方可能传入覆盖值）
    context.story.story_title = story_title;
    context.story.genre = genre;

    let llm_service = LlmService::new(app_handle);
    let agent = CommentatorAgent::new(llm_service);
    let commentaries = agent.comment_on_text(&context, &text).await.map_err(|e| {
        log::error!(
            "[story_commands] {} failed: {}",
            "generate_paragraph_commentaries",
            e
        );
        AppError::from(e)
    })?;

    serde_json::to_string(&commentaries).map_err(|e| {
        log::error!(
            "[story_commands] {} serialization failed: {}",
            "generate_paragraph_commentaries",
            e
        );
        AppError::from(e)
    })
}

// ==================== 记忆压缩命令 ====================

#[command(rename_all = "snake_case")]
pub async fn get_scene_versions(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<SceneVersion>, AppError> {
    let repo = SceneVersionRepository::new(pool.inner().clone());
    repo.get_versions(&scene_id).map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_scene_version(
    version_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<SceneVersion>, AppError> {
    let repo = SceneVersionRepository::new(pool.inner().clone());
    repo.get_version(&version_id).map_err(AppError::from)
}

/// 为指定场景创建版本快照，并自动生成 ChangeTrack diff
pub fn create_version_snapshot(
    pool: &DbPool,
    scene_id: &str,
    change_summary: &str,
    created_by: &str,
) -> Result<Option<SceneVersion>, AppError> {
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
    let version_repo = SceneVersionRepository::new(pool.clone());
    let track_repo = ChangeTrackRepository::new(pool.clone());

    let scene = match scene_repo.get_by_id(scene_id) {
        Ok(Some(s)) => s,
        Ok(None) => return Ok(None),
        Err(e) => return Err(AppError::from(e)),
    };

    // 获取上一版本内容用于 diff
    let prev_content = version_repo
        .get_versions(scene_id)
        .map_err(AppError::from)?
        .into_iter()
        .next()
        .and_then(|v| v.content);

    let creator = match created_by {
        "user" => CreatorType::User,
        "ai" => CreatorType::Ai,
        _ => CreatorType::System,
    };

    let version = version_repo
        .create_version(&scene, change_summary, creator, None, None)
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
    let scene_repo = crate::db::repositories::SceneRepository::new(pool.inner().clone());
    let version_repo = SceneVersionRepository::new(pool.inner().clone());
    let track_repo = ChangeTrackRepository::new(pool.inner().clone());

    let scene = scene_repo
        .get_by_id(&scene_id)
        .map_err(AppError::from)?
        .ok_or("Scene not found")?;

    // 获取上一版本内容用于 diff
    let prev_content = version_repo
        .get_versions(&scene_id)
        .map_err(AppError::from)?
        .into_iter()
        .next()
        .and_then(|v| v.content);

    let creator = match created_by.as_str() {
        "user" => CreatorType::User,
        "ai" => CreatorType::Ai,
        _ => CreatorType::System,
    };

    let version = version_repo
        .create_version(&scene, &change_summary, creator, None, confidence_score)
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
) -> Vec<crate::db::models::ChangeTrack> {
    if old == new {
        return vec![];
    }

    // 找公共前缀
    let mut prefix = 0;
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    while prefix < old_chars.len()
        && prefix < new_chars.len()
        && old_chars[prefix] == new_chars[prefix]
    {
        prefix += 1;
    }

    // 找公共后缀
    let mut suffix = 0;
    while suffix < old_chars.len() - prefix
        && suffix < new_chars.len() - prefix
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
    service
        .compare_versions(&from_version_id, &to_version_id)
        .map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_scene_version_chain(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<VersionChainNode>, AppError> {
    let service = SceneVersionService::new(pool.inner().clone());
    service.get_version_chain(&scene_id).map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn get_version_change_tracks(
    version_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::models::ChangeTrack>, AppError> {
    let repo = crate::db::repositories::ChangeTrackRepository::new(pool.inner().clone());
    repo.get_by_version(&version_id).map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn restore_scene_version(
    scene_id: String,
    version_id: String,
    restored_by: String,
    pool: State<'_, DbPool>,
) -> Result<SceneVersion, AppError> {
    let service = SceneVersionService::new(pool.inner().clone());
    let result = service
        .restore_version(&scene_id, &version_id, &restored_by)
        .map_err(AppError::from)?;
    Ok(result.new_version)
}

#[command(rename_all = "snake_case")]
pub async fn get_scene_version_stats(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<VersionStats, AppError> {
    let service = SceneVersionService::new(pool.inner().clone());
    service.get_version_stats(&scene_id).map_err(AppError::from)
}

#[command(rename_all = "snake_case")]
pub async fn delete_scene_version(
    version_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = SceneVersionRepository::new(pool.inner().clone());
    repo.delete_version(&version_id).map_err(AppError::from)
}

// ==================== 变更追踪命令 (修订模式) ====================

#[command(rename_all = "snake_case")]
pub async fn update_character_state(
    character_id: String,
    state: CharacterState,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let repo = CharacterRepository::new(pool.inner().clone());
    repo.update_character_state(&character_id, &state)
        .map_err(AppError::from)
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
    use crate::{
        creative_engine::cascade_rewriter::models::{
            CascadeTaskPayload, ChangeType, EntityChangeEvent,
        },
        task_system::{models::CreateTaskRequest, service::TaskService},
    };

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

    let payload_json = serde_json::to_string(&payload)
        .map_err(|e| AppError::internal(format!("序列化失败: {}", e)))?;

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
        fn create(
            &self,
            _story_id: &str,
            _sequence_number: i32,
            _title: Option<&str>,
        ) -> Result<Scene, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName(
                "mock create not implemented".to_string(),
            ))
        }
        fn get_by_id(&self, _id: &str) -> Result<Option<Scene>, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName(
                "mock get_by_id not implemented".to_string(),
            ))
        }
        fn get_by_story(&self, _story_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
            Ok(self.scenes.clone())
        }
        fn get_by_chapter(&self, _chapter_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName(
                "mock get_by_chapter not implemented".to_string(),
            ))
        }
        fn update(
            &self,
            _id: &str,
            _updates: &crate::db::SceneUpdate,
        ) -> Result<usize, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName(
                "mock update not implemented".to_string(),
            ))
        }
        fn delete(&self, _id: &str) -> Result<usize, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName(
                "mock delete not implemented".to_string(),
            ))
        }
        fn update_sequence(&self, _id: &str, _new_sequence: i32) -> Result<usize, rusqlite::Error> {
            Err(rusqlite::Error::InvalidParameterName(
                "mock update_sequence not implemented".to_string(),
            ))
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
        let mock = MockSceneRepo {
            scenes: scenes.clone(),
        };
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
