//! Memory pack domain types.
//!
//! These types are shared between the memory system and agent/creative modules.
//! Keeping them in the domain layer breaks the `memory ↔ agents` cycle.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 记忆包
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPack {
    pub working_memory: Vec<MemoryEntry>,
    pub episodic_memory: Vec<MemoryEntry>,
    pub semantic_memory: Vec<MemoryItemDto>,
    pub long_term_facts: Vec<MemoryItemDto>,
    pub active_constraints: Vec<MemoryItemDto>,
    pub recent_changes: Vec<HashMap<String, serde_json::Value>>,
    pub warnings: Vec<MemoryWarning>,
    pub stats: MemoryStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub layer: String,
    pub source: String,
    pub chapter: i32,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItemDto {
    pub id: String,
    pub category: String,
    pub subject: Option<String>,
    pub field: Option<String>,
    pub value: Option<String>,
    pub source_chapter: Option<i32>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryWarning {
    pub warning_type: String,
    pub count: usize,
    pub sample: Vec<MemoryItemDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total: usize,
    pub working_total: usize,
    pub episodic_total: usize,
    pub semantic_total: usize,
    pub injected: usize,
    pub layered_total_injected: usize,
    pub filtered: usize,
    pub conflicts: usize,
}
