//! Methodology domain types.
//!
//! Pure data definitions shared across the creative engine and commands.
//! Methodology engines and behavior remain in
//! `crate::creative_engine::methodology`.

use serde::{Deserialize, Serialize};

/// 方法论类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MethodologyType {
    Snowflake,
    SceneStructure,
    HeroJourney,
    CharacterDepth,
    HighDensityWorldBuilding,
}

impl MethodologyType {
    pub fn name(&self) -> &'static str {
        match self {
            MethodologyType::Snowflake => "雪花写作法",
            MethodologyType::SceneStructure => "场景结构规范",
            MethodologyType::HeroJourney => "英雄之旅",
            MethodologyType::CharacterDepth => "人物深度模型",
            MethodologyType::HighDensityWorldBuilding => "高密度世界构建法",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            MethodologyType::Snowflake => "从一句话逐步扩展为完整小说的十步创作法",
            MethodologyType::SceneStructure => "目标-冲突-灾难-反应-困境-决定六节拍场景结构",
            MethodologyType::HeroJourney => "约瑟夫·坎普贝尔的12阶段英雄之旅结构",
            MethodologyType::CharacterDepth => "目标-动机-冲突-秘密-弧光-顿悟六维人物模型",
            MethodologyType::HighDensityWorldBuilding => {
                "用极少元素通过状态驱动、桥节点连接、事件回流构建活的世界"
            }
        }
    }
}

/// 方法论配置（存储于数据库或配置中）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MethodologyConfig {
    pub methodology_type: MethodologyType,
    pub is_active: bool,
    pub current_step: Option<String>,
    pub custom_params: serde_json::Value,
}

impl Default for MethodologyConfig {
    fn default() -> Self {
        Self {
            methodology_type: MethodologyType::SceneStructure,
            is_active: true,
            current_step: None,
            custom_params: serde_json::json!({}),
        }
    }
}

/// 将别名规范为 Strategy / Genesis / WriteTimeBundle 使用的 canonical id。
/// `world_building` / `hdwb` → `high_density_world_building`。
pub fn normalize_methodology_id(id: &str) -> &str {
    match id.trim() {
        "world_building" | "hdwb" | "high_density_world_building" => "high_density_world_building",
        other => other,
    }
}

/// 创世结束后写入 `stories.methodology_step` 的保守推进值。
// Task 8 保留：唯一消费者（旧创世管线）已删除
#[allow(dead_code)]
pub fn final_methodology_step_after_genesis(methodology_id: &str) -> i32 {
    match normalize_methodology_id(methodology_id) {
        "snowflake" => 4,
        "high_density_world_building" => 2,
        _ => 1,
    }
}

/// Genesis 步骤 → 方法论子步/阶段 hint（供 resolve_methodology_prompt）。
// Task 8 保留：唯一消费者（旧创世管线）已删除
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenesisMethodStep {
    OpeningOrFirstChapter,
    World,
    Outline,
    Character,
    Scene,
    Foreshadow,
}

// Task 8 保留：唯一消费者（旧创世管线）已删除
#[allow(dead_code)]
pub fn methodology_step_hint(
    methodology_id: &str,
    step: GenesisMethodStep,
) -> Option<&'static str> {
    match (normalize_methodology_id(methodology_id), step) {
        ("snowflake", GenesisMethodStep::OpeningOrFirstChapter) => Some("1"),
        ("snowflake", GenesisMethodStep::Outline) => Some("2"),
        ("snowflake", GenesisMethodStep::Character) => Some("3"),
        ("snowflake", GenesisMethodStep::Scene) => Some("8"),
        ("high_density_world_building", GenesisMethodStep::World) => Some("1"),
        ("high_density_world_building", GenesisMethodStep::Outline)
        | ("high_density_world_building", GenesisMethodStep::Character) => Some("2"),
        _ => None,
    }
}

#[cfg(test)]
mod methodology_id_tests {
    use super::*;

    #[test]
    fn normalize_aliases_hdwb() {
        assert_eq!(
            normalize_methodology_id("world_building"),
            "high_density_world_building"
        );
        assert_eq!(
            normalize_methodology_id("hdwb"),
            "high_density_world_building"
        );
        assert_eq!(
            normalize_methodology_id("high_density_world_building"),
            "high_density_world_building"
        );
        assert_eq!(normalize_methodology_id("snowflake"), "snowflake");
    }

    #[test]
    fn final_step_mapping() {
        assert_eq!(final_methodology_step_after_genesis("snowflake"), 4);
        assert_eq!(final_methodology_step_after_genesis("world_building"), 2);
        assert_eq!(final_methodology_step_after_genesis("hero_journey"), 1);
    }

    #[test]
    fn step_hint_mapping() {
        assert_eq!(
            methodology_step_hint("snowflake", GenesisMethodStep::Scene),
            Some("8")
        );
        assert_eq!(
            methodology_step_hint("hdwb", GenesisMethodStep::World),
            Some("1")
        );
        assert_eq!(
            methodology_step_hint("hero_journey", GenesisMethodStep::World),
            None
        );
    }
}
