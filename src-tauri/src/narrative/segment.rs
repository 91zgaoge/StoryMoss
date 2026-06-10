//! 叙事感知分段模型 — LitSeg 叙事感知分段与检索增强 (E1)
//!
//! 基于 LitSeg 论文：在叙事边界处切分文本，而非均匀切分。
//! 每个 NarrativeChunk 是一个完整的叙事单元，自带上下文。

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// 文本块类型——用于检索时的语义加权
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ChunkType {
    /// 开场块——引入角色/世界观
    Introduction,
    /// 发展块——冲突升级
    Development,
    /// 转折块——局势逆转
    Turn,
    /// 高潮块——冲突顶点
    Climax,
    /// 回落块——紧张缓解
    Resolution,
    /// 过渡块——连接功能
    #[default]
    Transition,
}

impl std::fmt::Display for ChunkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ChunkType::Introduction => "开场",
            ChunkType::Development => "发展",
            ChunkType::Turn => "转折",
            ChunkType::Climax => "高潮",
            ChunkType::Resolution => "回落",
            ChunkType::Transition => "过渡",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for ChunkType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "introduction" | "开场" => Ok(ChunkType::Introduction),
            "development" | "发展" => Ok(ChunkType::Development),
            "turn" | "转折" => Ok(ChunkType::Turn),
            "climax" | "高潮" => Ok(ChunkType::Climax),
            "resolution" | "回落" => Ok(ChunkType::Resolution),
            "transition" | "过渡" => Ok(ChunkType::Transition),
            _ => Err(format!("Unknown chunk type: {}", s)),
        }
    }
}

/// 叙事感知文本块——LitSeg 的核心产出
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeChunk {
    pub id: String,
    pub story_id: String,
    /// 包含的章节范围（而非单个章节）
    pub chapter_range_start: i32,
    pub chapter_range_end: i32,
    /// 包含的场景 ID 列表
    pub scene_ids: Vec<String>,
    /// 包含的事件 ID 列表
    pub event_ids: Vec<String>,
    /// 文本内容
    pub text: String,
    /// 块的叙事类型（用于检索时加权）
    pub chunk_type: ChunkType,
    /// 是否叙事边界起点
    pub is_boundary_start: bool,
    /// 是否叙事边界终点
    pub is_boundary_end: bool,
    /// 关联的叙事线索 ID（指向 narrative_threads 表）
    pub thread_ids: Vec<String>,
    /// 时间戳
    pub created_at: DateTime<Local>,
}
