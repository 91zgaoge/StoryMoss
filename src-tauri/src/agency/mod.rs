//! Agency：多代理创作框架（创世 2.0）。
//! 黑板模型 + ReAct 工具循环 + 三角色（主创/管理/编辑审计）。
//! 设计：docs/plans/2026-07-17-agency-multi-agent-framework-design.md

pub mod asset_refresh;
pub mod beat_card;
pub mod beat_state;
pub mod board;
pub mod budget;
pub mod bus;
pub mod commands;
pub mod continue_assets;
pub mod continue_director;
pub mod continue_freeze;
pub mod continue_loop;
pub mod coordinator;
pub mod emotional_ledger;
pub mod eval_harness;
pub mod gate;
pub mod graders;
pub mod learning;
pub mod materialize;
pub mod models;
pub mod observe;
pub mod persist;
pub mod prose_ground;
pub mod repository;
pub mod roles;
pub mod session;
pub mod tool_loop;
pub mod tools;

pub use models::*;

#[cfg(test)]
mod golden_harness;
#[cfg(test)]
mod tests;
