//! Revision Commands

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
    scene_commands::create_version_snapshot,
};

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
    app_handle: AppHandle,
) -> Result<crate::db::models::ChangeTrack, AppError> {
    log::info!(
        "[story_commands] {} called: scene_id={:?}, chapter_id={:?}",
        "track_change",
        scene_id,
        chapter_id
    );
    let ct = match change_type.as_str() {
        "Delete" => ChangeType::Delete,
        "Format" => ChangeType::Format,
        _ => ChangeType::Insert,
    };

    let track = ChangeTrack::new(
        scene_id.clone(),
        chapter_id.clone(),
        author_id.unwrap_or_else(|| "user".to_string()),
        ct,
        from_pos,
        to_pos,
        content,
    );

    let repo = ChangeTrackRepository::new(pool.inner().clone());
    let result = repo.create(&track).map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "track_change", e);
        AppError::from(e)
    })?;

    // 查询 story_id 并发射同步事件
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = if let Some(ref sid) = scene_id {
        conn.query_row("SELECT story_id FROM scenes WHERE id = ?1", [sid], |row| {
            row.get(0)
        })
    } else if let Some(ref cid) = chapter_id {
        conn.query_row(
            "SELECT story_id FROM chapters WHERE id = ?1",
            [cid],
            |row| row.get(0),
        )
    } else {
        Err(rusqlite::Error::InvalidQuery)
    };
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "changeTracks",
        );
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn accept_change(
    change_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    log::info!(
        "[story_commands] {} called: change_id={}",
        "accept_change",
        change_id
    );
    let repo = ChangeTrackRepository::new(pool.inner().clone());
    let result = repo
        .update_status(&change_id, ChangeStatus::Accepted)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "accept_change", e);
            AppError::from(e)
        })?;

    // 自动创建版本快照
    if let Ok(Some(track)) = repo.get_by_id(&change_id) {
        if let Some(ref scene_id) = track.scene_id {
            let _ = create_version_snapshot(pool.inner(), scene_id, "接受变更", "system");
            let conn = pool.inner().get().map_err(AppError::from)?;
            let story_id: Result<String, rusqlite::Error> = conn.query_row(
                "SELECT story_id FROM scenes WHERE id = ?1",
                [scene_id],
                |row| row.get(0),
            );
            if let Ok(story_id) = story_id {
                let _ = crate::state_sync::StateSync::emit_data_refresh(
                    &app_handle,
                    Some(&story_id),
                    "changeTracks",
                );
            }
        }
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn reject_change(
    change_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    log::info!(
        "[story_commands] {} called: change_id={}",
        "reject_change",
        change_id
    );
    let repo = ChangeTrackRepository::new(pool.inner().clone());
    let result = repo
        .update_status(&change_id, ChangeStatus::Rejected)
        .map_err(|e| {
            log::error!("[story_commands] {} failed: {}", "reject_change", e);
            AppError::from(e)
        })?;

    // 自动创建版本快照
    if let Ok(Some(track)) = repo.get_by_id(&change_id) {
        if let Some(ref scene_id) = track.scene_id {
            let _ = create_version_snapshot(pool.inner(), scene_id, "拒绝变更", "system");
            let conn = pool.inner().get().map_err(AppError::from)?;
            let story_id: Result<String, rusqlite::Error> = conn.query_row(
                "SELECT story_id FROM scenes WHERE id = ?1",
                [scene_id],
                |row| row.get(0),
            );
            if let Ok(story_id) = story_id {
                let _ = crate::state_sync::StateSync::emit_data_refresh(
                    &app_handle,
                    Some(&story_id),
                    "changeTracks",
                );
            }
        }
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn get_pending_changes(
    scene_id: Option<String>,
    chapter_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::models::ChangeTrack>, AppError> {
    let repo = ChangeTrackRepository::new(pool.inner().clone());
    if let Some(sid) = scene_id {
        repo.get_pending_by_scene(&sid).map_err(AppError::from)
    } else if let Some(cid) = chapter_id {
        repo.get_pending_by_chapter(&cid).map_err(AppError::from)
    } else {
        Err(AppError::validation_failed(
            "Either scene_id or chapter_id must be provided",
            None::<String>,
        ))
    }
}

#[command(rename_all = "snake_case")]
pub async fn accept_all_changes(
    scene_id: Option<String>,
    chapter_id: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = ChangeTrackRepository::new(pool.inner().clone());
    let result = if let Some(sid) = scene_id.clone() {
        repo.accept_all_by_scene(&sid).map_err(AppError::from)?
    } else if let Some(cid) = chapter_id {
        repo.accept_all_by_chapter(&cid).map_err(AppError::from)?
    } else {
        return Err(AppError::validation_failed(
            "Either scene_id or chapter_id must be provided",
            None::<String>,
        ));
    };

    // 自动创建版本快照（仅场景级变更）并发射同步事件
    if let Some(ref sid) = scene_id {
        let _ = create_version_snapshot(pool.inner(), sid, "全部接受变更", "system");
        let conn = pool.inner().get().map_err(AppError::from)?;
        let story_id: Result<String, rusqlite::Error> =
            conn.query_row("SELECT story_id FROM scenes WHERE id = ?1", [sid], |row| {
                row.get(0)
            });
        if let Ok(story_id) = story_id {
            let _ = crate::state_sync::StateSync::emit_data_refresh(
                &app_handle,
                Some(&story_id),
                "changeTracks",
            );
        }
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn reject_all_changes(
    scene_id: Option<String>,
    chapter_id: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = ChangeTrackRepository::new(pool.inner().clone());
    let result = if let Some(sid) = scene_id.clone() {
        repo.reject_all_by_scene(&sid).map_err(AppError::from)?
    } else if let Some(cid) = chapter_id {
        repo.reject_all_by_chapter(&cid).map_err(AppError::from)?
    } else {
        return Err(AppError::validation_failed(
            "Either scene_id or chapter_id must be provided",
            None::<String>,
        ));
    };

    // 自动创建版本快照（仅场景级变更）并发射同步事件
    if let Some(ref sid) = scene_id {
        let _ = create_version_snapshot(pool.inner(), sid, "全部拒绝变更", "system");
        let conn = pool.inner().get().map_err(AppError::from)?;
        let story_id: Result<String, rusqlite::Error> =
            conn.query_row("SELECT story_id FROM scenes WHERE id = ?1", [sid], |row| {
                row.get(0)
            });
        if let Ok(story_id) = story_id {
            let _ = crate::state_sync::StateSync::emit_data_refresh(
                &app_handle,
                Some(&story_id),
                "changeTracks",
            );
        }
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
    app_handle: AppHandle,
) -> Result<crate::db::models::CommentThread, AppError> {
    log::info!(
        "[story_commands] {} called: scene_id={:?}, chapter_id={:?}",
        "create_comment_thread",
        scene_id,
        chapter_id
    );
    let at = match anchor_type.as_str() {
        "SceneLevel" => AnchorType::SceneLevel,
        _ => AnchorType::TextRange,
    };

    let thread = CommentThread::new(
        version_id,
        at,
        scene_id.clone(),
        chapter_id.clone(),
        from_pos,
        to_pos,
        selected_text,
    );

    let repo = CommentThreadRepository::new(pool.inner().clone());
    let result = repo.create_thread(&thread).map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "create_comment_thread", e);
        AppError::from(e)
    })?;

    // 查询 story_id 并发射同步事件
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = if let Some(ref sid) = scene_id {
        conn.query_row("SELECT story_id FROM scenes WHERE id = ?1", [sid], |row| {
            row.get(0)
        })
    } else if let Some(ref cid) = chapter_id {
        conn.query_row(
            "SELECT story_id FROM chapters WHERE id = ?1",
            [cid],
            |row| row.get(0),
        )
    } else {
        Err(rusqlite::Error::InvalidQuery)
    };
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "commentThreads",
        );
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn add_comment_message(
    thread_id: String,
    content: String,
    author_id: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::db::models::CommentMessage, AppError> {
    use chrono::Local;
    use uuid::Uuid;

    log::info!(
        "[story_commands] {} called: thread_id={}",
        "add_comment_message",
        thread_id
    );
    let message = CommentMessage {
        id: Uuid::new_v4().to_string(),
        thread_id,
        author_id: author_id.unwrap_or_else(|| "user".to_string()),
        author_name: None,
        content,
        created_at: Local::now(),
    };

    let repo = CommentThreadRepository::new(pool.inner().clone());
    let result = repo.add_message(&message).map_err(|e| {
        log::error!("[story_commands] {} failed: {}", "add_comment_message", e);
        AppError::from(e)
    })?;

    // 查询 story_id 并发射同步事件
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT COALESCE(s.story_id, c.story_id) FROM comment_threads t
         LEFT JOIN scenes s ON t.scene_id = s.id
         LEFT JOIN chapters c ON t.chapter_id = c.id
         WHERE t.id = ?1",
        [&message.thread_id],
        |row| row.get(0),
    );
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "commentThreads",
        );
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn get_comment_threads(
    scene_id: Option<String>,
    chapter_id: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::models::CommentThreadWithMessages>, AppError> {
    let repo = CommentThreadRepository::new(pool.inner().clone());
    if let Some(sid) = scene_id {
        repo.get_threads_by_scene(&sid).map_err(AppError::from)
    } else if let Some(cid) = chapter_id {
        repo.get_threads_by_chapter(&cid).map_err(AppError::from)
    } else {
        Err(AppError::validation_failed(
            "Either scene_id or chapter_id must be provided",
            None::<String>,
        ))
    }
}

#[command(rename_all = "snake_case")]
pub async fn resolve_comment_thread(
    thread_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = CommentThreadRepository::new(pool.inner().clone());
    let result = repo.resolve_thread(&thread_id).map_err(AppError::from)?;

    // 查询 story_id 并发射同步事件
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT COALESCE(s.story_id, c.story_id) FROM comment_threads t
         LEFT JOIN scenes s ON t.scene_id = s.id
         LEFT JOIN chapters c ON t.chapter_id = c.id
         WHERE t.id = ?1",
        [&thread_id],
        |row| row.get(0),
    );
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "commentThreads",
        );
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn reopen_comment_thread(
    thread_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let repo = CommentThreadRepository::new(pool.inner().clone());
    let result = repo.reopen_thread(&thread_id).map_err(AppError::from)?;

    // 查询 story_id 并发射同步事件
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT COALESCE(s.story_id, c.story_id) FROM comment_threads t
         LEFT JOIN scenes s ON t.scene_id = s.id
         LEFT JOIN chapters c ON t.chapter_id = c.id
         WHERE t.id = ?1",
        [&thread_id],
        |row| row.get(0),
    );
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "commentThreads",
        );
    }

    Ok(result)
}

#[command(rename_all = "snake_case")]
pub async fn delete_comment_thread(
    thread_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    // 先查询 story_id 用于同步事件（删除后无法获取）
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT COALESCE(s.story_id, c.story_id) FROM comment_threads t
         LEFT JOIN scenes s ON t.scene_id = s.id
         LEFT JOIN chapters c ON t.chapter_id = c.id
         WHERE t.id = ?1",
        [&thread_id],
        |row| row.get(0),
    );

    let repo = CommentThreadRepository::new(pool.inner().clone());
    let result = repo.delete_thread(&thread_id).map_err(AppError::from)?;

    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "commentThreads",
        );
    }

    Ok(result)
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
    log::info!(
        "[story_commands] {} called: story_id={}",
        "create_character_relationship",
        story_id
    );
    let repo = CharacterRelationshipRepository::new(pool.inner().clone());
    let relationship = repo
        .create(
            &story_id,
            &source_character_id,
            &target_character_id,
            &relationship_type,
            description.as_deref(),
            dynamic.as_deref(),
        )
        .map_err(|e| {
            log::error!(
                "[story_commands] {} failed: {}",
                "create_character_relationship",
                e
            );
            e.to_string()
        })?;

    // P0-3 修复: 发射同步事件，确保幕后界面自动刷新
    let _ = crate::state_sync::StateSync::emit_data_refresh(
        &app_handle,
        Some(&story_id),
        "characterRelationships",
    );

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
    log::info!(
        "[story_commands] {} called: relationship_id={}",
        "update_character_relationship",
        relationship_id
    );
    let repo = CharacterRelationshipRepository::new(pool.inner().clone());

    // D1 Phase 4: 查询旧关系数据用于级联改写对比
    let old_relationship = repo.get_by_id(&relationship_id).ok().flatten();

    repo.update(
        &relationship_id,
        relationship_type.as_deref(),
        description.as_deref(),
        dynamic.as_deref(),
    )
    .map_err(|e| {
        log::error!(
            "[story_commands] {} failed: {}",
            "update_character_relationship",
            e
        );
        e.to_string()
    })?;

    // P0-3 修复: 查询 story_id 并发射同步事件
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM character_relationships WHERE id = ?1",
        [&relationship_id],
        |row| row.get(0),
    );
    if let Ok(ref story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(story_id),
            "characterRelationships",
        );

        // D1 Phase 4: 关系敏感字段变更触发级联改写
        if let Some(ref old) = old_relationship {
            let mut changed_fields = Vec::new();
            let mut before_map = serde_json::Map::new();
            let mut after_map = serde_json::Map::new();

            if let Some(ref new_val) = relationship_type {
                if old.relationship_type != *new_val {
                    changed_fields.push("relationship_type".to_string());
                    before_map.insert(
                        "relationship_type".to_string(),
                        serde_json::json!(old.relationship_type),
                    );
                    after_map.insert("relationship_type".to_string(), serde_json::json!(new_val));
                }
            }
            if let Some(ref new_val) = description {
                if old.description.as_ref() != Some(new_val) {
                    changed_fields.push("description".to_string());
                    before_map.insert(
                        "description".to_string(),
                        serde_json::json!(old.description),
                    );
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
                let source_name = char_repo
                    .get_by_id(&old.source_character_id)
                    .ok()
                    .flatten()
                    .map(|c| c.name)
                    .unwrap_or_default();
                let target_name = char_repo
                    .get_by_id(&old.target_character_id)
                    .ok()
                    .flatten()
                    .map(|c| c.name)
                    .unwrap_or_default();

                let mut change_events = Vec::new();
                for (char_id, char_name, other_name) in [
                    (
                        &old.source_character_id,
                        source_name.clone(),
                        target_name.clone(),
                    ),
                    (
                        &old.target_character_id,
                        target_name.clone(),
                        source_name.clone(),
                    ),
                ] {
                    if char_name.is_empty() {
                        continue;
                    }

                    let mut char_before = before_map.clone();
                    let mut char_after = after_map.clone();
                    char_before.insert("name".to_string(), serde_json::json!(&char_name));
                    char_before.insert(
                        "relationship_with".to_string(),
                        serde_json::json!(&other_name),
                    );
                    char_after.insert("name".to_string(), serde_json::json!(&char_name));
                    char_after.insert(
                        "relationship_with".to_string(),
                        serde_json::json!(&other_name),
                    );

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
                    let payload =
                        crate::creative_engine::cascade_rewriter::models::CascadeTaskPayload {
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

                    if let Some(task_service) =
                        app_handle.try_state::<crate::task_system::service::TaskService>()
                    {
                        match task_service.create_task(req) {
                            Ok(task) => log::info!(
                                "[CascadeRewrite] Created task {} for relationship {}",
                                task.id,
                                relationship_id
                            ),
                            Err(e) => log::warn!(
                                "[CascadeRewrite] Failed to create task for relationship {}: {}",
                                relationship_id,
                                e
                            ),
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
    log::info!(
        "[story_commands] {} called: relationship_id={}",
        "delete_character_relationship",
        relationship_id
    );

    // 先查询 story_id 用于同步事件（删除后无法获取）
    let conn = pool.inner().get().map_err(AppError::from)?;
    let story_id: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT story_id FROM character_relationships WHERE id = ?1",
        [&relationship_id],
        |row| row.get(0),
    );

    let repo = CharacterRelationshipRepository::new(pool.inner().clone());
    repo.delete(&relationship_id).map_err(|e| {
        log::error!(
            "[story_commands] {} failed: {}",
            "delete_character_relationship",
            e
        );
        e.to_string()
    })?;

    // P0-3 修复: 发射同步事件
    if let Ok(story_id) = story_id {
        let _ = crate::state_sync::StateSync::emit_data_refresh(
            &app_handle,
            Some(&story_id),
            "characterRelationships",
        );
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

    Ok(relationships
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "story_id": r.story_id,
                "source_character_id": r.source_character_id,
                "target_character_id": r.target_character_id,
                "target_character_name": r.target_character_name,
                "relationship_type": r.relationship_type,
                "description": r.description,
                "dynamic": r.dynamic,
                "created_at": r.created_at,
            })
        })
        .collect())
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
