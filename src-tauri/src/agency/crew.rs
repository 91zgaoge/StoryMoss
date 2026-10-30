use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    agency::{
        lease::LeaseManager,
        models::{AgencyAgentTask, AgencyRun, ResourceLease},
        repository::AgencyRepository,
        task_repository::TaskRepository,
    },
    db::DbPool,
    error::AppError,
};

const SLOW_TASK_SECS: i64 = 90;
const STALE_TASK_SECS: i64 = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewAlert {
    pub severity: String, // warn | high
    pub kind: String,     /* StaleTask | SlowTask | FailedTask | FailedRun | OrphanTask |
                           * ResourceLeaseExpired */
    pub message: String,
    pub role: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrewSnapshot {
    pub story_id: String,
    pub story_title: String,
    pub runs: Vec<AgencyRun>,
    pub tasks: Vec<AgencyAgentTask>,
    pub leases: Vec<ResourceLease>,
    pub corpus_count: i64,
    pub alerts: Vec<CrewAlert>,
}

/// 巡检：只读、确定性、不调 LLM。导演/现场面板共用。
pub struct CrewInspector;

impl CrewInspector {
    pub fn inspect(pool: &DbPool, story_id: &str) -> Result<CrewSnapshot, AppError> {
        let story_title = story_title(pool, story_id)?;
        let runs = AgencyRepository::new(pool.clone())
            .list_runs_for_story(story_id, 20)
            .map_err(AppError::from)?;
        let tasks = TaskRepository::new(pool.clone())
            .list_for_story(story_id, 20)
            .map_err(AppError::from)?;
        let leases = LeaseManager::new(pool.clone()).list_for_story(story_id)?;
        let corpus_count: i64 = {
            let conn = pool
                .get()
                .map_err(|e| AppError::from(format!("pool: {e}")))?;
            conn.query_row(
                "SELECT COUNT(*) FROM corpus_items WHERE story_id = ?1",
                rusqlite::params![story_id],
                |r| r.get(0),
            )
            .unwrap_or(0)
        };

        let mut alerts = Vec::new();
        let terminal_runs: std::collections::HashSet<String> = runs
            .iter()
            .filter(|r| matches!(r.status.as_str(), "completed" | "failed" | "cancelled"))
            .map(|r| r.id.clone())
            .collect();

        for task in &tasks {
            if task.status == "running" {
                if let Some(hb) = task.heartbeat_at.as_deref() {
                    let age = age_secs(hb);
                    if age >= STALE_TASK_SECS {
                        alerts.push(CrewAlert {
                            severity: "high".into(),
                            kind: "StaleTask".into(),
                            message: format!(
                                "{} 任务心跳已 {}s 未更新，疑似卡住",
                                task.role.as_str(),
                                age
                            ),
                            role: Some(task.role.as_str().to_string()),
                            task_id: Some(task.id.clone()),
                            run_id: Some(task.run_id.clone()),
                        });
                    } else if age >= SLOW_TASK_SECS {
                        alerts.push(CrewAlert {
                            severity: "warn".into(),
                            kind: "SlowTask".into(),
                            message: format!(
                                "{} 任务心跳延迟 {}s（可能在等模型）",
                                task.role.as_str(),
                                age
                            ),
                            role: Some(task.role.as_str().to_string()),
                            task_id: Some(task.id.clone()),
                            run_id: Some(task.run_id.clone()),
                        });
                    }
                }
            }
            if task.status == "failed" {
                alerts.push(CrewAlert {
                    severity: "high".into(),
                    kind: "FailedTask".into(),
                    message: task
                        .error
                        .clone()
                        .unwrap_or_else(|| format!("{} 任务失败", task.role.as_str())),
                    role: Some(task.role.as_str().to_string()),
                    task_id: Some(task.id.clone()),
                    run_id: Some(task.run_id.clone()),
                });
            }
            if task.status == "running"
                && !task.run_id.starts_with("chat:")
                && terminal_runs.contains(&task.run_id)
            {
                alerts.push(CrewAlert {
                    severity: "warn".into(),
                    kind: "OrphanTask".into(),
                    message: format!("{} 任务仍在运行，但所属 run 已结束", task.role.as_str()),
                    role: Some(task.role.as_str().to_string()),
                    task_id: Some(task.id.clone()),
                    run_id: Some(task.run_id.clone()),
                });
            }
        }

        for run in &runs {
            if run.status == "failed" {
                alerts.push(CrewAlert {
                    severity: "high".into(),
                    kind: "FailedRun".into(),
                    message: format!(
                        "run {} 失败：{}",
                        run.id.chars().take(8).collect::<String>(),
                        run.error_message.as_deref().unwrap_or("未知错误")
                    ),
                    role: None,
                    task_id: None,
                    run_id: Some(run.id.clone()),
                });
            }
        }

        for lease in &leases {
            if !is_expired(&lease.expires_at) {
                continue;
            }
            alerts.push(CrewAlert {
                severity: "warn".into(),
                kind: "ResourceLeaseExpired".into(),
                message: format!(
                    "资源 {} 的租约已过期，仍由 {:?} 持有",
                    lease.resource, lease.owner_task_id
                ),
                role: None,
                task_id: lease.owner_task_id.clone(),
                run_id: lease.owner_run_id.clone(),
            });
        }

        Ok(CrewSnapshot {
            story_id: story_id.to_string(),
            story_title,
            runs,
            tasks,
            leases,
            corpus_count,
            alerts,
        })
    }
}

fn story_title(pool: &DbPool, story_id: &str) -> Result<String, AppError> {
    let conn = pool
        .get()
        .map_err(|e| AppError::from(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT title FROM stories WHERE id = ?1",
        rusqlite::params![story_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(AppError::from)
    .map(|v| v.unwrap_or_else(|| "未命名".to_string()))
}

fn age_secs(rfc3339: &str) -> i64 {
    let Ok(dt) = DateTime::parse_from_rfc3339(rfc3339) else {
        return 0;
    };
    Utc::now()
        .signed_duration_since(dt.with_timezone(&Utc))
        .num_seconds()
        .max(0)
}

fn is_expired(rfc3339: &str) -> bool {
    let Ok(dt) = DateTime::parse_from_rfc3339(rfc3339) else {
        return false;
    };
    dt.with_timezone(&Utc) <= Utc::now()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agency::models::{AgencyAgentTask, AgentRole},
        db::create_test_pool,
    };

    fn setup() -> (DbPool, String) {
        let pool = create_test_pool().unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at)
             VALUES ('story-1', '测试故事', 'now', 'now')",
            [],
        )
        .unwrap();
        (pool, "story-1".to_string())
    }

    #[tokio::test]
    async fn test_failed_and_lease_alerts() {
        let (pool, story_id) = setup();
        let task = AgencyAgentTask::new(
            "chat:s1",
            &story_id,
            AgentRole::Producer,
            "补资产",
            None,
            "{}",
        );
        TaskRepository::new(pool.clone())
            .create_task(&task)
            .unwrap();
        TaskRepository::new(pool.clone())
            .finish(&task.id, "failed", None, Some("模型失败"))
            .unwrap();
        let lease = LeaseManager::new(pool.clone())
            .acquire(&story_id, "assets:world", None, Some(&task.id), 0)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));

        let snap = CrewInspector::inspect(&pool, &story_id).unwrap();
        assert!(snap.alerts.iter().any(|a| a.kind == "FailedTask"));
        assert!(snap.alerts.iter().any(|a| a.kind == "ResourceLeaseExpired"));
        assert_eq!(snap.story_title, "测试故事");
        let _ = lease;
    }
}
