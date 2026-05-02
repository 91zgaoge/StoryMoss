//! 统一叙事元素模型
//!
//! 生产表（stories/characters/scenes）和参考表（reference_books/reference_characters/reference_scenes）
//! 共享同一套数据结构，通过 ElementSource 区分数据来源。

use serde::{Deserialize, Serialize};
// use chrono::{DateTime, Local};

/// 叙事元素类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementType {
    StoryMeta,      // 故事元信息
    WorldBuilding,  // 世界观
    Character,      // 角色
    Scene,          // 场景
    Outline,        // 大纲
    Relationship,   // 角色关系
    Foreshadowing,  // 伏笔
    PlotPoint,      // 情节点
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementSource {
    Generated,      // AI生成（Bootstrap/创世）
    Extracted,      // 从文本提取（拆书）
    UserCreated,    // 用户手动创建
    Imported,       // 从外部导入
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
    pub id: String,
    pub story_id: String,
    pub name: String,
    pub role_type: String,           // 主角/反派/导师/盟友/爱情线...
    pub personality: String,
    pub background: String,
    pub goals: String,
    pub fears: String,
    pub appearance: String,
    pub gender: String,
    pub age: i32,
    pub relationships: Vec<CharacterRelationship>,
    pub importance_score: f32,       // 1-10
    pub source: ElementSource,
    pub source_ref_id: Option<String>, // 关联外部ID（如拆书的book_id）
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterRelationship {
    pub target_name: String,
    pub relation_type: String,       // 朋友/敌人/恋人/师徒...
    pub description: Option<String>,
}

// ==================== 统一场景模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneElement {
    pub id: String,
    pub story_id: String,
    pub sequence_number: i32,
    pub title: String,
    pub summary: String,
    pub dramatic_goal: String,
    pub external_pressure: String,
    pub conflict_type: String,       // man_vs_man | man_vs_self | ...
    pub characters_present: Vec<String>,
    pub setting_location: String,
    pub setting_time: String,
    pub content: Option<String>,     // 正文内容（可选）
    pub source: ElementSource,
    pub source_ref_id: Option<String>,
}

// ==================== 统一世界观模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBuildingElement {
    pub id: String,
    pub story_id: String,
    pub concept: String,
    pub rules: Vec<WorldRule>,
    pub history: String,
    pub key_locations: Vec<String>,
    pub power_system: String,
    pub source: ElementSource,
    pub source_ref_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldRule {
    pub name: String,
    pub description: String,
    pub rule_type: String,           // physical | magic | social | historical...
    pub importance: i32,             // 1-10
}

// ==================== 统一大纲模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineElement {
    pub id: String,
    pub story_id: String,
    pub acts: Vec<OutlineAct>,
    pub total_scenes_estimate: i32,
    pub source: ElementSource,
    pub source_ref_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlineAct {
    pub act_number: i32,
    pub title: String,
    pub summary: String,
    pub key_plot_points: Vec<String>,
    pub estimated_scenes: i32,
}

// ==================== 统一伏笔模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeshadowingElement {
    pub id: String,
    pub story_id: String,
    pub content: String,
    pub importance: i32,             // 1-10
    pub target_act: i32,
    pub hint_style: String,          // 环境隐喻/对话暗示/物品象征...
    pub setup_scene_id: Option<String>,
    pub payoff_scene_id: Option<String>,
    pub status: ForeshadowingStatus,
    pub source: ElementSource,
    pub source_ref_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForeshadowingStatus {
    Setup,      // 已埋设
    Payoff,     // 已回收
    Abandoned,  // 已放弃
    Pending,    // 待处理
}

// ==================== 故事元信息模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryMetaElement {
    pub id: String,
    pub title: String,
    pub description: String,
    pub genre: String,
    pub tone: String,
    pub pacing: String,
    pub themes: Vec<String>,
    pub target_length: String,
    pub source: ElementSource,
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
