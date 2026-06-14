//! InsightExecutor - 分时架构时间线 3 的深度洞察执行器
//!
//! 设计依据：docs/plans/2026-06-14-time-sliced-intervention-design.md 模块 4/6
//!
//! 职责：低频（每 N 段 / 漂移阈值触发）的跨章节深度分析，防止长篇滚成大灾难。
//! - 追读力趋势（ReadingPowerEvaluator，最近 N 章）
//! - 追读债务汇总（ChaseDebt active debts）
//! - 未处理 annotation 汇总（从 text_annotations 统计）
//! - 报告写入 story_summaries（summary_type = "deep_insight"）
//!
//! 范围控制：本版聚焦"数据现成、不需额外 LLM"的洞察。
//! KG 深度遍历 / 向量检索 / Memory Ingest 留作后续增强（需更多依赖接入）。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::{
    executor::{TaskExecutionContext, TaskExecutor},
    models::{Task, TaskResult, TaskType},
};
use crate::{
    db::{ChaseDebtRepository, DbPool, StorySummaryRepository, TextAnnotationRepository},
    reading_power::ReadingPowerEvaluator,
    state_sync::events::SyncEvent,
};

/// 深度洞察 payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightPayload {
    pub story_id: String,
    pub chapter_number: i32,
    /// 评估的章节数（趋势窗口，默认最近 5 章）
    #[serde(default = "default_window")]
    pub trend_window: i32,
}

fn default_window() -> i32 {
    5
}

/// 深度洞察执行器
pub struct InsightExecutor {
    pub pool: DbPool,
    pub app_handle: AppHandle,
}

#[async_trait]
impl TaskExecutor for InsightExecutor {
    fn can_handle(&self, task_type: &TaskType) -> bool {
        *task_type == TaskType::DeepInsight
    }

    async fn execute(&self, task: &Task) -> Result<TaskResult, Box<dyn std::error::Error>> {
        let ctx =
            TaskExecutionContext::new(task.id.clone(), self.pool.clone(), self.app_handle.clone());
        ctx.start()?;

        let payload: InsightPayload = match task.payload.as_ref() {
            Some(json) => match serde_json::from_str(json) {
                Ok(p) => p,
                Err(e) => {
                    let msg = format!("[InsightExecutor] payload 解析失败: {}", e);
                    ctx.fail(&msg)?;
                    return Ok(task_err(msg));
                }
            },
            None => {
                let msg = "[InsightExecutor] 缺少 payload".to_string();
                ctx.fail(&msg)?;
                return Ok(task_err(msg));
            }
        };

        ctx.update_progress("analyze", 20, "正在分析追读力趋势...");
        let report = self.build_report(&payload).await;

        ctx.update_progress("save", 70, "正在保存洞察报告...");
        let report_json = serde_json::to_string_pretty(&report)?;

        // 存入 story_summaries（summary_type = "deep_insight"）
        let repo = StorySummaryRepository::new(self.pool.clone());
        if let Err(e) = repo.create_summary(&payload.story_id, "deep_insight", &report_json) {
            let msg = format!("[InsightExecutor] 保存报告失败: {}", e);
            ctx.fail(&msg)?;
            return Ok(task_err(msg));
        }

        ctx.update_progress("done", 100, "深度洞察完成");
        ctx.complete(Some(report_json.clone()))?;

        // 通知前端刷新（DataRefresh 让 NarrativeAnalysis 页重新拉取）
        let _ = self.app_handle.emit(
            "sync-event",
            SyncEvent::DataRefresh {
                story_id: Some(payload.story_id.clone()),
                resource_type: "summaries".to_string(),
            },
        );

        Ok(task_ok(Some(report_json)))
    }
}

impl InsightExecutor {
    /// 判断是否应触发深度洞察。
    /// 条件：距上次 insight 报告的章节数 >= interval（默认 5），或从未跑过。
    pub fn should_trigger(
        pool: &DbPool,
        story_id: &str,
        current_chapter: i32,
        interval: i32,
    ) -> bool {
        let repo = StorySummaryRepository::new(pool.clone());
        match repo.get_summary_by_type(story_id, "deep_insight") {
            Ok(Some(summary)) => {
                // 从报告 JSON 解析 chapter_range 的结束章节
                if let Ok(report) = serde_json::from_str::<InsightReport>(&summary.content) {
                    let last_chapter = report.chapter_range.1;
                    current_chapter - last_chapter >= interval
                } else {
                    // JSON 解析失败，保守触发
                    true
                }
            }
            Ok(None) => true, // 从未跑过
            Err(_) => true,   // 查询失败，保守触发
        }
    }

    /// 直接执行洞察（不经过 task_system 的 Task 调度）。
    /// 供 orchestrator 在条件触发时 spawn 调用。
    pub async fn run_insight(&self, payload: InsightPayload) {
        log::info!(
            "[InsightExecutor] run_insight: story={}, chapter={}, window={}",
            payload.story_id,
            payload.chapter_number,
            payload.trend_window
        );
        let report = self.build_report(&payload).await;
        let report_json = match serde_json::to_string_pretty(&report) {
            Ok(j) => j,
            Err(e) => {
                log::warn!("[InsightExecutor] 报告序列化失败: {}", e);
                return;
            }
        };
        let repo = StorySummaryRepository::new(self.pool.clone());
        if let Err(e) = repo.create_summary(&payload.story_id, "deep_insight", &report_json) {
            log::warn!("[InsightExecutor] 保存报告失败: {}", e);
            return;
        }
        let _ = self.app_handle.emit(
            "sync-event",
            SyncEvent::DataRefresh {
                story_id: Some(payload.story_id.clone()),
                resource_type: "summaries".to_string(),
            },
        );
        log::info!(
            "[InsightExecutor] 洞察完成，整体健康度: {:.0}/100",
            report.overall_health
        );
    }

    async fn build_report(&self, payload: &InsightPayload) -> InsightReport {
        let story_id = &payload.story_id;
        let window = payload.trend_window;

        // 1. 追读力趋势（最近 N 章）
        let evaluator = ReadingPowerEvaluator::new(self.pool.clone());
        let mut trend: Vec<ChapterReadingPower> = Vec::new();
        let start = (payload.chapter_number - window + 1).max(1);
        for ch in start..=payload.chapter_number {
            match evaluator.evaluate(story_id, ch) {
                Ok(eval) => trend.push(ChapterReadingPower {
                    chapter: ch,
                    score: eval.score as f32,
                    hook_strength: eval.hook_strength.clone(),
                    coolpoint_count: eval.coolpoint_patterns.len() as i32,
                    micropayoff_count: eval.micropayoffs.len() as i32,
                    debt_balance: eval.debt_balance,
                }),
                Err(_) => continue, // 某章无数据则跳过
            }
        }

        // 2. 追读债务汇总
        let debt_repo = ChaseDebtRepository::new(self.pool.clone());
        let active_debts = debt_repo.get_active_by_story(story_id).unwrap_or_default();
        let total_debt = active_debts.iter().map(|d| d.current_amount).sum::<f64>();
        let overdue_count = active_debts
            .iter()
            .filter(|d| d.status == "overdue")
            .count();

        // 3. 未处理 annotation 汇总（"小债"盘点）
        let ann_repo = TextAnnotationRepository::new(self.pool.clone());
        let unresolved = ann_repo
            .get_annotations_by_story(story_id)
            .unwrap_or_default();
        let high_count = unresolved.iter().filter(|a| a.severity == "high").count();
        let ai_audit_count = unresolved
            .iter()
            .filter(|a| a.annotation_type == crate::db::AnnotationType::AiAudit)
            .count();

        // 4. 整体健康度（综合分）
        let latest_score = trend.last().map(|t| t.score).unwrap_or(50.0) as f64;
        let debt_penalty = (total_debt * 2.0).min(30.0);
        let annotation_penalty = (high_count as f64 * 3.0).min(20.0);
        let overall_health = (latest_score - debt_penalty - annotation_penalty)
            .max(0.0)
            .min(100.0) as f32;

        InsightReport {
            story_id: story_id.clone(),
            evaluated_at: chrono::Local::now().to_rfc3339(),
            chapter_range: (start, payload.chapter_number),
            overall_health,
            reading_power_trend: trend,
            chase_debt: ChaseDebtSummary {
                total_amount: total_debt,
                active_count: active_debts.len(),
                overdue_count,
            },
            unresolved_annotations: AnnotationSummary {
                total: unresolved.len(),
                high_severity: high_count,
                ai_audit: ai_audit_count,
            },
        }
    }
}

// ==================== 报告数据结构 ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsightReport {
    pub story_id: String,
    pub evaluated_at: String,
    pub chapter_range: (i32, i32),
    /// 综合健康度 0-100（追读力 - 债务惩罚 - annotation 惩罚）
    pub overall_health: f32,
    pub reading_power_trend: Vec<ChapterReadingPower>,
    pub chase_debt: ChaseDebtSummary,
    pub unresolved_annotations: AnnotationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterReadingPower {
    pub chapter: i32,
    pub score: f32,
    pub hook_strength: String,
    pub coolpoint_count: i32,
    pub micropayoff_count: i32,
    pub debt_balance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaseDebtSummary {
    pub total_amount: f64,
    pub active_count: usize,
    pub overdue_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationSummary {
    pub total: usize,
    pub high_severity: usize,
    pub ai_audit: usize,
}

fn task_ok(result_json: Option<String>) -> TaskResult {
    TaskResult {
        success: true,
        result_json,
        error_message: None,
    }
}

fn task_err(error_message: impl Into<String>) -> TaskResult {
    TaskResult {
        success: false,
        result_json: None,
        error_message: Some(error_message.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_is_5() {
        assert_eq!(default_window(), 5);
    }

    #[test]
    fn insight_report_serializes() {
        let report = InsightReport {
            story_id: "test".to_string(),
            evaluated_at: "2026-01-01T00:00:00Z".to_string(),
            chapter_range: (1, 5),
            overall_health: 75.0,
            reading_power_trend: vec![ChapterReadingPower {
                chapter: 5,
                score: 80.0,
                hook_strength: "strong".to_string(),
                coolpoint_count: 2,
                micropayoff_count: 5,
                debt_balance: 1.0,
            }],
            chase_debt: ChaseDebtSummary {
                total_amount: 1.5,
                active_count: 2,
                overdue_count: 0,
            },
            unresolved_annotations: AnnotationSummary {
                total: 4,
                high_severity: 1,
                ai_audit: 3,
            },
        };
        let json = serde_json::to_string(&report).unwrap();
        let parsed: InsightReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.overall_health, 75.0);
        assert_eq!(parsed.reading_power_trend.len(), 1);
        assert_eq!(parsed.unresolved_annotations.ai_audit, 3);
    }
}
