//! Agent Orchestrator - Agent 协作编排器
//!
//! 实现 Agent 间的协作工作流，支持反馈闭环：
//! Writer → Inspector → Writer(改写) → ...
//!
//! 幕后运行，幕前只呈现最终结果。

use tauri::{AppHandle, Emitter, Manager};

use super::{
    service::{AgentService, AgentTask, AgentType},
    AgentResult,
};
use crate::{
    creative_engine::style::{StyleChecker, StyleDNA},
    db::{repositories::StyleDnaRepository, DbPool},
    error::AppError,
};

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
    /// 风格权重（0-1，默认 0.5）
    pub style_weight: f32,
    /// 叙事权重（0-1，默认 0.5）
    pub narrative_weight: f32,
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            rewrite_threshold: 0.75,
            max_feedback_loops: 2,
            keep_revision_history: true,
            style_weight: 0.5,
            narrative_weight: 0.5,
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
    /// 风格一致性评分（0-1）
    pub style_score: f32,
    /// 叙事推进评分（0-1）
    pub narrative_score: f32,
    /// 风格漂移详情
    pub drift_details: Vec<String>,
    /// 执行步骤记录
    pub steps: Vec<WorkflowStepResult>,
    /// 是否经过改写
    pub was_rewritten: bool,
    /// 改写次数
    pub rewrite_count: u32,
    /// 关联的 LLM request_id，供上层取消使用
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
    Generation, // 生成
    Inspection, // 质检
    Rewrite,    // 改写
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
        Self {
            service,
            config,
            app_handle,
        }
    }

    pub fn with_default_config(service: AgentService, app_handle: AppHandle) -> Self {
        Self::new(service, WorkflowConfig::default(), app_handle)
    }

    /// 发射工作流步骤事件到前端
    fn emit_step_event(
        &self,
        task_id: &str,
        step_type: WorkflowStepType,
        loop_idx: Option<u32>,
        score: Option<f32>,
    ) {
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
                let story_id = task.context.story.story_id.clone();
                let chapter_number = task.context.narrative.chapter_number;
                let input = task.input.clone();
                let skill_manager = skill_manager.clone();
                tauri::async_runtime::spawn(async move {
                    let context = crate::agents::AgentContext::minimal(story_id, input);
                    let data = serde_json::json!({ "chapter_number": chapter_number });
                    let _ = skill_manager
                        .execute_hooks(crate::skills::HookEvent::BeforeAiWrite, &context, data)
                        .await;
                    log::info!(
                        "[AgentOrchestrator] Hook executed: {:?}",
                        crate::skills::HookEvent::BeforeAiWrite
                    );
                });
            }
        }

        let result = match mode {
            GenerationMode::Fast => self.execute_fast(task.clone()).await,
            GenerationMode::Full => self.execute_full(task.clone()).await,
        };

        // v0.8.0: 自动写入记忆（创作完成后）
        if let Ok(ref workflow_result) = result {
            let pool = self.app_handle.state::<crate::db::DbPool>();
            let writer = crate::memory::writer::MemoryWriter::new(pool.inner().clone());
            let story_id = task.context.story.story_id.clone();
            let chapter_number = task.context.narrative.chapter_number as i32;
            let content = workflow_result.final_content.clone();
            tauri::async_runtime::spawn(async move {
                match writer.write(&story_id, chapter_number, &content).await {
                    Ok(_) => {
                        log::info!("[AgentOrchestrator] Memory updated for story {}", story_id)
                    }
                    Err(e) => log::warn!("[AgentOrchestrator] Memory write failed: {}", e),
                }
            });
        }

        // AfterAiWrite hook (only on success)
        if let Ok(ref workflow_result) = result {
            if let Some(manager) = crate::SKILL_MANAGER.get() {
                if let Ok(skill_manager) = manager.lock() {
                    let story_id = task.context.story.story_id.clone();
                    let chapter_number = task.context.narrative.chapter_number;
                    let content = workflow_result.final_content.clone();
                    let score_val = workflow_result.final_score;
                    let skill_manager = skill_manager.clone();
                    tauri::async_runtime::spawn(async move {
                        let context = crate::agents::AgentContext::minimal(story_id, content);
                        let data = serde_json::json!({ "chapter_number": chapter_number, "score": score_val });
                        let _ = skill_manager
                            .execute_hooks(crate::skills::HookEvent::AfterAiWrite, &context, data)
                            .await;
                        log::info!(
                            "[AgentOrchestrator] Hook executed: {:?}",
                            crate::skills::HookEvent::AfterAiWrite
                        );
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
            style_score: 0.0,
            narrative_score: 0.0,
            drift_details: Vec::new(),
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
    pub async fn execute_full(&self, task: AgentTask) -> Result<WorkflowResult, AppError> {
        let mut steps: Vec<WorkflowStepResult> = Vec::new();
        let mut rewrite_count: u32 = 0;
        let mut was_rewritten = false;

        // 步骤1: Writer 生成初稿
        self.emit_step_event(&task.id, WorkflowStepType::Generation, None, None);

        // v0.7.8: 3 候选并行生成选优（续写场景且有风格指纹时启用）
        let (writer_result, request_id, mut current_content) =
            if task.context.style.style_fingerprint.is_some()
                || task
                    .context
                    .narrative
                    .current_content
                    .as_ref()
                    .map(|c| c.len() > 100)
                    .unwrap_or(false)
            {
                let (r, req_id, content) = self.generate_candidates(&task, 3).await?;
                (r, Some(req_id), content)
            } else {
                let result = Box::pin(self.service.execute_writer_raw(task.clone())).await?;
                let req_id = result.request_id.clone();
                let content = result.content.clone();
                (result, req_id, content)
            };

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
            let base_inspect_score = inspect_result.score.unwrap_or(0.0);
            let mut style_issues = Vec::new();

            // v0.7.8: 双轨评分 — 从 Inspector JSON 响应中解析风格分数
            let (style_score, narrative_score, mut drift_details) =
                Self::parse_inspector_style_analysis(&inspect_result.content, base_inspect_score);

            // 综合分数 = 风格分 * style_weight + 叙事分 * narrative_weight
            let composite_score = style_score * self.config.style_weight
                + narrative_score * self.config.narrative_weight;

            // StyleChecker 验证（保留原有逻辑作为兜底）
            if let Some(ref blend) = task.context.style.style_blend {
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
                        let check_result =
                            StyleChecker::check_blend(&current_content, blend, &dnas);
                        if !check_result.passed {
                            style_issues = check_result.issues;
                        }
                    }
                }
            } else if let Some(ref style_id) = task.context.style.style_dna_id {
                let pool = self.app_handle.state::<DbPool>();
                {
                    let repo = StyleDnaRepository::new(pool.inner().clone());
                    if let Ok(Some(db_dna)) = repo.get_by_id(style_id) {
                        if let Ok(target_dna) = serde_json::from_str::<StyleDNA>(&db_dna.dna_json) {
                            let check_result = StyleChecker::check(&current_content, &target_dna);
                            if !check_result.passed {
                                style_issues = check_result.issues;
                            }
                        }
                    }
                }
            }

            self.emit_step_event(
                &task.id,
                WorkflowStepType::Inspection,
                Some(loop_idx),
                Some(composite_score),
            );

            let mut all_suggestions = inspect_result.suggestions.clone();
            all_suggestions.extend(style_issues);
            all_suggestions.extend(drift_details.clone());

            steps.push(WorkflowStepResult {
                step_type: WorkflowStepType::Inspection,
                agent_type: AgentType::Inspector,
                content: inspect_result.content.clone(),
                score: Some(composite_score),
                suggestions: all_suggestions,
            });

            // v0.7.8: 双轨达标判断
            let style_ok = style_score >= 0.70;
            let narrative_ok = narrative_score >= self.config.rewrite_threshold;

            if style_ok && narrative_ok {
                // 双达标，通过
                return Ok(WorkflowResult {
                    final_content: current_content,
                    final_score: composite_score,
                    style_score,
                    narrative_score,
                    drift_details,
                    steps,
                    was_rewritten,
                    rewrite_count,
                    request_id: request_id.clone(),
                });
            }

            // 需要改写，准备双轨反馈
            let feedback = Self::build_rewrite_feedback_dual(
                &inspect_result,
                style_score,
                narrative_score,
                &drift_details,
                self.config.style_weight,
            );
            was_rewritten = true;
            rewrite_count += 1;

            // 步骤3: Writer 改写
            self.emit_step_event(&task.id, WorkflowStepType::Rewrite, Some(loop_idx), None);
            let mut rewrite_context = task.context.clone();
            rewrite_context.narrative.selected_text = Some(current_content.clone());
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

            self.emit_step_event(
                &task.id,
                WorkflowStepType::Rewrite,
                Some(loop_idx),
                rewrite_result.score,
            );

            steps.push(WorkflowStepResult {
                step_type: WorkflowStepType::Rewrite,
                agent_type: AgentType::Writer,
                content: current_content.clone(),
                score: rewrite_result.score,
                suggestions: rewrite_result.suggestions.clone(),
            });
        }

        // 达到最大循环次数，返回最后一次结果
        let final_step = steps
            .iter()
            .filter(|s| s.step_type == WorkflowStepType::Inspection)
            .last();
        let final_score = final_step.and_then(|s| s.score).unwrap_or(0.0);
        // 提取最后一次的风格分数（从步骤建议中推断）
        let (last_style_score, last_narrative_score, last_drift) =
            if let Some(ref content) = final_step.map(|s| s.content.clone()) {
                Self::parse_inspector_style_analysis(content, final_score)
            } else {
                (0.0, final_score, Vec::new())
            };

        Ok(WorkflowResult {
            final_content: current_content,
            final_score,
            style_score: last_style_score,
            narrative_score: last_narrative_score,
            drift_details: last_drift,
            steps,
            was_rewritten,
            rewrite_count,
            request_id,
        })
    }

    /// 从 Inspector JSON 响应中解析风格分析（v0.7.8）和记忆分析（v0.8.0）
    fn parse_inspector_style_analysis(
        content: &str,
        fallback_score: f32,
    ) -> (f32, f32, Vec<String>) {
        // 尝试从 content 中提取 JSON
        let json_str = Self::extract_json_from_content(content);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
            let style_score = json
                .get("style_analysis")
                .and_then(|s| s.get("style_score"))
                .and_then(|s| s.as_f64())
                .map(|s| (s as f32 / 100.0).min(1.0))
                .unwrap_or(fallback_score);

            // v0.8.0: 叙事分包含 memory 维度
            let narrative_score = json
                .get("dimension_scores")
                .and_then(|d| {
                    let logic = d.get("logic").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let character = d.get("character").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let writing = d.get("writing").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let pacing = d.get("pacing").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let world = d.get("world").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let memory = d.get("memory").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let total = logic + character + writing + pacing + world + memory;
                    Some((total as f32 / 125.0).min(1.0)) // 6维度总分125，归一化到0-1
                })
                .unwrap_or(fallback_score);

            let mut drift_details: Vec<String> = json
                .get("style_analysis")
                .and_then(|s| s.get("function_word_drift"))
                .and_then(|f| f.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            // v0.8.0: 提取记忆冲突并合并到 drift_details
            if let Some(mem) = json.get("memory_analysis") {
                if let Some(conflicts) = mem.get("character_conflicts").and_then(|c| c.as_array()) {
                    for c in conflicts {
                        if let Some(s) = c.as_str() {
                            drift_details.push(format!("[记忆-角色] {}", s));
                        }
                    }
                }
                if let Some(misses) = mem.get("foreshadowing_misses").and_then(|c| c.as_array()) {
                    for c in misses {
                        if let Some(s) = c.as_str() {
                            drift_details.push(format!("[记忆-伏笔] {}", s));
                        }
                    }
                }
                if let Some(violations) =
                    mem.get("world_rule_violations").and_then(|c| c.as_array())
                {
                    for c in violations {
                        if let Some(s) = c.as_str() {
                            drift_details.push(format!("[记忆-世界观] {}", s));
                        }
                    }
                }
                if let Some(issues) = mem.get("timeline_issues").and_then(|c| c.as_array()) {
                    for c in issues {
                        if let Some(s) = c.as_str() {
                            drift_details.push(format!("[记忆-时间线] {}", s));
                        }
                    }
                }
            }

            return (style_score, narrative_score, drift_details);
        }

        // 解析失败，回退到 base score
        (fallback_score, fallback_score, Vec::new())
    }

    /// 从文本中提取 JSON（支持 markdown 代码块）
    fn extract_json_from_content(content: &str) -> String {
        let trimmed = content.trim();
        if let Some(start) = trimmed.find("```") {
            let after_start = &trimmed[start + 3..];
            let code_start = if after_start.starts_with("json") {
                after_start[4..].trim_start()
            } else {
                after_start.trim_start()
            };
            if let Some(end) = code_start.find("```") {
                return code_start[..end].trim().to_string();
            }
        }
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                if end > start {
                    return trimmed[start..=end].to_string();
                }
            }
        }
        trimmed.to_string()
    }

    /// 构建双轨改写反馈指令（v0.7.8）
    fn build_rewrite_feedback_dual(
        inspect_result: &AgentResult,
        style_score: f32,
        narrative_score: f32,
        drift_details: &[String],
        style_weight: f32,
    ) -> String {
        let mut feedback = String::from("【质检反馈】\n");

        feedback.push_str(&format!(
            "叙事评分: {:.0}% | 风格评分: {:.0}%\n",
            narrative_score * 100.0,
            style_score * 100.0
        ));

        // 判断哪个方向问题更严重
        let style_worse = style_score < narrative_score;
        let priority = if style_worse && style_weight >= 0.6 {
            "风格优先"
        } else if !style_worse && style_weight <= 0.4 {
            "叙事优先"
        } else {
            "平衡调整"
        };
        feedback.push_str(&format!("调整方向: {}\n", priority));

        // 风格漂移详情
        if !drift_details.is_empty() {
            feedback.push_str("\n【风格问题】\n");
            for detail in drift_details {
                feedback.push_str(&format!("- {}\n", detail));
            }
        }

        if !inspect_result.suggestions.is_empty() {
            feedback.push_str("\n【叙事/文笔问题】\n");
            for (i, suggestion) in inspect_result.suggestions.iter().enumerate() {
                feedback.push_str(&format!("{}. {}\n", i + 1, suggestion));
            }
        }

        if style_worse {
            feedback.push_str(
                "\n【重点】本次改写请优先解决风格一致性问题。保持与参考文本相同的句长分布、\
                 虚词偏好和四字格密度，宁可放慢叙事节奏也要保证语言风格统一。",
            );
        } else {
            feedback.push_str(
                "\n【重点】本次改写请优先解决叙事和文笔问题。在保持现有语言风格的基础上，\
                 改进情节连贯性和描写质量。",
            );
        }

        feedback
    }

    /// 构建改写反馈指令（保留原有单轨版本作为兼容）
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

    /// v0.7.8: 并行生成 N 个候选，用风格指纹打分选优
    ///
    /// 每个候选使用不同的 temperature 产生多样性：
    /// - 候选1: base_temp * 0.9 (更保守，接近训练分布)
    /// - 候选2: base_temp * 1.0 (基准)
    /// - 候选3: base_temp * 1.1 (更发散，探索性)
    async fn generate_candidates(
        &self,
        task: &AgentTask,
        count: usize,
    ) -> Result<(AgentResult, String, String), AppError> {
        let temps = [0.75_f32, 0.9_f32, 1.05_f32];
        let mut tasks = Vec::with_capacity(count);

        for i in 0..count {
            let mut candidate_task = task.clone();
            candidate_task.id = format!("{}-candidate-{}", task.id, i);
            candidate_task.parameters.insert(
                "temperature_override".to_string(),
                serde_json::json!(temps.get(i).copied().unwrap_or(0.9)),
            );
            tasks.push(candidate_task);
        }

        log::info!(
            "[Orchestrator] Generating {} candidates for task {}",
            count,
            task.id
        );

        // 并行执行所有候选
        let results: Vec<Result<AgentResult, AppError>> = futures::future::join_all(
            tasks
                .into_iter()
                .map(|t| self.service.execute_writer_raw(t)),
        )
        .await;

        // 获取参考指纹（从预计算或 current_content 提取）
        let reference_fp = task.context.style.style_fingerprint.clone().or_else(|| {
            task.context
                .narrative
                .current_content
                .as_ref()
                .and_then(|c| {
                    let cleaned = c.trim().trim_start_matches("...").trim_start();
                    let cleaned = if cleaned.starts_with('(') && cleaned.contains("已省略)") {
                        cleaned
                            .split_once('\n')
                            .map(|(_, rest)| rest)
                            .unwrap_or(cleaned)
                            .trim_start()
                    } else {
                        cleaned
                    };
                    if cleaned.len() > 50 {
                        Some(
                            crate::creative_engine::style::fingerprint::StyleFingerprint::from_text(
                                cleaned,
                            ),
                        )
                    } else {
                        None
                    }
                })
        });

        // 评分并选优
        let mut best_idx = 0_usize;
        let mut best_score = -1.0_f32;
        let mut candidates: Vec<(AgentResult, f32)> = Vec::with_capacity(count);

        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(agent_result) => {
                    let score = if let Some(ref ref_fp) = reference_fp {
                        Self::score_candidate_style(&agent_result.content, ref_fp)
                    } else {
                        agent_result.score.unwrap_or(0.5)
                    };
                    candidates.push((agent_result, score));
                    if score > best_score {
                        best_score = score;
                        best_idx = i;
                    }
                    log::info!("[Orchestrator] Candidate {} style_score={:.2}", i, score);
                }
                Err(e) => {
                    log::warn!("[Orchestrator] Candidate {} failed: {}", i, e);
                }
            }
        }

        if candidates.is_empty() {
            return Err(AppError::internal("所有候选生成均失败"));
        }

        let best = candidates.swap_remove(best_idx);
        log::info!(
            "[Orchestrator] Selected candidate {} with score {:.2} (from {} valid)",
            best_idx,
            best_score,
            candidates.len() + 1
        );

        let req_id = best.0.request_id.clone().unwrap_or_default();
        let content = best.0.content.clone();
        Ok((best.0, req_id, content))
    }

    /// 用风格指纹对候选文本打分（0-1，越高越匹配）
    fn score_candidate_style(
        text: &str,
        reference: &crate::creative_engine::style::fingerprint::StyleFingerprint,
    ) -> f32 {
        let candidate =
            crate::creative_engine::style::fingerprint::StyleFingerprint::from_text(text);

        // 句长匹配度 (0-1)
        let len_match = if reference.syntax.avg_sentence_length > 0.0 {
            let diff =
                (candidate.syntax.avg_sentence_length - reference.syntax.avg_sentence_length).abs();
            let ratio = diff / reference.syntax.avg_sentence_length;
            (1.0 - ratio).clamp(0.0, 1.0)
        } else {
            0.5
        };

        // 四字格密度匹配度 (0-1)
        let four_char_match = {
            let diff = (candidate.vocabulary.four_char_density
                - reference.vocabulary.four_char_density)
                .abs();
            (1.0 - diff / 20.0).clamp(0.0, 1.0) // 20% 为最大容忍偏离
        };

        // 虚词偏好匹配度 — 计算前5虚词的重叠率
        let function_word_match = {
            let ref_top: std::collections::HashSet<&String> = reference
                .vocabulary
                .function_words
                .iter()
                .map(|(w, _)| w)
                .collect();
            let cand_top: std::collections::HashSet<&String> = candidate
                .vocabulary
                .function_words
                .iter()
                .map(|(w, _)| w)
                .collect();
            if !ref_top.is_empty() {
                let overlap = ref_top.intersection(&cand_top).count() as f32;
                overlap / ref_top.len() as f32
            } else {
                0.5
            }
        };

        // 加权综合（句长 40% + 四字格 35% + 虚词 25%）
        let score = len_match * 0.4 + four_char_match * 0.35 + function_word_match * 0.25;
        score.clamp(0.0, 1.0)
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
            suggestions: vec!["对话不够自然".to_string(), "角色动机不充分".to_string()],
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
            style_score: 0.8,
            narrative_score: 0.9,
            drift_details: vec![],
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
