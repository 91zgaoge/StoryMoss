use serde::{Deserialize, Serialize};

/// 语料列表项。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusSummary {
    pub id: String,
    pub story_id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub tags: Vec<String>,
    pub status: String,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 语料完整详情（含原文、提炼结果、关联资产）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusDetail {
    pub id: String,
    pub story_id: String,
    pub kind: String,
    pub title: String,
    pub summary: String,
    pub content: String,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub tags: Vec<String>,
    pub status: String,
    pub pinned: bool,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub distilled: Option<CorpusDistillation>,
    pub links: Vec<CorpusLink>,
    pub created_at: String,
    pub updated_at: String,
}

/// 语料与资产的溯源链接。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusLink {
    pub id: String,
    pub corpus_id: String,
    pub target_type: String,
    pub target_id: String,
    pub note: Option<String>,
    pub created_at: String,
}

/// AI 提炼结果（distilled_json）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorpusDistillation {
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub conflicts: Vec<String>,
    #[serde(default)]
    pub archetypes: Vec<String>,
    #[serde(default)]
    pub world_hooks: Vec<String>,
    #[serde(default)]
    pub adaptation_angles: Vec<String>,
    #[serde(default)]
    pub cautions: Vec<String>,
}

/// 转为资产的草稿（不直接落库；前端预填现有编辑入口）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusPromotionDraft {
    pub target_type: String,
    pub title: String,
    pub fields: serde_json::Value,
}

/// 语料更新载荷。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CorpusUpdate {
    pub kind: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub source_url: Option<String>,
    pub source_name: Option<String>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub pinned: Option<bool>,
}

/// 图片预览载荷。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusMedia {
    pub mime: String,
    pub data_url: String,
}
