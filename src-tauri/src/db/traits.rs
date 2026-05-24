//! Repository Trait 抽象层
//!
//! 目标：打破 `db` 上帝模块，上层依赖接口而非具体实现。
//! 每个 trait 的方法签名与对应 Repository 实现完全一致，
//! 确保零成本迁移。

use rusqlite;
use crate::db::{
    Scene, Story, Character, Chapter, WorldBuilding, WritingStyle,
    CreateStoryRequest, CreateCharacterRequest, CreateChapterRequest, UpdateStoryRequest,
    SceneUpdate, WritingStyleUpdate, WorldRule, Culture,
};

// ==================== Scene Repository Trait ====================

pub trait SceneRepo {
    fn create(&self, story_id: &str, sequence_number: i32, title: Option<&str>) -> Result<Scene, rusqlite::Error>;
    fn get_by_id(&self, id: &str) -> Result<Option<Scene>, rusqlite::Error>;
    fn get_by_story(&self, story_id: &str) -> Result<Vec<Scene>, rusqlite::Error>;
    fn get_by_chapter(&self, chapter_id: &str) -> Result<Vec<Scene>, rusqlite::Error>;
    fn update(&self, id: &str, updates: &SceneUpdate) -> Result<usize, rusqlite::Error>;
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error>;
    fn update_sequence(&self, id: &str, new_sequence: i32) -> Result<usize, rusqlite::Error>;
}

// ==================== Story Repository Trait ====================

pub trait StoryRepo {
    fn create(&self, req: CreateStoryRequest) -> Result<Story, rusqlite::Error>;
    fn get_all(&self) -> Result<Vec<Story>, rusqlite::Error>;
    fn get_by_id(&self, id: &str) -> Result<Option<Story>, rusqlite::Error>;
    fn update(&self, id: &str, req: &UpdateStoryRequest) -> Result<usize, rusqlite::Error>;
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error>;
}

// ==================== Character Repository Trait ====================

pub trait CharacterRepo {
    fn create(&self, req: CreateCharacterRequest) -> Result<Character, rusqlite::Error>;
    fn get_by_story(&self, story_id: &str) -> Result<Vec<Character>, rusqlite::Error>;
    fn get_by_id(&self, id: &str) -> Result<Option<Character>, rusqlite::Error>;
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
    ) -> Result<usize, rusqlite::Error>;
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error>;
}

// ==================== Chapter Repository Trait ====================

pub trait ChapterRepo {
    fn create(&self, req: CreateChapterRequest) -> Result<Chapter, rusqlite::Error>;
    fn get_by_story(&self, story_id: &str) -> Result<Vec<Chapter>, rusqlite::Error>;
    fn get_by_id(&self, id: &str) -> Result<Option<Chapter>, rusqlite::Error>;
    fn update(
        &self,
        id: &str,
        title: Option<String>,
        outline: Option<String>,
        content: Option<String>,
        word_count: Option<i32>,
    ) -> Result<usize, rusqlite::Error>;
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error>;
}

// ==================== WorldBuilding Repository Trait ====================

pub trait WorldBuildingRepo {
    fn create(&self, story_id: &str, concept: &str) -> Result<WorldBuilding, rusqlite::Error>;
    fn get_by_id(&self, id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error>;
    fn get_by_story(&self, story_id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error>;
    fn update(
        &self,
        id: &str,
        concept: Option<&str>,
        rules: Option<&[WorldRule]>,
        history: Option<&str>,
        cultures: Option<&[Culture]>,
    ) -> Result<usize, rusqlite::Error>;
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error>;
}

// ==================== WritingStyle Repository Trait ====================

pub trait WritingStyleRepo {
    fn create(&self, story_id: &str, name: Option<&str>) -> Result<WritingStyle, rusqlite::Error>;
    fn get_by_story(&self, story_id: &str) -> Result<Option<WritingStyle>, rusqlite::Error>;
    fn update(&self, id: &str, updates: &WritingStyleUpdate) -> Result<usize, rusqlite::Error>;
}

