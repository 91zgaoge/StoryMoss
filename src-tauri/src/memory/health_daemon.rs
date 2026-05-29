//! Memory Health Daemon — 后台记忆健康守护进程
//!
//! v0.8.0: 简化版实现
//! - 每小时运行一次 RetentionManager
//! - 自动归档遗忘实体
//! - 发射 health report 事件到前端

use std::sync::Arc;

use tauri::Emitter;
use tokio::time::{interval, Duration};

use crate::db::DbPool;

pub struct MemoryHealthDaemon {
    pool: DbPool,
}

impl MemoryHealthDaemon {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 启动定时任务（每小时运行一次）
    pub async fn start(self: Arc<Self>, app_handle: tauri::AppHandle) {
        let mut ticker = interval(Duration::from_secs(3600));
        log::info!("[MemoryHealthDaemon] Started, running every 3600s");

        loop {
            ticker.tick().await;
            if let Err(e) = self.run_once(&app_handle).await {
                log::warn!("[MemoryHealthDaemon] Run failed: {}", e);
            }
        }
    }

    /// 单次运行
    async fn run_once(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        use crate::{
            db::repositories::{KnowledgeGraphRepository, StoryRepository},
            memory::retention::RetentionManager,
        };

        log::info!("[MemoryHealthDaemon] Running health check...");

        let story_repo = StoryRepository::new(self.pool.clone());
        let kg_repo = KnowledgeGraphRepository::new(self.pool.clone());

        let stories = story_repo
            .get_all()
            .map_err(|e| format!("Failed to get stories: {}", e))?;

        let manager = RetentionManager::new();
        let mut total_archived = 0;

        for story in &stories {
            let entities = kg_repo
                .get_entities_by_story(&story.id)
                .map_err(|e| format!("Failed to get entities: {}", e))?;

            let report = manager.generate_retention_report(&entities);
            let forgotten = manager.get_forgotten_entities(&entities);

            for (entity, _) in &forgotten {
                if let Err(e) = kg_repo.archive_entity(&entity.id) {
                    log::warn!(
                        "[MemoryHealthDaemon] Failed to archive entity {}: {}",
                        entity.id,
                        e
                    );
                } else {
                    total_archived += 1;
                }
            }

            // 统计各优先级数量
            let critical = report
                .level_distribution
                .get("critical")
                .copied()
                .unwrap_or(0);
            let high = report.level_distribution.get("high").copied().unwrap_or(0);
            let medium = report
                .level_distribution
                .get("medium")
                .copied()
                .unwrap_or(0);
            let low = report.level_distribution.get("low").copied().unwrap_or(0);
            let forgotten_count = report
                .level_distribution
                .get("forgotten")
                .copied()
                .unwrap_or(0);

            // 发射 health report 事件
            let _ = app_handle.emit(
                "memory-health-report",
                serde_json::json!({
                    "story_id": story.id,
                    "story_title": story.title,
                    "total_entities": report.total_entities,
                    "critical": critical,
                    "high": high,
                    "medium": medium,
                    "low": low,
                    "forgotten": forgotten_count,
                    "archived_this_run": forgotten.len(),
                }),
            );
        }

        log::info!(
            "[MemoryHealthDaemon] Archived {} forgotten entities across {} stories",
            total_archived,
            stories.len()
        );

        Ok(())
    }
}

/// 启动守护进程（在应用启动时调用）
pub fn spawn_daemon(pool: DbPool, app_handle: tauri::AppHandle) {
    let daemon = Arc::new(MemoryHealthDaemon::new(pool));
    tokio::spawn(async move {
        daemon.start(app_handle).await;
    });
    log::info!("[MemoryHealthDaemon] Spawned background task");
}
