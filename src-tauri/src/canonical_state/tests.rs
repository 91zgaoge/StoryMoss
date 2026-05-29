//! Canonical State 单元测试

use super::*;
use crate::db::{
    connection::create_test_pool,
    repositories::{
        CharacterRepository, SceneRepository, StoryRepository, WorldBuildingRepository,
    },
    CreateCharacterRequest, CreateStoryRequest,
};

fn block_on<F>(f: F) -> F::Output
where
    F: std::future::Future,
{
    tokio::runtime::Runtime::new().unwrap().block_on(f)
}

#[test]
fn test_pool_basic() {
    let pool = create_test_pool().unwrap();
    let _conn = pool.get().unwrap();
}

#[test]
fn test_story_repo_basic() {
    let pool = create_test_pool().unwrap();
    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试故事".to_string(),
            description: None,
            genre: Some("奇幻".to_string()),
            style_dna_id: None,
        })
        .unwrap();
    let fetched = story_repo.get_by_id(&story.id).unwrap();
    assert!(fetched.is_some());
}

#[test]
fn test_create_snapshot_empty_story() {
    let pool = create_test_pool().unwrap();
    let manager = CanonicalStateManager::new(pool.clone());

    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试故事".to_string(),
            description: None,
            genre: Some("奇幻".to_string()),
            style_dna_id: None,
        })
        .unwrap();

    let snapshot = block_on(manager.create_snapshot(&story.id)).unwrap();

    assert_eq!(snapshot.story_id, story.id);
    assert!(snapshot.character_states.is_empty());
    assert!(snapshot.world_facts.is_empty());
    assert!(snapshot.timeline.is_empty());
    assert_eq!(snapshot.narrative_phase, NarrativePhase::Setup);
}

#[test]
fn test_create_snapshot_with_scenes() {
    let pool = create_test_pool().unwrap();
    let manager = CanonicalStateManager::new(pool.clone());

    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试故事".to_string(),
            description: None,
            genre: Some("奇幻".to_string()),
            style_dna_id: None,
        })
        .unwrap();

    let scene_repo = SceneRepository::new(pool.clone());
    for i in 1..=10 {
        scene_repo
            .create(&story.id, i, Some(&format!("场景{}", i)))
            .unwrap();
    }

    let snapshot = block_on(manager.create_snapshot(&story.id)).unwrap();

    assert_eq!(snapshot.timeline.len(), 10);
    assert!(snapshot.story_context.current_scene_id.is_some());
    // 10 场景 → Setup 期 (0-15)
    assert_eq!(snapshot.narrative_phase, NarrativePhase::Setup);
}

#[test]
fn test_create_snapshot_with_characters() {
    let pool = create_test_pool().unwrap();
    let manager = CanonicalStateManager::new(pool.clone());

    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试故事".to_string(),
            description: None,
            genre: Some("奇幻".to_string()),
            style_dna_id: None,
        })
        .unwrap();

    let char_repo = CharacterRepository::new(pool.clone());
    let character = char_repo
        .create(CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "张三".to_string(),
            background: Some("主角".to_string()),
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,
        })
        .unwrap();

    let state = CharacterStateSnapshot {
        character_id: character.id.clone(),
        name: "张三".to_string(),
        current_location: Some("北京".to_string()),
        current_emotion: Some("愤怒".to_string()),
        active_goal: Some("复仇".to_string()),
        secrets_known: vec!["知道真相".to_string()],
        secrets_unknown: vec!["身世的秘密".to_string()],
        arc_progress: 0.5,
    };
    block_on(manager.update_character_state(&story.id, &character.id, state)).unwrap();

    let snapshot = block_on(manager.create_snapshot(&story.id)).unwrap();

    assert_eq!(snapshot.character_states.len(), 1);
    let cs = &snapshot.character_states[0];
    assert_eq!(cs.name, "张三");
    assert_eq!(cs.current_location, Some("北京".to_string()));
    assert_eq!(cs.current_emotion, Some("愤怒".to_string()));
    assert_eq!(cs.arc_progress, 0.5);
}

#[test]
fn test_create_snapshot_with_world_facts() {
    let pool = create_test_pool().unwrap();
    let manager = CanonicalStateManager::new(pool.clone());

    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试故事".to_string(),
            description: None,
            genre: Some("奇幻".to_string()),
            style_dna_id: None,
        })
        .unwrap();

    let wb_repo = WorldBuildingRepository::new(pool.clone());
    wb_repo.create(&story.id, "修仙世界").unwrap();

    let snapshot = block_on(manager.create_snapshot(&story.id)).unwrap();

    assert!(!snapshot.world_facts.is_empty());
    let setting_fact = snapshot
        .world_facts
        .iter()
        .find(|f| f.fact_type == "setting");
    assert!(setting_fact.is_some());
}

#[test]
fn test_narrative_phase_calculation() {
    let pool = create_test_pool().unwrap();
    let manager = CanonicalStateManager::new(pool.clone());

    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试故事".to_string(),
            description: None,
            genre: Some("奇幻".to_string()),
            style_dna_id: None,
        })
        .unwrap();

    let scene_repo = SceneRepository::new(pool.clone());

    // 20 个场景 → Rising 期 (16-70)
    for i in 1..=20 {
        scene_repo
            .create(&story.id, i, Some(&format!("场景{}", i)))
            .unwrap();
    }

    let snapshot = block_on(manager.create_snapshot(&story.id)).unwrap();
    assert_eq!(snapshot.narrative_phase, NarrativePhase::Rising);
}

#[test]
fn test_get_snapshot_returns_same_as_create() {
    let pool = create_test_pool().unwrap();
    let manager = CanonicalStateManager::new(pool.clone());

    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试故事".to_string(),
            description: None,
            genre: Some("奇幻".to_string()),
            style_dna_id: None,
        })
        .unwrap();

    let snapshot1 = block_on(manager.get_snapshot(&story.id)).unwrap();
    let snapshot2 = block_on(manager.create_snapshot(&story.id)).unwrap();

    assert_eq!(snapshot1.story_id, snapshot2.story_id);
    assert_eq!(snapshot1.narrative_phase, snapshot2.narrative_phase);
}

#[test]
fn test_narrative_phase_climax_detection() {
    let pool = create_test_pool().unwrap();
    let manager = CanonicalStateManager::new(pool.clone());

    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试故事".to_string(),
            description: None,
            genre: Some("奇幻".to_string()),
            style_dna_id: None,
        })
        .unwrap();

    let scene_repo = SceneRepository::new(pool.clone());
    // 创建 35 个场景，其中最近 3 个有高置信度和长内容
    for i in 1..=35 {
        let mut scene = scene_repo
            .create(&story.id, i, Some(&format!("场景{}", i)))
            .unwrap();
        if i > 32 {
            use crate::db::repositories::SceneUpdate;
            let long_content = "这是一段非常长的内容，超过了1000个字符的限制，\
                                因此需要多次重复以确保内容长度达到要求。"
                .repeat(25);
            let _ = scene_repo.update(
                &scene.id,
                &SceneUpdate {
                    content: Some(long_content.clone()),
                    confidence_score: Some(0.85),
                    ..Default::default()
                },
            );
            // 更新本地对象以便断言
            scene.content = Some(long_content);
            scene.confidence_score = Some(0.85);
        }
    }

    let snapshot = block_on(manager.create_snapshot(&story.id)).unwrap();
    // 35 个场景且最近 3 个高置信度+长内容 → 应检测为高潮期
    assert_eq!(snapshot.narrative_phase, NarrativePhase::Climax);
}

#[test]
fn test_narrative_phase_resolution_when_all_major_payoffs_resolved() {
    let pool = create_test_pool().unwrap();
    let manager = CanonicalStateManager::new(pool.clone());

    let story_repo = StoryRepository::new(pool.clone());
    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试故事".to_string(),
            description: None,
            genre: Some("奇幻".to_string()),
            style_dna_id: None,
        })
        .unwrap();

    let scene_repo = SceneRepository::new(pool.clone());
    // 创建 55 个场景（足够触发 Resolution 的场景数阈值）
    for i in 1..=55 {
        scene_repo
            .create(&story.id, i, Some(&format!("场景{}", i)))
            .unwrap();
    }

    // 创建一些低重要性伏笔（importance < 7，不构成主要伏笔），setup_scene
    // 设为最近场景避免逾期
    let tracker = crate::creative_engine::foreshadowing::ForeshadowingTracker::new(pool.clone());
    let scenes = scene_repo.get_by_story(&story.id).unwrap();
    let recent_scene_id = scenes.last().map(|s| s.id.as_str());
    tracker
        .add_foreshadowing(&story.id, "一个低重要性伏笔", recent_scene_id, 3)
        .unwrap();

    let snapshot = block_on(manager.create_snapshot(&story.id)).unwrap();
    // 有低重要性伏笔但无主要伏笔未回收，场景数 >= 50 → Resolution
    assert_eq!(snapshot.narrative_phase, NarrativePhase::Resolution);
}
