//! Memory commands

use tauri::State;

use crate::{
    db::DbPool,
    domain::memory_pack::{MemoryItemDto, MemoryPack},
    error::AppError,
    memory::MemoryTaskType,
};

// ==================== Memory Commands ====================

#[tauri::command(rename_all = "snake_case")]
pub fn build_memory_pack(
    pool: State<'_, DbPool>,
    story_id: String,
    chapter_number: i32,
    task_type: String,
    outline: Option<String>,
) -> Result<MemoryPack, AppError> {
    let memory_task_type = match task_type.as_str() {
        "plan" => MemoryTaskType::Plan,
        "review" => MemoryTaskType::Review,
        _ => MemoryTaskType::Write,
    };

    let pool = pool.inner().clone();
    let orchestrator = crate::memory::MemoryOrchestrator::new(pool.clone());
    let mut pack = orchestrator.build_memory_pack(
        &story_id,
        chapter_number,
        memory_task_type,
        outline.as_deref(),
    )?;

    // 解耦点：memory 模块不再直接依赖 crate::creative_engine::PayoffLedger
    // 在协调层（lib.rs）合并伏笔账本数据
    let ledger = crate::creative_engine::payoff_ledger::PayoffLedger::new(pool);
    if let Ok(overdue) = ledger.detect_overdue(&story_id, chapter_number) {
        for item in overdue {
            pack.active_constraints.push(MemoryItemDto {
                id: item.id,
                category: "overdue_foreshadowing".to_string(),
                subject: Some(item.title),
                field: Some("payoff".to_string()),
                value: Some(format!("{} (重要度: {})", item.summary, item.importance)),
                source_chapter: item.first_seen_scene,
                confidence: item.confidence,
            });
        }
    }
    if let Ok(recommendations) = ledger.recommend_payoff_timing(&story_id, chapter_number) {
        for rec in recommendations.iter().take(5) {
            pack.active_constraints.push(MemoryItemDto {
                id: rec.foreshadowing_id.clone(),
                category: "recommended_payoff".to_string(),
                subject: Some(rec.title.clone()),
                field: Some("urgency".to_string()),
                value: Some(format!(
                    "推荐在场景{}回收，原因: {}",
                    rec.recommended_scene, rec.reason
                )),
                source_chapter: Some(rec.recommended_scene),
                confidence: (rec.importance as f32 / 10.0).clamp(0.0, 1.0),
            });
        }
    }

    Ok(pack)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_memory_items(
    pool: State<'_, DbPool>,
    story_id: String,
) -> Result<Vec<crate::db::MemoryItem>, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::MemoryItemRepository::new(pool);
    repo.get_active_by_story(&story_id).map_err(AppError::from)
}

/// Unified read model: `kg_entities` ∪ `memory_items` via MemoryFacade / VIEW.
#[tauri::command(rename_all = "snake_case")]
pub fn get_story_memory_facts(
    pool: State<'_, DbPool>,
    story_id: String,
    limit: Option<usize>,
) -> Result<Vec<crate::memory::UnifiedMemoryFact>, AppError> {
    let pool = pool.inner().clone();
    let limit = limit.unwrap_or(crate::memory::DEFAULT_UNIFIED_FACTS_LIMIT);
    Ok(crate::memory::MemoryFacade::list_unified_facts(
        &pool, &story_id, limit,
    ))
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_memory_item(
    pool: State<'_, DbPool>,
    story_id: String,
    category: String,
    subject: Option<String>,
    field: Option<String>,
    value: Option<String>,
    source_chapter: Option<i32>,
    confidence: f32,
) -> Result<crate::db::MemoryItem, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::MemoryItemRepository::new(pool);
    repo.create(
        &story_id,
        &category,
        subject.as_deref(),
        field.as_deref(),
        value.as_deref(),
        source_chapter,
        confidence,
    )
    .map_err(AppError::from)
}
