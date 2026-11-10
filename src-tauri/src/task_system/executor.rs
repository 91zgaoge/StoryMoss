//! Task Executor
//!
//! 任务执行trait + 具体执行器实现。
//! 参考 memoh-X internal/schedule/trigger.go + internal/subagent/service.go
//! 设计。

use std::sync::Arc;

use chrono::Local;
use tauri::{Emitter, Runtime};

use super::{models::*, repository::TaskRepository};
use crate::db::DbPool;

/// 任务执行器 trait
#[async_trait::async_trait]
pub trait TaskExecutor: Send + Sync {
    /// 执行任务
    async fn execute(&self, task: &Task) -> Result<TaskResult, Box<dyn std::error::Error>>;

    /// 是否可以处理该任务类型
    fn can_handle(&self, task_type: &TaskType) -> bool;
}

/// 任务执行器注册表
pub struct ExecutorRegistry {
    executors: Vec<Arc<dyn TaskExecutor>>,
}

impl ExecutorRegistry {
    pub fn new() -> Self {
        Self {
            executors: Vec::new(),
        }
    }

    pub fn register(&mut self, executor: Arc<dyn TaskExecutor>) {
        self.executors.push(executor);
    }

    pub fn find_executor(&self, task_type: &TaskType) -> Option<Arc<dyn TaskExecutor>> {
        self.executors
            .iter()
            .find(|e| e.can_handle(task_type))
            .cloned()
    }
}

impl Default for ExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 通用的任务执行包装器
/// 负责：状态流转、心跳记录、日志记录、进度推送
pub struct TaskExecutionContext<R: Runtime = tauri::Wry> {
    pub task_id: String,
    pub pool: DbPool,
    pub app_handle: tauri::AppHandle<R>,
    progress: std::sync::Arc<std::sync::atomic::AtomicI32>,
}

impl<R: Runtime> TaskExecutionContext<R> {
    pub fn new(task_id: String, pool: DbPool, app_handle: tauri::AppHandle<R>) -> Self {
        Self {
            task_id,
            pool,
            app_handle,
            progress: std::sync::Arc::new(std::sync::atomic::AtomicI32::new(0)),
        }
    }

    pub fn get_progress(&self) -> i32 {
        self.progress.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 开始执行：只有 pending/新一轮的 completed/failed 可以进入 running。
    /// cancelled 不会被执行器复活（重新执行必须先走显式 trigger）。
    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let repo = TaskRepository::new(self.pool.clone());
        if !repo.start_execution(&self.task_id)? {
            return Err(format!("任务 {} 已取消或已在运行，拒绝开始执行", self.task_id).into());
        }
        repo.update_last_run(&self.task_id)?;
        repo.create_log(&self.task_id, "info", "任务开始执行")?;
        self.emit_status_changed("running", 0, Some("任务开始执行".to_string()));
        Ok(())
    }

    /// 记录心跳。任务已离开 running（取消/终态）后不再续命。
    pub fn heartbeat(&self) {
        let repo = TaskRepository::new(self.pool.clone());
        match repo.update_heartbeat_if_running(&self.task_id) {
            Ok(true) => {}
            Ok(false) => {
                log::debug!(
                    "[TaskExecution] Heartbeat skipped: task {} is no longer running",
                    self.task_id
                );
                return;
            }
            Err(e) => {
                log::warn!("[TaskExecution] Failed to record heartbeat: {}", e);
                return;
            }
        }
        // 发送心跳事件给前端
        let event = TaskHeartbeatEvent {
            task_id: self.task_id.clone(),
            timestamp: chrono::Local::now().to_rfc3339(),
        };
        let _ = self.app_handle.emit("task-heartbeat", &event);
    }

    /// 更新进度
    pub fn update_progress(&self, step: &str, progress: i32, message: &str) {
        self.progress
            .store(progress, std::sync::atomic::Ordering::Relaxed);
        let repo = TaskRepository::new(self.pool.clone());
        // v0.93.14: 进度、日志、心跳共用同一个 now（毫秒级一致），
        // 修「任务进度时间与日志时间不一致」的循环复现口径。
        let now = Local::now().to_rfc3339();
        match repo.record_progress_if_running(&self.task_id, progress, message, &now) {
            Ok(true) => {}
            Ok(false) => {
                log::debug!(
                    "[TaskExecution] Progress skipped: task {} is no longer running",
                    self.task_id
                );
                return;
            }
            Err(e) => {
                log::warn!("[TaskExecution] Failed to update progress: {}", e);
                return;
            }
        }

        let event = TaskProgressEvent {
            task_id: self.task_id.clone(),
            step: step.to_string(),
            progress,
            message: message.to_string(),
            timestamp: now,
        };
        let _ = self.app_handle.emit("task-progress", &event);
    }

    /// 记录日志
    pub fn log(&self, level: &str, message: &str) {
        let repo = TaskRepository::new(self.pool.clone());
        if let Err(e) = repo.create_log(&self.task_id, level, message) {
            log::warn!("[TaskExecution] Failed to create log: {}", e);
        }
        // v0.93.14: 日志也是存活信号——同步刷新心跳，避免「日志刚写、
        // 心跳却显示几分钟前」的错位（日志时间仍以 create_log 为准）。
        let _ = repo.update_heartbeat_if_running(&self.task_id);
    }

    /// 完成任务。终态任务（被取消/已失败）不会被迟到的结果覆盖。
    pub fn complete(&self, result_json: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
        let repo = TaskRepository::new(self.pool.clone());
        if !repo.complete_if_running(&self.task_id, result_json)? {
            return Err(format!(
                "任务 {} 已不在 running，拒绝写入 completed（可能已取消）",
                self.task_id
            )
            .into());
        }
        repo.create_log(&self.task_id, "info", "任务执行完成")?;
        self.emit_status_changed("completed", 100, Some("任务执行完成".to_string()));
        Ok(())
    }

    /// 标记失败。只允许从 running 写入，避免取消后迟到的失败覆盖终态。
    pub fn fail(&self, error: &str) -> Result<(), Box<dyn std::error::Error>> {
        let repo = TaskRepository::new(self.pool.clone());
        if !repo.fail_if_running(&self.task_id, error)? {
            return Err(format!(
                "任务 {} 已不在 running，拒绝写入 failed（可能已取消）",
                self.task_id
            )
            .into());
        }
        repo.create_log(&self.task_id, "error", &format!("任务执行失败: {}", error))?;
        self.emit_status_changed("failed", 0, Some(error.to_string()));
        Ok(())
    }

    /// 检查任务是否被取消
    pub fn is_cancelled(&self) -> bool {
        let repo = TaskRepository::new(self.pool.clone());
        match repo.get_by_id(&self.task_id) {
            Ok(Some(task)) => task.status == TaskStatus::Cancelled,
            _ => false,
        }
    }

    fn emit_status_changed(&self, status: &str, progress: i32, message: Option<String>) {
        let event = TaskStatusChangedEvent {
            task_id: self.task_id.clone(),
            status: status.to_string(),
            progress,
            message,
            timestamp: Local::now().to_rfc3339(),
        };
        let _ = self.app_handle.emit("task-status-changed", &event);
    }
}

/// 取消监听句柄：Drop 时自动 abort 监听协程。
///
/// 执行器把返回值绑定在 `execute` 作用域内即可——无论正常返回、`?` 提前
/// 返回还是墙钟超时 future 被 drop，监听都会随之停止，不会泄漏轮询协程
/// （v0.93.2 修复：续写训练旧实现从不 abort，每轮训练泄漏一个 5s 轮询）。
pub struct CancelMonitor {
    handle: tauri::async_runtime::JoinHandle<()>,
}

impl Drop for CancelMonitor {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// 桥接任务系统取消到执行器内部取消机制。
///
/// 每 500ms 轮询任务状态（与 [`TaskExecutionContext::is_cancelled`] 同一
/// 查询口径），发现 cancelled 时调用一次 `on_cancelled` 并退出轮询。三个
/// 执行器（续写训练/拆书/指导书提炼）共用此实现，替代各自独立 spawn 的
/// 轮询循环。
pub fn spawn_cancel_monitor<F>(task_id: String, pool: DbPool, on_cancelled: F) -> CancelMonitor
where
    F: Fn() + Send + 'static,
{
    let handle = tauri::async_runtime::spawn(async move {
        let repo = TaskRepository::new(pool);
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let cancelled = repo
                .get_by_id(&task_id)
                .ok()
                .flatten()
                .is_some_and(|t| t.status == TaskStatus::Cancelled);
            if cancelled {
                on_cancelled();
                break;
            }
        }
    });
    CancelMonitor { handle }
}
