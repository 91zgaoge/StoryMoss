#![allow(dead_code)]
//! 自动化服务核心实现

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use tauri::{AppHandle, Emitter, Wry};
use tokio::sync::RwLock;

use super::{
    handlers::AutomationHandler,
    triggers::{AutomationTrigger, TriggerCondition, TriggerEvent},
};
use crate::{
    db::{ChapterReadingPowerRepository, DbPool, DraftRepository, SceneRepository},
    error::AppError,
    reading_power::ReadingPowerEvaluator,
};

/// 排队的事件
#[derive(Debug, Clone)]
struct QueuedEvent {
    event: TriggerEvent,
    timestamp: chrono::DateTime<chrono::Utc>,
    retry_count: u32,
}

/// 自动化服务
pub struct AutomationService {
    app_handle: AppHandle<Wry>,
    db_pool: DbPool,
    triggers: Arc<RwLock<HashMap<String, AutomationTrigger>>>,
    handlers: Arc<RwLock<HashMap<String, AutomationHandler>>>,
    trigger_history: Arc<RwLock<HashMap<String, chrono::DateTime<chrono::Utc>>>>,
    event_queue: Arc<RwLock<Vec<QueuedEvent>>>,
    is_processing: Arc<RwLock<bool>>,
    is_shutdown: Arc<AtomicBool>,
}

impl AutomationService {
    /// 创建新的自动化服务实例
    pub fn new(app_handle: AppHandle<Wry>, db_pool: DbPool) -> Self {
        Self {
            app_handle,
            db_pool,
            triggers: Arc::new(RwLock::new(HashMap::new())),
            handlers: Arc::new(RwLock::new(HashMap::new())),
            trigger_history: Arc::new(RwLock::new(HashMap::new())),
            event_queue: Arc::new(RwLock::new(Vec::new())),
            is_processing: Arc::new(RwLock::new(false)),
            is_shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 初始化自动化服务
    pub async fn initialize(&self) -> Result<(), AppError> {
        log::info!("Initializing automation service...");

        // 注册默认触发器
        self.register_default_triggers().await?;

        // 注册默认处理器
        self.register_default_handlers().await?;

        // 启动事件处理循环
        self.start_event_processor().await;

        log::info!("Automation service initialized successfully");
        Ok(())
    }

    /// 注册默认触发器
    async fn register_default_triggers(&self) -> Result<(), AppError> {
        let mut triggers = self.triggers.write().await;

        // 故事创建触发器
        let story_trigger = AutomationTrigger::new(
            "story_created".to_string(),
            "当创建新故事时触发".to_string(),
            TriggerEvent::StoryCreated {
                story_id: String::new(),
            },
            vec![],
            "init_story_structure".to_string(),
        );
        triggers.insert("story_created".to_string(), story_trigger);

        // 章节创建触发器
        let chapter_trigger = AutomationTrigger::new(
            "chapter_created".to_string(),
            "当创建新章节时触发".to_string(),
            TriggerEvent::ChapterCreated {
                story_id: String::new(),
                chapter_id: String::new(),
            },
            vec![],
            "update_story_progress".to_string(),
        );
        triggers.insert("chapter_created".to_string(), chapter_trigger);

        // 角色创建触发器
        let character_trigger = AutomationTrigger::new(
            "character_created".to_string(),
            "当创建新角色时触发".to_string(),
            TriggerEvent::CharacterCreated {
                story_id: String::new(),
                character_id: String::new(),
            },
            vec![],
            "analyze_character_relationships".to_string(),
        );
        triggers.insert("character_created".to_string(), character_trigger);

        // 章节内容更新触发器
        let content_trigger = AutomationTrigger::new(
            "chapter_content_updated".to_string(),
            "当章节内容更新时触发".to_string(),
            TriggerEvent::ChapterContentUpdated {
                story_id: String::new(),
                chapter_id: String::new(),
                word_count: 0,
            },
            vec![TriggerCondition::WordCountThreshold { min_words: 100 }],
            "update_word_count".to_string(),
        );
        triggers.insert("chapter_content_updated".to_string(), content_trigger);

        // 场景内容更新时评估追读力
        let rp_update_trigger = AutomationTrigger::new(
            "evaluate_reading_power_on_update".to_string(),
            "场景内容更新时自动评估追读力".to_string(),
            TriggerEvent::SceneContentUpdated {
                story_id: String::new(),
                scene_id: String::new(),
                word_count: 0,
            },
            vec![TriggerCondition::Always],
            "evaluate_reading_power".to_string(),
        );
        triggers.insert(
            "evaluate_reading_power_on_update".to_string(),
            rp_update_trigger,
        );

        // 章节定稿时评估追读力
        let rp_finalize_trigger = AutomationTrigger::new(
            "evaluate_reading_power_on_finalize".to_string(),
            "章节定稿时自动评估追读力".to_string(),
            TriggerEvent::ChapterFinalized {
                story_id: String::new(),
                chapter_id: String::new(),
            },
            vec![TriggerCondition::Always],
            "evaluate_reading_power".to_string(),
        );
        triggers.insert(
            "evaluate_reading_power_on_finalize".to_string(),
            rp_finalize_trigger,
        );

        log::info!("Registered {} default triggers", triggers.len());
        Ok(())
    }

    /// 注册默认处理器
    async fn register_default_handlers(&self) -> Result<(), AppError> {
        let mut handlers = self.handlers.write().await;

        // 初始化故事结构处理器
        handlers.insert(
            "init_story_structure".to_string(),
            AutomationHandler {
                name: "init_story_structure".to_string(),
                description: "初始化故事的基本结构和元数据".to_string(),
                handler_type: "workflow".to_string(),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert(
                        "workflow_id".to_string(),
                        serde_json::Value::String("init_story".to_string()),
                    );
                    params
                },
                enabled: true,
            },
        );

        // 更新故事进度处理器
        handlers.insert(
            "update_story_progress".to_string(),
            AutomationHandler {
                name: "update_story_progress".to_string(),
                description: "更新故事的整体进度和统计信息".to_string(),
                handler_type: "workflow".to_string(),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert(
                        "workflow_id".to_string(),
                        serde_json::Value::String("update_progress".to_string()),
                    );
                    params
                },
                enabled: true,
            },
        );

        // 分析角色关系处理器
        handlers.insert(
            "analyze_character_relationships".to_string(),
            AutomationHandler {
                name: "analyze_character_relationships".to_string(),
                description: "分析和更新角色之间的关系".to_string(),
                handler_type: "workflow".to_string(),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert(
                        "workflow_id".to_string(),
                        serde_json::Value::String("analyze_relationships".to_string()),
                    );
                    params
                },
                enabled: true,
            },
        );

        // 更新字数统计处理器
        handlers.insert(
            "update_word_count".to_string(),
            AutomationHandler {
                name: "update_word_count".to_string(),
                description: "更新章节和故事的字数统计".to_string(),
                handler_type: "workflow".to_string(),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert(
                        "workflow_id".to_string(),
                        serde_json::Value::String("update_word_count".to_string()),
                    );
                    params
                },
                enabled: true,
            },
        );

        // 内容分析处理器
        handlers.insert(
            "analyze_content".to_string(),
            AutomationHandler {
                name: "analyze_content".to_string(),
                description: "分析章节内容并提取关键信息".to_string(),
                handler_type: "workflow".to_string(),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert(
                        "workflow_id".to_string(),
                        serde_json::Value::String("analyze_content".to_string()),
                    );
                    params
                },
                enabled: true,
            },
        );

        // 追读力评估处理器
        handlers.insert(
            "evaluate_reading_power".to_string(),
            AutomationHandler {
                name: "evaluate_reading_power".to_string(),
                description: "自动评估章节追读力".to_string(),
                handler_type: "workflow".to_string(),
                parameters: {
                    let mut params = HashMap::new();
                    params.insert(
                        "workflow_id".to_string(),
                        serde_json::Value::String("evaluate_reading_power".to_string()),
                    );
                    params
                },
                enabled: true,
            },
        );

        log::info!("Registered {} default handlers", handlers.len());
        Ok(())
    }

    /// 优雅关闭自动化服务
    pub fn shutdown(&self) {
        self.is_shutdown.store(true, Ordering::Relaxed);
        log::info!("Automation service shutdown requested");
    }

    /// 启动事件处理循环
    async fn start_event_processor(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            loop {
                if service.is_shutdown.load(Ordering::Relaxed) {
                    log::info!("Automation service event processor shutting down");
                    break;
                }
                if let Err(e) = service.process_event_queue().await {
                    log::error!("Error processing event queue: {}", e);
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        });
    }

    /// 处理事件队列
    async fn process_event_queue(&self) -> Result<(), AppError> {
        let mut is_processing = self.is_processing.write().await;
        if *is_processing {
            return Ok(());
        }
        *is_processing = true;
        drop(is_processing);

        let mut queue = self.event_queue.write().await;
        let events_to_process: Vec<QueuedEvent> = queue.drain(..).collect();
        drop(queue);

        for queued_event in events_to_process {
            if let Err(e) = self.process_single_event(&queued_event.event).await {
                log::error!("Failed to process event {:?}: {}", queued_event.event, e);

                // 重试逻辑
                if queued_event.retry_count < 3 {
                    let mut retry_event = queued_event.clone();
                    retry_event.retry_count += 1;
                    let mut queue = self.event_queue.write().await;
                    queue.push(retry_event);
                }
            }
        }

        let mut is_processing = self.is_processing.write().await;
        *is_processing = false;

        Ok(())
    }

    /// 处理单个事件
    async fn process_single_event(&self, event: &TriggerEvent) -> Result<(), AppError> {
        log::debug!("Processing event: {:?}", event);

        let triggers = self.triggers.read().await;
        let handlers = self.handlers.read().await;

        // 查找匹配的触发器
        for (trigger_id, trigger) in triggers.iter() {
            if !trigger.enabled {
                continue;
            }

            // 检查事件类型是否匹配
            if !trigger.matches_event(event) {
                continue;
            }

            log::info!("Trigger '{}' activated for event {:?}", trigger_id, event);

            // 执行关联的处理器
            if let Some(handler) = handlers.get(&trigger.handler_id) {
                if handler.enabled {
                    if let Err(e) = self.execute_handler(handler, event).await {
                        log::error!("Handler '{}' failed: {}", trigger.handler_id, e);
                    } else {
                        log::info!("Handler '{}' executed successfully", trigger.handler_id);
                    }
                }
            }

            // 记录触发历史
            let mut history = self.trigger_history.write().await;
            history.insert(trigger_id.clone(), chrono::Utc::now());
        }

        Ok(())
    }

    /// 执行处理器
    async fn execute_handler(
        &self,
        handler: &AutomationHandler,
        event: &TriggerEvent,
    ) -> Result<(), AppError> {
        match handler.handler_type.as_str() {
            "workflow" => {
                if let Some(workflow_id) = handler
                    .parameters
                    .get("workflow_id")
                    .and_then(|v| v.as_str())
                {
                    self.execute_workflow(workflow_id, &handler.parameters, event)
                        .await?;
                }
            }
            _ => {
                return Err(AppError::validation_failed(
                    format!("Unknown handler type: {}", handler.handler_type),
                    Some("handler_type"),
                ));
            }
        }

        Ok(())
    }

    /// 执行工作流
    async fn execute_workflow(
        &self,
        workflow_id: &str,
        _parameters: &HashMap<String, serde_json::Value>,
        event: &TriggerEvent,
    ) -> Result<String, AppError> {
        log::info!("Executing workflow: {}", workflow_id);

        match workflow_id {
            "init_story" => {
                if let TriggerEvent::StoryCreated { story_id } = event {
                    self.init_story_structure(story_id).await
                } else {
                    Err(AppError::validation_failed(
                        "Invalid event type for init_story workflow",
                        Some("event"),
                    ))
                }
            }
            "update_progress" => {
                if let TriggerEvent::ChapterCreated {
                    story_id,
                    chapter_id,
                } = event
                {
                    self.update_story_progress(story_id, Some(chapter_id)).await
                } else {
                    Err(AppError::validation_failed(
                        "Invalid event type for update_progress workflow",
                        Some("event"),
                    ))
                }
            }
            "analyze_relationships" => {
                if let TriggerEvent::CharacterCreated {
                    story_id,
                    character_id,
                } = event
                {
                    self.analyze_character_relationships(story_id, Some(character_id))
                        .await
                } else {
                    Err(AppError::validation_failed(
                        "Invalid event type for analyze_relationships workflow",
                        Some("event"),
                    ))
                }
            }
            "update_word_count" => {
                if let TriggerEvent::ChapterContentUpdated {
                    story_id,
                    chapter_id,
                    word_count,
                } = event
                {
                    self.update_word_count_stats(story_id, chapter_id, *word_count as i32)
                        .await
                } else {
                    Err(AppError::validation_failed(
                        "Invalid event type for update_word_count workflow",
                        Some("event"),
                    ))
                }
            }
            "analyze_content" => {
                if let TriggerEvent::ChapterContentUpdated {
                    story_id,
                    chapter_id,
                    ..
                } = event
                {
                    self.analyze_chapter_content(story_id, chapter_id).await
                } else {
                    Err(AppError::validation_failed(
                        "Invalid event type for analyze_content workflow",
                        Some("event"),
                    ))
                }
            }
            "evaluate_reading_power" => {
                let story_id = match event {
                    TriggerEvent::SceneContentUpdated { story_id, .. } => story_id.clone(),
                    TriggerEvent::ChapterFinalized { story_id, .. } => story_id.clone(),
                    _ => {
                        return Err(AppError::validation_failed(
                            "Invalid event type for evaluate_reading_power workflow",
                            Some("event"),
                        ))
                    }
                };
                self.evaluate_reading_power(event, &story_id).await
            }
            _ => Err(AppError::not_found("workflow", workflow_id)),
        }
    }

    /// 评估追读力
    async fn evaluate_reading_power(
        &self,
        event: &TriggerEvent,
        story_id: &str,
    ) -> Result<String, AppError> {
        log::info!("Evaluating reading power for story: {}", story_id);

        // 根据事件类型获取 chapter_number
        let (chapter_number, scene_id) = match event {
            TriggerEvent::SceneContentUpdated { scene_id, .. } => {
                let scene_repo = SceneRepository::new(self.db_pool.clone());
                let scene = scene_repo
                    .get_by_id(scene_id)?
                    .ok_or_else(|| AppError::not_found("Scene", scene_id))?;
                (scene.sequence_number, Some(scene_id.clone()))
            }
            TriggerEvent::ChapterFinalized { chapter_id, .. } => {
                let draft_repo = DraftRepository::new(self.db_pool.clone());
                let draft = draft_repo
                    .get_by_id(chapter_id)?
                    .ok_or_else(|| AppError::not_found("Draft", chapter_id))?;
                (draft.chapter_number, draft.scene_id.clone())
            }
            _ => {
                return Err(AppError::validation_failed(
                    "Invalid event type for reading power evaluation",
                    Some("event"),
                ))
            }
        };

        // 执行评估
        let evaluator = ReadingPowerEvaluator::new(self.db_pool.clone());
        let evaluation = evaluator.evaluate(story_id, chapter_number)?;

        // 保存结果到数据库
        let rp_repo = ChapterReadingPowerRepository::new(self.db_pool.clone());
        let coolpoint_json = serde_json::to_string(&evaluation.coolpoint_patterns)
            .unwrap_or_else(|_| "[]".to_string());
        let micropayoffs_json =
            serde_json::to_string(&evaluation.micropayoffs).unwrap_or_else(|_| "[]".to_string());

        match rp_repo.save(
            story_id,
            scene_id.as_deref(),
            chapter_number,
            evaluation.hook_type.as_deref(),
            &evaluation.hook_strength,
            Some(&coolpoint_json),
            Some(&micropayoffs_json),
            evaluation.is_transition,
        ) {
            Ok(record) => {
                // 补充更新 score 和 debt_balance（save 方法未覆盖的字段）
                let conn = self.db_pool.get()?;
                if let Err(e) = conn.execute(
                    "UPDATE chapter_reading_power SET debt_balance = ?1, override_count = ?2 \
                     WHERE id = ?3",
                    rusqlite::params![
                        evaluation.debt_balance,
                        evaluation.override_count,
                        record.id
                    ],
                ) {
                    log::warn!("Failed to update reading power details: {}", e);
                }
            }
            Err(e) => {
                log::warn!("Failed to save reading power result: {}", e);
            }
        }

        // 发送前端事件
        if let Err(e) = self.app_handle.emit(
            "reading_power_evaluated",
            serde_json::json!({
                "story_id": story_id,
                "chapter_number": chapter_number,
                "scene_id": scene_id,
                "score": evaluation.score,
                "hook_strength": evaluation.hook_strength,
                "debt_balance": evaluation.debt_balance,
                "override_count": evaluation.override_count,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ) {
            log::warn!("Failed to emit reading_power_evaluated event: {}", e);
        }

        log::info!(
            "Reading power evaluated for chapter {}: score={}, hook_strength={}",
            chapter_number,
            evaluation.score,
            evaluation.hook_strength
        );

        Ok(format!(
            "Reading power evaluated for chapter {}: score={:.2}, hook_strength={}",
            chapter_number, evaluation.score, evaluation.hook_strength
        ))
    }

    /// 初始化故事结构
    async fn init_story_structure(&self, story_id: &str) -> Result<String, AppError> {
        log::info!("Initializing story structure for story: {}", story_id);

        let conn = self.db_pool.get()?;

        // 创建默认的故事元数据
        conn.execute(
            "INSERT OR IGNORE INTO story_metadata (story_id, key, value) VALUES
             (?1, 'structure_initialized', 'true'),
             (?1, 'auto_sync_enabled', 'true'),
             (?1, 'last_analysis', ?2)",
            rusqlite::params![story_id, chrono::Utc::now().to_rfc3339()],
        )?;

        // 发送前端同步事件
        if let Err(e) = self.app_handle.emit(
            "story_structure_initialized",
            serde_json::json!({
                "story_id": story_id,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ) {
            log::warn!("Failed to emit story_structure_initialized event: {}", e);
        }

        Ok(format!("Story structure initialized for {}", story_id))
    }

    /// 更新故事进度
    async fn update_story_progress(
        &self,
        story_id: &str,
        chapter_id: Option<&str>,
    ) -> Result<String, AppError> {
        log::info!(
            "Updating story progress for story: {}, chapter: {:?}",
            story_id,
            chapter_id
        );

        let conn = self.db_pool.get()?;

        // 计算总章节数和字数
        let mut stmt = conn.prepare(
            "SELECT COUNT(*), COALESCE(SUM(word_count), 0) FROM chapters WHERE story_id = ?1",
        )?;

        let (chapter_count, total_words): (i64, i64) =
            stmt.query_row([story_id], |row| Ok((row.get(0)?, row.get(1)?)))?;

        // 更新故事元数据
        conn.execute(
            "INSERT OR REPLACE INTO story_metadata (story_id, key, value) VALUES
             (?1, 'chapter_count', ?2),
             (?1, 'total_words', ?3),
             (?1, 'last_updated', ?4)",
            rusqlite::params![
                story_id,
                chapter_count.to_string(),
                total_words.to_string(),
                chrono::Utc::now().to_rfc3339()
            ],
        )?;

        // 发送前端同步事件
        if let Err(e) = self.app_handle.emit(
            "story_progress_updated",
            serde_json::json!({
                "story_id": story_id,
                "chapter_count": chapter_count,
                "total_words": total_words,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ) {
            log::warn!("Failed to emit story_progress_updated event: {}", e);
        }

        Ok(format!(
            "Progress updated: {} chapters, {} words",
            chapter_count, total_words
        ))
    }

    /// 分析角色关系
    async fn analyze_character_relationships(
        &self,
        story_id: &str,
        character_id: Option<&str>,
    ) -> Result<String, AppError> {
        log::info!(
            "Analyzing character relationships for story: {}, character: {:?}",
            story_id,
            character_id
        );

        let conn = self.db_pool.get()?;

        // 获取所有角色
        let mut stmt = conn.prepare("SELECT id, name FROM characters WHERE story_id = ?1")?;

        let characters: Vec<(String, String)> = stmt
            .query_map([story_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        // 更新角色关系分析时间戳
        conn.execute(
            "INSERT OR REPLACE INTO story_metadata (story_id, key, value) VALUES
             (?1, 'relationships_analyzed', ?2),
             (?1, 'character_count', ?3)",
            rusqlite::params![
                story_id,
                chrono::Utc::now().to_rfc3339(),
                characters.len().to_string()
            ],
        )?;

        // 发送前端同步事件
        if let Err(e) = self.app_handle.emit(
            "character_relationships_updated",
            serde_json::json!({
                "story_id": story_id,
                "character_count": characters.len(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ) {
            log::warn!(
                "Failed to emit character_relationships_updated event: {}",
                e
            );
        }

        Ok(format!(
            "Analyzed relationships for {} characters",
            characters.len()
        ))
    }

    /// 更新字数统计
    async fn update_word_count_stats(
        &self,
        story_id: &str,
        chapter_id: &str,
        word_count: i32,
    ) -> Result<String, AppError> {
        log::info!(
            "Updating word count stats for story: {}, chapter: {}, words: {}",
            story_id,
            chapter_id,
            word_count
        );

        let conn = self.db_pool.get()?;

        // 更新章节字数
        conn.execute(
            "UPDATE chapters SET word_count = ?1, updated_at = ?2 WHERE id = ?3 AND story_id = ?4",
            rusqlite::params![
                word_count,
                chrono::Utc::now().to_rfc3339(),
                chapter_id,
                story_id
            ],
        )?;

        // 重新计算总字数
        let mut stmt =
            conn.prepare("SELECT COALESCE(SUM(word_count), 0) FROM chapters WHERE story_id = ?1")?;

        let total_words: i64 = stmt.query_row([story_id], |row| row.get(0))?;

        // 更新故事元数据
        conn.execute(
            "INSERT OR REPLACE INTO story_metadata (story_id, key, value) VALUES
             (?1, 'total_words', ?2),
             (?1, 'last_word_update', ?3)",
            rusqlite::params![
                story_id,
                total_words.to_string(),
                chrono::Utc::now().to_rfc3339()
            ],
        )?;

        // 发送前端同步事件
        if let Err(e) = self.app_handle.emit(
            "word_count_updated",
            serde_json::json!({
                "story_id": story_id,
                "chapter_id": chapter_id,
                "chapter_words": word_count,
                "total_words": total_words,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ) {
            log::warn!("Failed to emit word_count_updated event: {}", e);
        }

        Ok(format!(
            "Updated word count: chapter {} words, total {} words",
            word_count, total_words
        ))
    }

    /// 分析章节内容
    async fn analyze_chapter_content(
        &self,
        story_id: &str,
        chapter_id: &str,
    ) -> Result<String, AppError> {
        log::info!(
            "Analyzing chapter content for story: {}, chapter: {}",
            story_id,
            chapter_id
        );

        let conn = self.db_pool.get()?;

        // 获取章节内容
        let mut stmt =
            conn.prepare("SELECT content FROM chapters WHERE id = ?1 AND story_id = ?2")?;

        let content: String = stmt.query_row([chapter_id, story_id], |row| row.get(0))?;

        // 简单的内容分析（实际应用中可以集成AI分析）
        let word_count = content.split_whitespace().count();
        let char_count = content.chars().count();
        let paragraph_count = content.split("\n\n").count();

        // 更新分析结果
        conn.execute(
            "INSERT OR REPLACE INTO story_metadata (story_id, key, value) VALUES
             (?1, 'last_content_analysis', ?2),
             (?1, 'analyzed_chapter_id', ?3)",
            rusqlite::params![story_id, chrono::Utc::now().to_rfc3339(), chapter_id],
        )?;

        // 发送前端同步事件
        if let Err(e) = self.app_handle.emit(
            "content_analyzed",
            serde_json::json!({
                "story_id": story_id,
                "chapter_id": chapter_id,
                "analysis": {
                    "word_count": word_count,
                    "char_count": char_count,
                    "paragraph_count": paragraph_count
                },
                "timestamp": chrono::Utc::now().to_rfc3339()
            }),
        ) {
            log::warn!("Failed to emit content_analyzed event: {}", e);
        }

        Ok(format!(
            "Analyzed content: {} words, {} chars, {} paragraphs",
            word_count, char_count, paragraph_count
        ))
    }

    /// 触发事件
    pub async fn trigger_event(&self, event: TriggerEvent) -> Result<(), AppError> {
        log::debug!("Triggering event: {:?}", event);

        let queued_event = QueuedEvent {
            event,
            timestamp: chrono::Utc::now(),
            retry_count: 0,
        };

        let mut queue = self.event_queue.write().await;
        queue.push(queued_event);

        Ok(())
    }

    /// 获取所有触发器
    pub async fn get_triggers(&self) -> HashMap<String, AutomationTrigger> {
        self.triggers.read().await.clone()
    }

    /// 获取所有处理器
    pub async fn get_handlers(&self) -> HashMap<String, AutomationHandler> {
        self.handlers.read().await.clone()
    }

    /// 添加触发器
    pub async fn add_trigger(&self, trigger: AutomationTrigger) -> Result<(), AppError> {
        let mut triggers = self.triggers.write().await;
        triggers.insert(trigger.name.clone(), trigger);
        Ok(())
    }

    /// 添加处理器
    pub async fn add_handler(&self, handler: AutomationHandler) -> Result<(), AppError> {
        let mut handlers = self.handlers.write().await;
        handlers.insert(handler.name.clone(), handler);
        Ok(())
    }
}

impl Clone for AutomationService {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
            db_pool: self.db_pool.clone(),
            triggers: self.triggers.clone(),
            handlers: self.handlers.clone(),
            trigger_history: self.trigger_history.clone(),
            event_queue: self.event_queue.clone(),
            is_processing: self.is_processing.clone(),
            is_shutdown: self.is_shutdown.clone(),
        }
    }
}
