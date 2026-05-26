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
pub mod executor;
pub mod service;
pub mod novel_creation;
pub mod memory_compressor;
pub mod commentator;
pub mod distiller;
pub mod orchestrator;
pub mod context_optimizer;

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

// ==================== 数据结构 ====================

/// Agent执行上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub story_id: String,
    pub story_title: String,
    pub genre: String,       // 题材
    pub tone: String,        // 文风
    pub pacing: String,      // 节奏
    pub chapter_number: u32,
    pub characters: Vec<CharacterInfo>,
    pub previous_chapters: Vec<ChapterSummary>,
    pub current_content: Option<String>, // 当前章节全文（已过滤元信息，保留纯内容）
    pub selected_text: Option<String>,   // 用户选中的文本
    pub world_rules: Option<String>,     // 世界观规则（注入系统提示词）
    pub scene_structure: Option<String>, // 场景结构（注入系统提示词）
    pub methodology_id: Option<String>,  // 创作方法论ID（如 snowflake, scene_structure）
    pub methodology_step: Option<String>, // 方法论当前步骤
    pub style_dna_id: Option<String>,    // 风格DNA ID（向后兼容）
    pub style_blend: Option<crate::creative_engine::style::blend::StyleBlendConfig>, // 风格混合配置
    /// 风格指纹（v0.7.8: 续写加固 — 从参考文本提取的量化风格约束）
    #[serde(default)]
    pub style_fingerprint: Option<crate::creative_engine::style::fingerprint::StyleFingerprint>,
    /// 三层记忆包（Wave 3: MemoryPack 注入 AgentContext）
    #[serde(default)]
    pub memory_pack: Option<MemoryPack>,
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
    pub score: Option<f32>,  // 0.0 - 1.0
    pub suggestions: Vec<String>,
    /// 关联的 LLM request_id，供上层取消使用
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

// ==================== 辅助函数 ====================

impl AgentContext {
    /// 创建最小上下文（用于测试）
    pub fn minimal(story_id: String, _input: String) -> Self {
        Self {
            story_id,
            story_title: "未命名作品".to_string(),
            genre: "小说".to_string(),
            tone: "中性".to_string(),
            pacing: "正常".to_string(),
            chapter_number: 1,
            characters: vec![],
            previous_chapters: vec![],
            current_content: None,
            selected_text: None,
            world_rules: None,
            scene_structure: None,
            methodology_id: None,
            methodology_step: None,
            style_dna_id: None,
            style_blend: None,
            style_fingerprint: None,
            memory_pack: None,
        }
    }
    
    /// 构建角色描述字符串
    pub fn format_characters(&self) -> String {
        if self.characters.is_empty() {
            "暂无角色信息".to_string()
        } else {
            self.characters
                .iter()
                .map(|c| format!("{}（{}）: {}", c.name, c.role, c.personality))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
    
    /// 构建前文摘要
    pub fn format_previous_chapters(&self) -> String {
        if self.previous_chapters.is_empty() {
            "这是第一章".to_string()
        } else {
            self.previous_chapters
                .iter()
                .map(|c| format!("第{}章 {}: {}", c.number, c.title, c.summary))
                .collect::<Vec<_>>()
                .join("\n\n")
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
