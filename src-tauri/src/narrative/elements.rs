#![allow(dead_code)]
//! 统一叙事元素模型
//!
//! 注意：具体类型定义已迁移到
//! `domain::narrative_elements`，本模块保留为向后兼容的
//! 重新导出层，以便现有调用方逐步迁移。新代码应优先使用
//! `crate::domain::narrative_elements`。

pub use crate::domain::narrative_elements::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_type_display() {
        assert_eq!(format!("{}", ElementType::StoryMeta), "故事元信息");
        assert_eq!(format!("{}", ElementType::Character), "角色");
        assert_eq!(format!("{}", ElementType::Scene), "场景");
    }

    #[test]
    fn test_element_source_display() {
        assert_eq!(format!("{}", ElementSource::Generated), "AI生成");
        assert_eq!(format!("{}", ElementSource::Extracted), "文本提取");
        assert_eq!(format!("{}", ElementSource::UserCreated), "用户创建");
    }

    #[test]
    fn test_character_element_defaults() {
        let json = r#"{"name": "主角", "role_type": " protagonist", "personality": "勇敢", "background": "农家少年", "goals": "复仇", "fears": "失去亲人", "appearance": "黑发", "gender": "男", "age": 20}"#;
        let character: CharacterElement = serde_json::from_str(json).unwrap();
        assert_eq!(character.name, "主角");
        assert_eq!(character.id, ""); // serde(default)
        assert_eq!(character.story_id, ""); // serde(default)
        assert!(character.relationships.is_empty()); // serde(default)
        assert_eq!(character.importance_score, 0.0); // serde(default)
        assert_eq!(character.source, ElementSource::Generated); // serde(default)
    }

    #[test]
    fn test_scene_element_defaults() {
        let json = r#"{"sequence_number": 1, "title": "开篇", "summary": "故事开始", "dramatic_goal": "引入主角", "external_pressure": "无", "conflict_type": "man_vs_fate", "setting_location": "村庄", "setting_time": "清晨"}"#;
        let scene: SceneElement = serde_json::from_str(json).unwrap();
        assert_eq!(scene.title, "开篇");
        assert!(scene.characters_present.is_empty());
        assert_eq!(scene.source, ElementSource::Generated);
    }

    #[test]
    fn test_world_building_element_defaults() {
        let json = r#"{"concept": "修仙世界", "history": "万年历史"}"#;
        let wb: WorldBuildingElement = serde_json::from_str(json).unwrap();
        assert_eq!(wb.concept, "修仙世界");
        assert!(wb.rules.is_empty());
        assert!(wb.key_locations.is_empty());
        assert_eq!(wb.power_system, ""); // serde(default)
    }

    #[test]
    fn test_outline_element_serialization() {
        let outline = OutlineElement {
            id: "ol_1".to_string(),
            story_id: "s1".to_string(),
            acts: vec![OutlineAct {
                act_number: 1,
                title: "第一幕".to_string(),
                summary: "引入".to_string(),
                key_plot_points: vec!["事件A".to_string()],
                estimated_scenes: 5,
            }],
            total_scenes_estimate: 15,
            source: ElementSource::Generated,
            source_ref_id: None,
        };
        let json = serde_json::to_string(&outline).unwrap();
        let deserialized: OutlineElement = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.acts.len(), 1);
        assert_eq!(deserialized.acts[0].title, "第一幕");
    }

    #[test]
    fn test_foreshadowing_status_default() {
        let status: ForeshadowingStatus = Default::default();
        assert_eq!(status, ForeshadowingStatus::Setup);
    }

    #[test]
    fn test_narrative_bundle_builder() {
        let bundle = NarrativeBundle::new()
            .with_story_meta(StoryMetaElement {
                id: "sm1".to_string(),
                title: "测试小说".to_string(),
                description: "描述".to_string(),
                genre: "科幻".to_string(),
                genre_profile_ids: vec!["scifi".to_string()],
                tone: "热血".to_string(),
                pacing: "快节奏".to_string(),
                themes: vec!["成长".to_string()],
                target_length: "长篇".to_string(),
                protagonist_name: None,
                protagonist_desire: None,
                protagonist_wound: None,
                core_conflict: None,
                world_one_liner: None,
                survival_stakes: None,
                source: ElementSource::Generated,
                source_ref_id: None,
            })
            .add_character(CharacterElement {
                id: "c1".to_string(),
                story_id: "s1".to_string(),
                name: "主角".to_string(),
                role_type: "主角".to_string(),
                personality: "勇敢".to_string(),
                background: "农家".to_string(),
                goals: "复仇".to_string(),
                fears: "失去".to_string(),
                appearance: "黑发".to_string(),
                gender: "男".to_string(),
                age: 20,
                relationships: vec![],
                importance_score: 9.0,
                source: ElementSource::Generated,
                source_ref_id: None,
                status: ElementStatus::Active,
            });
        assert!(bundle.story_meta.is_some());
        assert_eq!(bundle.characters.len(), 1);
        assert_eq!(bundle.characters[0].name, "主角");
    }
}
