#![allow(dead_code)]
//! Task Heartbeat Monitor
//!
//! 参考 memoh-X internal/heartbeat/engine.go 的心跳检测设计。
//! 每 60 秒扫描所有 running 任务，检测心跳超时。

use std::time::Duration;

use chrono::Local;
use tauri::async_runtime::JoinHandle;
use tokio::time::interval;

use super::{models::*, repository::TaskRepository};
use crate::db::DbPool;

/// 心跳检测器
pub struct HeartbeatMonitor {
    pool: DbPool,
    check_interval_secs: u64,
    handle: Option<JoinHandle<()>>,
}

impl Clone for HeartbeatMonitor {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
            check_interval_secs: self.check_interval_secs,
            handle: None, // JoinHandle 不可 clone，clone 后停止状态
        }
    }
}

impl HeartbeatMonitor {
    pub fn new(pool: DbPool) -> Self {
        Self {
            pool,
            check_interval_secs: 60, // 每60秒检测一次
            handle: None,
        }
    }

    /// 启动心跳检测循环
    pub fn start(&mut self) {
        if self.handle.is_some() {
            log::warn!("[HeartbeatMonitor] Already running");
            return;
        }

        let pool = self.pool.clone();
        let interval_secs = self.check_interval_secs;

        let handle = tauri::async_runtime::spawn(async move {
            let mut ticker = interval(Duration::from_secs(interval_secs));
            ticker.tick().await; // 首次延迟

            loop {
                ticker.tick().await;
                if let Err(e) = Self::check_all(&pool).await {
                    log::error!("[HeartbeatMonitor] Check failed: {}", e);
                }
            }
        });

        self.handle = Some(handle);
        log::info!(
            "[HeartbeatMonitor] Started (interval: {}s)",
            self.check_interval_secs
        );
    }

    /// 停止心跳检测
    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            log::info!("[HeartbeatMonitor] Stopped");
        }
    }

    /// 记录任务心跳（由任务执行器调用）
    pub fn record_heartbeat(pool: &DbPool, task_id: &str) {
        let repo = TaskRepository::new(pool.clone());
        if let Err(e) = repo.update_heartbeat(task_id) {
            log::warn!(
                "[HeartbeatMonitor] Failed to record heartbeat for {}: {}",
                task_id,
                e
            );
        }
    }

    /// 检测所有 running 任务
    async fn check_all(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
        let repo = TaskRepository::new(pool.clone());
        let running_tasks = repo.list_running()?;

        let now = Local::now();

        for task in running_tasks {
            let timeout_secs = task.heartbeat_timeout_seconds as i64;

            let is_timeout = match &task.last_heartbeat_at {
                Some(heartbeat_str) => {
                    match chrono::DateTime::parse_from_rfc3339(heartbeat_str) {
                        Ok(heartbeat) => {
                            let elapsed =
                                now.signed_duration_since(heartbeat.with_timezone(&chrono::Local));
                            elapsed.num_seconds() > timeout_secs
                        }
                        Err(e) => {
                            log::warn!(
                                "[HeartbeatMonitor] Failed to parse heartbeat time for {}: {}",
                                task.id,
                                e
                            );
                            true // 解析失败视为超时
                        }
                    }
                }
                None => {
                    // 没有心跳记录，检查任务开始运行时间
                    match &task.last_run_at {
                        Some(run_str) => match chrono::DateTime::parse_from_rfc3339(run_str) {
                            Ok(run_time) => {
                                let elapsed = now
                                    .signed_duration_since(run_time.with_timezone(&chrono::Local));
                                elapsed.num_seconds() > timeout_secs
                            }
                            Err(_) => true,
                        },
                        None => true, // 既没有心跳也没有运行时间，视为异常
                    }
                }
            };

            if is_timeout {
                log::warn!(
                    "[HeartbeatMonitor] Task {} heartbeat timeout (last: {:?}), marking as failed",
                    task.id,
                    task.last_heartbeat_at
                );

                // 心跳超时 = 执行器已失去响应。以前写回 Pending 并安排
                // next_run_at，但调度器根本不读 next_run_at：任务会永久卡在
                // pending 且 executor 随后还会把它覆盖成 completed。现在只用
                // 同一个 running 守卫写 failed，保留最后进度；重试由用户显式
                // 触发（trigger → pending → running），不存在幽灵重试。
                let message = format!(
                    "心跳超时：任务失去响应超过 {} 秒，已标记为失败（需手动重试）",
                    timeout_secs
                );
                let applied = repo.fail_if_running_preserving_progress(&task.id, &message)?;
                if applied {
                    repo.create_log(
                        &task.id,
                        "error",
                        &format!(
                            "心跳超时检测：任务失去响应超过 {} 秒，自动标记为失败。",
                            timeout_secs
                        ),
                    )?;
                } else {
                    log::warn!(
                        "[HeartbeatMonitor] Task {} 已离开 running，跳过心跳失败写入（可能已取消）",
                        task.id
                    );
                }
            }
        }

        Ok(())
    }
}
