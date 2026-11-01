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

pub use compile::{compile_chapter_contract, compile_scene_contract};
pub use render::{render_chapter_contract, render_scene_brief, render_scene_tail};
pub use types::{ChapterContract, ContractError, EmotionChain, SceneContract, StateDelta};
pub use verify::{verify_contract, ContractStatus, ContractVerification, FieldResult};

/// P0 回滚开关：`STORYMOSS_NARRATIVE_CONTRACT=0` 时关闭契约编译与核对。
/// 默认开启；正式灰度开关（设置页）在 M5 接入前先以环境变量兜底。
pub fn enabled() -> bool {
    std::env::var("STORYMOSS_NARRATIVE_CONTRACT")
        .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}
