//! LitSeg 叙事感知分段 — Tauri IPC 命令（深度融合后）
//!
//! 查询增强后的现有表：
//! - analyze_narrative_structure → story_outlines.analyzed_structure_json
//! - get_narrative_events → scenes.narrative_* 字段
//! - get_narrative_threads → foreshadowing_tracker + character_states
//! - get_narrative_chunks → narrative_chunks（物化缓存）

use crate::db::DbPool;

/// 获取故事的叙事结构分析（从 story_outlines.analyzed_structure_json）
#[tauri::command]
pub async fn analyze_narrative_structure(
    story_id: String,
    state: tauri::State<'_, DbPool>,
) -> Result<serde_json::Value, String> {
    use crate::db::repositories::StoryOutlineRepository;

    let repo = StoryOutlineRepository::new(state.inner().clone());
    match repo.get_by_story(&story_id) {
        Ok(Some(outline)) => {
            let structure = outline
                .analyzed_structure_json
                .as_ref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                .unwrap_or_else(|| serde_json::json!([]));
            Ok(serde_json::json!({
                "success": true,
                "structure": structure,
            }))
        }
        Ok(None) => Ok(serde_json::json!({
            "success": true,
            "structure": serde_json::json!([]),
        })),
        Err(e) => Err(format!("获取叙事结构失败: {}", e)),
    }
}

/// 获取故事的叙事事件（从 scenes 表的 narrative 字段）
#[tauri::command]
pub async fn get_narrative_events(
    story_id: String,
    state: tauri::State<'_, DbPool>,
) -> Result<serde_json::Value, String> {
    use crate::db::repositories::SceneRepository;

    let repo = SceneRepository::new(state.inner().clone());
    match repo.get_by_story(&story_id) {
        Ok(scenes) => {
            let events: Vec<serde_json::Value> = scenes
                .into_iter()
                .filter(|s| s.narrative_intensity.is_some())
                .map(|s| {
                    serde_json::json!({
                        "scene_id": s.id,
                        "scene_number": s.sequence_number,
                        "title": s.title,
                        "intensity": s.narrative_intensity,
                        "sentiment": s.narrative_sentiment,
                        "event_types": s.narrative_event_types,
                        "act_number": s.act_number,
                        "position_in_act": s.position_in_act,
                    })
                })
                .collect();
            Ok(serde_json::json!({
                "success": true,
                "count": events.len(),
                "events": events,
            }))
        }
        Err(e) => Err(format!("获取叙事事件失败: {}", e)),
    }
}

/// 获取故事的叙事线索（从 foreshadowing_tracker + character_states）
#[tauri::command]
pub async fn get_narrative_threads(
    story_id: String,
    state: tauri::State<'_, DbPool>,
) -> Result<serde_json::Value, String> {
    use crate::creative_engine::foreshadowing::ForeshadowingTracker;

    let tracker = ForeshadowingTracker::new(state.inner().clone());
    let mut threads = Vec::new();

    // 未回收的伏笔
    if let Ok(unresolved) = tracker.get_unresolved(&story_id) {
        for fs in unresolved {
            threads.push(serde_json::json!({
                "type": "foreshadow",
                "content": fs.content,
                "status": format!("{}", fs.status),
                "risk_score": fs.risk_signals_score,
            }));
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "count": threads.len(),
        "threads": threads,
    }))
}

/// 获取故事的叙事感知文本块
#[tauri::command]
pub async fn get_narrative_chunks(
    story_id: String,
    state: tauri::State<'_, DbPool>,
) -> Result<serde_json::Value, String> {
    use crate::db::repositories_narrative_events::NarrativeChunkRepository;

    let repo = NarrativeChunkRepository::new(state.inner().clone());
    match repo.get_by_story(&story_id) {
        Ok(chunks) => Ok(serde_json::json!({
            "success": true,
            "count": chunks.len(),
            "chunks": chunks,
        })),
        Err(e) => Err(format!("获取叙事块失败: {}", e)),
    }
}
