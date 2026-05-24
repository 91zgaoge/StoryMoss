//! Cascade Rewriter — 级联改写器
//!
//! 当用户在 Backstage 修改角色、世界观、故事线时，
//! 自动识别受影响场景，生成增量改写预览，经用户确认后应用。

mod change_detector;
mod impact_analyzer;
mod rewrite_engine;
pub mod models;
mod repository;
pub mod executor;
pub mod commands;

pub use repository::EntityMentionRepository;
