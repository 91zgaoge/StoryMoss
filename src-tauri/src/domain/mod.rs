#![allow(unused_imports)]
//! StoryForge 中性领域类型模块
//!
//! 本模块只包含纯数据结构（DTO / Value Objects / Entities），不依赖任何业务模块。
//! 目标：切断 `db`、`narrative`、`creative_engine`、`agents`、`memory` 等模块之间的
//! 循环依赖，让它们可以只依赖类型而非互相引用。
//!
//! 迁入本模块的类型应满足：
//! 1. 被两个及以上业务模块共享；
//! 2. 不承载复杂业务逻辑（行为应下沉到对应领域服务）；
//! 3. 不依赖任何 `crate::` 业务模块，仅依赖 std / serde 等基础库。

pub mod agent_context;
pub mod agent_types;
pub mod contracts;
pub mod memory_pack;
pub mod methodology;
pub mod narrative_elements;
pub mod novel_creation;
pub mod strategy;
pub mod style;
pub mod subscription;

pub use agent_context::*;
pub use agent_types::*;
pub use contracts::*;
pub use memory_pack::*;
pub use methodology::*;
pub use narrative_elements::*;
pub use novel_creation::*;
pub use strategy::*;
pub use style::*;
pub use subscription::*;
