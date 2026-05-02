//! Narrative Element Model — 统一叙事元素模型
//!
//! 核心理念：无论正向生成（Bootstrap/创世）还是逆向分析（拆书），
//! 操作的叙事元素是同一套抽象。
//!
//! 模块结构：
//! - elements: 统一数据模型（CharacterElement, SceneElement 等）
//! - pipeline: Pipeline trait 和通用基础设施
//! - prompts: 统一 Prompt 模板（生成/提取两用）
//! - genesis: GenesisPipeline — 正向/创世流程
//! - analysis: AnalysisPipeline — 逆向/分析流程
//! - progress: 统一进度事件系统

pub mod elements;
pub mod pipeline;
pub mod prompts;
pub mod genesis;
pub mod analysis;
pub mod progress;
pub mod audit;
pub mod health;

// pub use elements::*;
// pub use pipeline::*;
// pub use progress::*;
