#![allow(dead_code)]
//! 统一叙事元素模型（Domain 层）
//!
//! 原位于 `narrative::elements`，为切断 `db -> narrative` 的循环依赖而下放到中性的
//! `domain` 模块。本模块只包含纯数据结构，不依赖任何业务模块。
//!
//! 生产表（stories/characters/scenes）和参考表（reference_books/
//! reference_characters/reference_scenes）共享同一套数据结构，通过
//! ElementSource 区分数据来源。

use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role_type: String, // 主角/反派/导师/盟友/爱情线...
    #[serde(default)]
    pub personality: String,
    #[serde(default)]
    pub background: String,
    #[serde(default)]
    pub goals: String,
    #[serde(default)]
    pub fears: String,
    #[serde(default)]
    pub appearance: String,
    #[serde(default)]
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
    #[serde(default)]
    pub target_name: String,
    #[serde(default)]
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
    // LitSeg: 叙事分析字段（从 conflict_type + emotional_tone 推导）
    #[serde(default)]
    pub narrative_intensity: f32, // 0.0-1.0
    #[serde(default)]
    pub narrative_sentiment: f32, // -1.0 ~ +1.0
    #[serde(default)]
    pub narrative_event_types: Vec<String>, // 关键事件归类
    #[serde(default)]
    pub act_number: i32, // 所属幕（pipeline 后填充）
    #[serde(default)]
    pub position_in_act: f32, // 在幕中的位置 0.0-1.0
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Culture {
    pub name: String,
    pub description: String,
    pub customs: Vec<String>,
    pub values: Vec<String>,
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
    #[serde(default)]
    pub genre_profile_ids: Vec<String>,
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
