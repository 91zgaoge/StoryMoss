//! Novel creation domain types.
//!
//! Shared option types produced by the novel-creation agent.

use serde::{Deserialize, Serialize};

use crate::domain::narrative_elements::{Culture, WorldRule};

/// 世界观选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldBuildingOption {
    pub id: String,
    pub concept: String,
    pub rules: Vec<WorldRule>,
    pub history: String,
    pub cultures: Vec<Culture>,
}

/// 角色谱选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterProfileOption {
    pub id: String,
    pub name: String,
    pub personality: String,
    pub background: String,
    pub goals: String,
    pub voice_style: String,
}

/// 文字风格选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WritingStyleOption {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tone: String,
    pub pacing: String,
    pub vocabulary_level: String,
    pub sentence_structure: String,
    pub sample_text: String,
}
