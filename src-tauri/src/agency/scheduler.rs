use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Semaphore};

use crate::{
    agency::{
        chat_repository::ChatRepository,
        coordinator::AgencyCoordinator,
        models::{AgencyAgentTask, AgentRole},
        reporter::TaskReporter,
        repository::{AgencyRepository, MAX_ACTIVE_WRITE_RUNS_PER_STORY},
        task_repository::TaskRepository,
    },
    db::{
        repositories::{ChapterRepository, SceneRepository},
        DbPool,
    },
    error::AppError,
};

pub const EVENT_CHAT_UPDATE: &str = "agency-chat-update";
pub const STORY_TASK_CONCURRENCY: usize = 4;

/// v0.86.0：导演单次「续写到第 N 章」的最大章数。与幕前批量续写
/// `agency_continue_batch` 的 `clamp(1, 5)` 上限一致；更长区间由导演
/// 用多个 dispatch + depends_on_action 串行分批。
pub const MAX_DIRECTED_BATCH_CHAPTERS: usize = 5;

/// 「续写到第 end 章自动停」的批量章数换算。
/// `next_chapter` 为原子分配器口径的下一可分配章号；目标章被越过或
/// 超出 [`MAX_DIRECTED_BATCH_CHAPTERS`] 时返回可操作错误原因。
pub(crate) fn directed_batch_count(next_chapter: i32, target_end: i32) -> Result<usize, String> {
    if target_end < next_chapter {
        return Err(format!(
            "目标章 第{target_end}章 已被最新章越过（下一章为 第{next_chapter}章），请改用修订或提高目标章号"
        ));
    }
    let count = (target_end - next_chapter + 1) as usize;
    if count > MAX_DIRECTED_BATCH_CHAPTERS {
        return Err(format!(
            "一次最多连写 {MAX_DIRECTED_BATCH_CHAPTERS} 章（第{next_chapter}→第{target_end} 需 {count} 章），请用多个 dispatch + depends_on_action 串行分批"
        ));
    }
    Ok(count)
}

type WaiterList = Vec<oneshot::Sender<()>>;

/// 进程级任务取消登记表：不同命令/调度器实例共享，cancel 命令能停掉
/// 任一实例派发的任务。
static TASK_CANCEL_FLAGS: Lazy<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn register_task_cancel(task_id: &str) -> Arc<AtomicBool> {
    let mut flags = TASK_CANCEL_FLAGS.lock().unwrap_or_else(|p| p.into_inner());
    let token = Arc::new(AtomicBool::new(false));
    flags.insert(task_id.to_string(), token.clone());
    token
}

fn cancel_task_flag(task_id: &str) -> bool {
    let flags = TASK_CANCEL_FLAGS.lock().unwrap_or_else(|p| p.into_inner());
    if let Some(token) = flags.get(task_id) {
        token.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

fn unregister_task_cancel(task_id: &str) {
    TASK_CANCEL_FLAGS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(task_id);
}

/// 子 agent 异步调度器：任务落库 → spawn → 角色/故事限流 → 心跳 → 完成回调。
/// `dispatch` 立即返回 task_id；完成写入任务表 + 可选聊天 follow-up。
#[derive(Clone)]
pub struct TaskScheduler {
    pool: DbPool,
    coordinator: Arc<AgencyCoordinator>,
    app: Option<AppHandle>,
    role_sems: Arc<HashMap<AgentRole, Arc<Semaphore>>>,
    story_sems: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
    waiters: Arc<Mutex<HashMap<String, WaiterList>>>,
}

impl TaskScheduler {
    pub fn new(pool: DbPool, coordinator: Arc<AgencyCoordinator>, app: Option<AppHandle>) -> Self {
        let mut role_sems = HashMap::new();
        for role in AgentRole::all() {
            let permits = match role {
                AgentRole::Director => 1usize,
                AgentRole::LeadWriter | AgentRole::Writer | AgentRole::OutlinePlanner => 1usize,
                AgentRole::Producer | AgentRole::EditorAuditor | AgentRole::Inspector => 2usize,
                AgentRole::StyleMimic => 1usize,
            };
            role_sems.insert(role, Arc::new(Semaphore::new(permits)));
        }
        Self {
            pool,
            coordinator,
            app,
            role_sems: Arc::new(role_sems),
            story_sems: Arc::new(Mutex::new(HashMap::new())),
            waiters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 派发子 agent 任务：立即返回 task_id，不等待完成。
    #[allow(clippy::too_many_arguments)]
    pub async fn dispatch_sub_agent(
        &self,
        story_id: &str,
        run_id: &str,
        role: AgentRole,
        premise: &str,
        instruction: &str,
        depends_on_task_id: Option<String>,
        parent_task_id: Option<String>,
        meta: String,
    ) -> Result<String, AppError> {
        // Director 是编排层，不是子 agent：导演只走 DirectorPlanner，
        // 把它塞进 ToolLoop 会让计划 JSON 被当成 tool action 解析（日志曾
        // 连续 8 轮 missing field `type` 熔断）。
        if role == AgentRole::Director {
            return Err(AppError::validation_failed(
                "Director 是总控，不能作为子 agent 任务；请用剧组对话",
                Some("role"),
            ));
        }
        let mut task =
            AgencyAgentTask::new(run_id, story_id, role, instruction, parent_task_id, meta);
        let pool = self.pool.clone();
        let task_c = task.clone();
        tokio::task::spawn_blocking(move || TaskRepository::new(pool).create_task(&task_c))
            .await
            .map_err(|e| AppError::from(format!("dispatch join error: {e}")))?
            .map_err(AppError::from)?;
        task.status = "queued".to_string();
        self.emit_task(&task);
        let task_id = task.id.clone();

        let sched = self.clone();
        let premise = premise.to_string();
        let instruction = instruction.to_string();
        let meta = serde_json::from_str::<serde_json::Value>(&task.meta_json).ok();
        let directed_chapter = role == AgentRole::LeadWriter
            && matches!(
                meta.as_ref()
                    .and_then(|v| v.get("directed_chapter").and_then(|x| x.as_bool())),
                Some(true)
            );
        let target_chapter: Option<i32> = meta
            .as_ref()
            .and_then(|v| v.get("target_chapter").and_then(|x| x.as_i64()))
            .and_then(|n| i32::try_from(n).ok())
            .filter(|n| *n >= 1);
        // v0.82.2：导演派发指定场景修订（scenes.sequence_number）
        let target_scene: Option<i32> = meta
            .as_ref()
            .and_then(|v| v.get("target_scene").and_then(|x| x.as_i64()))
            .and_then(|n| i32::try_from(n).ok())
            .filter(|n| *n >= 1);
        // v0.86.0：导演派发「续写到第 N 章（含）自动停」
        let target_end_chapter: Option<i32> = meta
            .as_ref()
            .and_then(|v| v.get("target_end_chapter").and_then(|x| x.as_i64()))
            .and_then(|n| i32::try_from(n).ok())
            .filter(|n| *n >= 1);
        let target_chapter = if target_scene.is_some() || target_end_chapter.is_some() {
            None
        } else {
            target_chapter
        };
        // v0.93.3: 派发可观测性——打包日志此前完全看不到导演派发了什么
        log::info!(
            "agency: 派发子任务 task={} role={} directed_chapter={} target_chapter={:?} target_scene={:?} target_end_chapter={:?}",
            task_id,
            role.as_str(),
            directed_chapter,
            target_chapter,
            target_scene,
            target_end_chapter
        );
        tokio::spawn(async move {
            if directed_chapter {
                sched
                    .run_directed_chapter_task(
                        task,
                        instruction,
                        depends_on_task_id,
                        target_chapter,
                        target_scene,
                        target_end_chapter,
                    )
                    .await;
            } else {
                sched.run_task(task, premise, depends_on_task_id).await;
            }
        });
        Ok(task_id)
    }

    pub fn list_tasks(&self, story_id: &str, limit: i32) -> Result<Vec<AgencyAgentTask>, AppError> {
        TaskRepository::new(self.pool.clone())
            .list_for_story(story_id, limit)
            .map_err(AppError::from)
    }

    /// 重试失败/取消/stale 任务：复用原 role/task/run_id/meta。
    pub async fn retry_task(&self, task_id: &str) -> Result<String, AppError> {
        let pool = self.pool.clone();
        let id = task_id.to_string();
        let old = tokio::task::spawn_blocking(move || TaskRepository::new(pool).get_task(&id))
            .await
            .map_err(|e| AppError::from(format!("retry join error: {e}")))?
            .map_err(AppError::from)?
            .ok_or_else(|| {
                AppError::validation_failed(
                    format!(
                        "任务不存在或已清理: {task_id}（target_id 须为巡检快照中的完整任务 ID）"
                    ),
                    Some("task_id"),
                )
            })?;
        if !matches!(old.status.as_str(), "failed" | "cancelled" | "stale") {
            return Err(AppError::validation_failed(
                format!("任务 {} 状态为 {}，不能重试", task_id, old.status),
                Some("task_status"),
            ));
        }
        let premise = parse_meta_premise(&old.meta_json).unwrap_or_else(|| old.task.clone());
        self.dispatch_sub_agent(
            &old.story_id,
            &old.run_id,
            old.role,
            &premise,
            &old.task,
            None,
            Some(old.id.clone()),
            old.meta_json.clone(),
        )
        .await
    }

    pub fn cancel_task(&self, task_id: &str) -> Result<(), AppError> {
        cancel_task_flag(task_id);
        // v0.80.5: 导演派发的 LeadWriter 以 task.id 作为续写 run id；主流程
        // 共享本次的 cancel flag，这里再清一次同 id 的附属后台任务
        // （资产回流/后台质检等）。
        crate::agency::coordinator::cancel_agency_run(task_id);
        let pool = self.pool.clone();
        let id = task_id.to_string();
        TaskRepository::new(pool)
            .set_status(&id, "cancelling")
            .map_err(AppError::from)?;
        Ok(())
    }

    async fn run_task(
        &self,
        task: AgencyAgentTask,
        premise: String,
        depends_on_task_id: Option<String>,
    ) {
        if let Some(parent) = depends_on_task_id.clone() {
            if !self.wait_for_parent(&parent).await {
                let reporter = TaskReporter::new(
                    self.pool.clone(),
                    &task.id,
                    &task.story_id,
                    &task.run_id,
                    task.role,
                    self.app.clone(),
                );
                let _ = reporter
                    .finish("failed", None, Some("父任务未完成或超时"))
                    .await;
                // v0.93.3: 失败终态写 error 级日志
                log::error!(
                    "agency: 对话派发任务失败（父任务未完成或超时）task={} role={}",
                    task.id,
                    task.role.as_str()
                );
                return;
            }
        }

        let story_sem = self.story_sem_for(&task.story_id);
        let _story_permit = match story_sem.acquire().await {
            Ok(p) => p,
            Err(_) => return,
        };
        let role_sem = self
            .role_sems
            .get(&task.role)
            .cloned()
            .unwrap_or_else(|| Arc::new(Semaphore::new(1)));
        let _role_permit = match role_sem.acquire().await {
            Ok(p) => p,
            Err(_) => return,
        };

        let cancel = register_task_cancel(&task.id);

        let reporter = TaskReporter::new(
            self.pool.clone(),
            &task.id,
            &task.story_id,
            &task.run_id,
            task.role,
            self.app.clone(),
        );
        if reporter.start().await.is_err() {
            return;
        }

        let result = self
            .coordinator
            .run_role_task_reported(
                &task.run_id,
                &task.story_id,
                task.role,
                &premise,
                &task.task,
                &reporter,
                Some(cancel.clone()),
            )
            .await;

        let cancelled = cancel.load(Ordering::SeqCst);
        // v0.80.4: aborted 不再一律等同「用户取消」。tool_loop 的
        // MaxTurns/Deadline/ParseFailures 熔断必须落 failed（带熔断主因），
        // 否则导演看到 cancelled 会误判成用户撤单并反复重新派发同一任务。
        // 只有 cancel 令牌命中才是真取消；LeadWriter/Writer 的熔断若已被
        // coordinator salvage 成草稿，则 aborted 已被清掉，走 completed。
        let (status, summary, error) = match result {
            Ok(r) if cancelled => ("cancelled", Some("已取消".to_string()), None),
            Ok(r) if r.aborted => {
                let reason = crate::agency::coordinator::circuit_break_reason(&r);
                (
                    "failed",
                    None,
                    Some(format!("tool_loop 熔断（{}），未产出可用结果", reason)),
                )
            }
            Ok(r) => {
                let summary = truncate_summary(&r.output);
                ("completed", Some(summary), None)
            }
            Err(e) if cancelled => ("cancelled", Some("已取消".to_string()), Some(e.to_string())),
            Err(e) => ("failed", None, Some(e.to_string())),
        };
        let _ = reporter
            .finish(status, summary.as_deref(), error.as_deref())
            .await;

        // v0.93.3: 失败终态写 error 级日志（此前 agency 失败链全程零 error，
        // 打包日志看不到任何错误级条目）
        if status == "failed" {
            log::error!(
                "agency: 对话派发任务失败 task={} role={} error={}",
                task.id,
                task.role.as_str(),
                error.as_deref().unwrap_or("未知错误")
            );
        }

        self.notify_waiters(&task.id);
        unregister_task_cancel(&task.id);
        if status == "completed" {
            // v0.95.0：成功也必须回填原占位消息（而不是追加一条新消息）。
            // 旧实现成功后占位消息永远停在 acting，前端会看到“已完成 + 思考中”
            // 两条并存。
            self.complete_chat_task(&task, summary.as_deref());
        } else {
            // v0.80.4: 失败/取消也要回填聊天占位消息，否则 @主创 等直达派发
            // 的占位消息会一直停在 acting，作者看不到失败原因。
            self.update_chat_task_status(&task, status, error.as_deref());
        }
    }

    /// v0.80.5：导演派发的 LeadWriter 任务走「章号分配 → 批量续写(1 章)
    /// → GateRunner/handle_gate → 装配」管线。章号由
    /// `story_chapter_sequences` 原子分配；质量门与装配复用批量续写同一
    /// 路径，不把黑板草稿直接写库。
    async fn run_directed_chapter_task(
        &self,
        task: AgencyAgentTask,
        instruction: String,
        depends_on_task_id: Option<String>,
        target_chapter: Option<i32>,
        target_scene: Option<i32>,
        target_end_chapter: Option<i32>,
    ) {
        if let Some(parent) = depends_on_task_id {
            if !self.wait_for_parent(&parent).await {
                let reporter = TaskReporter::new(
                    self.pool.clone(),
                    &task.id,
                    &task.story_id,
                    &task.run_id,
                    task.role,
                    self.app.clone(),
                );
                self.finish_directed_task(
                    &task,
                    &reporter,
                    "failed",
                    None,
                    Some("父任务未完成或超时".to_string()),
                )
                .await;
                return;
            }
        }

        let story_sem = self.story_sem_for(&task.story_id);
        let _story_permit = match story_sem.acquire().await {
            Ok(p) => p,
            Err(_) => return,
        };
        let role_sem = self
            .role_sems
            .get(&task.role)
            .cloned()
            .unwrap_or_else(|| Arc::new(Semaphore::new(1)));
        let _role_permit = match role_sem.acquire().await {
            Ok(p) => p,
            Err(_) => return,
        };

        let cancel = register_task_cancel(&task.id);
        let reporter = TaskReporter::new(
            self.pool.clone(),
            &task.id,
            &task.story_id,
            &task.run_id,
            task.role,
            self.app.clone(),
        );
        if reporter.start().await.is_err() {
            unregister_task_cancel(&task.id);
            return;
        }
        if cancel.load(Ordering::SeqCst) {
            self.finish_directed_task(
                &task,
                &reporter,
                "cancelled",
                Some("已取消".to_string()),
                None,
            )
            .await;
            return;
        }

        // 与 agency_continue_chapter 同一上限；超过时任务失败而不是静默排队。
        let pool = self.pool.clone();
        let sid = task.story_id.clone();
        let active = match tokio::task::spawn_blocking(move || {
            AgencyRepository::new(pool)
                .count_active_write_runs(&sid)
                .map_err(AppError::from)
        })
        .await
        {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => {
                self.finish_directed_task(&task, &reporter, "failed", None, Some(e.to_string()))
                    .await;
                return;
            }
            Err(e) => {
                self.finish_directed_task(
                    &task,
                    &reporter,
                    "failed",
                    None,
                    Some(format!("并发上限检查失败: {e}")),
                )
                .await;
                return;
            }
        };
        if active >= MAX_ACTIVE_WRITE_RUNS_PER_STORY {
            self.finish_directed_task(
                &task,
                &reporter,
                "failed",
                None,
                Some(format!(
                    "该故事并行创作任务已达上限（{MAX_ACTIVE_WRITE_RUNS_PER_STORY}）"
                )),
            )
            .await;
            return;
        }

        // v0.82.2: target_scene 表示「修订指定场景」——按 scenes.sequence_number
        // 找到场景与其所属章，走场景修订管线（gate 通过只更新该 scene）。
        if let Some(scene_seq) = target_scene {
            let pool = self.pool.clone();
            let sid = task.story_id.clone();
            let resolved = tokio::task::spawn_blocking(move || {
                let scenes = SceneRepository::new(pool.clone())
                    .get_by_story(&sid)
                    .map_err(AppError::from)?;
                let scene = scenes.into_iter().find(|s| s.sequence_number == scene_seq);
                let Some(scene) = scene else {
                    return Ok::<Option<(String, i32)>, AppError>(None);
                };
                let chapter_number = scene
                    .chapter_id
                    .and_then(|cid| ChapterRepository::new(pool).get_by_id(&cid).ok().flatten())
                    .map(|c| c.chapter_number)
                    .unwrap_or(scene.sequence_number);
                Ok(Some((scene.id, chapter_number)))
            })
            .await;
            let (scene_id, chapter_number) = match resolved {
                Ok(Ok(Some(v))) => v,
                Ok(Ok(None)) => {
                    self.finish_directed_task(
                        &task,
                        &reporter,
                        "failed",
                        None,
                        Some(format!("目标场景不存在: 场景{scene_seq}")),
                    )
                    .await;
                    return;
                }
                Ok(Err(e)) => {
                    self.finish_directed_task(
                        &task,
                        &reporter,
                        "failed",
                        None,
                        Some(e.to_string()),
                    )
                    .await;
                    return;
                }
                Err(e) => {
                    self.finish_directed_task(
                        &task,
                        &reporter,
                        "failed",
                        None,
                        Some(format!("目标场景校验失败: {e}")),
                    )
                    .await;
                    return;
                }
            };
            let result = self
                .coordinator
                .run_revise_scene_with_cancel(
                    &task.id,
                    &task.story_id,
                    &scene_id,
                    chapter_number,
                    &instruction,
                    cancel.clone(),
                )
                .await;
            let cancelled = cancel.load(Ordering::SeqCst);
            let (status, summary, error) = match result {
                Ok(chapter_result) if !cancelled => (
                    "completed",
                    Some(format!(
                        "第{}章场景{}已按导演指令修订并通过质量门并装配（scene={}，verdict={}）",
                        chapter_result.chapter_number,
                        scene_seq,
                        chapter_result.scene_id,
                        chapter_result.verdict.verdict
                    )),
                    None,
                ),
                Ok(_) => ("cancelled", Some("已取消".to_string()), None),
                Err(e) if cancelled => {
                    ("cancelled", Some("已取消".to_string()), Some(e.to_string()))
                }
                Err(e) => ("failed", None, Some(e.to_string())),
            };
            self.finish_directed_task(&task, &reporter, status, summary, error)
                .await;
            return;
        }

        // v0.80.6: target_chapter 表示「修订既有章」——不分配新章号，
        // 直接校验该章存在并走修订管线（仍过 GateRunner 后更新原 scene）。
        if let Some(ch) = target_chapter {
            let pool = self.pool.clone();
            let sid = task.story_id.clone();
            let exists = tokio::task::spawn_blocking(move || {
                SceneRepository::new(pool)
                    .get_by_story(&sid)
                    .map(|scenes| {
                        scenes
                            .into_iter()
                            .find(|s| s.sequence_number == ch)
                            .is_some()
                    })
                    .map_err(AppError::from)
            })
            .await;
            match exists {
                Ok(Ok(true)) => {}
                Ok(Ok(false)) => {
                    self.finish_directed_task(
                        &task,
                        &reporter,
                        "failed",
                        None,
                        Some(format!("目标章不存在: 第{ch}章")),
                    )
                    .await;
                    return;
                }
                Ok(Err(e)) => {
                    self.finish_directed_task(
                        &task,
                        &reporter,
                        "failed",
                        None,
                        Some(e.to_string()),
                    )
                    .await;
                    return;
                }
                Err(e) => {
                    self.finish_directed_task(
                        &task,
                        &reporter,
                        "failed",
                        None,
                        Some(format!("目标章校验失败: {e}")),
                    )
                    .await;
                    return;
                }
            }
            let result = self
                .coordinator
                .run_revise_chapter_with_cancel(
                    &task.id,
                    &task.story_id,
                    ch,
                    &instruction,
                    cancel.clone(),
                )
                .await;
            let cancelled = cancel.load(Ordering::SeqCst);
            let (status, summary, error) = match result {
                Ok(chapter_result) if !cancelled => (
                    "completed",
                    Some(format!(
                        "第{}章已按导演指令修订并通过质量门并装配（scene={}，verdict={}）",
                        chapter_result.chapter_number,
                        chapter_result.scene_id,
                        chapter_result.verdict.verdict
                    )),
                    None,
                ),
                Ok(_) => ("cancelled", Some("已取消".to_string()), None),
                Err(e) if cancelled => {
                    ("cancelled", Some("已取消".to_string()), Some(e.to_string()))
                }
                Err(e) => ("failed", None, Some(e.to_string())),
            };
            self.finish_directed_task(&task, &reporter, status, summary, error)
                .await;
            return;
        }

        // v0.86.0：「续写到第 N 章（含）自动停」——按下一可分配章号换算
        // 批量章数；目标章被越过或超出单次上限时任务失败并给出可操作原因。
        let batch_count: usize = match target_end_chapter {
            None => 1,
            Some(end) => {
                let pool = self.pool.clone();
                let sid = task.story_id.clone();
                let next = match tokio::task::spawn_blocking(move || {
                    AgencyRepository::new(pool)
                        .peek_next_chapter(&sid)
                        .map_err(AppError::from)
                })
                .await
                {
                    Ok(Ok(n)) => n,
                    Ok(Err(e)) => {
                        self.finish_directed_task(
                            &task,
                            &reporter,
                            "failed",
                            None,
                            Some(format!("目标章换算失败: {e}")),
                        )
                        .await;
                        return;
                    }
                    Err(e) => {
                        self.finish_directed_task(
                            &task,
                            &reporter,
                            "failed",
                            None,
                            Some(format!("目标章换算 join error: {e}")),
                        )
                        .await;
                        return;
                    }
                };
                match directed_batch_count(next, end) {
                    Ok(n) => n,
                    Err(reason) => {
                        self.finish_directed_task(&task, &reporter, "failed", None, Some(reason))
                            .await;
                        return;
                    }
                }
            }
        };

        // 章号分配：唯一写者 story_chapter_sequences，禁止猜 MAX+1。
        let pool = self.pool.clone();
        let sid = task.story_id.clone();
        let chapter = match tokio::task::spawn_blocking(move || {
            AgencyRepository::new(pool)
                .allocate_chapter_numbers(&sid, batch_count)
                .map_err(AppError::from)
        })
        .await
        {
            Ok(Ok(mut chapters)) => match chapters.first().copied() {
                Some(n) => n,
                None => {
                    self.finish_directed_task(
                        &task,
                        &reporter,
                        "failed",
                        None,
                        Some("章号分配失败".to_string()),
                    )
                    .await;
                    return;
                }
            },
            Ok(Err(e)) => {
                self.finish_directed_task(&task, &reporter, "failed", None, Some(e.to_string()))
                    .await;
                return;
            }
            Err(e) => {
                self.finish_directed_task(
                    &task,
                    &reporter,
                    "failed",
                    None,
                    Some(format!("章号分配 join error: {e}")),
                )
                .await;
                return;
            }
        };

        let result = self
            .coordinator
            .run_continue_batch_with_cancel(
                &task.id,
                &task.story_id,
                chapter,
                batch_count,
                &instruction,
                cancel.clone(),
            )
            .await;
        let cancelled = cancel.load(Ordering::SeqCst);
        let (status, summary, error) = match result {
            Ok(batch) if !cancelled => match (batch.chapters.first(), batch.chapters.last()) {
                (Some(first), Some(last)) if batch.chapters.len() > 1 => (
                    "completed",
                    Some(format!(
                        "第{}-{}章已全部续写并通过质量门并装配（{} 章，到达第{}章自动停止；末章 verdict={}）",
                        first.chapter_number,
                        last.chapter_number,
                        batch.chapters.len(),
                        target_end_chapter.unwrap_or(last.chapter_number),
                        last.verdict.verdict
                    )),
                    None,
                ),
                (Some(ch), _) => (
                    "completed",
                    Some(format!(
                        "第{}章已续写并通过质量门并装配（scene={}，verdict={}）",
                        ch.chapter_number, ch.scene_id, ch.verdict.verdict
                    )),
                    None,
                ),
                (None, _) => ("failed", None, Some("质量门/装配未产出章节".to_string())),
            },
            Ok(_) => ("cancelled", Some("已取消".to_string()), None),
            Err(e) if cancelled => ("cancelled", Some("已取消".to_string()), Some(e.to_string())),
            Err(e) => ("failed", None, Some(e.to_string())),
        };
        self.finish_directed_task(&task, &reporter, status, summary, error)
            .await;
    }

    async fn finish_directed_task(
        &self,
        task: &AgencyAgentTask,
        reporter: &TaskReporter,
        status: &str,
        summary: Option<String>,
        error: Option<String>,
    ) {
        let _ = reporter
            .finish(status, summary.as_deref(), error.as_deref())
            .await;
        // v0.93.3: 失败终态写 error 级日志
        if status == "failed" {
            log::error!(
                "agency: 导演派发任务失败 task={} role={} error={}",
                task.id,
                task.role.as_str(),
                error.as_deref().unwrap_or("未知错误")
            );
        }
        self.notify_waiters(&task.id);
        unregister_task_cancel(&task.id);
        if status == "completed" {
            self.complete_chat_task(task, summary.as_deref());
        } else {
            self.update_chat_task_status(task, status, error.as_deref());
        }
    }

    /// 父任务完成通知：注册 oneshot，避免「父已完成但通知未到」窗口。
    async fn wait_for_parent(&self, parent_task_id: &str) -> bool {
        if self.is_terminal(parent_task_id).await.unwrap_or(false) {
            return true;
        }
        let rx = {
            let mut waiters = self.waiters.lock().unwrap_or_else(|p| p.into_inner());
            let (tx, rx) = oneshot::channel();
            waiters
                .entry(parent_task_id.to_string())
                .or_default()
                .push(tx);
            rx
        };
        if self.is_terminal(parent_task_id).await.unwrap_or(false) {
            return true;
        }
        matches!(
            tokio::time::timeout(Duration::from_secs(300), rx).await,
            Ok(Ok(()))
        )
    }

    async fn is_terminal(&self, task_id: &str) -> Result<bool, AppError> {
        let pool = self.pool.clone();
        let id = task_id.to_string();
        let task = tokio::task::spawn_blocking(move || TaskRepository::new(pool).get_task(&id))
            .await
            .map_err(|e| AppError::from(format!("task join error: {e}")))?
            .map_err(AppError::from)?;
        Ok(task
            .map(|t| {
                matches!(
                    t.status.as_str(),
                    "completed" | "failed" | "cancelled" | "stale"
                )
            })
            .unwrap_or(false))
    }

    fn notify_waiters(&self, task_id: &str) {
        let senders = self
            .waiters
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(task_id)
            .unwrap_or_default();
        for tx in senders {
            let _ = tx.send(());
        }
    }

    fn story_sem_for(&self, story_id: &str) -> Arc<Semaphore> {
        let mut map = self.story_sems.lock().unwrap_or_else(|p| p.into_inner());
        map.entry(story_id.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(STORY_TASK_CONCURRENCY)))
            .clone()
    }

    fn emit_task(&self, task: &AgencyAgentTask) {
        if let Some(app) = &self.app {
            let _ = app.emit(
                crate::agency::reporter::EVENT_CREW_UPDATE,
                serde_json::json!({
                    "story_id": task.story_id,
                    "task": task,
                }),
            );
        }
    }

    /// 成功回填原占位消息：占位消息 → done，内容取任务摘要。
    ///
    /// 没有占位消息（非对话派发）时才追加 follow-up，保持后台任务可见。
    fn complete_chat_task(&self, task: &AgencyAgentTask, summary: Option<&str>) {
        let Some(message_id) = parse_meta_chat_message(&task.meta_json) else {
            self.append_chat_follow_up(task, summary);
            return;
        };
        let content = summary
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{} 任务完成", task.role.as_str()));
        let repo = ChatRepository::new(self.pool.clone());
        if repo
            .update_message_payload(&message_id, "done", Some(&content), None, None, None)
            .is_ok()
        {
            if let Some(app) = &self.app {
                let _ = app.emit(
                    EVENT_CHAT_UPDATE,
                    serde_json::json!({
                        "session_id": parse_meta_chat_session(&task.meta_json).unwrap_or_default(),
                        "message_id": message_id,
                        "role": task.role.as_str(),
                        "status": "done",
                        "content": content,
                        "task_ids": [task.id.clone()],
                    }),
                );
            }
        }
    }

    fn append_chat_follow_up(&self, task: &AgencyAgentTask, summary: Option<&str>) {
        let Some(session_id) = parse_meta_chat_session(&task.meta_json) else {
            return;
        };
        let Some(content) = summary else {
            return;
        };
        let repo = ChatRepository::new(self.pool.clone());
        if let Ok(msg) = repo.append_task_result(
            &session_id,
            task.role.as_str(),
            content,
            "done",
            &task.id,
            Some(&task.run_id),
        ) {
            if let Some(app) = &self.app {
                let _ = app.emit(
                    EVENT_CHAT_UPDATE,
                    serde_json::json!({
                        "session_id": msg.session_id,
                        "message_id": msg.id,
                        "role": msg.role,
                        "status": msg.status,
                        "content": msg.content,
                        "task_ids": [task.id.clone()],
                    }),
                );
            }
        }
    }

    /// 失败/取消回填原占位消息（不是追加 follow-up）：作者在对话里能看到
    /// 「主创任务失败：…」而不是永远 pending/acting。
    fn update_chat_task_status(&self, task: &AgencyAgentTask, status: &str, error: Option<&str>) {
        let Some(message_id) = parse_meta_chat_message(&task.meta_json) else {
            return;
        };
        let content = match status {
            "cancelled" => "已取消".to_string(),
            _ => {
                // v0.93.3: 用户可见的最终失败串同步写 error 级日志
                log::error!(
                    "agency: {} 任务失败 task={} message_id={} error={}",
                    task.role.as_str(),
                    task.id,
                    message_id,
                    error.unwrap_or("未知错误")
                );
                format!(
                    "{} 任务失败：{}",
                    task.role.as_str(),
                    error.unwrap_or("未知错误")
                )
            }
        };
        let repo = ChatRepository::new(self.pool.clone());
        if repo
            .update_message_payload(&message_id, status, Some(&content), None, None, error)
            .is_ok()
        {
            if let Some(app) = &self.app {
                let _ = app.emit(
                    EVENT_CHAT_UPDATE,
                    serde_json::json!({
                        "session_id": parse_meta_chat_session(&task.meta_json).unwrap_or_default(),
                        "message_id": message_id,
                        "role": task.role.as_str(),
                        "status": status,
                        "content": content,
                        "task_ids": [task.id.clone()],
                    }),
                );
            }
        }
    }
}

fn truncate_summary(text: &str) -> String {
    let s: String = text.chars().take(800).collect();
    if text.chars().count() > 800 {
        format!("{}…(已截断)", s)
    } else {
        s
    }
}

fn parse_meta_premise(meta: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(meta)
        .ok()
        .and_then(|v| {
            v.get("premise")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
}

fn parse_meta_chat_session(meta: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(meta)
        .ok()
        .and_then(|v| {
            v.get("chat_session_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
}

fn parse_meta_chat_message(meta: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(meta)
        .ok()
        .and_then(|v| {
            v.get("chat_message_id")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agency::{
            coordinator::AgencyCoordinator,
            tool_loop::{LlmTurn, LoopLlm},
        },
        db::create_test_pool,
        router::TaskType,
    };

    struct SlowFinalLlm {
        delay_ms: u64,
    }

    #[async_trait::async_trait]
    impl LoopLlm for SlowFinalLlm {
        async fn complete(
            &self,
            _s: &str,
            _u: &str,
            _t: TaskType,
            _m: i32,
        ) -> Result<String, AppError> {
            tokio::time::sleep(Duration::from_millis(self.delay_ms)).await;
            Ok(r#"{"type":"final","content":"完成"}"#.to_string())
        }

        async fn complete_turn_metered(
            &self,
            s: &str,
            u: &str,
            t: TaskType,
            m: i32,
            _tools: Option<&[crate::llm::adapter::ToolSpec]>,
        ) -> Result<(LlmTurn, i32, f64), AppError> {
            let content = self.complete(s, u, t, m).await?;
            Ok((
                LlmTurn {
                    content,
                    tool_calls: Vec::new(),
                },
                0,
                0.0,
            ))
        }
    }

    /// 每轮都返回一个不可用工具的 action：让 ToolLoop 走到最大轮数熔断，
    /// 用于验证熔断不再被调度器标成 cancelled。
    struct AlwaysToolLlm;

    #[async_trait::async_trait]
    impl LoopLlm for AlwaysToolLlm {
        async fn complete(
            &self,
            _s: &str,
            _u: &str,
            _t: TaskType,
            _m: i32,
        ) -> Result<String, AppError> {
            Ok(r#"{"type":"tool","name":"definitely_not_a_tool","args":{}}"#.to_string())
        }

        async fn complete_turn_metered(
            &self,
            s: &str,
            u: &str,
            t: TaskType,
            m: i32,
            _tools: Option<&[crate::llm::adapter::ToolSpec]>,
        ) -> Result<(LlmTurn, i32, f64), AppError> {
            let content = self.complete(s, u, t, m).await?;
            Ok((
                LlmTurn {
                    content,
                    tool_calls: Vec::new(),
                },
                0,
                0.0,
            ))
        }
    }

    fn setup(delay_ms: u64) -> TaskScheduler {
        setup_with_llm(Arc::new(SlowFinalLlm { delay_ms }))
    }

    fn setup_with_llm(llm: Arc<dyn LoopLlm>) -> TaskScheduler {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        let coordinator = Arc::new(AgencyCoordinator::for_test(pool.clone(), llm));
        TaskScheduler::new(pool, coordinator, None)
    }

    #[tokio::test]
    async fn test_director_role_is_rejected_as_subagent() {
        let sched = setup(0);
        let err = sched
            .dispatch_sub_agent(
                "story-1",
                "chat:s1",
                AgentRole::Director,
                "前提",
                "巡检",
                None,
                None,
                "{}".into(),
            )
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("Director"),
            "Director 不能进子 agent ToolLoop: {err}"
        );
    }

    #[tokio::test]
    async fn test_dispatch_returns_task_id_before_completion() {
        let sched = setup(150);
        let task_id = sched
            .dispatch_sub_agent(
                "story-1",
                "chat:s1",
                AgentRole::Producer,
                "前提",
                "补齐世界观",
                None,
                None,
                "{}".into(),
            )
            .await
            .unwrap();
        let task = TaskRepository::new(sched.pool.clone())
            .get_task(&task_id)
            .unwrap()
            .unwrap();
        assert!(matches!(task.status.as_str(), "queued" | "running"));
        tokio::time::sleep(Duration::from_millis(400)).await;
        let task = TaskRepository::new(sched.pool.clone())
            .get_task(&task_id)
            .unwrap()
            .unwrap();
        assert_eq!(task.status, "completed");
    }

    #[tokio::test]
    async fn test_dependency_task_waits_for_parent() {
        let sched = setup(120);
        let parent = sched
            .dispatch_sub_agent(
                "story-1",
                "chat:s1",
                AgentRole::Producer,
                "前提",
                "补齐资产",
                None,
                None,
                "{}".into(),
            )
            .await
            .unwrap();
        let child = sched
            .dispatch_sub_agent(
                "story-1",
                "chat:s1",
                AgentRole::EditorAuditor,
                "前提",
                "复审第12章",
                Some(parent.clone()),
                None,
                "{}".into(),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(80)).await;
        let child_task = TaskRepository::new(sched.pool.clone())
            .get_task(&child)
            .unwrap()
            .unwrap();
        assert_eq!(child_task.status, "queued");
        tokio::time::sleep(Duration::from_millis(500)).await;
        let parent_task = TaskRepository::new(sched.pool.clone())
            .get_task(&parent)
            .unwrap()
            .unwrap();
        let child_task = TaskRepository::new(sched.pool.clone())
            .get_task(&child)
            .unwrap()
            .unwrap();
        assert_eq!(parent_task.status, "completed");
        assert_eq!(child_task.status, "completed");
    }

    #[tokio::test]
    async fn test_tool_loop_breaker_is_failed_not_cancelled() {
        let sched = setup_with_llm(Arc::new(AlwaysToolLlm));
        let task_id = sched
            .dispatch_sub_agent(
                "story-1",
                "chat:s1",
                AgentRole::EditorAuditor,
                "前提",
                "复审草稿",
                None,
                None,
                "{}".into(),
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(400)).await;
        let task = TaskRepository::new(sched.pool.clone())
            .get_task(&task_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            task.status, "failed",
            "自动熔断必须落 failed，不能伪装成用户 cancelled"
        );
        assert!(
            task.error.unwrap_or_default().contains("熔断"),
            "失败原因需带熔断主因"
        );
    }
}
