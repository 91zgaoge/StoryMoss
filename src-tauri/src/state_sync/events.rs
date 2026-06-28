//! 跨窗口状态同步事件定义
//!
//! 所有数据变更操作完成后发射这些事件，前后台窗口监听并自动刷新对应数据。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 同步事件类型
///
/// 命名规范:
/// - `[Resource]Created`: 资源创建（前台/后台需要添加新条目）
/// - `[Resource]Updated`: 资源更新（前台/后台需要刷新现有条目）
/// - `[Resource]Deleted`: 资源删除（前台/后台需要移除条目）
/// - `[Resource]Selected`: 资源选择（前台/后台需要切换当前焦点）
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(rename_all = "camelCase")]
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
        /// Phase 4: 标记内容是否变更，供前端决定是否刷新编辑器正文
        content_changed: bool,
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
    /// v0.23.1: 章节 commit（含 projections）完成后发射，用于通知前后台刷新
    /// read-model。
    ChapterCommitted {
        story_id: String,
        chapter_id: String,
        chapter_number: i32,
        projection_status: HashMap<String, String>,
    },

    // === World Building 事件 ===
    WorldBuildingUpdated {
        story_id: String,
    },
    WorldBuildingCreated {
        story_id: String,
        world_building_id: String,
    },
    WorldBuildingDeleted {
        story_id: String,
        world_building_id: String,
    },

    // === Style DNA 事件 ===
    StyleDnaUpdated {
        story_id: String,
        style_dna_id: String,
    },

    // === Task 事件 ===
    TaskCreated {
        task_id: String,
        name: String,
    },
    TaskUpdated {
        task_id: String,
        status: String,
    },
    TaskCompleted {
        task_id: String,
        success: bool,
    },

    // === Annotation 事件 ===
    AnnotationCreated {
        story_id: String,
        annotation_id: String,
        scene_id: String,
    },
    AnnotationResolved {
        story_id: String,
        annotation_id: String,
        scene_id: String,
    },

    // === 角色关系事件 ===
    CharacterRelationshipsUpdated {
        story_id: String,
    },

    // === Payoff Ledger 事件 ===
    PayoffLedgerUpdated {
        story_id: String,
    },

    // === Ingestion 事件 ===
    IngestionCompleted {
        story_id: String,
        resource_type: String,
    },

    // === 元数据刷新事件 (批量刷新信号) ===
    DataRefresh {
        story_id: Option<String>,
        resource_type: String, // "stories" | "characters" | "scenes" | "chapters" | "all"
    },

    // === 订阅变更事件 ===
    SubscriptionChanged {
        user_id: String,
        tier: String,
    },

    // === 伏笔逾期事件 ===
    PayoffOverdue {
        story_id: String,
        count: usize,
        item_titles: Vec<String>,
    },

    /// P1-4: 异步审计发现 high 严重性问题时，请求前端提示用户是否自动修订。
    /// 保持用户控制权——不静默改文，仅提示。
    AuditRewriteSuggested {
        story_id: String,
        scene_id: Option<String>,
        chapter_id: Option<String>,
        /// high 严重性问题描述摘要（用于前端弹窗展示）
        issues: Vec<String>,
    },

    /// v0.23 TriShot BGP-2：后台自动改写器已自动修正高严重度问题并替换正文。
    /// 正文已写入修订历史，用户可撤销。前端展示 toast「AI 已修正 N
    /// 处问题，可撤销」。
    ContentAutoRevised {
        story_id: String,
        scene_id: Option<String>,
        chapter_id: Option<String>,
        /// 本次自动修正的问题数量
        revision_count: usize,
        /// 修正摘要（用于前端展示）
        summary: String,
    },

    /// v0.23 TriShot BGP-2：后台质检发现低严重度问题，生成修订建议供用户审阅。
    /// 不自动改文，前端展示「AI 有 N 条建议」审阅面板（采纳/忽略）。
    RevisionSuggested {
        story_id: String,
        scene_id: Option<String>,
        chapter_id: Option<String>,
        /// 建议列表（每条含维度、描述、建议改法）
        suggestions: Vec<String>,
    },
}

impl SyncEvent {
    /// 获取事件对应的资源类型标识
    #[allow(dead_code)]
    pub fn resource_type(&self) -> &str {
        match self {
            SyncEvent::StoryCreated { .. }
            | SyncEvent::StoryUpdated { .. }
            | SyncEvent::StoryDeleted { .. }
            | SyncEvent::StorySelected { .. } => "stories",
            SyncEvent::CharacterCreated { .. }
            | SyncEvent::CharacterUpdated { .. }
            | SyncEvent::CharacterDeleted { .. } => "characters",
            SyncEvent::SceneCreated { .. }
            | SyncEvent::SceneUpdated { .. }
            | SyncEvent::SceneDeleted { .. }
            | SyncEvent::SceneSelected { .. } => "scenes",
            SyncEvent::ChapterCreated { .. }
            | SyncEvent::ChapterUpdated { .. }
            | SyncEvent::ChapterDeleted { .. }
            | SyncEvent::ChapterCommitted { .. } => "chapters",
            SyncEvent::WorldBuildingUpdated { .. }
            | SyncEvent::WorldBuildingCreated { .. }
            | SyncEvent::WorldBuildingDeleted { .. } => "worldBuilding",
            SyncEvent::StyleDnaUpdated { .. } => "styleDna",
            SyncEvent::TaskCreated { .. }
            | SyncEvent::TaskUpdated { .. }
            | SyncEvent::TaskCompleted { .. } => "tasks",
            SyncEvent::AnnotationCreated { .. } | SyncEvent::AnnotationResolved { .. } => {
                "annotations"
            }
            SyncEvent::CharacterRelationshipsUpdated { .. } => "characterRelationships",
            SyncEvent::PayoffLedgerUpdated { .. } => "payoffLedger",
            SyncEvent::IngestionCompleted { .. } => "ingestion",
            SyncEvent::DataRefresh { resource_type, .. } => resource_type.as_str(),
            SyncEvent::SubscriptionChanged { .. } => "subscription",
            SyncEvent::PayoffOverdue { .. } => "payoffOverdue",
            SyncEvent::AuditRewriteSuggested { .. } => "auditRewrite",
            SyncEvent::ContentAutoRevised { .. } => "contentAutoRevised",
            SyncEvent::RevisionSuggested { .. } => "revisionSuggested",
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
            SyncEvent::ChapterCommitted { story_id, .. } => Some(story_id),
            SyncEvent::WorldBuildingUpdated { story_id, .. } => Some(story_id),
            SyncEvent::WorldBuildingCreated { story_id, .. } => Some(story_id),
            SyncEvent::WorldBuildingDeleted { story_id, .. } => Some(story_id),
            SyncEvent::StyleDnaUpdated { story_id, .. } => Some(story_id),
            SyncEvent::TaskCreated { .. } => None,
            SyncEvent::TaskUpdated { .. } => None,
            SyncEvent::TaskCompleted { .. } => None,
            SyncEvent::AnnotationCreated { story_id, .. } => Some(story_id),
            SyncEvent::AnnotationResolved { story_id, .. } => Some(story_id),
            SyncEvent::CharacterRelationshipsUpdated { story_id, .. } => Some(story_id),
            SyncEvent::PayoffLedgerUpdated { story_id, .. } => Some(story_id),
            SyncEvent::IngestionCompleted { story_id, .. } => Some(story_id),
            SyncEvent::DataRefresh { story_id, .. } => story_id.as_ref(),
            SyncEvent::SubscriptionChanged { .. } => None,
            SyncEvent::PayoffOverdue { story_id, .. } => Some(story_id),
            SyncEvent::AuditRewriteSuggested { story_id, .. } => Some(story_id),
            SyncEvent::ContentAutoRevised { story_id, .. } => Some(story_id),
            SyncEvent::RevisionSuggested { story_id, .. } => Some(story_id),
        }
    }
}

// =============================================================================
// Phase 1.1: TypeScript 绑定导出测试
// =============================================================================

#[cfg(test)]
mod ts_export_tests {
    use std::path::PathBuf;

    use super::*;

    /// 将 SyncEvent 导出为 TypeScript 类型定义。
    /// 运行 `cargo test ts_export_tests -- --nocapture` 即可更新前端绑定文件。
    #[test]
    fn export_sync_event_types() {
        let export_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src-frontend/src/generated");

        // 确保目标目录存在
        std::fs::create_dir_all(&export_dir).expect("创建 generated 目录失败");

        // 导出 SyncEvent（serde tag/content 模式会自动生成 discriminated union）
        SyncEvent::export_all_to(&export_dir).expect("导出 SyncEvent 失败");

        // 验证文件已生成
        let expected_path = export_dir.join("SyncEvent.ts");
        assert!(
            expected_path.exists(),
            "SyncEvent.ts 未生成到 {:?}",
            expected_path
        );

        println!("✅ TypeScript 绑定已导出到: {:?}", export_dir);
    }
}

#[cfg(test)]
mod trishot_event_tests {
    use super::*;

    #[test]
    fn test_content_auto_revised_serialization() {
        // v0.23 BGP-2：ContentAutoRevised 序列化为 tag/content 模式
        let event = SyncEvent::ContentAutoRevised {
            story_id: "s1".to_string(),
            scene_id: Some("sc1".to_string()),
            chapter_id: None,
            revision_count: 3,
            summary: "修正逻辑连贯性问题".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains("\"type\":\"contentAutoRevised\""),
            "json={}",
            json
        );
        assert!(json.contains("\"revision_count\":3"), "json={}", json);
        assert_eq!(event.story_id(), Some(&"s1".to_string()));
        assert_eq!(event.resource_type(), "contentAutoRevised");
    }

    #[test]
    fn test_revision_suggested_serialization() {
        // v0.23 BGP-2：RevisionSuggested 序列化为 tag/content 模式
        let event = SyncEvent::RevisionSuggested {
            story_id: "s2".to_string(),
            scene_id: None,
            chapter_id: Some("ch1".to_string()),
            suggestions: vec!["节奏偏慢".to_string(), "对话比例过高".to_string()],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains("\"type\":\"revisionSuggested\""),
            "json={}",
            json
        );
        assert!(json.contains("\"suggestions\""), "json={}", json);
        assert_eq!(event.story_id(), Some(&"s2".to_string()));
        assert_eq!(event.resource_type(), "revisionSuggested");
    }

    #[test]
    fn test_chapter_committed_serialization() {
        // v0.23.1: ChapterCommitted 序列化为 tag/content 模式并携带 projection_status
        let mut projection_status = HashMap::new();
        projection_status.insert("vector".to_string(), "success".to_string());
        projection_status.insert("kg".to_string(), "success".to_string());
        let event = SyncEvent::ChapterCommitted {
            story_id: "s1".to_string(),
            chapter_id: "ch1".to_string(),
            chapter_number: 3,
            projection_status,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains("\"type\":\"chapterCommitted\""),
            "json={}",
            json
        );
        assert!(json.contains("\"chapter_number\":3"), "json={}", json);
        assert!(json.contains("\"projection_status\""), "json={}", json);
        assert_eq!(event.story_id(), Some(&"s1".to_string()));
        assert_eq!(event.resource_type(), "chapters");
    }
}
