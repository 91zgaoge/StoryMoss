//! Agent Orchestrator - Agent 协作编排器
//!
//! 实现 Agent 间的协作工作流，支持反馈闭环：
//! Writer → Inspector → Writer(改写) → ...
//!
//! 幕后运行，幕前只呈现最终结果。

use super::AgentResult;
use super::service::{AgentService, AgentTask, AgentType};
use crate::db::DbPool;
use crate::db::repositories_v3::StyleDnaRepository;
use crate::creative_engine::style::{StyleChecker, StyleDNA};
use crate::error::AppError;
use tauri::{AppHandle, Emitter, Manager};

/// 生成模式 — 决定 Orchestrator 执行路径
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationMode {
    /// 快速模式：单轮 LLM，跳过 Inspector / StyleChecker
    /// 适用于 Ghost Text、实时补全等低延迟场景
    Fast,
    /// 完整模式：Writer → Inspector → Writer 反馈闭环
    /// 适用于标准写作、章节生成等高质量场景
    Full,
}

impl GenerationMode {
    pub fn name(&self) -> &'static str {
        match self {
            GenerationMode::Fast => "快速",
            GenerationMode::Full => "完整",
        }
    }
}

/// 工作流配置
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    /// 触发改写的质检分数阈值 (0.0 - 1.0)
    pub rewrite_threshold: f32,
    /// 最大反馈循环次数
    pub max_feedback_loops: u32,
    /// 是否在循环中保留历史版本
    pub keep_revision_history: bool,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            rewrite_threshold: 0.75,
            max_feedback_loops: 2,
            keep_revision_history: true,
        }
    }
}

/// 工作流执行结果
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    /// 最终内容
    pub final_content: String,
    /// 质检评分（最后一次）
    pub final_score: f32,
    /// 执行步骤记录
    pub steps: Vec<WorkflowStepResult>,
    /// 是否经过改写
    pub was_rewritten: bool,
    /// 改写次数
    pub rewrite_count: u32,
    /// 关联的 LLM request_id，供上层取消使用（v0.7.2）
    pub request_id: Option<String>,
}

/// 单个步骤的执行结果
#[derive(Debug, Clone)]
pub struct WorkflowStepResult {
    pub step_type: WorkflowStepType,
    pub agent_type: AgentType,
    pub content: String,
    pub score: Option<f32>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStepType {
    Generation,  // 生成
    Inspection,  // 质检
    Rewrite,     // 改写
}

impl WorkflowStepType {
    pub fn name(&self) -> &'static str {
        match self {
            WorkflowStepType::Generation => "生成",
            WorkflowStepType::Inspection => "质检",
            WorkflowStepType::Rewrite => "改写",
        }
    }
}

/// Agent 编排器
pub struct AgentOrchestrator {
    service: AgentService,
    config: WorkflowConfig,
    app_handle: AppHandle,
}

impl AgentOrchestrator {
    pub fn new(service: AgentService, config: WorkflowConfig, app_handle: AppHandle) -> Self {
        Self { service, config, app_handle }
    }

    pub fn with_default_config(service: AgentService, app_handle: AppHandle) -> Self {
        Self::new(service, WorkflowConfig::default(), app_handle)
    }

    /// 发射工作流步骤事件到前端
    fn emit_step_event(&self, task_id: &str, step_type: WorkflowStepType, loop_idx: Option<u32>, score: Option<f32>) {
        let event = serde_json::json!({
            "task_id": task_id,
            "step_type": step_type.name(),
            "loop_idx": loop_idx,
            "score": score.map(|s| (s * 100.0) as i32),
        });
        let _ = self.app_handle.emit("orchestrator-step", event);
    }

    /// 统一生成入口 — 根据 GenerationMode 选择执行路径
    ///
    /// - Fast: 单轮 Writer 生成，跳过质检，最低延迟
    /// - Full: Writer → Inspector → Writer 完整反馈闭环
    pub async fn generate(
        &self,
        task: AgentTask,
        mode: GenerationMode,
    ) -> Result<WorkflowResult, AppError> {
        // BeforeAiWrite hook
        if let Some(manager) = crate::SKILL_MANAGER.get() {
            if let Ok(skill_manager) = manager.lock() {
                let story_id = task.context.story_id.clone();
                let chapter_number = task.context.chapter_number;
                let input = task.input.clone();
                let skill_manager = skill_manager.clone();
                tauri::async_runtime::spawn(async move {
                    let context = crate::agents::AgentContext::minimal(story_id, input);
                    let data = serde_json::json!({ "chapter_number": chapter_number });
                    let _ = skill_manager.execute_hooks(crate::skills::HookEvent::BeforeAiWrite, &context, data).await;
                    log::info!("[AgentOrchestrator] Hook executed: {:?}", crate::skills::HookEvent::BeforeAiWrite);
                });
            }
        }

        let result = match mode {
            GenerationMode::Fast => self.execute_fast(task.clone()).await,
            GenerationMode::Full => self.execute_full(task.clone()).await,
        };

        // AfterAiWrite hook (only on success)
        if let Ok(ref workflow_result) = result {
            if let Some(manager) = crate::SKILL_MANAGER.get() {
                if let Ok(skill_manager) = manager.lock() {
                    let story_id = task.context.story_id.clone();
                    let chapter_number = task.context.chapter_number;
                    let content = workflow_result.final_content.clone();
                    let score_val = workflow_result.final_score;
                    let skill_manager = skill_manager.clone();
                    tauri::async_runtime::spawn(async move {
                        let context = crate::agents::AgentContext::minimal(story_id, content);
                        let data = serde_json::json!({ "chapter_number": chapter_number, "score": score_val });
                        let _ = skill_manager.execute_hooks(crate::skills::HookEvent::AfterAiWrite, &context, data).await;
                        log::info!("[AgentOrchestrator] Hook executed: {:?}", crate::skills::HookEvent::AfterAiWrite);
                    });
                }
            }
        }

        result
    }

    /// Fast 模式：单轮 LLM 生成，跳过 Inspector / StyleChecker
    async fn execute_fast(&self, task: AgentTask) -> Result<WorkflowResult, AppError> {
        self.emit_step_event(&task.id, WorkflowStepType::Generation, None, None);
        let writer_result = Box::pin(self.service.execute_writer_raw(task.clone())).await?;

        let steps = vec![WorkflowStepResult {
            step_type: WorkflowStepType::Generation,
            agent_type: AgentType::Writer,
            content: writer_result.content.clone(),
            score: writer_result.score,
            suggestions: writer_result.suggestions.clone(),
        }];

        Ok(WorkflowResult {
            final_content: writer_result.content,
            final_score: writer_result.score.unwrap_or(1.0),
            steps,
            was_rewritten: false,
            rewrite_count: 0,
            request_id: writer_result.request_id,
        })
    }

    /// Full 模式：Writer → Inspector → Writer 反馈闭环
    ///
    /// 流程：
    /// 1. Writer 生成初稿
    /// 2. Inspector 质检
    /// 3. 如果分数 < threshold，将质检反馈传给 Writer 改写
    /// 4. 重复 2-3 直到分数达标或达到最大循环次数
    pub async fn execute_full(
        &self,
        task: AgentTask,
    ) -> Result<WorkflowResult, AppError> {
        let mut steps: Vec<WorkflowStepResult> = Vec::new();
        let mut rewrite_count: u32 = 0;
        let mut was_rewritten = false;

        // 步骤1: Writer 生成初稿
        // v5.3.1: 调用 execute_writer_raw 而不是 execute_task，切断递归链
        self.emit_step_event(&task.id, WorkflowStepType::Generation, None, None);
        let writer_result = Box::pin(self.service.execute_writer_raw(task.clone())).await?;
        let request_id = writer_result.request_id.clone();
        let mut current_content = writer_result.content.clone();

        steps.push(WorkflowStepResult {
            step_type: WorkflowStepType::Generation,
            agent_type: AgentType::Writer,
            content: current_content.clone(),
            score: writer_result.score,
            suggestions: writer_result.suggestions.clone(),
        });

        // 反馈循环
        for loop_idx in 0..self.config.max_feedback_loops {
            // 步骤2: Inspector 质检
            self.emit_step_event(&task.id, WorkflowStepType::Inspection, Some(loop_idx), None);
            let inspect_task = AgentTask {
                id: format!("{}-inspect-{}", task.id, loop_idx),
                agent_type: AgentType::Inspector,
                context: task.context.clone(),
                input: current_content.clone(),
                parameters: task.parameters.clone(),
                tier: None,
            };

            let inspect_result = Box::pin(self.service.execute_task(inspect_task)).await?;
            let mut inspect_score = inspect_result.score.unwrap_or(0.0);
            let mut style_issues = Vec::new();

            // StyleChecker 验证：支持混合风格和单一 DNA
            if let Some(ref blend) = task.context.style_blend {
                let pool = self.app_handle.state::<DbPool>();
                {
                    let dna_repo = StyleDnaRepository::new(pool.inner().clone());
                    let mut dnas = Vec::new();
                    for comp in &blend.components {
                        if let Ok(Some(db_dna)) = dna_repo.get_by_id(&comp.dna_id) {
                            if let Ok(dna) = serde_json::from_str::<StyleDNA>(&db_dna.dna_json) {
                                dnas.push(dna);
                            }
                        }
                    }
                    if !dnas.is_empty() {
                        let check_result = StyleChecker::check_blend(&current_content, blend, &dnas);
                        if !check_result.passed {
                            style_issues = check_result.issues;
                            inspect_score = inspect_score
                                .min(self.config.rewrite_threshold - 0.01)
                                .max(0.0);
                        }
                    }
                }
            } else if let Some(ref style_id) = task.context.style_dna_id {
                let pool = self.app_handle.state::<DbPool>();
                {
                    let repo = StyleDnaRepository::new(pool.inner().clone());
                    if let Ok(Some(db_dna)) = repo.get_by_id(style_id) {
                        if let Ok(target_dna) = serde_json::from_str::<StyleDNA>(&db_dna.dna_json) {
                            let check_result = StyleChecker::check(&current_content, &target_dna);
                            if !check_result.passed {
                                style_issues = check_result.issues;
                                // 降低分数以确保触发改写
                                inspect_score = inspect_score
                                    .min(self.config.rewrite_threshold - 0.01)
                                    .max(0.0);
                            }
                        }
                    }
                }
            }

            self.emit_step_event(&task.id, WorkflowStepType::Inspection, Some(loop_idx), Some(inspect_score));

            let mut all_suggestions = inspect_result.suggestions.clone();
            all_suggestions.extend(style_issues);

            steps.push(WorkflowStepResult {
                step_type: WorkflowStepType::Inspection,
                agent_type: AgentType::Inspector,
                content: inspect_result.content.clone(),
                score: Some(inspect_score),
                suggestions: all_suggestions,
            });

            // 检查是否达标
            if inspect_score >= self.config.rewrite_threshold {
                // 质检通过，结束循环
                return Ok(WorkflowResult {
                    final_content: current_content,
                    final_score: inspect_score,
                    steps,
                    was_rewritten,
                    rewrite_count,
                    request_id: request_id.clone(),
                });
            }

            // 需要改写，准备反馈
            let feedback = Self::build_rewrite_feedback(&inspect_result);
            was_rewritten = true;
            rewrite_count += 1;

            // 步骤3: Writer 改写
            self.emit_step_event(&task.id, WorkflowStepType::Rewrite, Some(loop_idx), None);
            // v5.4.0: 将初稿内容放入 selected_text，确保 build_writer_prompt 使用 writer_rewrite_template
            let mut rewrite_context = task.context.clone();
            rewrite_context.selected_text = Some(current_content.clone());
            let rewrite_task = AgentTask {
                id: format!("{}-rewrite-{}", task.id, loop_idx),
                agent_type: AgentType::Writer,
                context: rewrite_context,
                input: feedback,
                parameters: {
                    let mut params = task.parameters.clone();
                    params.insert(
                        "original_content".to_string(),
                        serde_json::Value::String(current_content.clone()),
                    );
                    params.insert(
                        "rewrite_round".to_string(),
                        serde_json::Value::Number((loop_idx + 1).into()),
                    );
                    params
                },
                tier: None,
            };

            let rewrite_result = Box::pin(self.service.execute_task(rewrite_task)).await?;
            current_content = rewrite_result.content.clone();

            self.emit_step_event(&task.id, WorkflowStepType::Rewrite, Some(loop_idx), rewrite_result.score);

            steps.push(WorkflowStepResult {
                step_type: WorkflowStepType::Rewrite,
                agent_type: AgentType::Writer,
                content: current_content.clone(),
                score: rewrite_result.score,
                suggestions: rewrite_result.suggestions.clone(),
            });
        }

        // 达到最大循环次数，返回最后一次结果
        let final_score = steps
            .iter()
            .filter(|s| s.step_type == WorkflowStepType::Inspection)
            .last()
            .and_then(|s| s.score)
            .unwrap_or(0.0);

        Ok(WorkflowResult {
            final_content: current_content,
            final_score,
            steps,
            was_rewritten,
            rewrite_count,
            request_id,
        })
    }

    /// 构建改写反馈指令
    fn build_rewrite_feedback(inspect_result: &AgentResult) -> String {
        let mut feedback = String::from("【质检反馈】\n");

        if let Some(score) = inspect_result.score {
            feedback.push_str(&format!("质检评分: {:.0}%\n", score * 100.0));
        }

        if !inspect_result.suggestions.is_empty() {
            feedback.push_str("\n需要改进的方面：\n");
            for (i, suggestion) in inspect_result.suggestions.iter().enumerate() {
                feedback.push_str(&format!("{}. {}\n", i + 1, suggestion));
            }
        }

        feedback.push_str("\n请根据以上反馈改写内容，重点解决指出的问题，同时保持原文的优点。");
        feedback
    }

    /// 获取质检报告摘要
    pub fn generate_inspection_summary(result: &WorkflowResult) -> String {
        let mut summary = String::new();

        summary.push_str(&format!("质检评分: {:.0}%\n", result.final_score * 100.0));
        summary.push_str(&format!("改写次数: {}\n", result.rewrite_count));

        if result.was_rewritten {
            summary.push_str("状态: 经过质检反馈循环优化\n");
        } else {
            summary.push_str("状态: 初稿通过质检\n");
        }

        // 收集所有质检建议
        let all_suggestions: Vec<String> = result
            .steps
            .iter()
            .filter(|s| s.step_type == WorkflowStepType::Inspection)
            .flat_map(|s| s.suggestions.clone())
            .collect();

        if !all_suggestions.is_empty() {
            summary.push_str("\n质检建议：\n");
            for suggestion in all_suggestions {
                summary.push_str(&format!("- {}\n", suggestion));
            }
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_config_default() {
        let config = WorkflowConfig::default();
        assert_eq!(config.rewrite_threshold, 0.75);
        assert_eq!(config.max_feedback_loops, 2);
        assert!(config.keep_revision_history);
    }

    #[test]
    fn test_build_rewrite_feedback() {
        let inspect_result = AgentResult {
            content: "质检报告".to_string(),
            score: Some(0.6),
            suggestions: vec![
                "对话不够自然".to_string(),
                "角色动机不充分".to_string(),
            ],
            request_id: None,
        };

        let feedback = AgentOrchestrator::build_rewrite_feedback(&inspect_result);
        assert!(feedback.contains("质检评分"));
        assert!(feedback.contains("对话不够自然"));
        assert!(feedback.contains("角色动机不充分"));
        assert!(feedback.contains("请根据以上反馈改写"));
    }

    #[test]
    fn test_generate_inspection_summary() {
        let result = WorkflowResult {
            final_content: "最终内容".to_string(),
            final_score: 0.85,
            steps: vec![
                WorkflowStepResult {
                    step_type: WorkflowStepType::Generation,
                    agent_type: AgentType::Writer,
                    content: "初稿".to_string(),
                    score: None,
                    suggestions: vec![],
                },
                WorkflowStepResult {
                    step_type: WorkflowStepType::Inspection,
                    agent_type: AgentType::Inspector,
                    content: "质检".to_string(),
                    score: Some(0.85),
                    suggestions: vec!["建议1".to_string()],
                },
            ],
            was_rewritten: false,
            rewrite_count: 0,
            request_id: None,
        };

        let summary = AgentOrchestrator::generate_inspection_summary(&result);
        assert!(summary.contains("85%"));
        assert!(summary.contains("初稿通过质检"));
        assert!(summary.contains("建议1"));
    }
}
