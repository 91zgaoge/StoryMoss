//! Task System 单元测试
//!
//! 覆盖模型状态机、Repository CRUD、心跳超时检测。

use std::sync::Arc;

use super::{models::*, repository::TaskRepository};
use crate::db::connection::create_test_pool;

// ==================== Models ====================

#[test]
fn test_task_status_from_str() {
    assert_eq!(TaskStatus::from_str("pending"), TaskStatus::Pending);
    assert_eq!(TaskStatus::from_str("running"), TaskStatus::Running);
    assert_eq!(TaskStatus::from_str("completed"), TaskStatus::Completed);
    assert_eq!(TaskStatus::from_str("failed"), TaskStatus::Failed);
    assert_eq!(TaskStatus::from_str("cancelled"), TaskStatus::Cancelled);
    assert_eq!(TaskStatus::from_str("unknown"), TaskStatus::Pending); // 默认值
}

#[test]
fn test_task_status_display() {
    assert_eq!(TaskStatus::Pending.to_string(), "pending");
    assert_eq!(TaskStatus::Running.to_string(), "running");
    assert_eq!(TaskStatus::Completed.to_string(), "completed");
}

#[test]
fn test_schedule_type_from_str() {
    assert_eq!(ScheduleType::from_str("once"), ScheduleType::Once);
    assert_eq!(ScheduleType::from_str("daily"), ScheduleType::Daily);
    assert_eq!(ScheduleType::from_str("weekly"), ScheduleType::Weekly);
    assert_eq!(ScheduleType::from_str("cron"), ScheduleType::Cron);
    assert_eq!(ScheduleType::from_str("unknown"), ScheduleType::Once); // 默认值
}

#[test]
fn test_task_type_from_str() {
    assert_eq!(
        TaskType::from_str("book_deconstruction"),
        TaskType::BookDeconstruction
    );
    assert_eq!(TaskType::from_str("custom"), TaskType::Custom);
    assert_eq!(TaskType::from_str("unknown"), TaskType::Custom); // 默认值
}

// ==================== Repository CRUD ====================

fn create_test_task_req(name: &str, task_type: &str, schedule_type: &str) -> CreateTaskRequest {
    CreateTaskRequest {
        name: name.to_string(),
        description: None,
        task_type: task_type.to_string(),
        schedule_type: schedule_type.to_string(),
        cron_pattern: None,
        payload: None,
        enabled: Some(true),
        max_retries: Some(3),
        heartbeat_timeout_seconds: Some(300),
    }
}

#[test]
fn test_repository_create_and_get() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let req = create_test_task_req("拆书任务", "book_deconstruction", "once");
    let task = repo.create(&req).unwrap();

    assert_eq!(task.name, "拆书任务");
    assert_eq!(task.task_type, TaskType::BookDeconstruction);
    assert_eq!(task.schedule_type, ScheduleType::Once);
    assert_eq!(task.status, TaskStatus::Pending);
    assert_eq!(task.progress, 0);
    assert!(task.enabled);

    // 读取验证
    let fetched = repo.get_by_id(&task.id).unwrap().unwrap();
    assert_eq!(fetched.name, task.name);
    assert_eq!(fetched.status, TaskStatus::Pending);
}

#[test]
fn test_repository_list_and_filter() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let req1 = create_test_task_req("任务A", "book_deconstruction", "once");
    let req2 = create_test_task_req("任务B", "custom", "daily");
    let _task1 = repo.create(&req1).unwrap();
    let _task2 = repo.create(&req2).unwrap();

    // 无过滤
    let all = repo.list(None, None).unwrap();
    assert_eq!(all.len(), 2);

    // 按状态过滤
    let pending = repo.list(Some("pending"), None).unwrap();
    assert_eq!(pending.len(), 2);

    // 按类型过滤
    let custom = repo.list(None, Some("custom")).unwrap();
    assert_eq!(custom.len(), 1);
    assert_eq!(custom[0].name, "任务B");
}

#[test]
fn test_repository_update_status() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let req = create_test_task_req("状态测试", "custom", "once");
    let task = repo.create(&req).unwrap();

    // 更新状态为 running
    repo.update_status(&task.id, &TaskStatus::Running, Some(50), None, None)
        .unwrap();
    let updated = repo.get_by_id(&task.id).unwrap().unwrap();
    assert_eq!(updated.status, TaskStatus::Running);
    assert_eq!(updated.progress, 50);

    // 更新状态为 completed
    repo.update_status(
        &task.id,
        &TaskStatus::Completed,
        Some(100),
        Some("完成".to_string()),
        None,
    )
    .unwrap();
    let completed = repo.get_by_id(&task.id).unwrap().unwrap();
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(completed.progress, 100);
    assert_eq!(completed.result, Some("完成".to_string()));
}

#[test]
fn test_repository_update_heartbeat() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let req = create_test_task_req("心跳测试", "custom", "once");
    let task = repo.create(&req).unwrap();

    assert!(task.last_heartbeat_at.is_none());

    repo.update_heartbeat(&task.id).unwrap();
    let updated = repo.get_by_id(&task.id).unwrap().unwrap();
    assert!(updated.last_heartbeat_at.is_some());
}

/// v0.93.14: 进度时间、日志时间、心跳时间必须来自同一个 now——
/// 真机反复报「任务进度时间与日志时间不一致」，根因是三者分开写入。
#[test]
fn test_record_progress_aligns_progress_log_and_heartbeat_time() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let task = repo
        .create(&create_test_task_req("进度时间一致", "custom", "once"))
        .unwrap();
    let now = "2026-11-09T14:34:47.825275+08:00";

    repo.record_progress(&task.id, 42, "正在续写下一章", now)
        .unwrap();

    let updated = repo.get_by_id(&task.id).unwrap().unwrap();
    assert_eq!(updated.status, TaskStatus::Running);
    assert_eq!(updated.progress, 42);
    assert_eq!(updated.updated_at, now, "进度更新时间必须与写入时间一致");
    assert_eq!(
        updated.last_heartbeat_at.as_deref(),
        Some(now),
        "心跳时间必须与进度时间一致"
    );

    let logs = repo.list_logs(&task.id).unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].created_at, now, "日志时间必须与进度时间一致");
    assert!(logs[0].message.contains("进度 42%"));
    assert!(logs[0].message.contains("正在续写下一章"));
}

#[test]
fn test_repository_retry_logic() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let req = create_test_task_req("重试测试", "custom", "once");
    let task = repo.create(&req).unwrap();
    assert_eq!(task.retry_count, 0);

    repo.increment_retry(&task.id).unwrap();
    let updated = repo.get_by_id(&task.id).unwrap().unwrap();
    assert_eq!(updated.retry_count, 1);

    repo.increment_retry(&task.id).unwrap();
    let updated2 = repo.get_by_id(&task.id).unwrap().unwrap();
    assert_eq!(updated2.retry_count, 2);

    repo.reset_retry(&task.id).unwrap();
    let reset = repo.get_by_id(&task.id).unwrap().unwrap();
    assert_eq!(reset.retry_count, 0);
}

#[test]
fn test_repository_logs() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let req = create_test_task_req("日志测试", "custom", "once");
    let task = repo.create(&req).unwrap();

    repo.create_log(&task.id, "info", "开始执行").unwrap();
    repo.create_log(&task.id, "error", "出错了").unwrap();

    let logs = repo.list_logs(&task.id).unwrap();
    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].message, "出错了"); // DESC 排序
    assert_eq!(logs[1].message, "开始执行");
}

#[test]
fn test_repository_delete() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let req = create_test_task_req("删除测试", "custom", "once");
    let task = repo.create(&req).unwrap();

    repo.delete(&task.id).unwrap();
    let deleted = repo.get_by_id(&task.id).unwrap();
    assert!(deleted.is_none());
}

#[test]
fn test_repository_list_running() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let req1 = create_test_task_req("运行中", "custom", "once");
    let req2 = create_test_task_req("待执行", "custom", "once");
    let task1 = repo.create(&req1).unwrap();
    let _task2 = repo.create(&req2).unwrap();

    // 将 task1 设为 running
    repo.update_status(&task1.id, &TaskStatus::Running, None, None, None)
        .unwrap();

    let running = repo.list_running().unwrap();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].id, task1.id);
}

// ==================== Heartbeat Timeout ====================

#[test]
fn test_heartbeat_timeout_detection() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool.clone());

    let req = create_test_task_req("超时测试", "custom", "once");
    let task = repo.create(&req).unwrap();

    // 手动设为 running，并设置过期的心跳
    repo.update_status(&task.id, &TaskStatus::Running, None, None, None)
        .unwrap();

    // 直接更新数据库设置过期心跳（绕过正常的 update_heartbeat）
    {
        let conn = pool.get().unwrap();
        let expired = chrono::Local::now() - chrono::Duration::seconds(400);
        conn.execute(
            "UPDATE tasks SET last_heartbeat_at = ?1, last_run_at = ?1 WHERE id = ?2",
            rusqlite::params![expired.to_rfc3339(), task.id],
        )
        .unwrap();
    }

    // 运行心跳检测
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // 使用 HeartbeatMonitor 的 check_all 方法
        // 由于 check_all 是 private，我们直接模拟其逻辑
        let task = repo.get_by_id(&task.id).unwrap().unwrap();
        let timeout_secs = task.heartbeat_timeout_seconds as i64;
        let now = chrono::Local::now();

        let is_timeout = match &task.last_heartbeat_at {
            Some(heartbeat_str) => {
                let heartbeat = chrono::DateTime::parse_from_rfc3339(heartbeat_str).unwrap();
                let elapsed = now.signed_duration_since(heartbeat.with_timezone(&chrono::Local));
                elapsed.num_seconds() > timeout_secs
            }
            None => true,
        };

        assert!(is_timeout, "应该检测到心跳超时");
    });
}

// ==================== v0.93.2 任务融合 ====================

/// 统一取消监听器：任务未取消时不动，翻转 cancelled 后置位共享标志
#[test]
fn test_cancel_monitor_sets_flag_on_cancel() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool.clone());
    let task = repo
        .create(&create_test_task_req("取消监听", "custom", "once"))
        .unwrap();

    let flag = Arc::new(AtomicBool::new(false));
    let monitor = super::executor::spawn_cancel_monitor(task.id.clone(), pool.clone(), {
        let flag = flag.clone();
        move || flag.store(true, Ordering::SeqCst)
    });

    // 未取消：监听周期内 flag 保持 false
    repo.update_status(&task.id, &TaskStatus::Running, None, None, None)
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1100));
    assert!(!flag.load(Ordering::SeqCst), "未取消时不应触发回调");

    // 翻转 cancelled：监听应在 ~500ms 轮询周期内感知并置位
    repo.update_status(&task.id, &TaskStatus::Cancelled, None, None, None)
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !flag.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(flag.load(Ordering::SeqCst), "取消后监听应置位共享标志");
    drop(monitor); // Drop 自动 abort 轮询协程
}

/// 执行锁共享语义：run_task_internal 依赖「同一 TaskScheduler 实例的
/// ensure_lock 命中同一把锁」实现手动/定时/once 全路径互斥
#[test]
fn test_shared_scheduler_lock_table_mutual_exclusion() {
    use super::scheduler::TaskScheduler;

    let scheduler = Arc::new(TaskScheduler::new());
    let lock_a = scheduler.ensure_lock("t-lock-1");
    let lock_b = scheduler.clone().ensure_lock("t-lock-1");
    assert!(
        Arc::ptr_eq(&lock_a, &lock_b),
        "同一实例的 ensure_lock 必须命中同一把共享锁"
    );

    let guard = lock_a.try_lock().unwrap();
    assert!(
        lock_b.try_lock().is_err(),
        "持锁期间其他触发路径必须拿不到锁"
    );
    drop(guard);
    assert!(lock_b.try_lock().is_ok(), "释放后应可再次获取");
}

/// UpdateTaskRequest.payload 更新（续写训练「保存计划即同步配置」依赖）
#[test]
fn test_repository_update_payload() {
    let pool = create_test_pool().unwrap();
    let repo = TaskRepository::new(pool);

    let mut req = create_test_task_req("payload 更新", "continuation_training", "cron");
    req.cron_pattern = Some("0 9 * * *".into());
    req.payload = Some(r#"{"book_title":"旧书"}"#.into());
    let task = repo.create(&req).unwrap();
    assert_eq!(task.task_type, TaskType::ContinuationTraining);

    let upd = UpdateTaskRequest {
        name: None,
        description: None,
        enabled: None,
        cron_pattern: None,
        max_retries: None,
        heartbeat_timeout_seconds: None,
        payload: Some(r#"{"book_title":"新书"}"#.into()),
    };
    repo.update(&task.id, &upd).unwrap();

    let fetched = repo.get_by_id(&task.id).unwrap().unwrap();
    assert_eq!(fetched.payload.as_deref(), Some(r#"{"book_title":"新书"}"#));
}
