//! Chapter 领域服务
//!
//! 将原本混杂在 commands/chapter.rs 中的业务编排逻辑提取到领域层：
//! - 30s debounce 自动 commit
//! - 逾期伏笔检测
//! - 自动化服务触发
//! - 状态同步事件发射
//! - Skill Hook 执行

use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::{
    automation::service::AutomationService,
    creative_engine::payoff_ledger::PayoffLedger,
    db::{Chapter, ChapterRepository, DbPool, SceneRepository},
    state_sync::StateSync,
    story_system::SceneCommitService,
    CHAPTER_COMMIT_DEBOUNCE, CHAPTER_COMMIT_DEBOUNCE_SECONDS, SKILL_MANAGER, VECTOR_STORE,
};

// ==================== 组件 1: Commit Debouncer ====================

/// 章节保存后的自动 commit debouncer。
///
/// W4-B9: 防止频繁保存导致重复 commit。每次 `update_chapter` 成功后
/// 调用 `schedule`，若 30s 内无新调度则执行 `SceneCommitService::auto_commit`。
pub struct ChapterCommitDebouncer;

impl ChapterCommitDebouncer {
    /// 调度一次 debounced auto commit。
    pub fn schedule(
        chapter_id: String,
        story_id: String,
        chapter_number: i32,
        _content: Option<String>,
        pool: DbPool,
        app_handle: AppHandle,
    ) {
        let scheduled_time = Instant::now();
        {
            let mut debounce = CHAPTER_COMMIT_DEBOUNCE.lock().unwrap();
            debounce.insert(chapter_id.clone(), scheduled_time);
            // 清理超过 24 小时的过期条目，防止内存泄漏
            const MAX_DEBOUNCE_AGE_HOURS: u64 = 24;
            debounce.retain(|_, last_time| {
                Instant::now().duration_since(*last_time)
                    < Duration::from_secs(MAX_DEBOUNCE_AGE_HOURS * 3600)
            });
        }

        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_secs(CHAPTER_COMMIT_DEBOUNCE_SECONDS)).await;
            let should_commit = {
                let debounce = CHAPTER_COMMIT_DEBOUNCE.lock().unwrap();
                debounce
                    .get(&chapter_id)
                    .map(|t| *t == scheduled_time)
                    .unwrap_or(false)
            };

            if should_commit {
                let repo = ChapterRepository::new(pool.clone());
                let scene_repo = SceneRepository::new(pool.clone());
                if let Ok(Some(chapter)) = repo.get_by_id(&chapter_id) {
                    let scene_id = scene_repo
                        .get_by_chapter(&chapter_id)
                        .ok()
                        .and_then(|scenes| scenes.into_iter().next())
                        .map(|s| s.id);
                    let service = SceneCommitService::new(pool);
                    let store = VECTOR_STORE.get();
                    if let Err(e) = service
                        .auto_commit(
                            &story_id,
                            scene_id.as_deref(),
                            Some(&chapter_id),
                            chapter_number,
                            chapter.content.as_deref(),
                            Some(app_handle),
                            store,
                        )
                        .await
                    {
                        log::warn!(
                            "[ChapterCommitDebouncer] auto_commit failed for chapter {}: {}",
                            chapter_id,
                            e
                        );
                    }
                }
            } else {
                log::info!(
                    "[ChapterCommitDebouncer] auto_commit skipped for chapter {} (debounced)",
                    chapter_id
                );
            }
        });
    }
}

// ==================== 组件 2: Payoff Detector ====================

/// 逾期伏笔检测器。
///
/// P1 修复: 章节保存/创建后检查逾期伏笔，并向前端发射同步事件。
pub struct PayoffDetector;

impl PayoffDetector {
    /// 检测指定故事的逾期伏笔并发射事件。
    pub fn detect_and_emit(
        story_id: String,
        chapter_number: i32,
        pool: DbPool,
        app_handle: AppHandle,
    ) {
        tauri::async_runtime::spawn(async move {
            let ledger = PayoffLedger::new(pool);
            match ledger.detect_overdue(&story_id, chapter_number) {
                Ok(overdue) if !overdue.is_empty() => {
                    log::info!(
                        "[PayoffDetector] {} overdue payoffs detected for story {}",
                        overdue.len(),
                        story_id
                    );
                    let _ = StateSync::emit_payoff_overdue(&app_handle, &story_id, &overdue);
                }
                _ => {}
            }
        });
    }
}

// ==================== 组件 3: Automation Trigger ====================

/// 自动化事件触发器。
///
/// 将 automation_service 的事件触发封装为可复用的同步/异步方法。
pub struct AutomationTrigger;

impl AutomationTrigger {
    /// 触发 ChapterContentUpdated 自动化事件。
    pub fn trigger_chapter_content_updated(
        automation_service: AutomationService,
        story_id: String,
        chapter_id: String,
        word_count: usize,
    ) {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = automation_service
                .trigger_event(
                    crate::automation::triggers::TriggerEvent::ChapterContentUpdated {
                        story_id,
                        chapter_id,
                        word_count,
                    },
                )
                .await
            {
                log::warn!(
                    "[AutomationTrigger] Failed to trigger chapter content updated: {}",
                    e
                );
            }
        });
    }

    /// 触发 ChapterCreated 自动化事件。
    pub fn trigger_chapter_created(
        automation_service: AutomationService,
        story_id: String,
        chapter_id: String,
    ) {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = automation_service
                .trigger_event(crate::automation::triggers::TriggerEvent::ChapterCreated {
                    story_id,
                    chapter_id,
                })
                .await
            {
                log::warn!(
                    "[AutomationTrigger] Failed to trigger chapter created automation: {}",
                    e
                );
            }
        });
    }
}

// ==================== 领域服务: ChapterService ====================

/// Chapter 领域服务 orchestrator。
///
/// 命令层（commands/chapter.rs）只负责参数校验和调用本服务，
/// 所有业务规则、编排、副作用管理均下沉到此处。
pub struct ChapterService {
    pool: DbPool,
    app_handle: AppHandle,
}

impl ChapterService {
    pub fn new(pool: DbPool, app_handle: AppHandle) -> Self {
        Self { pool, app_handle }
    }

    /// `update_chapter` 成功后的后续业务处理。
    ///
    /// 包含：状态同步事件、自动化触发、逾期伏笔检测、debounced auto commit。
    pub fn on_chapter_updated(
        &self,
        chapter_id: &str,
        story_id: &str,
        chapter_number: i32,
        title: Option<String>,
        word_count: Option<i32>,
        automation_service: &AutomationService,
    ) {
        // 1. 保存状态事件
        let _ = crate::window::WindowManager::send_to_frontstage(
            &self.app_handle,
            crate::window::FrontstageEvent::SaveStatus {
                saved: true,
                timestamp: Some(chrono::Local::now().to_rfc3339()),
            },
        );

        // 2. 章节更新同步事件
        let _ = StateSync::emit_chapter_updated(
            &self.app_handle,
            chapter_id,
            title.as_deref(),
            story_id,
        );

        // 3. 自动化触发
        let word_count_val = word_count.unwrap_or(0) as usize;
        AutomationTrigger::trigger_chapter_content_updated(
            automation_service.clone(),
            story_id.to_string(),
            chapter_id.to_string(),
            word_count_val,
        );

        // 4. 逾期伏笔检测
        PayoffDetector::detect_and_emit(
            story_id.to_string(),
            chapter_number,
            self.pool.clone(),
            self.app_handle.clone(),
        );

        // 5. Debounced auto commit
        ChapterCommitDebouncer::schedule(
            chapter_id.to_string(),
            story_id.to_string(),
            chapter_number,
            None, // content will be fetched inside debouncer
            self.pool.clone(),
            self.app_handle.clone(),
        );
    }

    /// `create_chapter` 成功后的后续业务处理。
    ///
    /// 包含：Skill Hook、状态同步事件、逾期伏笔检测、自动化触发、即时 auto
    /// commit。
    pub fn on_chapter_created(
        &self,
        chapter: &Chapter,
        title: Option<String>,
        automation_service: &AutomationService,
    ) {
        // 1. AfterChapterSave Skill Hook
        if let Some(manager) = SKILL_MANAGER.get() {
            if let Ok(skill_manager) = manager.lock() {
                let story_id = chapter.story_id.clone();
                let chapter_id = chapter.id.clone();
                let chapter_number = chapter.chapter_number;
                let skill_manager = skill_manager.clone();
                tauri::async_runtime::spawn(async move {
                    let context = crate::agents::AgentContext::minimal(story_id, String::new());
                    let data = serde_json::json!({ "chapter_id": chapter_id, "chapter_number": chapter_number });
                    let _ = skill_manager
                        .execute_hooks(crate::skills::HookEvent::AfterChapterSave, &context, data)
                        .await;
                    log::info!(
                        "Hook executed: {:?}",
                        crate::skills::HookEvent::AfterChapterSave
                    );
                });
            }
        }

        // 2. 章节创建同步事件
        let _ = StateSync::emit_chapter_created(
            &self.app_handle,
            &chapter.story_id,
            &chapter.id,
            title.as_deref(),
        );

        // 3. 关联 Scene 更新同步事件（chapter 创建会自动创建/关联 scene）
        {
            let scene_repo = SceneRepository::new(self.pool.clone());
            if let Ok(scenes) = scene_repo.get_by_chapter(&chapter.id) {
                if let Some(scene) = scenes.first() {
                    let _ = StateSync::emit_scene_updated(
                        &self.app_handle,
                        &chapter.story_id,
                        &scene.id,
                        title.as_deref(),
                    );
                }
            }
        }

        // 4. 逾期伏笔检测
        PayoffDetector::detect_and_emit(
            chapter.story_id.clone(),
            chapter.chapter_number,
            self.pool.clone(),
            self.app_handle.clone(),
        );

        // 5. 自动化触发
        AutomationTrigger::trigger_chapter_created(
            automation_service.clone(),
            chapter.story_id.clone(),
            chapter.id.clone(),
        );

        // 6. 新建章节后自动触发 chapter commit（无需 debounce，首次创建只执行一次）
        let scene_id_for_commit = {
            let scene_repo = SceneRepository::new(self.pool.clone());
            scene_repo
                .get_by_chapter(&chapter.id)
                .ok()
                .and_then(|scenes| scenes.into_iter().next())
                .map(|s| s.id)
        };

        let chapter_id = chapter.id.clone();
        let app_handle = self.app_handle.clone();
        let story_id = chapter.story_id.clone();
        let chapter_number = chapter.chapter_number;
        let content = chapter.content.clone();
        let pool = self.pool.clone();

        tauri::async_runtime::spawn(async move {
            let service = SceneCommitService::new(pool);
            let store = VECTOR_STORE.get();
            if let Err(e) = service
                .auto_commit(
                    &story_id,
                    scene_id_for_commit.as_deref(),
                    Some(&chapter_id),
                    chapter_number,
                    content.as_deref(),
                    Some(app_handle),
                    store,
                )
                .await
            {
                log::warn!(
                    "[ChapterService] auto_commit failed for new chapter {}: {}",
                    chapter_id,
                    e
                );
            }
        });
    }
}
