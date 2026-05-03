use serde::{Deserialize, Serialize};
use chrono::{DateTime, Local};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub style_dna_id: Option<String>,
    pub methodology_id: Option<String>,
    pub methodology_step: Option<i32>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    pub id: String,
    pub story_id: String,
    pub name: String,
    pub background: Option<String>,
    pub personality: Option<String>,
    pub goals: Option<String>,
    pub appearance: Option<String>,
    pub gender: Option<String>,
    pub age: Option<i32>,
    pub dynamic_traits: Vec<DynamicTrait>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicTrait {
    #[serde(rename = "trait")]
    pub trait_name: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub story_id: String,
    pub chapter_number: i32,
    pub title: Option<String>,
    pub outline: Option<String>,
    pub content: Option<String>,
    pub word_count: Option<i32>,
    pub model_used: Option<String>,
    pub cost: Option<f64>,
    /// 关联的场景ID (v5.1.0 - Chapter↔Scene双轨映射)
    pub scene_id: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
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

// ==================== Auth Models (v4.5.0) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub is_local_user: bool,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthAccount {
    pub id: String,
    pub user_id: String,
    pub provider: String,
    pub provider_account_id: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Local>>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub token: String,
    pub expires_at: DateTime<Local>,
    pub created_at: DateTime<Local>,
}

// Auth request/response models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthUrlResponse {
    pub auth_url: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub google_enabled: bool,
    pub github_enabled: bool,
    pub wechat_enabled: bool,
    pub qq_enabled: bool,
}

// ==================== Export Template Models (v5.4.0) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportTemplate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub format: String,
    pub template_content: String,
    pub is_builtin: bool,
    pub is_user_created: bool,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Deserialize)]
pub struct CreateExportTemplateRequest {
    pub name: String,
    pub description: Option<String>,
    pub format: String,
    pub template_content: String,
}

// ==================== AI Operation History Models (v5.4.0) ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiOperation {
    pub id: String,
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
    pub status: String,
    pub created_at: DateTime<Local>,
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
