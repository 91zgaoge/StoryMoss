//! End-to-end test for `CreativeEnginePort` using in-memory fake ports.
//!
//! This test proves that the core creative flow can run against `FakeLlmPort`,
//! `FakeRuntimeContractProvider`, and `FakeVectorStore` without Tauri, a real
//! LLM, or a real vector database.

use std::sync::Arc;

use crate::{
    creative_engine::adapter::CreativeEngineAdapter,
    db::{
        connection::create_test_pool,
        repositories::{SceneRepository, StoryRepository},
        CreateStoryRequest, SceneUpdate,
    },
    domain::creative_engine::CreativeEnginePort,
    ports::{fake_runtime_contract, FakeLlmPort, FakeRuntimeContractProvider, FakeVectorStore},
};

#[tokio::test]
async fn creative_engine_port_e2e_with_fakes() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Prepare an isolated test database and seed a story + scene.
    let pool = create_test_pool()?;
    let story_repo = StoryRepository::new(pool.clone());
    let scene_repo = SceneRepository::new(pool.clone());

    let story = story_repo.create(CreateStoryRequest {
        title: "Fake Engine Test".to_string(),
        description: Some("Testing fake ports".to_string()),
        genre: Some("都市言情".to_string()),
        style_dna_id: None,
        genre_profile_id: None,
        methodology_id: None,
        reference_book_id: None,
    })?;

    let scene = scene_repo.create(&story.id, 1, Some("重逢"))?;
    scene_repo.update(
        &scene.id,
        &SceneUpdate {
            dramatic_goal: Some("主角在咖啡馆偶遇故人".to_string()),
            outline_content: Some("两人寒暄，暗藏旧情".to_string()),
            ..Default::default()
        },
    )?;

    // 2. Wire up the fakes.
    let fake_llm_response = r#"{"intent":"continue","selected_asset_ids":["scene_outline"],"synthesized_prompt":"FAKE_SYNTHESIZED_PROMPT_MARKER: 继续写两人重逢的场景","needs_refinement":false,"refinement_focus":null,"confidence":0.9}"#;
    let fake_llm = Arc::new(FakeLlmPort::new(fake_llm_response));
    let fake_contract = FakeRuntimeContractProvider::new().with_default(fake_runtime_contract());
    let fake_vector_store = Arc::new(FakeVectorStore::new());

    let engine = CreativeEngineAdapter::new(pool, fake_llm.clone())
        .with_runtime_contract_provider(Arc::new(fake_contract))
        .with_vector_store(fake_vector_store);

    // 3. Load the write-time bundle through the port.
    let bundle = engine.load_write_time_bundle(&story.id, 1, None, None)?;
    assert!(
        bundle.scene_outline.is_some(),
        "bundle should contain scene outline"
    );
    assert_eq!(
        bundle
            .scene_outline
            .as_ref()
            .unwrap()
            .dramatic_goal
            .as_deref(),
        Some("主角在咖啡馆偶遇故人"),
        "bundle should carry the scene dramatic goal"
    );
    assert!(
        bundle.runtime_contract.is_some(),
        "fake runtime contract should be present in the bundle"
    );

    // 4. Synthesize a prompt through the port; the fake LLM response should be
    //    parsed and returned instead of the local fallback.
    let manifest = engine.build_asset_manifest(&bundle);
    let bundle_prompt = engine.render_bundle_prompt(&bundle);
    let result = engine
        .synthesize_prompt("继续写", None, &manifest, &bundle_prompt, None)
        .await;

    assert!(
        result
            .synthesized_prompt
            .contains("FAKE_SYNTHESIZED_PROMPT_MARKER"),
        "synthesized prompt should contain the fake LLM response marker"
    );
    assert!(!result.is_fallback, "should not fall back to bundle prompt");

    // 5. The fake LLM should have been called at least once (probe +
    //    synthesis).
    assert!(
        !fake_llm.calls().is_empty(),
        "fake LLM should record at least one call"
    );

    Ok(())
}
