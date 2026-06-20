//! SING (Synthetic Intention Graph) 意图图模块
//!
//! 基于 arXiv:2606.16591v2 的意图-工具异构图理论，实现：
//! - 意图合成（三阶段：Query Synthesis → Chain Expansion → Atomic Extraction）
//! - 分层发现（Server-level PPR + Tool-level 语义融合）
//! - 动态 ReAct（Discover → Invoke → Respond）
//! - 图传播评分（Personalized PageRank + 协同过滤）
//!
//! 核心目标：将用户自然语言意图准确映射到 StoryForge 的技能、方法论、风格等资产。

pub mod asset_sync;
pub mod builder;
pub mod commands;
pub mod context;
pub mod discovery;
pub mod graph;
pub mod models;
pub mod planner;
pub mod reactor;
pub mod scorer;

#[cfg(test)]
pub mod tests;

pub use asset_sync::{AssetSyncEngine, SyncStats};
pub use builder::{IntentSynthesisPipeline, SynthesizedQuery};
#[allow(unused_imports)]
pub use commands::{get_execution_graph_detail, get_intention_graph_diagnostics};
pub use context::IntentContext;
pub use models::{
    AssetNode, AssetType, cosine_similarity, deserialize_embedding, ExecutionGraph,
    ExecutionGraphStatus, ExecutionNode, ExecutionNodeStatus, IntentionAssetEdgeType,
    IntentionNode, IntentSynthesisResult, IntentType, serialize_embedding,
};
pub use planner::IntentionGraphPlanner;
#[allow(unused_imports)]
pub use reactor::{DynamicReactor, ReActAction};
pub use scorer::GraphScorer;
