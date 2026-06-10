//! StoryStateManager — RESERVED FOR FUTURE USE (Phase 4)
//!
//! This module provides a runtime story state manager with character arcs,
//! plot progression, and world state tracking. It is currently NOT integrated
//! into the active creative flow and overlaps with CanonicalStateManager +
//! StateSync.
//!
//! Re-enable when a dedicated runtime state machine is needed.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError};

/// Global story state manager
pub struct StoryStateManager {
    pool: DbPool,
    current_story_id: Arc<Mutex<Option<String>>>,
}

/// Complete story state for runtime
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryState {
    pub story_id: String,
    pub story_info: StoryInfo,
    pub characters: HashMap<String, CharacterState>,
    pub chapters: Vec<ChapterState>,
    pub plot_progression: PlotProgression,
    pub world_state: WorldState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryInfo {
    pub title: String,
    pub description: Option<String>,
    pub genre: String,
    pub tone: String,
    pub pacing: String,
    pub target_chapters: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterState {
    pub id: String,
    pub name: String,
    pub arc_progress: f32, // 0.0 - 1.0
    pub current_emotion: String,
    pub relationships: HashMap<String, Relationship>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub target_id: String,
    pub affinity: f32, // -1.0 to 1.0
    pub relationship_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterState {
    pub id: String,
    pub number: u32,
    pub title: Option<String>,
    pub status: ChapterStatus,
    pub word_count: u32,
    pub key_events: Vec<String>,
    pub pov_character: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChapterStatus {
    Planned,
    Outlined,
    Writing,
    Completed,
    Revising,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotProgression {
    pub current_arc: String,
    pub tension_level: f32, // 0.0 - 1.0
    pub plot_points_hit: Vec<String>,
    pub foreshadowing_queue: Vec<String>,
    pub unresolved_conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldState {
    pub locations: HashMap<String, LocationState>,
    pub lore_elements: Vec<LoreElement>,
    pub timeline: Vec<TimelineEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationState {
    pub id: String,
    pub name: String,
    pub current_occupants: Vec<String>,
    pub atmosphere: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoreElement {
    pub id: String,
    pub name: String,
    pub content: String,
    pub is_revealed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: String,
    pub description: String,
    pub timestamp: DateTime<Utc>,
    pub chapter_id: Option<String>,
}

impl StoryStateManager {
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool,
            current_story_id: Arc::new(Mutex::new(None)),
        }
    }

    pub fn create_state(&self, story_id: String, info: StoryInfo) -> Result<StoryState, AppError> {
        let state = StoryState {
            story_id: story_id.clone(),
            story_info: info,
            characters: HashMap::new(),
            chapters: Vec::new(),
            plot_progression: PlotProgression {
                current_arc: "introduction".to_string(),
                tension_level: 0.0,
                plot_points_hit: Vec::new(),
                foreshadowing_queue: Vec::new(),
                unresolved_conflicts: Vec::new(),
            },
            world_state: WorldState {
                locations: HashMap::new(),
                lore_elements: Vec::new(),
                timeline: Vec::new(),
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.save_state(&state)?;
        *self.current_story_id.lock().unwrap() = Some(story_id);
        Ok(state)
    }

    fn save_state(&self, state: &StoryState) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let state_json = serde_json::to_string(state).map_err(AppError::from)?;
        conn.execute(
            "INSERT OR REPLACE INTO story_runtime_states (id, story_id, state_json, updated_at)
             VALUES (
                 COALESCE((SELECT id FROM story_runtime_states WHERE story_id = ?1), ?2),
                 ?1, ?3, ?4
             )",
            params![
                &state.story_id,
                uuid::Uuid::new_v4().to_string(),
                state_json,
                Utc::now().to_rfc3339(),
            ],
        )
        .map_err(AppError::from)?;
        Ok(())
    }

    pub fn get_state(&self, story_id: &str) -> Result<Option<StoryState>, AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let mut stmt = conn
            .prepare("SELECT state_json FROM story_runtime_states WHERE story_id = ?1")
            .map_err(AppError::from)?;

        let result = stmt.query_row([story_id], |row| {
            let json: String = row.get(0)?;
            Ok(json)
        });

        match result {
            Ok(json) => {
                let state: StoryState = serde_json::from_str(&json).map_err(AppError::from)?;
                Ok(Some(state))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::from(e)),
        }
    }

    pub fn update_state(
        &self,
        story_id: &str,
        updater: impl FnOnce(&mut StoryState),
    ) -> Result<(), AppError> {
        let mut state = match self.get_state(story_id)? {
            Some(s) => s,
            None => return Err(AppError::internal("Story state not found")),
        };
        updater(&mut state);
        state.updated_at = Utc::now();
        self.save_state(&state)
    }

    pub fn set_current_story(&self, story_id: String) {
        *self.current_story_id.lock().unwrap() = Some(story_id);
    }

    pub fn get_current_story(&self) -> Option<String> {
        self.current_story_id.lock().unwrap().clone()
    }

    pub fn add_character(&self, story_id: &str, character: CharacterState) -> Result<(), AppError> {
        self.update_state(story_id, |state| {
            state.characters.insert(character.id.clone(), character);
        })
    }

    pub fn update_chapter_status(
        &self,
        story_id: &str,
        chapter_id: &str,
        status: ChapterStatus,
    ) -> Result<(), AppError> {
        self.update_state(story_id, |state| {
            if let Some(chapter) = state.chapters.iter_mut().find(|c| c.id == chapter_id) {
                chapter.status = status;
            }
        })
    }

    pub fn add_plot_point(&self, story_id: &str, point: String) -> Result<(), AppError> {
        self.update_state(story_id, |state| {
            state.plot_progression.plot_points_hit.push(point);
        })
    }

    pub fn get_all_states(&self) -> Result<Vec<StoryState>, AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let mut stmt = conn
            .prepare("SELECT state_json FROM story_runtime_states")
            .map_err(AppError::from)?;

        let rows = stmt
            .query_map([], |row| Ok(row.get::<_, String>(0)?))
            .map_err(AppError::from)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;

        rows.iter()
            .map(|json| serde_json::from_str(json).map_err(AppError::from))
            .collect()
    }
}

impl Default for StoryStateManager {
    fn default() -> Self {
        // Note: Default without pool is a placeholder.
        // Production code should always use StoryStateManager::new(pool).
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder().max_size(1).build(manager).unwrap();
        Self {
            pool,
            current_story_id: Arc::new(Mutex::new(None)),
        }
    }
}
