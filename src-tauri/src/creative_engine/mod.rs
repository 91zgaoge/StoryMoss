//! Creative Engine - 智能化创作引擎
//!
//! 提供创作上下文构建、故事连续性管理、方法论驱动等核心能力。
//! 所有模块在幕后运行，为 Agent 提供真实、完整的创作上下文。

pub mod adapter;
pub mod adaptive;
pub mod asset_capability_manifest;
pub mod asset_snapshot;
pub mod beat_cards;
pub mod cascade_rewriter;
pub mod context_builder;
pub mod context_prioritizer;
pub mod continuity;
pub mod foreshadowing;
pub mod methodology;
pub mod payoff_ledger;
pub mod pressure_relationships;
pub mod prompt_synthesis;
pub mod reader_promise;
pub mod story_engines;
pub mod style;
pub mod workflow;
pub mod write_time_bundle;

pub use context_builder::StoryContextBuilder;
