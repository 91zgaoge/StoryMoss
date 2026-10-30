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
        task_repository::TaskRepository,
    },
    db::DbPool,
    error::AppError,
};

pub const EVENT_CHAT_UPDATE: &str = "agency-chat-update";
pub const STORY_TASK_CONCURRENCY: usize = 4;

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
        tokio::spawn(async move {
            sched.run_task(task, premise, depends_on_task_id).await;
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
            .ok_or_else(|| AppError::from(format!("任务不存在: {task_id}")))?;
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
        let (status, summary, error) = match result {
            Ok(r) if r.aborted || cancelled => ("cancelled", Some("已取消".to_string()), None),
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

        self.notify_waiters(&task.id);
        unregister_task_cancel(&task.id);
        if status == "completed" {
            self.append_chat_follow_up(&task, summary.as_deref());
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

    fn setup(delay_ms: u64) -> TaskScheduler {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        let coordinator = Arc::new(AgencyCoordinator::for_test(
            pool.clone(),
            Arc::new(SlowFinalLlm { delay_ms }),
        ));
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
}
