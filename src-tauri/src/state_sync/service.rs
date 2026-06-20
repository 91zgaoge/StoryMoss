#![allow(dead_code)]
//! 状态同步服务
//!
//! 提供便捷的静态方法，在所有数据变更操作后发射同步事件。
//! 前后台窗口通过监听 `sync-event` 频道实现自动刷新。

use tauri::{AppHandle, Emitter, Runtime};

use super::events::SyncEvent;

/// 状态同步发射器
///
/// 所有方法都是静态方法，无需实例化。直接通过 AppHandle 发射事件到所有窗口。
pub struct StateSync;

impl StateSync {
    /// 发射同步事件到所有窗口
    fn emit_event<R: Runtime>(app: &AppHandle<R>, event: SyncEvent) {
        let event_name = match &event {
            SyncEvent::StoryCreated { .. } => "story-created",
            SyncEvent::StoryUpdated { .. } => "story-updated",
            SyncEvent::StoryDeleted { .. } => "story-deleted",
            SyncEvent::StorySelected { .. } => "story-selected",
            SyncEvent::CharacterCreated { .. } => "character-created",
            SyncEvent::CharacterUpdated { .. } => "character-updated",
            SyncEvent::CharacterDeleted { .. } => "character-deleted",
            SyncEvent::SceneCreated { .. } => "scene-created",
            SyncEvent::SceneUpdated { .. } => "scene-updated",
            SyncEvent::SceneDeleted { .. } => "scene-deleted",
            SyncEvent::SceneSelected { .. } => "scene-selected",
            SyncEvent::ChapterCreated { .. } => "chapter-created",
            SyncEvent::ChapterUpdated { .. } => "chapter-updated",
            SyncEvent::ChapterDeleted { .. } => "chapter-deleted",
            SyncEvent::WorldBuildingUpdated { .. } => "world-building-updated",
            SyncEvent::WorldBuildingCreated { .. } => "world-building-created",
            SyncEvent::WorldBuildingDeleted { .. } => "world-building-deleted",
            SyncEvent::StyleDnaUpdated { .. } => "style-dna-updated",
            SyncEvent::TaskCreated { .. } => "task-created",
            SyncEvent::TaskUpdated { .. } => "task-updated",
            SyncEvent::TaskCompleted { .. } => "task-completed",
            SyncEvent::AnnotationCreated { .. } => "annotation-created",
            SyncEvent::AnnotationResolved { .. } => "annotation-resolved",
            SyncEvent::CharacterRelationshipsUpdated { .. } => "character-relationships-updated",
            SyncEvent::PayoffLedgerUpdated { .. } => "payoff-ledger-updated",
            SyncEvent::IngestionCompleted { .. } => "ingestion-completed",
            SyncEvent::DataRefresh { .. } => "data-refresh",
            SyncEvent::SubscriptionChanged { .. } => "subscription-changed",
            SyncEvent::PayoffOverdue { .. } => "payoff-overdue",
            SyncEvent::AuditRewriteSuggested { .. } => "audit-rewrite-suggested",
        };

        // 发射到通用频道 `sync-event`
        if let Err(e) = app.emit("sync-event", &event) {
            log::warn!("[StateSync] Failed to emit sync-event: {}", e);
        }

        // 同时发射到具体事件频道（便于前端单独监听）
        if let Err(e) = app.emit(event_name, &event) {
            log::warn!("[StateSync] Failed to emit {}: {}", event_name, e);
        }

        log::debug!(
            "[StateSync] Emitted {} for {:?}",
            event_name,
            event.story_id()
        );
    }

    // ==================== Story 事件 ====================

    pub fn emit_story_created<R: Runtime>(app: &AppHandle<R>, story_id: &str, title: &str) {
        Self::emit_event(
            app,
            SyncEvent::StoryCreated {
                story_id: story_id.to_string(),
                title: Some(title.to_string()),
            },
        );
    }

    pub fn emit_story_updated<R: Runtime>(app: &AppHandle<R>, story_id: &str, title: Option<&str>) {
        Self::emit_event(
            app,
            SyncEvent::StoryUpdated {
                story_id: story_id.to_string(),
                title: title.map(|s| s.to_string()),
            },
        );
    }

    pub fn emit_story_deleted<R: Runtime>(app: &AppHandle<R>, story_id: &str) {
        Self::emit_event(
            app,
            SyncEvent::StoryDeleted {
                story_id: story_id.to_string(),
            },
        );
    }

    #[allow(dead_code)]
    pub fn emit_story_selected<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        title: Option<&str>,
    ) {
        Self::emit_event(
            app,
            SyncEvent::StorySelected {
                story_id: story_id.to_string(),
                title: title.map(|s| s.to_string()),
            },
        );
    }

    // ==================== Character 事件 ====================

    pub fn emit_character_created<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        character_id: &str,
        name: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::CharacterCreated {
                story_id: story_id.to_string(),
                character_id: character_id.to_string(),
                name: name.to_string(),
            },
        );
    }

    pub fn emit_character_updated<R: Runtime>(
        app: &AppHandle<R>,
        character_id: &str,
        name: Option<&str>,
        story_id: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::CharacterUpdated {
                story_id: story_id.to_string(),
                character_id: character_id.to_string(),
                name: name.map(|s| s.to_string()),
            },
        );
    }

    pub fn emit_character_deleted<R: Runtime>(
        app: &AppHandle<R>,
        character_id: &str,
        story_id: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::CharacterDeleted {
                story_id: story_id.to_string(),
                character_id: character_id.to_string(),
            },
        );
    }

    // ==================== Scene 事件 ====================

    pub fn emit_scene_created<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        scene_id: &str,
        title: Option<&str>,
    ) {
        Self::emit_event(
            app,
            SyncEvent::SceneCreated {
                story_id: story_id.to_string(),
                scene_id: scene_id.to_string(),
                title: title.map(|s| s.to_string()),
            },
        );
    }

    pub fn emit_scene_updated<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        scene_id: &str,
        title: Option<&str>,
    ) {
        Self::emit_event(
            app,
            SyncEvent::SceneUpdated {
                story_id: story_id.to_string(),
                scene_id: scene_id.to_string(),
                title: title.map(|s| s.to_string()),
            },
        );
    }

    pub fn emit_scene_deleted<R: Runtime>(app: &AppHandle<R>, story_id: &str, scene_id: &str) {
        Self::emit_event(
            app,
            SyncEvent::SceneDeleted {
                story_id: story_id.to_string(),
                scene_id: scene_id.to_string(),
            },
        );
    }

    #[allow(dead_code)]
    pub fn emit_scene_selected<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        scene_id: &str,
        title: Option<&str>,
    ) {
        Self::emit_event(
            app,
            SyncEvent::SceneSelected {
                story_id: story_id.to_string(),
                scene_id: scene_id.to_string(),
                title: title.map(|s| s.to_string()),
            },
        );
    }

    // ==================== Chapter 事件 ====================

    pub fn emit_chapter_created<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        chapter_id: &str,
        title: Option<&str>,
    ) {
        Self::emit_event(
            app,
            SyncEvent::ChapterCreated {
                story_id: story_id.to_string(),
                chapter_id: chapter_id.to_string(),
                title: title.map(|s| s.to_string()),
            },
        );
    }

    pub fn emit_chapter_updated<R: Runtime>(
        app: &AppHandle<R>,
        chapter_id: &str,
        title: Option<&str>,
        story_id: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::ChapterUpdated {
                story_id: story_id.to_string(),
                chapter_id: chapter_id.to_string(),
                title: title.map(|s| s.to_string()),
            },
        );
    }

    pub fn emit_chapter_deleted<R: Runtime>(app: &AppHandle<R>, chapter_id: &str, story_id: &str) {
        Self::emit_event(
            app,
            SyncEvent::ChapterDeleted {
                story_id: story_id.to_string(),
                chapter_id: chapter_id.to_string(),
            },
        );
    }

    // ==================== World Building 事件 ====================

    pub fn emit_world_building_updated<R: Runtime>(app: &AppHandle<R>, story_id: &str) {
        Self::emit_event(
            app,
            SyncEvent::WorldBuildingUpdated {
                story_id: story_id.to_string(),
            },
        );
    }

    // ==================== 角色关系事件 ====================

    pub fn emit_character_relationships_updated<R: Runtime>(app: &AppHandle<R>, story_id: &str) {
        Self::emit_event(
            app,
            SyncEvent::CharacterRelationshipsUpdated {
                story_id: story_id.to_string(),
            },
        );
    }

    // ==================== Payoff Ledger 事件 ====================

    pub fn emit_payoff_ledger_updated<R: Runtime>(app: &AppHandle<R>, story_id: &str) {
        Self::emit_event(
            app,
            SyncEvent::PayoffLedgerUpdated {
                story_id: story_id.to_string(),
            },
        );
    }

    // ==================== Ingestion 事件 ====================

    pub fn emit_ingestion_completed<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        resource_type: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::IngestionCompleted {
                story_id: story_id.to_string(),
                resource_type: resource_type.to_string(),
            },
        );
    }

    // ==================== 批量刷新事件 ====================

    pub fn emit_data_refresh<R: Runtime>(
        app: &AppHandle<R>,
        story_id: Option<&str>,
        resource_type: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::DataRefresh {
                story_id: story_id.map(|s| s.to_string()),
                resource_type: resource_type.to_string(),
            },
        );
    }

    pub fn emit_subscription_changed<R: Runtime>(app: &AppHandle<R>, user_id: &str, tier: &str) {
        Self::emit_event(
            app,
            SyncEvent::SubscriptionChanged {
                user_id: user_id.to_string(),
                tier: tier.to_string(),
            },
        );
    }

    pub fn emit_world_building_created<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        world_building_id: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::WorldBuildingCreated {
                story_id: story_id.to_string(),
                world_building_id: world_building_id.to_string(),
            },
        );
    }

    pub fn emit_world_building_deleted<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        world_building_id: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::WorldBuildingDeleted {
                story_id: story_id.to_string(),
                world_building_id: world_building_id.to_string(),
            },
        );
    }

    pub fn emit_style_dna_updated<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        style_dna_id: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::StyleDnaUpdated {
                story_id: story_id.to_string(),
                style_dna_id: style_dna_id.to_string(),
            },
        );
    }

    pub fn emit_task_created<R: Runtime>(app: &AppHandle<R>, task_id: &str, name: &str) {
        Self::emit_event(
            app,
            SyncEvent::TaskCreated {
                task_id: task_id.to_string(),
                name: name.to_string(),
            },
        );
    }

    pub fn emit_task_updated<R: Runtime>(app: &AppHandle<R>, task_id: &str, status: &str) {
        Self::emit_event(
            app,
            SyncEvent::TaskUpdated {
                task_id: task_id.to_string(),
                status: status.to_string(),
            },
        );
    }

    pub fn emit_task_completed<R: Runtime>(app: &AppHandle<R>, task_id: &str, success: bool) {
        Self::emit_event(
            app,
            SyncEvent::TaskCompleted {
                task_id: task_id.to_string(),
                success,
            },
        );
    }

    pub fn emit_annotation_created<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        annotation_id: &str,
        scene_id: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::AnnotationCreated {
                story_id: story_id.to_string(),
                annotation_id: annotation_id.to_string(),
                scene_id: scene_id.to_string(),
            },
        );
    }

    pub fn emit_annotation_resolved<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        annotation_id: &str,
        scene_id: &str,
    ) {
        Self::emit_event(
            app,
            SyncEvent::AnnotationResolved {
                story_id: story_id.to_string(),
                annotation_id: annotation_id.to_string(),
                scene_id: scene_id.to_string(),
            },
        );
    }

    pub fn emit_payoff_overdue<R: Runtime>(
        app: &AppHandle<R>,
        story_id: &str,
        items: &[crate::creative_engine::payoff_ledger::PayoffLedgerItem],
    ) {
        let titles: Vec<String> = items.iter().map(|i| i.title.clone()).collect();
        Self::emit_event(
            app,
            SyncEvent::PayoffOverdue {
                story_id: story_id.to_string(),
                count: items.len(),
                item_titles: titles,
            },
        );
    }
}
