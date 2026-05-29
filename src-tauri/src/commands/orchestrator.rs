//! Orchestrator commands

use tauri::{AppHandle, State};

use crate::{
    db::{ChapterRepository, DbPool, StoryRepository},
    error::AppError,
    is_novel_creation_intent, record_ai_operation,
};

/// 预检命令 - 写作前检查阻塞性问题
#[tauri::command(rename_all = "snake_case")]
pub fn check_preflight(
    story_id: String,
    chapter_number: i32,
    pool: State<'_, DbPool>,
) -> Result<crate::story_system::preflight::PreflightResult, AppError> {
    let pool = pool.inner().clone();
    let checker = crate::story_system::preflight::PreflightChecker::new();
    Ok(checker.check(&pool, &story_id, chapter_number))
}

/// 智能执行命令 - 新一代意图理解与执行入口
#[tauri::command(rename_all = "snake_case")]
pub async fn smart_execute(
    user_input: String,
    current_content: Option<String>,
    style_weight: Option<i32>,
    pool: State<'_, DbPool>,
    app_handle: AppHandle,
) -> Result<crate::planner::PlanExecutionResult, AppError> {
    let style_weight = style_weight.unwrap_or(50);
    use tauri::Emitter;

    let pool = pool.inner().clone();

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

    emit_progress("loading_context", "正在加载故事上下文...", 1, 5);

    // 构建 PlanContext：从当前系统状态推断
    let stories = StoryRepository::new(pool.clone()).get_all().map_err(|e| {
        AppError::internal(format!("[smart_execute] Failed to load stories: {}", e))
    })?;
    let current_story = stories.first().cloned();
    let current_story_id = current_story.as_ref().map(|s| s.id.clone());

    let chapters = if let Some(ref story_id) = current_story_id {
        ChapterRepository::new(pool.clone())
            .get_by_story(story_id)
            .map_err(|e| {
                AppError::internal(format!("[smart_execute] Failed to load chapters: {}", e))
            })?
    } else {
        vec![]
    };

    let chapter_count = chapters.len();

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

    if is_bootstrap_intent {
        log::info!("[smart_execute] Detected novel creation intent, starting GenesisPipeline");
        let mut ctx =
            crate::narrative::genesis::GenesisContext::new(app_handle.clone(), user_input.clone());
        let session_id = ctx.session_id.clone();
        let llm = crate::llm::LlmService::new(app_handle.clone());
        let quick_steps = crate::narrative::genesis::GenesisPipeline::quick_phase_steps();
        let cancel_flag = crate::narrative::pipeline::register_pipeline_cancel(&session_id);
        let executor = crate::narrative::pipeline::NarrativePipelineExecutor::new(quick_steps)
            .with_cancel_flag(cancel_flag);

        // 进度回调：同时发射新旧两种事件（向后兼容）
        let app_handle_progress = app_handle.clone();
        let progress_callback = std::sync::Arc::new(
            move |evt: crate::narrative::progress::PipelineProgressEvent| {
                // 发射新事件
                let _ = app_handle_progress.emit("pipeline-progress", &evt);
                // 发射旧事件（向后兼容，前端仍在监听 novel-bootstrap-progress）
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

        match executor
            .execute(&mut ctx, &llm, progress_callback.clone())
            .await
        {
            Ok(()) => {
                let _ = app_handle.emit(
                    "pipeline-complete",
                    crate::narrative::progress::PipelineCompleteEvent {
                        pipeline_id: ctx.session_id.clone(),
                        pipeline_type: crate::narrative::progress::PipelineType::Genesis,
                        success: true,
                        total_elapsed_seconds: 0,
                        elements_created: crate::narrative::progress::ElementsCount::default(),
                        error_message: None,
                    },
                );
                let story_id = ctx.story_id.clone();
                let session_id = ctx.session_id.clone();
                let first_chapter = ctx.first_chapter_content.clone();
                let bundle = ctx.bundle.clone();

                // 发射 story_created 同步事件
                let _ = crate::state_sync::StateSync::emit_story_created(
                    &app_handle,
                    &story_id,
                    "新故事",
                );

                // Record AI operation (before user_input is moved)
                let user_input_for_record = user_input.clone();
                record_ai_operation(crate::db::CreateAiOperationRequest {
                    story_id: story_id.clone(),
                    scene_id: None,
                    chapter_id: None,
                    operation_type: "bootstrap".to_string(),
                    operation_name: "小说创世".to_string(),
                    input_summary: Some(user_input_for_record),
                    output_summary: first_chapter
                        .as_ref()
                        .map(|c| c.chars().take(200).collect()),
                    previous_content: None,
                    new_content: first_chapter.clone(),
                    metadata: Some(serde_json::json!({"session_id": session_id}).to_string()),
                });

                // 启动后台阶段
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
                    );
                    let llm_bg = crate::llm::LlmService::new(app_handle_bg.clone());
                    let bg_steps =
                        crate::narrative::genesis::GenesisPipeline::background_phase_steps();
                    let bg_cancel_flag =
                        crate::narrative::pipeline::register_pipeline_cancel(&session_id_bg);
                    let bg_executor =
                        crate::narrative::pipeline::NarrativePipelineExecutor::new(bg_steps)
                            .with_cancel_flag(bg_cancel_flag);

                    let progress_callback_bg = std::sync::Arc::new(
                        move |evt: crate::narrative::progress::PipelineProgressEvent| {
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

                    // P0-5 修复: 根据后台阶段实际结果设置 success/error，不再硬编码
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

                    // 统计实际生成的元素数量
                    let elements_created = if success {
                        let mut counts = crate::narrative::progress::ElementsCount::default();
                        // 从上下文统计实际生成的元素（P0-5 修复: 使用 ElementsCount 的正确字段名）
                        counts.world_rules = if bg_ctx.bundle.world_building.is_some() {
                            1
                        } else {
                            0
                        };
                        counts.characters = bg_ctx.bundle.characters.len();
                        counts.scenes = bg_ctx.bundle.scenes.len();
                        counts.foreshadowings = bg_ctx.bundle.foreshadowings.len();
                        // outline 映射到 plot_points
                        counts.plot_points = if bg_ctx.bundle.outline.is_some() {
                            1
                        } else {
                            0
                        };
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

                return Ok(crate::planner::PlanExecutionResult {
                    success: true,
                    steps_completed: 2,
                    final_content: first_chapter,
                    messages: vec![
                        format!("story_created:{}", story_id),
                        format!("session_id:{}", session_id),
                        "novel_bootstrap_completed".to_string(),
                    ],
                });
            }
            Err(e) => {
                log::error!("[smart_execute] GenesisPipeline failed: {}", e);
                return Err(AppError::internal(format!("小说初始化失败: {}", e)));
            }
        }
    }

    // Phase 3: 加载场景结构信息 + 增强上下文
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
    ) = if let Some(ref story_id) = current_story_id {
        let scene_repo = crate::db::repositories::SceneRepository::new(pool.clone());
        let scenes = scene_repo.get_by_story(story_id).map_err(|e| {
            AppError::internal(format!("[smart_execute] Failed to load scenes: {}", e))
        })?;
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

        // ===== 增强上下文加载 =====
        // 世界观摘要
        let wb_repo = crate::db::repositories::WorldBuildingRepository::new(pool.clone());
        let world_building_summary = wb_repo.get_by_story(story_id).ok().flatten().map(|wb| {
            let rules_summary = wb
                .rules
                .iter()
                .filter(|r| r.importance >= 7)
                .map(|r| format!("{}: {}", r.name, r.description.as_deref().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("; ");
            format!("概念：{}；核心规则：{}", wb.concept, rules_summary)
        });

        // 角色列表
        let char_repo = crate::db::repositories::CharacterRepository::new(pool.clone());
        let character_list = char_repo
            .get_by_story(story_id)
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
            crate::creative_engine::foreshadowing::ForeshadowingTracker::new(pool.clone());
        let foreshadowing_status = foreshadowing_tracker
            .get_unresolved(story_id)
            .ok()
            .map(|records| records.into_iter().take(5).map(|r| r.content).collect())
            .unwrap_or_default();

        // 风格DNA / 风格混合
        let style_dna_info = {
            use crate::{
                creative_engine::style::blend::StyleBlendConfig,
                db::repositories::StoryStyleConfigRepository,
            };

            // 优先检查混合配置
            let blend_info = if let Some(ref story) = current_story {
                let blend_repo = StoryStyleConfigRepository::new(pool.clone());
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
            } else {
                None
            };

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
        let mcp_tools_available = {
            let connections = crate::MCP_CONNECTIONS.lock().await;
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
        )
    };

    emit_progress("context_loaded", "故事上下文加载完成", 2, 5);

    // Clone values before they are moved into plan_context
    let story_id_for_record = current_story_id.clone();
    let scene_id_for_record = current_scene_id.clone();
    let chapter_id_for_record = chapters.last().map(|c| c.id.clone());
    let input_for_record = user_input.clone();
    let prev_content_for_record = current_content_preview.clone();

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
        selected_text: None,
        style_weight,
        chapter_number,
    };

    // 执行计划（内部会自动检查模板库并生成计划）
    emit_progress("executing", "开始执行创作计划...", 3, 5);
    let executor = crate::planner::PlanExecutor::new(app_handle);
    let result = executor
        .execute_with_context(&plan_context)
        .await
        .map_err(|e| AppError::internal(format!("[smart_execute] Plan execution failed: {}", e)))?;
    emit_progress("completed", "创作计划执行完成", 5, 5);

    // 如果计划执行失败（所有步骤都失败或没有内容/空内容），返回错误
    let is_empty_content = result
        .final_content
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if !result.success || is_empty_content {
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

    // Record AI operation for non-bootstrap generation
    if let Some(ref story_id) = story_id_for_record {
        record_ai_operation(crate::db::CreateAiOperationRequest {
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
        });
    }

    Ok(result)
}

/// 获取输入栏智能提示 — 由LLM根据当前故事上下文生成建议
#[tauri::command(rename_all = "snake_case")]
pub async fn get_input_hint(
    app_handle: AppHandle,
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

    // 尝试用 LLM 生成更个性化的建议
    let llm_hint = if let Some(ref story) = current_story {
        let llm_service = crate::llm::LlmService::new(app_handle);
        let prompt = format!(
            "你是一个AI写作助手。当前故事：{}，字数：{}，章节数：{}。\
             请生成一条简短的输入建议（12字以内），告诉用户下一步可以做什么。建议要自然、有创意、\
             贴合故事。只输出建议内容，不要解释。",
            story.title,
            word_count,
            chapters.len()
        );
        match llm_service.generate(prompt, Some(30), Some(0.7)).await {
            Ok(response) => {
                let hint = response
                    .content
                    .trim()
                    .replace(['"', '\'', '「', '」'], "")
                    .trim()
                    .to_string();
                if !hint.is_empty() && hint.chars().count() <= 20 {
                    Some(hint)
                } else {
                    None
                }
            }
            Err(e) => {
                log::debug!("[get_input_hint] LLM generation failed: {}", e);
                None
            }
        }
    } else {
        None
    };

    // 优先返回 LLM 建议，否则返回规则建议
    if let Some(hint) = llm_hint {
        Ok(hint)
    } else if let Some(hint) = candidates.first() {
        Ok(hint.clone())
    } else {
        Ok("输入指令开始创作".to_string())
    }
}

// ===== 模型驱动的智能编排命令 =====

// ===== 模型驱动的智能编排命令 =====
