#![allow(unused_imports)]
//! StoryMoss 中性领域类型模块
//!
//! 本模块只包含纯数据结构（DTO / Value Objects /
//! Entities），不依赖任何业务模块。 目标：切断 `db`、`narrative`、
//! `creative_engine`、`agents`、`memory` 等模块之间的
//! 循环依赖，让它们可以只依赖类型而非互相引用。
//!
//! 迁入本模块的类型应满足：
//! 1. 被两个及以上业务模块共享；
//! 2. 不承载复杂业务逻辑（行为应下沉到对应领域服务）；
//! 3. 不依赖任何 `crate::` 业务模块，仅依赖 std / serde 等基础库。

pub mod adaptive;
pub mod agent_context;
pub mod agent_service;
pub mod agent_types;
pub mod asset_snapshot;
pub mod continuity;
pub mod contracts;
pub mod creative_engine;
pub mod foreshadowing;
pub mod memory_pack;
pub mod methodology;
pub mod narrative_elements;
pub mod novel_creation;
pub mod prompt_synthesis;
pub mod search;
pub mod strategy;
pub mod style;
pub mod subscription;
pub mod write_time_bundle;

pub use adaptive::*;
pub use agent_context::*;
pub use agent_service::*;
pub use agent_types::*;
pub use asset_snapshot::*;
pub use continuity::*;
pub use contracts::*;
pub use creative_engine::*;
pub use foreshadowing::*;
pub use memory_pack::*;
pub use methodology::*;
pub use narrative_elements::*;
pub use novel_creation::*;
pub use prompt_synthesis::*;
pub use search::*;
pub use strategy::*;
pub use style::*;
pub use subscription::*;
pub use write_time_bundle::*;
