pub mod connection;
pub mod dto;
pub mod migrations;
pub mod models;
pub mod repositories;
pub mod repositories_change_track;
pub mod repositories_chapter;
pub mod repositories_character;
pub mod repositories_comment_thread;
pub mod repositories_export;
pub mod repositories_knowledge_graph;
pub mod repositories_narrative;
pub mod repositories_narrative_events;
pub mod repositories_pipeline;
pub mod repositories_scene;
pub mod repositories_scene_annotation;
pub mod repositories_scene_version;
pub mod repositories_story;
pub mod repositories_story_summary;
pub mod repositories_story_system;
pub mod repositories_studio_config;
pub mod repositories_text_annotation;
pub mod repositories_world_building;
pub mod repositories_writing_style;
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
#[allow(unused_imports)]
pub use repositories_change_track::*;
#[allow(unused_imports)]
pub use repositories_chapter::*;
#[allow(unused_imports)]
pub use repositories_character::*;
#[allow(unused_imports)]
pub use repositories_comment_thread::*;
#[allow(unused_imports)]
pub use repositories_export::*;
#[allow(unused_imports)]
pub use repositories_knowledge_graph::*;
#[allow(unused_imports)]
pub use repositories_pipeline::*;
#[allow(unused_imports)]
pub use repositories_scene::*;
#[allow(unused_imports)]
pub use repositories_scene_annotation::*;
#[allow(unused_imports)]
pub use repositories_scene_version::*;
#[allow(unused_imports)]
pub use repositories_story::*;
#[allow(unused_imports)]
pub use repositories_story_summary::*;
#[allow(unused_imports)]
pub use repositories_story_system::*;
#[allow(unused_imports)]
pub use repositories_studio_config::*;
#[allow(unused_imports)]
pub use repositories_text_annotation::*;
#[allow(unused_imports)]
pub use repositories_world_building::*;
#[allow(unused_imports)]
pub use repositories_writing_style::*;
pub use traits::{
    ChapterRepo, CharacterRepo, SceneRepo, StoryRepo, WorldBuildingRepo, WritingStyleRepo,
};
