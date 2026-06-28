use std::sync::Arc;

use tauri::{AppHandle, Manager};

use super::types::*;
use crate::{
    db::{
        ChapterRepository, DbPool, DraftRepository, DraftStatus, PostProcessRepository,
        PostProcessStatus, SceneRepository, SceneUpdate, StepStatus,
    },
    llm::LlmService,
    ports::VectorStore,
};

/// 执行定稿
///
/// 1. 将 refined/reviewed 草稿状态更新为 finalized
/// 2. 同步 content 到 chapters 表（向后兼容）
/// 3. 启动 PostProcessPipeline
pub async fn finalize_draft(
    story_id: &str,
    draft_id: &str,
    chapter_info: &ChapterInfo,
    config: &PipelineConfig,
    pool: &DbPool,
    app_handle: &AppHandle,
    callbacks: &dyn PipelineCallbacks,
    vector_store: &dyn VectorStore,
) -> Result<String, PipelineError> {
    callbacks.progress("finalize", 0.05);

    // 1. 读取草稿
    let draft_repo = DraftRepository::new(pool.clone());
    let draft = draft_repo
        .get_by_id(draft_id)
        .map_err(|e| PipelineError {
            phase: "finalize".to_string(),
            message: format!("读取草稿失败: {}", e),
            recoverable: true,
        })?
        .ok_or_else(|| PipelineError {
            phase: "finalize".to_string(),
            message: "草稿不存在".to_string(),
            recoverable: true,
        })?;

    // 验证状态
    if draft.status != DraftStatus::Refined && draft.status != DraftStatus::Reviewed {
        return Err(PipelineError {
            phase: "finalize".to_string(),
            message: format!(
                "草稿状态为 {:?}，无法定稿。请先执行修稿和审稿。",
                draft.status
            ),
            recoverable: true,
        });
    }

    callbacks.log(&format!(
        "[定稿] 开始定稿：第{}章",
        chapter_info.chapter_number
    ));
    callbacks.progress("finalize", 0.1);

    // 2. 更新草稿状态为 finalized
    draft_repo
        .update_status(draft_id, DraftStatus::Finalized)
        .map_err(|e| PipelineError {
            phase: "finalize".to_string(),
            message: format!("更新草稿状态失败: {}", e),
            recoverable: false,
        })?;

    callbacks.progress("finalize", 0.2);

    // 3. 同步到 scenes 表（Phase 1: Scene 为内容真相源）
    let chapter_repo = ChapterRepository::new(pool.clone());
    let scene_repo = SceneRepository::new(pool.clone());
    if let Ok(chapters) = chapter_repo.get_by_story(story_id) {
        if let Some(chapter) = chapters
            .into_iter()
            .find(|c| c.chapter_number == draft.chapter_number)
        {
            // 更新章元数据（不含 content）
            let _ = chapter_repo.update(&chapter.id, None, None, Some(draft.word_count));
            // 内容写入关联 Scene
            if let Ok(scenes) = scene_repo.get_by_chapter(&chapter.id) {
                if let Some(scene) = scenes.first() {
                    let _ = scene_repo.update(
                        &scene.id,
                        &SceneUpdate {
                            content: Some(draft.content.clone()),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    callbacks.progress("finalize", 0.3);

    // 4. 启动后处理（如果启用）
    if config.enable_finalize_post_process {
        let post_process_repo = PostProcessRepository::new(pool.clone());

        let run = post_process_repo
            .create_run(story_id, draft.chapter_number, "finalize", None)
            .map_err(|e| PipelineError {
                phase: "finalize".to_string(),
                message: format!("创建后处理运行记录失败: {}", e),
                recoverable: false,
            })?;

        let steps = super::build_finalize_steps(
            story_id,
            draft.chapter_number,
            chapter_info.title.as_deref().unwrap_or(""),
            &draft.content,
        );

        // 创建步骤记录并保存步骤对象（含 id）
        let mut step_records = Vec::new();
        for step in &steps {
            match post_process_repo.create_step(&run.id, &step.key, &step.label, step.critical) {
                Ok(step_record) => step_records.push((step.clone(), step_record)),
                Err(e) => {
                    log::warn!("[finalize] 创建步骤记录失败 {}: {}", step.key, e);
                }
            }
        }

        callbacks.log(&format!("[定稿] 后处理已启动，run_id={}", run.id));
        callbacks.progress("finalize", 0.5);

        // 执行后处理步骤
        let llm_service = LlmService::new(app_handle.clone());
        for (step_def, step_record) in &step_records {
            callbacks.log(&format!("[定稿] 执行步骤: {}", step_def.label));

            // 标记步骤为运行中
            let _ = post_process_repo.update_step_status(
                &step_record.id,
                StepStatus::Running,
                Some(&format!("开始执行 {}", step_def.key)),
                None,
            );

            let result = super::run_post_process_step(
                story_id,
                draft.chapter_number,
                &draft.content,
                step_def,
                pool,
                &llm_service,
                vector_store,
            )
            .await;

            match result {
                Ok(_) => {
                    let _ = post_process_repo.update_step_status(
                        &step_record.id,
                        StepStatus::Success,
                        Some(&format!("{} 执行完成", step_def.key)),
                        None,
                    );
                    callbacks.log(&format!("[定稿] 步骤 {} 完成", step_def.key));
                }
                Err(e) => {
                    let _ = post_process_repo.update_step_status(
                        &step_record.id,
                        StepStatus::Failed,
                        Some(&format!("{} 执行失败", step_def.key)),
                        Some(&e.message),
                    );
                    if step_def.critical {
                        // 关键步骤失败，更新运行状态并返回错误
                        let _ = post_process_repo.update_run_status(
                            &run.id,
                            PostProcessStatus::Failed,
                            Some(&e.message),
                        );
                        return Err(PipelineError {
                            phase: format!("post_process:{}", step_def.key),
                            message: e.message,
                            recoverable: false,
                        });
                    } else {
                        log::warn!("[finalize] 非关键步骤 {} 失败: {}", step_def.key, e.message);
                    }
                }
            }
        }

        // 完成后处理
        post_process_repo
            .update_run_status(&run.id, PostProcessStatus::Completed, None)
            .map_err(|e| PipelineError {
                phase: "finalize".to_string(),
                message: format!("更新后处理状态失败: {}", e),
                recoverable: false,
            })?;

        callbacks.log("[定稿] 后处理完成");
        callbacks.progress("finalize", 1.0);
        if let Some(automation_service) =
            app_handle.try_state::<crate::automation::service::AutomationService>()
        {
            let _ = automation_service
                .trigger_event(
                    crate::automation::triggers::TriggerEvent::ChapterFinalized {
                        story_id: story_id.to_string(),
                        chapter_id: draft_id.to_string(),
                    },
                )
                .await;
        }

        Ok(run.id)
    } else {
        callbacks.log("[定稿] 后处理已跳过");
        callbacks.progress("finalize", 1.0);
        Ok(String::new())
    }
}
