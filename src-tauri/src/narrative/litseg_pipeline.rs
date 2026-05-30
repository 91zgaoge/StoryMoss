//! LitSeg 叙事分析流水线 — 在 ingest 完成后触发
//!
//! 依次执行：
//! 1. ThreadTracker 推断线索 → 保存 narrative_threads
//! 2. NarrativeStructureAnalyzer 分析幕结构 → 保存 narrative_structure +
//!    positions
//! 3. NarrativeChunker 生成文本块 → 保存 narrative_chunks

use std::{str::FromStr, sync::Arc};

use chrono::Local;

use crate::{
    db::{
        repositories_narrative_events::{
            NarrativeChunkRepository, NarrativeEventRepository,
            NarrativeStructurePositionRepository, NarrativeStructureRepository,
            NarrativeThreadRepository,
        },
        DbPool,
    },
    llm::LlmService,
    narrative::{
        chunker::{NarrativeChunker, SceneRef},
        event::NarrativeEvent as DomainEvent,
        structure::NarrativeStructure as DomainStructure,
        structure_analyzer::NarrativeStructureAnalyzer,
        thread_tracker::ThreadTracker,
    },
};

/// 运行完整的叙事分析流水线
///
/// 在 ingest 完成后异步触发，失败不阻塞主流程。
pub async fn run_narrative_analysis(
    story_id: &str,
    pool: DbPool,
    _llm_service: Option<Arc<LlmService>>,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("[NarrativePipeline] 开始叙事分析: story_id={}", story_id);

    // Step 1: 读取 narrative_events
    let event_repo = NarrativeEventRepository::new(pool.clone());
    let db_events = event_repo.get_by_story(story_id)?;
    if db_events.is_empty() {
        log::info!("[NarrativePipeline] 无叙事事件，跳过分析");
        return Ok(());
    }

    // 转换为域模型
    let events: Vec<DomainEvent> = db_events.into_iter().map(db_event_to_domain).collect();

    // Step 2: 推断叙事线索
    log::info!("[NarrativePipeline] 推断叙事线索...");
    let tracker = ThreadTracker;
    let threads = ThreadTracker::infer_threads(&events);
    save_threads(story_id, &threads, pool.clone())?;

    // Step 3: 分析叙事结构
    log::info!("[NarrativePipeline] 分析叙事结构...");
    let analyzer = NarrativeStructureAnalyzer::new();
    let structure = analyzer.analyze(story_id, &events);
    save_structure(story_id, &structure, pool.clone())?;

    // Step 4: 生成叙事感知文本块（需要场景数据）
    log::info!("[NarrativePipeline] 生成叙事感知文本块...");
    if let Ok(scenes) = fetch_scenes(story_id, pool.clone()) {
        let chunks = NarrativeChunker::chunk_story(story_id, &events, &scenes);
        save_chunks(story_id, &chunks, pool.clone())?;
    }

    log::info!("[NarrativePipeline] 叙事分析完成: story_id={}", story_id);
    Ok(())
}

// ==================== 转换函数 ====================

fn db_event_to_domain(db: crate::db::models::NarrativeEvent) -> DomainEvent {
    use crate::{db::ConflictType, narrative::event::EventType};

    DomainEvent {
        id: db.id,
        story_id: db.story_id,
        chapter_number: db.chapter_number,
        scene_id: db.scene_id,
        event_type: EventType::from_str(&db.event_type).unwrap_or_default(),
        intensity: db.intensity,
        sentiment: db.sentiment,
        description: db.description,
        involved_character_ids: serde_json::from_str(&db.involved_character_ids)
            .unwrap_or_default(),
        conflict_types: serde_json::from_str::<Vec<String>>(&db.conflict_types)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| ConflictType::from_str(&s).ok())
            .collect(),
        preceding_event_id: db.preceding_event_id,
        following_event_id: db.following_event_id,
        act_number: db.act_number,
        position_in_act: db.position_in_act,
        created_at: db.created_at.parse().unwrap_or_else(|_| Local::now()),
    }
}

// ==================== 保存函数 ====================

fn save_threads(
    story_id: &str,
    threads: &[crate::narrative::thread::NarrativeThread],
    pool: DbPool,
) -> Result<(), rusqlite::Error> {
    let repo = NarrativeThreadRepository::new(pool);
    for (idx, thread) in threads.iter().enumerate() {
        let (thread_type, target_id, thread_data) = match thread {
            crate::narrative::thread::NarrativeThread::CharacterArc(t) => (
                "character_arc",
                t.character_id.clone(),
                serde_json::to_string(t).unwrap_or_default(),
            ),
            crate::narrative::thread::NarrativeThread::Foreshadow(t) => (
                "foreshadow",
                t.id.clone(),
                serde_json::to_string(t).unwrap_or_default(),
            ),
            crate::narrative::thread::NarrativeThread::ConflictEscalation(t) => (
                "conflict_escalation",
                format!("{:?}", t.conflict_type),
                serde_json::to_string(t).unwrap_or_default(),
            ),
        };
        let db_thread = crate::db::models::NarrativeThread {
            id: format!("nth_{}_{}", story_id, idx),
            story_id: story_id.to_string(),
            thread_type: thread_type.to_string(),
            target_id,
            thread_data,
            created_at: Local::now().to_rfc3339(),
        };
        if let Err(e) = repo.insert(&db_thread) {
            log::warn!("[NarrativePipeline] 保存线索失败: {}", e);
        }
    }
    Ok(())
}

fn save_structure(
    story_id: &str,
    structure: &DomainStructure,
    pool: DbPool,
) -> Result<(), rusqlite::Error> {
    let structure_repo = NarrativeStructureRepository::new(pool.clone());
    let position_repo = NarrativeStructurePositionRepository::new(pool);

    // 保存幕级划分
    for act in &structure.acts {
        let db_act = crate::db::models::NarrativeStructure {
            id: format!("{}_{}", story_id, act.act_number),
            story_id: story_id.to_string(),
            act_number: act.act_number,
            act_type: format!("{:?}", act.act_type).to_lowercase(),
            start_chapter: act.start_chapter,
            end_chapter: act.end_chapter,
            summary: None,
            created_at: Local::now().to_rfc3339(),
        };
        if let Err(e) = structure_repo.insert(&db_act) {
            log::warn!("[NarrativePipeline] 保存幕结构失败: {}", e);
        }
    }

    // 保存事件位置
    // TODO: 需要从 structure_analyzer 中获取每个事件的 position 信息
    // 当前简化处理：只保存幕结构

    Ok(())
}

fn fetch_scenes(story_id: &str, pool: DbPool) -> Result<Vec<SceneRef>, rusqlite::Error> {
    use crate::db::repositories::SceneRepository;

    let repo = SceneRepository::new(pool);
    let scenes = repo.get_by_story(story_id)?;

    Ok(scenes
        .into_iter()
        .map(|s| SceneRef {
            id: s.id,
            chapter_number: s.sequence_number,
            content: s.content.unwrap_or_default(),
        })
        .collect())
}

fn save_chunks(
    story_id: &str,
    chunks: &[crate::narrative::segment::NarrativeChunk],
    pool: DbPool,
) -> Result<(), rusqlite::Error> {
    let repo = NarrativeChunkRepository::new(pool);
    for (idx, chunk) in chunks.iter().enumerate() {
        let db_chunk = crate::db::models::NarrativeChunk {
            id: format!("nch_{}_{}", story_id, idx),
            story_id: story_id.to_string(),
            chapter_range_start: chunk.chapter_range_start,
            chapter_range_end: chunk.chapter_range_end,
            scene_ids: serde_json::to_string(&chunk.scene_ids).unwrap_or_default(),
            event_ids: serde_json::to_string(&chunk.event_ids).unwrap_or_default(),
            text: chunk.text.clone(),
            chunk_type: format!("{:?}", chunk.chunk_type).to_lowercase(),
            is_boundary_start: chunk.is_boundary_start,
            is_boundary_end: chunk.is_boundary_end,
            thread_ids: serde_json::to_string(&chunk.thread_ids).unwrap_or_default(),
            vector_id: None,
            created_at: Local::now().to_rfc3339(),
        };
        if let Err(e) = repo.insert(&db_chunk) {
            log::warn!("[NarrativePipeline] 保存文本块失败: {}", e);
        }
    }
    Ok(())
}
