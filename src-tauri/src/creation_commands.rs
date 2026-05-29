//! Creation Commands

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
};

#[command(rename_all = "snake_case")]
pub async fn generate_world_building_options(
    user_input: String,
    app_handle: AppHandle,
) -> Result<Vec<WorldBuildingOption>, AppError> {
    let llm_service = LlmService::new(app_handle);
    let agent = NovelCreationAgent::new(llm_service);
    let options = GenerationOptions::default();

    agent
        .generate_world_building_options(&user_input, &options)
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

    agent
        .generate_character_profiles(&world_building, &options)
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

    agent
        .generate_writing_styles(&genre, &world_building, &options)
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

    agent
        .generate_first_scene(&world_building, &characters, &writing_style)
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

/// 在事务中持久化故事核心要素（Story + WorldBuilding + Characters +
/// WritingStyle + Scene） 供 create_story_with_wizard 和 CreationWorkflowEngine
/// 复用
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
    log::info!(
        "[story_commands] {} called: title={}",
        "create_story_with_wizard",
        title
    );
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
        )
        .map_err(|e| {
            log::error!(
                "[story_commands] {} element persistence failed: {}",
                "create_story_with_wizard",
                e
            );
            e
        })?;

        tx.commit().map_err(AppError::from)?;
        (story, created_chars, scene)
    };
    let story_id = story.id.clone();
    log::info!(
        "[story_commands] {} steps 1-5 committed: story_id={}",
        "create_story_with_wizard",
        story_id
    );

    // === 步骤 6: 异步 Ingest ===
    let ingest_text = format!(
        "世界观：{}\n\n历史背景：{}\n\n角色设定：\n{}\n\n文字风格：{}\n\n首个场景：{}\n\n{}",
        world_building.concept,
        &world_building.history,
        characters
            .iter()
            .map(|c| format!("- {}：{}，目标：{}", c.name, c.personality, c.goals))
            .collect::<Vec<_>>()
            .join("\n"),
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

    let ingest_result = pipeline.ingest(&ingest_content).await.map_err(|e| {
        log::error!(
            "[story_commands] {} ingest failed: {}",
            "create_story_with_wizard",
            e
        );
        AppError::from(e)
    })?;
    log::info!(
        "[story_commands] {} step 6 completed: {} entities, {} relations",
        "create_story_with_wizard",
        ingest_result.entities.len(),
        ingest_result.relations.len()
    );

    // === 事务 2: 保存 KG 数据 ===
    let (saved_entities, saved_relations) = {
        let mut conn2 = pool_ref
            .get()
            .map_err(|e| AppError::from(rusqlite::Error::InvalidParameterName(e.to_string())))?;
        let tx2 = conn2.transaction().map_err(AppError::from)?;
        let kg_repo = KnowledgeGraphRepository::new(pool_ref.clone());

        let mut saved_entities = 0usize;
        for entity in &ingest_result.entities {
            kg_repo
                .create_entity_in_tx(
                    &tx2,
                    &story_id,
                    &entity.name,
                    &entity.entity_type.to_string(),
                    &entity.attributes,
                    entity.embedding.clone(),
                )
                .map_err(|e| {
                    log::error!(
                        "[story_commands] {} KG entity save failed: {}",
                        "create_story_with_wizard",
                        e
                    );
                    AppError::from(e)
                })?;
            saved_entities += 1;
        }

        let entity_name_to_id: std::collections::HashMap<String, String> = ingest_result
            .entities
            .iter()
            .map(|e| (e.name.clone(), e.id.clone()))
            .collect();

        let mut saved_relations = 0usize;
        for relation in &ingest_result.relations {
            if let (Some(source_id), Some(target_id)) = (
                entity_name_to_id.get(&relation.source_id),
                entity_name_to_id.get(&relation.target_id),
            ) {
                kg_repo
                    .create_relation_in_tx(
                        &tx2,
                        &story_id,
                        source_id,
                        target_id,
                        &relation.relation_type.to_string(),
                        relation.strength,
                    )
                    .map_err(|e| {
                        log::error!(
                            "[story_commands] {} KG relation save failed: {}",
                            "create_story_with_wizard",
                            e
                        );
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
    let final_wb = wb_repo
        .get_by_story(&story_id)
        .map_err(|e| {
            log::error!(
                "[story_commands] {} final WB query failed: {}",
                "create_story_with_wizard",
                e
            );
            AppError::from(e)
        })?
        .ok_or_else(|| AppError::from("World building not found".to_string()))?;

    let ws_repo = WritingStyleRepository::new(pool_ref.clone());
    let final_ws = ws_repo
        .get_by_story(&story_id)
        .map_err(|e| {
            log::error!(
                "[story_commands] {} final WS query failed: {}",
                "create_story_with_wizard",
                e
            );
            AppError::from(e)
        })?
        .ok_or_else(|| AppError::from("Writing style not found".to_string()))?;

    let scene_repo = SceneRepository::new(pool_ref.clone());
    let final_scene = scene_repo
        .get_by_id(&scene.id)
        .map_err(|e| {
            log::error!(
                "[story_commands] {} final scene query failed: {}",
                "create_story_with_wizard",
                e
            );
            AppError::from(e)
        })?
        .ok_or_else(|| AppError::from("Scene not found".to_string()))?;

    log::info!(
        "[story_commands] {} completed successfully",
        "create_story_with_wizard"
    );

    // P0-2 修复: 发射同步事件，确保幕后界面自动刷新新创建的内容
    let _ = crate::state_sync::StateSync::emit_story_created(&app_handle, &story_id, &story.title);
    let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, Some(&story_id), "all");

    // 触发自动化事件：故事创建完成
    if let Err(e) = automation_service
        .trigger_event(crate::automation::triggers::TriggerEvent::StoryCreated {
            story_id: story_id.clone(),
        })
        .await
    {
        log::warn!(
            "[story_commands] Failed to trigger story created automation: {}",
            e
        );
    }

    // 触发自动化事件：角色创建完成（为每个创建的角色）
    for character in &created_chars {
        if let Err(e) = automation_service
            .trigger_event(
                crate::automation::triggers::TriggerEvent::CharacterCreated {
                    story_id: story_id.clone(),
                    character_id: character.id.clone(),
                },
            )
            .await
        {
            log::warn!(
                "[story_commands] Failed to trigger character created automation for {}: {}",
                character.id,
                e
            );
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
    })
}

// ==================== 场景版本命令 ====================

use crate::versions::service::{SceneVersionService, VersionChainNode, VersionDiff, VersionStats};

#[command(rename_all = "snake_case")]
pub async fn run_creation_workflow(
    story_id: String,
    mode: String, // "ai_only" | "ai_first" | "human_first"
    initial_input: String,
    app_handle: AppHandle,
    pool: State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    use crate::{
        agents::service::AgentService,
        creative_engine::workflow::{CreationMode, CreationWorkflowEngine},
    };

    log::info!(
        "[story_commands] {} called: story_id={}, mode={}",
        "run_creation_workflow",
        story_id,
        mode
    );
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
            log::info!(
                "[story_commands] {} completed: success={}",
                "run_creation_workflow",
                result.success
            );
            // 查询刚创建的 Scene ID，供前端直接跳转
            let scene_repo = SceneRepository::new(pool.inner().clone());
            let scene_id = scene_repo
                .get_by_story(&story_id)
                .ok()
                .and_then(|scenes| scenes.into_iter().last())
                .map(|s| s.id);

            // AiOnly 模式：spawn 后台 enrich 任务，从正文生成完整 World/Character/Style
            if mode == CreationMode::AiOnly {
                let pool_clone = pool.inner().clone();
                let story_id_clone = story_id.clone();
                let app_handle_clone = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    log::info!(
                        "[story_commands] Spawning background enrich for story_id={}",
                        story_id_clone
                    );
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

// ==================== StyleDNA 命令 ====================

#[tauri::command]
pub async fn list_style_dnas(pool: State<'_, DbPool>) -> Result<Vec<serde_json::Value>, AppError> {
    let repo = StyleDnaRepository::new(pool.inner().clone());
    match repo.get_all() {
        Ok(dnas) => {
            let result: Vec<serde_json::Value> = dnas
                .into_iter()
                .map(|d| {
                    serde_json::json!({
                        "id": d.id,
                        "name": d.name,
                        "author": d.author,
                        "is_builtin": d.is_builtin,
                        "is_user_created": d.is_user_created,
                    })
                })
                .collect();
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
    app_handle: AppHandle,
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
        Ok(_) => {
            let _ = crate::state_sync::StateSync::emit_data_refresh(
                &app_handle,
                Some(&story_id),
                "storyStyle",
            );
            Ok(())
        }
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
    use crate::{creative_engine::style::StyleAnalyzer, llm::service::LlmService};

    let llm_service = LlmService::new(app_handle.clone());
    let dna_name = name.unwrap_or_else(|| "自定义风格".to_string());

    let dna = StyleAnalyzer::analyze_with_llm(&text, &dna_name, &llm_service).await?;
    let dna_json =
        serde_json::to_string(&dna).map_err(|e| format!("序列化 StyleDNA 失败: {}", e))?;

    let repo = StyleDnaRepository::new(pool.inner().clone());
    let record = repo
        .create(&dna_name, dna.meta.author.as_deref(), &dna_json, false)
        .map_err(|e| format!("保存 StyleDNA 失败: {}", e))?;

    let _ = crate::state_sync::StateSync::emit_data_refresh(&app_handle, None, "styleDnas");

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
    let records = tracker.get_all(&story_id).map_err(AppError::from)?;
    let result: Vec<serde_json::Value> = records
        .into_iter()
        .map(|r| {
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
        })
        .collect();
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
    log::info!(
        "[story_commands] {} called: story_id={}",
        "create_foreshadowing",
        story_id
    );
    let tracker = ForeshadowingTracker::new(pool.inner().clone());
    let result = tracker
        .add_foreshadowing(&story_id, &content, setup_scene_id.as_deref(), importance)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "create_foreshadowing", e);
            AppError::from(e)
        });
    if result.is_ok() {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "foreshadowings",
        );
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
    }
    .map_err(AppError::from);

    if result.is_ok() {
        // 查询 story_id 以发射同步事件
        let conn = pool.inner().get().map_err(AppError::from)?;
        let story_id: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT story_id FROM foreshadowing_tracker WHERE id = ?1",
            [&id],
            |row| row.get(0),
        );
        if let Ok(story_id) = story_id {
            let _ = crate::state_sync::StateSync::emit_data_refresh(
                &app_handle,
                Some(&story_id),
                "foreshadowings",
            );
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
    let items = ledger.get_ledger(&story_id).map_err(AppError::from)?;

    let result: Vec<serde_json::Value> = items
        .into_iter()
        .map(|item| {
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
        })
        .collect();

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
    let items = ledger
        .detect_overdue(&story_id, current_scene_number)
        .map_err(AppError::from)?;

    let result: Vec<serde_json::Value> = items
        .into_iter()
        .map(|item| {
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
        })
        .collect();

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
    let recs = ledger
        .recommend_payoff_timing(&story_id, current_scene_number)
        .map_err(AppError::from)?;

    let result: Vec<serde_json::Value> = recs
        .into_iter()
        .map(|rec| {
            serde_json::json!({
                "foreshadowing_id": rec.foreshadowing_id,
                "ledger_key": rec.ledger_key,
                "title": rec.title,
                "recommended_scene": rec.recommended_scene,
                "urgency": rec.urgency.to_string(),
                "reason": rec.reason,
                "importance": rec.importance,
            })
        })
        .collect();

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
    let result = ledger
        .update_ledger_fields(
            &foreshadowing_id,
            target_start_scene,
            target_end_scene,
            risk_signals,
            scope,
            ledger_key,
        )
        .map_err(AppError::from);

    if result.is_ok() {
        // 查询 story_id 以发射同步事件
        let conn = pool.inner().get().map_err(AppError::from)?;
        let story_id: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT story_id FROM foreshadowing_tracker WHERE id = ?1",
            [&foreshadowing_id],
            |row| row.get(0),
        );
        if let Ok(story_id) = story_id {
            let _ = crate::state_sync::StateSync::emit_data_refresh(
                &app_handle,
                Some(&story_id),
                "foreshadowings",
            );
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
    use std::collections::HashMap;

    use crate::agents::{
        commands::ExecuteAgentRequest,
        service::{AgentService, AgentTask, AgentType},
    };

    log::info!(
        "[story_commands] {} called: scene_id={}",
        "generate_scene_outline",
        scene_id
    );
    let scene_repo = SceneRepository::new(pool.inner().clone());
    let scene = scene_repo
        .get_by_id(&scene_id)
        .map_err(|e| {
            log::error!(
                "[story_commands] {} scene lookup failed: {}",
                "generate_scene_outline",
                e
            );
            AppError::from(e)
        })?
        .ok_or("Scene not found")?;

    // 构建输入：场景规划信息
    let mut input_parts = Vec::new();
    input_parts.push(format!(
        "场景标题: {}",
        scene.title.as_deref().unwrap_or("未命名")
    ));
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
        log::error!(
            "[story_commands] {} LLM task failed: {}",
            "generate_scene_outline",
            e
        );
        e
    })?;
    log::info!(
        "[story_commands] {} completed successfully",
        "generate_scene_outline"
    );

    // 保存大纲到数据库
    let _ = scene_repo.update(
        &scene_id,
        &crate::db::repositories::SceneUpdate {
            outline_content: Some(result.content.clone()),
            execution_stage: Some("outline".to_string()),
            ..Default::default()
        },
    );

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn generate_scene_draft(
    scene_id: String,
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
        "generate_scene_draft",
        scene_id
    );
    let scene_repo = SceneRepository::new(pool.inner().clone());
    let scene = scene_repo
        .get_by_id(&scene_id)
        .map_err(|e| {
            log::error!(
                "[story_commands] {} scene lookup failed: {}",
                "generate_scene_draft",
                e
            );
            AppError::from(e)
        })?
        .ok_or("Scene not found")?;

    // 优先使用 outline_content，否则使用 dramatic_goal 等信息
    let outline = scene
        .outline_content
        .as_ref()
        .ok_or("场景还没有大纲，请先生成大纲")?;

    let mut input_parts = Vec::new();
    input_parts.push(format!(
        "场景标题: {}",
        scene.title.as_deref().unwrap_or("未命名")
    ));
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
    let workflow_result = orchestrator
        .generate(task, crate::agents::orchestrator::GenerationMode::Full)
        .await
        .map_err(|e| {
            log::error!(
                "[story_commands] {} LLM task failed: {}",
                "generate_scene_draft",
                e
            );
            e
        })?;
    log::info!(
        "[story_commands] {} completed successfully",
        "generate_scene_draft"
    );

    // 保存草稿到数据库
    let _ = scene_repo.update(
        &scene_id,
        &crate::db::repositories::SceneUpdate {
            draft_content: Some(workflow_result.final_content.clone()),
            execution_stage: Some("drafting".to_string()),
            ..Default::default()
        },
    );

    let result = crate::agents::AgentResult {
        content: workflow_result.final_content,
        score: Some(workflow_result.final_score),
        suggestions: workflow_result
            .steps
            .iter()
            .flat_map(|s| s.suggestions.clone())
            .collect(),
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
    app_handle: AppHandle,
) -> Result<serde_json::Value, AppError> {
    use crate::creative_engine::style::blend::StyleBlendConfig;

    log::info!(
        "[story_commands] {} called: story_id={}, name={}",
        "set_story_style_blend",
        story_id,
        name
    );
    // 验证 JSON 格式
    let blend: StyleBlendConfig =
        serde_json::from_str(&blend_json).map_err(|e| format!("混合配置格式错误: {}", e))?;

    // 验证权重合理性
    if let Err(errors) = blend.validate() {
        return Err(AppError::validation_failed(
            format!("验证失败: {}", errors.join("; ")),
            None::<String>,
        ));
    }

    let repo = StoryStyleConfigRepository::new(pool.inner().clone());

    // 查找是否已有同名配置
    let existing = repo.get_all_by_story(&story_id).map_err(AppError::from)?;

    if let Some(existing_config) = existing.iter().find(|c| c.name == name) {
        // 更新现有配置
        repo.update(&existing_config.id, Some(&name), Some(&blend_json))
            .map_err(|e| {
                log::error!(
                    "[story_commands] {} update failed: {}",
                    "set_story_style_blend",
                    e
                );
                AppError::from(e)
            })?;
        repo.set_active(&story_id, &existing_config.id)
            .map_err(|e| {
                log::error!(
                    "[story_commands] {} set_active failed: {}",
                    "set_story_style_blend",
                    e
                );
                AppError::from(e)
            })?;

        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "storyStyleBlend",
        );
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
        let config = repo.create(&story_id, &name, &blend_json).map_err(|e| {
            log::error!(
                "[story_commands] {} create failed: {}",
                "set_story_style_blend",
                e
            );
            AppError::from(e)
        })?;
        repo.set_active(&story_id, &config.id).map_err(|e| {
            log::error!(
                "[story_commands] {} set_active failed: {}",
                "set_story_style_blend",
                e
            );
            AppError::from(e)
        })?;

        log::info!(
            "[story_commands] {} created new config",
            "set_story_style_blend"
        );
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "storyStyleBlend",
        );
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
    app_handle: AppHandle,
) -> Result<(), AppError> {
    use crate::creative_engine::style::blend::StyleBlendConfig;

    log::info!(
        "[story_commands] {} called: scene_id={}",
        "update_scene_style_blend",
        scene_id
    );
    // 验证 JSON 格式（如果提供了）
    if let Some(ref json) = blend_override {
        let _: StyleBlendConfig =
            serde_json::from_str(json).map_err(|e| format!("混合配置格式错误: {}", e))?;
    }

    let repo = SceneRepository::new(pool.inner().clone());
    let updates = SceneUpdate {
        style_blend_override: blend_override,
        ..Default::default()
    };
    repo.update(&scene_id, &updates).map_err(|e| {
        log::error!(
            "[story_commands] {} failed: {}",
            "update_scene_style_blend",
            e
        );
        AppError::from(e)
    })?;

    log::info!(
        "[story_commands] {} completed successfully",
        "update_scene_style_blend"
    );
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM scenes WHERE id = ?1",
        [&scene_id],
        |row| row.get(0),
    );
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "storyStyleBlend",
        );
    }
    Ok(())
}

#[command(rename_all = "snake_case")]
pub async fn check_style_drift(
    text: String,
    story_id: String,
    scene_number: Option<i32>,
    pool: State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    use crate::creative_engine::style::{
        blend::StyleBlendConfig, dna::StyleDNA, StyleDriftChecker,
    };

    // 1. 获取风格混合配置（scene override → story active）
    let blend = if let Some(n) = scene_number {
        let scene_repo = SceneRepository::new(pool.inner().clone());
        if let Ok(Some(scene)) = scene_repo
            .get_by_story(&story_id)
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
        repo.get_active_by_story(&story_id)
            .ok()
            .flatten()
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

    Ok(outline.map(|o| {
        serde_json::json!({
            "id": o.id,
            "story_id": o.story_id,
            "content": o.content,
            "structure_json": o.structure_json,
            "act_count": o.act_count,
            "total_scenes_estimate": o.total_scenes_estimate,
            "created_at": o.created_at,
            "updated_at": o.updated_at,
        })
    }))
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
    let _ = crate::state_sync::StateSync::emit_data_refresh(
        &app_handle,
        Some(&story_id),
        "storyOutlines",
    );
    Ok(())
}
