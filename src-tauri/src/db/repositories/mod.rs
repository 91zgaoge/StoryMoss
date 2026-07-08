//! Repository 层

pub use chrono::Local;
pub use rusqlite::{params, OptionalExtension};
pub use serde::{Deserialize, Serialize};
pub use serde_json;
pub use uuid::Uuid;

pub use crate::db::{dto::*, models::*, traits::*, DbPool};

pub mod scene_repository;
pub use scene_repository::{SceneRepository, SceneUpdate};
pub mod scene_version_repository;
pub use scene_version_repository::SceneVersionRepository;
pub mod world_building_repository;
pub use world_building_repository::WorldBuildingRepository;
pub mod writing_style_repository;
pub use writing_style_repository::{WritingStyleRepository, WritingStyleUpdate};
pub mod studio_config_repository;
pub use studio_config_repository::{StudioConfigRepository, StudioConfigUpdate};
pub mod knowledge_graph_repository;
pub use knowledge_graph_repository::KnowledgeGraphRepository;
pub mod scene_annotation_repository;
pub use scene_annotation_repository::SceneAnnotationRepository;
pub mod text_annotation_repository;
pub use text_annotation_repository::TextAnnotationRepository;
pub mod story_summary_repository;
pub use story_summary_repository::StorySummaryRepository;
pub mod change_track_repository;
pub use change_track_repository::ChangeTrackRepository;
pub mod comment_thread_repository;
pub use comment_thread_repository::CommentThreadRepository;
pub mod story_style_config_repository;
pub use story_style_config_repository::StoryStyleConfigRepository;
pub mod style_dna_repository;
pub use style_dna_repository::StyleDnaRepository;
pub mod style_snapshot_repository;
pub use style_snapshot_repository::StyleSnapshotRepository;
pub mod user_feedback_repository;
pub use user_feedback_repository::{FeedbackStats, UserFeedbackRepository};
pub mod user_preference_repository;
pub use user_preference_repository::UserPreferenceRepository;
pub mod story_outline_repository;
pub use story_outline_repository::StoryOutlineRepository;
pub mod character_relationship_repository;
pub use character_relationship_repository::CharacterRelationshipRepository;
pub mod scene_character_repository;
pub use scene_character_repository::SceneCharacterRepository;
pub mod scene_divider_repository;
pub use scene_divider_repository::SceneDividerRepository;
pub mod story_repository;
pub use story_repository::StoryRepository;
pub mod character_repository;
pub use character_repository::CharacterRepository;
pub mod chapter_repository;
pub use chapter_repository::ChapterRepository;
pub mod user_repository;
pub use user_repository::UserRepository;
pub mod genesis_run_repository;
pub use genesis_run_repository::GenesisRunRepository;

// ==================== Trait Implementations ====================
use crate::db::traits::{
    ChapterRepo, CharacterRepo, SceneRepo, StoryRepo, WorldBuildingRepo, WritingStyleRepo,
};

impl SceneRepo for SceneRepository {
    fn create(
        &self,
        story_id: &str,
        sequence_number: i32,
        title: Option<&str>,
    ) -> Result<Scene, rusqlite::Error> {
        self.create(story_id, sequence_number, title)
    }
    fn get_by_id(&self, id: &str) -> Result<Option<Scene>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn get_by_chapter(&self, chapter_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
        self.get_by_chapter(chapter_id)
    }
    fn update(&self, id: &str, updates: &SceneUpdate) -> Result<usize, rusqlite::Error> {
        self.update(id, updates)
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
    fn update_sequence(&self, id: &str, new_sequence: i32) -> Result<usize, rusqlite::Error> {
        self.update_sequence(id, new_sequence)
    }
}

impl StoryRepo for StoryRepository {
    fn create(&self, req: CreateStoryRequest) -> Result<Story, rusqlite::Error> {
        self.create(req)
    }
    fn get_all(&self) -> Result<Vec<Story>, rusqlite::Error> {
        self.get_all()
    }
    fn get_by_id(&self, id: &str) -> Result<Option<Story>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn update(&self, id: &str, req: &UpdateStoryRequest) -> Result<usize, rusqlite::Error> {
        self.update(id, req)
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
}

impl CharacterRepo for CharacterRepository {
    fn create(&self, req: CreateCharacterRequest) -> Result<Character, rusqlite::Error> {
        self.create(req)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Vec<Character>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn get_by_id(&self, id: &str) -> Result<Option<Character>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn update(
        &self,
        id: &str,
        name: Option<String>,
        background: Option<String>,
        personality: Option<String>,
        goals: Option<String>,
        appearance: Option<String>,
        gender: Option<String>,
        age: Option<i32>,
    ) -> Result<usize, rusqlite::Error> {
        self.update(
            id,
            name,
            background,
            personality,
            goals,
            appearance,
            gender,
            age,
        )
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
}

impl ChapterRepo for ChapterRepository {
    fn create(&self, req: CreateChapterRequest) -> Result<Chapter, rusqlite::Error> {
        self.create(req)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Vec<Chapter>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn get_by_id(&self, id: &str) -> Result<Option<Chapter>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn update(
        &self,
        id: &str,
        title: Option<String>,
        outline: Option<String>,
        word_count: Option<i32>,
    ) -> Result<usize, rusqlite::Error> {
        self.update(id, title, outline, word_count)
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
}

impl WorldBuildingRepo for WorldBuildingRepository {
    fn create(&self, story_id: &str, concept: &str) -> Result<WorldBuilding, rusqlite::Error> {
        self.create(story_id, concept)
    }
    fn get_by_id(&self, id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn update(
        &self,
        id: &str,
        concept: Option<&str>,
        rules: Option<&[WorldRule]>,
        history: Option<&str>,
        cultures: Option<&[Culture]>,
    ) -> Result<usize, rusqlite::Error> {
        self.update(id, concept, rules, history, cultures)
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
}

impl WritingStyleRepo for WritingStyleRepository {
    fn create(&self, story_id: &str, name: Option<&str>) -> Result<WritingStyle, rusqlite::Error> {
        self.create(story_id, name)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Option<WritingStyle>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn update(&self, id: &str, updates: &WritingStyleUpdate) -> Result<usize, rusqlite::Error> {
        self.update(id, updates)
    }
}

impl KnowledgeGraphRepo for KnowledgeGraphRepository {
    fn get_entities_by_story(&self, story_id: &str) -> Result<Vec<Entity>, rusqlite::Error> {
        self.get_entities_by_story(story_id)
    }
}

impl StoryOutlineRepo for StoryOutlineRepository {
    fn get_by_story(&self, story_id: &str) -> Result<Option<StoryOutline>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
}

impl StoryStyleConfigRepo for StoryStyleConfigRepository {
    fn get_active_by_story(
        &self,
        story_id: &str,
    ) -> Result<Option<StoryStyleConfig>, rusqlite::Error> {
        self.get_active_by_story(story_id)
    }
}

impl StyleDnaRepo for StyleDnaRepository {
    fn get_by_id(&self, id: &str) -> Result<Option<StyleDNA>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn get_many_by_ids(&self, ids: &[String]) -> Result<Vec<StyleDNA>, rusqlite::Error> {
        self.get_many_by_ids(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn story_req(title: &str) -> CreateStoryRequest {
        CreateStoryRequest {
            title: title.to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        }
    }

    #[test]
    fn test_scene_repository_create_and_get() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool);

        let story = story_repo.create(story_req("场景测试")).unwrap();
        let scene = scene_repo.create(&story.id, 1, Some("开场")).unwrap();

        assert_eq!(scene.sequence_number, 1);
        assert_eq!(scene.title.as_deref(), Some("开场"));

        let fetched = scene_repo.get_by_id(&scene.id).unwrap().unwrap();
        assert_eq!(fetched.id, scene.id);
        assert_eq!(fetched.story_id, story.id);
    }

    #[test]
    fn test_scene_repository_update_not_found_returns_zero() {
        let pool = create_test_pool().unwrap();
        let repo = SceneRepository::new(pool);

        let count = repo
            .update(
                "non-existent-scene-id",
                &SceneUpdate {
                    title: Some("新标题".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(count, 0);
    }

    #[test]
    fn test_scene_repository_create_for_missing_story_fails() {
        let pool = create_test_pool().unwrap();
        let repo = SceneRepository::new(pool);

        let result = repo.create("missing-story-id", 1, Some("开场"));
        assert!(
            result.is_err(),
            "expected foreign-key error for missing story"
        );
    }
}
