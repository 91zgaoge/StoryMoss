//! Orchestrator commands

use tauri::{AppHandle, Manager, State};

use crate::{
    db::{Chapter, ChapterRepository, DbPool, Story, StoryRepository},
    error::AppError,
    is_novel_creation_intent, record_ai_operation,
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
/// v0.14.0: 外层包裹 180 秒整体超时，确保任何环节卡死都能快速失败。
/// 超时时主动取消所有进行中的 LLM 生成，避免孤儿任务继续占用模型资源。
#[tauri::command(rename_all = "snake_case")]
pub async fn smart_execute(
    user_input: String,
    current_content: Option<String>,
    style_weight: Option<i32>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::planner::PlanExecutionResult, AppError> {
    // v0.15.5: 从 AppConfig 读取硬超时，默认 180s
    // v0.18.1 修复：使用 app_data_dir() 而非 current_dir()，确保读取到用户实际配置
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
    let smart_execute_timeout = crate::config::AppConfig::load(&app_dir)
        .map(|c| c.smart_execute_total_timeout_secs)
        .unwrap_or(180u64);
    let pool_inner = pool.inner().clone();

    match tokio::time::timeout(
        std::time::Duration::from_secs(smart_execute_timeout),
        smart_execute_inner(
            user_input,
            current_content,
            style_weight,
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
    let pool_for_loader = pool.clone();
    log::info!("[smart_execute] STEP 1/5 loading stories+chapters (spawn_blocking)...");
    let t1 = std::time::Instant::now();
    let (stories, current_story, current_story_id, chapters) =
        tokio::task::spawn_blocking(move || -> Result<SmartExecuteContext, AppError> {
            let stories = StoryRepository::new(pool_for_loader.clone())
                .get_all()
                .map_err(|e| {
                    AppError::internal(format!("[smart_execute] Failed to load stories: {}", e))
                })?;
            let current_story = stories.first().cloned();
            let current_story_id = current_story.as_ref().map(|s| s.id.clone());
            let chapters = if let Some(ref story_id) = current_story_id {
                ChapterRepository::new(pool_for_loader.clone())
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
        })
        .await
        .map_err(|e| AppError::internal(format!("[smart_execute] 上下文加载任务失败: {}", e)))??;
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

    // 优先使用前端传来的实时编辑器内容，其次回退到数据库中最后一章的内容
    let current_content_preview = current_content
        .filter(|c| !c.trim().is_empty())
        .or_else(|| chapters.last().and_then(|c| c.content.clone()))
        .map(|content| {
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

    // 检测是否需要启动小说初始化工作流
    let is_bootstrap_intent = is_novel_creation_intent(&user_input);

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
        wf(
            "smart_execute.bootstrap.enter",
            "进入创世模式（GenesisPipeline 快速阶段）",
            Some(serde_json::json!({
                "total_timeout_secs": crate::config::AppConfig::load(&app_handle.path().app_data_dir().unwrap_or_default())
                    .map(|c| c.smart_execute_total_timeout_secs).unwrap_or(180u64),
            })),
        );
        let app_dir = app_handle.path().app_data_dir().unwrap_or_default();
        let total_timeout = crate::config::AppConfig::load(&app_dir)
            .map(|c| c.smart_execute_total_timeout_secs)
            .unwrap_or(180u64);
        log::warn!(
            "[smart_execute] 检测到小说创建意图，启动 GenesisPipeline 快速阶段，total_timeout={}s",
            total_timeout
        );
        let mut ctx =
            crate::narrative::genesis::GenesisContext::new(app_handle.clone(), user_input.clone());
        let session_id = ctx.session_id.clone();
        let llm = crate::llm::LlmService::new(app_handle.clone());
        let cancel_flag = crate::narrative::pipeline::register_pipeline_cancel(&session_id);

        // v0.23.14: 快速阶段 = 故事概念 + 第一章正文（TriShot 模式，目标 30-60 秒）
        // 正文生成后立即返回给用户，策略选择与世界观/大纲/角色等推入后台。
        let quick_steps = crate::narrative::genesis::GenesisPipeline::quick_phase_steps();
        let quick_step_count = quick_steps.len();
        let executor = crate::narrative::pipeline::NarrativePipelineExecutor::new(quick_steps)
            .with_cancel_flag(cancel_flag.clone());

        // 进度回调：同时发射新旧两种事件（向后兼容）
        let app_handle_progress = app_handle.clone();
        let progress_callback = std::sync::Arc::new(
            move |evt: crate::narrative::progress::PipelineProgressEvent| {
                let _ = app_handle_progress.emit("pipeline-progress", &evt);
                let _ = app_handle_progress.emit(
                    "novel-bootstrap-progress",
                    crate::planner::bootstrap::BootstrapProgressEvent {
                        session_id: evt.pipeline_id.clone(),
                        step_name: evt.step_name.clone(),
                        step_number: evt.step_number,
                        total_steps: evt.total_steps,
                        message: evt.message.clone(),
                        status: format!("{:?}", evt.status).to_lowercase(),
                    },
                );
            },
        );

        log::warn!(
            "[smart_execute] GenesisPipeline 快速阶段开始执行，共 {} 步",
            quick_step_count
        );
        match executor
            .execute(&mut ctx, &llm, progress_callback.clone())
            .await
        {
            Ok(()) => {
                log::warn!(
                    "[smart_execute] GenesisPipeline 快速阶段成功完成 story_id={}",
                    ctx.story_id
                );
                // 从上下文读取第一章正文内容（已在 FirstChapterGenerationStep
                // 中保存到数据库并发射 ChapterSwitch 事件）
                let final_content = {
                    let bundle = ctx.bundle.read().await;
                    bundle
                        .story_meta
                        .as_ref()
                        .and_then(|meta| {
                            if meta.title.is_empty() {
                                None
                            } else {
                                Some(format!("故事《{}》已创建，第一章正文已生成。", meta.title))
                            }
                        })
                        .or_else(|| ctx.first_chapter_content.clone())
                };

                // [DEBUG] Bug A 关键日志：final_content 是摘要还是完整正文？
                wf(
                    "genesis.final_content",
                    "Genesis 快速阶段完成，确定 final_content",
                    Some(serde_json::json!({
                        "story_id": &ctx.story_id,
                        "story_meta_title": ctx.bundle.read().await.story_meta.as_ref().map(|m| m.title.clone()).unwrap_or_default(),
                        "first_chapter_content_len": ctx.first_chapter_content.as_ref().map(|s| s.len()).unwrap_or(0),
                        "final_content_len": final_content.as_ref().map(|s| s.len()).unwrap_or(0),
                        "final_content_is_summary": final_content.as_ref().map(|s| s.contains("已创建，第一章正文已生成")).unwrap_or(false),
                        "final_content_preview": final_content.as_ref().map(|s| &s[..s.len().min(100)]).unwrap_or("EMPTY"),
                        "messages": ["story_created", "novel_bootstrap_first_chapter_ready"],
                    })),
                );

                let story_id = ctx.story_id.clone();
                let session_id = ctx.session_id.clone();
                let bundle = ctx.bundle.read().await.clone();
                let selected_strategy = ctx.selected_strategy.clone();

                // 发射 story_created 同步事件，让前端立即进入工作台
                let _ = crate::state_sync::StateSync::emit_story_created(
                    &app_handle,
                    &story_id,
                    "新故事",
                );

                // v0.23.56: record_ai_operation 已移至 emit_progress("completed") 之后
                // 的 spawn_blocking 中，此处不再同步调用。

                // v0.23.14: 后台阶段：策略选择 + 世界观/大纲/角色/场景/伏笔/知识图谱
                let app_handle_bg = app_handle.clone();
                let story_id_bg = story_id.clone();
                let session_id_bg = session_id.clone();
                let user_input_bg = user_input.clone();
                tauri::async_runtime::spawn(async move {
                    let story_id_for_emit = story_id_bg.clone();
                    let app_handle_for_emit = app_handle_bg.clone();
                    let mut bg_ctx = crate::narrative::genesis::GenesisContext::for_background(
                        app_handle_bg.clone(),
                        story_id_bg,
                        session_id_bg.clone(),
                        user_input_bg,
                        bundle,
                        selected_strategy,
                    );
                    let llm_bg = crate::llm::LlmService::new(app_handle_bg.clone());

                    // v0.23.14: background_steps 包含 StrategySelectionStep +
                    // 世界观/大纲/角色/场景/伏笔/知识图谱
                    let bg_steps = crate::narrative::genesis::GenesisPipeline::background_steps();
                    let bg_cancel_flag =
                        crate::narrative::pipeline::register_pipeline_cancel(&session_id_bg);
                    let bg_executor =
                        crate::narrative::pipeline::NarrativePipelineExecutor::new(bg_steps)
                            .with_cancel_flag(bg_cancel_flag);

                    let progress_callback_bg = std::sync::Arc::new(
                        move |mut evt: crate::narrative::progress::PipelineProgressEvent| {
                            // 标记为后台阶段：前端监听器据此跳过注册 running activity，
                            // 不禁用输入框（后台作业只做状态提示，不阻塞用户写作）。
                            evt.metadata = Some(serde_json::json!({"background": true}));
                            let _ = app_handle_bg.emit("pipeline-progress", &evt);
                            let _ = app_handle_bg.emit(
                                "novel-bootstrap-progress",
                                crate::planner::bootstrap::BootstrapProgressEvent {
                                    session_id: evt.pipeline_id.clone(),
                                    step_name: evt.step_name.clone(),
                                    step_number: evt.step_number,
                                    total_steps: evt.total_steps,
                                    message: evt.message.clone(),
                                    status: format!("{:?}", evt.status).to_lowercase(),
                                },
                            );
                        },
                    );

                    let bg_start = std::time::Instant::now();
                    let bg_result = bg_executor
                        .execute(&mut bg_ctx, &llm_bg, progress_callback_bg)
                        .await;
                    let bg_elapsed = bg_start.elapsed().as_secs();

                    let (success, error_message) = match &bg_result {
                        Ok(_) => {
                            log::info!("[GenesisPipeline] 后台阶段完成，发射数据刷新事件");
                            crate::state_sync::StateSync::emit_data_refresh(
                                &app_handle_for_emit,
                                Some(&story_id_for_emit),
                                "all",
                            );
                            (true, None)
                        }
                        Err(e) => {
                            log::warn!("[GenesisPipeline] 后台阶段失败: {}", e);
                            (false, Some(format!("{}", e)))
                        }
                    };

                    let elements_created = if success {
                        let mut counts = crate::narrative::progress::ElementsCount::default();
                        let bundle = bg_ctx.bundle.read().await;
                        counts.world_rules = if bundle.world_building.is_some() {
                            1
                        } else {
                            0
                        };
                        counts.characters = bundle.characters.len();
                        counts.scenes = bundle.scenes.len();
                        counts.foreshadowings = bundle.foreshadowings.len();
                        counts.plot_points = if bundle.outline.is_some() { 1 } else { 0 };
                        counts
                    } else {
                        crate::narrative::progress::ElementsCount::default()
                    };
                    let _ = app_handle_for_emit.emit(
                        "pipeline-complete",
                        crate::narrative::progress::PipelineCompleteEvent {
                            pipeline_id: session_id_bg.clone(),
                            pipeline_type: crate::narrative::progress::PipelineType::Genesis,
                            success,
                            total_elapsed_seconds: bg_elapsed,
                            elements_created,
                            error_message,
                        },
                    );
                });

                // v0.23.37: Genesis 快速阶段成功后补发 completed，清除前端主活动。
                // 此前 Genesis 路径在 :115 发了 loading_context（映射为"准备上下文"）后
                // 直接 return，跳过了常规路径 :819 的 completed，导致主活动永远 running、
                // 底部状态栏卡在"准备上下文"。
                // v0.23.56: emit_progress("completed") 必须在 record_ai_operation 之前，
                // 确保前端立即收到完成事件并释放等待，不因 DB 写入阻塞 invoke resolve。
                emit_progress("completed", "小说创世完成", 5, 5);

                // v0.23.56: record_ai_operation 是同步 DB 写入，移入 spawn_blocking
                // 避免 tokio worker 线程被 DB 操作阻塞导致 smart_execute invoke 延迟 resolve。
                let pool_for_record = pool.clone();
                let input_for_record = user_input.clone();
                let sid_for_record = story_id.clone();
                let sid_session = session_id.clone();
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
                                serde_json::json!({"session_id": sid_session, "mode": "quick_phase"})
                                    .to_string(),
                            ),
                        },
                    );
                });

                return Ok(crate::planner::PlanExecutionResult {
                    success: true,
                    steps_completed: 1,
                    // v0.23.14: 返回第一章正文（已在快速阶段生成），让用户立即可见
                    final_content,
                    messages: vec![
                        format!("story_created:{}", story_id),
                        format!("session_id:{}", session_id),
                        // v0.23.15: 语义变更——第一章已就绪，后台仅生成世界观/大纲等。
                        // 前端据此走"正文已就绪"路径，而非"后台生成中"路径。
                        "novel_bootstrap_first_chapter_ready".to_string(),
                    ],
                    error: None,
                });
            }
            Err(e) => {
                log::error!(
                    "[smart_execute] GenesisPipeline concept generation failed: {}",
                    e
                );
                // v0.23.37: 同样补发 error，避免主活动卡在 running。
                emit_progress("error", &format!("小说初始化失败: {}", e), 5, 5);
                // 将 PipelineError 转换为 AppError，保留 LLM 超时语义
                let app_err = match e {
                    crate::narrative::pipeline::PipelineError::LlmError(ref msg)
                        if msg.to_lowercase().contains("timeout")
                            || msg.to_lowercase().contains("timed out") =>
                    {
                        AppError::llm_timeout(300_000)
                    }
                    crate::narrative::pipeline::PipelineError::Cancelled(msg) => {
                        AppError::cancelled(msg)
                    }
                    _ => AppError::internal(format!("小说初始化失败: {}", e)),
                };
                return Err(app_err);
            }
        }
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
    let prev_content_for_record = current_content_preview.clone();

    // v0.10.0: 构建当前故事的创作策略上下文
    // v0.14.0: spawn_blocking 包裹同步 DB 查询
    // v0.17.1: 输入清晰度检测 → 后端透明补全中文叙事四元组
    let strategy_story = current_story.clone();
    let strategy_pool = pool.clone();
    let input_clarity = crate::intent::detect_input_clarity(&user_input);
    let selected_strategy = tokio::task::spawn_blocking(move || {
        build_selected_strategy(&strategy_story, &strategy_pool, input_clarity)
    })
    .await
    .unwrap_or(None);

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
        selected_text: None,
        style_weight,
        chapter_number,
        selected_strategy,
    };

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
        let error_msg = if result
            .messages
            .iter()
            .any(|m| m.contains("超时") || m.contains("timed out") || m.contains("timeout"))
        {
            "模型响应超时，请检查模型服务是否正常运行".to_string()
        } else if result.messages.is_empty() {
            "计划执行失败：未生成任何内容".to_string()
        } else {
            format!("计划执行失败：{}", result.messages.join("; "))
        };
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
        .or_else(|| chapters.last().and_then(|c| c.content.clone()));

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
        // 使用 GenreResolver 解析 story.genre，支持精确/别名/子串/同义词/复合题材
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
}
