//! 状态同步服务
//! 
//! 提供便捷的静态方法，在所有数据变更操作后发射同步事件。
//! 前后台窗口通过监听 `sync-event` 频道实现自动刷新。

use tauri::{AppHandle, Emitter};
use super::events::SyncEvent;

/// 状态同步发射器
/// 
/// 所有方法都是静态方法，无需实例化。直接通过 AppHandle 发射事件到所有窗口。
pub struct StateSync;

impl StateSync {
    /// 发射同步事件到所有窗口
    fn emit_event(app: &AppHandle, event: SyncEvent) {
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
            SyncEvent::DataRefresh { .. } => "data-refresh",
        };

        // 发射到通用频道 `sync-event`
        if let Err(e) = app.emit("sync-event", &event) {
            log::warn!("[StateSync] Failed to emit sync-event: {}", e);
        }

        // 同时发射到具体事件频道（便于前端单独监听）
        if let Err(e) = app.emit(event_name, &event) {
            log::warn!("[StateSync] Failed to emit {}: {}", event_name, e);
        }

        log::debug!("[StateSync] Emitted {} for {:?}", event_name, event.story_id());
    }

    // ==================== Story 事件 ====================

    pub fn emit_story_created(app: &AppHandle, story_id: &str, title: &str) {
        Self::emit_event(app, SyncEvent::StoryCreated {
            story_id: story_id.to_string(),
            title: Some(title.to_string()),
        });
    }

    pub fn emit_story_updated(app: &AppHandle, story_id: &str, title: Option<&str>) {
        Self::emit_event(app, SyncEvent::StoryUpdated {
            story_id: story_id.to_string(),
            title: title.map(|s| s.to_string()),
        });
    }

    pub fn emit_story_deleted(app: &AppHandle, story_id: &str) {
        Self::emit_event(app, SyncEvent::StoryDeleted {
            story_id: story_id.to_string(),
        });
    }

    pub fn emit_story_selected(app: &AppHandle, story_id: &str, title: Option<&str>) {
        Self::emit_event(app, SyncEvent::StorySelected {
            story_id: story_id.to_string(),
            title: title.map(|s| s.to_string()),
        });
    }

    // ==================== Character 事件 ====================

    pub fn emit_character_created(app: &AppHandle, story_id: &str, character_id: &str, name: &str) {
        Self::emit_event(app, SyncEvent::CharacterCreated {
            story_id: story_id.to_string(),
            character_id: character_id.to_string(),
            name: name.to_string(),
        });
    }

    pub fn emit_character_updated(app: &AppHandle, character_id: &str, name: Option<&str>, story_id: &str) {
        Self::emit_event(app, SyncEvent::CharacterUpdated {
            story_id: story_id.to_string(),
            character_id: character_id.to_string(),
            name: name.map(|s| s.to_string()),
        });
    }

    pub fn emit_character_deleted(app: &AppHandle, character_id: &str, story_id: &str) {
        Self::emit_event(app, SyncEvent::CharacterDeleted {
            story_id: story_id.to_string(),
            character_id: character_id.to_string(),
        });
    }

    // ==================== Scene 事件 ====================

    pub fn emit_scene_created(app: &AppHandle, story_id: &str, scene_id: &str, title: Option<&str>) {
        Self::emit_event(app, SyncEvent::SceneCreated {
            story_id: story_id.to_string(),
            scene_id: scene_id.to_string(),
            title: title.map(|s| s.to_string()),
        });
    }

    pub fn emit_scene_updated(app: &AppHandle, story_id: &str, scene_id: &str, title: Option<&str>) {
        Self::emit_event(app, SyncEvent::SceneUpdated {
            story_id: story_id.to_string(),
            scene_id: scene_id.to_string(),
            title: title.map(|s| s.to_string()),
        });
    }

    pub fn emit_scene_deleted(app: &AppHandle, story_id: &str, scene_id: &str) {
        Self::emit_event(app, SyncEvent::SceneDeleted {
            story_id: story_id.to_string(),
            scene_id: scene_id.to_string(),
        });
    }

    pub fn emit_scene_selected(app: &AppHandle, story_id: &str, scene_id: &str, title: Option<&str>) {
        Self::emit_event(app, SyncEvent::SceneSelected {
            story_id: story_id.to_string(),
            scene_id: scene_id.to_string(),
            title: title.map(|s| s.to_string()),
        });
    }

    // ==================== Chapter 事件 ====================

    pub fn emit_chapter_created(app: &AppHandle, story_id: &str, chapter_id: &str, title: Option<&str>) {
        Self::emit_event(app, SyncEvent::ChapterCreated {
            story_id: story_id.to_string(),
            chapter_id: chapter_id.to_string(),
            title: title.map(|s| s.to_string()),
        });
    }

    pub fn emit_chapter_updated(app: &AppHandle, chapter_id: &str, title: Option<&str>, story_id: &str) {
        Self::emit_event(app, SyncEvent::ChapterUpdated {
            story_id: story_id.to_string(),
            chapter_id: chapter_id.to_string(),
            title: title.map(|s| s.to_string()),
        });
    }

    pub fn emit_chapter_deleted(app: &AppHandle, chapter_id: &str, story_id: &str) {
        Self::emit_event(app, SyncEvent::ChapterDeleted {
            story_id: story_id.to_string(),
            chapter_id: chapter_id.to_string(),
        });
    }

    // ==================== World Building 事件 ====================

    pub fn emit_world_building_updated(app: &AppHandle, story_id: &str) {
        Self::emit_event(app, SyncEvent::WorldBuildingUpdated {
            story_id: story_id.to_string(),
        });
    }

    // ==================== 批量刷新事件 ====================

    pub fn emit_data_refresh(app: &AppHandle, story_id: Option<&str>, resource_type: &str) {
        Self::emit_event(app, SyncEvent::DataRefresh {
            story_id: story_id.map(|s| s.to_string()),
            resource_type: resource_type.to_string(),
        });
    }
}
