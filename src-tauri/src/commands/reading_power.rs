//! Reading Power commands

use tauri::State;

use crate::{db::DbPool, error::AppError};

// ==================== Reading Power Commands ====================

#[tauri::command(rename_all = "snake_case")]
pub fn evaluate_reading_power(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
) -> Result<crate::reading_power::ReadingPowerEvaluation, AppError> {
    let pool = pool.inner().clone();
    let evaluator = crate::reading_power::ReadingPowerEvaluator::new(pool);
    evaluator
        .evaluate(&story_id, chapter_number)
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_reading_power_trend(
    story_id: String,
    last_n: i64,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::reading_power::ReadingPowerEvaluation>, AppError> {
    let pool = pool.inner().clone();
    let evaluator = crate::reading_power::ReadingPowerEvaluator::new(pool);
    evaluator
        .get_trend(&story_id, last_n)
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_chase_debts(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::ChaseDebt>, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::ChaseDebtRepository::new(pool);
    repo.get_active_by_story(&story_id).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn create_override_contract(
    story_id: String,
    chapter_number: i32,
    constraint_type: String,
    constraint_id: String,
    rationale_type: String,
    rationale_text: String,
    payback_plan: String,
    due_chapter: i32,
    pool: State<'_, DbPool>,
) -> Result<crate::db::OverrideContract, AppError> {
    let pool = pool.inner().clone();
    let manager = crate::reading_power::DebtManager::new(pool);
    manager
        .create_override_contract(
            &story_id,
            chapter_number,
            &constraint_type,
            &constraint_id,
            &rationale_type,
            &rationale_text,
            &payback_plan,
            due_chapter,
        )
        .map_err(AppError::from)
}
