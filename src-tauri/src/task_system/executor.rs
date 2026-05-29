//! Task Executor
//!
//! 任务执行trait + 具体执行器实现。
//! 参考 memoh-X internal/schedule/trigger.go + internal/subagent/service.go
//! 设计。

use std::sync::Arc;

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

    /// 开始执行：更新状态为 running
    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let repo = TaskRepository::new(self.pool.clone());
        repo.update_status(&self.task_id, &TaskStatus::Running, Some(0), None, None)?;
        repo.update_last_run(&self.task_id)?;
        repo.create_log(&self.task_id, "info", "任务开始执行")?;
        self.emit_status_changed("running", 0, Some("任务开始执行".to_string()));
        Ok(())
    }

    /// 记录心跳
    pub fn heartbeat(&self) {
        let repo = TaskRepository::new(self.pool.clone());
        if let Err(e) = repo.update_heartbeat(&self.task_id) {
            log::warn!("[TaskExecution] Failed to record heartbeat: {}", e);
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
        if let Err(e) = repo.update_status(
            &self.task_id,
            &TaskStatus::Running,
            Some(progress),
            None,
            None,
        ) {
            log::warn!("[TaskExecution] Failed to update progress: {}", e);
        }

        let event = TaskProgressEvent {
            task_id: self.task_id.clone(),
            step: step.to_string(),
            progress,
            message: message.to_string(),
        };
        let _ = self.app_handle.emit("task-progress", &event);
    }

    /// 记录日志
    pub fn log(&self, level: &str, message: &str) {
        let repo = TaskRepository::new(self.pool.clone());
        if let Err(e) = repo.create_log(&self.task_id, level, message) {
            log::warn!("[TaskExecution] Failed to create log: {}", e);
        }
    }

    /// 完成任务
    pub fn complete(&self, result_json: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
        let repo = TaskRepository::new(self.pool.clone());
        repo.reset_retry(&self.task_id)?;
        repo.update_status(
            &self.task_id,
            &TaskStatus::Completed,
            Some(100),
            result_json.clone(),
            None,
        )?;
        repo.create_log(&self.task_id, "info", "任务执行完成")?;
        self.emit_status_changed("completed", 100, Some("任务执行完成".to_string()));
        Ok(())
    }

    /// 标记失败
    pub fn fail(&self, error: &str) -> Result<(), Box<dyn std::error::Error>> {
        let repo = TaskRepository::new(self.pool.clone());
        repo.update_status(
            &self.task_id,
            &TaskStatus::Failed,
            None,
            None,
            Some(error.to_string()),
        )?;
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
        };
        let _ = self.app_handle.emit("task-status-changed", &event);
    }
}
