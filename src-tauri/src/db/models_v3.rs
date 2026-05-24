//! 数据模型
//! 
//! 包含场景化叙事、知识图谱、工作室配置等新模型
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};

// ==================== 通用类型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicTrait {
    #[serde(rename = "trait")]
    pub trait_name: String,
    pub confidence: f32,
}

// ==================== 场景模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub story_id: String,
    pub sequence_number: i32,
    pub title: Option<String>,
    
    // 戏剧结构
    pub dramatic_goal: Option<String>,
    pub external_pressure: Option<String>,
    pub conflict_type: Option<ConflictType>,
    
    // 角色参与
    pub characters_present: Vec<String>,
    pub character_conflicts: Vec<CharacterConflict>,
    
    // 内容
    pub content: Option<String>,
    
    // 场景设置
    pub setting_location: Option<String>,
    pub setting_time: Option<String>,
    pub setting_atmosphere: Option<String>,
    
    // 关联
    pub previous_scene_id: Option<String>,
    pub next_scene_id: Option<String>,
    
    // 结构化大纲字段
    pub execution_stage: Option<String>, // planning | outline | drafting | review | final
    pub outline_content: Option<String>,
    pub draft_content: Option<String>,
    
    // 元数据
    pub model_used: Option<String>,
    pub cost: Option<f64>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    
    // 置信度评分 (0-1)
    pub confidence_score: Option<f32>,
    
    // 风格混合覆盖
    pub style_blend_override: Option<String>,
    
    // 关联伏笔ID列表
    pub foreshadowing_ids: Option<Vec<String>>,
    
    // 关联的章节ID
    pub chapter_id: Option<String>,
}

/// 场景分隔节点
///
/// 在 1:N 架构下，Chapter 的 content 是多个 Scene 的聚合视图。
/// SceneDividerNode 标记 Scene 边界，支撑连续编辑表面上的 divider 插入/删除/重排。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneDividerNode {
    pub id: String,
    pub chapter_id: String,
    /// 在章节内的顺序位置（从 0 开始）
    pub position: i32,
    /// 该 divider 之后的 Scene ID
    pub scene_id: String,
    /// 可选标签（如 "Scene 2"、"转折"）
    pub label: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictType {
    ManVsMan,        // 人与人
    ManVsSelf,       // 人与自我
    ManVsSociety,    // 人与社会
    ManVsNature,     // 人与自然
    ManVsTechnology, // 人与科技
    ManVsFate,       // 人与命运
    ManVsSupernatural, // 人与超自然
    ManVsTime,       // 人与时间
    ManVsMorality,   // 人与道德
    ManVsIdentity,   // 人与身份
    FactionVsFaction, // 群体冲突
}

impl std::fmt::Display for ConflictType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ConflictType::ManVsMan => "人与人",
            ConflictType::ManVsSelf => "人与自我",
            ConflictType::ManVsSociety => "人与社会",
            ConflictType::ManVsNature => "人与自然",
            ConflictType::ManVsTechnology => "人与科技",
            ConflictType::ManVsFate => "人与命运",
            ConflictType::ManVsSupernatural => "人与超自然",
            ConflictType::ManVsTime => "人与时间",
            ConflictType::ManVsMorality => "人与道德",
            ConflictType::ManVsIdentity => "人与身份",
            ConflictType::FactionVsFaction => "群体冲突",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for ConflictType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "人与人" | "ManVsMan" => Ok(ConflictType::ManVsMan),
            "人与自我" | "ManVsSelf" => Ok(ConflictType::ManVsSelf),
            "人与社会" | "ManVsSociety" => Ok(ConflictType::ManVsSociety),
            "人与自然" | "ManVsNature" => Ok(ConflictType::ManVsNature),
            "人与科技" | "ManVsTechnology" => Ok(ConflictType::ManVsTechnology),
            "人与命运" | "ManVsFate" => Ok(ConflictType::ManVsFate),
            "人与超自然" | "ManVsSupernatural" => Ok(ConflictType::ManVsSupernatural),
            "人与时间" | "ManVsTime" => Ok(ConflictType::ManVsTime),
            "人与道德" | "ManVsMorality" => Ok(ConflictType::ManVsMorality),
            "人与身份" | "ManVsIdentity" => Ok(ConflictType::ManVsIdentity),
            "群体冲突" | "FactionVsFaction" => Ok(ConflictType::FactionVsFaction),
            _ => Err(format!("Unknown conflict type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterConflict {
    pub character_a_id: String,
    pub character_b_id: String,
    pub conflict_nature: String,
    pub stakes: String,
}

// ==================== 场景-角色关联模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCharacter {
    pub id: String,
    pub scene_id: String,
    pub character_id: String,
    pub character_name: Option<String>, // 冗余字段，便于显示
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCharacterAction {
    pub id: String,
    pub scene_id: String,
    pub character_id: String,
    pub action_type: String, // dialogue, action, thought, etc.
    pub content: String,
    pub created_at: DateTime<Local>,
}

// ==================== 保留配置模型 (Phase 1.4) ====================

/// 艾宾浩斯遗忘曲线配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// 衰减率 λ (0.01 架构级, 0.05 默认, 0.1 瞬态)
    pub lambda: f64,
    /// 每次访问的强化奖励
    pub reinforcement_bonus: f64,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            lambda: 0.05,
            reinforcement_bonus: 0.1,
        }
    }
}

/// 保留优先级
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetentionPriority {
    Critical,    // 关键记忆（世界观、主要角色）
    High,        // 重要记忆（次要角色、关键事件）
    Medium,      // 普通记忆
    Low,         // 可压缩记忆
    Forgotten,   // 已遗忘/可归档
}

// ==================== 场景版本模型 (新增) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneVersion {
    pub id: String,
    pub scene_id: String,
    pub version_number: i32,
    
    // 版本内容快照
    pub title: Option<String>,
    pub content: Option<String>,
    pub dramatic_goal: Option<String>,
    pub external_pressure: Option<String>,
    pub conflict_type: Option<ConflictType>,
    pub characters_present: Vec<String>,
    pub character_conflicts: Vec<CharacterConflict>,
    pub setting_location: Option<String>,
    pub setting_time: Option<String>,
    pub setting_atmosphere: Option<String>,
    
    // 版本元数据
    pub word_count: i32,
    pub change_summary: String,
    pub created_by: CreatorType,  // user/ai/system
    pub model_used: Option<String>, // AI生成时使用的模型
    pub confidence_score: Option<f32>, // AI生成置信度
    
    // 版本链 (Supersession)
    pub previous_version_id: Option<String>,
    pub superseded_by: Option<String>, // 被哪个版本取代
    
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CreatorType {
    User,
    Ai,
    System,
}

impl std::fmt::Display for CreatorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CreatorType::User => "user",
            CreatorType::Ai => "ai",
            CreatorType::System => "system",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for CreatorType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(CreatorType::User),
            "ai" => Ok(CreatorType::Ai),
            "system" => Ok(CreatorType::System),
            _ => Err(format!("Unknown creator type: {}", s)),
        }
    }
}

// ==================== 场景批注模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneAnnotation {
    pub id: String,
    pub scene_id: String,
    pub story_id: String,
    pub content: String,
    pub annotation_type: AnnotationType,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub resolved_at: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationType {
    Note,    // 普通笔记
    Todo,    // 待办事项
    Warning, // 警告/注意
    Idea,    // 灵感/想法
}

impl std::fmt::Display for AnnotationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AnnotationType::Note => "note",
            AnnotationType::Todo => "todo",
            AnnotationType::Warning => "warning",
            AnnotationType::Idea => "idea",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for AnnotationType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "note" => Ok(AnnotationType::Note),
            "todo" => Ok(AnnotationType::Todo),
            "warning" => Ok(AnnotationType::Warning),
            "idea" => Ok(AnnotationType::Idea),
            _ => Err(format!("Unknown annotation type: {}", s)),
        }
    }
}

// ==================== 文本内联批注模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextAnnotation {
    pub id: String,
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub content: String,
    pub annotation_type: AnnotationType,
    pub from_pos: i32,
    pub to_pos: i32,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub resolved_at: Option<DateTime<Local>>,
}

// ==================== 故事摘要模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorySummary {
    pub id: String,
    pub story_id: String,
    pub summary_type: String,
    pub content: String,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

// ==================== 故事大纲模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryOutline {
    pub id: String,
    pub story_id: String,
    pub content: String,
    pub structure_json: Option<String>,
    pub act_count: i32,
    pub total_scenes_estimate: Option<i32>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryOutlineAct {
    pub act_number: i32,
    pub title: String,
    pub summary: String,
    pub key_plot_points: Vec<String>,
    pub estimated_scenes: i32,
}

// ==================== 角色关系模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterRelationship {
    pub id: String,
    pub story_id: String,
    pub source_character_id: String,
    pub target_character_id: String,
    pub target_character_name: Option<String>, // 填充时查询
    pub relationship_type: String,
    pub description: Option<String>,
    pub dynamic: Option<String>,
    pub created_at: DateTime<Local>,
}

// ==================== 世界观模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBuilding {
    pub id: String,
    pub story_id: String,
    pub concept: String,
    pub rules: Vec<WorldRule>,
    pub history: Option<String>,
    pub cultures: Vec<Culture>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldRule {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub rule_type: RuleType,
    pub importance: i32, // 1-10
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RuleType {
    Magic,       // 魔法规则
    Technology,  // 科技规则
    Social,      // 社会规则
    Physical,    // 物理规则
    Biological,  // 生物规则
    Historical,  // 历史规则
    Cultural,    // 文化规则
    Custom,      // 自定义
}

impl std::fmt::Display for RuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RuleType::Magic => "魔法",
            RuleType::Technology => "科技",
            RuleType::Social => "社会",
            RuleType::Physical => "物理",
            RuleType::Biological => "生物",
            RuleType::Historical => "历史",
            RuleType::Cultural => "文化",
            RuleType::Custom => "自定义",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Culture {
    pub name: String,
    pub description: String,
    pub customs: Vec<String>,
    pub values: Vec<String>,
}

// ==================== 场景设置模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setting {
    pub id: String,
    pub story_id: String,
    pub name: String,
    pub description: Option<String>,
    pub location_type: LocationType,
    pub sensory_details: SensoryDetails,
    pub significance: Option<String>,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocationType {
    City,
    Building,
    Nature,
    Underground,
    Underwater,
    Space,
    Dream,
    Virtual,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SensoryDetails {
    pub visual: Vec<String>,
    pub auditory: Vec<String>,
    pub olfactory: Vec<String>,
    pub tactile: Vec<String>,
    pub gustatory: Vec<String>,
}

// ==================== 文字风格模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingStyle {
    pub id: String,
    pub story_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub vocabulary_level: Option<String>,
    pub sentence_structure: Option<String>,
    pub custom_rules: Vec<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

// ==================== 知识图谱模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub story_id: String,
    pub name: String,
    pub entity_type: EntityType,
    pub attributes: serde_json::Value,
    pub embedding: Option<Vec<f32>>,
    pub first_seen: DateTime<Local>,
    pub last_updated: DateTime<Local>,
    
    // 置信度评分 (0-1)
    pub confidence_score: Option<f32>,
    // 访问计数（用于遗忘曲线）
    pub access_count: i32,
    // 最后访问时间
    pub last_accessed: Option<DateTime<Local>>,
    
    // 归档状态
    pub is_archived: bool,
    pub archived_at: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    Character,
    Location,
    Item,
    Organization,
    Concept,
    Event,
    PlotDevice,
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EntityType::Character => "Character",
            EntityType::Location => "Location",
            EntityType::Item => "Item",
            EntityType::Organization => "Organization",
            EntityType::Concept => "Concept",
            EntityType::Event => "Event",
            EntityType::PlotDevice => "PlotDevice",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for EntityType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Character" | "角色" => Ok(EntityType::Character),
            "Location" | "地点" => Ok(EntityType::Location),
            "Item" | "物品" => Ok(EntityType::Item),
            "Organization" | "组织" => Ok(EntityType::Organization),
            "Concept" | "概念" => Ok(EntityType::Concept),
            "Event" | "事件" => Ok(EntityType::Event),
            "PlotDevice" => Ok(EntityType::PlotDevice),
            _ => Err(format!("Unknown entity type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relation {
    pub id: String,
    pub story_id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation_type: RelationType,
    pub strength: f32,
    pub evidence: Vec<String>,
    pub first_seen: DateTime<Local>,
    
    // 置信度评分
    pub confidence_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationType {
    // 人际关系
    Friend,
    Enemy,
    Family,
    Lover,
    Mentor,
    Rival,
    Ally,
    
    // 物品关系
    LocatedAt,
    BelongsTo,
    Uses,
    Owns,
    Created,
    Destroyed,
    
    // 组织关系
    PartOf,
    Leads,
    MemberOf,
    FounderOf,
    
    // 因果关系
    Causes,
    Enables,
    Prevents,
    ResultsIn,
    
    // 语义关系
    SimilarTo,
    OppositeOf,
    RelatedTo,
    EvolvesInto,
    
    // 动态关系
    Supersedes,   // 取代
    Contradicts,  // 矛盾
    
    // 创世引擎关系
    ParticipatesIn, // 角色参与场景
    SetUpIn,        // 伏笔在场景中埋设
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RelationType::Friend => "Friend",
            RelationType::Enemy => "Enemy",
            RelationType::Family => "Family",
            RelationType::Lover => "Lover",
            RelationType::Mentor => "Mentor",
            RelationType::Rival => "Rival",
            RelationType::Ally => "Ally",
            RelationType::LocatedAt => "LocatedAt",
            RelationType::BelongsTo => "BelongsTo",
            RelationType::Uses => "Uses",
            RelationType::Owns => "Owns",
            RelationType::Created => "Created",
            RelationType::Destroyed => "Destroyed",
            RelationType::PartOf => "PartOf",
            RelationType::Leads => "Leads",
            RelationType::MemberOf => "MemberOf",
            RelationType::FounderOf => "FounderOf",
            RelationType::Causes => "Causes",
            RelationType::Enables => "Enables",
            RelationType::Prevents => "Prevents",
            RelationType::ResultsIn => "ResultsIn",
            RelationType::SimilarTo => "SimilarTo",
            RelationType::OppositeOf => "OppositeOf",
            RelationType::RelatedTo => "RelatedTo",
            RelationType::EvolvesInto => "EvolvesInto",
            RelationType::Supersedes => "Supersedes",
            RelationType::Contradicts => "Contradicts",
            RelationType::ParticipatesIn => "ParticipatesIn",
            RelationType::SetUpIn => "SetUpIn",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for RelationType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Friend" => Ok(RelationType::Friend),
            "Enemy" => Ok(RelationType::Enemy),
            "Family" => Ok(RelationType::Family),
            "Lover" => Ok(RelationType::Lover),
            "Mentor" => Ok(RelationType::Mentor),
            "Rival" => Ok(RelationType::Rival),
            "Ally" => Ok(RelationType::Ally),
            "LocatedAt" => Ok(RelationType::LocatedAt),
            "BelongsTo" => Ok(RelationType::BelongsTo),
            "Uses" => Ok(RelationType::Uses),
            "Owns" => Ok(RelationType::Owns),
            "Created" => Ok(RelationType::Created),
            "Destroyed" => Ok(RelationType::Destroyed),
            "PartOf" => Ok(RelationType::PartOf),
            "Leads" => Ok(RelationType::Leads),
            "MemberOf" => Ok(RelationType::MemberOf),
            "FounderOf" => Ok(RelationType::FounderOf),
            "Causes" => Ok(RelationType::Causes),
            "Enables" => Ok(RelationType::Enables),
            "Prevents" => Ok(RelationType::Prevents),
            "ResultsIn" => Ok(RelationType::ResultsIn),
            "SimilarTo" => Ok(RelationType::SimilarTo),
            "OppositeOf" => Ok(RelationType::OppositeOf),
            "RelatedTo" => Ok(RelationType::RelatedTo),
            "EvolvesInto" => Ok(RelationType::EvolvesInto),
            "Supersedes" => Ok(RelationType::Supersedes),
            "Contradicts" => Ok(RelationType::Contradicts),
            "ParticipatesIn" => Ok(RelationType::ParticipatesIn),
            "SetUpIn" => Ok(RelationType::SetUpIn),
            _ => Err(format!("Unknown relation type: {}", s)),
        }
    }
}

// ==================== 工作室配置模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudioConfig {
    pub id: String,
    pub story_id: String,
    pub pen_name: Option<String>,
    pub llm_config: LlmStudioConfig,
    pub ui_config: UiStudioConfig,
    pub agent_bots: Vec<AgentBotConfig>,
    pub frontstage_theme: Option<String>,
    pub backstage_theme: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmStudioConfig {
    pub default_provider: String,
    pub default_model: String,
    pub generation_temperature: f32,
    pub max_tokens: i32,
    pub profiles: Vec<LlmProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProfile {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub temperature: f32,
    pub max_tokens: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UiStudioConfig {
    pub frontstage_font_size: i32,
    pub frontstage_font_family: String,
    pub frontstage_line_height: f32,
    pub frontstage_paper_color: String,
    pub frontstage_text_color: String,
    pub backstage_theme: String,
    pub backstage_accent_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBotConfig {
    pub id: String,
    pub name: String,
    pub agent_type: AgentBotType,
    pub enabled: bool,
    pub llm_profile_id: String,
    pub system_prompt: String,
    pub custom_settings: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentBotType {
    WorldBuilding,  // 世界观助手
    Character,      // 人物助手
    WritingStyle,   // 文风助手
    Plot,           // 情节助手
    Scene,          // 场景助手
    Memory,         // 记忆助手
}

// ==================== Request/Response 模型 ====================

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateSceneRequest {
    pub story_id: String,
    pub sequence_number: i32,
    pub title: Option<String>,
    pub dramatic_goal: Option<String>,
    pub external_pressure: Option<String>,
    pub conflict_type: Option<String>,
    pub characters_present: Vec<String>,
    pub setting_location: Option<String>,
    pub content: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UpdateSceneRequest {
    pub title: Option<String>,
    pub dramatic_goal: Option<String>,
    pub external_pressure: Option<String>,
    pub conflict_type: Option<String>,
    pub characters_present: Option<Vec<String>>,
    pub character_conflicts: Option<Vec<CharacterConflict>>,
    pub content: Option<String>,
    pub setting_location: Option<String>,
    pub setting_time: Option<String>,
    pub setting_atmosphere: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateWorldBuildingRequest {
    pub story_id: String,
    pub concept: String,
    pub rules: Option<Vec<WorldRule>>,
    pub history: Option<String>,
    pub cultures: Option<Vec<Culture>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateWritingStyleRequest {
    pub story_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub vocabulary_level: Option<String>,
    pub sentence_structure: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StudioExportRequest {
    pub story_id: String,
    pub include_world_building: bool,
    pub include_characters: bool,
    pub include_writing_style: bool,
    pub include_scenes: bool,
    pub include_llm_config: bool,
    pub include_ui_config: bool,
    pub include_agent_bots: bool,
}

#[derive(Debug, Serialize)]
pub struct StudioExportData {
    pub manifest: ExportManifest,
    pub story: crate::db::models::Story,
    pub world_building: Option<WorldBuilding>,
    pub characters: Vec<crate::db::models::Character>,
    pub writing_style: Option<WritingStyle>,
    pub scenes: Vec<Scene>,
    pub studio_config: Option<StudioConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportManifest {
    pub version: String,
    pub exported_at: DateTime<Local>,
    pub story_id: String,
    pub story_title: String,
}

// ==================== 场景版本相关请求/响应 (新增) ====================

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateSceneVersionRequest {
    pub scene_id: String,
    pub change_summary: String,
    pub created_by: String, // "user" | "ai" | "system"
    pub model_used: Option<String>,
    pub confidence_score: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct SceneVersionDiff {
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompareVersionsResponse {
    pub version_a: SceneVersion,
    pub version_b: SceneVersion,
    pub differences: Vec<SceneVersionDiff>,
}

// ==================== 变更追踪模型 (修订模式) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeTrack {
    pub id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub version_id: Option<String>,
    pub author_id: String,
    pub author_name: Option<String>,
    pub change_type: ChangeType,
    pub from_pos: i32,
    pub to_pos: i32,
    pub content: Option<String>,
    pub status: ChangeStatus,
    pub created_at: DateTime<Local>,
    pub resolved_at: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Insert,
    Delete,
    Format,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeStatus {
    Pending,
    Accepted,
    Rejected,
}

impl ChangeTrack {
    pub fn new(
        scene_id: Option<String>,
        chapter_id: Option<String>,
        author_id: String,
        change_type: ChangeType,
        from_pos: i32,
        to_pos: i32,
        content: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            scene_id,
            chapter_id,
            version_id: None,
            author_id,
            author_name: None,
            change_type,
            from_pos,
            to_pos,
            content,
            status: ChangeStatus::Pending,
            created_at: Local::now(),
            resolved_at: None,
        }
    }
}


// ==================== 评论线程模型 (修订模式) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentThread {
    pub id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub version_id: Option<String>,
    pub anchor_type: AnchorType,
    pub from_pos: Option<i32>,
    pub to_pos: Option<i32>,
    pub selected_text: Option<String>,
    pub status: ThreadStatus,
    pub created_at: DateTime<Local>,
    pub resolved_at: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentMessage {
    pub id: String,
    pub thread_id: String,
    pub author_id: String,
    pub author_name: Option<String>,
    pub content: String,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnchorType {
    TextRange,
    SceneLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ThreadStatus {
    Open,
    Resolved,
}

impl CommentThread {
    pub fn new(
        version_id: Option<String>,
        anchor_type: AnchorType,
        scene_id: Option<String>,
        chapter_id: Option<String>,
        from_pos: Option<i32>,
        to_pos: Option<i32>,
        selected_text: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            version_id,
            anchor_type,
            scene_id,
            chapter_id,
            from_pos,
            to_pos,
            selected_text,
            status: ThreadStatus::Open,
            created_at: Local::now(),
            resolved_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentThreadWithMessages {
    pub thread: CommentThread,
    pub messages: Vec<CommentMessage>,
}


// ==================== StyleDNA 模型 (深度风格系统) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleDNA {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub dna_json: String,
    pub is_builtin: bool,
    pub is_user_created: bool,
    pub created_at: DateTime<Local>,
}


// ==================== StyleDNA 六维向量快照 (W3-B7) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleSnapshot {
    pub id: String,
    pub story_id: String,
    pub chapter_number: Option<i32>,
    pub scene_number: Option<i32>,
    pub sentence_length: f64,
    pub dialogue_ratio: f64,
    pub metaphor_density: f64,
    pub inner_monologue_ratio: f64,
    pub emotion_density: f64,
    pub rhythm_score: f64,
    pub computed_at: DateTime<Local>,
}

// ==================== 风格混合配置模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryStyleConfig {
    pub id: String,
    pub story_id: String,
    pub name: String,
    pub blend_json: String,  // JSON serialized Vec<BlendComponent>
    pub is_active: bool,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}


// ==================== 用户反馈与偏好模型 (自适应学习) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFeedbackLog {
    pub id: String,
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub feedback_type: FeedbackType,
    pub agent_type: Option<String>,
    pub original_ai_text: String,
    pub final_text: String,
    pub ai_score: Option<f32>,
    pub user_satisfaction: Option<i32>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    Accept,   // 用户直接接受 AI 建议
    Reject,   // 用户拒绝 AI 建议
    Modify,   // 用户修改后接受
}

impl std::fmt::Display for FeedbackType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FeedbackType::Accept => write!(f, "accept"),
            FeedbackType::Reject => write!(f, "reject"),
            FeedbackType::Modify => write!(f, "modify"),
        }
    }
}

impl std::str::FromStr for FeedbackType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "accept" => Ok(FeedbackType::Accept),
            "reject" => Ok(FeedbackType::Reject),
            "modify" => Ok(FeedbackType::Modify),
            _ => Err(format!("Unknown feedback type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreference {
    pub id: String,
    pub story_id: String,
    pub preference_type: PreferenceType,
    pub preference_key: String,
    pub preference_value: String,
    pub confidence: f32,
    pub evidence_count: i32,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceType {
    Style,      // 风格偏好
    Content,    // 内容偏好
    Structure,  // 结构偏好
    Dialogue,   // 对话偏好
    Pacing,     // 节奏偏好
}

impl std::fmt::Display for PreferenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreferenceType::Style => write!(f, "style"),
            PreferenceType::Content => write!(f, "content"),
            PreferenceType::Structure => write!(f, "structure"),
            PreferenceType::Dialogue => write!(f, "dialogue"),
            PreferenceType::Pacing => write!(f, "pacing"),
        }
    }
}

impl std::str::FromStr for PreferenceType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "style" => Ok(PreferenceType::Style),
            "content" => Ok(PreferenceType::Content),
            "structure" => Ok(PreferenceType::Structure),
            "dialogue" => Ok(PreferenceType::Dialogue),
            "pacing" => Ok(PreferenceType::Pacing),
            _ => Err(format!("Unknown preference type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub style_dna_id: Option<String>,
    pub methodology_id: Option<String>,
    pub methodology_step: Option<i32>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub id: String,
    pub story_id: String,
    pub name: String,
    pub background: Option<String>,
    pub personality: Option<String>,
    pub goals: Option<String>,
    pub appearance: Option<String>,
    pub gender: Option<String>,
    pub age: Option<i32>,
    pub dynamic_traits: Vec<DynamicTrait>,
    // --- 动态状态字段 ---
    pub cs_location: Option<String>,
    pub cs_power_level: Option<String>,
    pub cs_physical_state: Option<String>,
    pub cs_mental_state: Option<String>,
    pub cs_key_items: Option<String>,
    pub cs_recent_events: Option<String>,
    pub cs_updated_at_chapter: Option<i32>,
    pub cs_json: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub story_id: String,
    pub chapter_number: i32,
    pub title: Option<String>,
    pub outline: Option<String>,
    pub content: Option<String>,
    pub word_count: Option<i32>,
    pub model_used: Option<String>,
    pub cost: Option<f64>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

// Request/Response models
#[derive(Debug, Deserialize)]
pub struct CreateStoryRequest {
    pub title: String,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub style_dna_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStoryRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub style_dna_id: Option<String>,
    pub methodology_id: Option<String>,
    pub methodology_step: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCharacterRequest {
    pub story_id: String,
    pub name: String,
    pub background: Option<String>,
    pub personality: Option<String>,
    pub goals: Option<String>,
    pub appearance: Option<String>,
    pub gender: Option<String>,
    pub age: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChapterRequest {
    pub story_id: String,
    pub chapter_number: i32,
    pub title: Option<String>,
    pub outline: Option<String>,
    pub content: Option<String>,
}

// ==================== Auth Models ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub is_local_user: bool,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthAccount {
    pub id: String,
    pub user_id: String,
    pub provider: String,
    pub provider_account_id: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Local>>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub token: String,
    pub expires_at: DateTime<Local>,
    pub created_at: DateTime<Local>,
}

// Auth request/response models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

// ==================== Export Template Models ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub format: String,
    pub template_content: String,
    pub is_builtin: bool,
    pub is_user_created: bool,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExportTemplateRequest {
    pub name: String,
    pub description: Option<String>,
    pub format: String,
    pub template_content: String,
}

// ==================== AI Operation History Models ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiOperation {
    pub id: String,
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub operation_type: String,
    pub operation_name: String,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
    pub previous_content: Option<String>,
    pub new_content: Option<String>,
    pub metadata: Option<String>,
    pub status: String,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAiOperationRequest {
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub operation_type: String,
    pub operation_name: String,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
    pub previous_content: Option<String>,
    pub new_content: Option<String>,
    pub metadata: Option<String>,
}

// ==================== Story System Models ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryContract {
    pub id: String,
    pub story_id: String,
    pub contract_type: String, // MASTER_SETTING | VOLUME | CHAPTER | REVIEW
    pub contract_json: String,
    pub version: i32,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneCommit {
    pub id: String,
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub chapter_number: i32,
    pub status: String, // pending | accepted | rejected
    pub outline_snapshot_json: Option<String>,
    pub review_result_json: Option<String>,
    pub fulfillment_result_json: Option<String>,
    pub accepted_events_json: Option<String>,
    pub state_deltas_json: Option<String>,
    pub entity_deltas_json: Option<String>,
    pub summary_text: Option<String>,
    pub dominant_strand: Option<String>,
    pub projection_status_json: Option<String>,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisRun {
    pub id: String,
    pub story_id: Option<String>,
    pub session_id: String,
    pub premise: String,
    pub status: String, // pending | running | paused | completed | failed
    pub current_step: Option<String>,
    pub current_step_number: i32,
    pub total_steps: i32,
    pub steps_json: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItem {
    pub id: String,
    pub story_id: String,
    pub category: String, // world_rule | character_state | relationship | story_fact | open_loop | reader_promise | timeline
    pub subject: Option<String>,
    pub field: Option<String>,
    pub value: Option<String>,
    pub source_chapter: Option<i32>,
    pub confidence: f32,
    pub status: String, // active | archived | conflicting
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterReadingPower {
    pub id: String,
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_number: i32,
    pub hook_type: Option<String>,
    pub hook_strength: String,
    pub coolpoint_patterns_json: Option<String>,
    pub micropayoffs_json: Option<String>,
    pub hard_violations_json: Option<String>,
    pub soft_suggestions_json: Option<String>,
    pub is_transition: bool,
    pub override_count: i32,
    pub debt_balance: f64,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaseDebt {
    pub id: i64,
    pub story_id: String,
    pub debt_type: String,
    pub original_amount: f64,
    pub current_amount: f64,
    pub interest_rate: f64,
    pub source_chapter: i32,
    pub due_chapter: i32,
    pub override_contract_id: Option<i64>,
    pub status: String, // active | paid | overdue | written_off
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverrideContract {
    pub id: i64,
    pub story_id: String,
    pub chapter_number: i32,
    pub constraint_type: String,
    pub constraint_id: String,
    pub rationale_type: String,
    pub rationale_text: String,
    pub payback_plan: String,
    pub due_chapter: i32,
    pub status: String, // pending | fulfilled | overdue | cancelled
    pub fulfilled_at: Option<DateTime<Local>>,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    pub id: String,
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_number: i32,
    pub severity: String, // critical | high | medium | low
    pub category: String, // continuity | setting | character | timeline | ai_flavor | logic | pacing | other
    pub location: Option<String>,
    pub description: String,
    pub evidence: Option<String>,
    pub fix_hint: Option<String>,
    pub blocking: bool,
    pub resolved: bool,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenreProfile {
    pub id: String,
    pub genre_name: String,
    pub canonical_name: String,
    pub aliases_json: Option<String>,
    pub core_tone: Option<String>,
    pub pacing_strategy: Option<String>,
    pub anti_patterns_json: Option<String>,
    pub reference_tables_json: Option<String>,
    pub is_builtin: bool,
    pub created_at: DateTime<Local>,
}

// ==================== Pipeline 模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blueprint {
    pub id: String,
    pub story_id: String,
    pub chapter_number: i32,
    pub title: Option<String>,
    pub role: Option<String>,
    pub purpose: Option<String>,
    pub key_events: Option<String>,
    pub characters: Option<String>,
    pub suspense_hook: Option<String>,
    pub user_guidance: Option<String>,
    pub notes: Option<String>,
    pub notes_updated_at: Option<DateTime<Local>>,
    pub knowledge_query_hint: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBlueprintRequest {
    pub story_id: String,
    pub chapter_number: i32,
    pub title: Option<String>,
    pub role: Option<String>,
    pub purpose: Option<String>,
    pub key_events: Option<Vec<String>>,
    pub characters: Option<Vec<String>>,
    pub suspense_hook: Option<String>,
    pub user_guidance: Option<String>,
    pub knowledge_query_hint: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateBlueprintRequest {
    pub title: Option<String>,
    pub role: Option<String>,
    pub purpose: Option<String>,
    pub key_events: Option<Vec<String>>,
    pub characters: Option<Vec<String>>,
    pub suspense_hook: Option<String>,
    pub user_guidance: Option<String>,
    pub notes: Option<String>,
    pub knowledge_query_hint: Option<String>,
}

// --- Draft ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Draft {
    pub id: String,
    pub story_id: String,
    pub chapter_number: i32,
    pub version: i32,
    pub status: DraftStatus,
    pub source: DraftSource,
    pub content: String,
    pub word_count: i32,
    pub model_used: Option<String>,
    pub cost: Option<f64>,
    pub metadata: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DraftStatus {
    Draft,
    Refined,
    Reviewed,
    Finalized,
    Archived,
}

impl std::fmt::Display for DraftStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DraftStatus::Draft => write!(f, "draft"),
            DraftStatus::Refined => write!(f, "refined"),
            DraftStatus::Reviewed => write!(f, "reviewed"),
            DraftStatus::Finalized => write!(f, "finalized"),
            DraftStatus::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for DraftStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "draft" => Ok(DraftStatus::Draft),
            "refined" => Ok(DraftStatus::Refined),
            "reviewed" => Ok(DraftStatus::Reviewed),
            "finalized" => Ok(DraftStatus::Finalized),
            "archived" => Ok(DraftStatus::Archived),
            _ => Err(format!("Unknown draft status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DraftSource {
    Write,
    Rewrite,
    Refine,
    ReviewFix,
}

impl std::fmt::Display for DraftSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DraftSource::Write => write!(f, "write"),
            DraftSource::Rewrite => write!(f, "rewrite"),
            DraftSource::Refine => write!(f, "refine"),
            DraftSource::ReviewFix => write!(f, "review_fix"),
        }
    }
}

impl std::str::FromStr for DraftSource {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "write" => Ok(DraftSource::Write),
            "rewrite" => Ok(DraftSource::Rewrite),
            "refine" => Ok(DraftSource::Refine),
            "review_fix" => Ok(DraftSource::ReviewFix),
            _ => Err(format!("Unknown draft source: {}", s)),
        }
    }
}

// --- Revision ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    pub id: String,
    pub story_id: String,
    pub draft_id: String,
    pub revision_index: i32,
    pub revision_type: RevisionType,
    pub status: RevisionStatus,
    pub user_prompt: Option<String>,
    pub original_content: String,
    pub revised_content: String,
    pub word_count: i32,
    pub change_summary: Option<String>,
    pub model_used: Option<String>,
    pub cost: Option<f64>,
    pub metadata: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevisionType {
    Refine,
    ReviewFix,
    UserEdit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RevisionStatus {
    Pending,
    Merged,
    Discarded,
    Superseded,
}

impl std::fmt::Display for RevisionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RevisionType::Refine => write!(f, "refine"),
            RevisionType::ReviewFix => write!(f, "review_fix"),
            RevisionType::UserEdit => write!(f, "user_edit"),
        }
    }
}

impl std::str::FromStr for RevisionType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "refine" => Ok(RevisionType::Refine),
            "review_fix" => Ok(RevisionType::ReviewFix),
            "user_edit" => Ok(RevisionType::UserEdit),
            _ => Err(format!("Unknown revision type: {}", s)),
        }
    }
}

impl std::fmt::Display for RevisionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RevisionStatus::Pending => write!(f, "pending"),
            RevisionStatus::Merged => write!(f, "merged"),
            RevisionStatus::Discarded => write!(f, "discarded"),
            RevisionStatus::Superseded => write!(f, "superseded"),
        }
    }
}

impl std::str::FromStr for RevisionStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(RevisionStatus::Pending),
            "merged" => Ok(RevisionStatus::Merged),
            "discarded" => Ok(RevisionStatus::Discarded),
            "superseded" => Ok(RevisionStatus::Superseded),
            _ => Err(format!("Unknown revision status: {}", s)),
        }
    }
}

// --- Pipeline Review (审稿报告) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineReview {
    pub id: String,
    pub story_id: String,
    pub draft_id: String,
    pub review_index: i32,
    pub content: String,
    pub dimensions: Option<String>,
    pub issues: Option<String>,
    pub overall_score: Option<f32>,
    pub review_focus: Option<String>,
    pub model_used: Option<String>,
    pub cost: Option<f64>,
    pub metadata: Option<String>,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDimension {
    pub name: String,
    pub score: f32,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssueItem {
    pub severity: String,
    pub dimension: String,
    pub description: String,
    pub suggestion: String,
}

// --- Post Process ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessRun {
    pub id: String,
    pub story_id: String,
    pub chapter_number: i32,
    pub source_label: String,
    pub scope: Option<String>,
    pub status: PostProcessStatus,
    pub started_at: DateTime<Local>,
    pub completed_at: Option<DateTime<Local>>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessStep {
    pub id: String,
    pub run_id: String,
    pub step_key: String,
    pub step_label: String,
    pub status: StepStatus,
    pub critical: bool,
    pub log_output: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<DateTime<Local>>,
    pub completed_at: Option<DateTime<Local>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostProcessStatus {
    Running,
    Completed,
    Failed,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

impl std::fmt::Display for PostProcessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PostProcessStatus::Running => write!(f, "running"),
            PostProcessStatus::Completed => write!(f, "completed"),
            PostProcessStatus::Failed => write!(f, "failed"),
            PostProcessStatus::Partial => write!(f, "partial"),
        }
    }
}

impl std::str::FromStr for PostProcessStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "running" => Ok(PostProcessStatus::Running),
            "completed" => Ok(PostProcessStatus::Completed),
            "failed" => Ok(PostProcessStatus::Failed),
            "partial" => Ok(PostProcessStatus::Partial),
            _ => Err(format!("Unknown post process status: {}", s)),
        }
    }
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepStatus::Pending => write!(f, "pending"),
            StepStatus::Running => write!(f, "running"),
            StepStatus::Success => write!(f, "success"),
            StepStatus::Failed => write!(f, "failed"),
            StepStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for StepStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(StepStatus::Pending),
            "running" => Ok(StepStatus::Running),
            "success" => Ok(StepStatus::Success),
            "failed" => Ok(StepStatus::Failed),
            "skipped" => Ok(StepStatus::Skipped),
            _ => Err(format!("Unknown step status: {}", s)),
        }
    }
}

// --- LLM Call ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCall {
    pub id: String,
    pub story_id: Option<String>,
    pub draft_id: Option<String>,
    pub revision_id: Option<String>,
    pub model_id: String,
    pub model_name: Option<String>,
    pub purpose: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub duration_ms: i32,
    pub success: bool,
    pub error_message: Option<String>,
    pub prompt_preview: Option<String>,
    pub metadata: Option<String>,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Deserialize)]
pub struct RecordLlmCallRequest {
    pub story_id: Option<String>,
    pub draft_id: Option<String>,
    pub model_id: String,
    pub model_name: Option<String>,
    pub purpose: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub duration_ms: i32,
    pub success: bool,
    pub error_message: Option<String>,
}

// --- Character State ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterState {
    pub location: Option<String>,
    pub power_level: Option<String>,
    pub physical_state: Option<String>,
    pub mental_state: Option<String>,
    pub key_items: Option<String>,
    pub recent_events: Option<String>,
    pub updated_at_chapter: Option<i32>,
}
