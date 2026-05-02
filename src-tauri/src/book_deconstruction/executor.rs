//! Book Deconstruction Task Executor
//!
//! 将拆书分析实现为 TaskExecutor trait，接入任务系统。

use super::chunker::create_chunks;
use super::models::*;
use super::parser::parse_book;
use super::repository::*;
use crate::db::DbPool;
use crate::llm::LlmService;
use crate::task_system::executor::{TaskExecutionContext, TaskExecutor};
use crate::task_system::models::*;
use tauri::{AppHandle, Emitter, Manager};
use std::sync::Arc;

pub struct BookDeconstructionExecutor {
    pool: DbPool,
    llm_service: LlmService,
    app_handle: AppHandle,
}

impl BookDeconstructionExecutor {
    pub fn new(pool: DbPool, llm_service: LlmService, app_handle: AppHandle) -> Self {
        Self {
            pool,
            llm_service,
            app_handle,
        }
    }
}

#[async_trait::async_trait]
impl TaskExecutor for BookDeconstructionExecutor {
    fn can_handle(&self, task_type: &TaskType) -> bool {
        *task_type == TaskType::BookDeconstruction
    }

    async fn execute(
        &self,
        task: &Task,
    ) -> Result<TaskResult, Box<dyn std::error::Error>> {
        let ctx = TaskExecutionContext::new(
            task.id.clone(),
            self.pool.clone(),
            self.app_handle.clone(),
        );

        ctx.log("info", "开始拆书分析任务");

        // 解析 payload
        let payload: serde_json::Value = match task.payload.as_deref() {
            Some(p) => serde_json::from_str(p).unwrap_or(serde_json::json!({})),
            None => serde_json::json!({}),
        };

        let book_id = payload.get("book_id")
            .and_then(|v| v.as_str())
            .ok_or("Missing book_id in task payload")?;
        let file_path_str = payload.get("file_path")
            .and_then(|v| v.as_str())
            .ok_or("Missing file_path in task payload")?;
        let file_path = std::path::Path::new(file_path_str);

        ctx.update_progress("parsing", 0, "正在解析文件...");
        ctx.heartbeat();

        // 解析文件（同步操作，用 spawn_blocking 避免阻塞异步运行时）
        let file_path_owned = file_path.to_path_buf();
        
        let parsed = match tokio::task::spawn_blocking(move || {
            parse_book(&file_path_owned, None)
        }).await {
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

        // 读取并发数配置
        let _concurrency = {
            let app_dir = self.app_handle.path().app_data_dir().unwrap_or_default();
            crate::config::AppConfig::load(&app_dir)
                .map(|c| c.book_deconstruction_concurrency)
                .unwrap_or(3)
        };

        // v5.3.0: 使用新的 AnalysisPipeline 替代 BookAnalyzer
        // 转换 TextChunk 类型
        let narrative_chunks: Vec<crate::narrative::analysis::TextChunk> = chunks.iter().map(|c| {
            crate::narrative::analysis::TextChunk {
                index: c.index,
                title: c.title.clone(),
                content: c.content.clone(),
                word_count: c.word_count,
            }
        }).collect();

        let mut analysis_ctx = crate::narrative::analysis::AnalysisContext::new(
            book_id.to_string(),
            book_id.to_string(), // story_id 暂时用 book_id
            narrative_chunks,
            word_count,
            self.pool.clone(),
        );

        let llm = self.llm_service.clone();
        let steps = crate::narrative::analysis::AnalysisPipeline::steps();
        let pipeline_executor = crate::narrative::pipeline::NarrativePipelineExecutor::new(steps);

        // 进度回调：同时发射新旧两种事件（向后兼容）
        let app_handle_progress = self.app_handle.clone();
        let book_id_for_progress = book_id.to_string();
        let heartbeat_ctx = TaskExecutionContext::new(
            task.id.clone(),
            self.pool.clone(),
            self.app_handle.clone(),
        );
        let progress_callback = Arc::new(move |evt: crate::narrative::progress::PipelineProgressEvent| {
            // 发射新事件
            let _ = app_handle_progress.emit("pipeline-progress", &evt);
            // 发射旧事件（向后兼容）
            let _ = app_handle_progress.emit("book-analysis-progress", BookAnalysisProgressEvent {
                book_id: book_id_for_progress.clone(),
                status: match evt.status {
                    crate::narrative::progress::StepStatus::Running => "analyzing".to_string(),
                    crate::narrative::progress::StepStatus::Completed => "completed".to_string(),
                    crate::narrative::progress::StepStatus::Failed => "failed".to_string(),
                    _ => "analyzing".to_string(),
                },
                progress: evt.progress_percent,
                current_step: evt.step_name.clone(),
                message: Some(evt.message.clone()),
                active_threads: 0,
                total_chunks: 0,
                processed_chunks: 0,
            });
            // 心跳保活
            heartbeat_ctx.heartbeat();
        });

        let pipeline_result = pipeline_executor.execute(&mut analysis_ctx, &llm, progress_callback).await;

        let analysis_result = match pipeline_result {
            Ok(()) => convert_bundle_to_analysis_result(&analysis_ctx.bundle),
            Err(crate::narrative::pipeline::PipelineError::Cancelled(msg)) => {
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

        // 保存分析结果
        {
            let repo = ReferenceBookRepository::new(self.pool.clone());
            let _ = repo.update_analysis_result(
                book_id,
                Some(analysis_result.book.title.as_str()),
                analysis_result.book.author.as_deref(),
                analysis_result.book.genre.as_deref(),
                analysis_result.book.world_setting.as_deref(),
                analysis_result.book.plot_summary.as_deref(),
                analysis_result.book.story_arc.as_deref(),
            );
            let _ = repo.update_status(book_id, AnalysisStatus::Completed, 100);

            ctx.update_progress("saving", 96, &format!("正在保存 {} 个人物...", analysis_result.characters.len()));
            let char_repo = ReferenceCharacterRepository::new(self.pool.clone());
            let _ = char_repo.create_batch(&analysis_result.characters);

            ctx.update_progress("saving", 98, &format!("正在保存 {} 个场景...", analysis_result.scenes.len()));
            let scene_repo = ReferenceSceneRepository::new(self.pool.clone());
            let _ = scene_repo.create_batch(&analysis_result.scenes);
        }

        // 向量化存储
        ctx.update_progress("saving", 99, "正在生成向量嵌入...");
        {
            let service = super::service::BookDeconstructionService::new(
                self.pool.clone(),
                self.llm_service.clone(),
                self.app_handle.clone(),
            );
            if let Err(e) = service.store_embeddings(book_id, &analysis_result).await {
                log::warn!("[BookDeconstructionExecutor] store_embeddings failed: {}", e);
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

// ==================== v5.3.0: 结果转换器 ====================
/// 将 NarrativeBundle 转换为 BookAnalysisResult（兼容旧接口）
fn convert_bundle_to_analysis_result(bundle: &crate::narrative::elements::NarrativeBundle) -> BookAnalysisResult {
    use chrono::Local;

    let now = Local::now();

    // 构建 ReferenceBook
    let book = if let Some(ref meta) = bundle.story_meta {
        let world_setting = bundle.world_building.as_ref()
            .map(|w| serde_json::to_string(w).ok())
            .flatten();
        let story_arc = bundle.outline.as_ref()
            .map(|o| serde_json::to_string(&o.acts).ok())
            .flatten();
        ReferenceBook {
            id: meta.id.clone(),
            title: meta.title.clone(),
            author: None,
            genre: Some(meta.genre.clone()),
            word_count: None,
            file_format: None,
            file_hash: None,
            file_path: None,
            world_setting,
            plot_summary: Some(meta.description.clone()),
            story_arc,
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
            analysis_status: AnalysisStatus::Completed,
            analysis_progress: 100,
            analysis_error: None,
            task_id: None,
            created_at: now,
            updated_at: now,
        }
    };

    // 转换角色
    let characters: Vec<ReferenceCharacter> = bundle.characters.iter().map(|c| {
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
    }).collect();

    // 转换场景
    let scenes: Vec<ReferenceScene> = bundle.scenes.iter().map(|s| {
        let chars_present_json = serde_json::to_string(&s.characters_present).ok();
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
            created_at: now,
        }
    }).collect();

    BookAnalysisResult { book, characters, scenes }
}
