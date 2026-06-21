//! Agent context domain types.
//!
//! Shared execution context data structures used by agents and the creative engine.

use serde::{Deserialize, Serialize};

use crate::domain::{
    contracts::RuntimeContract,
    memory_pack::{MemoryEntry, MemoryPack},
    style::{StyleBlendConfig, StyleFingerprint},
};

// ==================== 子上下文结构 ====================

/// 故事元数据上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoryContext {
    #[serde(default)]
    pub story_id: String,
    #[serde(default)]
    pub story_title: String,
    #[serde(default)]
    pub description: Option<String>, // 作品简介
    #[serde(default)]
    pub genre: String, // 题材
    #[serde(default)]
    pub tone: String, // 文风
    #[serde(default)]
    pub pacing: String, // 节奏
    /// v0.10.0: 体裁画像 ID，用于 Writer 注入体裁专家策略
    #[serde(default)]
    pub genre_profile_id: Option<String>,
    /// v0.9.3: 预计算的个性化偏好扩展，避免每个候选都查库
    #[serde(default)]
    pub personalizer_extension: Option<String>,
}

/// 叙事内容上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NarrativeContext {
    #[serde(default)]
    pub chapter_number: u32,
    #[serde(default)]
    pub characters: Vec<CharacterInfo>,
    #[serde(default)]
    pub previous_chapters: Vec<ChapterSummary>,
    #[serde(default)]
    pub current_content: Option<String>,
    #[serde(default)]
    pub selected_text: Option<String>,
    // LitSeg E1: 叙事结构感知增强
    #[serde(default)]
    pub narrative_structure: Option<NarrativeStructureContext>,
    #[serde(default)]
    pub active_threads: Vec<String>,
    /// v0.22.5: 叙事事件历史（强度/情感/事件类型），用于节奏/情绪一致性提示
    #[serde(default)]
    pub narrative_event_history: Option<String>,
    /// 当前章节/场景大纲与草稿内容（v0.9.6：让 Writer 知道场景节拍目标）
    #[serde(default)]
    pub outline_context: Option<String>,
}

/// LitSeg 叙事结构上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeStructureContext {
    pub current_act: String,
    pub act_number: i32,
    pub position_in_act: f32,
    pub dramatic_function: String,
    pub is_near_boundary: bool,
}

impl Default for NarrativeStructureContext {
    fn default() -> Self {
        Self {
            current_act: "发展".to_string(),
            act_number: 1,
            position_in_act: 0.5,
            dramatic_function: "发展".to_string(),
            is_near_boundary: false,
        }
    }
}

/// 风格配置上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StyleContext {
    #[serde(default)]
    pub style_dna_id: Option<String>, // 风格DNA ID（向后兼容）
    #[serde(default)]
    pub style_blend: Option<StyleBlendConfig>, /* 风格混合配置 */
    /// 风格指纹（v0.7.8: 续写加固 — 从参考文本提取的量化风格约束）
    #[serde(default)]
    pub style_fingerprint: Option<StyleFingerprint>,
    /// v0.9.3: 预计算的风格 DNA 提示词扩展，避免每个候选都查库
    #[serde(default)]
    pub style_dna_extension: Option<String>,
    /// 写作风格详细设定（来自 writing_styles 表）
    #[serde(default)]
    pub writing_style_name: Option<String>,
    #[serde(default)]
    pub writing_style_description: Option<String>,
    #[serde(default)]
    pub writing_style_vocabulary_level: Option<String>,
    #[serde(default)]
    pub writing_style_sentence_structure: Option<String>,
    #[serde(default)]
    pub writing_style_custom_rules: Option<String>,
}

/// 世界观与方法论上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorldContext {
    #[serde(default)]
    pub world_rules: Option<String>, // 世界观规则（注入系统提示词）
    #[serde(default)]
    pub scene_structure: Option<String>, // 场景结构（注入系统提示词）
    #[serde(default)]
    pub methodology_id: Option<String>, // 创作方法论ID（如 snowflake, scene_structure）
    #[serde(default)]
    pub methodology_step: Option<String>, // 方法论当前步骤
}

/// 记忆上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentMemoryContext {
    /// 三层记忆包（Wave 3: MemoryPack 注入 AgentContext）
    #[serde(default)]
    pub memory_pack: Option<MemoryPack>,
    /// v0.8.0: 记忆上下文（混合路由后的结构化记忆 + 一致性报告）
    #[serde(default)]
    pub memory: Option<MemoryContext>,
}

// ==================== 主上下文结构 ====================

/// Agent执行上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentContext {
    #[serde(default)]
    pub story: StoryContext,
    #[serde(default)]
    pub narrative: NarrativeContext,
    #[serde(default)]
    pub style: StyleContext,
    #[serde(default)]
    pub world: WorldContext,
    #[serde(default)]
    pub memory: AgentMemoryContext,
    /// Story System 运行时合同（写前真源），供 Writer / Inspector 加载约束
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_contract: Option<RuntimeContract>,
}

/// 角色信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub name: String,
    pub personality: String,
    pub role: String,
    pub appearance: Option<String>,
    pub gender: Option<String>,
    pub age: Option<i32>,
}

/// 章节摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterSummary {
    pub title: String,
    pub number: u32,
    pub summary: String,
}

// ==================== v0.8.0: 记忆融合基础设施 ====================

/// 任务级记忆上下文 — 贯穿创作任务全生命周期
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryContext {
    /// 本次注入的记忆（由 MemoryRouter 生成）
    pub injected_memories: Vec<ScoredMemoryEntry>,
    /// 记忆一致性报告（由 Inspector 生成）
    pub consistency_report: Option<MemoryConsistencyReport>,
    /// 待写入记忆系统的更新队列
    pub update_queue: Vec<MemoryUpdate>,
    /// 路由策略
    pub strategy: RoutingStrategy,
}

/// 带相关度评分的记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredMemoryEntry {
    pub entry: MemoryEntry,
    pub relevance_score: f32, // 0-100
    pub reason: String,       // 注入理由
}

/// 记忆一致性报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConsistencyReport {
    pub memory_score: f32,      // 0-1
    pub conflicts: Vec<String>, // 冲突描述列表
}

/// 待写入记忆系统的更新
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUpdate {
    pub layer: MemoryLayer,
    pub content: String,
    pub source_chapter: i32,
    pub entity_refs: Vec<String>,
}

/// 记忆层类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryLayer {
    Working,
    Episodic,
    Semantic,
}

/// 路由策略
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum RoutingStrategy {
    #[default]
    Adaptive,
    Fast,
    Precise,
}
