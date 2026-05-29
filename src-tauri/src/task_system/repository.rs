//! Task System Repository
//!
//! 数据库 CRUD 操作，参考 memoh-X 的 sqlc queries 风格

use chrono::Local;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::models::*;
use crate::db::DbPool;

pub struct TaskRepository {
    pool: DbPool,
}

impl TaskRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // ==================== Task CRUD ====================

    pub fn create(&self, req: &CreateTaskRequest) -> Result<Task, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();
        let enabled = req.enabled.unwrap_or(true);
        let max_retries = req.max_retries.unwrap_or(3);
        let heartbeat_timeout = req.heartbeat_timeout_seconds.unwrap_or(300);
        let task_type = TaskType::from_str(&req.task_type);
        let schedule_type = ScheduleType::from_str(&req.schedule_type);

        // 计算下次运行时间
        let next_run_at = if enabled && schedule_type != ScheduleType::Once {
            Some(Self::compute_next_run(
                &schedule_type,
                req.cron_pattern.as_deref(),
                &now,
            )?)
        } else {
            None
        };

        conn.execute(
            "INSERT INTO tasks (
                id, name, description, task_type, schedule_type, cron_pattern,
                payload, status, progress, result, error_message,
                max_retries, retry_count, enabled,
                last_run_at, next_run_at, last_heartbeat_at, heartbeat_timeout_seconds,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
             ?18, ?19, ?20)",
            params![
                &id,
                &req.name,
                &req.description,
                task_type.to_string(),
                schedule_type.to_string(),
                &req.cron_pattern,
                &req.payload,
                "pending",
                0,
                None::<&str>,
                None::<&str>,
                max_retries,
                0,
                enabled as i32,
                None::<&str>,
                &next_run_at,
                None::<&str>,
                heartbeat_timeout,
                &now,
                &now,
            ],
        )?;

        Ok(Task {
            id,
            name: req.name.clone(),
            description: req.description.clone(),
            task_type,
            schedule_type,
            cron_pattern: req.cron_pattern.clone(),
            payload: req.payload.clone(),
            status: TaskStatus::Pending,
            progress: 0,
            result: None,
            error_message: None,
            max_retries,
            retry_count: 0,
            enabled,
            last_run_at: None,
            next_run_at,
            last_heartbeat_at: None,
            heartbeat_timeout_seconds: heartbeat_timeout,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Task>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, task_type, schedule_type, cron_pattern,
                    payload, status, progress, result, error_message,
                    max_retries, retry_count, enabled,
                    last_run_at, next_run_at, last_heartbeat_at, heartbeat_timeout_seconds,
                    created_at, updated_at
             FROM tasks WHERE id = ?1",
        )?;

        let task = stmt
            .query_row([id], |row| {
                Ok(Task {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    task_type: TaskType::from_str(&row.get::<_, String>(3)?),
                    schedule_type: ScheduleType::from_str(&row.get::<_, String>(4)?),
                    cron_pattern: row.get(5)?,
                    payload: row.get(6)?,
                    status: TaskStatus::from_str(&row.get::<_, String>(7)?),
                    progress: row.get(8)?,
                    result: row.get(9)?,
                    error_message: row.get(10)?,
                    max_retries: row.get(11)?,
                    retry_count: row.get(12)?,
                    enabled: row.get::<_, i32>(13)? != 0,
                    last_run_at: row.get(14)?,
                    next_run_at: row.get(15)?,
                    last_heartbeat_at: row.get(16)?,
                    heartbeat_timeout_seconds: row.get(17)?,
                    created_at: row.get(18)?,
                    updated_at: row.get(19)?,
                })
            })
            .optional()?;

        Ok(task)
    }

    pub fn list(
        &self,
        status_filter: Option<&str>,
        task_type_filter: Option<&str>,
    ) -> Result<Vec<Task>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;

        let sql = match (status_filter, task_type_filter) {
            (Some(_), Some(_)) => {
                "SELECT id, name, description, task_type, schedule_type, cron_pattern,
                    payload, status, progress, result, error_message,
                    max_retries, retry_count, enabled,
                    last_run_at, next_run_at, last_heartbeat_at, heartbeat_timeout_seconds,
                    created_at, updated_at
             FROM tasks WHERE status = ?1 AND task_type = ?2 ORDER BY created_at DESC"
            }
            (Some(_), None) => {
                "SELECT id, name, description, task_type, schedule_type, cron_pattern,
                    payload, status, progress, result, error_message,
                    max_retries, retry_count, enabled,
                    last_run_at, next_run_at, last_heartbeat_at, heartbeat_timeout_seconds,
                    created_at, updated_at
             FROM tasks WHERE status = ?1 ORDER BY created_at DESC"
            }
            (None, Some(_)) => {
                "SELECT id, name, description, task_type, schedule_type, cron_pattern,
                    payload, status, progress, result, error_message,
                    max_retries, retry_count, enabled,
                    last_run_at, next_run_at, last_heartbeat_at, heartbeat_timeout_seconds,
                    created_at, updated_at
             FROM tasks WHERE task_type = ?1 ORDER BY created_at DESC"
            }
            (None, None) => {
                "SELECT id, name, description, task_type, schedule_type, cron_pattern,
                    payload, status, progress, result, error_message,
                    max_retries, retry_count, enabled,
                    last_run_at, next_run_at, last_heartbeat_at, heartbeat_timeout_seconds,
                    created_at, updated_at
             FROM tasks ORDER BY created_at DESC"
            }
        };

        let mut stmt = conn.prepare(sql)?;

        let rows = match (status_filter, task_type_filter) {
            (Some(s), Some(t)) => stmt.query_map([s, t], Self::map_task)?,
            (Some(s), None) => stmt.query_map([s], Self::map_task)?,
            (None, Some(t)) => stmt.query_map([t], Self::map_task)?,
            (None, None) => stmt.query_map([], Self::map_task)?,
        };

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }

        Ok(tasks)
    }

    pub fn list_enabled_scheduled(&self) -> Result<Vec<Task>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, task_type, schedule_type, cron_pattern,
                    payload, status, progress, result, error_message,
                    max_retries, retry_count, enabled,
                    last_run_at, next_run_at, last_heartbeat_at, heartbeat_timeout_seconds,
                    created_at, updated_at
             FROM tasks WHERE enabled = 1 AND schedule_type != 'once'
             ORDER BY next_run_at ASC",
        )?;

        let rows = stmt.query_map([], Self::map_task)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn list_running(&self) -> Result<Vec<Task>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, task_type, schedule_type, cron_pattern,
                    payload, status, progress, result, error_message,
                    max_retries, retry_count, enabled,
                    last_run_at, next_run_at, last_heartbeat_at, heartbeat_timeout_seconds,
                    created_at, updated_at
             FROM tasks WHERE status = 'running'",
        )?;

        let rows = stmt.query_map([], Self::map_task)?;
        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row?);
        }
        Ok(tasks)
    }

    pub fn update(
        &self,
        id: &str,
        req: &UpdateTaskRequest,
    ) -> Result<Task, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let now = Local::now().to_rfc3339();

        // 先获取现有数据
        let existing = self.get_by_id(id)?.ok_or("Task not found")?;

        let name = req.name.as_ref().unwrap_or(&existing.name);
        let description = req.description.as_ref().or(existing.description.as_ref());
        let enabled = req.enabled.unwrap_or(existing.enabled);
        let cron_pattern = req.cron_pattern.as_ref().or(existing.cron_pattern.as_ref());
        let max_retries = req.max_retries.unwrap_or(existing.max_retries);
        let heartbeat_timeout = req
            .heartbeat_timeout_seconds
            .unwrap_or(existing.heartbeat_timeout_seconds);

        // 如果启用了定时任务且 schedule_type 不是 once，重新计算 next_run_at
        let next_run_at = if enabled && existing.schedule_type != ScheduleType::Once {
            Some(Self::compute_next_run(
                &existing.schedule_type,
                cron_pattern.map(|s| s.as_str()),
                &now,
            )?)
        } else {
            None
        };

        conn.execute(
            "UPDATE tasks SET
                name = ?1,
                description = ?2,
                enabled = ?3,
                cron_pattern = ?4,
                max_retries = ?5,
                heartbeat_timeout_seconds = ?6,
                next_run_at = ?7,
                updated_at = ?8
             WHERE id = ?9",
            params![
                name,
                description,
                enabled as i32,
                cron_pattern,
                max_retries,
                heartbeat_timeout,
                &next_run_at,
                &now,
                id,
            ],
        )?;

        self.get_by_id(id)?
            .ok_or_else(|| "Task not found after update".into())
    }

    pub fn update_status(
        &self,
        id: &str,
        status: &TaskStatus,
        progress: Option<i32>,
        result: Option<String>,
        error_message: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let now = Local::now().to_rfc3339();

        let progress = progress.unwrap_or(0);

        conn.execute(
            "UPDATE tasks SET status = ?1, progress = ?2, result = ?3, error_message = ?4, \
             updated_at = ?5 WHERE id = ?6",
            params![
                status.to_string(),
                progress,
                &result,
                &error_message,
                &now,
                id,
            ],
        )?;

        Ok(())
    }

    pub fn update_next_run_at(
        &self,
        id: &str,
        next_run_at: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE tasks SET next_run_at = ?1, updated_at = ?2 WHERE id = ?3",
            params![next_run_at, &now, id],
        )?;
        Ok(())
    }

    pub fn update_heartbeat(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let now = Local::now().to_rfc3339();

        conn.execute(
            "UPDATE tasks SET last_heartbeat_at = ?1, updated_at = ?2 WHERE id = ?3",
            params![&now, &now, id],
        )?;

        Ok(())
    }

    pub fn update_last_run(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let now = Local::now().to_rfc3339();

        // 获取任务信息以计算下次运行时间
        let task = self.get_by_id(id)?.ok_or("Task not found")?;
        let next_run_at = if task.enabled && task.schedule_type != ScheduleType::Once {
            Some(Self::compute_next_run(
                &task.schedule_type,
                task.cron_pattern.as_deref(),
                &now,
            )?)
        } else {
            None
        };

        conn.execute(
            "UPDATE tasks SET last_run_at = ?1, next_run_at = ?2, updated_at = ?3 WHERE id = ?4",
            params![&now, &next_run_at, &now, id],
        )?;

        Ok(())
    }

    pub fn increment_retry(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        conn.execute(
            "UPDATE tasks SET retry_count = retry_count + 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn reset_retry(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        conn.execute(
            "UPDATE tasks SET retry_count = 0 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ==================== Task Logs ====================

    pub fn create_log(
        &self,
        task_id: &str,
        level: &str,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        conn.execute(
            "INSERT INTO task_logs (id, task_id, log_level, message, created_at) VALUES (?1, ?2, \
             ?3, ?4, ?5)",
            params![&id, task_id, level, message, &now],
        )?;

        Ok(())
    }

    pub fn list_logs(&self, task_id: &str) -> Result<Vec<TaskLog>, Box<dyn std::error::Error>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, task_id, log_level, message, created_at FROM task_logs WHERE task_id = ?1 \
             ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([task_id], |row| {
            Ok(TaskLog {
                id: row.get(0)?,
                task_id: row.get(1)?,
                log_level: row.get(2)?,
                message: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut logs = Vec::new();
        for row in rows {
            logs.push(row?);
        }
        Ok(logs)
    }

    // ==================== Helpers ====================

    fn map_task(row: &rusqlite::Row) -> Result<Task, rusqlite::Error> {
        Ok(Task {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            task_type: TaskType::from_str(&row.get::<_, String>(3)?),
            schedule_type: ScheduleType::from_str(&row.get::<_, String>(4)?),
            cron_pattern: row.get(5)?,
            payload: row.get(6)?,
            status: TaskStatus::from_str(&row.get::<_, String>(7)?),
            progress: row.get(8)?,
            result: row.get(9)?,
            error_message: row.get(10)?,
            max_retries: row.get(11)?,
            retry_count: row.get(12)?,
            enabled: row.get::<_, i32>(13)? != 0,
            last_run_at: row.get(14)?,
            next_run_at: row.get(15)?,
            last_heartbeat_at: row.get(16)?,
            heartbeat_timeout_seconds: row.get(17)?,
            created_at: row.get(18)?,
            updated_at: row.get(19)?,
        })
    }

    /// 计算下次运行时间
    fn compute_next_run(
        schedule_type: &ScheduleType,
        cron_pattern: Option<&str>,
        _from: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let now = chrono::Local::now();
        let next = match schedule_type {
            ScheduleType::Daily => now + chrono::Duration::days(1),
            ScheduleType::Weekly => now + chrono::Duration::weeks(1),
            ScheduleType::Cron => {
                // 简化：cron 表达式解析为从当前时间 +1小时作为下次
                // 实际生产环境应引入 cron 解析库
                if let Some(pattern) = cron_pattern {
                    // 基本解析: "分 时 * * *" 格式
                    let parts: Vec<&str> = pattern.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let (Ok(minute), Ok(hour)) =
                            (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                        {
                            let mut next_date = now
                                .date_naive()
                                .and_hms_opt(hour, minute, 0)
                                .unwrap_or(now.naive_local());
                            if next_date <= now.naive_local() {
                                next_date = next_date + chrono::Duration::days(1);
                            }
                            return Ok(next_date.and_utc().to_rfc3339());
                        }
                    }
                }
                now + chrono::Duration::hours(1)
            }
            ScheduleType::Once => now, // once 任务不设置 next_run
        };
        Ok(next.to_rfc3339())
    }
}
