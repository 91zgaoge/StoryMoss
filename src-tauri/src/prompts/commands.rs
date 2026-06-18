//! v0.17.1 提示词注册表 IPC 命令

use tauri::{command, AppHandle, Manager, State};

use crate::{db::DbPool, error::AppError, prompts::registry};

/// 列出所有内置 prompt（含 override 状态）
#[command(rename_all = "snake_case")]
pub fn list_prompt_entries(
    pool: State<'_, DbPool>,
) -> Result<Vec<registry::PromptEntry>, AppError> {
    registry::list_prompts(pool.inner())
}

/// 保存用户对某个 prompt 的覆盖
#[command(rename_all = "snake_case")]
pub fn save_prompt_override(
    prompt_id: String,
    content: String,
    pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    registry::save_override(pool.inner(), &prompt_id, &content)
}

/// 重置某个 prompt 到内置默认（删除 override）
#[command(rename_all = "snake_case")]
pub fn reset_prompt_override(prompt_id: String, pool: State<'_, DbPool>) -> Result<(), AppError> {
    registry::reset_override(pool.inner(), &prompt_id)
}

/// 获取单个 prompt 的当前生效内容（含 override 解析）
#[command(rename_all = "snake_case")]
pub fn resolve_prompt_content(
    prompt_id: String,
    app_handle: AppHandle,
) -> Result<String, AppError> {
    let pool = app_handle
        .try_state::<DbPool>()
        .ok_or_else(|| AppError::internal("DbPool not available".to_string()))?;
    registry::resolve_prompt(pool.inner(), &prompt_id)
}
