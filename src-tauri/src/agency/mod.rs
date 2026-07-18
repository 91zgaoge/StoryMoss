//! Agency：多代理创作框架（创世 2.0）。
//! 黑板模型 + ReAct 工具循环 + 三角色（主创/管理/编辑审计）。
//! 设计：docs/plans/2026-07-17-agency-multi-agent-framework-design.md

pub mod board;
pub mod budget;
pub mod bus;
pub mod commands;
pub mod coordinator;
pub mod eval_harness;
pub mod gate;
pub mod graders;
pub mod materialize;
pub mod models;
pub mod repository;
pub mod roles;
pub mod session;
pub mod tool_loop;
pub mod tools;

pub use models::*;
