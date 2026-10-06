//! LitSeg 叙事感知分段 — Tauri IPC 命令（深度融合后）
//!
//! 查询增强后的现有表：
//! - analyze_narrative_structure → story_outlines.analyzed_structure_json
//! - get_narrative_events → scenes.narrative_* 字段
//! - get_narrative_threads → story_system::ForeshadowingService（单一真源）
//! - get_narrative_chunks → narrative_chunks（物化缓存）

use crate::{
    db::DbPool, domain::foreshadowing::ForeshadowingProvider, error::AppError,
    story_system::foreshadowing_service::ForeshadowingServiceImpl,
};

/// 获取故事的叙事结构分析（从 story_outlines.analyzed_structure_json）
#[tauri::command]
pub async fn analyze_narrative_structure(
    story_id: String,
    state: tauri::State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
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
        Err(e) => Err(AppError::internal(format!("获取叙事结构失败: {}", e))),
    }
}

/// 获取故事的叙事事件（从 scenes 表的 narrative 字段）
#[tauri::command]
pub async fn get_narrative_events(
    story_id: String,
    state: tauri::State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
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
        Err(e) => Err(AppError::internal(format!("获取叙事事件失败: {}", e))),
    }
}

/// 获取故事的叙事线索（从 story_system::ForeshadowingService 单一真源读取）
#[tauri::command]
pub async fn get_narrative_threads(
    story_id: String,
    state: tauri::State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    get_narrative_threads_inner(&story_id, state.inner())
}

/// 内部实现，便于单元测试（无需构造 tauri::State）。
fn get_narrative_threads_inner(
    story_id: &str,
    pool: &DbPool,
) -> Result<serde_json::Value, AppError> {
    let service = ForeshadowingServiceImpl::new(pool.clone());
    let mut threads = Vec::new();

    // 未回收的伏笔：读取失败时返回空数组，避免前端因命令错误而崩溃。
    let unresolved = match service.get_unresolved(story_id) {
        Ok(records) => records,
        Err(e) => {
            log::warn!("[get_narrative_threads] 读取叙事线索失败: {}", e);
            return Ok(serde_json::json!({ "threads": [] }));
        }
    };
    for fs in unresolved {
        threads.push(serde_json::json!({
            "type": "foreshadow",
            "content": fs.content,
            "status": format!("{}", fs.status),
            "risk_score": fs.risk_signals_score,
        }));
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
) -> Result<serde_json::Value, AppError> {
    use crate::db::repositories_narrative_events::NarrativeChunkRepository;

    let repo = NarrativeChunkRepository::new(state.inner().clone());
    match repo.get_by_story(&story_id) {
        Ok(chunks) => Ok(serde_json::json!({
            "success": true,
            "count": chunks.len(),
            "chunks": chunks,
        })),
        Err(e) => Err(AppError::internal(format!("获取叙事块失败: {}", e))),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;

    fn in_memory_pool() -> DbPool {
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        r2d2::Pool::builder().max_size(1).build(manager).unwrap()
    }

    fn in_memory_pool_with_schema() -> DbPool {
        let pool = in_memory_pool();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute_batch(
            "CREATE TABLE stories (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT,
                status TEXT,
                word_count INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE foreshadowing_tracker (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                content TEXT NOT NULL,
                setup_scene_id TEXT,
                payoff_scene_id TEXT,
                status TEXT NOT NULL DEFAULT 'setup',
                importance INTEGER,
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                setup_event_id TEXT,
                payoff_event_id TEXT,
                risk_signals_score REAL DEFAULT 0.0,
                target_start_scene INTEGER,
                target_end_scene INTEGER,
                evidence TEXT,
                strength INTEGER DEFAULT 5,
                subtlety INTEGER DEFAULT 5,
                related_foreshadow_ids TEXT DEFAULT '[]'
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, 'Test', ?2, ?2)",
            rusqlite::params!["story-1", now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO foreshadowing_tracker \
             (id, story_id, content, status, created_at, setup_event_id, risk_signals_score, importance) \
             VALUES (?1, ?2, ?3, 'setup', ?4, ?5, ?6, ?7)",
            rusqlite::params!["fs-1", "story-1", "神秘钥匙", now, "evt-1", 0.5, 8],
        )
        .unwrap();
        pool
    }

    #[test]
    fn get_narrative_threads_returns_records() {
        let pool = in_memory_pool_with_schema();
        let result = get_narrative_threads_inner("story-1", &pool).unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["count"], 1);
        let threads = result["threads"].as_array().unwrap();
        assert_eq!(threads[0]["content"], "神秘钥匙");
        assert_eq!(threads[0]["risk_score"], 0.5);
    }

    #[test]
    fn get_narrative_threads_returns_empty_on_error() {
        // 空数据库缺少 foreshadowing_tracker
        // 表，服务读取会失败，命令应返回空数组。
        let pool = in_memory_pool();
        let result = get_narrative_threads_inner("story-1", &pool).unwrap();
        assert_eq!(result["threads"].as_array().unwrap().len(), 0);
    }
}
