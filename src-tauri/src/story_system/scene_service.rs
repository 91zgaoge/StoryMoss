#![allow(dead_code)]
//! Scene 领域服务
//!
//! 将原本混杂在 scene_commands.rs 中的业务编排逻辑提取到领域层：
//! - 内容变更时自动知识图谱 Ingest
//! - 向量索引更新
//! - setting 字段变更同步触发 world_building 更新
//! - 状态同步事件发射
//! - 自动化服务触发
//! - Skill Hook 执行
//! - Phase 3: SceneCommitDebouncer — 场景保存后 30s 触发 auto_commit

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use tauri::{AppHandle, Manager};

use crate::{
    automation::service::AutomationService,
    db::{DbPool, KnowledgeGraphRepository, Scene, SceneRepository, SceneUpdate},
    llm::LlmService,
    memory::ingest::{IngestContent, IngestPipeline},
    ports::VectorStore,
    state_sync::StateSync,
    story_system::SceneCommitService,
};

/// Phase 3: 场景级 auto_commit 防抖状态（模块级共享）。
static SCENE_COMMIT_DEBOUNCE: OnceLock<Arc<Mutex<HashMap<String, Instant>>>> = OnceLock::new();

fn get_scene_debounce_map() -> Arc<Mutex<HashMap<String, Instant>>> {
    SCENE_COMMIT_DEBOUNCE
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

const SCENE_COMMIT_DEBOUNCE_SECS: u64 = 30;

/// v0.26.50: 场景 AutoIngest 防抖（与 auto_commit 同窗口）。
/// 根因：幕前每次自动保存都立刻 spawn IngestPipeline
/// LLM，与用户续写抢本地模型， 导致「深度思考」假超时 /
/// 卡死；打字本身不应触发即时后台 LLM。
static SCENE_INGEST_DEBOUNCE: OnceLock<Arc<Mutex<HashMap<String, Instant>>>> = OnceLock::new();

fn get_scene_ingest_debounce_map() -> Arc<Mutex<HashMap<String, Instant>>> {
    SCENE_INGEST_DEBOUNCE
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}

const SCENE_INGEST_DEBOUNCE_SECS: u64 = 30;

// ==================== 组件 1: Scene Ingestor ====================

/// 场景内容自动 Ingest 器。
///
/// 当场景内容或关键元数据被更新时，后台分析并更新知识图谱和向量索引。
pub struct SceneIngestor;

impl SceneIngestor {
    /// 检查是否有值得 ingest 的字段发生变更。
    pub fn should_ingest(updates: &SceneUpdate) -> bool {
        updates.content.is_some()
            || updates.title.is_some()
            || updates.dramatic_goal.is_some()
            || updates.external_pressure.is_some()
            || updates.conflict_type.is_some()
            || updates.outline_content.is_some()
            || updates.draft_content.is_some()
            || updates.setting_location.is_some()
            || updates.setting_time.is_some()
            || updates.setting_atmosphere.is_some()
    }

    /// 启动后台 ingest 任务（立即执行，调用方负责防抖）。
    pub fn spawn_ingest(
        scene_id: String,
        pool: DbPool,
        app_handle: AppHandle,
        vector_store: Arc<dyn VectorStore>,
    ) {
        Self::spawn_ingest_now(scene_id, pool, app_handle, vector_store);
    }

    /// v0.26.50: 防抖后启动 ingest——停止输入 SCENE_INGEST_DEBOUNCE_SECS
    /// 后才跑。
    pub fn spawn_ingest_debounced(
        scene_id: String,
        pool: DbPool,
        app_handle: AppHandle,
        vector_store: Arc<dyn VectorStore>,
    ) {
        let scheduled_time = Instant::now();
        {
            let debounce_arc = get_scene_ingest_debounce_map();
            let mut debounce = debounce_arc.lock().unwrap_or_else(|e| e.into_inner());
            debounce.insert(scene_id.clone(), scheduled_time);
            debounce.retain(|_, last_time| {
                Instant::now().duration_since(*last_time) < Duration::from_secs(24 * 3600)
            });
        }

        let debounce_map = get_scene_ingest_debounce_map();
        let scene_id_for_ingest = scene_id;
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(SCENE_INGEST_DEBOUNCE_SECS)).await;
            let should_run = {
                let debounce = debounce_map.lock().unwrap_or_else(|e| e.into_inner());
                debounce
                    .get(&scene_id_for_ingest)
                    .map(|t| *t == scheduled_time)
                    .unwrap_or(false)
            };
            if !should_run {
                log::debug!(
                    "[SceneIngestor] Scene {} ingest skipped (debounced)",
                    scene_id_for_ingest
                );
                return;
            }
            Self::spawn_ingest_now(scene_id_for_ingest, pool, app_handle, vector_store);
        });
    }

    pub(crate) fn spawn_ingest_now(
        scene_id: String,
        pool: DbPool,
        app_handle: AppHandle,
        vector_store: Arc<dyn VectorStore>,
    ) {
        tauri::async_runtime::spawn(async move {
            // v0.26.50: 与创作路径串行化，避免打字/保存触发的 ingest
            // 抢占本地模型。
            let bg_permit = crate::concurrency::BACKGROUND_LLM_SEMAPHORE.acquire().await;
            if bg_permit.is_err() {
                log::warn!(
                    "[SceneIngestor] Scene {}: failed to acquire BACKGROUND_LLM_SEMAPHORE",
                    scene_id
                );
                return;
            }
            let _bg_permit = bg_permit.unwrap();

            let permit = crate::memory::writer::MEMORY_WRITER_SEMAPHORE
                .acquire()
                .await;
            if permit.is_err() {
                log::warn!(
                    "[SceneIngestor] Scene {}: failed to acquire ingest permit",
                    scene_id
                );
                return;
            }
            let _permit = permit.unwrap();

            let scene_repo = SceneRepository::new(pool.clone());
            let Some(scene) = (match scene_repo.get_by_id(&scene_id) {
                Ok(Some(s)) => Some(s),
                _ => None,
            }) else {
                return;
            };

            let story_id = scene.story_id;
            let content = scene.content.unwrap_or_default();
            if content.len() <= 50 {
                return;
            }

            let content_for_vector = content.clone();
            let app_handle_for_sync = app_handle.clone();
            let llm_service = LlmService::new(app_handle.clone());

            let ingest_result = {
                let pipeline = IngestPipeline::new(llm_service)
                    .with_pool(pool.clone())
                    .with_app_handle(app_handle.clone());
                let ingest_content = IngestContent {
                    text: content,
                    source: format!("scene:{}", scene_id),
                    story_id: story_id.clone(),
                    scene_id: Some(scene_id.clone()),
                };
                match pipeline.ingest(&ingest_content).await {
                    Ok(result) => Some(result),
                    Err(e) => {
                        log::warn!("[SceneIngestor] Scene {}: ingest failed: {}", scene_id, e);
                        None
                    }
                }
            };

            let Some(ingest_result) = ingest_result else {
                return;
            };

            let kg_repo = KnowledgeGraphRepository::new(pool.clone());
            let saved_entities = kg_repo
                .save_entities_batch(&ingest_result.entities)
                .unwrap_or(0);
            let saved_relations = kg_repo
                .save_relations_batch(&ingest_result.relations)
                .unwrap_or(0);

            // D1 Phase 4: 提取实体引用索引（entity_mentions）
            let mention_repo =
                crate::creative_engine::cascade_rewriter::EntityMentionRepository::new(
                    pool.clone(),
                );
            let _ = mention_repo.delete_by_scene(&scene_id);
            let content_for_search = content_for_vector.clone();
            let now = chrono::Utc::now().to_rfc3339();
            for entity in &ingest_result.entities {
                let entity_name = &entity.name;
                let mut start = 0usize;
                while let Some(pos) = content_for_search[start..].find(entity_name) {
                    let absolute_pos = start + pos;
                    let end_pos = absolute_pos + entity_name.len();
                    let mention = crate::creative_engine::cascade_rewriter::models::EntityMention {
                        id: uuid::Uuid::new_v4().to_string(),
                        story_id: story_id.clone(),
                        scene_id: scene_id.clone(),
                        entity_id: entity.id.clone(),
                        entity_type: entity.entity_type.to_string(),
                        start_pos: absolute_pos as i32,
                        end_pos: end_pos as i32,
                        mention_text: entity_name.clone(),
                        confidence: 1.0,
                        created_at: now.clone(),
                        updated_at: now.clone(),
                    };
                    if let Err(e) = mention_repo.create(&mention) {
                        log::warn!(
                            "[SceneIngestor] Failed to create entity mention for {} in scene {}: {}",
                            entity_name,
                            scene_id,
                            e
                        );
                    }
                    start = end_pos;
                }
            }

            log::info!(
                "[SceneIngestor] Scene {}: {} entities, {} relations saved to KG",
                scene_id,
                saved_entities,
                saved_relations
            );

            let _ = StateSync::emit_ingestion_completed(&app_handle_for_sync, &story_id, "scene");
            let _ = StateSync::emit_data_refresh(
                &app_handle_for_sync,
                Some(&story_id),
                "knowledgeGraph",
            );

            // P0-T4: 改稿级联影响分析——编辑旧章后自动产出「受影响下游章节 +
            // 疑似冲突」清单（只报告不改写，作者在级联中心决策）。
            crate::creative_engine::cascade_rewriter::impact_report::spawn_analyze_after_scene_ingest(
                app_handle_for_sync.clone(),
                pool.clone(),
                story_id.clone(),
                scene_id.clone(),
            );

            // 向量索引更新
            match crate::embeddings::embed_text_async(content_for_vector.clone()).await {
                Ok(embedding) => {
                    let record = crate::vector::VectorRecord {
                        id: format!("scene:{}", scene_id),
                        story_id: story_id.clone(),
                        chapter_id: scene.chapter_id.clone().unwrap_or_default(),
                        chapter_number: scene.sequence_number,
                        text: content_for_vector,
                        record_type: "scene".to_string(),
                        metadata: None,
                        embedding,
                    };
                    match vector_store.upsert(record).await {
                        Ok(_) => {
                            log::info!("[SceneIngestor] Scene {} indexed to vector store", scene_id)
                        }
                        Err(e) => {
                            log::warn!("[SceneIngestor] Failed to index scene {}: {}", scene_id, e)
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[SceneIngestor] Failed to generate embedding for scene {}: {}",
                        scene_id,
                        e
                    );
                }
            }
        });
    }
}

// ==================== 组件 2: Scene Automation Trigger ====================

/// 场景相关自动化事件触发器。
pub struct SceneAutomationTrigger;

impl SceneAutomationTrigger {
    pub fn trigger_scene_content_updated(
        automation_service: AutomationService,
        story_id: String,
        scene_id: String,
        word_count: usize,
    ) {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = automation_service
                .trigger_event(
                    crate::automation::triggers::TriggerEvent::SceneContentUpdated {
                        story_id,
                        scene_id,
                        word_count,
                    },
                )
                .await
            {
                log::warn!(
                    "[SceneAutomationTrigger] Failed to trigger scene content updated: {}",
                    e
                );
            }
        });
    }

    pub fn trigger_scene_created(
        automation_service: AutomationService,
        story_id: String,
        scene_id: String,
    ) {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = automation_service
                .trigger_event(crate::automation::triggers::TriggerEvent::SceneCreated {
                    story_id,
                    scene_id,
                })
                .await
            {
                log::warn!(
                    "[SceneAutomationTrigger] Failed to trigger scene created: {}",
                    e
                );
            }
        });
    }
}

// ==================== 领域服务: SceneService ====================

/// Scene 领域服务 orchestrator。
///
/// 命令层（scene_commands.rs）只负责参数校验和调用本服务，
/// 所有业务规则、编排、副作用管理均下沉到此处。
pub struct SceneService {
    pool: DbPool,
    app_handle: AppHandle,
    vector_store: Arc<dyn VectorStore>,
}

impl SceneService {
    pub fn new(pool: DbPool, app_handle: AppHandle, vector_store: Arc<dyn VectorStore>) -> Self {
        Self {
            pool,
            app_handle,
            vector_store,
        }
    }

    /// `update_scene` 成功后的后续业务处理。
    pub fn on_scene_updated(
        &self,
        scene_id: &str,
        story_id: &str,
        updates: &SceneUpdate,
        automation_service: &AutomationService,
    ) {
        // 1. 自动 Ingest：仅元数据变更立刻防抖；正文变更交给
        //    schedule_commit_and_split 同窗观察/ingest，避免双烧。
        if crate::agency::observe::should_spawn_ingest_on_update(
            updates.content.is_some(),
            SceneIngestor::should_ingest(updates),
        ) {
            SceneIngestor::spawn_ingest_debounced(
                scene_id.to_string(),
                self.pool.clone(),
                self.app_handle.clone(),
                self.vector_store.clone(),
            );
        }

        // 2. setting 字段变更同步触发 world_building 更新
        if updates.setting_location.is_some()
            || updates.setting_time.is_some()
            || updates.setting_atmosphere.is_some()
        {
            let _ = StateSync::emit_world_building_updated(&self.app_handle, story_id);
        }

        // 3. 场景更新同步事件（Phase 4: 标记内容是否变更）
        let content_changed = updates.content.is_some();
        let _ = StateSync::emit_scene_updated(
            &self.app_handle,
            story_id,
            scene_id,
            updates.title.as_deref(),
            content_changed,
        );

        // 4. 自动化触发
        let word_count = updates
            .content
            .as_ref()
            .map(|c| c.split_whitespace().count())
            .unwrap_or(0);
        SceneAutomationTrigger::trigger_scene_content_updated(
            automation_service.clone(),
            story_id.to_string(),
            scene_id.to_string(),
            word_count,
        );

        // 5. Phase 3: Scene-level debounced auto_commit (30s idle)
        //    + v0.26.57: 同窗口尝试自动划分章节（仅最新章、超阈值时）
        self.schedule_commit_and_split(scene_id, story_id, content_changed);
    }

    /// Phase 3: 场景保存后 30s 空闲触发防抖 auto_commit；
    /// 内容变更时在同一窗口先尝试自动划分章节（仅最新章、超阈值时，
    /// 见 `chapter_splitter::maybe_split_latest_chapter`）。
    ///
    /// 任何「场景已持久化」的路径（`on_scene_updated`、`update_scene` 命令）
    /// 都应调用本函数，保证防抖语义一致。
    pub fn schedule_commit_and_split(&self, scene_id: &str, story_id: &str, content_changed: bool) {
        let scene_id_for_commit = scene_id.to_string();
        let story_id_for_commit = story_id.to_string();
        let pool = self.pool.clone();
        let app_handle = self.app_handle.clone();
        let vector_store = self.vector_store.clone();
        let content_changed_for_split = content_changed;

        let scheduled_time = Instant::now();
        {
            let debounce_arc = get_scene_debounce_map();
            let mut debounce = debounce_arc.lock().unwrap();
            debounce.insert(scene_id_for_commit.clone(), scheduled_time);
            // 清理超过 24 小时的过期条目
            debounce.retain(|_, last_time| {
                Instant::now().duration_since(*last_time) < Duration::from_secs(24 * 3600)
            });
        }

        let debounce_map = get_scene_debounce_map();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(SCENE_COMMIT_DEBOUNCE_SECS)).await;
            let should_commit = {
                let debounce = debounce_map.lock().unwrap();
                debounce
                    .get(&scene_id_for_commit)
                    .map(|t| *t == scheduled_time)
                    .unwrap_or(false)
            };

            if should_commit {
                let scene_repo = SceneRepository::new(pool.clone());
                if let Ok(Some(scene)) = scene_repo.get_by_id(&scene_id_for_commit) {
                    let chapter_number = scene.sequence_number;
                    let chapter_id = scene.chapter_id.clone();

                    // v0.26.57: 内容变更后尝试自动分章（在 auto_commit 之前，
                    // 使 commit 看到截断后的本章内容）。
                    if content_changed_for_split {
                        if let Some(ref cid) = chapter_id {
                            if let Ok(app_dir) = app_handle.path().app_data_dir() {
                                // map_err 到 String：Box<dyn StdError> 非
                                // Send， if-let
                                // 临时 Result 会跨下方 spawn_blocking 的
                                // await。
                                if let Ok(config) = crate::config::AppConfig::load(&app_dir)
                                    .map_err(|e| e.to_string())
                                {
                                    // v0.33.x fix: maybe_split_latest_chapter
                                    // 是重同步工作
                                    // （最多 50 轮全量读写 + 多次 SQLite
                                    // 事务），直接跑在
                                    // tokio worker 线程上会饿死所有 IPC。挪到
                                    // blocking 线程池。
                                    let pool_for_split = pool.clone();
                                    let app_handle_for_split = app_handle.clone();
                                    let story_id_for_split = story_id_for_commit.clone();
                                    let cid_for_split = cid.clone();
                                    let split_result = tokio::task::spawn_blocking(move || {
                                        crate::story_system::chapter_splitter::maybe_split_latest_chapter(
                                            &pool_for_split,
                                            &app_handle_for_split,
                                            &story_id_for_split,
                                            &cid_for_split,
                                            &config,
                                        )
                                        // AppError 内含非 Send 的 dyn StdError，无法跨 await
                                        // 携带——在 blocking 线程内即转为 String。
                                        .map_err(|e| e.to_string())
                                    })
                                    .await;
                                    match split_result {
                                        Ok(Ok(Some(new_id))) => {
                                            log::info!(
                                                "[SceneCommit] auto chapter split → {}",
                                                new_id
                                            );
                                        }
                                        Ok(Ok(None)) => {}
                                        Ok(Err(e)) => {
                                            log::warn!(
                                                "[SceneCommit] auto chapter split failed: {}",
                                                e
                                            );
                                        }
                                        Err(e) => {
                                            log::warn!(
                                                "[SceneCommit] split task join failed: {}",
                                                e
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 分章后重新读取 scene（内容可能已截断）
                    let scene_for_commit = scene_repo
                        .get_by_id(&scene_id_for_commit)
                        .ok()
                        .flatten()
                        .unwrap_or(scene);

                    let pool_for_obs = pool.clone();
                    let app_for_obs = app_handle.clone();
                    let vs_for_obs = vector_store.clone();
                    let story_for_obs = story_id_for_commit.clone();
                    let scene_for_obs = scene_id_for_commit.clone();
                    let content_for_obs = scene_for_commit.content.clone().unwrap_or_default();

                    let service = SceneCommitService::new(pool);
                    let store: Option<&dyn VectorStore> = Some(vector_store.as_ref());
                    if let Err(e) = service
                        .auto_commit(
                            &story_id_for_commit,
                            Some(&scene_id_for_commit),
                            chapter_id.as_deref(),
                            chapter_number,
                            scene_for_commit.content.as_deref(),
                            None,
                            Some(app_handle),
                            store,
                        )
                        .await
                    {
                        log::warn!(
                            "[SceneCommit] auto_commit failed for scene {}: {}",
                            scene_id_for_commit,
                            e
                        );
                    }

                    if content_changed_for_split {
                        let pool_d = pool_for_obs.clone();
                        let sid = story_for_obs.clone();
                        let scid = scene_for_obs.clone();
                        let chars = content_for_obs.chars().count();
                        let work = tokio::task::spawn_blocking(move || {
                            crate::agency::observe::lookup_post_commit_work(
                                &pool_d, &sid, &scid, chars,
                            )
                        })
                        .await
                        .ok();
                        match work {
                            Some(crate::agency::observe::PostCommitWork::Observe) => {
                                crate::agency::observe::spawn_observe_run(
                                    app_for_obs,
                                    pool_for_obs,
                                    story_for_obs,
                                    scene_for_obs,
                                    content_for_obs,
                                );
                            }
                            Some(crate::agency::observe::PostCommitWork::Ingest) => {
                                SceneIngestor::spawn_ingest_now(
                                    scene_for_obs,
                                    pool_for_obs,
                                    app_for_obs,
                                    vs_for_obs,
                                );
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
    }

    /// `create_scene` 成功后的后续业务处理。
    pub fn on_scene_created(
        &self,
        scene: &Scene,
        has_extra: bool,
        has_setting_changes: bool,
        automation_service: &AutomationService,
    ) {
        // 1. OnSceneCreate Skill Hook
        {
            let skill_manager = crate::skills::SkillManager::from_app_handle(&self.app_handle);
            let story_id = scene.story_id.clone();
            let scene_id = scene.id.clone();
            let scene_title = scene.title.clone();
            tauri::async_runtime::spawn(async move {
                let context =
                    crate::domain::agent_context::AgentContext::minimal(story_id, String::new());
                let data = serde_json::json!({ "scene_id": scene_id, "scene_title": scene_title });
                let _ = skill_manager
                    .execute_hooks(crate::skills::HookEvent::OnSceneCreate, &context, data)
                    .await;
                log::info!(
                    "Hook executed: {:?}",
                    crate::skills::HookEvent::OnSceneCreate
                );
            });
        }

        // 2. 如果额外字段被更新，发射 scene_updated 确保前端缓存刷新（P1-9）
        if has_extra {
            let _ = StateSync::emit_scene_updated(
                &self.app_handle,
                &scene.story_id,
                &scene.id,
                scene.title.as_deref(),
                scene.content.is_some(), // Phase 4
            );
        }

        // 3. 场景创建同步事件
        let _ = StateSync::emit_scene_created(
            &self.app_handle,
            &scene.story_id,
            &scene.id,
            scene.title.as_deref(),
        );

        // 4. setting 字段变更同步触发 world_building 更新
        if has_setting_changes {
            let _ = StateSync::emit_world_building_updated(&self.app_handle, &scene.story_id);
        }

        // 5. 自动化触发
        SceneAutomationTrigger::trigger_scene_created(
            automation_service.clone(),
            scene.story_id.clone(),
            scene.id.clone(),
        );
    }

    /// `delete_scene` 成功后的后续业务处理。
    pub fn on_scene_deleted(&self, scene_id: &str, story_id: &str) {
        // W2-F3: 场景删除后同步触发 world_building 更新（清理无引用规则）
        let _ = StateSync::emit_world_building_updated(&self.app_handle, story_id);
        let _ = StateSync::emit_scene_deleted(&self.app_handle, story_id, scene_id);
    }
}

#[cfg(test)]
mod tests {
    use super::SceneIngestor;
    use crate::db::SceneUpdate;

    #[test]
    fn test_should_ingest_content_update() {
        let mut update = SceneUpdate::default();
        update.content = Some("new content".to_string());
        assert!(SceneIngestor::should_ingest(&update));
    }

    #[test]
    fn test_should_ingest_title_update() {
        let mut update = SceneUpdate::default();
        update.title = Some("new title".to_string());
        assert!(SceneIngestor::should_ingest(&update));
    }

    #[test]
    fn test_should_ingest_empty_update() {
        let update = SceneUpdate::default();
        assert!(!SceneIngestor::should_ingest(&update));
    }

    #[test]
    fn test_should_ingest_setting_location_update() {
        let mut update = SceneUpdate::default();
        update.setting_location = Some("castle".to_string());
        assert!(SceneIngestor::should_ingest(&update));
    }

    #[test]
    fn test_should_ingest_only_navigation_fields() {
        let mut update = SceneUpdate::default();
        update.previous_scene_id = Some("scene-1".to_string());
        update.next_scene_id = Some("scene-3".to_string());
        assert!(!SceneIngestor::should_ingest(&update));
    }

    #[test]
    fn test_ingest_debounce_secs_matches_commit_window() {
        // 契约：打字自动保存触发的 ingest 必须与 auto_commit 同窗口，
        // 否则用户停笔前就会抢占本地模型。
        assert_eq!(
            super::SCENE_INGEST_DEBOUNCE_SECS,
            super::SCENE_COMMIT_DEBOUNCE_SECS
        );
        assert!(super::SCENE_INGEST_DEBOUNCE_SECS >= 15);
    }
}
