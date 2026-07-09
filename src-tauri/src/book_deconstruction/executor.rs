//! Book Deconstruction Task Executor
//!
//! 将拆书分析实现为 TaskExecutor trait，接入任务系统。

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use super::{chunker::create_chunks, models::*, parser::parse_book, repository::*};
use crate::{
    db::{
        repositories_narrative::{
            NarrativeCharacterRepository, NarrativeSceneRepository,
            NarrativeWorldBuildingRepository,
        },
        DbPool,
    },
    llm::LlmService,
    narrative::elements::ElementStatus,
    ports::VectorStore,
    task_system::{
        executor::{TaskExecutionContext, TaskExecutor},
        models::*,
    },
};

pub struct BookDeconstructionExecutor {
    pool: DbPool,
    llm_service: LlmService,
    app_handle: AppHandle,
    vector_store: Arc<dyn VectorStore>,
}

impl BookDeconstructionExecutor {
    pub fn new(
        pool: DbPool,
        llm_service: LlmService,
        app_handle: AppHandle,
        vector_store: Arc<dyn VectorStore>,
    ) -> Self {
        Self {
            pool,
            llm_service,
            app_handle,
            vector_store,
        }
    }
}

#[async_trait::async_trait]
impl TaskExecutor for BookDeconstructionExecutor {
    fn can_handle(&self, task_type: &TaskType) -> bool {
        *task_type == TaskType::BookDeconstruction
    }

    async fn execute(&self, task: &Task) -> Result<TaskResult, Box<dyn std::error::Error>> {
        log::info!("[BookDeconstructionExecutor] Task {} started", task.id);
        let ctx =
            TaskExecutionContext::new(task.id.clone(), self.pool.clone(), self.app_handle.clone());

        ctx.log("info", "开始拆书分析任务");

        // 解析 payload
        let payload: serde_json::Value = match task.payload.as_deref() {
            Some(p) => match serde_json::from_str(p) {
                Ok(v) => v,
                Err(e) => {
                    log::error!("[BookDeconstructionExecutor] Invalid payload: {}", e);
                    return Ok(TaskResult {
                        success: false,
                        result_json: None,
                        error_message: Some(format!("Invalid payload: {}", e)),
                    });
                }
            },
            None => serde_json::json!({}),
        };

        let book_id = payload
            .get("book_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing book_id in task payload")?;
        let file_path_str = payload
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or("Missing file_path in task payload")?;
        let file_path = std::path::Path::new(file_path_str);

        // 注册 Pipeline 取消标志，并桥接任务系统取消到 Pipeline 取消
        let cancel_flag = crate::narrative::pipeline::register_pipeline_cancel(book_id);
        let loop_book_id = book_id.to_string();
        let loop_task_id = task.id.clone();
        let loop_pool = self.pool.clone();
        let loop_app_handle = self.app_handle.clone();
        let cancel_monitor = tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let check_ctx = TaskExecutionContext::new(
                    loop_task_id.clone(),
                    loop_pool.clone(),
                    loop_app_handle.clone(),
                );
                if check_ctx.is_cancelled() {
                    log::info!(
                        "[BookDeconstructionExecutor] Task cancelled, cancelling pipeline {}",
                        loop_book_id
                    );
                    crate::narrative::pipeline::cancel_pipeline(&loop_book_id);
                    break;
                }
            }
        });

        ctx.update_progress("parsing", 0, "正在解析文件...");
        ctx.heartbeat();

        // 解析文件（同步操作，用 spawn_blocking 避免阻塞异步运行时）
        let file_path_owned = file_path.to_path_buf();

        let parsed =
            match tokio::task::spawn_blocking(move || parse_book(&file_path_owned, None)).await {
                Ok(Ok(p)) => p,
                Ok(Err(e)) => {
                    ctx.log("error", &format!("文件解析失败: {}", e));
                    return Ok(TaskResult {
                        success: false,
                        result_json: None,
                        error_message: Some(format!("文件解析失败: {}", e)),
                    });
                }
                Err(e) => {
                    ctx.log("error", &format!("解析任务异常: {}", e));
                    return Ok(TaskResult {
                        success: false,
                        result_json: None,
                        error_message: Some(format!("解析任务异常: {}", e)),
                    });
                }
            };

        ctx.update_progress("chunking", 5, "正在分块处理...");
        ctx.heartbeat();

        let chunks = create_chunks(&parsed);
        let word_count = parsed.word_count;

        // 更新 book 记录中的状态为分析中
        {
            let repo = ReferenceBookRepository::new(self.pool.clone());
            let _ = repo.update_status(book_id, AnalysisStatus::Analyzing, 5);
        }

        ctx.update_progress("analyzing", 10, "开始LLM分析...");
        ctx.heartbeat();

        // 转换 TextChunk 类型
        let narrative_chunks: Vec<crate::narrative::analysis::TextChunk> = chunks
            .iter()
            .map(|c| crate::narrative::analysis::TextChunk {
                index: c.index,
                title: c.title.clone(),
                content: c.content.clone(),
                word_count: c.word_count,
            })
            .collect();

        let concurrency = {
            let app_dir = self.app_handle.path().app_data_dir().unwrap_or_default();
            crate::config::AppConfig::load(&app_dir)
                .map(|c| c.book_deconstruction_concurrency)
                .unwrap_or(3)
                .clamp(1, 100)
        };

        let mut analysis_ctx = crate::narrative::analysis::AnalysisContext::with_concurrency(
            book_id.to_string(),
            book_id.to_string(), // story_id 暂时用 book_id
            narrative_chunks,
            word_count,
            self.pool.clone(),
            concurrency,
        );

        let llm = self.llm_service.clone();
        let steps = crate::narrative::analysis::AnalysisPipeline::steps();
        let pipeline_executor = crate::narrative::pipeline::NarrativePipelineExecutor::new(steps)
            .with_cancel_flag(cancel_flag);

        // 进度回调：同时发射新旧两种事件（向后兼容）
        let app_handle_progress = self.app_handle.clone();
        let book_id_for_progress = book_id.to_string();
        let heartbeat_ctx =
            TaskExecutionContext::new(task.id.clone(), self.pool.clone(), self.app_handle.clone());
        let progress_callback = Arc::new(
            move |evt: crate::narrative::progress::PipelineProgressEvent| {
                // 发射新事件
                let _ = app_handle_progress.emit("pipeline-progress", &evt);
                // 发射旧事件（向后兼容）
                let _ = app_handle_progress.emit(
                    "book-analysis-progress",
                    BookAnalysisProgressEvent {
                        book_id: book_id_for_progress.clone(),
                        status: match evt.status {
                            crate::narrative::progress::StepStatus::Running => {
                                "analyzing".to_string()
                            }
                            crate::narrative::progress::StepStatus::Completed => {
                                "completed".to_string()
                            }
                            crate::narrative::progress::StepStatus::Failed => "failed".to_string(),
                            _ => "analyzing".to_string(),
                        },
                        progress: evt.progress_percent,
                        current_step: evt.step_name.clone(),
                        message: Some(evt.message.clone()),
                        active_threads: 0,
                        total_chunks: 0,
                        processed_chunks: 0,
                    },
                );
                // 心跳保活
                heartbeat_ctx.heartbeat();
            },
        );

        let pipeline_result = pipeline_executor
            .execute(&mut analysis_ctx, &llm, progress_callback)
            .await;

        // 清理 Pipeline 取消监控
        cancel_monitor.abort();
        let _ = cancel_monitor.await;
        crate::narrative::pipeline::unregister_pipeline_cancel(book_id);

        // ========== LitSeg 后处理阶段 ==========
        // 在 7 步 Pipeline 完成后，对提取的叙事元素进行结构分析
        let analyzed_structure_json = if pipeline_result.is_ok() {
            log::info!(
                "[BookDeconstructionExecutor] Pipeline completed for book {}, running LitSeg post-processing",
                book_id
            );

            // 1. 为每个 SceneElement 计算 narrative_intensity / sentiment / event_types
            for scene in &mut analysis_ctx.bundle.scenes {
                if !scene.conflict_type.is_empty() {
                    scene.narrative_intensity =
                        crate::narrative::intensity_mapper::conflict_type_to_intensity(
                            &scene.conflict_type,
                        );
                }
                // 注意：SceneElement 目前没有 emotional_tone 字段
                // 需要从 LLM 响应中提取，或通过其他方式推断
                // 暂时用 conflict_type 的 intensity 作为 sentiment 的绝对值符号
                scene.narrative_sentiment = scene.narrative_intensity * 0.2 - 0.1; // 轻微负面偏置

                // 从 dramatic_goal + external_pressure 推断 event_types
                let mut event_types = Vec::new();
                if !scene.conflict_type.is_empty() {
                    event_types.push(crate::narrative::intensity_mapper::classify_event_type(
                        &scene.conflict_type,
                    ));
                }
                if !scene.dramatic_goal.is_empty() {
                    event_types.push("development".to_string());
                }
                scene.narrative_event_types = event_types;
            }

            // 2. 构建 NarrativeEvent 列表，运行 NarrativeStructureAnalyzer
            let narrative_events: Vec<crate::narrative::event::NarrativeEvent> = analysis_ctx
                .bundle
                .scenes
                .iter()
                .map(|s| crate::narrative::event::NarrativeEvent {
                    id: s.id.clone(),
                    story_id: s.story_id.clone(),
                    chapter_number: s.sequence_number,
                    scene_id: Some(s.id.clone()),
                    event_type: crate::narrative::event::EventType::ConflictEruption,
                    intensity: s.narrative_intensity,
                    sentiment: s.narrative_sentiment,
                    description: s.summary.clone(),
                    involved_character_ids: s.characters_present.clone(),
                    conflict_types: vec![],
                    preceding_event_id: None,
                    following_event_id: None,
                    act_number: 1,
                    position_in_act: 1,
                    created_at: chrono::Local::now(),
                })
                .collect();

            let analyzer = crate::narrative::structure_analyzer::NarrativeStructureAnalyzer::new();
            let structure = analyzer.analyze(book_id, &narrative_events);

            // 3. 为每个 SceneElement 标注 act_number 和 position_in_act
            for scene in &mut analysis_ctx.bundle.scenes {
                if let Some(act) = structure.acts.iter().find(|a| {
                    scene.sequence_number >= a.start_chapter
                        && scene.sequence_number <= a.end_chapter
                }) {
                    scene.act_number = act.act_number;
                    let act_len = (act.end_chapter - act.start_chapter + 1).max(1) as f32;
                    let offset = (scene.sequence_number - act.start_chapter) as f32;
                    scene.position_in_act = offset / act_len;
                }
            }

            log::info!(
                "[BookDeconstructionExecutor] LitSeg analysis: {} acts detected for book {}",
                structure.acts.len(),
                book_id
            );

            // 4. 序列化幕结构为 JSON
            serde_json::to_string(&structure.acts).ok()
        } else {
            None
        };

        if pipeline_result.is_ok() {
            let _ = self.app_handle.emit(
                "pipeline-complete",
                crate::narrative::progress::PipelineCompleteEvent {
                    pipeline_id: task.id.clone(),
                    pipeline_type: crate::narrative::progress::PipelineType::Analysis,
                    success: true,
                    total_elapsed_seconds: 0,
                    elements_created: crate::narrative::progress::ElementsCount::default(),
                    error_message: None,
                },
            );
        }

        let analysis_result = match pipeline_result {
            Ok(()) => convert_bundle_to_analysis_result(&analysis_ctx.bundle),
            Err(crate::narrative::pipeline::PipelineError::Cancelled(msg)) => {
                log::warn!(
                    "[BookDeconstructionExecutor] Pipeline cancelled for task {}",
                    task.id
                );
                ctx.log("warn", &format!("分析被取消: {}", msg));
                let repo = ReferenceBookRepository::new(self.pool.clone());
                let _ = repo.update_status(book_id, AnalysisStatus::Cancelled, ctx.get_progress());
                return Ok(TaskResult {
                    success: false,
                    result_json: None,
                    error_message: Some(msg),
                });
            }
            Err(e) => {
                ctx.log("error", &format!("分析失败: {}", e));
                let repo = ReferenceBookRepository::new(self.pool.clone());
                let _ = repo.update_error(book_id, &e.to_string());
                return Ok(TaskResult {
                    success: false,
                    result_json: None,
                    error_message: Some(format!("分析失败: {}", e)),
                });
            }
        };

        ctx.update_progress("saving", 93, "正在保存分析结果...");
        ctx.heartbeat();

        // 保存分析结果到 narrative_* 表（W3-B3 存储同构化）
        {
            let repo = ReferenceBookRepository::new(self.pool.clone());
            let _ = repo.update_analysis_result_with_structure(
                book_id,
                Some(analysis_result.book.title.as_str()),
                analysis_result.book.author.as_deref(),
                analysis_result.book.genre.as_deref(),
                analysis_result.book.world_setting.as_deref(),
                analysis_result.book.plot_summary.as_deref(),
                analysis_result.book.story_arc.as_deref(),
                analyzed_structure_json.as_deref(),
            );
            let _ = repo.update_status(book_id, AnalysisStatus::Completed, 100);

            // 设置 narrative 元素状态为 Reference
            for character in &mut analysis_ctx.bundle.characters {
                character.status = ElementStatus::Reference;
            }
            for scene in &mut analysis_ctx.bundle.scenes {
                scene.status = ElementStatus::Reference;
            }
            if let Some(ref mut wb) = analysis_ctx.bundle.world_building {
                wb.status = ElementStatus::Reference;
            }

            ctx.update_progress(
                "saving",
                96,
                &format!(
                    "正在保存 {} 个人物...",
                    analysis_ctx.bundle.characters.len()
                ),
            );
            let char_repo = NarrativeCharacterRepository::new(self.pool.clone());
            let _ = char_repo.create_batch(&analysis_ctx.bundle.characters);

            ctx.update_progress(
                "saving",
                98,
                &format!("正在保存 {} 个场景...", analysis_ctx.bundle.scenes.len()),
            );
            let scene_repo = NarrativeSceneRepository::new(self.pool.clone());
            let _ = scene_repo.create_batch(&analysis_ctx.bundle.scenes);

            if let Some(ref wb) = analysis_ctx.bundle.world_building {
                ctx.update_progress("saving", 99, "正在保存世界观...");
                let wb_repo = NarrativeWorldBuildingRepository::new(self.pool.clone());
                let _ = wb_repo.create(wb);
            }

            // 伏笔写入 foreshadowing_tracker（story_id = book_id；转故事时再复制）
            let fw_count = persist_bundle_foreshadowings(
                &self.pool,
                book_id,
                &analysis_ctx.bundle.foreshadowings,
            );
            if fw_count > 0 {
                log::info!(
                    "[BookDeconstructionExecutor] persisted {} foreshadowings for book {}",
                    fw_count,
                    book_id
                );
            }
        }

        // 向量化存储
        ctx.update_progress("saving", 99, "正在生成向量嵌入...");
        {
            let service = super::service::BookDeconstructionService::new(
                self.pool.clone(),
                self.llm_service.clone(),
                self.app_handle.clone(),
                self.vector_store.clone(),
            );
            if let Err(e) = service.store_embeddings(book_id, &analysis_result).await {
                log::warn!(
                    "[BookDeconstructionExecutor] store_embeddings failed: {}",
                    e
                );
            }
        }

        ctx.update_progress("completed", 100, "分析完成");
        ctx.log("info", "拆书分析任务完成");

        // 构建结果 JSON
        let result_json = serde_json::json!({
            "book_id": book_id,
            "title": analysis_result.book.title,
            "author": analysis_result.book.author,
            "genre": analysis_result.book.genre,
            "word_count": word_count,
            "character_count": analysis_result.characters.len(),
            "scene_count": analysis_result.scenes.len(),
        });

        Ok(TaskResult {
            success: true,
            result_json: Some(result_json.to_string()),
            error_message: None,
        })
    }
}

// ==================== 结果转换器 ====================
/// 将 NarrativeBundle 转换为 BookAnalysisResult（兼容旧接口）
/// 将 bundle 伏笔写入 foreshadowing_tracker；单条失败 warn，不 fail pipeline。
/// 返回成功写入条数。
pub(crate) fn persist_bundle_foreshadowings(
    pool: &DbPool,
    book_id: &str,
    foreshadowings: &[crate::narrative::elements::ForeshadowingElement],
) -> usize {
    use crate::creative_engine::foreshadowing::ForeshadowingTracker;

    if foreshadowings.is_empty() {
        return 0;
    }
    let tracker = ForeshadowingTracker::new(pool.clone());
    let mut ok = 0usize;
    for fw in foreshadowings {
        let content = fw.content.trim();
        if content.is_empty() {
            continue;
        }
        match tracker.add_foreshadowing(
            book_id,
            content,
            fw.setup_scene_id.as_deref(),
            fw.importance,
        ) {
            Ok(_) => ok += 1,
            Err(e) => {
                log::warn!(
                    "[BookDeconstructionExecutor] foreshadowing persist failed for book {}: {}",
                    book_id,
                    e
                );
            }
        }
    }
    ok
}

/// 将参考书（book_id）上的伏笔复制到新故事；失败 fail-open。
pub(crate) fn copy_foreshadowings_to_story(
    pool: &DbPool,
    from_book_id: &str,
    to_story_id: &str,
) -> usize {
    use crate::creative_engine::foreshadowing::ForeshadowingTracker;

    let tracker = ForeshadowingTracker::new(pool.clone());
    let records = match tracker.get_all(from_book_id) {
        Ok(r) => r,
        Err(e) => {
            log::warn!(
                "[BookDeconstruction] list foreshadowings for {} failed: {}",
                from_book_id,
                e
            );
            return 0;
        }
    };
    let mut ok = 0usize;
    for r in records {
        match tracker.add_foreshadowing(
            to_story_id,
            &r.content,
            r.setup_scene_id.as_deref(),
            r.importance,
        ) {
            Ok(_) => ok += 1,
            Err(e) => {
                log::warn!(
                    "[BookDeconstruction] copy foreshadowing to story {} failed: {}",
                    to_story_id,
                    e
                );
            }
        }
    }
    ok
}

fn convert_bundle_to_analysis_result(
    bundle: &crate::narrative::elements::NarrativeBundle,
) -> BookAnalysisResult {
    use chrono::Local;

    let now = Local::now();

    // 构建 ReferenceBook
    let book = if let Some(ref meta) = bundle.story_meta {
        let world_setting = bundle
            .world_building
            .as_ref()
            .map(|w| serde_json::to_string(w).ok())
            .flatten();
        let story_arc = bundle
            .outline
            .as_ref()
            .map(|o| serde_json::to_string(&o.acts).ok())
            .flatten();
        let analyzed_structure_json = bundle
            .outline
            .as_ref()
            .map(|o| serde_json::to_string(&o.acts).ok())
            .flatten();
        ReferenceBook {
            id: meta.id.clone(),
            title: meta.title.clone(),
            author: meta
                .author
                .as_ref()
                .map(|a| a.trim().to_string())
                .filter(|a| !a.is_empty()),
            genre: Some(meta.genre.clone()),
            word_count: None,
            file_format: None,
            file_hash: None,
            file_path: None,
            world_setting,
            plot_summary: Some(meta.description.clone()),
            story_arc,
            analyzed_structure_json,
            analysis_status: AnalysisStatus::Completed,
            analysis_progress: 100,
            analysis_error: None,
            task_id: None,
            created_at: now,
            updated_at: now,
        }
    } else {
        ReferenceBook {
            id: "unknown".to_string(),
            title: "未命名".to_string(),
            author: None,
            genre: None,
            word_count: None,
            file_format: None,
            file_hash: None,
            file_path: None,
            world_setting: None,
            plot_summary: None,
            story_arc: None,
            analyzed_structure_json: None,
            analysis_status: AnalysisStatus::Completed,
            analysis_progress: 100,
            analysis_error: None,
            task_id: None,
            created_at: now,
            updated_at: now,
        }
    };

    // 转换角色
    let characters: Vec<ReferenceCharacter> = bundle
        .characters
        .iter()
        .map(|c| {
            let relationships_json = serde_json::to_string(&c.relationships).ok();
            ReferenceCharacter {
                id: c.id.clone(),
                book_id: c.story_id.clone(),
                name: c.name.clone(),
                role_type: Some(c.role_type.clone()),
                personality: Some(c.personality.clone()),
                appearance: Some(c.appearance.clone()),
                relationships: relationships_json,
                key_scenes: None,
                importance_score: Some(c.importance_score),
                created_at: now,
            }
        })
        .collect();

    // 转换场景（含 LitSeg 叙事字段）
    let scenes: Vec<ReferenceScene> = bundle
        .scenes
        .iter()
        .map(|s| {
            let chars_present_json = serde_json::to_string(&s.characters_present).ok();
            let event_types_json = serde_json::to_string(&s.narrative_event_types).ok();
            ReferenceScene {
                id: s.id.clone(),
                book_id: s.story_id.clone(),
                sequence_number: s.sequence_number,
                title: Some(s.title.clone()),
                summary: Some(s.summary.clone()),
                characters_present: chars_present_json,
                key_events: None,
                conflict_type: Some(s.conflict_type.clone()),
                emotional_tone: None,
                // LitSeg 叙事字段
                narrative_intensity: Some(s.narrative_intensity).filter(|&v| v > 0.0),
                narrative_sentiment: Some(s.narrative_sentiment).filter(|&v| v != 0.0),
                narrative_event_types: event_types_json,
                act_number: Some(s.act_number).filter(|&v| v > 0),
                position_in_act: Some(s.position_in_act).filter(|&v| v > 0.0),
                created_at: now,
            }
        })
        .collect();

    BookAnalysisResult {
        book,
        characters,
        scenes,
    }
}

#[cfg(test)]
mod convert_bundle_tests {
    use super::convert_bundle_to_analysis_result;
    use crate::domain::{
        ElementSource, NarrativeBundle, OutlineAct, OutlineElement, StoryMetaElement,
    };

    fn sample_meta(author: Option<&str>) -> StoryMetaElement {
        StoryMetaElement {
            id: "book-1".into(),
            title: "样例".into(),
            description: "简介".into(),
            genre: "科幻".into(),
            genre_profile_ids: vec![],
            tone: "暗黑".into(),
            pacing: "快".into(),
            themes: vec![],
            target_length: "长篇".into(),
            author: author.map(|s| s.to_string()),
            protagonist_name: None,
            protagonist_desire: None,
            protagonist_wound: None,
            core_conflict: None,
            world_one_liner: None,
            survival_stakes: None,
            source: ElementSource::Extracted,
            source_ref_id: Some("book-1".into()),
        }
    }

    #[test]
    fn convert_bundle_passes_author_and_story_arc() {
        let outline = OutlineElement {
            id: "ol1".into(),
            story_id: "book-1".into(),
            acts: vec![OutlineAct {
                act_number: 1,
                title: "主线".into(),
                summary: "崛起".into(),
                key_plot_points: vec!["觉醒".into()],
                estimated_scenes: 0,
            }],
            total_scenes_estimate: 0,
            source: ElementSource::Extracted,
            source_ref_id: Some("book-1".into()),
        };
        let bundle = NarrativeBundle::new()
            .with_story_meta(sample_meta(Some("李四")))
            .with_outline(outline);

        let result = convert_bundle_to_analysis_result(&bundle);
        assert_eq!(result.book.author.as_deref(), Some("李四"));
        let arc = result.book.story_arc.expect("story_arc from outline");
        assert!(
            arc.contains("崛起"),
            "story_arc should serialize acts: {arc}"
        );
    }

    #[test]
    fn convert_bundle_author_none_when_missing() {
        let bundle = NarrativeBundle::new().with_story_meta(sample_meta(None));
        let result = convert_bundle_to_analysis_result(&bundle);
        assert!(result.book.author.is_none());
        assert!(result.book.story_arc.is_none());
    }
}
