use std::collections::HashMap;

use super::{
    models::{EntityChangeEvent, SceneImpact},
    repository::EntityMentionRepository,
};
use crate::{db::DbPool, error::AppError};

pub struct ImpactAnalyzer {
    mention_repo: EntityMentionRepository,
}

impl ImpactAnalyzer {
    pub fn new(pool: DbPool) -> Self {
        Self {
            mention_repo: EntityMentionRepository::new(pool),
        }
    }

    pub fn analyze(&self, change: &EntityChangeEvent) -> Result<Vec<SceneImpact>, AppError> {
        let mentions = self.mention_repo.get_by_entity(&change.entity_id)?;

        let mut scene_impacts: HashMap<String, SceneImpact> = HashMap::new();
        for mention in mentions {
            let impact = scene_impacts
                .entry(mention.scene_id.clone())
                .or_insert_with(|| SceneImpact::new(&mention.scene_id));
            impact.mention_count += 1;
            impact.confidence_sum += mention.confidence;
        }

        let mut results: Vec<SceneImpact> = scene_impacts.into_values().collect();
        results.sort_by(|a, b| {
            b.score()
                .partial_cmp(&a.score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }
}
