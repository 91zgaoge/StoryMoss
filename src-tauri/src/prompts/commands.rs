//! PromptRegistry IPC 命令

use tauri::State;

use super::registry;
use crate::{db::DbPool, error::AppError};

/// 列出所有提示词条目
#[tauri::command(rename_all = "snake_case")]
pub fn list_prompt_entries(
    pool: State<'_, DbPool>,
) -> Result<Vec<registry::PromptEntry>, AppError> {
    registry::list_prompt_entries(&pool)
}

/// 保存提示词覆盖
#[tauri::command(rename_all = "snake_case")]
pub fn save_prompt_override(
    pool: State<'_, DbPool>,
    prompt_id: String,
    content: String,
) -> Result<(), AppError> {
    registry::save_override(&pool, &prompt_id, &content)
}

/// 重置提示词为默认
#[tauri::command(rename_all = "snake_case")]
pub fn reset_prompt_override(pool: State<'_, DbPool>, prompt_id: String) -> Result<(), AppError> {
    registry::reset_override(&pool, &prompt_id)
}

/// 批量重置所有提示词覆盖
#[tauri::command(rename_all = "snake_case")]
pub fn reset_all_prompt_overrides(pool: State<'_, DbPool>) -> Result<usize, AppError> {
    registry::reset_all_overrides(&pool)
}

/// 解析提示词内容（用于调试/预览）
#[tauri::command(rename_all = "snake_case")]
pub fn resolve_prompt_content(
    pool: State<'_, DbPool>,
    prompt_id: String,
) -> Result<String, AppError> {
    registry::resolve_prompt(&pool, &prompt_id)
}

/// v0.26.34: 获取当前使用的 prompts 资源目录路径。
#[tauri::command(rename_all = "snake_case")]
pub fn get_prompts_directory() -> Result<String, AppError> {
    registry::get_prompts_directory()
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| AppError::Internal {
            message: "无法定位 prompts 资源目录".to_string(),
        })
}

/// v0.26.38: 用系统文件管理器打开 prompts 资源目录（绕过 shell.open
/// 本地路径限制）。
#[tauri::command(rename_all = "snake_case")]
pub fn open_prompts_directory() -> Result<String, AppError> {
    let dir = registry::get_prompts_directory().ok_or_else(|| AppError::Internal {
        message: "无法定位 prompts 资源目录".to_string(),
    })?;
    let path_str = dir.to_string_lossy().to_string();

    let status = {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open").arg(&dir).status()
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer").arg(&dir).status()
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            std::process::Command::new("xdg-open").arg(&dir).status()
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
        {
            return Err(AppError::Internal {
                message: format!("当前平台不支持打开目录: {}", path_str),
            });
        }
    };

    match status {
        Ok(s) if s.success() => Ok(path_str),
        Ok(s) => Err(AppError::Internal {
            message: format!("打开目录失败（exit {:?}）: {}", s.code(), path_str),
        }),
        Err(e) => Err(AppError::Internal {
            message: format!("打开目录失败: {} ({})", e, path_str),
        }),
    }
}

/// v0.26.38: 静态预览某生成场景会组合哪些提示词（0 LLM，只读声明）。
#[tauri::command(rename_all = "snake_case")]
pub fn preview_prompt_composition(
    scene: String,
) -> Result<registry::PromptCompositionPreview, AppError> {
    Ok(registry::preview_prompt_composition(&scene))
}
