//! Book Deconstruction Service
//!
//! 业务逻辑层：整合解析器、分块器、分析器，对外提供高层 API。

use std::path::Path;

use chrono::Local;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use super::{
    analyzer::BookAnalyzer, chunker::create_chunks, models::*, parser::parse_book, repository::*,
};
use crate::{
    db::{
        repositories::{SceneRepository, StoryOutlineRepository, WorldBuildingRepository},
        repositories_narrative::{
            NarrativeCharacterRepository, NarrativeSceneRepository,
            NarrativeWorldBuildingRepository,
        },
        CreateCharacterRequest, CreateStoryRequest, DbPool, StoryRepository,
    },
    error::AppError,
    llm::LlmService,
    narrative::elements::{ElementSource, ElementStatus},
    task_system::{models::CreateTaskRequest, service::TaskService},
};

const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024; // 100MB

pub struct BookDeconstructionService {
    pool: DbPool,
    llm_service: LlmService,
    app_handle: AppHandle,
}

impl BookDeconstructionService {
    pub fn new(pool: DbPool, llm_service: LlmService, app_handle: AppHandle) -> Self {
        Self {
            pool,
            llm_service,
            app_handle,
        }
    }

    // ==================== 上传并分析 ====================

    pub async fn upload_and_analyze(&self, file_path: &Path) -> Result<String, ParseError> {
        // 1. 校验文件
        self.validate_file(file_path)?;
        let size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
        log::info!(
            "[BookDeconstruction] Upload: path={} size={} bytes",
            file_path.display(),
            size
        );

        // 2. 计算文件哈希
        let file_hash = self.compute_file_hash(file_path).await?;

        // 3. 检查重复
        let book_repo = ReferenceBookRepository::new(self.pool.clone());
        if let Ok(Some(existing)) = book_repo.get_by_hash(&file_hash) {
            log::info!("[BookDeconstruction] File already exists: {}", existing.id);
            return Ok(existing.id);
        }

        // 4. 生成 book_id
        let book_id = Uuid::new_v4().to_string();

        // 5. 复制到应用数据目录
        let app_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        let books_dir = app_dir.join("books");
        std::fs::create_dir_all(&books_dir)
            .map_err(|e| ParseError::IoError(format!("Failed to create books directory: {}", e)))?;

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt")
            .to_lowercase();
        let dest_path = books_dir.join(format!("{}.{}", book_id, ext));

        tokio::fs::copy(file_path, &dest_path)
            .await
            .map_err(|e| ParseError::IoError(format!("Failed to copy file: {}", e)))?;

        // 6. 解析文件
        self.emit_progress(&book_id, "extracting", 0, "正在解析文件...")
            .await;
        let parsed = parse_book(file_path, None)?;

        // 7. 创建数据库记录
        let now = Local::now();
        let book = ReferenceBook {
            id: book_id.clone(),
            title: parsed.title.clone().unwrap_or_else(|| "未命名".to_string()),
            author: parsed.author.clone(),
            genre: None,
            word_count: Some(parsed.word_count as i64),
            file_format: Some(ext),
            file_hash: Some(file_hash),
            file_path: Some(dest_path.to_string_lossy().to_string()),
            world_setting: None,
            plot_summary: None,
            story_arc: None,
            analyzed_structure_json: None,
            analysis_status: AnalysisStatus::Pending,
            analysis_progress: 0,
            analysis_error: None,
            task_id: None,
            created_at: now,
            updated_at: now,
        };

        book_repo.create(&book).map_err(|e| {
            ParseError::StorageError(format!("Failed to create book record: {}", e))
        })?;

        // 8. 创建任务，由任务系统执行分析
        let payload = serde_json::json!({
            "book_id": book_id,
            "file_path": dest_path.to_string_lossy().to_string(),
        })
        .to_string();

        let task_req = CreateTaskRequest {
            name: format!(
                "拆书: {}",
                parsed.title.clone().unwrap_or_else(|| "未命名".to_string())
            ),
            description: Some(format!("分析 {} 字的小说文件", parsed.word_count)),
            task_type: "book_deconstruction".to_string(),
            schedule_type: "once".to_string(),
            cron_pattern: None,
            payload: Some(payload),
            enabled: Some(true),
            max_retries: Some(3),
            heartbeat_timeout_seconds: Some(300),
        };

        // 使用全局共享的 TaskService（已注册 executor）
        let task_service = self.app_handle.state::<TaskService>();
        match task_service.create_task(task_req) {
            Ok(task) => {
                log::info!(
                    "[BookDeconstruction] Created task {} for book {}",
                    task.id,
                    book_id
                );
                // 更新 book 记录关联 task_id
                let repo = ReferenceBookRepository::new(self.pool.clone());
                let _ = repo.update_task_id(&book_id, &task.id);
                let _ = repo.update_status(&book_id, AnalysisStatus::Pending, 0);
            }
            Err(e) => {
                log::error!(
                    "[BookDeconstruction] Failed to create task for book {}: {}",
                    book_id,
                    e
                );
                // 回退：直接后台分析
                let pool = self.pool.clone();
                let llm_service = self.llm_service.clone();
                let app_handle = self.app_handle.clone();
                let book_id_clone = book_id.clone();
                let chunks = create_chunks(&parsed);
                let word_count = parsed.word_count;
                tauri::async_runtime::spawn(async move {
                    let service = BookDeconstructionService::new(
                        pool.clone(),
                        llm_service.clone(),
                        app_handle.clone(),
                    );
                    if let Err(e) = service
                        .run_analysis(&book_id_clone, &chunks, word_count)
                        .await
                    {
                        log::error!(
                            "[BookDeconstruction] Fallback analysis failed for {}: {}",
                            book_id_clone,
                            e
                        );
                        let repo = ReferenceBookRepository::new(pool.clone());
                        let _ = repo.update_error(&book_id_clone, &e.to_string());
                    }
                });
            }
        }

        Ok(book_id)
    }

    /// 执行分析（后台任务）
    async fn run_analysis(
        &self,
        book_id: &str,
        chunks: &[TextChunk],
        word_count: usize,
    ) -> Result<(), AnalysisError> {
        log::info!("[BookDeconstruction] Running analysis for {}", book_id);

        // 更新状态为分析中
        let repo = ReferenceBookRepository::new(self.pool.clone());
        repo.update_status(book_id, AnalysisStatus::Analyzing, 0)
            .map_err(|e| AnalysisError::StorageError(e.to_string()))?;

        // 读取并发数配置
        let concurrency = {
            let app_dir = self.app_handle.path().app_data_dir().unwrap_or_default();
            crate::config::AppConfig::load(&app_dir)
                .map(|c| c.book_deconstruction_concurrency)
                .unwrap_or(3)
        };

        // 执行 LLM 分析
        let analyzer = BookAnalyzer::new(
            self.llm_service.clone(),
            self.app_handle.clone(),
            self.pool.clone(),
            concurrency,
        );

        let result = analyzer
            .analyze(book_id, chunks, word_count, None, None)
            .await?;

        // 保存分析结果到数据库
        repo.update_analysis_result_with_structure(
            book_id,
            Some(result.book.title.as_str()),
            result.book.author.as_deref(),
            result.book.genre.as_deref(),
            result.book.world_setting.as_deref(),
            result.book.plot_summary.as_deref(),
            result.book.story_arc.as_deref(),
            result.book.analyzed_structure_json.as_deref(),
        )
        .map_err(|e| AnalysisError::StorageError(e.to_string()))?;

        repo.update_status(book_id, AnalysisStatus::Completed, 100)
            .map_err(|e| AnalysisError::StorageError(e.to_string()))?;

        // 保存人物（参考表，兼容旧接口）
        let char_repo = ReferenceCharacterRepository::new(self.pool.clone());
        char_repo
            .create_batch(&result.characters)
            .map_err(|e| AnalysisError::StorageError(e.to_string()))?;

        // 保存场景（参考表，兼容旧接口）
        let scene_repo = ReferenceSceneRepository::new(self.pool.clone());
        scene_repo
            .create_batch(&result.scenes)
            .map_err(|e| AnalysisError::StorageError(e.to_string()))?;

        // W3-B3: 同步保存到 narrative_* 统一表
        {
            let narrative_chars: Vec<crate::narrative::elements::CharacterElement> = result
                .characters
                .iter()
                .map(|c| crate::narrative::elements::CharacterElement {
                    id: c.id.clone(),
                    story_id: book_id.to_string(),
                    name: c.name.clone(),
                    role_type: c.role_type.clone().unwrap_or_default(),
                    personality: c.personality.clone().unwrap_or_default(),
                    background: String::new(),
                    goals: String::new(),
                    fears: String::new(),
                    appearance: c.appearance.clone().unwrap_or_default(),
                    gender: String::new(),
                    age: 0,
                    relationships: Vec::new(),
                    importance_score: c.importance_score.unwrap_or(0.0),
                    source: ElementSource::Extracted,
                    source_ref_id: Some(book_id.to_string()),
                    status: ElementStatus::Reference,
                })
                .collect();
            let nchar_repo = NarrativeCharacterRepository::new(self.pool.clone());
            let _ = nchar_repo.create_batch(&narrative_chars);

            let narrative_scenes: Vec<crate::narrative::elements::SceneElement> = result
                .scenes
                .iter()
                .map(|s| {
                    let chars_present: Vec<String> = s
                        .characters_present
                        .as_ref()
                        .and_then(|cp| serde_json::from_str(cp).ok())
                        .unwrap_or_default();
                    crate::narrative::elements::SceneElement {
                        id: s.id.clone(),
                        story_id: book_id.to_string(),
                        sequence_number: s.sequence_number,
                        title: s.title.clone().unwrap_or_default(),
                        summary: s.summary.clone().unwrap_or_default(),
                        dramatic_goal: String::new(),
                        external_pressure: String::new(),
                        conflict_type: s.conflict_type.clone().unwrap_or_default(),
                        characters_present: chars_present,
                        setting_location: String::new(),
                        setting_time: String::new(),
                        content: None,
                        narrative_intensity: s.narrative_intensity.unwrap_or(0.0),
                        narrative_sentiment: s.narrative_sentiment.unwrap_or(0.0),
                        narrative_event_types: s
                            .narrative_event_types
                            .as_ref()
                            .and_then(|et| serde_json::from_str(et).ok())
                            .unwrap_or_default(),
                        act_number: s.act_number.unwrap_or(1),
                        position_in_act: s.position_in_act.unwrap_or(0.0),
                        source: ElementSource::Extracted,
                        source_ref_id: Some(book_id.to_string()),
                        status: ElementStatus::Reference,
                    }
                })
                .collect();
            let nscene_repo = NarrativeSceneRepository::new(self.pool.clone());
            let _ = nscene_repo.create_batch(&narrative_scenes);

            if let Some(ref world_json) = result.book.world_setting {
                if let Ok(mut wb) = serde_json::from_str::<
                    crate::narrative::elements::WorldBuildingElement,
                >(world_json)
                {
                    wb.id = uuid::Uuid::new_v4().to_string();
                    wb.story_id = book_id.to_string();
                    wb.source = ElementSource::Extracted;
                    wb.source_ref_id = Some(book_id.to_string());
                    wb.status = ElementStatus::Reference;
                    let wb_repo = NarrativeWorldBuildingRepository::new(self.pool.clone());
                    let _ = wb_repo.create(&wb);
                }
            }
        }

        // 向量化存储
        self.store_embeddings(book_id, &result).await?;

        self.emit_progress(book_id, "completed", 100, "分析完成")
            .await;
        log::info!("[BookDeconstruction] Analysis completed for {}", book_id);

        Ok(())
    }

    // ==================== 查询操作 ====================

    pub fn get_status(&self, book_id: &str) -> Result<AnalysisStatusResponse, AppError> {
        let repo = ReferenceBookRepository::new(self.pool.clone());
        let book = repo
            .get_by_id(book_id)
            .map_err(AppError::from)?
            .ok_or_else(|| "Book not found".to_string())?;

        Ok(AnalysisStatusResponse {
            book_id: book_id.to_string(),
            status: book.analysis_status.to_string(),
            progress: book.analysis_progress,
            current_step: None,
            error: book.analysis_error,
            active_threads: 0,
            max_threads: 0,
            task_id: book.task_id,
        })
    }

    pub fn get_analysis(&self, book_id: &str) -> Result<BookAnalysisResult, AppError> {
        let book_repo = ReferenceBookRepository::new(self.pool.clone());

        let book = book_repo
            .get_by_id(book_id)
            .map_err(AppError::from)?
            .ok_or_else(|| "Book not found".to_string())?;

        // W3-B3: 从 narrative_* 统一表读取拆书结果
        let nchar_repo = NarrativeCharacterRepository::new(self.pool.clone());
        let narrative_chars = nchar_repo.get_by_story(book_id).map_err(AppError::from)?;
        let characters: Vec<ReferenceCharacter> = narrative_chars
            .into_iter()
            .map(|c| {
                let relationships_json = serde_json::to_string(&c.relationships).ok();
                ReferenceCharacter {
                    id: c.id,
                    book_id: c.story_id,
                    name: c.name,
                    role_type: Some(c.role_type),
                    personality: Some(c.personality),
                    appearance: Some(c.appearance),
                    relationships: relationships_json,
                    key_scenes: None,
                    importance_score: Some(c.importance_score),
                    created_at: Local::now(),
                }
            })
            .collect();

        let nscene_repo = NarrativeSceneRepository::new(self.pool.clone());
        let narrative_scenes = nscene_repo.get_by_story(book_id).map_err(AppError::from)?;
        let scenes: Vec<ReferenceScene> = narrative_scenes
            .into_iter()
            .map(|s| {
                let chars_present_json = serde_json::to_string(&s.characters_present).ok();
                ReferenceScene {
                    id: s.id,
                    book_id: s.story_id,
                    sequence_number: s.sequence_number,
                    title: Some(s.title),
                    summary: Some(s.summary),
                    characters_present: chars_present_json,
                    key_events: None,
                    conflict_type: Some(s.conflict_type),
                    emotional_tone: None,
                    narrative_intensity: Some(s.narrative_intensity),
                    narrative_sentiment: Some(s.narrative_sentiment),
                    narrative_event_types: None,
                    act_number: Some(s.act_number),
                    position_in_act: Some(s.position_in_act),
                    created_at: Local::now(),
                }
            })
            .collect();

        Ok(BookAnalysisResult {
            book,
            characters,
            scenes,
        })
    }

    pub fn list_books(&self) -> Result<Vec<ReferenceBookSummary>, AppError> {
        let repo = ReferenceBookRepository::new(self.pool.clone());
        repo.list_all().map_err(AppError::from)
    }

    // ==================== 删除 ====================

    pub fn delete_book(&self, book_id: &str) -> Result<(), AppError> {
        // 删除数据库记录
        let book_repo = ReferenceBookRepository::new(self.pool.clone());
        let char_repo = ReferenceCharacterRepository::new(self.pool.clone());
        let scene_repo = ReferenceSceneRepository::new(self.pool.clone());

        char_repo.delete_by_book(book_id).map_err(AppError::from)?;
        scene_repo.delete_by_book(book_id).map_err(AppError::from)?;
        book_repo.delete(book_id).map_err(AppError::from)?;

        // W3-B3: 同时删除 narrative_* 统一表中的记录
        let nchar_repo = NarrativeCharacterRepository::new(self.pool.clone());
        let nscene_repo = NarrativeSceneRepository::new(self.pool.clone());
        let nwb_repo = NarrativeWorldBuildingRepository::new(self.pool.clone());
        let _ = nchar_repo.delete_by_story(book_id);
        let _ = nscene_repo.delete_by_story(book_id);
        let _ = nwb_repo.delete_by_story(book_id);

        // 删除文件
        let app_dir = self.app_handle.path().app_data_dir().unwrap_or_default();
        let books_dir = app_dir.join("books");
        for ext in &["txt", "pdf", "epub"] {
            let file_path = books_dir.join(format!("{}.{}", book_id, ext));
            if file_path.exists() {
                let _ = std::fs::remove_file(&file_path);
            }
        }

        log::info!("[BookDeconstruction] Book deleted: {}", book_id);
        Ok(())
    }

    /// 取消拆书分析
    pub fn cancel_analysis(&self, book_id: &str) -> Result<(), AppError> {
        let book_repo = ReferenceBookRepository::new(self.pool.clone());
        let book = book_repo
            .get_by_id(book_id)
            .map_err(AppError::from)?
            .ok_or_else(|| "Book not found".to_string())?;

        // 如果有关联的任务，取消任务
        if let Some(task_id) = book.task_id {
            let task_service = self.app_handle.state::<TaskService>();
            if let Err(e) = task_service.cancel_task(&task_id) {
                log::warn!(
                    "[BookDeconstruction] Failed to cancel task {}: {}",
                    task_id,
                    e
                );
            }
        }

        // 更新 book 状态为已取消
        book_repo
            .update_status(book_id, AnalysisStatus::Cancelled, book.analysis_progress)
            .map_err(AppError::from)?;

        Ok(())
    }

    // ==================== 一键转故事 ====================

    pub async fn convert_to_story(&self, book_id: &str) -> Result<String, AppError> {
        log::info!("[BookDeconstruction] Converting book {} to story", book_id);
        let analysis = self.get_analysis(book_id)?;
        let pool = self.pool.clone();

        // 1. 创建故事
        let story_repo = StoryRepository::new(pool.clone());
        let story = story_repo
            .create(CreateStoryRequest {
                title: analysis.book.title.clone(),
                description: analysis.book.plot_summary.clone(),
                genre: analysis.book.genre.clone(),
                style_dna_id: None,
            })
            .map_err(AppError::from)?;
        let story_id = story.id;
        log::info!(
            "[BookDeconstruction] Convert step {}: created {}",
            "story",
            story_id
        );

        // 2. 创建世界观
        if let Some(ref world_setting) = analysis.book.world_setting {
            let wb_repo = WorldBuildingRepository::new(pool.clone());
            wb_repo
                .create(&story_id, world_setting)
                .map_err(AppError::from)?;
            log::info!(
                "[BookDeconstruction] Convert step {}: created {}",
                "world_building",
                story_id
            );
        }

        // 3. 创建角色（合并 personality + appearance 作为 background）
        for (_i, character) in analysis.characters.iter().enumerate() {
            let char_repo = crate::db::CharacterRepository::new(pool.clone());
            let background = match (&character.personality, &character.appearance) {
                (Some(p), Some(_a)) => Some(format!("{}", p)),
                (Some(p), None) => Some(p.clone()),
                (None, Some(a)) => Some(a.clone()),
                (None, None) => None,
            };
            char_repo
                .create(CreateCharacterRequest {
                    story_id: story_id.clone(),
                    name: character.name.clone(),
                    background,
                    personality: character.personality.clone(),
                    goals: None,
                    appearance: character.appearance.clone(),
                    gender: None,
                    age: None,
                })
                .map_err(AppError::from)?;
        }
        log::info!(
            "[BookDeconstruction] Convert step {}: created {}",
            "characters",
            analysis.characters.len()
        );

        // 4. 创建场景（summary 保存为 content，保留 outline）
        for scene in &analysis.scenes {
            let scene_repo = SceneRepository::new(pool.clone());
            let created = scene_repo
                .create(&story_id, scene.sequence_number, scene.title.as_deref())
                .map_err(AppError::from)?;
            // 保存 summary 为 content
            if scene.summary.is_some() {
                use crate::db::repositories::SceneUpdate;
                let _ = scene_repo.update(
                    &created.id,
                    &SceneUpdate {
                        title: None,
                        content: scene.summary.clone(),
                        characters_present: scene.characters_present.as_ref().map(|s| {
                            s.split(',')
                                .map(|x| x.trim().to_string())
                                .filter(|x| !x.is_empty())
                                .collect()
                        }),
                        ..Default::default()
                    },
                );
            }
        }
        log::info!(
            "[BookDeconstruction] Convert step {}: created {}",
            "scenes",
            analysis.scenes.len()
        );

        // 5. 创建 StoryOutlines（如果存在叙事结构分析结果）— LitSeg Phase 4
        if let Some(ref structure_json) = analysis.book.analyzed_structure_json {
            let outline_repo = StoryOutlineRepository::new(pool.clone());
            let act_count = serde_json::from_str::<Vec<serde_json::Value>>(structure_json)
                .ok()
                .map(|v| v.len() as i32)
                .unwrap_or(3);
            let _ = outline_repo.create(
                &story_id,
                "由拆书分析自动生成的叙事结构",
                Some(structure_json),
                act_count,
                Some(analysis.scenes.len() as i32),
            );
            log::info!(
                "[BookDeconstruction] Convert step {}: created story_outline with {} acts for story {}",
                "story_outline",
                act_count,
                story_id
            );
        }

        // W3-B3: 将 narrative_* 表中对应记录状态从 Reference 更新为 Active
        {
            let conn = self.pool.get().map_err(AppError::from)?;
            let _ = conn.execute(
                "UPDATE narrative_characters SET status = 'active' WHERE story_id = ?1",
                [book_id],
            );
            let _ = conn.execute(
                "UPDATE narrative_scenes SET status = 'active' WHERE story_id = ?1",
                [book_id],
            );
            let _ = conn.execute(
                "UPDATE narrative_world_buildings SET status = 'active' WHERE story_id = ?1",
                [book_id],
            );
            log::info!(
                "[BookDeconstruction] Convert step {}: activated narrative elements for {}",
                "status_update",
                book_id
            );
        }

        Ok(story_id)
    }

    // ==================== 内部辅助 ====================

    fn validate_file(&self, file_path: &Path) -> Result<(), ParseError> {
        // 检查文件大小
        let metadata = std::fs::metadata(file_path)
            .map_err(|e| ParseError::IoError(format!("Failed to read file metadata: {}", e)))?;

        if metadata.len() > MAX_FILE_SIZE {
            return Err(ParseError::FileTooLarge(format!(
                "File size {} exceeds maximum {}",
                metadata.len(),
                MAX_FILE_SIZE
            )));
        }

        // 检查扩展名
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "txt" | "pdf" | "epub" => Ok(()),
            _ => Err(ParseError::InvalidFormat(format!(
                "Unsupported file format: {}",
                ext
            ))),
        }
    }

    async fn compute_file_hash(&self, file_path: &Path) -> Result<String, ParseError> {
        let bytes = tokio::fs::read(file_path)
            .await
            .map_err(|e| ParseError::IoError(format!("Failed to read file: {}", e)))?;

        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let result = hasher.finalize();
        Ok(hex::encode(result))
    }

    pub(crate) async fn store_embeddings(
        &self,
        book_id: &str,
        result: &BookAnalysisResult,
    ) -> Result<(), AnalysisError> {
        use crate::{embeddings::embed_text_async, vector::VectorRecord};

        let store = match crate::VECTOR_STORE.get() {
            Some(s) => s,
            None => {
                log::warn!(
                    "[BookDeconstruction] Vector store not initialized, skipping embeddings"
                );
                return Ok(());
            }
        };

        log::info!(
            "[BookDeconstruction] Storing embeddings for book {}",
            book_id
        );

        // 为场景生成 embedding
        let mut scene_records = Vec::new();
        for (idx, scene) in result.scenes.iter().enumerate() {
            let text = format!(
                "{}\n{}",
                scene.title.as_deref().unwrap_or(""),
                scene.summary.as_deref().unwrap_or("")
            );
            if text.trim().is_empty() {
                continue;
            }
            if let Ok(embedding) = embed_text_async(text.clone()).await {
                let metadata = serde_json::json!({
                    "act_number": scene.act_number,
                    "narrative_intensity": scene.narrative_intensity,
                    "narrative_sentiment": scene.narrative_sentiment,
                    "position_in_act": scene.position_in_act,
                })
                .to_string();
                scene_records.push(VectorRecord {
                    id: format!("{}_scene_{}", book_id, idx),
                    story_id: book_id.to_string(),
                    chapter_id: scene.id.clone(),
                    chapter_number: scene.sequence_number as i32,
                    text,
                    record_type: "reference_scene".to_string(),
                    metadata: Some(metadata),
                    embedding,
                });
            }
        }
        for record in scene_records {
            if let Err(e) = store.upsert(record).await {
                log::warn!(
                    "[BookDeconstruction] Failed to upsert scene embedding: {}",
                    e
                );
            }
        }

        // 为人物生成 embedding
        let mut char_records = Vec::new();
        for (idx, character) in result.characters.iter().enumerate() {
            let text = format!(
                "{}\n{}",
                character.name,
                character.personality.as_deref().unwrap_or("")
            );
            if text.trim().is_empty() {
                continue;
            }
            if let Ok(embedding) = embed_text_async(text.clone()).await {
                char_records.push(VectorRecord {
                    id: format!("{}_char_{}", book_id, idx),
                    story_id: book_id.to_string(),
                    chapter_id: character.id.clone(),
                    chapter_number: 0,
                    text,
                    record_type: "reference_character".to_string(),
                    metadata: None,
                    embedding,
                });
            }
        }
        for record in char_records {
            if let Err(e) = store.upsert(record).await {
                log::warn!(
                    "[BookDeconstruction] Failed to upsert character embedding: {}",
                    e
                );
            }
        }

        log::info!(
            "[BookDeconstruction] Embeddings stored for book {}",
            book_id
        );
        Ok(())
    }

    async fn emit_progress(&self, book_id: &str, status: &str, progress: i32, message: &str) {
        let event = BookAnalysisProgressEvent {
            book_id: book_id.to_string(),
            status: status.to_string(),
            progress,
            current_step: message.to_string(),
            message: Some(message.to_string()),
            active_threads: 0,
            total_chunks: 0,
            processed_chunks: 0,
        };
        let _ = self.app_handle.emit("book-analysis-progress", event);
    }
}

// ==================== 响应类型 ====================

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalysisStatusResponse {
    pub book_id: String,
    pub status: String,
    pub progress: i32,
    pub current_step: Option<String>,
    pub error: Option<String>,
    /// 当前活跃的 LLM 并发线程数
    #[serde(default)]
    pub active_threads: i32,
    /// 最大 LLM 并发线程数
    #[serde(default)]
    pub max_threads: i32,
    /// 关联的任务ID
    #[serde(default)]
    pub task_id: Option<String>,
}
