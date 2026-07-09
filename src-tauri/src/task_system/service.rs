#![allow(dead_code)]
//! Task Service
//!
//! 业务服务层：整合 Repository + Scheduler + Heartbeat + ExecutorRegistry
//! 参考 memoh-X internal/schedule/service.go 设计。

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Runtime};

use super::{
    executor::{ExecutorRegistry, TaskExecutionContext},
    heartbeat::HeartbeatMonitor,
    models::*,
    repository::TaskRepository,
};
use crate::{db::DbPool, error::AppError, state_sync::service::StateSync};

pub struct TaskService<R: Runtime = tauri::Wry> {
    pool: DbPool,
    app_handle: AppHandle<R>,
    scheduler: Arc<TaskScheduler>,
    heartbeat: Arc<std::sync::Mutex<HeartbeatMonitor>>,
    executors: Arc<std::sync::Mutex<ExecutorRegistry>>,
}

impl<R: Runtime> Clone for TaskService<R> {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            app_handle: self.app_handle.clone(),
            scheduler: self.scheduler.clone(),
            heartbeat: self.heartbeat.clone(),
            executors: self.executors.clone(),
        }
    }
}

impl<R: Runtime> TaskService<R> {
    pub fn new(pool: DbPool, app_handle: AppHandle<R>) -> Self {
        let scheduler = Arc::new(TaskScheduler::new());
        let heartbeat = Arc::new(std::sync::Mutex::new(HeartbeatMonitor::new(pool.clone())));
        let executors = Arc::new(std::sync::Mutex::new(ExecutorRegistry::new()));

        Self {
            pool,
            app_handle,
            scheduler,
            heartbeat,
            executors,
        }
    }

    /// 注册执行器
    pub fn register_executor(&self, executor: Arc<dyn super::executor::TaskExecutor>) {
        let mut executors = self.executors.lock().unwrap();
        executors.register(executor);
    }

    /// 启动服务：加载所有启用的定时任务，启动心跳检测
    pub fn bootstrap(&self) -> Result<(), Box<dyn std::error::Error>> {
        let repo = TaskRepository::new(self.pool.clone());
        let tasks = repo.list_enabled_scheduled()?;

        let mut registered = 0;
        for task in tasks {
            if let Err(e) = self.register_scheduled_task(&task) {
                log::error!(
                    "[TaskService] Failed to register scheduled task {}: {}",
                    task.id,
                    e
                );
            } else {
                registered += 1;
            }
        }

        // 启动心跳检测
        {
            let mut heartbeat = self.heartbeat.lock().unwrap();
            heartbeat.start();
        }

        log::info!(
            "[TaskService] Bootstrapped: {} scheduled tasks registered, heartbeat monitor started",
            registered
        );

        Ok(())
    }

    /// 关闭服务
    pub fn shutdown(&self) {
        self.scheduler.stop_all();
        {
            let mut heartbeat = self.heartbeat.lock().unwrap();
            heartbeat.stop();
        }
        log::info!("[TaskService] Shutdown complete");
    }

    // ==================== CRUD ====================

    pub fn create_task(&self, req: CreateTaskRequest) -> Result<Task, AppError> {
        let repo = TaskRepository::new(self.pool.clone());
        let task = repo.create(&req).map_err(AppError::from)?;

        // 如果启用了且不是一次性任务，注册到调度器
        if task.enabled && task.schedule_type != ScheduleType::Once {
            if let Err(e) = self.register_scheduled_task(&task) {
                log::error!("[TaskService] Failed to register new scheduled task: {}", e);
            }
        }

        // 如果是一次性任务且启用，立即触发
        if task.enabled
            && task.schedule_type == ScheduleType::Once
            && task.status == TaskStatus::Pending
        {
            let task_id = task.id.clone();
            let pool = self.pool.clone();
            let app_handle = self.app_handle.clone();
            let executors = self.executors.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = Self::run_task_internal(&task_id, pool, app_handle, executors).await
                {
                    log::error!("[TaskService] Failed to run once task {}: {}", task_id, e);
                }
            });
        }

        // 发送事件
        let _ = self.app_handle.emit(
            "task-created",
            &serde_json::json!({
                "task_id": &task.id,
                "name": &task.name,
            }),
        );
        StateSync::emit_task_created(&self.app_handle, &task.id, &task.name);

        Ok(task)
    }

    pub fn update_task(&self, id: &str, req: UpdateTaskRequest) -> Result<Task, AppError> {
        let repo = TaskRepository::new(self.pool.clone());
        let old_task = repo
            .get_by_id(id)
            .map_err(AppError::from)?
            .ok_or_else(|| "Task not found".to_string())?;

        let was_scheduled = old_task.enabled && old_task.schedule_type != ScheduleType::Once;

        let task = repo.update(id, &req).map_err(AppError::from)?;

        let is_scheduled = task.enabled && task.schedule_type != ScheduleType::Once;

        // 重新注册定时任务
        if was_scheduled || is_scheduled {
            self.scheduler.unregister(id);
        }
        if is_scheduled {
            if let Err(e) = self.register_scheduled_task(&task) {
                log::error!("[TaskService] Failed to re-register scheduled task: {}", e);
            }
        }

        StateSync::emit_task_updated(&self.app_handle, &task.id, &task.status.to_string());

        Ok(task)
    }

    pub fn delete_task(&self, id: &str) -> Result<(), AppError> {
        self.scheduler.unregister(id);
        let repo = TaskRepository::new(self.pool.clone());
        repo.delete(id).map_err(AppError::from)?;
        Ok(())
    }

    pub fn list_tasks(&self, status_filter: Option<String>) -> Result<Vec<Task>, AppError> {
        let repo = TaskRepository::new(self.pool.clone());
        let filter = status_filter.as_deref();
        repo.list(filter, None).map_err(AppError::from)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<Task>, AppError> {
        let repo = TaskRepository::new(self.pool.clone());
        repo.get_by_id(id).map_err(AppError::from)
    }

    pub fn get_task_logs(&self, task_id: &str) -> Result<Vec<TaskLog>, AppError> {
        let repo = TaskRepository::new(self.pool.clone());
        repo.list_logs(task_id).map_err(AppError::from)
    }

    // ==================== Execution ====================

    /// 手动触发任务执行
    pub fn trigger_task(&self, id: &str) -> Result<(), AppError> {
        let repo = TaskRepository::new(self.pool.clone());
        let task = repo
            .get_by_id(id)
            .map_err(AppError::from)?
            .ok_or_else(|| "Task not found".to_string())?;

        if task.status == TaskStatus::Running {
            return Err(AppError::internal("Task is already running"));
        }

        let task_id = id.to_string();
        let pool = self.pool.clone();
        let app_handle = self.app_handle.clone();
        let executors = self.executors.clone();

        tauri::async_runtime::spawn(async move {
            if let Err(e) = Self::run_task_internal(&task_id, pool, app_handle, executors).await {
                log::error!("[TaskService] Manual trigger failed for {}: {}", task_id, e);
            }
        });

        Ok(())
    }

    /// 取消任务
    pub fn cancel_task(&self, id: &str) -> Result<(), AppError> {
        let repo = TaskRepository::new(self.pool.clone());
        let task = repo
            .get_by_id(id)
            .map_err(AppError::from)?
            .ok_or_else(|| "Task not found".to_string())?;

        if task.status != TaskStatus::Running {
            return Err(AppError::internal("Task is not running"));
        }

        repo.update_status(
            id,
            &TaskStatus::Cancelled,
            None,
            None,
            Some("用户手动取消".to_string()),
        )
        .map_err(AppError::from)?;

        repo.create_log(id, "warn", "任务被用户手动取消")
            .map_err(AppError::from)?;

        let event = TaskStatusChangedEvent {
            task_id: id.to_string(),
            status: "cancelled".to_string(),
            progress: task.progress,
            message: Some("任务已取消".to_string()),
        };
        let _ = self.app_handle.emit("task-status-changed", &event);
        StateSync::emit_task_updated(&self.app_handle, id, "cancelled");

        Ok(())
    }

    // ==================== Internal ====================

    /// 注册定时任务到调度器
    fn register_scheduled_task(&self, task: &Task) -> Result<(), Box<dyn std::error::Error>> {
        let task_id = task.id.clone();
        let pool = self.pool.clone();
        let app_handle = self.app_handle.clone();
        let executors = self.executors.clone();

        self.scheduler.register(task, move || {
            let tid = task_id.clone();
            let p = pool.clone();
            let ah = app_handle.clone();
            let ex = executors.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = Self::run_task_internal(&tid, p, ah, ex).await {
                    log::error!(
                        "[TaskService] Scheduled task execution failed for {}: {}",
                        tid,
                        e
                    );
                }
            });
        })?;

        Ok(())
    }

    /// 内部执行任务
    async fn run_task_internal(
        task_id: &str,
        pool: DbPool,
        app_handle: AppHandle<R>,
        executors: Arc<std::sync::Mutex<ExecutorRegistry>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo = TaskRepository::new(pool.clone());

        let task = repo
            .get_by_id(task_id)?
            .ok_or_else(|| format!("Task {} not found", task_id))?;

        // 获取执行器
        let executor = {
            let registry = executors.lock().unwrap();
            registry.find_executor(&task.task_type)
        };

        let executor = match executor {
            Some(e) => e,
            None => {
                let err_msg = format!(
                    "No executor found for task type: {}",
                    task.task_type.to_string()
                );
                repo.update_status(
                    task_id,
                    &TaskStatus::Failed,
                    None,
                    None,
                    Some(err_msg.clone()),
                )?;
                repo.create_log(task_id, "error", &err_msg)?;
                return Err(err_msg.into());
            }
        };

        // 获取执行锁（防止重叠执行）
        let scheduler = TaskScheduler::new();
        let lock = scheduler.ensure_lock(task_id);
        let _guard = lock.lock().await;

        // 创建执行上下文
        let ctx = TaskExecutionContext::new(task_id.to_string(), pool.clone(), app_handle.clone());

        // 开始执行
        if let Err(e) = ctx.start() {
            log::error!("[TaskService] Failed to start task {}: {}", task_id, e);
            return Err(e);
        }

        // 墙钟超时与心跳超时分离：
        // - 心跳超时（HeartbeatMonitor）= 无进度多久杀任务
        // - 墙钟超时 = 整任务最长允许跑多久
        // 拆书是多块串行/并行 LLM，总时长可达数小时；若把 heartbeat 当墙钟，
        // 中长篇必在 300s 被掐死。其它任务保持「heartbeat 兼墙钟」旧语义。
        let timeout_secs = wall_clock_timeout_secs(&task);
        let result = match tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            executor.execute(&task),
        )
        .await
        {
            Ok(res) => res,
            Err(_) => {
                let err_msg = format!("任务执行超时 ({} 秒)", timeout_secs);
                log::error!("[TaskService] {}", err_msg);
                if let Err(e2) = ctx.fail(&err_msg) {
                    log::error!(
                        "[TaskService] Failed to mark task {} as failed: {}",
                        task_id,
                        e2
                    );
                }
                return Err(err_msg.into());
            }
        };

        match result {
            Ok(task_result) => {
                if task_result.success {
                    if let Err(e) = ctx.complete(task_result.result_json) {
                        log::error!("[TaskService] Failed to complete task {}: {}", task_id, e);
                    }
                    StateSync::emit_task_completed(&app_handle, task_id, true);
                } else {
                    let err = task_result
                        .error_message
                        .unwrap_or_else(|| "Unknown error".to_string());
                    if let Err(e) = ctx.fail(&err) {
                        log::error!(
                            "[TaskService] Failed to mark task {} as failed: {}",
                            task_id,
                            e
                        );
                    }
                    StateSync::emit_task_completed(&app_handle, task_id, false);
                }
            }
            Err(e) => {
                let err_msg = format!("Execution error: {}", e);
                if let Err(e2) = ctx.fail(&err_msg) {
                    log::error!(
                        "[TaskService] Failed to mark task {} as failed: {}",
                        task_id,
                        e2
                    );
                }
                StateSync::emit_task_completed(&app_handle, task_id, false);
            }
        }

        Ok(())
    }
}

/// 拆书墙钟上限：12 小时（心跳仍由 HeartbeatMonitor 按 heartbeat_timeout
/// 杀僵死任务）
pub(crate) const BOOK_DECONSTRUCTION_WALL_CLOCK_SECS: u64 = 12 * 60 * 60;

/// 计算任务墙钟超时秒数。
pub(crate) fn wall_clock_timeout_secs(task: &Task) -> u64 {
    match task.task_type {
        TaskType::BookDeconstruction => BOOK_DECONSTRUCTION_WALL_CLOCK_SECS,
        _ => task.heartbeat_timeout_seconds.max(60) as u64,
    }
}

use super::scheduler::TaskScheduler;

#[cfg(test)]
mod wall_clock_tests {
    use chrono::Local;

    use super::*;

    fn sample_task(task_type: TaskType, heartbeat: i32) -> Task {
        Task {
            id: "t1".into(),
            name: "test".into(),
            description: None,
            task_type,
            schedule_type: ScheduleType::Once,
            cron_pattern: None,
            payload: None,
            status: TaskStatus::Pending,
            progress: 0,
            result: None,
            error_message: None,
            max_retries: 0,
            retry_count: 0,
            enabled: true,
            last_run_at: None,
            next_run_at: None,
            last_heartbeat_at: None,
            heartbeat_timeout_seconds: heartbeat,
            created_at: Local::now().to_rfc3339(),
            updated_at: Local::now().to_rfc3339(),
        }
    }

    #[test]
    fn book_deconstruction_wall_clock_is_twelve_hours() {
        let t = sample_task(TaskType::BookDeconstruction, 600);
        assert_eq!(
            wall_clock_timeout_secs(&t),
            BOOK_DECONSTRUCTION_WALL_CLOCK_SECS
        );
    }

    #[test]
    fn other_tasks_use_heartbeat_as_wall_clock() {
        let t = sample_task(TaskType::Custom, 300);
        assert_eq!(wall_clock_timeout_secs(&t), 300);
        let t2 = sample_task(TaskType::AiGeneration, 30);
        assert_eq!(wall_clock_timeout_secs(&t2), 60); // floor at 60
    }
}
