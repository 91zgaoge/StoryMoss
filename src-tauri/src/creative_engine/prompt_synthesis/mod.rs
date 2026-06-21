//! TriShot 提示词合成系统（v0.23）
//!
//! 把 WriteTimeBundle 的 ~17 段资产清单化为紧凑清单，用最快模型智能选择
//! 并合成连贯提示词，替代「笨拼接」。
//!
//! 设计依据：docs/plans/2026-06-21-trishot-pipeline-design.md

pub mod manifest;
pub mod refiner;
pub mod synthesizer;

#[cfg(test)]
mod integration_tests {
    use super::manifest::AssetManifest;
    use crate::creative_engine::write_time_bundle::{
        CoreCharacter, GenreCategory, StoryMeta, WriteTimeBundle,
    };

    /// 集成测试：清单构建 + 合成（不调 LLM）流程验证
    #[test]
    fn test_manifest_and_fallback() {
        let mut bundle = empty_bundle();
        bundle.contract_redlines = Some("测试红线".into());
        bundle.core_characters.push(CoreCharacter {
            name: "测试".into(),
            identity: None,
            physical_state: None,
            mental_state: None,
            location: None,
            personality: None,
        });
        bundle.overdue_foreshadowings.push("测试伏笔".into());

        let manifest = AssetManifest::build(&bundle);
        let bundle_prompt = bundle.to_prompt();

        // 验证清单包含三项
        assert_eq!(manifest.items.len(), 3);
        assert_eq!(manifest.items[0].id, "redline");

        // 验证回退结果可用
        let fallback =
            super::synthesizer::SynthesisResult::fallback(bundle_prompt.clone());
        assert!(fallback.is_fallback);
        assert_eq!(fallback.synthesized_prompt, bundle_prompt);
    }

    fn empty_bundle() -> WriteTimeBundle {
        WriteTimeBundle {
            contract_redlines: None,
            core_characters: vec![],
            scene_outline: None,
            genre_antipatterns: vec![],
            style_slice: None,
            story_meta: StoryMeta {
                title: "test".into(),
                genre: None,
                tone: None,
                pacing: None,
                description: None,
            },
            genre_category: GenreCategory::Unknown,
            narrative_phase_guidance: None,
            pending_foreshadowings: vec![],
            overdue_foreshadowings: vec![],
            style_dna_summary: None,
            narrative_quartet: None,
            style_dna_extension: None,
            methodology_extension: None,
            genre_profile_strategy: None,
            secondary_genre_profile_strategy: None,
            writing_strategy_constraints: None,
            runtime_contract: None,
            reference_scene_fewshots: vec![],
        }
    }
}
