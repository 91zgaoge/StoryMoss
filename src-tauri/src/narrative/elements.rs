//! 统一叙事元素模型
//!
//! 生产表（stories/characters/scenes）和参考表（reference_books/
//! reference_characters/reference_scenes） 共享同一套数据结构，通过
//! ElementSource 区分数据来源。

use serde::{Deserialize, Serialize};
// use chrono::{DateTime, Local};

/// 叙事元素类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    StoryMeta,     // 故事元信息
    WorldBuilding, // 世界观
    Character,     // 角色
    Scene,         // 场景
    Outline,       // 大纲
    Relationship,  // 角色关系
    Foreshadowing, // 伏笔
    PlotPoint,     // 情节点
}

impl std::fmt::Display for ElementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ElementType::StoryMeta => "故事元信息",
            ElementType::WorldBuilding => "世界观",
            ElementType::Character => "角色",
            ElementType::Scene => "场景",
            ElementType::Outline => "大纲",
            ElementType::Relationship => "角色关系",
            ElementType::Foreshadowing => "伏笔",
            ElementType::PlotPoint => "情节点",
        };
        write!(f, "{}", s)
    }
}

/// 元素来源 — 标识数据是如何产生的
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementSource {
    #[default]
    Generated, // AI生成（Bootstrap/创世）
    Extracted,   // 从文本提取（拆书）
    UserCreated, // 用户手动创建
    Imported,    // 从外部导入
}

/// 元素状态 — 区分活跃创作元素与参考材料 (W3-B3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementStatus {
    #[default]
    Active, // 当前故事正在使用的活跃元素
    Reference, // 从拆书/参考材料导入，尚未激活
    Archived,  // 已归档，不再使用
}

impl std::fmt::Display for ElementStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ElementStatus::Active => "active",
            ElementStatus::Reference => "reference",
            ElementStatus::Archived => "archived",
        };
        write!(f, "{}", s)
    }
}

impl std::fmt::Display for ElementSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ElementSource::Generated => "AI生成",
            ElementSource::Extracted => "文本提取",
            ElementSource::UserCreated => "用户创建",
            ElementSource::Imported => "外部导入",
        };
        write!(f, "{}", s)
    }
}

// ==================== 统一角色模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterElement {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub story_id: String,
    pub name: String,
    pub role_type: String, // 主角/反派/导师/盟友/爱情线...
    pub personality: String,
    pub background: String,
    pub goals: String,
    pub fears: String,
    pub appearance: String,
    pub gender: String,
    #[serde(default)]
    pub age: i32,
    #[serde(default)]
    pub relationships: Vec<CharacterRelationship>,
    #[serde(default)]
    pub importance_score: f32, // 1-10
    #[serde(default)]
    pub source: ElementSource,
    #[serde(default)]
    pub source_ref_id: Option<String>, // 关联外部ID（如拆书的book_id）
    #[serde(default)]
    pub status: ElementStatus, // active / reference / archived (W3-B3)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterRelationship {
    pub target_name: String,
    pub relation_type: String, // 朋友/敌人/恋人/师徒...
    pub description: Option<String>,
}

// ==================== 统一场景模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneElement {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub story_id: String,
    #[serde(default)]
    pub sequence_number: i32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub dramatic_goal: String,
    #[serde(default)]
    pub external_pressure: String,
    #[serde(default)]
    pub conflict_type: String, // man_vs_man | man_vs_self | ...
    #[serde(default)]
    pub characters_present: Vec<String>,
    #[serde(default)]
    pub setting_location: String,
    #[serde(default)]
    pub setting_time: String,
    pub content: Option<String>, // 正文内容（可选）
    #[serde(default)]
    pub source: ElementSource,
    #[serde(default)]
    pub source_ref_id: Option<String>,
    #[serde(default)]
    pub status: ElementStatus,
}

// ==================== 统一世界观模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBuildingElement {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub story_id: String,
    #[serde(default)]
    pub concept: String,
    #[serde(default)]
    pub rules: Vec<WorldRule>,
    #[serde(default)]
    pub history: String,
    #[serde(default)]
    pub key_locations: Vec<String>,
    #[serde(default)]
    pub power_system: String,
    #[serde(default)]
    pub source: ElementSource,
    #[serde(default)]
    pub source_ref_id: Option<String>,
    #[serde(default)]
    pub status: ElementStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldRule {
    pub name: String,
    pub description: String,
    pub rule_type: String, // physical | magic | social | historical...
    pub importance: i32,   // 1-10
}

// ==================== 统一大纲模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineElement {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub story_id: String,
    pub acts: Vec<OutlineAct>,
    #[serde(default)]
    pub total_scenes_estimate: i32,
    #[serde(default)]
    pub source: ElementSource,
    #[serde(default)]
    pub source_ref_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineAct {
    pub act_number: i32,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub key_plot_points: Vec<String>,
    #[serde(default)]
    pub estimated_scenes: i32,
}

// ==================== 统一伏笔模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeshadowingElement {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub story_id: String,
    pub content: String,
    pub importance: i32, // 1-10
    pub target_act: i32,
    pub hint_style: String, // 环境隐喻/对话暗示/物品象征...
    pub setup_scene_id: Option<String>,
    pub payoff_scene_id: Option<String>,
    #[serde(default)]
    pub status: ForeshadowingStatus,
    #[serde(default)]
    pub source: ElementSource,
    #[serde(default)]
    pub source_ref_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForeshadowingStatus {
    #[default]
    Setup, // 已埋设
    Payoff,    // 已回收
    Abandoned, // 已放弃
    Pending,   // 待处理
}

// ==================== 故事元信息模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryMetaElement {
    #[serde(default)]
    pub id: String,
    pub title: String,
    pub description: String,
    pub genre: String,
    pub tone: String,
    pub pacing: String,
    pub themes: Vec<String>,
    pub target_length: String,
    #[serde(default)]
    pub source: ElementSource,
    #[serde(default)]
    pub source_ref_id: Option<String>,
}

// ==================== 叙事元素容器 ====================
/// 一个完整的叙事元素集合，代表一部小说的全部结构要素

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NarrativeBundle {
    pub story_meta: Option<StoryMetaElement>,
    pub world_building: Option<WorldBuildingElement>,
    pub outline: Option<OutlineElement>,
    pub characters: Vec<CharacterElement>,
    pub scenes: Vec<SceneElement>,
    pub foreshadowings: Vec<ForeshadowingElement>,
}

impl NarrativeBundle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_story_meta(mut self, meta: StoryMetaElement) -> Self {
        self.story_meta = Some(meta);
        self
    }

    pub fn with_world_building(mut self, wb: WorldBuildingElement) -> Self {
        self.world_building = Some(wb);
        self
    }

    pub fn with_outline(mut self, outline: OutlineElement) -> Self {
        self.outline = Some(outline);
        self
    }

    pub fn add_character(mut self, character: CharacterElement) -> Self {
        self.characters.push(character);
        self
    }

    pub fn add_scene(mut self, scene: SceneElement) -> Self {
        self.scenes.push(scene);
        self
    }

    pub fn add_foreshadowing(mut self, fw: ForeshadowingElement) -> Self {
        self.foreshadowings.push(fw);
        self
    }
}

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
                tone: "热血".to_string(),
                pacing: "快节奏".to_string(),
                themes: vec!["成长".to_string()],
                target_length: "长篇".to_string(),
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
