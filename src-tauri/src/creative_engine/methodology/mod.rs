#![allow(dead_code)]
//! 创作方法论引擎
//!
//! 将经典创作方法论编码为系统级提示词规范，
//! 在幕后配置，幕前无感知自动应用。
//!
//! 支持的方法论：
//! - 雪花写作法 (Snowflake)
//! - 场景结构规范 (Scene Structure:
//!   Goal-Conflict-Disaster-Reaction-Dilemma-Decision)
//! - 英雄之旅 (Hero's Journey)
//! - 人物深度模型 (Character Depth)

pub mod character_depth;
pub mod hero_journey;
pub mod high_density_world_building;
pub mod scene_structure;
pub mod snowflake;

pub use character_depth::CharacterDepthModel;
pub use hero_journey::{HeroJourneyMethodology, HeroJourneyStage};
pub use high_density_world_building::{HighDensityWorldBuildingMethodology, WorldBuildingPhase};
pub use scene_structure::SceneStructureMethodology;
pub use snowflake::{SnowflakeMethodology, SnowflakeStep};

pub use crate::domain::methodology::{MethodologyConfig, MethodologyType};

/// 方法论 trait - 所有方法论必须实现
pub trait Methodology: Send + Sync {
    /// 方法论名称
    fn name(&self) -> &'static str;
    /// 方法论描述
    fn description(&self) -> &'static str;
    /// 获取该方法论的 system prompt 片段
    fn system_prompt_extension(&self) -> String;
    /// 获取输出格式要求（JSON Schema 或文本描述）
    fn output_schema(&self) -> Option<String>;
    /// 获取该方法论的当前阶段/步骤（如有）
    fn current_step(&self) -> Option<String>;
}

/// 方法论引擎 - 根据配置选择并应用方法论
pub struct MethodologyEngine;

impl MethodologyEngine {
    /// 根据配置生成 system prompt 扩展
    pub fn build_prompt_extension(config: &MethodologyConfig) -> String {
        if !config.is_active {
            return String::new();
        }

        let methodology: Box<dyn Methodology> = match config.methodology_type {
            MethodologyType::Snowflake => {
                let step = config
                    .current_step
                    .as_ref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(SnowflakeStep::OneSentence);
                Box::new(SnowflakeMethodology::new(step))
            }
            MethodologyType::SceneStructure => Box::new(SceneStructureMethodology::default()),
            MethodologyType::HeroJourney => {
                let stage = config
                    .current_step
                    .as_ref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(HeroJourneyStage::OrdinaryWorld);
                Box::new(HeroJourneyMethodology::new(stage))
            }
            MethodologyType::CharacterDepth => Box::new(CharacterDepthModel::default()),
            MethodologyType::HighDensityWorldBuilding => {
                let phase = config
                    .current_step
                    .as_ref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(WorldBuildingPhase::Seed);
                Box::new(HighDensityWorldBuildingMethodology::new(phase))
            }
        };

        methodology.system_prompt_extension()
    }

    /// 获取所有可用方法论列表
    pub fn list_available() -> Vec<MethodologyType> {
        vec![
            MethodologyType::Snowflake,
            MethodologyType::SceneStructure,
            MethodologyType::HeroJourney,
            MethodologyType::CharacterDepth,
            MethodologyType::HighDensityWorldBuilding,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_methodology_type_name() {
        assert_eq!(MethodologyType::Snowflake.name(), "雪花写作法");
        assert_eq!(MethodologyType::SceneStructure.name(), "场景结构规范");
        assert_eq!(MethodologyType::HeroJourney.name(), "英雄之旅");
        assert_eq!(MethodologyType::CharacterDepth.name(), "人物深度模型");
        assert_eq!(
            MethodologyType::HighDensityWorldBuilding.name(),
            "高密度世界构建法"
        );
    }

    #[test]
    fn test_list_available() {
        let list = MethodologyEngine::list_available();
        assert_eq!(list.len(), 5);
    }

    #[test]
    fn test_build_prompt_extension_scene_structure() {
        let config = MethodologyConfig {
            methodology_type: MethodologyType::SceneStructure,
            is_active: true,
            current_step: None,
            custom_params: serde_json::json!({}),
        };
        let ext = MethodologyEngine::build_prompt_extension(&config);
        assert!(ext.contains("目标场景"));
        assert!(ext.contains("反应场景"));
    }

    #[test]
    fn test_build_prompt_extension_inactive() {
        let config = MethodologyConfig {
            methodology_type: MethodologyType::Snowflake,
            is_active: false,
            current_step: None,
            custom_params: serde_json::json!({}),
        };
        let ext = MethodologyEngine::build_prompt_extension(&config);
        assert!(ext.is_empty());
    }
}
