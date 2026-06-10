//! DTO (Data Transfer Object) 模块
//!
//! 包含请求/响应模型，从 models.rs 迁移至此，避免贫血模型与 DTO 混杂。

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

use crate::db::models::{
    CharacterConflict, Culture, Scene, SceneVersion, StudioConfig, WorldBuilding, WorldRule,
    WritingStyle,
};

// ==================== Request/Response 模型 ====================

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateSceneRequest {
    pub story_id: String,
    pub sequence_number: i32,
    pub title: Option<String>,
    pub dramatic_goal: Option<String>,
    pub external_pressure: Option<String>,
    pub conflict_type: Option<String>,
    pub characters_present: Vec<String>,
    pub setting_location: Option<String>,
    pub content: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UpdateSceneRequest {
    pub title: Option<String>,
    pub dramatic_goal: Option<String>,
    pub external_pressure: Option<String>,
    pub conflict_type: Option<String>,
    pub characters_present: Option<Vec<String>>,
    pub character_conflicts: Option<Vec<CharacterConflict>>,
    pub content: Option<String>,
    pub setting_location: Option<String>,
    pub setting_time: Option<String>,
    pub setting_atmosphere: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateWorldBuildingRequest {
    pub story_id: String,
    pub concept: String,
    pub rules: Option<Vec<WorldRule>>,
    pub history: Option<String>,
    pub cultures: Option<Vec<Culture>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateWritingStyleRequest {
    pub story_id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub vocabulary_level: Option<String>,
    pub sentence_structure: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct StudioExportRequest {
    pub story_id: String,
    pub include_world_building: bool,
    pub include_characters: bool,
    pub include_writing_style: bool,
    pub include_scenes: bool,
    pub include_llm_config: bool,
    pub include_ui_config: bool,
    pub include_agent_bots: bool,
}

#[derive(Debug, Serialize)]
pub struct StudioExportData {
    pub manifest: ExportManifest,
    pub story: crate::db::models::Story,
    pub world_building: Option<WorldBuilding>,
    pub characters: Vec<crate::db::models::Character>,
    pub writing_style: Option<WritingStyle>,
    pub scenes: Vec<Scene>,
    pub studio_config: Option<StudioConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportManifest {
    pub version: String,
    pub exported_at: DateTime<Local>,
    pub story_id: String,
    pub story_title: String,
}

// ==================== 场景版本相关请求/响应 (新增) ====================

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CreateSceneVersionRequest {
    pub scene_id: String,
    pub change_summary: String,
    pub created_by: String, // "user" | "ai" | "system"
    pub model_used: Option<String>,
    pub confidence_score: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct SceneVersionDiff {
    pub field: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CompareVersionsResponse {
    pub version_a: SceneVersion,
    pub version_b: SceneVersion,
    pub differences: Vec<SceneVersionDiff>,
}

// Request/Response models
#[derive(Debug, Deserialize)]
pub struct CreateStoryRequest {
    pub title: String,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub style_dna_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStoryRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub style_dna_id: Option<String>,
    pub methodology_id: Option<String>,
    pub methodology_step: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCharacterRequest {
    pub story_id: String,
    pub name: String,
    pub background: Option<String>,
    pub personality: Option<String>,
    pub goals: Option<String>,
    pub appearance: Option<String>,
    pub gender: Option<String>,
    pub age: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChapterRequest {
    pub story_id: String,
    pub chapter_number: i32,
    pub title: Option<String>,
    pub outline: Option<String>,
    pub content: Option<String>,
}

// Auth request/response models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExportTemplateRequest {
    pub name: String,
    pub description: Option<String>,
    pub format: String,
    pub template_content: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAiOperationRequest {
    pub story_id: String,
    pub scene_id: Option<String>,
    pub chapter_id: Option<String>,
    pub operation_type: String,
    pub operation_name: String,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
    pub previous_content: Option<String>,
    pub new_content: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBlueprintRequest {
    pub story_id: String,
    pub chapter_number: i32,
    pub title: Option<String>,
    pub role: Option<String>,
    pub purpose: Option<String>,
    pub key_events: Option<Vec<String>>,
    pub characters: Option<Vec<String>>,
    pub suspense_hook: Option<String>,
    pub user_guidance: Option<String>,
    pub knowledge_query_hint: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateBlueprintRequest {
    pub title: Option<String>,
    pub role: Option<String>,
    pub purpose: Option<String>,
    pub key_events: Option<Vec<String>>,
    pub characters: Option<Vec<String>>,
    pub suspense_hook: Option<String>,
    pub user_guidance: Option<String>,
    pub notes: Option<String>,
    pub knowledge_query_hint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RecordLlmCallRequest {
    pub story_id: Option<String>,
    pub draft_id: Option<String>,
    pub model_id: String,
    pub model_name: Option<String>,
    pub purpose: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub duration_ms: i32,
    pub success: bool,
    pub error_message: Option<String>,
}
