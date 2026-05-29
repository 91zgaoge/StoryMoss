//! Cascade Rewriter 用户交互命令
//!
//! 提供 Diff 预览数据的查询，以及接受/拒绝改写片段的应用接口。

use tauri::{command, AppHandle, State};

use super::models::{CascadeTaskResult, RewriteSegment, UserDecision};
use crate::{
    db::{
        repositories::{SceneRepository, SceneUpdate},
        DbPool,
    },
    error::AppError,
};

/// 获取级联改写任务的结果（用于 Diff 预览）
#[command(rename_all = "snake_case")]
pub async fn get_cascade_rewrite_result(
    task_id: String,
    pool: State<'_, DbPool>,
) -> Result<CascadeTaskResult, AppError> {
    let repo = crate::task_system::repository::TaskRepository::new(pool.inner().clone());
    let task = repo
        .get_by_id(&task_id)
        .map_err(|e| AppError::internal(format!("查询任务失败: {}", e)))?
        .ok_or_else(|| AppError::not_found("Task", &task_id))?;

    let result_json = task
        .result
        .ok_or_else(|| AppError::internal("任务暂无结果"))?;

    let result: CascadeTaskResult = serde_json::from_str(&result_json)
        .map_err(|e| AppError::internal(format!("解析任务结果失败: {}", e)))?;

    Ok(result)
}

/// 接受指定的改写片段，将其应用到对应场景的 content 中
#[command(rename_all = "snake_case")]
pub async fn apply_cascade_rewrite(
    task_id: String,
    accepted_indices: Vec<usize>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<usize, AppError> {
    let task_repo = crate::task_system::repository::TaskRepository::new(pool.inner().clone());
    let task = task_repo
        .get_by_id(&task_id)
        .map_err(|e| AppError::internal(format!("查询任务失败: {}", e)))?
        .ok_or_else(|| AppError::not_found("Task", &task_id))?;

    let result_json = task
        .result
        .ok_or_else(|| AppError::internal("任务暂无结果"))?;

    let mut result: CascadeTaskResult = serde_json::from_str(&result_json)
        .map_err(|e| AppError::internal(format!("解析任务结果失败: {}", e)))?;

    // 先收集有效的改写片段（clone，避免与 result.segments 的借用冲突）
    let mut rewrites: Vec<(usize, RewriteSegment)> = Vec::new();
    for &idx in &accepted_indices {
        if let Some(segment) = result.segments.get(idx) {
            if segment.user_decision == UserDecision::Pending {
                rewrites.push((idx, segment.clone()));
            }
        }
    }

    // 按 scene_id 分组
    let mut scene_rewrites: std::collections::HashMap<String, Vec<(usize, RewriteSegment)>> =
        std::collections::HashMap::new();
    for (idx, segment) in rewrites {
        scene_rewrites
            .entry(segment.scene_id.clone())
            .or_default()
            .push((idx, segment));
    }

    let mut applied_count = 0;
    let scene_repo = SceneRepository::new(pool.inner().clone());

    for (scene_id, mut segments) in scene_rewrites {
        // 对每个 scene，按 paragraph_index 降序处理，避免替换后索引偏移
        segments.sort_by_key(|(_, seg)| std::cmp::Reverse(seg.paragraph_index));

        let scene = scene_repo
            .get_by_id(&scene_id)
            .map_err(AppError::from)?
            .ok_or_else(|| AppError::not_found("Scene", &scene_id))?;

        let content = scene.content.unwrap_or_default();
        if content.is_empty() {
            continue;
        }

        let mut paragraphs: Vec<String> = content.split('\n').map(|s| s.to_string()).collect();
        let mut modified = false;

        for (idx, segment) in &segments {
            let pidx = segment.paragraph_index as usize;
            if pidx < paragraphs.len() {
                paragraphs[pidx] = segment.rewritten_text.clone();
                modified = true;
                applied_count += 1;
                // 标记为已接受
                result.segments[*idx].user_decision = UserDecision::Accepted;
            }
        }

        if modified {
            let new_content = paragraphs.join("\n");
            let _ = scene_repo.update(
                &scene_id,
                &SceneUpdate {
                    content: Some(new_content),
                    ..Default::default()
                },
            );

            // 发射场景更新同步事件
            let _ = crate::state_sync::StateSync::emit_scene_updated(
                &app_handle,
                &scene.story_id,
                &scene_id,
                scene.title.as_deref(),
            );
        }
    }

    // 更新任务结果
    let updated_result_json = serde_json::to_string(&result)
        .map_err(|e| AppError::internal(format!("序列化结果失败: {}", e)))?;
    let _ = task_repo.update_status(
        &task_id,
        &crate::task_system::models::TaskStatus::Completed,
        Some(100),
        Some(updated_result_json),
        None,
    );

    Ok(applied_count)
}

/// 拒绝指定的改写片段
#[command(rename_all = "snake_case")]
pub async fn reject_cascade_rewrite(
    task_id: String,
    rejected_indices: Vec<usize>,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    let task_repo = crate::task_system::repository::TaskRepository::new(pool.inner().clone());
    let task = task_repo
        .get_by_id(&task_id)
        .map_err(|e| AppError::internal(format!("查询任务失败: {}", e)))?
        .ok_or_else(|| AppError::not_found("Task", &task_id))?;

    let result_json = task
        .result
        .ok_or_else(|| AppError::internal("任务暂无结果"))?;

    let mut result: CascadeTaskResult = serde_json::from_str(&result_json)
        .map_err(|e| AppError::internal(format!("解析任务结果失败: {}", e)))?;

    let mut rejected_count = 0;
    for &idx in &rejected_indices {
        if let Some(segment) = result.segments.get_mut(idx) {
            if segment.user_decision == UserDecision::Pending {
                segment.user_decision = UserDecision::Rejected;
                rejected_count += 1;
            }
        }
    }

    // 更新任务结果
    let updated_result_json = serde_json::to_string(&result)
        .map_err(|e| AppError::internal(format!("序列化结果失败: {}", e)))?;
    let _ = task_repo.update_status(
        &task_id,
        &crate::task_system::models::TaskStatus::Completed,
        Some(100),
        Some(updated_result_json),
        None,
    );

    Ok(rejected_count)
}
