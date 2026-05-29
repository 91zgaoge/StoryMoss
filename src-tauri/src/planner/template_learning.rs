//! Plan Template Learning - 计划模板学习
//!
//! Records successful execution plans and reuses them for similar requests.
//! 支持 SQLite 持久化，重启后学习成果不丢失。

use serde::{Deserialize, Serialize};

use super::ExecutionPlan;
use crate::{db::DbPool, error::AppError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTemplate {
    pub id: String,
    pub trigger_patterns: Vec<String>,
    pub plan: ExecutionPlan,
    pub success_count: u32,
    pub failure_count: u32,
}

pub struct PlanTemplateLibrary {
    templates: Vec<PlanTemplate>,
    pool: DbPool,
}

impl PlanTemplateLibrary {
    pub fn new(pool: DbPool) -> Self {
        let mut library = Self {
            templates: Vec::new(),
            pool,
        };
        if let Err(e) = library.load_from_db() {
            log::warn!("[PlanTemplateLibrary] Failed to load from DB: {}", e);
        }
        library
    }

    /// 从数据库加载所有模板
    fn load_from_db(&mut self) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, trigger_patterns, plan_json, success_count, failure_count FROM \
                 plan_templates",
            )
            .map_err(AppError::from)?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let patterns_json: String = row.get(1)?;
                let plan_json: String = row.get(2)?;
                let success_count: i64 = row.get(3)?;
                let failure_count: i64 = row.get(4)?;
                let patterns: Vec<String> =
                    serde_json::from_str(&patterns_json).unwrap_or_default();
                let plan: ExecutionPlan =
                    serde_json::from_str(&plan_json).unwrap_or(ExecutionPlan {
                        understanding: String::new(),
                        steps: vec![],
                        fallback_message: String::new(),
                    });
                Ok(PlanTemplate {
                    id,
                    trigger_patterns: patterns,
                    plan,
                    success_count: success_count as u32,
                    failure_count: failure_count as u32,
                })
            })
            .map_err(AppError::from)?;

        for row in rows {
            if let Ok(template) = row {
                self.templates.push(template);
            }
        }
        log::info!(
            "[PlanTemplateLibrary] Loaded {} templates from DB",
            self.templates.len()
        );
        Ok(())
    }

    pub fn find_match(&self, user_input: &str) -> Option<&PlanTemplate> {
        self.templates
            .iter()
            .find(|t| t.trigger_patterns.iter().any(|p| user_input.contains(p)))
    }

    pub fn record_success(&mut self, user_input: &str, plan: ExecutionPlan) {
        let patterns: Vec<String> = user_input
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .map(|w| w.to_string())
            .collect();

        if !patterns.is_empty() {
            let template = PlanTemplate {
                id: uuid::Uuid::new_v4().to_string(),
                trigger_patterns: patterns.clone(),
                plan: plan.clone(),
                success_count: 1,
                failure_count: 0,
            };
            // 保存到数据库
            if let Err(e) = self.save_to_db(&template) {
                log::warn!("[PlanTemplateLibrary] Failed to save template to DB: {}", e);
            }
            self.templates.push(template);
        }
    }

    fn save_to_db(&self, template: &PlanTemplate) -> Result<(), AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let patterns_json =
            serde_json::to_string(&template.trigger_patterns).map_err(AppError::from)?;
        let plan_json = serde_json::to_string(&template.plan).map_err(AppError::from)?;
        let created_at = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO plan_templates (id, trigger_patterns, plan_json, success_count, \
             failure_count, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            [
                &template.id,
                &patterns_json,
                &plan_json,
                &(template.success_count as i64).to_string(),
                &(template.failure_count as i64).to_string(),
                &created_at,
            ],
        )
        .map_err(AppError::from)?;
        Ok(())
    }
}
