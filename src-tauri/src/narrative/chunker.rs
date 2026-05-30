//! 叙事感知分段器 — LitSeg Phase 5
//!
//! 基于叙事结构生成文本块，在叙事边界处切分。
//! 每个 NarrativeChunk 是一个完整的叙事单元。

use crate::{
    db::ConflictType,
    narrative::{
        event::{EventType, NarrativeEvent},
        segment::{ChunkType, NarrativeChunk},
        structure::{Act, ActType, NarrativeStructure},
        structure_analyzer::NarrativeStructureAnalyzer,
    },
};

/// 叙事感知分段器
pub struct NarrativeChunker;

impl NarrativeChunker {
    /// 为故事生成叙事感知文本块
    pub fn chunk_story(
        story_id: &str,
        events: &[NarrativeEvent],
        scenes: &[SceneRef],
    ) -> Vec<NarrativeChunk> {
        if events.is_empty() || scenes.is_empty() {
            return vec![];
        }

        // Step 1: 分析叙事结构
        let analyzer = NarrativeStructureAnalyzer::new();
        let structure = analyzer.analyze(story_id, events);

        // Step 2: 按幕生成 chunk
        let mut chunks = Vec::new();
        for act in &structure.acts {
            let act_chunks = Self::chunk_act(story_id, act, events, scenes);
            chunks.extend(act_chunks);
        }

        chunks
    }

    /// 将幕级文本按叙事边界进一步细分
    fn chunk_act(
        story_id: &str,
        act: &Act,
        events: &[NarrativeEvent],
        scenes: &[SceneRef],
    ) -> Vec<NarrativeChunk> {
        // 筛选该幕的场景和事件
        let act_scenes: Vec<&SceneRef> = scenes
            .iter()
            .filter(|s| {
                s.chapter_number >= act.start_chapter && s.chapter_number <= act.end_chapter
            })
            .collect();

        let act_events: Vec<&NarrativeEvent> = events
            .iter()
            .filter(|e| {
                e.chapter_number >= act.start_chapter && e.chapter_number <= act.end_chapter
            })
            .collect();

        if act_scenes.is_empty() {
            return vec![];
        }

        // 在幕内查找叙事边界
        let boundaries = Self::find_narrative_boundaries_within_act(&act_events);

        if boundaries.is_empty() {
            // 无边界 → 整个幕作为一个 chunk
            vec![Self::make_chunk(
                story_id,
                act,
                &act_scenes,
                &act_events,
                ChunkType::from_act_type(&act.act_type),
                true,
                true,
            )]
        } else {
            // 按边界切分
            let mut chunks = Vec::new();
            let mut start_idx = 0;

            for boundary_idx in &boundaries {
                let chunk_scenes = &act_scenes[start_idx..=*boundary_idx];
                let chunk_events = &act_events[start_idx..=*boundary_idx];
                chunks.push(Self::make_chunk(
                    story_id,
                    act,
                    chunk_scenes,
                    chunk_events,
                    ChunkType::from_act_type(&act.act_type),
                    start_idx == 0, // 第一个 chunk 是边界起点
                    false,          // 非边界终点（除最后一个）
                ));
                start_idx = *boundary_idx + 1;
            }

            // 最后一个 chunk
            if start_idx < act_scenes.len() {
                let chunk_scenes = &act_scenes[start_idx..];
                let chunk_events = &act_events[start_idx..];
                if let Some(last) = chunks.last_mut() {
                    last.is_boundary_end = true;
                }
                chunks.push(Self::make_chunk(
                    story_id,
                    act,
                    chunk_scenes,
                    chunk_events,
                    ChunkType::from_act_type(&act.act_type),
                    false,
                    true,
                ));
            }

            chunks
        }
    }

    /// 在幕内查找叙事边界 — 基于事件强度突变
    fn find_narrative_boundaries_within_act(events: &[&NarrativeEvent]) -> Vec<usize> {
        if events.len() < 3 {
            return vec![];
        }

        let intensities: Vec<f32> = events.iter().map(|e| e.intensity).collect();
        if intensities.len() < 3 {
            return vec![];
        }

        // 计算强度差分
        let diffs: Vec<f32> = intensities
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .collect();

        if diffs.is_empty() {
            return vec![];
        }

        let avg_diff: f32 = diffs.iter().sum::<f32>() / diffs.len() as f32;
        let threshold = avg_diff * 1.5;

        diffs
            .iter()
            .enumerate()
            .filter(|(_, d)| **d > threshold)
            .map(|(i, _)| i)
            .collect()
    }

    fn make_chunk(
        story_id: &str,
        act: &Act,
        scenes: &[&SceneRef],
        events: &[&NarrativeEvent],
        chunk_type: ChunkType,
        is_boundary_start: bool,
        is_boundary_end: bool,
    ) -> NarrativeChunk {
        let scene_ids: Vec<String> = scenes.iter().map(|s| s.id.clone()).collect();
        let event_ids: Vec<String> = events.iter().map(|e| e.id.clone()).collect();

        // 聚合文本
        let text = scenes
            .iter()
            .map(|s| s.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let chapter_range_start = scenes.first().map(|s| s.chapter_number).unwrap_or(1);
        let chapter_range_end = scenes.last().map(|s| s.chapter_number).unwrap_or(1);

        NarrativeChunk {
            id: format!(
                "chunk_{}_{}_{}",
                story_id, chapter_range_start, chapter_range_end
            ),
            story_id: story_id.to_string(),
            chapter_range_start,
            chapter_range_end,
            scene_ids,
            event_ids,
            text,
            chunk_type,
            is_boundary_start,
            is_boundary_end,
            thread_ids: vec![],
            created_at: chrono::Local::now(),
        }
    }
}

/// 场景引用（轻量级，用于 chunker）
pub struct SceneRef {
    pub id: String,
    pub chapter_number: i32,
    pub content: String,
}

impl ChunkType {
    fn from_act_type(act_type: &ActType) -> Self {
        match act_type {
            ActType::Introduction => ChunkType::Introduction,
            ActType::Development => ChunkType::Development,
            ActType::Turn => ChunkType::Turn,
            ActType::Resolution => ChunkType::Resolution,
        }
    }
}
