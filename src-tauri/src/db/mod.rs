pub mod chapter_split;
pub mod character_identity;
pub mod connection;
pub mod dto;
pub mod migrations;
pub mod models;
pub mod repositories;
pub mod repositories_export;
pub mod repositories_narrative;
pub mod repositories_narrative_events;
pub mod repositories_pipeline;
pub mod repositories_story_system;
pub mod traits;

#[cfg(test)]
#[path = "repositories_tests.rs"]
mod repositories_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub use connection::create_test_pool;
pub use connection::{init_db, DbPool};
pub use dto::*;
pub use models::*;
pub use repositories::*;
pub use repositories_export::*;
pub use repositories_pipeline::*;
pub use repositories_story_system::*;
pub use traits::{
    ChapterRepo, CharacterRepo, SceneRepo, StoryRepo, WorldBuildingRepo, WritingStyleRepo,
};
