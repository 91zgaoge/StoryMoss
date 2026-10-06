//! Orchestrator commands

use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::{
    db::{
        Chapter, ChapterRepository, CharacterRepository, DbPool, Story, StoryOutlineRepository,
        StoryRepository,
    },
    error::AppError,
    error_recovery::retry_with_backoff,
    record_ai_operation,
};

/// smart_execute 初始上下文加载结果类型别名，降低闭包类型复杂度
type SmartExecuteContext = (Vec<Story>, Option<Story>, Option<String>, Vec<Chapter>);

/// 预检命令 - 写作前检查阻塞性问题
#[tauri::command(rename_all = "snake_case")]
pub async fn check_preflight(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
) -> Result<crate::story_system::preflight::PreflightResult, AppError> {
    let pool = pool.inner().clone();
    let checker = crate::story_system::preflight::PreflightChecker::new();
    Ok(checker.check(&pool, &story_id, chapter_number).await)
}

/// 智能执行命令 - 新一代意图理解与执行入口
///
/// v0.14.0: 外层包裹 600 秒整体超时，确保任何环节卡死都能快速失败。
/// 超时时主动取消所有进行中的 LLM 生成，避免孤儿任务继续占用模型资源。
#[tauri::command(rename_all = "snake_case")]
pub async fn smart_execute(
    user_input: String,
    current_content: Option<String>,
    style_weight: Option<i32>,
    intent_classification: Option<crate::intent::WritingIntentClassification>,
    scene_id: Option<String>,
    selected_text: Option<String>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::planner::PlanExecutionResult, AppError> {
    // v0.15.5: 从 AppConfig 读取硬超时，默认 600s（与 serde 默认一致）
    // v0.18.1 修复：使用 app_data_dir() 而非
    // current_dir()，确保读取到用户实际配置
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
    let smart_execute_timeout = crate::config::AppConfig::load(&app_dir)
        .map(|c| c.smart_execute_total_timeout_secs)
        .unwrap_or(600u64);
    let pool_inner = pool.inner().clone();

    match tokio::time::timeout(
        std::time::Duration::from_secs(smart_execute_timeout),
        smart_execute_inner(
            user_input,
            current_content,
            style_weight,
            intent_classification,
            scene_id,
            selected_text,
            pool_inner,
            app_handle.clone(),
        ),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => {
            log::error!(
                "[smart_execute] 整体超时（{}秒），正在取消所有进行中的 LLM 生成",
                smart_execute_timeout
            );
            // 取消所有进行中的 LLM 生成，避免孤儿任务
            let llm = crate::llm::LlmService::new(app_handle.clone());
            llm.cancel_all_generations();
            // 清理后台活动状态
            use tauri::Emitter;
            let _ = app_handle.emit(
                "smart-execute-progress",
                crate::planner::SmartExecuteProgress {
                    stage: "timeout".to_string(),
                    message: format!(
                        "智能创作整体超时（{}秒），已自动取消。请检查模型服务是否正常运行。",
                        smart_execute_timeout
                    ),
                    step_number: 0,
                    total_steps: 0,
                },
            );
            Err(AppError::llm_timeout(smart_execute_timeout * 1000))
        }
    }
}

/// smart_execute 内部实现（无整体超时，由外层 smart_execute 包裹）
async fn smart_execute_inner(
    user_input: String,
    current_content: Option<String>,
    style_weight: Option<i32>,
    intent_classification: Option<crate::intent::WritingIntentClassification>,
    scene_id: Option<String>,
    selected_text: Option<String>,
    pool: crate::db::DbPool,
    app_handle: AppHandle,
) -> Result<crate::planner::PlanExecutionResult, AppError> {
    let style_weight = style_weight.unwrap_or(50);
    use tauri::Emitter;

    // 辅助函数：发送 smart_execute 整体进度事件
    let app_handle_for_progress = app_handle.clone();
    let emit_progress =
        move |stage: &str, message: &str, step_number: usize, total_steps: usize| {
            let _ = app_handle_for_progress.emit(
                "smart-execute-progress",
                crate::planner::SmartExecuteProgress {
                    stage: stage.to_string(),
                    message: message.to_string(),
                    step_number,
                    total_steps,
                },
            );
        };

    emit_progress("loading_context", "正在读取故事信息...", 1, 5);
    log::info!(
        "[smart_execute] START user_input={:?} current_content_len={}",
        user_input,
        current_content
            .as_ref()
            .map(|c| c.chars().count())
            .unwrap_or(0)
    );

    // 构建 PlanContext：从当前系统状态推断
    // v0.9.5: 将同步 DB 查询移入 spawn_blocking，避免阻塞 tokio worker
    // v0.25.0: 对 DB 查询加 retry_with_backoff，容忍偶发锁定
    log::info!("[smart_execute] STEP 1/5 loading stories+chapters (spawn_blocking + retry)...");
    let t1 = std::time::Instant::now();
    let pool_for_loader = pool.clone();
    let context_load = retry_with_backoff(
        move || {
            let pool = pool_for_loader.clone();
            async move {
                // flatten Result<Result<...>, JoinError> into
                // Result<SmartExecuteContext, AppError>
                let inner = tokio::task::spawn_blocking(
                    move || -> Result<SmartExecuteContext, AppError> {
                        let stories =
                            StoryRepository::new(pool.clone()).get_all().map_err(|e| {
                                AppError::internal(format!(
                                    "[smart_execute] Failed to load stories: {}",
                                    e
                                ))
                            })?;
                        let current_story = stories.first().cloned();
                        let current_story_id = current_story.as_ref().map(|s| s.id.clone());
                        let chapters = if let Some(ref story_id) = current_story_id {
                            ChapterRepository::new(pool.clone())
                                .get_by_story(story_id)
                                .map_err(|e| {
                                    AppError::internal(format!(
                                        "[smart_execute] Failed to load chapters: {}",
                                        e
                                    ))
                                })?
                        } else {
                            vec![]
                        };
                        Ok((stories, current_story, current_story_id, chapters))
                    },
                )
                .await
                .map_err(|e| {
                    AppError::internal(format!("[smart_execute] 上下文加载任务失败: {}", e))
                })?;
                inner
            }
        },
        2,
        50,
        500,
        "smart_execute context load",
    )
    .await;

    let (stories, current_story, current_story_id, chapters) = match context_load {
        crate::error_recovery::RecoveryOutcome::Success(ctx) => ctx,
        crate::error_recovery::RecoveryOutcome::RetriedSuccess(ctx, attempts) => {
            log::warn!("[smart_execute] 上下文加载经 {} 次重试后成功", attempts);
            ctx
        }
        crate::error_recovery::RecoveryOutcome::DegradedSuccess(ctx, reason) => {
            log::warn!("[smart_execute] 上下文加载降级成功: {}", reason);
            ctx
        }
        crate::error_recovery::RecoveryOutcome::Failed(e) => return Err(e),
    };
    log::info!(
        "[smart_execute] STEP 1/5 done in {:?} (stories={}, chapters={}, story_id={:?})",
        t1.elapsed(),
        stories.len(),
        chapters.len(),
        current_story_id
    );

    let chapter_count = chapters.len();

    // [DEBUG] 获取 WorkflowLogger，关键日志点写入
    // creative_workflow.log（诊断卡片自动收集）
    let wf_logger = app_handle
        .try_state::<std::sync::Arc<crate::workflow_logger::WorkflowLogger>>()
        .map(|l| l.clone());
    let current_content_len = current_content.as_ref().map(|s| s.len()).unwrap_or(0);
    let wf = |phase: &str, message: &str, details: Option<serde_json::Value>| {
        if let Some(ref l) = wf_logger {
            l.info(phase, message, details);
        }
    };

    // 优先使用前端传来的实时编辑器内容，其次回退到数据库中最后一章的 Scene
    // 聚合内容
    let current_content_full = current_content
        .filter(|c| !c.trim().is_empty())
        .or_else(|| {
            chapters
                .last()
                .and_then(|c| ChapterRepository::new(pool.clone()).get_content(&c.id).ok())
        });
    // 预览仅用于 prompt 上下文 / has_current_content，超长时从尾部截断；
    // previous_content 落库必须用上面的 current_content_full 全文（见下方
    // prev_content_for_record），rollback 会将其原样写回，存截断预览会丢内容。
    let current_content_preview = current_content_full.clone().map(|content| {
        let max_chars = 6000;
        let total = content.chars().count();
        if total > max_chars {
            // 从尾部截断：保留最后 max_chars 个字符，前面加省略号
            let skip = total - max_chars;
            let preview: String = content.chars().skip(skip).collect();
            format!("...(前{}字已省略)\n{}", skip, preview)
        } else {
            content
        }
    });

    // v0.30.11: 用 LLM 写作意图分类替代 is_novel_creation_intent 朴素子串匹配
    // （"讲一个 bookstore 的故事"会命中 "story" 误触发创世）。前端在
    // smart_execute 前调 classify_intent 取得分类并传入（避免重复
    // LLM）；未提供时后端兜底自调 classify_writing_intent（8s 超时 +
    // 保守兜底 is_new_novel=false）。
    let has_existing_story = !stories.is_empty();
    let has_current_content = current_content_preview.is_some();
    let mut classification = match intent_classification.clone() {
        Some(c) => c,
        None => {
            log::info!("[smart_execute] 前端未传意图分类，后端兜底 LLM 分类");
            let parser = crate::intent::IntentParser::new(app_handle.clone());
            parser
                .classify_writing_intent(&user_input, has_existing_story, has_current_content)
                .await
        }
    };
    crate::intent::apply_asset_refresh_override(&mut classification, &user_input);
    let is_bootstrap_intent = classification.is_new_novel;

    wf(
        "smart_execute.start",
        "smart_execute 开始",
        Some(serde_json::json!({
            "is_bootstrap_intent": is_bootstrap_intent,
            "user_input": &user_input,
            "current_content_len": current_content_len,
        })),
    );

    if is_bootstrap_intent {
        // 创世 2.0 走 agency 多代理框架：进度镜像到 smart-execute-progress，
        // 返回形状满足前端兼容契约（见 P2 计划 Global Constraints）。
        // total_timeout
        // 读取沿用函数顶部现有代码（config.smart_execute_total_timeout_secs，
        // 默认 600）。
        let app_dir = app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let total_timeout = crate::config::AppConfig::load(&app_dir)
            .map(|c| c.smart_execute_total_timeout_secs)
            .unwrap_or(600u64);
        log::warn!(
            "[smart_execute] 检测到小说创建意图，启动 agency 创世流程，total_timeout={}s",
            total_timeout
        );
        emit_progress("analyzing", "创世 2.0 启动（多代理）", 0, 6);
        let run_id = Uuid::new_v4().to_string();
        let coordinator =
            crate::agency::coordinator::AgencyCoordinator::new(app_handle.clone(), pool.clone());
        // 进度镜像：agency phase → smart-execute-progress
        let sink: crate::agency::coordinator::ProgressSink = std::sync::Arc::new({
            let app = app_handle.clone();
            move |phase: &str, status: &str, message: &str| {
                let step = match phase {
                    "concept" => 1,
                    "assets" => 2,
                    "writing" => 3,
                    "review" | "revision" => 4,
                    "assembly" => 5,
                    _ => 6,
                };
                let _ = app.emit(
                    "smart-execute-progress",
                    crate::planner::SmartExecuteProgress {
                        stage: if status == "running" {
                            phase.to_string()
                        } else {
                            status.to_string()
                        },
                        message: message.to_string(),
                        step_number: step,
                        total_steps: 6,
                    },
                );
            }
        });
        let genesis_future = coordinator.run_genesis_with_sink(&run_id, &user_input, Some(sink));
        match tokio::time::timeout(
            std::time::Duration::from_secs(total_timeout),
            genesis_future,
        )
        .await
        {
            Ok(Ok(result)) => {
                // 取装配场景正文（final_content
                // 契约：完整第一章正文，非摘要文案）
                let pool_c = pool.clone();
                let scene_id = result.scene_id.clone();
                let content = tokio::task::spawn_blocking(move || -> Result<String, AppError> {
                    let scene = crate::db::repositories::SceneRepository::new(pool_c)
                        .get_by_id(&scene_id)
                        .map_err(AppError::from)?
                        .ok_or_else(|| AppError::from("装配场景不存在"))?;
                    // v0.30.46 fix: content
                    // 为空时返回错误而非静默吞掉，避免前端拿到空白正文。
                    let content = scene.content.unwrap_or_default();
                    if content.trim().is_empty() {
                        return Err(AppError::from(
                            "装配场景正文为空（scenes.content 为空），请检查生成链路",
                        ));
                    }
                    Ok(content)
                })
                .await
                .map_err(|e| AppError::from(format!("scene read join error: {}", e)))??;

                // 与旧路径一致的通知：发射
                // story_created，让前端立即进入工作台（签名见原 :377）
                let _ = crate::state_sync::StateSync::emit_story_created(
                    &app_handle,
                    &result.story_id,
                    "新故事",
                );

                // record_ai_operation（沿用原 :561-586
                // 代码，operation_type="bootstrap"，
                // metadata 记 run_id/story_id）；同步 DB 写入移入
                // spawn_blocking， 避免阻塞 tokio worker 导致
                // invoke 延迟 resolve。
                let pool_for_record = pool.clone();
                let input_for_record = user_input.clone();
                let sid_for_record = result.story_id.clone();
                let sid_session = run_id.clone();
                let sid_meta = result.story_id.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = record_ai_operation(
                        &pool_for_record,
                        crate::db::CreateAiOperationRequest {
                            story_id: sid_for_record,
                            scene_id: None,
                            chapter_id: None,
                            operation_type: "bootstrap".to_string(),
                            operation_name: "小说创世".to_string(),
                            input_summary: Some(input_for_record),
                            output_summary: None,
                            previous_content: None,
                            new_content: None,
                            metadata: Some(
                                serde_json::json!({"session_id": sid_session, "story_id": sid_meta})
                                    .to_string(),
                            ),
                        },
                    );
                });

                emit_progress("completed", "小说创世完成", 6, 6);
                return Ok(
                    crate::agency::coordinator::AgencyCoordinator::build_bootstrap_result(
                        &result, content, &run_id,
                    ),
                );
            }
            Ok(Err(e)) => {
                log::error!("[smart_execute] agency 创世失败: {}", e);
                emit_progress("error", &format!("创世失败: {}", e), 6, 6);
                return Err(e);
            }
            Err(_) => {
                // 超时：定点取消本 run 在途 LLM 调用，保留 LLM_TIMEOUT
                // 语义（用法见 :86）
                let llm = crate::llm::LlmService::new(app_handle.clone());
                crate::agency::coordinator::cancel_requests_for_run(&llm, &run_id);
                // 补落终态：超时臂直接 return，协调器的 finish_run 不一定执行，
                // 不落 failed 会残留 running 僵尸 run
                // 卡死该故事续写（finish_run 终态守护下幂等）。
                let pool_t = pool.clone();
                let rid_t = run_id.clone();
                let _ =
                    tokio::task::spawn_blocking(move || {
                        let _ = crate::agency::repository::AgencyRepository::new(pool_t)
                            .finish_run(&rid_t, "failed", None, Some("timeout"));
                    })
                    .await;
                emit_progress("timeout", "创世超时", 6, 6);
                return Err(AppError::llm_timeout(total_timeout * 1000));
            }
        }
    }

    if classification.task_type
        == crate::creative_engine::asset_capability_manifest::AssetTaskType::AssetRefresh
    {
        let story_id = current_story_id.clone().ok_or_else(|| {
            AppError::validation_failed("请先在左侧选择或创建一个作品", Some("no_story_selected"))
        })?;
        log::warn!("[smart_execute] 按正文重写设定，story_id={story_id}");
        emit_progress("executing", "正在按正文重写设定...", 2, 4);
        let coordinator =
            crate::agency::coordinator::AgencyCoordinator::new(app_handle.clone(), pool.clone());
        let result = coordinator
            .run_asset_refresh(&story_id, scene_id.as_deref(), &user_input)
            .await?;
        emit_progress("completed", "设定已按正文更新", 4, 4);
        return Ok(result);
    }

    // 续写走 Agency Append：硬门——有划词/内联选区则留给 PlanExecutor Full，禁止
    // Append。 current_scene_id 在 Phase 3
    // 才加载；此处禁止「最新有内容场景」回退（前端必须传 scene_id）。
    if crate::agency::persist::should_agency_append_continue(
        classification.is_continuation,
        selected_text.as_deref(),
    ) {
        let persist = crate::agency::persist::resolve_persist_mode(true, scene_id.clone(), false)?;
        let story_id = current_story_id.clone().ok_or_else(|| {
            AppError::validation_failed("请先在左侧选择或创建一个作品", Some("no_story_selected"))
        })?;
        log::warn!("[smart_execute] 续写走 Agency Append，story_id={story_id}");
        emit_progress("executing", "Agency 续写中...", 3, 5);
        let run_id = Uuid::new_v4().to_string();
        let coordinator =
            crate::agency::coordinator::AgencyCoordinator::new(app_handle.clone(), pool.clone());
        let result = coordinator
            .run_continue(
                &run_id,
                &story_id,
                persist,
                &user_input,
                current_content_full.as_deref(),
            )
            .await?;
        emit_progress("completed", "续写完成", 5, 5);
        return Ok(crate::planner::PlanExecutionResult {
            success: true,
            steps_completed: 1,
            final_content: Some(result.increment),
            messages: vec!["续写完成".into()],
            error: None,
            result_kind: None,
            asset_refresh_draft: None,
        });
    }

    // Phase 3: 加载场景结构信息 + 增强上下文
    wf(
        "smart_execute.continue_write.enter",
        "进入续写模式（加载场景结构 + 增强上下文）",
        Some(serde_json::json!({
            "current_content_len": current_content_len,
        })),
    );
    let (
        _scenes,
        scene_count,
        scenes_summary,
        current_scene_id,
        current_scene_stage,
        total_word_count,
        latest_chapter_word_count,
        story_progress,
        world_building_summary,
        character_list,
        foreshadowing_status,
        style_dna_info,
        mcp_tools_available,
        chapter_number,
        deep_insight_summary,
    ) = if let Some(ref story_id) = current_story_id {
        emit_progress("loading_context", "正在读取章节与场景结构...", 1, 5);
        log::info!(
            "[smart_execute] STEP 2/5 loading scenes (spawn_blocking, story_id={})...",
            story_id
        );
        let t2 = std::time::Instant::now();
        let pool_for_scenes = pool.clone();
        let story_id_for_scenes = story_id.clone();
        let scenes = tokio::task::spawn_blocking(move || {
            let scene_repo = crate::db::repositories::SceneRepository::new(pool_for_scenes);
            scene_repo.get_by_story(&story_id_for_scenes)
        })
        .await
        .map_err(|e| AppError::internal(format!("[smart_execute] 场景加载任务失败: {}", e)))?
        .map_err(|e| AppError::internal(format!("[smart_execute] Failed to load scenes: {}", e)))?;
        log::info!(
            "[smart_execute] STEP 2/5 done in {:?} (scenes={})",
            t2.elapsed(),
            scenes.len()
        );
        let scene_count = scenes.len();

        let scenes_summary: Vec<crate::planner::SceneStructureSummary> = scenes
            .iter()
            .map(|s| {
                let word_count = s.content.as_ref().map(|c| c.chars().count()).unwrap_or(0)
                    + s.draft_content
                        .as_ref()
                        .map(|c| c.chars().count())
                        .unwrap_or(0);
                crate::planner::SceneStructureSummary {
                    scene_id: s.id.clone(),
                    sequence_number: s.sequence_number,
                    title: s.title.clone(),
                    execution_stage: s.execution_stage.clone(),
                    has_content: s.content.is_some() || s.draft_content.is_some(),
                    word_count,
                }
            })
            .collect();

        // 当前场景 = 最新有内容的场景，或最新场景
        let current_scene = scenes
            .iter()
            .filter(|s| s.content.is_some() || s.draft_content.is_some())
            .max_by_key(|s| s.sequence_number)
            .or_else(|| scenes.iter().max_by_key(|s| s.sequence_number));

        let current_scene_id = current_scene.map(|s| s.id.clone());
        let current_scene_stage = current_scene.and_then(|s| s.execution_stage.clone());
        let chapter_number = current_scene.map(|s| s.sequence_number).unwrap_or(1);

        let total_word_count = chapters
            .iter()
            .filter_map(|c| c.word_count)
            .map(|w| w as usize)
            .sum::<usize>()
            + scenes_summary.iter().map(|s| s.word_count).sum::<usize>();

        let latest_chapter_word_count = chapters
            .last()
            .and_then(|c| c.word_count)
            .map(|w| w as usize)
            .unwrap_or(0);

        // 故事进度判断
        let story_progress = if scene_count == 0 {
            "just_started".to_string()
        } else {
            let completed_scenes = scenes_summary.iter().filter(|s| s.has_content).count();
            let ratio = if scene_count > 0 {
                completed_scenes as f32 / scene_count as f32
            } else {
                0.0
            };
            if ratio < 0.15 {
                "just_started".to_string()
            } else if ratio < 0.4 {
                "developing".to_string()
            } else if ratio < 0.7 {
                "midpoint".to_string()
            } else if ratio < 0.9 {
                "climax".to_string()
            } else {
                "resolution".to_string()
            }
        };

        emit_progress("loading_context", "正在读取世界观、角色与伏笔...", 1, 5);
        log::info!(
            "[smart_execute] STEP 3/5 loading world/chars/foreshadowing (spawn_blocking)..."
        );
        let t3 = std::time::Instant::now();

        // v0.9.5: 将多个同步上下文查询批量移入 spawn_blocking
        let pool_for_context = pool.clone();
        let story_id_for_context = story_id.clone();
        let (world_building_summary, character_list, foreshadowing_status, deep_insight_summary) =
            tokio::task::spawn_blocking(move || {
                // 世界观摘要
                let wb_repo =
                    crate::db::repositories::WorldBuildingRepository::new(pool_for_context.clone());
                let world_building_summary = wb_repo
                    .get_by_story(&story_id_for_context)
                    .ok()
                    .flatten()
                    .map(|wb| {
                        let rules_summary = wb
                            .rules
                            .iter()
                            .filter(|r| r.importance >= 7)
                            .map(|r| {
                                format!("{}: {}", r.name, r.description.as_deref().unwrap_or(""))
                            })
                            .collect::<Vec<_>>()
                            .join("; ");
                        format!("概念：{}；核心规则：{}", wb.concept, rules_summary)
                    });

                // 角色列表
                let char_repo =
                    crate::db::repositories::CharacterRepository::new(pool_for_context.clone());
                let character_list = char_repo
                    .get_by_story(&story_id_for_context)
                    .ok()
                    .map(|chars| {
                        chars
                            .iter()
                            .map(|c| {
                                let role = c.background.as_deref().unwrap_or("主要角色");
                                format!("{}（{}）", c.name, role)
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // 活跃伏笔
                let foreshadowing_tracker =
                    crate::creative_engine::foreshadowing::ForeshadowingTracker::new(
                        pool_for_context.clone(),
                    );
                let foreshadowing_status = foreshadowing_tracker
                    .get_unresolved(&story_id_for_context)
                    .ok()
                    .map(|records| records.into_iter().take(5).map(|r| r.content).collect())
                    .unwrap_or_default();

                // v0.22.5: 加载最新深度洞察摘要
                let deep_insight_summary =
                    crate::db::repositories::StorySummaryRepository::new(pool_for_context.clone())
                        .get_summary_by_type(&story_id_for_context, "deep_insight")
                        .ok()
                        .flatten()
                        .map(|s| s.content.chars().take(800).collect::<String>());

                (
                    world_building_summary,
                    character_list,
                    foreshadowing_status,
                    deep_insight_summary,
                )
            })
            .await
            .map_err(|e| {
                AppError::internal(format!("[smart_execute] 上下文加载任务失败: {}", e))
            })?;
        log::info!("[smart_execute] STEP 3/5 done in {:?}", t3.elapsed());

        emit_progress("loading_context", "正在读取风格配置...", 1, 5);
        log::info!("[smart_execute] STEP 4/5 loading style+MCP...");
        let t4 = std::time::Instant::now();

        // 风格DNA / 风格混合
        // v0.14.0: spawn_blocking 包裹同步 DB 查询
        let style_dna_info = {
            use crate::{
                db::repositories::StoryStyleConfigRepository, domain::style::StyleBlendConfig,
            };

            let pool_for_style = pool.clone();
            let story_for_style = current_story.clone();
            let blend_info = tokio::task::spawn_blocking(move || -> Option<String> {
                let story = story_for_style.as_ref()?;
                let blend_repo = StoryStyleConfigRepository::new(pool_for_style);
                if let Ok(Some(config)) = blend_repo.get_active_by_story(&story.id) {
                    if let Ok(blend) = serde_json::from_str::<StyleBlendConfig>(&config.blend_json)
                    {
                        let comps = blend
                            .components
                            .iter()
                            .map(|c| format!("{}:{:.0}%", c.dna_name, c.weight * 100.0))
                            .collect::<Vec<_>>()
                            .join(", ");
                        Some(format!("风格混合 [{}]: {}", blend.name, comps))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .await
            .unwrap_or(None);

            // 回退到单一风格DNA
            if blend_info.is_some() {
                blend_info
            } else {
                current_story
                    .as_ref()
                    .and_then(|s| s.style_dna_id.clone())
                    .map(|dna_id| format!("风格DNA ID: {}", dna_id))
            }
        };

        // 异步加载MCP工具列表
        log::info!("[smart_execute] STEP 4a acquiring MCP_CONNECTIONS lock...");
        let mcp_tools_available = {
            let connections = crate::MCP_CONNECTIONS.lock().await;
            log::info!(
                "[smart_execute] STEP 4a MCP lock acquired, {} connections",
                connections.len()
            );
            connections
                .iter()
                .flat_map(|(_id, client)| {
                    client
                        .get_tools()
                        .iter()
                        .map(|t| format!("{}: {}", t.name, t.description))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        };

        log::info!(
            "[smart_execute] STEP 4/5 done in {:?} (context loading complete)",
            t4.elapsed()
        );

        (
            scenes,
            scene_count,
            scenes_summary,
            current_scene_id,
            current_scene_stage,
            total_word_count,
            latest_chapter_word_count,
            story_progress,
            world_building_summary,
            character_list,
            foreshadowing_status,
            style_dna_info,
            mcp_tools_available,
            chapter_number,
            deep_insight_summary,
        )
    } else {
        (
            vec![],
            0,
            vec![],
            None,
            None,
            0,
            0,
            "no_story".to_string(),
            None,
            vec![],
            vec![],
            None,
            vec![],
            1,
            None,
        )
    };

    // v0.15.3: 续写请求但没有作品时，返回友好错误而非让 PlanExecutor 崩溃
    if current_story_id.is_none() {
        return Err(AppError::validation_failed(
            "请先在左侧选择或创建一个作品，再使用智能创作功能",
            Some("no_story_selected"),
        ));
    }

    emit_progress("context_loaded", "故事上下文加载完成", 2, 5);

    // Clone values before they are moved into plan_context
    let story_id_for_record = current_story_id.clone();
    let scene_id_for_record = current_scene_id.clone();
    let chapter_id_for_record = chapters.last().map(|c| c.id.clone());
    let input_for_record = user_input.clone();
    // previous_content 必须存全文而非截断预览：rollback 会把它原样写回
    // Scene 作为章节全部内容，存截断预览会导致超长手稿回滚丢内容。
    let prev_content_for_record = current_content_full;

    // v0.10.0: 构建当前故事的创作策略上下文
    // v0.14.0: spawn_blocking 包裹同步 DB 查询
    // v0.17.1: 输入清晰度检测 -> 后端透明补全中文叙事四元组
    // v0.30.11: 优先用 LLM 分类的 input_clarity（smart_execute 入口已分类），
    // detect_input_clarity 仅作无分类时的字面兜底。
    let strategy_story = current_story.clone();
    let strategy_pool = pool.clone();
    let input_clarity = classification.input_clarity;
    let selected_strategy = tokio::task::spawn_blocking(move || {
        build_selected_strategy(&strategy_story, &strategy_pool, input_clarity)
    })
    .await
    .unwrap_or(None);

    // v0.31.0: 推荐方法论写回（story 无显式值才落库，显式值不覆盖）
    {
        let wb_pool = pool.clone();
        let wb_story = current_story.clone();
        let wb_strategy = selected_strategy.clone();
        let _ = tokio::task::spawn_blocking(move || {
            persist_recommended_methodology(&wb_pool, &wb_story, &wb_strategy);
        })
        .await;
    }

    let plan_context = crate::planner::PlanContext {
        current_story_id,
        has_story: !stories.is_empty(),
        has_chapters: !chapters.is_empty(),
        chapter_count,
        current_content_preview,
        user_input: user_input.clone(),
        scene_count,
        scenes_summary,
        current_scene_id,
        current_scene_stage,
        total_word_count,
        latest_chapter_word_count,
        story_progress,
        world_building_summary,
        character_list,
        foreshadowing_status,
        style_dna_info,
        mcp_tools_available,
        deep_insight_summary,
        selected_text: selected_text.clone(),
        style_weight,
        chapter_number,
        selected_strategy,
        intent_classification: Some(classification.clone()),
    };

    // v0.31.x: 非散文审计意图自动路由——绕过 plan pipeline，直达 inspector
    // 审计。 此前 inspector-only 审计计划成功但 final_content=None，被误报
    // "创作计划未能生成有效内容"；更早版本则把审查报告当正文返回、
    // 被前端追加进手稿。现在报告以 result_kind="audit_report" 返回，
    // 前端渲染为报告消息而非正文。专用审计 UI（audit_story / SceneAuditPanel）
    // 不受影响。
    if crate::planner::is_non_prose_audit_intent(Some(&classification)) {
        log::info!("[smart_execute] 检测到非散文审计意图，路由到专用审计路径");
        emit_progress("executing", "正在审计当前内容...", 3, 5);
        let executor = crate::planner::PlanExecutor::new(app_handle.clone());
        let result = executor
            .execute_audit_report(&plan_context)
            .await
            .map_err(|e| {
                emit_progress("error", &format!("审计失败: {}", e), 5, 5);
                e
            })?;
        emit_progress("completed", "审计完成", 5, 5);
        if let Some(ref story_id) = story_id_for_record {
            record_ai_operation(
                &pool,
                crate::db::CreateAiOperationRequest {
                    story_id: story_id.clone(),
                    scene_id: scene_id_for_record,
                    chapter_id: chapter_id_for_record,
                    operation_type: "smart_execute_audit".to_string(),
                    operation_name: "智能审计".to_string(),
                    input_summary: Some(input_for_record),
                    output_summary: result
                        .final_content
                        .as_ref()
                        .map(|c| c.chars().take(200).collect()),
                    previous_content: prev_content_for_record,
                    // 报告不是正文，不作为 new_content 落库，
                    // 避免撤销/恢复通道把手稿替换为审计报告
                    new_content: None,
                    metadata: Some(serde_json::json!({"result_kind": "audit_report"}).to_string()),
                },
            );
        }
        return Ok(result);
    }

    // 执行计划（内部会自动检查模板库并生成计划）
    emit_progress("executing", "开始执行创作计划...", 3, 5);
    log::info!("[smart_execute] STEP 5/5 calling PlanExecutor::execute_with_context...");
    let executor = crate::planner::PlanExecutor::new(app_handle);
    let t5 = std::time::Instant::now();
    let result = executor
        .execute_with_context(&plan_context)
        .await
        .map_err(|e| {
            emit_progress("error", &format!("计划执行失败: {}", e), 5, 5);
            AppError::internal(format!("[smart_execute] Plan execution failed: {}", e))
        })?;
    log::info!(
        "[smart_execute] STEP 5/5 done in {:?}, total elapsed: {:?}",
        t5.elapsed(),
        t1.elapsed()
    );
    // v0.15.2: 仅在实际成功时才发 completed，失败时发 error
    // 修复 v0.15.0/v0.15.1 中"已完成"事件在失败前就发射的 bug
    let is_empty_content = result
        .final_content
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if !result.success || is_empty_content {
        emit_progress("error", "创作计划未能生成有效内容", 5, 5);
        // 优先透传底层错误（如 LLM_TIMEOUT），让前端能展示"检查模型"等恢复动作
        if let Some(ref err) = result.error {
            return Err(err.clone());
        }
        let error_msg = build_plan_failure_message(&result.messages, is_empty_content);
        return Err(AppError::internal(error_msg));
    }

    // 仅在真正成功时发射完成事件
    emit_progress("completed", "创作计划执行完成", 5, 5);

    // Record AI operation for non-bootstrap generation
    if let Some(ref story_id) = story_id_for_record {
        record_ai_operation(
            &pool,
            crate::db::CreateAiOperationRequest {
                story_id: story_id.clone(),
                scene_id: scene_id_for_record,
                chapter_id: chapter_id_for_record,
                operation_type: "smart_execute".to_string(),
                operation_name: "AI 续写".to_string(),
                input_summary: Some(input_for_record),
                output_summary: result
                    .final_content
                    .as_ref()
                    .map(|c| c.chars().take(200).collect()),
                previous_content: prev_content_for_record,
                new_content: result.final_content.clone(),
                metadata: Some(
                    serde_json::json!({"steps_completed": result.steps_completed}).to_string(),
                ),
            },
        );
    }

    Ok(result)
}

/// 获取输入栏智能提示 — 由LLM根据当前故事上下文生成建议
#[tauri::command(rename_all = "snake_case")]
pub async fn get_input_hint(
    _app_handle: AppHandle,
    current_content: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<String, AppError> {
    let pool = pool.inner().clone();

    // 获取当前故事状态
    let stories = StoryRepository::new(pool.clone())
        .get_all()
        .map_err(|e| AppError::internal(format!("Failed to load stories: {}", e)))?;
    let current_story = stories.first().cloned();
    let current_story_id = current_story.as_ref().map(|s| s.id.clone());

    let chapters = if let Some(ref story_id) = current_story_id {
        ChapterRepository::new(pool.clone())
            .get_by_story(story_id)
            .map_err(|e| AppError::internal(format!("Failed to load chapters: {}", e)))?
    } else {
        vec![]
    };

    let content_preview = current_content
        .filter(|c| !c.trim().is_empty())
        .or_else(|| {
            chapters
                .last()
                .and_then(|c| ChapterRepository::new(pool.clone()).get_content(&c.id).ok())
        });

    let word_count = content_preview
        .as_ref()
        .map(|c| c.chars().count())
        .unwrap_or(0);

    // 构建规则驱动的候选建议
    let mut candidates: Vec<String> = vec![];

    if stories.is_empty() {
        candidates.push("写一个新故事".to_string());
        candidates.push("创作一部科幻小说".to_string());
        candidates.push("我想写一个关于...的故事".to_string());
    } else if chapters.is_empty() {
        candidates.push("创建第一章".to_string());
        candidates.push("开始写作".to_string());
    } else if word_count < 100 {
        candidates.push("续写".to_string());
        candidates.push("展开这个场景".to_string());
        candidates.push("增加环境描写".to_string());
    } else if word_count < 1000 {
        candidates.push("续写下一段".to_string());
        candidates.push("润色当前段落".to_string());
        candidates.push("增加对话".to_string());
    } else {
        candidates.push("续写".to_string());
        candidates.push("调整节奏".to_string());
        candidates.push("生成古典评点".to_string());
        candidates.push("优化对话".to_string());
    }

    // 如果有角色，添加角色相关建议
    if let Some(ref story_id) = current_story_id {
        let char_repo = crate::db::repositories::CharacterRepository::new(pool.clone());
        if let Ok(chars) = char_repo.get_by_story(story_id) {
            if let Some(first_char) = chars.first() {
                candidates.push(format!("让{}出场", first_char.name));
            }
            if chars.len() >= 2 {
                candidates.push("增加人物冲突".to_string());
            }
        }

        // 如果有场景信息，添加场景相关建议
        let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
        if let Ok(scenes) = scene_repo.get_by_story(story_id) {
            let scene_count = scenes.len();
            let has_content = scenes
                .iter()
                .any(|s| s.content.is_some() || s.draft_content.is_some());
            if scene_count > 0 && !has_content {
                candidates.push("为当前场景写内容".to_string());
            }
        }
    }

    // v0.11.7-hotfix: 不再调用 LLM 生成输入建议。
    // 该 LLM 调用会在输入框获得焦点时自动触发，产生 agent-stage-update
    // 事件并被聚合为
    // 主后台活动，导致用户还没输入任何文字就进入“运行进程”且输入框被禁用。
    // 现在仅使用上面的规则候选，返回零成本且不会阻塞 UI。
    log::debug!(
        "[get_input_hint] Returning rule-based hint for story={:?}, word_count={}",
        current_story_id,
        word_count
    );

    if let Some(hint) = candidates.first() {
        Ok(hint.clone())
    } else {
        Ok("输入指令开始创作".to_string())
    }
}

/// v0.30.24: 纯函数--判断输入是否应跳过 logline 生成。
/// 空输入或 ≥ 100 字符（详细 premise 无需增强）时跳过，与 v0.30.22 `< 100 字符`
/// 触发对齐。
fn should_skip_logline_generation(user_input: &str) -> bool {
    let trimmed = user_input.trim();
    trimmed.is_empty() || trimmed.chars().count() >= 100
}

/// v0.30.24: 纯函数--校验 LLM 返回的 logline 是否有效（非空且 ≥ 10 字符）。
fn is_valid_logline(logline: &str) -> bool {
    let trimmed = logline.trim();
    !trimmed.is_empty() && trimmed.chars().count() >= 10
}

/// v0.30.27: Logline 幽灵提示--用户输入简单创世指令时，后台生成一段可直接
/// 追加到原输入后的增强后缀，前端以输入框内幽灵文本显示；用户按 -> 后，
/// 后缀被追加到原输入，形成完整增强指令再提交给 LLM。
///
/// 当提供 story_id 时，会拉取故事大纲、当前章节大纲、角色列表与最近正文，
/// 渲染 `agency_logline_suffix_contextual` prompt，生成贴合上下文的后缀。
///
/// - 输入为空或 ≥ 100 字符 -> 返回 None
/// - 使用 `agency_logline_suffix` / `agency_logline_suffix_contextual` prompt
///   资产
/// - 15s 超时，失败/超时静默返回 None（不报错，不阻塞 UI）
#[tauri::command(rename_all = "snake_case")]
pub async fn generate_logline_hint(
    user_input: String,
    story_id: Option<String>,
    chapter_number: Option<i32>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<Option<String>, AppError> {
    if should_skip_logline_generation(&user_input) {
        return Ok(None);
    }
    let trimmed = user_input.trim();

    // 尝试构建上下文感知 prompt；失败或无上下文时回退到通用 prompt。
    let system =
        build_contextual_logline_system(story_id.as_deref(), chapter_number, &pool, trimmed)
            .await
            .unwrap_or_else(|| {
                crate::prompts::registry::resolve_prompt_default_with_vars(
                    "agency_logline_suffix",
                    &std::collections::HashMap::new(),
                )
                .unwrap_or_else(|| {
                    "你是故事概念设计师。用户输入了一句简单的创世指令（如'写一部现代间谍小说'）。\
                 请只输出一段应直接追加到该指令后的增强后缀，使其成为包含主角、催化事件、\
                 核心任务与失败后果的强力 logline。不要重复原输入，只输出后缀。"
                        .to_string()
                })
            });
    let user_prompt = format!(
        "用户输入：{}\n\n请生成可直接追加到该输入后的增强后缀。",
        trimmed
    );

    let llm_service = crate::llm::service::LlmService::new(app_handle.clone());
    let labelled = llm_service.generate_for_task_with_system_prompt(
        crate::router::TaskType::Brainstorming,
        user_prompt,
        Some(1024),
        Some(0.7),
        Some("logline_hint"),
        Some(system),
        None,
    );

    match tokio::time::timeout(std::time::Duration::from_secs(15), labelled).await {
        Ok((_, Ok(resp))) => {
            let logline = resp.content.trim().to_string();
            if !is_valid_logline(&logline) {
                log::warn!("[generate_logline_hint] logline 过短或为空，丢弃");
                Ok(None)
            } else {
                log::info!(
                    "[generate_logline_hint] logline 生成成功（{} 字符）",
                    logline.chars().count()
                );
                Ok(Some(logline))
            }
        }
        Ok((_, Err(e))) => {
            log::warn!("[generate_logline_hint] LLM 调用失败，静默降级: {}", e);
            Ok(None)
        }
        Err(_) => {
            log::warn!("[generate_logline_hint] 15s 超时，静默降级");
            Ok(None)
        }
    }
}

/// v0.30.27: 根据已有后台资产构建上下文感知的 logline system prompt。
/// 无 story_id、读取失败或 prompt 资产不存在时返回 None，由调用方回退通用
/// prompt。
async fn build_contextual_logline_system(
    story_id: Option<&str>,
    chapter_number: Option<i32>,
    pool: &State<'_, DbPool>,
    user_input: &str,
) -> Option<String> {
    let story_id = story_id?;

    let pool_inner = pool.inner().clone();
    let story_id_owned = story_id.to_string();

    let ctx = match tokio::task::spawn_blocking(move || {
        build_logline_context_sync(&story_id_owned, chapter_number, &pool_inner)
    })
    .await
    {
        Ok(Ok(ctx)) => ctx,
        Ok(Err(e)) => {
            log::warn!(
                "[generate_logline_hint] 拉取上下文失败，回退通用 prompt: {}",
                e
            );
            return None;
        }
        Err(e) => {
            log::warn!("[generate_logline_hint] 上下文任务 join 失败: {}", e);
            return None;
        }
    };

    let mut vars: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    vars.insert("user_input".to_string(), user_input.to_string());
    vars.insert("story_outline".to_string(), ctx.story_outline);
    vars.insert("scene_outline".to_string(), ctx.scene_outline);
    vars.insert("characters".to_string(), ctx.characters);
    vars.insert("current_content".to_string(), ctx.current_content);
    vars.insert("world_setting".to_string(), ctx.world_setting);

    crate::prompts::registry::resolve_prompt_default_with_vars(
        "agency_logline_suffix_contextual",
        &vars,
    )
}

#[derive(Debug, Default)]
struct LoglineContext {
    story_outline: String,
    scene_outline: String,
    characters: String,
    current_content: String,
    world_setting: String,
}

fn build_logline_context_sync(
    story_id: &str,
    chapter_number: Option<i32>,
    pool: &DbPool,
) -> Result<LoglineContext, AppError> {
    let story_outline_repo = StoryOutlineRepository::new(pool.clone());
    let chapter_repo = ChapterRepository::new(pool.clone());
    let character_repo = CharacterRepository::new(pool.clone());

    let story_outline = story_outline_repo
        .get_by_story(story_id)
        .ok()
        .flatten()
        .map(|o| o.content)
        .unwrap_or_default();

    let chapters = chapter_repo.get_by_story(story_id).unwrap_or_default();
    let target_chapter =
        chapter_number.and_then(|cn| chapters.iter().find(|c| c.chapter_number == cn));

    let scene_outline = target_chapter
        .as_ref()
        .map(|c| c.outline.clone().unwrap_or_default())
        .unwrap_or_default();

    let current_content = target_chapter
        .as_ref()
        .and_then(|c| chapter_repo.get_content(&c.id).ok())
        .map(|s| truncate_chars(&s, 1200))
        .unwrap_or_default();

    let characters = character_repo
        .get_by_story(story_id)
        .unwrap_or_default()
        .iter()
        .map(|c| {
            format!(
                "{}：背景{}；目标{}；性格{}",
                c.name,
                c.background.as_deref().unwrap_or("无"),
                c.goals.as_deref().unwrap_or("无"),
                c.personality.as_deref().unwrap_or("无")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // v0.30.32: 纳入世界观（concept + rules 前3 +
    // history），让增强后缀与世界观规则一致，
    // 不再在不知世界规则下提出违反世界观的设定。与
    // build_continue_writer_context 同源但更精简。
    let world_setting = crate::db::repositories::WorldBuildingRepository::new(pool.clone())
        .get_by_story(story_id)
        .ok()
        .flatten()
        .map(|w| {
            let mut parts = vec![format!("概念：{}", w.concept)];
            if !w.rules.is_empty() {
                let rules = w
                    .rules
                    .iter()
                    .take(3)
                    .map(|r| format!("- {}：{}", r.name, r.description.as_deref().unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join("\n");
                parts.push(format!("核心规则：\n{}", rules));
            }
            if let Some(ref h) = w.history {
                if !h.trim().is_empty() {
                    parts.push(format!("历史：{}", h));
                }
            }
            truncate_chars(&parts.join("\n"), 1000)
        })
        .unwrap_or_default();

    Ok(LoglineContext {
        story_outline,
        scene_outline,
        characters,
        current_content,
        world_setting,
    })
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect::<String>() + "…"
    }
}

/// v0.10.0: 根据 Story 已保存的策略元数据构建 SelectedStrategy
fn build_selected_strategy(
    current_story: &Option<crate::db::Story>,
    pool: &crate::db::DbPool,
    input_clarity: crate::intent::InputClarity,
) -> Option<crate::domain::strategy::SelectedStrategy> {
    let story = current_story.as_ref()?;

    // P3-2: 当 story 未显式设定资产时，尝试按题材自动匹配 GenreProfile，
    // 让四元组推断能生效（审计报告发现 4.2.4：此前直接返回 None，
    // 导致未在 story 上配置资产的用户无法享受四元组增强）。
    let mut auto_genre_profile_id: Option<String> = None;
    let mut auto_canonical_name: Option<String> = None;
    let mut auto_reader_promise: Option<String> = None;
    let mut rationale_parts = Vec::new();
    let mut strategy = crate::domain::strategy::SelectedStrategy::default();
    if story.genre_profile_id.is_none()
        && story.methodology_id.is_none()
        && story.style_dna_id.is_none()
    {
        // 使用 GenreResolver 解析
        // story.genre，支持精确/别名/子串/同义词/复合题材
        if let Some(ref genre) = story.genre {
            if !genre.trim().is_empty() {
                let repo = crate::db::GenreProfileRepository::new(pool.clone());
                let resolver = crate::strategy::GenreResolver::new();
                match resolver.resolve_from_text(genre, &repo) {
                    Ok(matches) if !matches.is_empty() => {
                        if let Some(first) = matches.first() {
                            auto_genre_profile_id = Some(first.profile_id.clone());
                            auto_canonical_name = Some(first.canonical_name.clone());
                        }
                        let secondary: Vec<String> = matches
                            .iter()
                            .skip(1)
                            .map(|m| m.profile_id.clone())
                            .collect();
                        if !secondary.is_empty() {
                            let _ = serde_json::to_string(&secondary).map(|s| {
                                strategy.parameters.insert(
                                    "secondary_genre_profile_ids".to_string(),
                                    serde_json::Value::String(s),
                                );
                            });
                        }
                        log::info!(
                            "[build_selected_strategy] GenreResolver 自动匹配题材画像: {} -> {:?}",
                            genre,
                            matches
                                .iter()
                                .map(|m| &m.canonical_name)
                                .collect::<Vec<_>>()
                        );
                    }
                    _ => {}
                }
            }
        }
        // 若仍未匹配到，则确实无可用资产
        if auto_genre_profile_id.is_none() {
            return None;
        }
    }

    // 优先使用 story 显式设定，回退自动匹配
    strategy.genre_profile_id = story
        .genre_profile_id
        .clone()
        .or_else(|| auto_genre_profile_id.clone());
    strategy.methodology_id = story.methodology_id.clone();
    if let Some(ref dna_id) = story.style_dna_id {
        strategy.style_dna_ids.push(dna_id.clone());
    }

    // v0.17.1: 取出 GenreProfile 的 canonical_name 与 reader_promise
    // 供智能后台预访谈使用（不调 LLM，纯启发式）
    let mut canonical_name: Option<String> = None;
    let mut reader_promise: Option<String> = None;

    if let Some(ref profile_id) = strategy.genre_profile_id {
        let repo = crate::db::GenreProfileRepository::new(pool.clone());
        if let Ok(Some(profile)) = repo.get_by_id(profile_id) {
            rationale_parts.push(format!("体裁画像：{}", profile.genre_name));
            canonical_name = Some(profile.canonical_name.clone());
            reader_promise = profile.reader_promise.clone();

            // v0.22.2: 硬约束——若体裁画像有推荐资产，跳过 LLM 策略选择直接使用
            if story.style_dna_id.is_none() {
                if let Some(ref rec) = profile.recommended_style_dna_ids {
                    if let Ok(ids) = serde_json::from_str::<Vec<String>>(rec) {
                        strategy.style_dna_ids = ids;
                        rationale_parts.push(format!(
                            "风格 DNA（题材推荐）：{:?}",
                            strategy.style_dna_ids
                        ));
                    }
                }
            }
            if story.methodology_id.is_none() {
                if let Some(ref rec) = profile.recommended_methodology_id {
                    strategy.methodology_id = Some(rec.clone());
                    rationale_parts.push(format!("方法论（题材推荐）：{}", rec));
                }
            }
            if let Some(ref rec) = profile.recommended_skill_ids {
                if let Ok(ids) = serde_json::from_str::<Vec<String>>(rec) {
                    strategy.skill_ids = ids;
                }
            }
        } else {
            rationale_parts.push(format!("体裁画像 ID：{}", profile_id));
        }
    }
    // 若自动匹配已取到 canonical_name，优先使用（避免重复查询）
    if canonical_name.is_none() {
        canonical_name = auto_canonical_name.take();
        reader_promise = auto_reader_promise.take();
    }
    if let Some(ref methodology_id) = story.methodology_id {
        rationale_parts.push(format!("方法论：{}", methodology_id));
    }
    if let Some(ref dna_id) = story.style_dna_id {
        rationale_parts.push(format!("风格 DNA：{}", dna_id));
    }

    // v0.31: 优先读取向导持久化的策略四元组（stories.strategy_json）。
    // 放在 infer_narrative_quartet 之前：持久化字段先占位，启发式只补
    // 缺失字段（infer 对已是 Some 的字段不覆盖）。NULL / 解析失败
    // （旧数据）跳过本段，行为与现状完全一致。
    if let Some(ref json) = story.strategy_json {
        match serde_json::from_str::<crate::domain::strategy::SelectedStrategy>(json) {
            Ok(persisted) => {
                let mut loaded = false;
                if persisted.emotional_payoff.is_some() {
                    strategy.emotional_payoff = persisted.emotional_payoff;
                    loaded = true;
                }
                if persisted.pressure_relationship_id.is_some() {
                    strategy.pressure_relationship_id = persisted.pressure_relationship_id;
                    loaded = true;
                }
                if persisted.conflict_arena.is_some() {
                    strategy.conflict_arena = persisted.conflict_arena;
                    loaded = true;
                }
                if !persisted.story_engine_ids.is_empty() {
                    strategy.story_engine_ids = persisted.story_engine_ids;
                    loaded = true;
                }
                if !persisted.beat_card_ids.is_empty() {
                    strategy.beat_card_ids = persisted.beat_card_ids;
                    loaded = true;
                }
                if loaded {
                    rationale_parts.push("策略四元组（向导持久化）".to_string());
                }
            }
            Err(e) => {
                log::warn!(
                    "[build_selected_strategy] strategy_json 解析失败，回退启发式: {}",
                    e
                );
            }
        }
    }

    // v0.17.1: 模糊或半确定输入时透明补全中文叙事四元组
    crate::strategy::infer_narrative_quartet(
        &mut strategy,
        canonical_name.as_deref(),
        reader_promise.as_deref(),
        input_clarity,
    );

    if strategy.emotional_payoff.is_some()
        || strategy.pressure_relationship_id.is_some()
        || !strategy.story_engine_ids.is_empty()
        || !strategy.beat_card_ids.is_empty()
    {
        rationale_parts.push(format!("智能后台四元组（{}）", input_clarity.as_str()));
    }

    strategy.rationale = rationale_parts.join("，");
    Some(strategy)
}

/// v0.31.0: 推荐方法论写回——story 无显式 methodology_id 且推荐存在时落库，
/// 后续续写与 WriteTimeBundle 加载可直接读取。有显式值（用户选择或此前写回）
/// 不覆盖。返回是否发生了写回。
fn persist_recommended_methodology(
    pool: &crate::db::DbPool,
    current_story: &Option<crate::db::Story>,
    selected_strategy: &Option<crate::domain::strategy::SelectedStrategy>,
) -> bool {
    let story = match current_story {
        Some(s) => s,
        None => return false,
    };
    if story
        .methodology_id
        .as_deref()
        .map(|m| !m.trim().is_empty())
        .unwrap_or(false)
    {
        return false; // 显式值不覆盖
    }
    let recommended = match selected_strategy
        .as_ref()
        .and_then(|s| s.methodology_id.as_deref())
    {
        Some(m) if !m.trim().is_empty() => m,
        _ => return false,
    };
    let req = crate::db::UpdateStoryRequest {
        title: None,
        description: None,
        genre: None,
        tone: None,
        pacing: None,
        style_dna_id: None,
        genre_profile_id: None,
        methodology_id: Some(recommended.to_string()),
        methodology_step: None,
        reference_book_id: None,
        strategy_json: None,
        ..Default::default()
    };
    match crate::db::StoryRepository::new(pool.clone()).update(&story.id, &req) {
        Ok(_) => {
            log::info!(
                "[smart_execute] 推荐方法论写回: story={} methodology={}",
                story.id,
                recommended
            );
            true
        }
        Err(e) => {
            log::warn!("[smart_execute] 推荐方法论写回失败: {}", e);
            false
        }
    }
}

/// v0.30.51: 根据计划执行消息构造用户可读的失败原因。
/// messages 混合了成功步骤（"Step X completed: ..."）与失败步骤
/// （"Step X failed: ..."）。此前无条件 join 全部消息，导致底层模型
/// 返回空内容时用户看到 "计划执行失败：Step sanitized_writer completed:
/// writer" 这类把成功步骤误报为失败原因的诊断。只保留失败步骤的消息。
fn build_plan_failure_message(messages: &[String], is_empty_content: bool) -> String {
    if messages
        .iter()
        .any(|m| m.contains("超时") || m.contains("timed out") || m.contains("timeout"))
    {
        return "模型响应超时，请检查模型服务是否正常运行".to_string();
    }
    let failure_msgs: Vec<&str> = messages
        .iter()
        .map(|s| s.as_str())
        .filter(|m| m.contains(" failed: ") || m.contains("失败"))
        .collect();
    if !failure_msgs.is_empty() {
        format!("计划执行失败：{}", failure_msgs.join("; "))
    } else if is_empty_content {
        "模型返回了空内容，未能生成正文，请重试或在设置中切换模型".to_string()
    } else if messages.is_empty() {
        "计划执行失败：未生成任何内容".to_string()
    } else {
        format!("计划执行失败：{}", messages.join("; "))
    }
}

// ===== 模型驱动的智能编排命令 =====

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;
    use crate::{
        db::{create_test_pool, GenreProfileRepository, Story},
        intent::InputClarity,
    };

    fn story_with_genre(genre: &str) -> Story {
        Story {
            id: "story-1".to_string(),
            title: "测试故事".to_string(),
            description: None,
            genre: Some(genre.to_string()),
            tone: None,
            pacing: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            methodology_step: None,
            reference_book_id: None,
            logline: None,
            strategy_json: None,
            story_format: "novel".to_string(),
            production_constraints: None,
            created_at: Local::now(),
            updated_at: Local::now(),
        }
    }

    /// 测试环境：create_test_pool() 中的 legacy inline migration 会被 SQL
    /// 文件迁移覆盖， 导致 genre_profiles
    /// 等表未创建。这里手动补齐测试所需表。
    fn ensure_genre_profiles_table(pool: &crate::db::DbPool) {
        let conn = pool.get().expect("get conn");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS genre_profiles (
                id TEXT PRIMARY KEY,
                genre_name TEXT NOT NULL UNIQUE,
                canonical_name TEXT NOT NULL,
                aliases_json TEXT,
                core_tone TEXT,
                pacing_strategy TEXT,
                anti_patterns_json TEXT,
                reference_tables_json TEXT,
                typical_structure_json TEXT,
                is_builtin INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                reader_promise TEXT,
                recommended_style_dna_ids TEXT,
                recommended_methodology_id TEXT,
                recommended_skill_ids TEXT,
                min_quality_tier TEXT DEFAULT 'medium'
            );
            CREATE INDEX IF NOT EXISTS idx_genre_profiles_canonical ON genre_profiles(canonical_name);"
        ).expect("create genre_profiles table");
    }

    /// Phase 1.4 审计测试：build_selected_strategy 通过 GenreResolver
    /// 解析复合题材 "异星球末世生存"，并保留 secondary genre IDs。
    #[test]
    fn test_build_selected_strategy_resolves_compound_genre() {
        let pool = create_test_pool().expect("test pool");
        ensure_genre_profiles_table(&pool);
        let repo = GenreProfileRepository::new(pool.clone());

        // 创建两个题材画像，并包含能触发复合匹配的关键词
        let apocalyptic = repo
            .create(
                "末世流",
                "Post-apocalyptic",
                Some("[\"post-apocalyptic\", \"apocalyptic\", \"末世\", \"末日\", \"废土\", \"末世生存\"]"),
                Some("文明崩溃后的世界"),
                Some("快节奏"),
                Some("[]"),
                None,
                None,
                true,
            )
            .expect("create apocalyptic");
        let alien = repo
            .create(
                "异星世界",
                "Alien World",
                Some("[\"alien world\", \"alien planet\", \"异星球\", \"异星\"]"),
                Some("陌生星球"),
                Some("中快节奏"),
                Some("[]"),
                None,
                None,
                true,
            )
            .expect("create alien-world");

        let apocalyptic_id = apocalyptic.id.clone();
        let alien_id = alien.id.clone();

        let story = story_with_genre("异星球末世生存");
        let strategy = build_selected_strategy(&Some(story), &pool, InputClarity::Vague)
            .expect("应通过 GenreResolver 匹配到题材画像");

        assert!(
            strategy.genre_profile_id.is_some(),
            "应自动设置主题材画像 ID"
        );
        let primary = strategy.genre_profile_id.as_deref().unwrap();
        assert!(
            primary == apocalyptic_id || primary == alien_id,
            "主题材应为已创建画像之一，实际为 {}",
            primary
        );

        let secondary = strategy
            .parameters
            .get("secondary_genre_profile_ids")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .expect("应保存次要题材画像 ID 列表");
        assert_eq!(secondary.len(), 1, "应解析出 1 个次要题材");
        let other = if primary == apocalyptic_id {
            &alien_id
        } else {
            &apocalyptic_id
        };
        assert_eq!(&secondary[0], other, "次要题材应为另一个画像");
    }

    #[test]
    fn test_build_selected_strategy_returns_none_for_unmatched_genre() {
        let pool = create_test_pool().expect("test pool");
        ensure_genre_profiles_table(&pool);
        let story = story_with_genre("完全不存在的题材 XYZ123");
        let strategy = build_selected_strategy(&Some(story), &pool, InputClarity::Vague);
        assert!(strategy.is_none(), "无法匹配任何题材画像时应返回 None");
    }

    // ===== v0.30.24: generate_logline_hint 守卫逻辑测试 =====

    #[test]
    fn test_should_skip_logline_generation_empty_input() {
        assert!(should_skip_logline_generation(""));
        assert!(should_skip_logline_generation("   "));
        assert!(should_skip_logline_generation("\n\t"));
    }

    #[test]
    fn test_should_skip_logline_generation_long_input() {
        // 100 字符 -> 跳过（详细 premise 无需增强）
        let long_input = "a".repeat(100);
        assert!(should_skip_logline_generation(&long_input));
        // 99 字符 -> 不跳过
        let short_input = "a".repeat(99);
        assert!(!should_skip_logline_generation(&short_input));
    }

    #[test]
    fn test_should_skip_logline_generation_normal_input() {
        // 正常创世指令 -> 不跳过
        assert!(!should_skip_logline_generation("写一部现代间谍的长篇小说"));
        assert!(!should_skip_logline_generation("  写一部科幻小说  "));
    }

    #[test]
    fn test_is_valid_logline() {
        // 空 -> 无效
        assert!(!is_valid_logline(""));
        assert!(!is_valid_logline("   "));
        // < 10 字符 -> 无效
        assert!(!is_valid_logline("短句"));
        assert!(!is_valid_logline("abcdefghi")); // 9 字符
                                                 // ≥ 10 字符 -> 有效
        assert!(is_valid_logline(
            "当一个退役特工发现妻子是间谍后必须阻止她引爆情报网络"
        ));
        assert!(is_valid_logline("abcdefghij")); // 10 字符
    }

    #[test]
    fn test_build_plan_failure_message_filters_success_steps() {
        // v0.30.51 回归：模型返回空内容时，messages 里只有成功步骤，
        // 不能把 "Step sanitized_writer completed: writer" 当作失败原因展示。
        let messages = vec!["Step sanitized_writer completed: writer".to_string()];
        let msg = build_plan_failure_message(&messages, true);
        assert!(
            !msg.contains("completed"),
            "成功步骤不应出现在失败诊断中: {}",
            msg
        );
        assert!(msg.contains("空内容"), "应提示空内容: {}", msg);
    }

    #[test]
    fn test_build_plan_failure_message_prefers_failure_steps() {
        let messages = vec![
            "Step outline completed: outline_planner".to_string(),
            "Step sanitized_writer failed: 模型 deepseek-v4 返回了空内容".to_string(),
        ];
        let msg = build_plan_failure_message(&messages, true);
        assert!(msg.contains("sanitized_writer failed"), "{}", msg);
        assert!(!msg.contains("outline completed"), "{}", msg);
    }

    #[test]
    fn test_build_plan_failure_message_timeout() {
        let messages = vec!["Step writer failed: 步骤 写作助手 超时".to_string()];
        let msg = build_plan_failure_message(&messages, false);
        assert!(msg.contains("超时"), "{}", msg);
    }

    #[test]
    fn test_build_plan_failure_message_empty_messages() {
        let msg = build_plan_failure_message(&[], false);
        assert!(msg.contains("未生成任何内容"), "{}", msg);
    }

    fn create_story_with_methodology(pool: &crate::db::DbPool, mid: Option<&str>) -> Story {
        StoryRepository::new(pool.clone())
            .create(crate::db::CreateStoryRequest {
                title: "写回测试故事".to_string(),
                description: None,
                genre: Some("玄幻".to_string()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: mid.map(|s| s.to_string()),
                reference_book_id: None,
            })
            .expect("create story")
    }

    #[test]
    fn test_persist_recommended_methodology_writes_back_when_story_has_none() {
        let pool = create_test_pool().expect("test pool");
        let story = create_story_with_methodology(&pool, None);
        let mut strategy = crate::domain::strategy::SelectedStrategy::default();
        strategy.methodology_id = Some("snowflake".to_string());

        let written = persist_recommended_methodology(&pool, &Some(story.clone()), &Some(strategy));
        assert!(written, "story 无显式方法论且推荐存在时应写回");

        let reloaded = StoryRepository::new(pool.clone())
            .get_by_id(&story.id)
            .expect("get")
            .expect("story exists");
        assert_eq!(reloaded.methodology_id.as_deref(), Some("snowflake"));
    }

    #[test]
    fn test_persist_recommended_methodology_never_overrides_explicit() {
        let pool = create_test_pool().expect("test pool");
        let story = create_story_with_methodology(&pool, Some("hero_journey"));
        let mut strategy = crate::domain::strategy::SelectedStrategy::default();
        strategy.methodology_id = Some("snowflake".to_string());

        let written = persist_recommended_methodology(&pool, &Some(story.clone()), &Some(strategy));
        assert!(!written, "story 有显式方法论时不得覆盖");

        let reloaded = StoryRepository::new(pool.clone())
            .get_by_id(&story.id)
            .expect("get")
            .expect("story exists");
        assert_eq!(reloaded.methodology_id.as_deref(), Some("hero_journey"));
    }

    // ===== v0.31: strategy_json 持久化 =====

    #[test]
    fn test_strategy_json_round_trip_via_repository() {
        let pool = create_test_pool().expect("test pool");
        let repo = crate::db::StoryRepository::new(pool.clone());
        let story = repo
            .create(crate::db::CreateStoryRequest {
                title: "回环测试".to_string(),
                description: None,
                genre: Some("末世".to_string()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .expect("create story");
        // 新故事 strategy_json 为 NULL（与旧数据一致）
        assert!(story.strategy_json.is_none());

        let json = r#"{"beat_card_ids":["beat_mentor_fallback"],"story_engine_ids":["engine_underdog"],"pressure_relationship_id":"rel_debt","emotional_payoff":"爽","conflict_arena":"公开审查"}"#;
        repo.update_strategy_json(&story.id, json)
            .expect("update strategy_json");
        let loaded = repo
            .get_by_id(&story.id)
            .expect("get_by_id")
            .expect("story exists");
        assert_eq!(loaded.strategy_json.as_deref(), Some(json));
        // 部分 JSON（只含四元组字段）可反序列化为 SelectedStrategy
        let parsed: crate::domain::strategy::SelectedStrategy =
            serde_json::from_str(loaded.strategy_json.as_deref().unwrap())
                .expect("deserialize partial strategy");
        assert_eq!(
            parsed.beat_card_ids,
            vec!["beat_mentor_fallback".to_string()]
        );
        assert_eq!(parsed.story_engine_ids, vec!["engine_underdog".to_string()]);
        assert_eq!(parsed.pressure_relationship_id.as_deref(), Some("rel_debt"));
        assert_eq!(parsed.emotional_payoff.as_deref(), Some("爽"));
        assert_eq!(parsed.conflict_arena.as_deref(), Some("公开审查"));
    }

    #[test]
    fn test_build_selected_strategy_null_strategy_json_unchanged() {
        // NULL（旧数据）回退：行为与现状一致——GenreResolver 自动匹配 +
        // infer_narrative_quartet 启发式补齐四元组。
        let pool = create_test_pool().expect("test pool");
        ensure_genre_profiles_table(&pool);
        let repo = GenreProfileRepository::new(pool.clone());
        repo.create(
            "末世流",
            "Post-apocalyptic",
            Some("[\"末世\", \"末世生存\"]"),
            Some("文明崩溃后的世界"),
            Some("快节奏"),
            Some("[]"),
            None,
            None,
            true,
        )
        .expect("create profile");
        let story = story_with_genre("末世"); // strategy_json = None
        let strategy = build_selected_strategy(&Some(story), &pool, InputClarity::Vague)
            .expect("应匹配到题材画像");
        assert!(strategy.genre_profile_id.is_some());
        // 启发式四元组仍生效（infer_narrative_quartet 从 reader_promise
        // 等补齐）
        assert!(strategy.rationale.contains("体裁画像"));
    }

    #[test]
    fn test_build_selected_strategy_prefers_persisted_quartet() {
        // 持久化四元组优先于启发式：beat_card_ids/emotional_payoff 来自
        // strategy_json，未被 infer_narrative_quartet 覆盖。
        let pool = create_test_pool().expect("test pool");
        ensure_genre_profiles_table(&pool);
        let repo = GenreProfileRepository::new(pool.clone());
        repo.create(
            "末世流",
            "Post-apocalyptic",
            Some("[\"末世\", \"末世生存\"]"),
            Some("文明崩溃后的世界"),
            Some("快节奏"),
            Some("[]"),
            None,
            None,
            true,
        )
        .expect("create profile");
        let mut story = story_with_genre("末世");
        story.strategy_json =
            Some(r#"{"beat_card_ids":["beat_wizard_pick"],"emotional_payoff":"燃"}"#.to_string());
        let strategy = build_selected_strategy(&Some(story), &pool, InputClarity::Vague)
            .expect("应匹配到题材画像");
        assert_eq!(strategy.beat_card_ids, vec!["beat_wizard_pick".to_string()]);
        assert_eq!(strategy.emotional_payoff.as_deref(), Some("燃"));
        assert!(strategy.rationale.contains("向导持久化"));
    }
}
