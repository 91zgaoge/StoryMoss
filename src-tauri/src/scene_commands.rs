//! Scene Commands

#![allow(unused_imports)]

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Manager, State};

use crate::{
    agents::novel_creation::{GenerationOptions, NovelCreationAgent, SceneProposal},
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
    domain::novel_creation::{CharacterProfileOption, WorldBuildingOption, WritingStyleOption},
    error::AppError,
    llm::LlmService,
    memory::retention::RetentionManager,
    story_system::scene_service::SceneService,
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
    vector_store: State<'_, std::sync::Arc<dyn crate::ports::VectorStore>>,
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
    let has_content = content.is_some(); // Phase 4: 记录后才 move
    if has_extra {
        // v0.30.46 fix: create 时携带的 content
        // 等字段若更新失败，必须显式报错， 避免首次粘贴的正文静默丢失。
        repo.update(
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
        )
        .map_err(|e| {
            log::error!(
                "[story_commands] create_scene extra fields update failed: {}",
                e
            );
            AppError::from(e)
        })?;
        // P1-9 修复: 额外字段更新后发射 scene_updated，确保前端缓存刷新
        let _ = crate::state_sync::StateSync::emit_scene_updated(
            &app_handle,
            &story_id,
            &scene.id,
            scene.title.as_deref(),
            has_content, // Phase 4: 使用预先保存的值
        );
        // W2-F3: setting 字段变更同步触发 world_building 更新
        if has_setting_changes {
            let _ =
                crate::state_sync::StateSync::emit_world_building_updated(&app_handle, &story_id);
        }
    }

    // 委托领域服务处理后续业务编排
    let service = SceneService::new(
        pool.inner().clone(),
        app_handle,
        vector_store.inner().clone(),
    );
    service.on_scene_created(
        &scene,
        has_extra,
        has_setting_changes,
        automation_service.inner(),
    );

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
pub async fn get_story_scenes_paged(
    story_id: String,
    limit: i64,
    offset: i64,
    pool: State<'_, DbPool>,
) -> Result<Vec<Scene>, AppError> {
    let repo = SceneRepository::new(pool.inner().clone());
    repo.get_by_story_paged(&story_id, limit, offset)
        .map_err(AppError::from)
}

/// 查询指定章节关联的场景（幕前定位最新章时补拉其场景，保证 sceneId 解析）。
#[command(rename_all = "snake_case")]
pub async fn get_chapter_scenes(
    chapter_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<Scene>, AppError> {
    let repo = SceneRepository::new(pool.inner().clone());
    repo.get_by_chapter(&chapter_id).map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryWordCountResponse {
    pub total_chars: i64,
    pub scene_count: i64,
}

#[command(rename_all = "snake_case")]
pub async fn get_story_word_count(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<StoryWordCountResponse, AppError> {
    let repo = SceneRepository::new(pool.inner().clone());
    let total_chars = repo.total_content_length_by_story(&story_id)?;
    let scene_count = repo.count_by_story(&story_id)?;
    Ok(StoryWordCountResponse {
        total_chars,
        scene_count,
    })
}

#[command(rename_all = "snake_case")]
pub async fn get_scene(
    scene_id: String,
    pool: State<'_, DbPool>,
) -> Result<Option<Scene>, AppError> {
    let repo = SceneRepository::new(pool.inner().clone());
    repo.get_by_id(&scene_id).map_err(AppError::from)
}

/// 自定义方法论最大步数求解：不存在按 1 步；瞬时 DB 错误返回 None，
/// 调用方本次不推进，避免把 methodology_step 静默回退为 1。
fn custom_methodology_max_steps(
    result: Result<
        Option<crate::guidebook_distillation::CustomMethodology>,
        Box<dyn std::error::Error>,
    >,
) -> Option<i32> {
    match result {
        Ok(Some(cm)) => Some(cm.max_steps()),
        Ok(None) => Some(1),
        Err(_) => None,
    }
}

/// v0.31.0: 章节完成后推进 methodology_step（到该方法论最大步数停留）。
/// 返回是否发生了推进。story 无方法论或已到顶时为 noop。
fn advance_methodology_step(pool: &crate::db::DbPool, story_id: &str) -> bool {
    let story_repo = crate::db::StoryRepository::new(pool.clone());
    let story = match story_repo.get_by_id(story_id) {
        Ok(Some(s)) => s,
        _ => return false,
    };
    let mid = match story.methodology_id.as_deref() {
        Some(m) if !m.trim().is_empty() => m,
        _ => return false,
    };
    let current = story.methodology_step.unwrap_or(1);
    let next = if crate::domain::methodology::is_custom_methodology_id(mid) {
        // 自定义方法论：最大步数 = 步骤数，到顶停留；DB 瞬时错误本次不推进
        let max = match custom_methodology_max_steps(
            crate::guidebook_distillation::CustomMethodologyRepository::new(pool.clone())
                .get_by_id(mid),
        ) {
            Some(m) => m,
            None => return false,
        };
        (current.max(1) + 1).min(max)
    } else {
        crate::domain::methodology::next_methodology_step(mid, current)
    };
    if next == current {
        return false;
    }
    let req = crate::db::UpdateStoryRequest {
        title: None,
        description: None,
        genre: None,
        tone: None,
        pacing: None,
        style_dna_id: None,
        genre_profile_id: None,
        methodology_id: None,
        methodology_step: Some(next),
        reference_book_id: None,
        strategy_json: None,
        ..Default::default()
    };
    match story_repo.update(story_id, &req) {
        Ok(_) => {
            log::info!(
                "[update_scene] 章节完成，方法论 {} step {} -> {}",
                mid,
                current,
                next
            );
            true
        }
        Err(e) => {
            log::warn!("[update_scene] 推进 methodology_step 失败: {}", e);
            false
        }
    }
}

#[command(rename_all = "snake_case")]
pub async fn update_scene(
    scene_id: String,
    updates: SceneUpdate,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    automation_service: State<'_, crate::automation::service::AutomationService>,
    vector_store: State<'_, std::sync::Arc<dyn crate::ports::VectorStore>>,
) -> Result<usize, AppError> {
    log::info!(
        "[story_commands] {} called: scene_id={}",
        "update_scene",
        scene_id
    );
    // A3: 将同步 DB 查询/更新移到 spawn_blocking，避免阻塞 tokio worker。
    let pool_clone = pool.inner().clone();
    let scene_id_clone = scene_id.clone();
    let updates_clone = updates.clone();
    let (result, story_id_opt, had_content_before, prior_content) = tokio::task::spawn_blocking(
        move || -> Result<(usize, Option<String>, bool, Option<String>), AppError> {
            let pool_for_life = pool_clone.clone();
            let repo = SceneRepository::new(pool_clone);
            // 获取 story_id 用于同步事件（P0-3 修复: 避免 unwrap_or_default
            // 导致空字符串）
            let prior_scene = repo.get_by_id(&scene_id_clone).ok().flatten();
            let story_id_opt = prior_scene.as_ref().map(|s| s.story_id.clone());
            // v0.31.0: 章节完成检测——正文（或草稿）从无到有视为本章完成
            let had_content_before = prior_scene
                .as_ref()
                .map(|s| s.content.is_some() || s.draft_content.is_some())
                .unwrap_or(true);
            // v0.62.0 P2-B：保留前文，供作者手改的风格逆向学习做 before/after
            // 对比
            let prior_content = prior_scene.as_ref().and_then(|s| s.content.clone());
            let result = repo.update(&scene_id_clone, &updates_clone).map_err(|e| {
                log::error!("[story_commands] {} failed: {}", "update_scene", e);
                AppError::from(e)
            })?;
            // v0.64.7：续写/保存途中写成的死亡立即持久化——不必等 30s 后的
            // 章提交（真机第 10 章就是在一章之内把已死的公主写成活人的）。
            if let (Some(story_id), Some(content)) =
                (story_id_opt.as_deref(), updates_clone.content.as_deref())
            {
                let chapter_number = prior_scene
                    .as_ref()
                    .and_then(|s| s.chapter_id.clone())
                    .and_then(|cid| {
                        ChapterRepository::new(pool_for_life.clone())
                            .get_by_id(&cid)
                            .ok()
                            .flatten()
                    })
                    .map(|c| c.chapter_number);
                let marked = crate::story_system::life_status::refresh_after_text(
                    &pool_for_life,
                    story_id,
                    chapter_number,
                    content,
                );
                if !marked.is_empty() {
                    log::warn!(
                        "[story_commands] update_scene 检出死亡并标记已死：{:?}",
                        marked
                    );
                }
            }
            Ok((result, story_id_opt, had_content_before, prior_content))
        },
    )
    .await
    .map_err(|e| AppError::from(format!("[update_scene] spawn_blocking join error: {}", e)))??;

    // v0.26.50: AutoIngest 走 SceneIngestor
    // 防抖路径，避免每次自动保存立刻抢本地模型。正文变更改由
    // schedule_commit_and_split 同窗观察/ingest（v0.51.0）。
    let should_ingest = crate::story_system::scene_service::SceneIngestor::should_ingest(&updates);
    if crate::agency::observe::should_spawn_ingest_on_update(
        updates.content.is_some(),
        should_ingest,
    ) {
        crate::story_system::scene_service::SceneIngestor::spawn_ingest_debounced(
            scene_id.clone(),
            pool.inner().clone(),
            app_handle.clone(),
            vector_store.inner().clone(),
        );
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
        let content_changed = updates.content.is_some();
        let _ = crate::state_sync::StateSync::emit_scene_updated(
            &app_handle,
            story_id,
            &scene_id,
            updates.title.as_deref(),
            content_changed, // Phase 4
        );
        let word_count = updates
            .content
            .as_ref()
            .map(|c| c.split_whitespace().count())
            .unwrap_or(0);
        // user_edit 观察埋点（best-effort）：人类编辑触发（content 变更且
        // source 非 agency——agency 装配写入跳过，防自观察）。
        if content_changed && updates.source.as_deref() != Some("agency") {
            // v0.62.0 P2-B：作者手改 → 风格逆向学习（防抖 120s，按故事合并）。
            if let (Some(before), Some(after)) =
                (prior_content.as_deref(), updates.content.as_deref())
            {
                crate::story_system::style_learning::note_user_edit(
                    app_handle.clone(),
                    pool.inner().clone(),
                    story_id.clone(),
                    before,
                    after,
                );
            }
            if let Ok(dir) = app_handle.path().app_data_dir() {
                let logger = crate::agency::learning::ObservationLogger::new(dir);
                let sid = story_id.clone();
                let scid = scene_id.clone();
                tokio::spawn(async move {
                    logger.log(
                        &sid,
                        "user_edit",
                        "human",
                        serde_json::json!({
                            "scene_id": scid,
                            "word_count": word_count,
                        }),
                    );
                });
            }
        }
        let _ = automation_service
            .trigger_event(
                crate::automation::triggers::TriggerEvent::SceneContentUpdated {
                    story_id: story_id.clone(),
                    scene_id: scene_id.clone(),
                    word_count,
                },
            )
            .await;
        // v0.26.57 修复接线：编辑器保存路径此前未调度防抖
        // auto_commit + 自动分章（step 5 曾只在无人调用的
        // SceneService::on_scene_updated 中），此处补上，语义与领域服务一致。
        crate::story_system::scene_service::SceneService::new(
            pool.inner().clone(),
            app_handle.clone(),
            vector_store.inner().clone(),
        )
        .schedule_commit_and_split(&scene_id, story_id, content_changed);
    }
    // v0.31.0: 章节完成（正文从无到有）→ methodology_step +1（到顶停留）
    if updates.content.is_some() && !had_content_before {
        if let Some(ref story_id) = story_id_opt {
            let pool_step = pool.inner().clone();
            let story_id_step = story_id.clone();
            let _ = tokio::task::spawn_blocking(move || {
                advance_methodology_step(&pool_step, &story_id_step);
            })
            .await;
        }
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
        false, // Phase 4: reorder 不改变内容
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
    use crate::{agents::commentator::CommentatorAgent, domain::agent_context::AgentContext};

    log::info!(
        "[story_commands] {} called: story_id={}",
        "generate_paragraph_commentaries",
        story_id
    );
    let pool = app_handle.state::<crate::db::DbPool>();
    let builder = crate::creative_engine::StoryContextBuilder::new(pool.inner().clone());
    let mut context = match builder.build_quick(&story_id).await {
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

    // v0.60.0：从 KG 解析实体名（此前直接复用 entity_id，改写 prompt 里
    // 只有裸 id，模型无法判断改写对象）。解析失败回退 id 保持兼容。
    let entity_name = {
        let pool = _pool.inner();
        pool.get()
            .ok()
            .and_then(|conn| {
                conn.query_row(
                    "SELECT name FROM kg_entities WHERE id = ?1",
                    rusqlite::params![&entity_id],
                    |row| row.get::<_, String>(0),
                )
                .ok()
            })
            .unwrap_or_else(|| entity_id.clone())
    };
    let change_event = EntityChangeEvent {
        story_id: story_id.clone(),
        entity_id: entity_id.clone(),
        entity_type: entity_type.clone(),
        entity_name,
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
            narrative_intensity: None,
            narrative_sentiment: None,
            narrative_event_types: None,
            narrative_preceding_scene_id: None,
            narrative_following_scene_id: None,
            act_number: None,
            position_in_act: None,

            source: None,
            is_auto_generated: None,
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

    fn create_story(pool: &crate::db::DbPool, methodology_id: Option<&str>) -> crate::db::Story {
        crate::db::StoryRepository::new(pool.clone())
            .create(crate::db::CreateStoryRequest {
                title: "步进测试故事".to_string(),
                description: None,
                genre: Some("玄幻".to_string()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: methodology_id.map(|s| s.to_string()),
                reference_book_id: None,
            })
            .expect("create story")
    }

    fn update_step_req(step: i32) -> crate::db::UpdateStoryRequest {
        crate::db::UpdateStoryRequest {
            title: None,
            description: None,
            genre: None,
            tone: None,
            pacing: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            methodology_step: Some(step),
            reference_book_id: None,
            strategy_json: None,
            ..Default::default()
        }
    }

    #[test]
    fn test_advance_methodology_step_on_chapter_complete() {
        let pool = crate::db::create_test_pool().expect("test pool");
        let story = create_story(&pool, Some("snowflake"));

        assert!(advance_methodology_step(&pool, &story.id));
        let reloaded = crate::db::StoryRepository::new(pool.clone())
            .get_by_id(&story.id)
            .expect("get")
            .expect("story exists");
        assert_eq!(reloaded.methodology_step, Some(2));
    }

    #[test]
    fn test_advance_methodology_step_stays_at_max() {
        let pool = crate::db::create_test_pool().expect("test pool");
        let story = create_story(&pool, Some("snowflake"));
        crate::db::StoryRepository::new(pool.clone())
            .update(&story.id, &update_step_req(10))
            .expect("set step 10");

        assert!(!advance_methodology_step(&pool, &story.id), "到顶应停留");
        let reloaded = crate::db::StoryRepository::new(pool.clone())
            .get_by_id(&story.id)
            .expect("get")
            .expect("story exists");
        assert_eq!(reloaded.methodology_step, Some(10));
    }

    #[test]
    fn test_advance_methodology_step_noop_without_methodology() {
        let pool = crate::db::create_test_pool().expect("test pool");
        let story = create_story(&pool, None);
        assert!(!advance_methodology_step(&pool, &story.id));
        let reloaded = crate::db::StoryRepository::new(pool.clone())
            .get_by_id(&story.id)
            .expect("get")
            .expect("story exists");
        assert_eq!(reloaded.methodology_step, None);
    }

    #[test]
    fn test_custom_methodology_max_steps_db_error_means_no_advance() {
        // 瞬时 DB 错误：返回 None（不推进），避免误回退 methodology_step
        let err: Result<
            Option<crate::guidebook_distillation::CustomMethodology>,
            Box<dyn std::error::Error>,
        > = Err("simulated db error".into());
        assert_eq!(custom_methodology_max_steps(err), None);
    }

    #[test]
    fn test_custom_methodology_max_steps_missing_is_one() {
        assert_eq!(custom_methodology_max_steps(Ok(None)), Some(1));
    }

    #[test]
    fn test_custom_methodology_max_steps_uses_step_count() {
        let cm = crate::guidebook_distillation::CustomMethodology {
            id: "custom_t1".into(),
            guidebook_id: None,
            name: "三步法".into(),
            description: None,
            steps: (1..=3)
                .map(|i| crate::guidebook_distillation::MethodologyStep {
                    title: format!("s{}", i),
                    instruction: "i".into(),
                    checklist: vec![],
                })
                .collect(),
            patterns: vec![],
            cheatsheet: crate::guidebook_distillation::Cheatsheet::default(),
            enabled: true,
            created_at: chrono::Local::now(),
            updated_at: chrono::Local::now(),
        };
        assert_eq!(custom_methodology_max_steps(Ok(Some(cm))), Some(3));
    }

    #[test]
    fn test_advance_methodology_step_custom_methodology_advances() {
        let pool = crate::db::create_test_pool().expect("test pool");
        // 种一个 3 步的 custom methodology
        let cm = crate::guidebook_distillation::CustomMethodology {
            id: "custom_adv1".into(),
            guidebook_id: None,
            name: "三步法".into(),
            description: None,
            steps: (1..=3)
                .map(|i| crate::guidebook_distillation::MethodologyStep {
                    title: format!("s{}", i),
                    instruction: "i".into(),
                    checklist: vec![],
                })
                .collect(),
            patterns: vec![],
            cheatsheet: crate::guidebook_distillation::Cheatsheet::default(),
            enabled: true,
            created_at: chrono::Local::now(),
            updated_at: chrono::Local::now(),
        };
        crate::guidebook_distillation::CustomMethodologyRepository::new(pool.clone())
            .create(&cm)
            .expect("create custom methodology");
        let story = create_story(&pool, Some("custom_adv1"));

        assert!(advance_methodology_step(&pool, &story.id));
        let reloaded = crate::db::StoryRepository::new(pool.clone())
            .get_by_id(&story.id)
            .expect("get")
            .expect("story exists");
        assert_eq!(reloaded.methodology_step, Some(2));
    }
}
