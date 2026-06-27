//! Tri-shot prompt synthesis domain types.
//!
//! Pure data definitions shared between the creative engine and agents.
//! Behavior remains in `crate::creative_engine::prompt_synthesis`.

use serde::{Deserialize, Serialize};

/// 单条资产清单项
#[derive(Debug, Clone, Serialize)]
pub struct AssetManifestItem {
    /// 稳定 ID（用于 LLM 引用选中资产）
    pub id: String,
    /// 资产类别
    pub kind: String,
    /// 人类可读标签
    pub label: String,
    /// 一行摘要
    pub one_line: String,
    /// 相关性标签
    pub tags: Vec<String>,
}

/// 资产清单
#[derive(Debug, Clone, Serialize)]
pub struct AssetManifest {
    /// 清单项列表
    pub items: Vec<AssetManifestItem>,
    /// 故事元信息
    pub story_title: String,
    pub story_genre: Option<String>,
    pub story_tone: Option<String>,
    pub story_pacing: Option<String>,
    pub story_description: Option<String>,
}

/// 合成结果
#[derive(Debug, Clone)]
pub struct SynthesisResult {
    /// 识别到的用户意图
    pub intent: String,
    /// LLM 选中的资产 ID 列表
    pub selected_asset_ids: Vec<String>,
    /// 合成后的综合提示词
    pub synthesized_prompt: String,
    /// 是否需要 Call 2 精修
    pub needs_refinement: bool,
    /// 精修重点
    pub refinement_focus: Option<String>,
    /// 合成置信度
    pub confidence: f32,
    /// 是否为回退结果
    pub is_fallback: bool,
    /// v0.23.61: LLM 选择的提示词框架（方法论/质量门/条件注入器/额外提示词）
    pub framework_selections: Option<FrameworkSelections>,
}

/// v0.23.61: 最快模型选择的提示词框架。
///
/// Call 1 收到 `prompt_framework_catalog` 后，根据创作意图和题材
/// 选择适合的方法论家族、质量门类型、条件注入器和额外推荐提示词。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameworkSelections {
    /// 选中的方法论 ID（如 "snowflake", "hero_journey"）
    #[serde(default)]
    pub methodology: Option<String>,
    /// 选中的质量门 ID（如 "pipeline_review", "mini_review_system"）
    #[serde(default)]
    pub quality_gate: Option<String>,
    /// 应注入的条件约束提示词 ID 列表
    #[serde(default)]
    pub contextual_injectors: Vec<String>,
    /// 额外推荐的具体 prompt_id 列表（用于注入创作指导）
    #[serde(default)]
    pub prompt_hints: Vec<String>,
}

impl SynthesisResult {
    /// 构造回退结果
    pub fn fallback(bundle_prompt: String) -> Self {
        Self {
            intent: "unknown".into(),
            selected_asset_ids: vec![],
            synthesized_prompt: bundle_prompt,
            needs_refinement: false,
            refinement_focus: None,
            confidence: 0.0,
            is_fallback: true,
            framework_selections: None,
        }
    }
}
