//! LitSeg 叙事分析流水线 — 在 ingest 完成后触发
//!
//! 从 scenes 表中的叙事字段提取事件，推断幕级结构，
//! 并将分析结果写回 story_outlines.analyzed_structure_json。

use std::str::FromStr;

use crate::{
    db::{DbPool, SceneRepository, StoryOutlineRepository},
    narrative::{
        event::{EventType, NarrativeEvent},
        structure_analyzer::NarrativeStructureAnalyzer,
    },
};

/// 运行完整的叙事分析流水线
///
/// 1. 读取 scenes 表中的 narrative_intensity / narrative_sentiment /
///    narrative_event_types / act_number / position_in_act。
/// 2. 构建 NarrativeEvent 列表。
/// 3. 调用 NarrativeStructureAnalyzer 推断幕级结构。
/// 4. 将结构 JSON 写回 story_outlines.analyzed_structure_json。
pub async fn run_narrative_analysis(
    story_id: &str,
    pool: DbPool,
    _llm_service: Option<std::sync::Arc<crate::llm::LlmService>>,
) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("[NarrativePipeline] 开始叙事分析: story_id={}", story_id);

    let scene_repo = SceneRepository::new(pool.clone());
    let scenes = scene_repo.get_by_story(story_id)?;

    // 把 scenes 表中的叙事字段转换为 NarrativeEvent
    let mut events: Vec<NarrativeEvent> = scenes
        .into_iter()
        .filter(|s| s.narrative_intensity.is_some())
        .map(|s| {
            let event_types: Vec<String> = s
                .narrative_event_types
                .as_ref()
                .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
                .unwrap_or_default();
            let event_type = event_types
                .first()
                .cloned()
                .and_then(|t| EventType::from_str(&t).ok())
                .unwrap_or(EventType::Transition);

            NarrativeEvent {
                id: s.id.clone(),
                story_id: s.story_id.clone(),
                chapter_number: s.sequence_number,
                scene_id: Some(s.id),
                event_type,
                intensity: s.narrative_intensity.unwrap_or(0.5),
                sentiment: s.narrative_sentiment.unwrap_or(0.0),
                description: s.title.unwrap_or_default(),
                involved_character_ids: s.characters_present,
                conflict_types: vec![],
                preceding_event_id: None,
                following_event_id: None,
                act_number: s.act_number.unwrap_or(1),
                position_in_act: s.position_in_act.unwrap_or(1),
                created_at: chrono::Local::now(),
            }
        })
        .collect();

    // 按章节号排序，保证时间线正确
    events.sort_by_key(|e| e.chapter_number);

    // 复用已有结构分析器
    let analyzer = NarrativeStructureAnalyzer::new();
    let structure = analyzer.analyze(story_id, &events);

    // 写回 story_outlines.analyzed_structure_json
    let outline_repo = StoryOutlineRepository::new(pool);
    let acts_json = serde_json::to_string(&structure.acts)?;
    let updated = outline_repo.update_analyzed_structure_json(story_id, &acts_json)?;
    if updated == 0 {
        // 若不存在 story_outlines 行，则创建一个占位行
        outline_repo.create(story_id, "", None, structure.acts.len() as i32, None)?;
        outline_repo.update_analyzed_structure_json(story_id, &acts_json)?;
    }

    log::info!(
        "[NarrativePipeline] 叙事分析完成: story_id={}, acts={}",
        story_id,
        structure.acts.len()
    );
    Ok(())
}
