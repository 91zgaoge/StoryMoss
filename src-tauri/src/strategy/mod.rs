//! 能力发现与策略选择层
//!
//! 把技能、方法论、体裁画像、Style DNA、Workflow 统一为可被发现与选择的资产，
//! 供 Agency 框架、Planner、Writer 在创作过程中自动调用。

pub mod asset_catalog;
pub mod genre_resolver;
pub mod models;
pub mod quartet_inference;
pub mod selector;

pub use asset_catalog::load_all_assets;
pub use genre_resolver::{GenreMatch, GenreResolver};
pub use models::{
    AssetKind, SelectableAsset, SelectedStrategy, SelectionContext, StrategyOverrides,
};
pub use quartet_inference::infer_narrative_quartet;
pub use selector::StrategySelector;
