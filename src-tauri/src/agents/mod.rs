//! Agent System - 智能代理系统
//!
//! 提供创作辅助的智能Agent框架
//!
//! ## Agent类型
//! - Writer: 写作助手 - 生成和改写内容
//! - Inspector: 质检员 - 检查内容质量
//! - OutlinePlanner: 大纲规划师 - 设计故事结构
//! - StyleMimic: 风格模仿师 - 分析和模仿文风
//! - PlotAnalyzer: 情节分析师 - 分析情节复杂度
#![allow(dead_code)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::memory::orchestrator::MemoryPack;

pub mod commands;
pub mod commentator;
pub mod context_optimizer;
pub mod distiller;
pub mod executor;
pub mod memory_compressor;
pub mod novel_creation;
pub mod orchestrator;
pub mod service;

// ==================== 核心Trait ====================

/// Agent特性 - 所有Agent必须实现
#[async_trait]
pub trait Agent: Send + Sync {
    /// Agent名称
    fn name(&self) -> &str;

    /// Agent描述
    fn description(&self) -> &str;

    /// 执行Agent任务
    async fn execute(
        &self,
        context: &AgentContext,
        input: &str,
    ) -> Result<AgentResult, Box<dyn std::error::Error>>;
}

// ==================== 子上下文结构 ====================

/// 故事元数据上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoryContext {
    #[serde(default)]
    pub story_id: String,
    #[serde(default)]
    pub story_title: String,
    #[serde(default)]
    pub genre: String, // 题材
    #[serde(default)]
    pub tone: String, // 文风
    #[serde(default)]
    pub pacing: String, // 节奏
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
    pub style_blend: Option<crate::creative_engine::style::blend::StyleBlendConfig>, /* 风格混合配置 */
    /// 风格指纹（v0.7.8: 续写加固 — 从参考文本提取的量化风格约束）
    #[serde(default)]
    pub style_fingerprint: Option<crate::creative_engine::style::fingerprint::StyleFingerprint>,
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
}

/// 角色信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterInfo {
    pub name: String,
    pub personality: String,
    pub role: String,
}

/// 章节摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterSummary {
    pub title: String,
    pub number: u32,
    pub summary: String,
}

/// Agent执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub content: String,
    pub score: Option<f32>, // 0.0 - 1.0
    pub suggestions: Vec<String>,
    /// 关联的 LLM request_id，供上层取消使用
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
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
    pub entry: crate::memory::orchestrator::MemoryEntry,
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

// ==================== 辅助函数 ====================

impl AgentContext {
    /// 创建最小上下文（用于测试）
    pub fn minimal(story_id: String, _input: String) -> Self {
        Self {
            story: StoryContext {
                story_id,
                story_title: "未命名作品".to_string(),
                genre: "小说".to_string(),
                tone: "中性".to_string(),
                pacing: "正常".to_string(),
            },
            narrative: NarrativeContext {
                chapter_number: 1,
                characters: vec![],
                previous_chapters: vec![],
                current_content: None,
                selected_text: None,
                narrative_structure: None,
                active_threads: vec![],
            },
            style: StyleContext {
                style_dna_id: None,
                style_blend: None,
                style_fingerprint: None,
            },
            world: WorldContext {
                world_rules: None,
                scene_structure: None,
                methodology_id: None,
                methodology_step: None,
            },
            memory: AgentMemoryContext {
                memory_pack: None,
                memory: None,
            },
        }
    }

    /// 构建角色描述字符串
    pub fn format_characters(&self) -> String {
        if self.narrative.characters.is_empty() {
            "暂无角色信息".to_string()
        } else {
            self.narrative
                .characters
                .iter()
                .map(|c| format!("{}（{}）: {}", c.name, c.role, c.personality))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    /// 构建前文摘要
    pub fn format_previous_chapters(&self) -> String {
        if self.narrative.previous_chapters.is_empty() {
            "这是第一章".to_string()
        } else {
            self.narrative
                .previous_chapters
                .iter()
                .map(|c| format!("第{}章 {}: {}", c.number, c.title, c.summary))
                .collect::<Vec<_>>()
                .join("\n\n")
        }
    }

    /// 构建叙事结构上下文描述
    pub fn format_narrative_structure(&self) -> String {
        if let Some(ref ns) = self.narrative.narrative_structure {
            let mut parts = vec![
                format!("当前幕: 第{}幕（{}）", ns.act_number, ns.current_act),
                format!("幕内位置: {:.0}%", ns.position_in_act * 100.0),
                format!("戏剧功能: {}", ns.dramatic_function),
            ];
            if ns.is_near_boundary {
                parts.push("注意: 接近叙事边界，可能发生转折".to_string());
            }
            if !self.narrative.active_threads.is_empty() {
                parts.push(format!(
                    "活跃线索: {}",
                    self.narrative.active_threads.join(", ")
                ));
            }
            parts.join("\n")
        } else {
            "叙事结构信息暂不可用".to_string()
        }
    }
}

impl AgentResult {
    /// 创建简单结果
    pub fn simple(content: String) -> Self {
        Self {
            content,
            score: None,
            suggestions: vec![],
            request_id: None,
        }
    }

    /// 创建带评分的结果
    pub fn with_score(content: String, score: f32) -> Self {
        Self {
            content,
            score: Some(score.clamp(0.0, 1.0)),
            suggestions: vec![],
            request_id: None,
        }
    }

    /// 是否高质量
    pub fn is_high_quality(&self) -> bool {
        self.score.map(|s| s >= 0.8).unwrap_or(true)
    }
}
