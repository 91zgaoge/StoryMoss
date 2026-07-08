//! 创作策略领域类型
//!
//! 定义模型选择出的创作策略以及用户手动覆盖项。
//! 这些类型被 strategy、commands、planner、narrative 等多个模块共享。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 模型选择出的创作策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectedStrategy {
    /// 选择理由
    #[serde(default, alias = "reasoning")]
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

    // ==================== v0.17.0 中文叙事增强 ====================
    /// 主情绪 / 读者爽点承诺（爽 / 甜 / 虐 / 恨 / 惊 / 燃 / 怕 / 痛 / 治愈 等）
    #[serde(default)]
    pub emotional_payoff: Option<String>,
    /// 高压关系 ID（不带前缀，13 选 1）
    #[serde(default)]
    pub pressure_relationship_id: Option<String>,
    /// 冲突场（公开审查 / 拍卖 / 法庭 / 家宴 / 直播 / 私密 等）
    #[serde(default)]
    pub conflict_arena: Option<String>,
    /// 选中的剧情引擎 ID 列表（建议 2-4 个，正交组合）
    #[serde(default)]
    pub story_engine_ids: Vec<String>,
    /// 选中的桥段卡 ID 列表（建议 1-2 张作骨架）
    #[serde(default)]
    pub beat_card_ids: Vec<String>,
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
            emotional_payoff: None,
            pressure_relationship_id: None,
            conflict_arena: None,
            story_engine_ids: Vec::new(),
            beat_card_ids: Vec::new(),
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
        // v0.17.0 中文叙事增强字段：用户锁定值优先
        if let Some(payoff) = &overrides.emotional_payoff {
            self.emotional_payoff = Some(payoff.clone());
        }
        if let Some(rel) = &overrides.pressure_relationship_id {
            self.pressure_relationship_id = Some(rel.clone());
        }
        if let Some(arena) = &overrides.conflict_arena {
            self.conflict_arena = Some(arena.clone());
        }
        if !overrides.story_engine_ids.is_empty() {
            self.story_engine_ids = overrides.story_engine_ids.clone();
        }
        if !overrides.beat_card_ids.is_empty() {
            self.beat_card_ids = overrides.beat_card_ids.clone();
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
    // v0.17.0 中文叙事增强：允许用户在 UI 中锁定四元组取值
    #[serde(default)]
    pub emotional_payoff: Option<String>,
    #[serde(default)]
    pub pressure_relationship_id: Option<String>,
    #[serde(default)]
    pub conflict_arena: Option<String>,
    #[serde(default)]
    pub story_engine_ids: Vec<String>,
    #[serde(default)]
    pub beat_card_ids: Vec<String>,
}
