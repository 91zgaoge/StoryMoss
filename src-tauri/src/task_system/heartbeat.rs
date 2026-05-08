//! Task Heartbeat Monitor
//!
//! 参考 memoh-X internal/heartbeat/engine.go 的心跳检测设计。
//! 每 60 秒扫描所有 running 任务，检测心跳超时。

use super::models::*;
use super::repository::TaskRepository;
use crate::db::DbPool;
use chrono::Local;
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tokio::time::interval;

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
        log::info!("[HeartbeatMonitor] Started (interval: {}s)", self.check_interval_secs);
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
            log::warn!("[HeartbeatMonitor] Failed to record heartbeat for {}: {}", task_id, e);
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
                            let elapsed = now.signed_duration_since(heartbeat.with_timezone(&chrono::Local));
                            elapsed.num_seconds() > timeout_secs
                        }
                        Err(e) => {
                            log::warn!("[HeartbeatMonitor] Failed to parse heartbeat time for {}: {}", task.id, e);
                            true // 解析失败视为超时
                        }
                    }
                }
                None => {
                    // 没有心跳记录，检查任务开始运行时间
                    match &task.last_run_at {
                        Some(run_str) => {
                            match chrono::DateTime::parse_from_rfc3339(run_str) {
                                Ok(run_time) => {
                                    let elapsed = now.signed_duration_since(run_time.with_timezone(&chrono::Local));
                                    elapsed.num_seconds() > timeout_secs
                                }
                                Err(_) => true,
                            }
                        }
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

                // P0-6 修复: 超时后若可重试，将状态改回 Pending 并安排重试
                if task.retry_count < task.max_retries {
                    let new_retry = task.retry_count + 1;
                    // 指数退避: 每次重试等待 30 * 2^(retry_count) 秒
                    let backoff_secs = 30u64 * (2u64.pow(new_retry as u32));
                    let next_run = (Local::now() + chrono::Duration::seconds(backoff_secs as i64)).to_rfc3339();
                    
                    repo.update_status(
                        &task.id,
                        &TaskStatus::Pending,
                        Some(task.progress),
                        None,
                        Some(format!("心跳超时，准备重试 ({}/{})", new_retry, task.max_retries)),
                    )?;
                    repo.update_next_run_at(&task.id, Some(&next_run))?;
                    repo.increment_retry(&task.id)?;
                    
                    log::info!(
                        "[HeartbeatMonitor] Task {} rescheduled for retry ({}/{}), backoff={}s",
                        task.id, new_retry, task.max_retries, backoff_secs
                    );
                    repo.create_log(
                        &task.id,
                        "info",
                        &format!("心跳超时，已安排重试 ({}/{}), 退避 {} 秒", new_retry, task.max_retries, backoff_secs),
                    )?;
                } else {
                    // 超过最大重试次数，标记为失败
                    repo.update_status(
                        &task.id,
                        &TaskStatus::Failed,
                        Some(task.progress),
                        None,
                        Some("心跳超时：任务执行过程中失去响应，且已超过最大重试次数".to_string()),
                    )?;
                    repo.create_log(
                        &task.id,
                        "error",
                        &format!(
                            "心跳超时检测：任务失去响应超过 {} 秒，自动标记为失败（重试已耗尽）。",
                            timeout_secs
                        ),
                    )?;
                }
            }
        }

        Ok(())
    }
}
