//! 叙事契约编译器（P0：确定性场契约）。
//!
//! - `compile`：节拍卡 → `SceneContract`
//! - `render`：`SceneContract` → writer 的一页场简报
//! - `verify`：正文 → 逐字段证据核对
//!
//! 章契约（LLM 生成）在 M3 接入；本模块不新增每拍 LLM 调用。

pub mod compile;
pub mod render;
pub mod types;
pub mod verify;

pub use compile::compile_scene_contract;
pub use render::{render_scene_brief, render_scene_tail};
pub use types::{ContractError, EmotionChain, SceneContract, StateDelta};
pub use verify::{verify_contract, ContractStatus, ContractVerification, FieldResult};
