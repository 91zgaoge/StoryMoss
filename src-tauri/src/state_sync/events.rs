//! 跨窗口状态同步事件定义
//! 
//! 所有数据变更操作完成后发射这些事件，前后台窗口监听并自动刷新对应数据。

use serde::{Deserialize, Serialize};

/// 同步事件类型
/// 
/// 命名规范:
/// - `[Resource]Created`: 资源创建（前台/后台需要添加新条目）
/// - `[Resource]Updated`: 资源更新（前台/后台需要刷新现有条目）
/// - `[Resource]Deleted`: 资源删除（前台/后台需要移除条目）
/// - `[Resource]Selected`: 资源选择（前台/后台需要切换当前焦点）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "payload")]
pub enum SyncEvent {
    // === Story 事件 ===
    StoryCreated {
        story_id: String,
        title: Option<String>,
    },
    StoryUpdated {
        story_id: String,
        title: Option<String>,
    },
    StoryDeleted {
        story_id: String,
    },
    StorySelected {
        story_id: String,
        title: Option<String>,
    },

    // === Character 事件 ===
    CharacterCreated {
        story_id: String,
        character_id: String,
        name: String,
    },
    CharacterUpdated {
        story_id: String,
        character_id: String,
        name: Option<String>,
    },
    CharacterDeleted {
        story_id: String,
        character_id: String,
    },

    // === Scene 事件 (Chapter ↔ Scene 双向映射) ===
    SceneCreated {
        story_id: String,
        scene_id: String,
        title: Option<String>,
    },
    SceneUpdated {
        story_id: String,
        scene_id: String,
        title: Option<String>,
    },
    SceneDeleted {
        story_id: String,
        scene_id: String,
    },
    SceneSelected {
        story_id: String,
        scene_id: String,
        title: Option<String>,
    },

    // === Chapter 事件 ===
    ChapterCreated {
        story_id: String,
        chapter_id: String,
        title: Option<String>,
    },
    ChapterUpdated {
        story_id: String,
        chapter_id: String,
        title: Option<String>,
    },
    ChapterDeleted {
        story_id: String,
        chapter_id: String,
    },

    // === World Building 事件 ===
    WorldBuildingUpdated {
        story_id: String,
    },

    // === 元数据刷新事件 (批量刷新信号) ===
    DataRefresh {
        story_id: Option<String>,
        resource_type: String, // "stories" | "characters" | "scenes" | "chapters" | "all"
    },
}

impl SyncEvent {
    /// 获取事件对应的资源类型标识
    #[allow(dead_code)]
    pub fn resource_type(&self) -> &str {
        match self {
            SyncEvent::StoryCreated { .. } |
            SyncEvent::StoryUpdated { .. } |
            SyncEvent::StoryDeleted { .. } |
            SyncEvent::StorySelected { .. } => "stories",
            SyncEvent::CharacterCreated { .. } |
            SyncEvent::CharacterUpdated { .. } |
            SyncEvent::CharacterDeleted { .. } => "characters",
            SyncEvent::SceneCreated { .. } |
            SyncEvent::SceneUpdated { .. } |
            SyncEvent::SceneDeleted { .. } |
            SyncEvent::SceneSelected { .. } => "scenes",
            SyncEvent::ChapterCreated { .. } |
            SyncEvent::ChapterUpdated { .. } |
            SyncEvent::ChapterDeleted { .. } => "chapters",
            SyncEvent::WorldBuildingUpdated { .. } => "worldBuilding",
            SyncEvent::DataRefresh { resource_type, .. } => resource_type.as_str(),
        }
    }

    /// 获取关联的故事ID
    pub fn story_id(&self) -> Option<&String> {
        match self {
            SyncEvent::StoryCreated { story_id, .. } => Some(story_id),
            SyncEvent::StoryUpdated { story_id, .. } => Some(story_id),
            SyncEvent::StoryDeleted { story_id, .. } => Some(story_id),
            SyncEvent::StorySelected { story_id, .. } => Some(story_id),
            SyncEvent::CharacterCreated { story_id, .. } => Some(story_id),
            SyncEvent::CharacterUpdated { story_id, .. } => Some(story_id),
            SyncEvent::CharacterDeleted { story_id, .. } => Some(story_id),
            SyncEvent::SceneCreated { story_id, .. } => Some(story_id),
            SyncEvent::SceneUpdated { story_id, .. } => Some(story_id),
            SyncEvent::SceneDeleted { story_id, .. } => Some(story_id),
            SyncEvent::SceneSelected { story_id, .. } => Some(story_id),
            SyncEvent::ChapterCreated { story_id, .. } => Some(story_id),
            SyncEvent::ChapterUpdated { story_id, .. } => Some(story_id),
            SyncEvent::ChapterDeleted { story_id, .. } => Some(story_id),
            SyncEvent::WorldBuildingUpdated { story_id, .. } => Some(story_id),
            SyncEvent::DataRefresh { story_id, .. } => story_id.as_ref(),
        }
    }
}
