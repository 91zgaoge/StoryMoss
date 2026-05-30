//! 叙事感知检索增强 — LitSeg Phase 5
//!
//! 在标准混合检索（RRF）后增加叙事感知重排序层。

use crate::{
    memory::hybrid_search::HybridSearchResult,
    narrative::{segment::ChunkType, structure::ActType},
};

/// 叙事意图 — 查询的叙事阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NarrativeIntent {
    Introduction,
    Development,
    Turn,
    Climax,
    Resolution,
    General,
}

/// 叙事感知检索结果
#[derive(Debug, Clone)]
pub struct NarrativeSearchResult {
    pub id: String,
    pub content: String,
    pub score: f64,
    pub chunk_type: ChunkType,
    pub is_narrative_boundary: bool,
    pub narrative_boost: f64,
}

/// 叙事感知重排序器
pub struct NarrativeReRanker;

impl NarrativeReRanker {
    /// 对标准混合检索结果进行叙事感知重排序
    pub fn re_rank(
        base_results: Vec<HybridSearchResult>,
        story_progress: &str,
        current_act: Option<ActType>,
    ) -> Vec<NarrativeSearchResult> {
        let intent = Self::classify_narrative_intent(story_progress);

        let mut results: Vec<NarrativeSearchResult> = base_results
            .into_iter()
            .map(|r| NarrativeSearchResult {
                id: r.id,
                content: r.content,
                score: r.hybrid_score as f64,
                chunk_type: ChunkType::Transition,
                is_narrative_boundary: false,
                narrative_boost: 1.0,
            })
            .collect();

        for result in &mut results {
            // 1. 叙事边界优先（LitSeg 论文：+20%）
            if result.is_narrative_boundary {
                result.score *= 1.2;
                result.narrative_boost = result.narrative_boost.max(1.2);
            }

            // 2. 叙事结构一致性（LitSeg 论文：+15%）
            if Self::matches_intent(&result.chunk_type, intent) {
                result.score *= 1.15;
                result.narrative_boost = result.narrative_boost.max(1.15);
            }

            // 3. 同幕优先（LitSeg 论文：+15%）
            if let Some(ref act) = current_act {
                if result.chunk_type == chunk_type_from_act(act) {
                    result.score *= 1.15;
                    result.narrative_boost = result.narrative_boost.max(1.15);
                }
            }
        }

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    fn classify_narrative_intent(story_progress: &str) -> NarrativeIntent {
        match story_progress {
            "just_started" => NarrativeIntent::Introduction,
            "developing" => NarrativeIntent::Development,
            "midpoint" => NarrativeIntent::Turn,
            "climax" => NarrativeIntent::Climax,
            "resolution" => NarrativeIntent::Resolution,
            _ => NarrativeIntent::General,
        }
    }

    fn matches_intent(chunk_type: &ChunkType, intent: NarrativeIntent) -> bool {
        match (chunk_type, intent) {
            (ChunkType::Introduction, NarrativeIntent::Introduction) => true,
            (ChunkType::Development, NarrativeIntent::Development) => true,
            (ChunkType::Turn, NarrativeIntent::Turn) => true,
            (ChunkType::Climax, NarrativeIntent::Climax) => true,
            (ChunkType::Resolution, NarrativeIntent::Resolution) => true,
            _ => false,
        }
    }
}

fn chunk_type_from_act(act_type: &ActType) -> ChunkType {
    match act_type {
        ActType::Introduction => ChunkType::Introduction,
        ActType::Development => ChunkType::Development,
        ActType::Turn => ChunkType::Turn,
        ActType::Resolution => ChunkType::Resolution,
    }
}
