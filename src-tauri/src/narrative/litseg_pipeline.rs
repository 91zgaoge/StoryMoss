//! LitSeg 叙事分析流水线 — 在 ingest 完成后触发
//!
//! 注意: narrative_events / narrative_threads / narrative_structure 表已合并到现有表。
//! 此模块保留为向后兼容的存根，实际叙事分析在 book_deconstruction/executor.rs 中完成。

use crate::db::DbPool;

/// 运行完整的叙事分析流水线（存根实现）
///
/// 实际的 LitSeg 叙事分析已集成到拆书流程中：
/// - narrative_intensity / narrative_sentiment → scenes 表
/// - 幕结构 → story_outlines.analyzed_structure_json
/// - 角色弧光 → character_states
pub async fn run_narrative_analysis(
    story_id: &str,
    _pool: DbPool,
    _llm_service: Option<std::sync::Arc<crate::llm::LlmService>>,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!(
        "[NarrativePipeline] 叙事分析已集成到拆书流程，跳过独立运行: story_id={}",
        story_id
    );
    Ok(())
}
