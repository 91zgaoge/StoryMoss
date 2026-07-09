//! Genre commands

use tauri::State;

use crate::{db::DbPool, error::AppError};

// ==================== Genre Profile Commands ====================

#[tauri::command(rename_all = "snake_case")]
pub fn get_genre_profiles(
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::GenreProfile>, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::GenreProfileRepository::new(pool);
    repo.get_all().map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn get_genre_profile(
    pool: State<'_, DbPool>,
    genre_name: String,
) -> Result<Option<crate::db::GenreProfile>, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::GenreProfileRepository::new(pool);
    repo.get_by_name(&genre_name).map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub fn save_genre_profile(
    pool: State<'_, DbPool>,
    id: Option<String>,
    genre_name: String,
    canonical_name: String,
    aliases_json: Option<String>,
    core_tone: Option<String>,
    pacing_strategy: Option<String>,
    anti_patterns_json: Option<String>,
    reference_tables_json: Option<String>,
    typical_structure_json: Option<String>,
) -> Result<crate::db::GenreProfile, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::GenreProfileRepository::new(pool);

    if let Some(existing_id) = id {
        // 更新现有记录
        repo.update(
            &existing_id,
            &genre_name,
            &canonical_name,
            aliases_json.as_deref(),
            core_tone.as_deref(),
            pacing_strategy.as_deref(),
            anti_patterns_json.as_deref(),
            reference_tables_json.as_deref(),
            typical_structure_json.as_deref(),
        )
        .map_err(AppError::from)?;
        repo.get_by_id(&existing_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("GenreProfile", &existing_id))
    } else {
        // 创建新记录（用户自定义，is_builtin = false）
        repo.create(
            &genre_name,
            &canonical_name,
            aliases_json.as_deref(),
            core_tone.as_deref(),
            pacing_strategy.as_deref(),
            anti_patterns_json.as_deref(),
            reference_tables_json.as_deref(),
            typical_structure_json.as_deref(),
            false,
        )
        .map_err(AppError::from)
    }
}

#[tauri::command(rename_all = "snake_case")]
pub fn delete_genre_profile(pool: State<'_, DbPool>, id: String) -> Result<usize, AppError> {
    let pool = pool.inner().clone();
    let repo = crate::db::GenreProfileRepository::new(pool);
    // 只允许删除非内置体裁
    if let Ok(Some(profile)) = repo.get_by_id(&id) {
        if profile.is_builtin {
            return Err(AppError::validation_failed(
                "内置体裁不可删除",
                None::<String>,
            ));
        }
    }
    repo.delete(&id).map_err(AppError::from)
}
