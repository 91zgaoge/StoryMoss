//! Cascade Rewriter 用户交互命令
//!
//! 提供 Diff 预览数据的查询，以及接受/拒绝改写片段的应用接口。

use tauri::{command, AppHandle, Manager, State};

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
                true, // Phase 4: cascade rewrite changes content
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

// ==================== v0.60.0 P0-T4：改稿级联影响报告 ====================

/// 列出改稿级联影响条目（可按决策状态过滤）。
#[command(rename_all = "snake_case")]
pub async fn list_cascade_impacts(
    story_id: String,
    decision: Option<String>,
    limit: Option<i64>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::creative_engine::cascade_rewriter::impact_report::CascadeImpact>, AppError> {
    crate::creative_engine::cascade_rewriter::impact_report::list_impacts(
        pool.inner(),
        &story_id,
        decision.as_deref(),
        limit.unwrap_or(100),
    )
    .map_err(|e| AppError::internal(format!("查询级联影响失败: {}", e)))
}

/// 作者动作：忽略某条级联影响（不再在级联中心高亮）。
#[command(rename_all = "snake_case")]
pub async fn ignore_cascade_impact(
    impact_id: String,
    pool: State<'_, DbPool>,
) -> Result<usize, AppError> {
    crate::creative_engine::cascade_rewriter::impact_report::mark_impact_decision(
        pool.inner(),
        &impact_id,
        "ignored",
    )
    .map_err(|e| AppError::internal(format!("更新级联影响失败: {}", e)))
}

/// 作者动作：重跑某场景的分析（re-ingest）。
///
/// 完成后该场景的「分析可能已失效」标记会被自动清除（见
/// `impact_report::analyze_after_scene_ingest` 的收口）。
#[command(rename_all = "snake_case")]
pub async fn reanalyze_scene(
    scene_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
    vector_store: State<'_, std::sync::Arc<dyn crate::ports::VectorStore>>,
) -> Result<(), AppError> {
    crate::story_system::scene_service::SceneIngestor::spawn_ingest_now(
        scene_id,
        pool.inner().clone(),
        app_handle,
        vector_store.inner().clone(),
    );
    Ok(())
}

/// 作者动作：由级联影响条目触发既有 cascade_rewrite 引擎改写目标场景。
///
/// 实体名/类型从 KG 解析；`after_json` 携带「上游章正文已修改 + 节选」，
/// 让改写 prompt 获得变更上下文。任务创建后由 TaskService 立即异步执行。
#[command(rename_all = "snake_case")]
pub async fn trigger_cascade_rewrite_for_impact(
    impact_id: String,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<String, AppError> {
    use crate::task_system::{models::CreateTaskRequest, service::TaskService};

    let pool_owned = pool.inner().clone();
    let impact = crate::creative_engine::cascade_rewriter::impact_report::load_impact(
        &pool_owned,
        &impact_id,
    )
    .map_err(|e| AppError::internal(format!("查询级联影响失败: {}", e)))?
    .ok_or_else(|| AppError::not_found("CascadeImpact", &impact_id))?;

    let events =
        crate::creative_engine::cascade_rewriter::impact_report::build_change_events_for_impact(
            &pool_owned,
            &impact,
        );
    if events.is_empty() {
        return Err(AppError::internal(
            "该影响条目没有可解析的实体，无法触发级联改写",
        ));
    }
    let payload = super::models::CascadeTaskPayload {
        story_id: impact.story_id.clone(),
        change_events: events,
    };
    let payload_json = serde_json::to_string(&payload)
        .map_err(|e| AppError::internal(format!("序列化级联任务失败: {}", e)))?;
    let task_service: State<TaskService> = app_handle.state();
    let source_chapter = impact
        .source_chapter_number
        .map(|c| format!("第{c}章"))
        .unwrap_or_else(|| "未知章".to_string());
    let target_chapter = impact
        .target_chapter_number
        .map(|c| format!("第{c}章"))
        .unwrap_or_else(|| "目标场景".to_string());
    let task = task_service
        .create_task(CreateTaskRequest {
            name: format!("级联改写: {source_chapter}改稿 → {target_chapter}"),
            description: Some(format!(
                "由改稿影响报告触发（{} → {}）",
                impact.source_scene_id, impact.target_scene_id
            )),
            task_type: "cascade_rewrite".to_string(),
            schedule_type: "once".to_string(),
            cron_pattern: None,
            payload: Some(payload_json),
            enabled: Some(true),
            max_retries: Some(1),
            heartbeat_timeout_seconds: Some(300),
        })
        .map_err(|e| AppError::internal(format!("创建级联改写任务失败: {}", e)))?;

    let _ = crate::creative_engine::cascade_rewriter::impact_report::mark_impact_decision(
        &pool_owned,
        &impact_id,
        "rewrite_requested",
    );
    Ok(task.id)
}
