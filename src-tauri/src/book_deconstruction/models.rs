//! Book Deconstruction Models
//!
//! Data structures for the book deconstruction feature.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

// ==================== 分析状态 ====================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AnalysisStatus {
    Pending,
    Extracting,
    Analyzing,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AnalysisStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AnalysisStatus::Pending => "pending",
            AnalysisStatus::Extracting => "extracting",
            AnalysisStatus::Analyzing => "analyzing",
            AnalysisStatus::Completed => "completed",
            AnalysisStatus::Failed => "failed",
            AnalysisStatus::Cancelled => "cancelled",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for AnalysisStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(AnalysisStatus::Pending),
            "extracting" => Ok(AnalysisStatus::Extracting),
            "analyzing" => Ok(AnalysisStatus::Analyzing),
            "completed" => Ok(AnalysisStatus::Completed),
            "failed" => Ok(AnalysisStatus::Failed),
            "cancelled" => Ok(AnalysisStatus::Cancelled),
            _ => Err(format!("Unknown analysis status: {}", s)),
        }
    }
}

// ==================== 参考小说主表模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceBook {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub genre: Option<String>,
    pub word_count: Option<i64>,
    pub file_format: Option<String>,
    pub file_hash: Option<String>,
    pub file_path: Option<String>,
    pub world_setting: Option<String>,
    pub plot_summary: Option<String>,
    pub story_arc: Option<String>,
    // LitSeg: 分析后的叙事结构（起承转合幕级划分）
    pub analyzed_structure_json: Option<String>,
    pub analysis_status: AnalysisStatus,
    pub analysis_progress: i32,
    pub analysis_error: Option<String>,
    pub task_id: Option<String>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceBookSummary {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub genre: Option<String>,
    pub word_count: Option<i64>,
    pub file_format: Option<String>,
    pub analysis_status: String,
    pub analysis_progress: i32,
    pub created_at: String,
}

// ==================== 参考人物表模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceCharacter {
    pub id: String,
    pub book_id: String,
    pub name: String,
    pub role_type: Option<String>,
    pub personality: Option<String>,
    pub appearance: Option<String>,
    pub relationships: Option<String>,
    pub key_scenes: Option<String>,
    pub importance_score: Option<f32>,
    pub created_at: DateTime<Local>,
}

// ==================== 参考场景/章节表模型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceScene {
    pub id: String,
    pub book_id: String,
    pub sequence_number: i32,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub characters_present: Option<String>,
    pub key_events: Option<String>,
    pub conflict_type: Option<String>,
    pub emotional_tone: Option<String>,
    // LitSeg: 叙事分析字段
    pub narrative_intensity: Option<f32>,
    pub narrative_sentiment: Option<f32>,
    pub narrative_event_types: Option<String>, // JSON ["conflict_eruption", "character_arc"]
    pub act_number: Option<i32>,
    pub position_in_act: Option<f32>,
    pub created_at: DateTime<Local>,
}

// ==================== 分析结果聚合 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookAnalysisResult {
    pub book: ReferenceBook,
    pub characters: Vec<ReferenceCharacter>,
    pub scenes: Vec<ReferenceScene>,
}

// ==================== 分析进度事件 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookAnalysisProgressEvent {
    pub book_id: String,
    pub status: String,
    pub progress: i32,
    pub current_step: String,
    pub message: Option<String>,
    /// 当前活跃的 LLM 并发线程数
    #[serde(default)]
    pub active_threads: i32,
    /// 总文本块数
    #[serde(default)]
    pub total_chunks: i32,
    /// 已处理的文本块数
    #[serde(default)]
    pub processed_chunks: i32,
}

// ==================== 文件解析结果 ====================

#[derive(Debug, Clone)]
pub struct ParsedBook {
    pub title: Option<String>,
    pub author: Option<String>,
    pub chapters: Vec<ParsedChapter>,
    pub raw_text: String,
    pub word_count: usize,
}

#[derive(Debug, Clone)]
pub struct ParsedChapter {
    pub title: Option<String>,
    pub content: String,
    pub word_count: usize,
}

// ==================== 文本分块 ====================

#[derive(Debug, Clone)]
pub struct TextChunk {
    pub index: usize,
    pub title: Option<String>,
    pub content: String,
    pub word_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChunkingStrategy {
    Full,           // 短篇：全文一次处理
    ByChapters,     // 中篇：按章节分块
    NarrativeAware, // 长篇：叙事感知分块（章节边界+场景转换点）
    #[allow(dead_code)]
    MergedBlocks, // 保留兼容（旧版固定大小分块）
    #[allow(dead_code)]
    SampledBlocks, // 保留兼容（已不再使用）
}

// ==================== 错误类型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParseError {
    IoError(String),
    InvalidFormat(String),
    NoTextExtracted(String),
    EncodingError(String),
    FileTooLarge(String),
    StorageError(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::IoError(msg) => write!(f, "IO error: {}", msg),
            ParseError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
            ParseError::NoTextExtracted(msg) => write!(f, "No text extracted: {}", msg),
            ParseError::EncodingError(msg) => write!(f, "Encoding error: {}", msg),
            ParseError::FileTooLarge(msg) => write!(f, "File too large: {}", msg),
            ParseError::StorageError(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl std::error::Error for ParseError {}

// ==================== 分析错误类型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnalysisError {
    LlmError(String),
    ParseError(String),
    StorageError(String),
    Timeout(String),
    Cancelled(String),
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisError::LlmError(msg) => write!(f, "LLM error: {}", msg),
            AnalysisError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            AnalysisError::StorageError(msg) => write!(f, "Storage error: {}", msg),
            AnalysisError::Timeout(msg) => write!(f, "Timeout: {}", msg),
            AnalysisError::Cancelled(msg) => write!(f, "Cancelled: {}", msg),
        }
    }
}

impl std::error::Error for AnalysisError {}

// ==================== 元信息提取结果 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub genre: Option<String>,
    pub genre_tags: Vec<String>,
    pub estimated_word_count: Option<i64>,
}

// ==================== 人物提取结果 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedCharacter {
    pub name: String,
    pub role_type: Option<String>,
    pub personality: Option<String>,
    pub appearance: Option<String>,
    pub relationships: Vec<CharacterRelationship>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterRelationship {
    pub target_name: String,
    pub relation_type: String,
    pub description: Option<String>,
}

// ==================== 章节概要结果 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSceneSummary {
    pub sequence_number: i32,
    pub title: Option<String>,
    pub summary: String,
    pub characters_present: Vec<String>,
    pub key_events: Vec<String>,
    pub conflict_type: Option<String>,
    pub emotional_tone: Option<String>,
}

// ==================== 故事线结果 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedStoryArc {
    pub main_arc: String,
    pub sub_arcs: Vec<String>,
    pub climaxes: Vec<String>,
    pub turning_points: Vec<String>,
}

// ==================== LLM Prompt 响应类型 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCharacterResponse {
    pub characters: Vec<LlmCharacterItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCharacterItem {
    pub name: String,
    #[serde(rename = "role_type")]
    pub role_type: Option<String>,
    pub personality: Option<String>,
    pub appearance: Option<String>,
    pub relationships: Option<Vec<LlmRelationshipItem>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRelationshipItem {
    pub target: String,
    #[serde(rename = "type")]
    pub relation_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSceneSummaryResponse {
    pub summary: String,
    pub characters_present: Vec<String>,
    pub key_events: Vec<String>,
    pub conflict_type: Option<String>,
    pub emotional_tone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMetadataResponse {
    pub title: Option<String>,
    pub author: Option<String>,
    pub genre: Option<String>,
    pub genre_tags: Option<Vec<String>>,
    pub estimated_word_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmWorldSettingResponse {
    pub world_setting: String,
    pub power_system: Option<String>,
    pub social_structure: Option<String>,
    pub geography: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStoryArcResponse {
    pub main_arc: String,
    pub sub_arcs: Vec<String>,
    pub climaxes: Vec<String>,
    pub turning_points: Vec<String>,
}
