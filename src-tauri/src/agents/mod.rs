#![allow(dead_code)]
#![allow(unused_imports)]
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

use async_trait::async_trait;

pub mod commands;
pub mod commentator;
pub mod context_optimizer;
// v0.21.0: 已删除死代码
// Agent（inspector/distiller/memory_compressor/outline_planner/style_mimic）
// 这些 Agent 的功能已由 agents/service.rs 统一实现
pub mod executor;
pub mod in_generation_checker;
pub mod novel_creation;
pub mod orchestrator;
pub mod pre_generation_gate;
pub mod service;
pub mod subagents;
pub(crate) mod trim_utils;

// 数据类型已下沉到中性 domain 层以打破循环依赖；agents
// 继续重新导出保持向后兼容。
pub use crate::domain::{
    agent_context::*,
    agent_types::{AgentResult, AgentTask, AgentType},
    novel_creation::*,
};

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

// ==================== AgentType 扩展方法 ====================

impl AgentType {
    pub fn agent_id(&self) -> &'static str {
        match self {
            AgentType::Writer => "writer",
            AgentType::Inspector => "inspector",
            AgentType::OutlinePlanner => "outline_planner",
            AgentType::StyleMimic => "style_mimic",
            AgentType::PlotAnalyzer => "plot_analyzer",
            AgentType::MemoryCompressor => "memory_compressor",
            AgentType::Commentator => "commentator",
            AgentType::KnowledgeDistiller => "knowledge_distiller",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            AgentType::Writer => "根据上下文生成或改写章节内容",
            AgentType::Inspector => "检查内容质量、逻辑连贯性、人物一致性",
            AgentType::OutlinePlanner => "设计故事大纲、章节结构",
            AgentType::StyleMimic => "分析并模仿特定文风",
            AgentType::PlotAnalyzer => "分析情节复杂度、检测漏洞",
            AgentType::MemoryCompressor => "将详细内容压缩为高层记忆摘要",
            AgentType::Commentator => "以金圣叹风格对小说段落进行实时文学点评",
            AgentType::KnowledgeDistiller => "将知识图谱蒸馏为高层故事摘要与世界观总结",
        }
    }
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
                ..Default::default()
            },
            narrative: NarrativeContext {
                chapter_number: 1,
                characters: vec![],
                previous_chapters: vec![],
                current_content: None,
                selected_text: None,
                narrative_structure: None,
                active_threads: vec![],
                narrative_event_history: None,
                outline_context: None,
            },
            style: StyleContext {
                style_dna_id: None,
                style_blend: None,
                style_fingerprint: None,
                ..Default::default()
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
            runtime_contract: None,
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
                .map(|c| {
                    let mut parts = vec![format!("{}（{}）", c.name, c.role)];
                    if let Some(ref gender) = c.gender {
                        parts.push(format!("性别: {}", gender));
                    }
                    if let Some(age) = c.age {
                        parts.push(format!("年龄: {}", age));
                    }
                    if let Some(ref appearance) = c.appearance {
                        if !appearance.trim().is_empty() {
                            parts.push(format!("外貌: {}", appearance));
                        }
                    }
                    parts.push(format!("性格与目标: {}", c.personality));
                    parts.join("；")
                })
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
