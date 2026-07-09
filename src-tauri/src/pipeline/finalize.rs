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

/// 将草稿内容写入目标场景。
///
/// 优先 `explicit_scene_id` / `draft.scene_id`；旧草稿无 scene_id 时回退
/// chapter_number → 该章第一个 scene（兼容路径）。
pub fn write_draft_content_to_scene(
    pool: &DbPool,
    story_id: &str,
    chapter_number: i32,
    content: &str,
    word_count: i32,
    explicit_scene_id: Option<&str>,
    draft_scene_id: Option<&str>,
) -> Result<Option<String>, rusqlite::Error> {
    let chapter_repo = ChapterRepository::new(pool.clone());
    let scene_repo = SceneRepository::new(pool.clone());

    let target_scene_id = resolve_finalize_target_scene_id(explicit_scene_id, draft_scene_id);

    if let Some(ref sid) = target_scene_id {
        scene_repo.update(
            sid,
            &SceneUpdate {
                content: Some(content.to_string()),
                ..Default::default()
            },
        )?;
        if let Ok(chapters) = chapter_repo.get_by_story(story_id) {
            if let Some(chapter) = chapters
                .into_iter()
                .find(|c| c.chapter_number == chapter_number)
            {
                let _ = chapter_repo.update(&chapter.id, None, None, Some(word_count));
            }
        }
        return Ok(Some(sid.clone()));
    }

    // 兼容旧草稿：chapter → first scene
    if let Ok(chapters) = chapter_repo.get_by_story(story_id) {
        if let Some(chapter) = chapters
            .into_iter()
            .find(|c| c.chapter_number == chapter_number)
        {
            let _ = chapter_repo.update(&chapter.id, None, None, Some(word_count));
            if let Ok(scenes) = scene_repo.get_by_chapter(&chapter.id) {
                if let Some(scene) = scenes.first() {
                    scene_repo.update(
                        &scene.id,
                        &SceneUpdate {
                            content: Some(content.to_string()),
                            ..Default::default()
                        },
                    )?;
                    return Ok(Some(scene.id.clone()));
                }
            }
        }
    }
    Ok(None)
}

/// 优先显式 scene_id，其次草稿上的 scene_id。
pub fn resolve_finalize_target_scene_id(
    explicit_scene_id: Option<&str>,
    draft_scene_id: Option<&str>,
) -> Option<String> {
    explicit_scene_id
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            draft_scene_id
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
}

/// 执行定稿
///
/// 1. 将 refined/reviewed 草稿状态更新为 finalized
/// 2. 同步 content 到 scenes 表（优先 scene_id，否则 chapter→first-scene
///    兼容旧草稿）
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
    scene_id: Option<&str>,
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

    // 3. 同步到 scenes 表（优先显式/草稿 scene_id，否则 chapter→first-scene 兼容）
    if let Err(e) = write_draft_content_to_scene(
        pool,
        story_id,
        draft.chapter_number,
        &draft.content,
        draft.word_count,
        scene_id,
        draft.scene_id.as_deref(),
    ) {
        log::warn!("[finalize] 写入场景内容失败: {}", e);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        connection::create_test_pool, CreateStoryRequest, DraftSource, DraftStatus,
        SceneRepository, SceneUpdate, StoryRepository,
    };

    fn story_req(title: &str) -> CreateStoryRequest {
        CreateStoryRequest {
            title: title.into(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        }
    }

    #[test]
    fn resolve_prefers_explicit_scene_id() {
        assert_eq!(
            resolve_finalize_target_scene_id(Some("scene-b"), Some("scene-a")).as_deref(),
            Some("scene-b")
        );
        assert_eq!(
            resolve_finalize_target_scene_id(None, Some("scene-a")).as_deref(),
            Some("scene-a")
        );
        assert_eq!(resolve_finalize_target_scene_id(Some(""), None), None);
    }

    #[test]
    fn finalize_with_scene_id_updates_only_that_scene() {
        let pool = create_test_pool().expect("test pool");
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool.clone());
        let draft_repo = DraftRepository::new(pool.clone());

        let story = story_repo.create(story_req("定稿场景测试")).unwrap();
        // scene.create 会按 sequence_number 自动建/挂 chapter
        let scene_a = scene_repo.create(&story.id, 1, Some("场景A")).unwrap();
        let scene_b = scene_repo.create(&story.id, 2, Some("场景B")).unwrap();
        let chapter_id = scene_a.chapter_id.clone().expect("scene_a chapter");
        {
            let conn = pool.get().unwrap();
            // 同章多场景：把 B 也挂到 A 的 chapter
            conn.execute(
                "UPDATE scenes SET chapter_id = ?1 WHERE id = ?2",
                rusqlite::params![&chapter_id, &scene_b.id],
            )
            .unwrap();
        }
        scene_repo
            .update(
                &scene_a.id,
                &SceneUpdate {
                    content: Some("A原内容".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        scene_repo
            .update(
                &scene_b.id,
                &SceneUpdate {
                    content: Some("B原内容".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        let draft = draft_repo
            .create(
                &story.id,
                1,
                1,
                DraftStatus::Reviewed,
                DraftSource::Write,
                "定稿写入B",
                5,
                None,
                None,
                None,
                Some(&scene_b.id),
            )
            .unwrap();

        let written = write_draft_content_to_scene(
            &pool,
            &story.id,
            1,
            &draft.content,
            draft.word_count,
            Some(&scene_b.id),
            draft.scene_id.as_deref(),
        )
        .unwrap();
        assert_eq!(written.as_deref(), Some(scene_b.id.as_str()));

        let a_after = scene_repo.get_by_id(&scene_a.id).unwrap().unwrap();
        let b_after = scene_repo.get_by_id(&scene_b.id).unwrap().unwrap();
        assert_eq!(a_after.content.as_deref(), Some("A原内容"));
        assert_eq!(b_after.content.as_deref(), Some("定稿写入B"));

        let latest = draft_repo
            .get_latest_by_scene(&story.id, &scene_b.id)
            .unwrap()
            .expect("draft by scene");
        assert_eq!(latest.id, draft.id);
        assert_eq!(latest.scene_id.as_deref(), Some(scene_b.id.as_str()));
    }

    #[test]
    fn finalize_without_scene_id_falls_back_to_first_scene() {
        let pool = create_test_pool().expect("test pool");
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool.clone());

        let story = story_repo.create(story_req("兼容回退")).unwrap();
        let scene_a = scene_repo.create(&story.id, 1, Some("场景A")).unwrap();
        let scene_b = scene_repo.create(&story.id, 2, Some("场景B")).unwrap();
        let chapter_id = scene_a.chapter_id.clone().expect("scene_a chapter");
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "UPDATE scenes SET chapter_id = ?1 WHERE id = ?2",
                rusqlite::params![&chapter_id, &scene_b.id],
            )
            .unwrap();
        }
        scene_repo
            .update(
                &scene_a.id,
                &SceneUpdate {
                    content: Some("A原".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        scene_repo
            .update(
                &scene_b.id,
                &SceneUpdate {
                    content: Some("B原".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        let written =
            write_draft_content_to_scene(&pool, &story.id, 1, "旧草稿内容", 4, None, None).unwrap();
        assert_eq!(written.as_deref(), Some(scene_a.id.as_str()));

        let a_after = scene_repo.get_by_id(&scene_a.id).unwrap().unwrap();
        let b_after = scene_repo.get_by_id(&scene_b.id).unwrap().unwrap();
        assert_eq!(a_after.content.as_deref(), Some("旧草稿内容"));
        assert_eq!(b_after.content.as_deref(), Some("B原"));
    }
}
