//! 策略选择模型
//!
//! 定义可被模型发现与选择的创作资产，以及策略选择请求上下文。
//! 策略结果类型（SelectedStrategy / StrategyOverrides）已迁移至
//! domain::strategy。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use crate::domain::strategy::{SelectedStrategy, StrategyOverrides};

/// 可被发现与选择的创作资产种类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// 智能体（Writer / Inspector / OutlinePlanner 等）
    Agent,
    /// 技能（builtin 或用户技能）
    Skill,
    /// 系统命令（create_story / update_character 等）
    SystemCommand,
    /// MCP 外部工具
    McpTool,
    /// 创作方法论（雪花法、场景结构、英雄之旅等）
    Methodology,
    /// 体裁画像（43 个网文模板）
    GenreProfile,
    /// 风格 DNA
    StyleDna,
    /// 工作流模板
    Workflow,
    /// 经典桥段卡（v0.17.0：30+ 张可复用叙事功能模板）
    BeatCard,
    /// 剧情引擎（v0.17.0：21 种正交叙事动力）
    StoryEngine,
    /// 高压关系（v0.17.0：13 种角色对位关系，冲突放大器）
    PressureRelationship,
}

impl std::fmt::Display for AssetKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AssetKind::Agent => "agent",
            AssetKind::Skill => "skill",
            AssetKind::SystemCommand => "system_command",
            AssetKind::McpTool => "mcp_tool",
            AssetKind::Methodology => "methodology",
            AssetKind::GenreProfile => "genre_profile",
            AssetKind::StyleDna => "style_dna",
            AssetKind::Workflow => "workflow",
            AssetKind::BeatCard => "beat_card",
            AssetKind::StoryEngine => "story_engine",
            AssetKind::PressureRelationship => "pressure_relationship",
        };
        write!(f, "{}", s)
    }
}

/// 统一的可选择资产描述
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectableAsset {
    /// 全局唯一 ID，例如 "genre_profile.apocalyptic" / "methodology.snowflake"
    pub id: String,
    /// 资产种类
    pub kind: AssetKind,
    /// 人类可读名称
    pub name: String,
    /// 一句话描述
    pub description: String,
    /// 何时应该被选择
    pub when_to_use: String,
    /// 输入要求（可选）
    pub input_description: Option<String>,
    /// 输出说明（可选）
    pub output_description: Option<String>,
    /// 资产载荷，按 kind 反序列化
    pub payload: serde_json::Value,
    /// 额外元数据
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl SelectableAsset {
    /// 构建一个简短提示文本，用于注入 LLM 上下文
    #[allow(dead_code)]
    pub fn to_prompt_line(&self) -> String {
        format!(
            "- {} ({}): {} — when_to_use: {}",
            self.id, self.kind, self.description, self.when_to_use
        )
    }

    /// 构建分组标题下的完整条目
    pub fn to_prompt_entry(&self) -> String {
        let mut lines = vec![
            format!("- {} ({}): {}", self.id, self.name, self.description),
            format!("  when_to_use: {}", self.when_to_use),
        ];
        if let Some(input) = &self.input_description {
            lines.push(format!("  input: {}", input));
        }
        if let Some(output) = &self.output_description {
            lines.push(format!("  output: {}", output));
        }
        lines.join("\n")
    }
}

/// 策略选择请求上下文
#[derive(Debug, Clone, Default)]
pub struct SelectionContext {
    /// 用户原始输入
    pub user_input: String,
    /// 当前故事阶段
    pub story_progress: String,
    /// 是否已有故事
    pub has_story: bool,
    /// 当前故事 ID（用于加载关联参考书籍）
    pub story_id: Option<String>,
    /// 当前故事的体裁（自由文本，可能来自 concept）
    pub genre_hint: Option<String>,
    /// LLM 已标准化的题材画像 ID 列表（优先于 genre_hint）
    pub preferred_genre_profile_ids: Vec<String>,
    /// 当前故事的方法论（若已设置）
    pub methodology_hint: Option<String>,
    /// 目标字数或长度
    pub word_count_target: Option<i32>,
    /// 额外用户偏好
    #[allow(dead_code)]
    pub user_preferences: HashMap<String, serde_json::Value>,
}
