//! StoryMoss Model Gateway — v0.14.0
//!
//! 模型网关统一接管所有 LLM 调用方的路由、模型健康探测、任务分配与 fallback。
//!
//! 模块结构：
//! - `types`:   通用类型（健康快照、路由决策、网关状态等）
//! - `health`:  模型健康探测与注册表
//! - `registry`: 网关视角的模型注册表
//! - `dispatcher`: 任务分类与复杂度评估
//! - `executor`: 候选链执行与 fallback
//! - `commands`: 暴露给前端的 Tauri 命令

pub mod benchmark;
pub mod capability_store;
pub mod commands;
pub mod dispatcher;
pub mod executor;
pub mod health;
pub mod registry;
pub mod scheduler;
pub mod types;
pub mod upgrader;

#[allow(unused_imports)]
pub use commands::*;
#[allow(unused_imports)]
pub use dispatcher::*;
#[allow(unused_imports)]
pub use executor::*;
#[allow(unused_imports)]
pub use health::*;
#[allow(unused_imports)]
pub use registry::*;
#[allow(unused_imports)]
pub use scheduler::*;
#[allow(unused_imports)]
pub use types::*;
