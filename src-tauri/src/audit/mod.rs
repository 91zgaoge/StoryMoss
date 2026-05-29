//! Audit System - 统一审计报告系统
//!
//! 整合 ContinuityEngine、StyleChecker、QualityChecker，
//! 为场景生成五维审计报告。

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    creative_engine::{
        continuity::{ContinuityEngine, Severity as ContinuitySeverity},
        payoff_ledger::PayoffLedger,
        style::{dna::StyleDNA, StyleAnalyzer, StyleChecker},
        workflow::quality::{QualityChecker, Severity as QualitySeverity},
    },
    db::{
        repositories::{SceneRepository, StoryRepository, StyleDnaRepository},
        DbPool,
    },
    error::AppError,
    llm::service::LlmService,
};

pub mod commands;

/// 审计报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub scene_id: String,
    pub overall_score: f32,
    pub dimensions: Vec<AuditDimension>,
    pub has_blocking_issues: bool,
    pub audit_type: String,
    pub content_word_count: usize,
}

/// 审计维度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditDimension {
    pub name: String,
    pub score: f32,
    pub issues: Vec<AuditIssue>,
}

/// 审计问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditIssue {
    pub severity: String,
    pub message: String,
    pub suggestion: Option<String>,
}

/// 审计服务
pub struct AuditService {
    pool: DbPool,
}

impl AuditService {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 审计场景
    pub async fn audit_scene(
        &self,
        scene_id: &str,
        audit_type: &str,
        app_handle: Option<&AppHandle>,
    ) -> Result<AuditReport, AppError> {
        // 1. 获取场景信息
        let scene_repo = SceneRepository::new(self.pool.clone());
        let scene = scene_repo
            .get_by_id(scene_id)
            .map_err(|e| format!("获取场景失败: {}", e))?
            .ok_or("场景不存在")?;

        let content = scene.content.unwrap_or_default();
        let content_len = content.chars().count();

        // 智能升降级：字数 < 200 或 > 5000 自动升级为完整审计
        let effective_audit_type = if content_len < 200 || content_len > 5000 {
            "full"
        } else {
            audit_type
        };

        // 2. 获取故事信息（用于风格 DNA）
        let story_repo = StoryRepository::new(self.pool.clone());
        let story = story_repo
            .get_by_id(&scene.story_id)
            .map_err(|e| format!("获取故事失败: {}", e))?
            .ok_or("故事不存在")?;

        // 3. 运行各项检查
        let continuity_dim = self.check_continuity(&scene.story_id, scene_id, &content)?;
        let character_dim = self.check_character(&content)?;
        let style_dim = self.check_style(&content, story.style_dna_id.as_deref())?;
        let pacing_dim = self.check_pacing(&content)?;
        let payoff_dim = self.check_payoff(&scene.story_id, scene_id, scene.sequence_number)?;

        let mut dimensions = vec![
            continuity_dim,
            character_dim,
            style_dim,
            pacing_dim,
            payoff_dim,
        ];

        // 4. 完整审计：LLM 深度评估
        if effective_audit_type == "full" {
            if let Some(handle) = app_handle {
                let llm_dim = self.llm_deep_audit(&content, handle).await?;
                // 用 LLM 评估结果加权修正各维度
                self.merge_llm_dimensions(&mut dimensions, &llm_dim);
            }
        }

        // 5. 计算总分
        let overall_score = if dimensions.is_empty() {
            1.0
        } else {
            dimensions.iter().map(|d| d.score).sum::<f32>() / dimensions.len() as f32
        };

        let has_blocking_issues = dimensions
            .iter()
            .any(|d| d.issues.iter().any(|i| i.severity == "blocking"));

        Ok(AuditReport {
            scene_id: scene_id.to_string(),
            overall_score,
            dimensions,
            has_blocking_issues,
            audit_type: effective_audit_type.to_string(),
            content_word_count: content_len,
        })
    }

    // ==================== 各维度检查 ====================

    fn check_continuity(
        &self,
        story_id: &str,
        scene_id: &str,
        content: &str,
    ) -> Result<AuditDimension, AppError> {
        let engine = ContinuityEngine::new(self.pool.clone());
        let check = engine.check_scene_continuity(story_id, scene_id, content)?;

        let mut issues = Vec::new();
        let mut score = 1.0f32;

        for issue in &check.issues {
            let severity_str = match issue.severity {
                ContinuitySeverity::Critical => {
                    score -= 0.25;
                    "blocking"
                }
                ContinuitySeverity::Warning => {
                    score -= 0.1;
                    "warning"
                }
                ContinuitySeverity::Info => {
                    score -= 0.02;
                    "info"
                }
            };
            issues.push(AuditIssue {
                severity: severity_str.to_string(),
                message: issue.message.clone(),
                suggestion: issue.suggestion.clone(),
            });
        }

        Ok(AuditDimension {
            name: "continuity".to_string(),
            score: score.max(0.0),
            issues,
        })
    }

    fn check_character(&self, content: &str) -> Result<AuditDimension, AppError> {
        let checker = QualityChecker::new();
        let report = checker.check(content);

        let mut issues = Vec::new();

        // 从 QualityChecker 中提取 character 相关问题
        for qi in &report.issues {
            if matches!(
                qi.category,
                crate::creative_engine::workflow::quality::IssueCategory::Character
            ) {
                let severity_str = match qi.severity {
                    QualitySeverity::Critical => "blocking",
                    QualitySeverity::Major => "warning",
                    QualitySeverity::Moderate => "warning",
                    QualitySeverity::Minor => "info",
                };
                issues.push(AuditIssue {
                    severity: severity_str.to_string(),
                    message: qi.description.clone(),
                    suggestion: Some(qi.suggestion.clone()),
                });
            }
        }

        Ok(AuditDimension {
            name: "character".to_string(),
            score: report.character_consistency_score,
            issues,
        })
    }

    fn check_style(
        &self,
        content: &str,
        style_dna_id: Option<&str>,
    ) -> Result<AuditDimension, AppError> {
        let mut issues = Vec::new();

        // 获取目标 StyleDNA
        let target_dna = if let Some(id) = style_dna_id {
            let repo = StyleDnaRepository::new(self.pool.clone());
            match repo.get_by_id(id) {
                Ok(Some(db_dna)) => serde_json::from_str::<StyleDNA>(&db_dna.dna_json).ok(),
                _ => None,
            }
        } else {
            None
        };

        let style_score = if let Some(dna) = target_dna {
            let result = StyleChecker::check(content, &dna);
            for issue in &result.issues {
                issues.push(AuditIssue {
                    severity: "warning".to_string(),
                    message: issue.clone(),
                    suggestion: Some("尝试调整文风以匹配目标风格".to_string()),
                });
            }
            result.score
        } else {
            // 无目标风格时，用默认标准进行基础检查
            let default_dna = StyleAnalyzer::analyze_sample(content, "默认风格");
            let result = StyleChecker::check(content, &default_dna);
            result.score
        };

        // 引号一致性检查（额外）
        if content.contains('"') && content.contains('「') {
            issues.push(AuditIssue {
                severity: "warning".to_string(),
                message: "引号使用不一致：同时出现双引号和直角引号".to_string(),
                suggestion: Some("统一使用「」或 \"\" 作为对话引号".to_string()),
            });
        }

        Ok(AuditDimension {
            name: "style".to_string(),
            score: style_score,
            issues,
        })
    }

    fn check_pacing(&self, content: &str) -> Result<AuditDimension, AppError> {
        let checker = QualityChecker::new();
        let report = checker.check(content);

        let mut issues = Vec::new();
        let mut score = report.structure_score;

        // 段落长度检查
        let paragraphs: Vec<&str> = content
            .split('\n')
            .filter(|p| !p.trim().is_empty())
            .collect();
        let avg_para_len = if !paragraphs.is_empty() {
            content.chars().count() as f32 / paragraphs.len() as f32
        } else {
            0.0
        };
        if avg_para_len > 300.0 {
            score -= 0.1;
            issues.push(AuditIssue {
                severity: "warning".to_string(),
                message: "段落过长，可能影响阅读节奏".to_string(),
                suggestion: Some("建议每段控制在 200 字以内".to_string()),
            });
        }

        // 对话比例检查（过于密集或稀疏）
        let dialogue_markers = ['"', '「', '『'];
        let dialogue_count = content
            .chars()
            .filter(|&c| dialogue_markers.contains(&c))
            .count()
            / 2;
        let char_count = content.chars().count();
        let dialogue_ratio = if char_count > 0 {
            dialogue_count as f32 / char_count as f32
        } else {
            0.0
        };
        if dialogue_ratio > 0.5 {
            score -= 0.08;
            issues.push(AuditIssue {
                severity: "info".to_string(),
                message: "对话比例过高，可能缺乏叙事推进".to_string(),
                suggestion: Some("适当增加叙述和描写，平衡对话与叙事".to_string()),
            });
        } else if dialogue_ratio < 0.05 && char_count > 500 {
            score -= 0.05;
            issues.push(AuditIssue {
                severity: "info".to_string(),
                message: "对话比例过低，场景可能显得沉闷".to_string(),
                suggestion: Some("适当增加角色对话，增强场景活力".to_string()),
            });
        }

        // 时间线索检查
        let time_markers = ["早上", "中午", "晚上", "第二天", "后来", "之前", "与此同时"];
        let has_time = time_markers.iter().any(|&m| content.contains(m));
        if !has_time && char_count > 800 {
            issues.push(AuditIssue {
                severity: "info".to_string(),
                message: "缺乏明确的时间过渡线索".to_string(),
                suggestion: Some("适当增加时间词，帮助读者理解时间线".to_string()),
            });
        }

        Ok(AuditDimension {
            name: "pacing".to_string(),
            score: score.max(0.0),
            issues,
        })
    }

    fn check_payoff(
        &self,
        story_id: &str,
        _scene_id: &str,
        scene_number: i32,
    ) -> Result<AuditDimension, AppError> {
        let ledger = PayoffLedger::new(self.pool.clone());
        let items = ledger.get_ledger(story_id)?;

        let mut issues = Vec::new();
        let mut score = 1.0f32;

        // 检测逾期伏笔
        let overdue = ledger.detect_overdue(story_id, scene_number)?;
        for item in &overdue {
            score -= 0.08;
            issues.push(AuditIssue {
                severity: "warning".to_string(),
                message: format!(
                    "伏笔「{}」已逾期（重要性 {}/10）",
                    item.title, item.importance
                ),
                suggestion: Some(format!(
                    "建议在场景 {} 之前回收此伏笔",
                    item.target_end_scene
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "后续".to_string())
                )),
            });
        }

        // 检查当前场景是否有新伏笔被设置
        let setups_in_scene: Vec<_> = items
            .iter()
            .filter(|i| {
                i.first_seen_scene == Some(scene_number)
                    && matches!(
                        i.current_status,
                        crate::creative_engine::payoff_ledger::PayoffStatus::Setup
                    )
            })
            .collect();

        if !setups_in_scene.is_empty() {
            for item in &setups_in_scene {
                issues.push(AuditIssue {
                    severity: "info".to_string(),
                    message: format!(
                        "本场景设置了新伏笔：{}（重要性 {}/10）",
                        item.title, item.importance
                    ),
                    suggestion: Some("确保后续场景中有计划地回收此伏笔".to_string()),
                });
            }
        }

        // 检查当前场景是否回收了伏笔
        let payoffs_in_scene: Vec<_> = items
            .iter()
            .filter(|i| {
                i.last_touched_scene == Some(scene_number)
                    && matches!(
                        i.current_status,
                        crate::creative_engine::payoff_ledger::PayoffStatus::PaidOff
                    )
            })
            .collect();

        if !payoffs_in_scene.is_empty() {
            for item in &payoffs_in_scene {
                issues.push(AuditIssue {
                    severity: "info".to_string(),
                    message: format!("本场景回收了伏笔：{}", item.title),
                    suggestion: None,
                });
            }
        }

        // 如果有大量未回收伏笔，扣分
        let unresolved_count = items
            .iter()
            .filter(|i| {
                matches!(
                    i.current_status,
                    crate::creative_engine::payoff_ledger::PayoffStatus::Setup
                        | crate::creative_engine::payoff_ledger::PayoffStatus::Hinted
                        | crate::creative_engine::payoff_ledger::PayoffStatus::PendingPayoff
                )
            })
            .count();
        if unresolved_count > 10 {
            score -= 0.1;
            issues.push(AuditIssue {
                severity: "warning".to_string(),
                message: format!(
                    "故事中积累了 {} 个未回收伏笔，可能导致读者困惑",
                    unresolved_count
                ),
                suggestion: Some("建议整理伏笔回收计划，避免悬念过度堆积".to_string()),
            });
        }

        Ok(AuditDimension {
            name: "payoff".to_string(),
            score: score.max(0.0),
            issues,
        })
    }

    /// LLM 深度评估
    async fn llm_deep_audit(
        &self,
        content: &str,
        app_handle: &AppHandle,
    ) -> Result<AuditDimension, AppError> {
        let llm = LlmService::new(app_handle.clone());
        let checker = QualityChecker::new();
        let report = checker.check_with_llm(content, &llm).await?;

        let issues: Vec<AuditIssue> = report
            .issues
            .iter()
            .map(|qi| {
                let severity_str = match qi.severity {
                    QualitySeverity::Critical => "blocking",
                    QualitySeverity::Major => "warning",
                    QualitySeverity::Moderate => "warning",
                    QualitySeverity::Minor => "info",
                };
                AuditIssue {
                    severity: severity_str.to_string(),
                    message: qi.description.clone(),
                    suggestion: Some(qi.suggestion.clone()),
                }
            })
            .collect();

        Ok(AuditDimension {
            name: "llm_deep".to_string(),
            score: report.overall_score,
            issues,
        })
    }

    /// 合并 LLM 深度评估结果到各维度
    fn merge_llm_dimensions(&self, dimensions: &mut Vec<AuditDimension>, llm_dim: &AuditDimension) {
        // 用 LLM 总体评分对各维度进行轻微加权修正
        let llm_score = llm_dim.score;
        for dim in dimensions.iter_mut() {
            // 向 LLM 评分靠拢 20%
            dim.score = dim.score * 0.8 + llm_score * 0.2;
            dim.score = dim.score.clamp(0.0, 1.0);
        }

        // 将 LLM 发现的问题分配到对应维度
        for issue in &llm_dim.issues {
            let target = if issue.message.contains("结构") || issue.message.contains("节奏") {
                dimensions.iter_mut().find(|d| d.name == "pacing")
            } else if issue.message.contains("人物") || issue.message.contains("角色") {
                dimensions.iter_mut().find(|d| d.name == "character")
            } else if issue.message.contains("风格") || issue.message.contains("文笔") {
                dimensions.iter_mut().find(|d| d.name == "style")
            } else if issue.message.contains("连贯")
                || issue.message.contains("逻辑")
                || issue.message.contains("一致")
            {
                dimensions.iter_mut().find(|d| d.name == "continuity")
            } else if issue.message.contains("伏笔") || issue.message.contains("回收") {
                dimensions.iter_mut().find(|d| d.name == "payoff")
            } else {
                // 默认放到 continuity
                dimensions.iter_mut().find(|d| d.name == "continuity")
            };

            if let Some(target_dim) = target {
                target_dim.issues.push(issue.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_issue_creation() {
        let issue = AuditIssue {
            severity: "blocking".to_string(),
            message: "测试问题".to_string(),
            suggestion: Some("修复建议".to_string()),
        };
        assert_eq!(issue.severity, "blocking");
        assert_eq!(issue.suggestion, Some("修复建议".to_string()));
    }

    #[test]
    fn test_audit_dimension_scoring() {
        let dim = AuditDimension {
            name: "continuity".to_string(),
            score: 0.85,
            issues: vec![],
        };
        assert_eq!(dim.name, "continuity");
        assert!(dim.score >= 0.0 && dim.score <= 1.0);
    }
}
