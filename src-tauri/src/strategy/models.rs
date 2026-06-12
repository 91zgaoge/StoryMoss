//! 策略选择模型
//!
//! 定义可被模型发现与选择的创作资产，以及策略选择结果。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
    /// 当前故事的体裁（自由文本，可能来自 concept）
    pub genre_hint: Option<String>,
    /// 当前故事的方法论（若已设置）
    pub methodology_hint: Option<String>,
    /// 目标字数或长度
    pub word_count_target: Option<i32>,
    /// 额外用户偏好
    #[allow(dead_code)]
    pub user_preferences: HashMap<String, serde_json::Value>,
}

/// 模型选择出的创作策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedStrategy {
    /// 选择理由
    pub rationale: String,
    /// 选中的体裁画像 ID（不带前缀）
    pub genre_profile_id: Option<String>,
    /// 选中的方法论 ID（不带前缀）
    pub methodology_id: Option<String>,
    /// 选中的 Style DNA ID 列表
    pub style_dna_ids: Vec<String>,
    /// 建议激活的技能 ID 列表
    pub skill_ids: Vec<String>,
    /// 建议使用的 Workflow ID
    pub workflow_id: Option<String>,
    /// 对其他创作参数的覆盖建议
    pub parameters: HashMap<String, serde_json::Value>,
}

impl Default for SelectedStrategy {
    fn default() -> Self {
        Self {
            rationale: String::new(),
            genre_profile_id: None,
            methodology_id: None,
            style_dna_ids: Vec::new(),
            skill_ids: Vec::new(),
            workflow_id: None,
            parameters: HashMap::new(),
        }
    }
}

impl SelectedStrategy {
    /// 合并用户手动锁定项到策略中
    pub fn merge_user_overrides(&mut self, overrides: &StrategyOverrides) {
        if let Some(genre) = &overrides.genre_profile_id {
            self.genre_profile_id = Some(genre.clone());
        }
        if let Some(methodology) = &overrides.methodology_id {
            self.methodology_id = Some(methodology.clone());
        }
        if !overrides.style_dna_ids.is_empty() {
            self.style_dna_ids = overrides.style_dna_ids.clone();
        }
        if !overrides.skill_ids.is_empty() {
            self.skill_ids = overrides.skill_ids.clone();
        }
    }
}

/// 用户手动覆盖项
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyOverrides {
    pub genre_profile_id: Option<String>,
    pub methodology_id: Option<String>,
    pub style_dna_ids: Vec<String>,
    pub skill_ids: Vec<String>,
}
