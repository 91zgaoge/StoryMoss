#![allow(dead_code)]
//! Task Scheduler
//!
//! 基于 tokio::time 的任务调度器，参考 memoh-X CronPool 设计。
//! 引入 cron crate 支持标准 Cron 表达式

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tauri::async_runtime::JoinHandle;
use tokio::time::{interval, sleep, Duration};

use super::models::*;

/// 共享任务调度器
pub struct TaskScheduler {
    /// 任务ID -> 定时器句柄
    handles: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    /// 任务ID -> 互斥锁（防止重叠执行）
    locks: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>>,
}

impl TaskScheduler {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(Mutex::new(HashMap::new())),
            locks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册一个定时任务
    ///
    /// - once 任务：立即执行（不注册定时器，由调用方直接触发）
    /// - daily/weekly/cron 任务：注册 tokio 定时器
    pub fn register<F>(&self, task: &Task, callback: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn() + Send + 'static,
    {
        let task_id = task.id.clone();

        // 先注销已存在的定时器（只 abort timer，不重造执行锁）
        self.unregister(&task_id);

        // 复用既有执行锁：update_task/toggle 会反复 register，如果每次都换
        // 一把新锁，在途执行仍持有旧 Arc，第二次触发就能绕过互斥并发跑。
        let _lock = self.ensure_lock(&task_id);

        match task.schedule_type {
            ScheduleType::Once => {
                // 一次性任务不注册定时器，由调用方直接触发
                log::info!("[TaskScheduler] Registered once task: {}", task_id);
            }
            ScheduleType::Daily => {
                let handle =
                    self.spawn_interval(task_id.clone(), Duration::from_secs(86400), callback);
                {
                    let mut handles = self.handles.lock().unwrap();
                    handles.insert(task_id.clone(), handle);
                }
                log::info!("[TaskScheduler] Registered daily task: {}", task_id);
            }
            ScheduleType::Weekly => {
                let handle =
                    self.spawn_interval(task_id.clone(), Duration::from_secs(604800), callback);
                {
                    let mut handles = self.handles.lock().unwrap();
                    handles.insert(task_id.clone(), handle);
                }
                log::info!("[TaskScheduler] Registered weekly task: {}", task_id);
            }
            ScheduleType::Cron => {
                // P1-13 修复: 使用 cron crate 解析标准 Cron
                // 表达式，精确计算下次执行时间
                let schedule = Self::parse_cron_schedule(task.cron_pattern.as_deref())?;
                let handle = self.spawn_cron(task_id.clone(), schedule, callback);
                {
                    let mut handles = self.handles.lock().unwrap();
                    handles.insert(task_id.clone(), handle);
                }
                log::info!("[TaskScheduler] Registered cron task: {}", task_id);
            }
        }

        Ok(())
    }

    /// 注销任务定时器
    pub fn unregister(&self, task_id: &str) {
        let mut handles = self.handles.lock().unwrap();
        if let Some(handle) = handles.remove(task_id) {
            handle.abort();
            log::info!("[TaskScheduler] Unregistered task: {}", task_id);
        }

        // 执行锁必须随任务生命周期保留：unregister 只停 timer，若这里 remove
        // 锁，register 会新建一把，旧 guard 仍在旧 Arc 上，互斥语义失效。
    }

    /// 检查任务是否已注册
    pub fn is_registered(&self, task_id: &str) -> bool {
        let handles = self.handles.lock().unwrap();
        handles.contains_key(task_id)
    }

    /// 获取任务的执行锁（用于立即执行时防止重叠）
    pub fn get_lock(&self, task_id: &str) -> Option<Arc<tokio::sync::Mutex<()>>> {
        let locks = self.locks.lock().unwrap();
        locks.get(task_id).cloned()
    }

    /// 创建或获取任务的执行锁
    pub fn ensure_lock(&self, task_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.locks.lock().unwrap();
        locks
            .entry(task_id.to_string())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    /// 停止所有定时器
    pub fn stop_all(&self) {
        let mut handles = self.handles.lock().unwrap();
        for (id, handle) in handles.drain() {
            handle.abort();
            log::info!("[TaskScheduler] Stopped task: {}", id);
        }
        let mut locks = self.locks.lock().unwrap();
        locks.clear();
    }

    // ==================== Internal ====================

    fn spawn_interval<F>(&self, task_id: String, duration: Duration, callback: F) -> JoinHandle<()>
    where
        F: Fn() + Send + 'static,
    {
        tauri::async_runtime::spawn(async move {
            let mut ticker = interval(duration);
            // 第一次延迟执行，避免启动时立即触发
            ticker.tick().await;

            loop {
                ticker.tick().await;

                // 重叠互斥统一由 run_task_internal 的共享执行锁处理
                // （手动/定时/once 全路径互斥；此处不再前置 try_lock）
                log::info!("[TaskScheduler] Triggering scheduled task: {}", task_id);
                callback();
            }
        })
    }

    /// 使用 cron crate 解析标准 Cron 表达式
    ///
    /// v0.99.28: 前端（训练页/任务页）使用标准 5 字段 `分 时 日 月 周`
    /// （如 `0 9 * * *`），而 `cron` crate 要求 6/7 字段（多一个秒字段）。
    /// 旧实现直接 `parse()`，导致 UI 创建的定时任务全部注册失败——真机
    /// 日志 `Invalid cron pattern '0 9 * * *'`，任务保存成功但从未执行。
    fn parse_cron_schedule(
        pattern: Option<&str>,
    ) -> Result<cron::Schedule, Box<dyn std::error::Error>> {
        let pattern = pattern.ok_or("Cron pattern is required for cron schedule type")?;
        let normalized = normalize_cron_pattern(pattern);
        let schedule: cron::Schedule = normalized
            .parse()
            .map_err(|e| format!("Invalid cron pattern '{}': {:?}", pattern, e))?;
        Ok(schedule)
    }

    /// Cron 调度：精确计算下次执行时间，而非固定间隔
    fn spawn_cron<F>(
        &self,
        task_id: String,
        schedule: cron::Schedule,
        callback: F,
    ) -> JoinHandle<()>
    where
        F: Fn() + Send + 'static,
    {
        tauri::async_runtime::spawn(async move {
            // 获取 upcoming 时间点，跳过已过期的时间
            let mut upcoming = schedule.upcoming(chrono::Utc);
            while let Some(next) = upcoming.next() {
                let now = chrono::Utc::now();
                if next > now {
                    let sleep_duration = (next - now).to_std().unwrap_or(Duration::from_secs(60));
                    sleep(sleep_duration).await;

                    // 重叠互斥统一由 run_task_internal 的共享执行锁处理
                    log::info!("[TaskScheduler] Triggering cron task: {}", task_id);
                    callback();
                }
            }
            log::warn!("[TaskScheduler] Cron task {} schedule exhausted", task_id);
        })
    }
}

/// 归一化 cron 表达式。
///
/// 5 字段（`分 时 日 月 周`）是前端与用户习惯的标准写法，`cron` crate 需要
/// 秒字段；补 `0` 秒后再解析。已是 6/7 字段或 `@daily` 别名的表达式保持
/// 原样，不改变既有语义。
fn normalize_cron_pattern(pattern: &str) -> String {
    let trimmed = pattern.trim();
    let fields: Vec<&str> = trimmed.split_whitespace().collect();
    if fields.len() == 5 {
        format!("0 {}", fields.join(" "))
    } else {
        trimmed.to_string()
    }
}

impl Clone for TaskScheduler {
    fn clone(&self) -> Self {
        Self {
            handles: self.handles.clone(),
            locks: self.locks.clone(),
        }
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_field_cron_gets_zero_second_prefix() {
        assert_eq!(normalize_cron_pattern("0 9 * * *"), "0 0 9 * * *");
        assert_eq!(normalize_cron_pattern("  30  8  *  *  1  "), "0 30 8 * * 1");
    }

    #[test]
    fn five_field_cron_parses_and_schedules_daily_nine() {
        // 真机 v0.99.27：UI 默认 `0 9 * * *` 注册失败，定时训练从未执行。
        let schedule = TaskScheduler::parse_cron_schedule(Some("0 9 * * *"))
            .expect("标准 5 字段 cron 应可解析");
        let next = schedule
            .upcoming(chrono::Utc)
            .next()
            .expect("应存在下一次触发时间");
        assert_eq!(next.format("%H:%M:%S").to_string(), "09:00:00");
    }

    #[test]
    fn six_field_cron_keeps_seconds_semantics() {
        assert_eq!(normalize_cron_pattern("*/5 * * * * *"), "*/5 * * * * *");
        let schedule = TaskScheduler::parse_cron_schedule(Some("*/5 * * * * *"))
            .expect("6 字段 cron 应保持可用");
        assert!(schedule.upcoming(chrono::Utc).next().is_some());
    }

    #[test]
    fn invalid_cron_still_reports_error() {
        assert!(TaskScheduler::parse_cron_schedule(Some("not a cron")).is_err());
        assert!(TaskScheduler::parse_cron_schedule(None).is_err());
    }
}
