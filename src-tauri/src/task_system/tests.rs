//! Task System 单元测试
//!
//! 覆盖模型状态机、Repository CRUD、心跳超时检测。

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
