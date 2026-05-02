//! 创作方法论提示词模板库
//!
//! 将经典创作方法论编码为可复用的提示词模板：
//! - 雪花写作法 (Snowflake Method): 从一句话到完整小说的10步渐进细化
//! - 英雄之旅 (Hero's Journey): 约瑟夫·坎贝尔的12阶段单一体神话结构
//! - 场景结构 (Scene Structure): Swain 的场景-续接模型

pub mod snowflake;
pub mod hero_journey;
pub mod scene_structure;

/// 所有支持的方法论
pub enum Methodology {
    Snowflake,
    HeroJourney,
    SceneStructure,
}

impl Methodology {
    pub fn name(&self) -> &'static str {
        match self {
            Methodology::Snowflake => "雪花写作法",
            Methodology::HeroJourney => "英雄之旅",
            Methodology::SceneStructure => "场景结构",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Methodology::Snowflake => "从一句话到完整小说的10步渐进细化方法论",
            Methodology::HeroJourney => "约瑟夫·坎贝尔的12阶段单一体神话结构",
            Methodology::SceneStructure => "Dwight V. Swain 的场景-续接叙事模型",
        }
    }
}
