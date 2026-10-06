//! Creative engine adapter implementing the neutral `CreativeEnginePort`.
//!
//! This keeps all concrete creative-engine behavior in `creative_engine` while
//! allowing `agents` to depend only on `domain` types/traits.

use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    db::DbPool,
    domain::{
        adaptive::GenerationStrategy,
        agent_context::NarrativeStructureContext,
        asset_snapshot::{ActiveConflict, AssetSnapshot, CharacterStateSnapshot},
        continuity::ConsistencyCheck,
        creative_engine::{CreativeEnginePort, RuntimeContractProvider},
        methodology::MethodologyConfig,
        prompt_synthesis::{AssetManifest, SynthesisResult},
        style::{CraftSliderHint, SanitizeOutcome, StyleBlendConfig, StyleCheckResult, StyleDNA},
        write_time_bundle::WriteTimeBundle,
    },
    error::AppError,
    ports::{LlmPort, VectorStore},
};

/// Adapter that owns a `DbPool` and injected ports, delegating to the
/// concrete creative-engine implementations.
pub struct CreativeEngineAdapter {
    pool: DbPool,
    llm_port: Arc<dyn LlmPort>,
    runtime_contract_provider: Option<Arc<dyn RuntimeContractProvider>>,
    #[allow(dead_code)]
    vector_store: Option<Arc<dyn VectorStore>>,
}

impl CreativeEngineAdapter {
    pub fn new(pool: DbPool, llm_port: Arc<dyn LlmPort>) -> Self {
        Self {
            pool,
            llm_port,
            runtime_contract_provider: None,
            vector_store: None,
        }
    }

    pub fn with_runtime_contract_provider(
        mut self,
        provider: Arc<dyn RuntimeContractProvider>,
    ) -> Self {
        self.runtime_contract_provider = Some(provider);
        self
    }

    pub fn with_vector_store(mut self, vector_store: Arc<dyn VectorStore>) -> Self {
        self.vector_store = Some(vector_store);
        self
    }

    fn runtime_contract_provider(&self) -> Arc<dyn RuntimeContractProvider> {
        self.runtime_contract_provider.clone().unwrap_or_else(|| {
            Arc::new(crate::story_system::StorySystemEngine::new(
                self.pool.clone(),
            ))
        })
    }
}

#[async_trait]
impl CreativeEnginePort for CreativeEngineAdapter {
    fn load_write_time_bundle(
        &self,
        story_id: &str,
        chapter_number: i32,
        style_slice_override: Option<String>,
        secondary_genre_profile_ids: Option<Vec<String>>,
    ) -> Result<WriteTimeBundle, AppError> {
        WriteTimeBundle::load_sync(
            &self.pool,
            story_id,
            chapter_number,
            style_slice_override,
            secondary_genre_profile_ids,
            Some(self.runtime_contract_provider()),
        )
        .map_err(AppError::from)
    }

    fn render_bundle_prompt(&self, bundle: &WriteTimeBundle) -> String {
        bundle.to_prompt()
    }

    async fn build_adaptive_strategy(
        &self,
        story_id: &str,
        base_temperature: Option<f32>,
        story_progress: Option<&str>,
        scene_stage: Option<&str>,
    ) -> Result<GenerationStrategy, AppError> {
        crate::creative_engine::adaptive::AdaptiveGenerator::new(self.pool.clone())
            .build_strategy_with_context(story_id, base_temperature, story_progress, scene_stage)
            .await
            .map_err(AppError::from)
    }

    fn check_scene_continuity(
        &self,
        story_id: &str,
        scene_id: &str,
        content: &str,
    ) -> Result<ConsistencyCheck, AppError> {
        crate::creative_engine::continuity::ContinuityEngine::new(self.pool.clone())
            .check_scene_continuity(story_id, scene_id, content)
            .map_err(AppError::from)
    }

    fn build_methodology_prompt_extension(&self, config: &MethodologyConfig) -> String {
        crate::creative_engine::methodology::MethodologyEngine::build_prompt_extension(
            config,
            Some(&self.pool),
        )
    }

    async fn build_prompt_personalizer_extension(
        &self,
        story_id: &str,
    ) -> Result<String, AppError> {
        crate::creative_engine::adaptive::PromptPersonalizer::new(self.pool.clone())
            .build_prompt_extension(story_id)
            .await
            .map_err(AppError::from)
    }

    fn sanitize_style_brief(&self, text: &str) -> SanitizeOutcome {
        crate::creative_engine::style::living_author_guard::sanitize_style_brief(text)
    }

    fn default_craft_sliders(&self) -> Vec<CraftSliderHint> {
        crate::creative_engine::style::living_author_guard::default_craft_sliders()
    }

    fn render_craft_sliders(&self, sliders: &[CraftSliderHint]) -> String {
        crate::creative_engine::style::living_author_guard::render_craft_sliders(sliders)
    }

    fn load_asset_snapshot(&self, story_id: &str, style_dna_id: Option<&str>) -> AssetSnapshot {
        // 用同一个 CanonicalStateManager 同时实现 ForeshadowingPort /
        // PayoffLedgerPort 并提供角色状态/活跃冲突快照。manager
        // 内部不再依赖 creative_engine， 因此 creative_engine ->
        // canonical_state 的单向依赖不再构成循环。
        let manager = Arc::new(crate::canonical_state::CanonicalStateManager::new(
            self.pool.clone(),
        ));
        let foreshadowing_port: Arc<dyn crate::domain::creative_engine::ForeshadowingPort> =
            manager.clone();
        let payoff_ledger_port: Arc<dyn crate::domain::creative_engine::PayoffLedgerPort> =
            manager.clone();

        let internal = crate::creative_engine::asset_snapshot::CreativeAssetSnapshot::load_sync(
            &self.pool,
            story_id,
            style_dna_id,
            foreshadowing_port,
            payoff_ledger_port,
        );

        let canonical = manager.get_snapshot_sync(story_id).ok();

        let narrative_phase_guidance = internal.narrative_phase_guidance();
        let pending_foreshadowings = internal.pending_foreshadowings(3);
        let overdue_foreshadowings = internal.overdue_foreshadowings(1);
        let style_dna_summary = internal.style_dna_summary;

        let (character_states, active_conflicts) = canonical
            .map(|c| {
                let chars = c
                    .character_states
                    .into_iter()
                    .map(|cs| CharacterStateSnapshot {
                        character_id: cs.character_id,
                        name: cs.name,
                        current_location: cs.current_location,
                        current_emotion: cs.current_emotion,
                        active_goal: cs.active_goal,
                        arc_progress: cs.arc_progress,
                        secrets_known: cs.secrets_known,
                        secrets_unknown: cs.secrets_unknown,
                    })
                    .collect();
                let conflicts = c
                    .story_context
                    .active_conflicts
                    .into_iter()
                    .map(|c| ActiveConflict {
                        conflict_type: c.conflict_type,
                        parties: c.parties,
                        stakes: c.stakes,
                    })
                    .collect();
                (chars, conflicts)
            })
            .unwrap_or_default();

        AssetSnapshot {
            narrative_phase_guidance,
            style_dna_summary,
            pending_foreshadowings,
            overdue_foreshadowings,
            character_states,
            active_conflicts,
        }
    }

    fn check_style(&self, text: &str, target: &StyleDNA) -> StyleCheckResult {
        crate::creative_engine::style::StyleChecker::check(text, target)
    }

    fn check_style_blend(
        &self,
        text: &str,
        blend: &StyleBlendConfig,
        dnas: &[StyleDNA],
    ) -> StyleCheckResult {
        crate::creative_engine::style::StyleChecker::check_blend(text, blend, dnas)
    }

    fn build_asset_manifest(&self, bundle: &WriteTimeBundle) -> AssetManifest {
        crate::creative_engine::prompt_synthesis::manifest::AssetManifest::build(bundle)
    }

    async fn synthesize_prompt(
        &self,
        instruction: &str,
        current_content_preview: Option<&str>,
        manifest: &AssetManifest,
        bundle_prompt: &str,
        asset_capability_summary: Option<&str>,
    ) -> SynthesisResult {
        crate::creative_engine::prompt_synthesis::synthesizer::PromptSynthesizer::synthesize(
            self.llm_port.clone(),
            instruction,
            current_content_preview,
            manifest,
            bundle_prompt,
            asset_capability_summary,
            Some(&self.pool),
        )
        .await
    }

    async fn refine_prompt(
        &self,
        synthesized_prompt: &str,
        refinement_focus: Option<&str>,
        story_title: &str,
        story_genre: Option<&str>,
        story_tone: Option<&str>,
    ) -> String {
        crate::creative_engine::prompt_synthesis::refiner::PromptRefiner::refine(
            self.llm_port.clone(),
            synthesized_prompt,
            refinement_focus,
            story_title,
            story_genre,
            story_tone,
            Some(&self.pool),
        )
        .await
    }

    async fn build_narrative_structure_context(
        &self,
        story_id: &str,
        chapter_number: Option<i32>,
    ) -> Result<NarrativeStructureContext, AppError> {
        crate::creative_engine::context_builder::StoryContextBuilder::new(self.pool.clone())
            .with_contract_provider(self.runtime_contract_provider())
            .build_narrative_structure_context_async(story_id, chapter_number)
            .await
            .map_err(AppError::from)
    }

    async fn fetch_active_threads(&self, story_id: &str) -> Result<Vec<String>, AppError> {
        crate::creative_engine::context_builder::StoryContextBuilder::new(self.pool.clone())
            .with_contract_provider(self.runtime_contract_provider())
            .fetch_active_threads_async(story_id)
            .await
            .map_err(AppError::from)
    }

    async fn build_narrative_event_history(
        &self,
        story_id: &str,
    ) -> Result<Option<String>, AppError> {
        crate::creative_engine::context_builder::StoryContextBuilder::new(self.pool.clone())
            .with_contract_provider(self.runtime_contract_provider())
            .build_narrative_event_history_async(story_id)
            .await
            .map_err(AppError::from)
    }

    fn get_foreshadowing_hints(
        &self,
        story_id: &str,
        limit: usize,
    ) -> Result<Vec<String>, AppError> {
        crate::creative_engine::foreshadowing::ForeshadowingTracker::new(self.pool.clone())
            .get_writing_hints(story_id, limit)
            .map_err(AppError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::creative_engine::CreativeEnginePort;

    #[test]
    fn _creative_engine_adapter_implements_port() {
        fn assert_port<T: CreativeEnginePort>() {}
        assert_port::<CreativeEngineAdapter>();
    }
}
