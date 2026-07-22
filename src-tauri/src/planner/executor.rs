//! PlanExecutor - Dumb executor that faithfully runs LLM-generated plans
//!
//! All intelligence is in the plan. This executor just follows instructions.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use super::{ExecutionPlan, PlanContext, PlanExecutorProgress, PlanGenerator, PlanStep};
use crate::{
    capabilities::{CapabilityEvolutionEngine, ExecutionRecord},
    error::AppError,
    intention_graph::IntentionGraphPlanner,
    planner::PlanTemplateLibrary,
    router::TaskType,
};

#[derive(Debug, Clone, Serialize)]
pub struct PlanExecutionResult {
    pub success: bool,
    pub steps_completed: usize,
    pub final_content: Option<String>,
    pub messages: Vec<String>,
    /// 若计划执行过程中产生可恢复的结构化错误（如 LLM_TIMEOUT），透传给前端。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

pub struct PlanExecutor {
    app_handle: AppHandle,
    pool: crate::db::DbPool,
    template_library: Mutex<PlanTemplateLibrary>,
    evolution_engine: CapabilityEvolutionEngine,
    intention_graph_planner: Option<IntentionGraphPlanner>,
}

impl PlanExecutor {
    pub fn new(app_handle: AppHandle) -> Self {
        let pool = app_handle.state::<crate::db::DbPool>().inner().clone();
        let llm_service = crate::llm::LlmService::new(app_handle.clone());
        let evolution_engine = CapabilityEvolutionEngine::new(llm_service, &app_handle);

        // 尝试初始化意图图规划器（SING 集成）
        let intention_graph_planner =
            IntentionGraphPlanner::from_app_handle(app_handle.clone()).ok();

        if intention_graph_planner.is_some() {
            log::info!(
                "[PlanExecutor] IntentionGraphPlanner initialized (SING integration active)"
            );
        } else {
            log::warn!("[PlanExecutor] IntentionGraphPlanner not available, using legacy PlanGenerator only");
        }

        Self {
            app_handle,
            pool: pool.clone(),
            template_library: Mutex::new(PlanTemplateLibrary::new(pool)),
            evolution_engine,
            intention_graph_planner,
        }
    }

    /// v0.30.11: 模板重放已禁用。`PlanTemplateLibrary::find_match`
    /// 用朴素子串匹配 `user_input.contains(p)`，且 trigger_patterns 来自
    /// LLM understanding 文本经 `split_whitespace` 切词（中文整句变一个
    /// pattern），本质噪声匹配，任何匹配器 都无法可靠工作（曾导致"
    /// 继续写当前这部小说"命中"这部小说"重放错误的 style_enhancer
    /// 计划，返回"请提供文本"而非正文）。意图路由已改由 LLM 分类器
    /// （`classify_writing_intent`）负责。缺失时直接走 planner LLM，最坏 +一次
    /// LLM 往返。DB 表与 `record_success` 保留供未来严格匹配观测。
    pub fn find_template(&self, _user_input: &str) -> Option<ExecutionPlan> {
        None
    }

    /// Adapt a template plan to the current context by replacing placeholders
    fn adapt_template_plan(&self, template: ExecutionPlan, context: &PlanContext) -> ExecutionPlan {
        let mut plan = template;
        if let Some(story_id) = &context.current_story_id {
            for step in &mut plan.steps {
                for value in step.parameters.values_mut() {
                    if let Some(s) = value.as_str() {
                        if s.contains("{{story_id}}") {
                            *value = serde_json::Value::String(s.replace("{{story_id}}", story_id));
                        }
                    }
                }
            }
        }
        plan
    }

    /// Execute a plan, checking the template library first
    pub async fn execute_with_context(
        &self,
        context: &PlanContext,
    ) -> Result<PlanExecutionResult, AppError> {
        log::info!("[PlanExecutor] execute_with_context START");
        // v0.23 TriShot 快速路径：当 AppConfig.generation_mode == "tri_shot" 时，
        // 跳过计划生成 LLM（Call 1 路由合成器替代），直接构造单步 writer plan。
        let app_dir = self.app_handle.path().app_data_dir().unwrap_or_default();
        let generation_mode = crate::config::AppConfig::load(&app_dir)
            .map(|c| c.generation_mode.clone())
            .unwrap_or_else(|_| "auto".to_string());
        let is_trishot = generation_mode == "tri_shot" || generation_mode == "trishot";

        // v0.30.11: 用 LLM 分类的 is_new_novel 替代 is_novel_creation_intent 子串匹配。
        let is_new_novel = context
            .intent_classification
            .as_ref()
            .map(|c| c.is_new_novel)
            .unwrap_or(false);
        if is_trishot && !is_new_novel {
            log::info!("[PlanExecutor] TriShot 快速路径：跳过计划生成，直接 writer step");
            let plan = Self::make_trishot_plan(context);
            return Ok(self.execute_plan(plan, context).await);
        }

        // Before generating a new plan, check PlanTemplateLibrary for matching
        // templates.
        // v0.30.11: 模板重放已禁用（find_template 恒返回 None，见其注释）。
        // 续写意图仍记录日志便于诊断；分类经 PlanContext 贯穿。
        let is_continuation = context
            .intent_classification
            .as_ref()
            .map(|c| c.is_continuation)
            .unwrap_or(false);
        let template_plan = if is_continuation {
            log::info!(
                "[PlanExecutor] 续写意图检测到，跳过模板匹配，走 planner LLM: {}",
                context.user_input
            );
            None
        } else {
            self.find_template(&context.user_input)
        };
        let mut plan = if let Some(template_plan) = template_plan {
            log::info!(
                "[PlanExecutor] Using template plan for input: {}",
                context.user_input
            );
            self.adapt_template_plan(template_plan, context)
        } else if let Some(ref ig_planner) = self.intention_graph_planner {
            // SING 意图图路径：优先尝试 IntentionGraphPlanner
            log::info!("[PlanExecutor] Trying IntentionGraphPlanner (SING)...");
            let t_plan = std::time::Instant::now();
            match ig_planner.generate_plan(context).await {
                Ok(plan) => {
                    log::info!(
                        "[PlanExecutor] IntentionGraphPlanner succeeded in {:?} ({} steps, understanding: {})",
                        t_plan.elapsed(),
                        plan.steps.len(),
                        plan.understanding
                    );

                    // v0.20.1: 持久化执行图到意图图数据库，供前端诊断面板查询。
                    // 修复审计报告 P0-3：此前 record_execution_graph 从未被调用，
                    // 导致诊断面板"最近执行"永远为空。
                    let request_id = Uuid::new_v4().to_string();
                    if let Err(e) = ig_planner
                        .record_execution_graph(
                            &request_id,
                            context.current_story_id.as_deref(),
                            &context.user_input,
                            None,
                            &serde_json::to_string(&plan).unwrap_or_default(),
                        )
                        .await
                    {
                        log::warn!("[PlanExecutor] Failed to record execution graph: {}", e);
                    }

                    plan
                }
                Err(e) => {
                    log::warn!(
                        "[PlanExecutor] IntentionGraphPlanner failed ({}), falling back to PlanGenerator",
                        e
                    );
                    // 回退到原有 PlanGenerator
                    let llm_service = crate::llm::LlmService::new(self.app_handle.clone());
                    let generator =
                        PlanGenerator::new(llm_service).with_app_handle(self.app_handle.clone());
                    let t_plan = std::time::Instant::now();
                    match generator.generate_plan(context).await {
                        Ok(plan) => {
                            log::info!(
                                "[PlanExecutor] PlanGenerator fallback succeeded in {:?} ({} steps)",
                                t_plan.elapsed(),
                                plan.steps.len()
                            );
                            plan
                        }
                        Err(e) => {
                            log::warn!(
                                "[PlanExecutor] PlanGenerator also failed ({}), falling back to direct writer",
                                e
                            );
                            // Fallback: direct writer execution with user input as instruction
                            ExecutionPlan {
                                understanding: format!(
                                    "Direct execution fallback for: {}",
                                    context.user_input
                                ),
                                steps: vec![PlanStep {
                                    step_id: "fallback_writer".to_string(),
                                    capability_id: "writer".to_string(),
                                    purpose:
                                        "Fallback: execute user request directly via writer agent"
                                            .to_string(),
                                    parameters: {
                                        let mut p = HashMap::new();
                                        p.insert(
                                            "story_id".to_string(),
                                            serde_json::Value::String(
                                                context
                                                    .current_story_id
                                                    .clone()
                                                    .unwrap_or_default(),
                                            ),
                                        );
                                        p.insert(
                                            "instruction".to_string(),
                                            serde_json::Value::String(context.user_input.clone()),
                                        );
                                        p
                                    },
                                    depends_on: vec![],
                                    long_running: false,
                                }],
                                fallback_message: "计划生成失败，已回退到直接写作模式".to_string(),
                            }
                        }
                    }
                }
            }
        } else {
            log::info!("[PlanExecutor] No template found, calling PlanGenerator::generate_plan...");
            let llm_service = crate::llm::LlmService::new(self.app_handle.clone());
            let generator =
                PlanGenerator::new(llm_service).with_app_handle(self.app_handle.clone());
            let t_plan = std::time::Instant::now();
            match generator.generate_plan(context).await {
                Ok(plan) => {
                    log::info!(
                        "[PlanExecutor] Plan generation succeeded in {:?} ({} steps)",
                        t_plan.elapsed(),
                        plan.steps.len()
                    );
                    plan
                }
                Err(e) => {
                    log::warn!(
                        "[PlanExecutor] Plan generation failed ({}), falling back to direct writer",
                        e
                    );
                    // Fallback: direct writer execution with user input as instruction
                    ExecutionPlan {
                        understanding: format!(
                            "Direct execution fallback for: {}",
                            context.user_input
                        ),
                        steps: vec![PlanStep {
                            step_id: "fallback_writer".to_string(),
                            capability_id: "writer".to_string(),
                            purpose: "Fallback: execute user request directly via writer agent"
                                .to_string(),
                            parameters: {
                                let mut p = HashMap::new();
                                p.insert(
                                    "story_id".to_string(),
                                    serde_json::Value::String(
                                        context.current_story_id.clone().unwrap_or_default(),
                                    ),
                                );
                                p.insert(
                                    "instruction".to_string(),
                                    serde_json::Value::String(context.user_input.clone()),
                                );
                                p
                            },
                            depends_on: vec![],
                            long_running: false,
                        }],
                        fallback_message: "计划生成失败，已回退到直接写作模式".to_string(),
                    }
                }
            }
        };

        // Inject PlanContext information into every step so agents get full context
        for step in &mut plan.steps {
            if let Some(ref preview) = context.current_content_preview {
                step.parameters
                    .entry("current_content".to_string())
                    .or_insert_with(|| serde_json::Value::String(preview.clone()));
            }
            if let Some(ref story_id) = context.current_story_id {
                step.parameters
                    .entry("story_id".to_string())
                    .or_insert_with(|| serde_json::Value::String(story_id.clone()));
            }
        }

        Ok(self.execute_plan(plan, context).await)
    }

    /// v0.23 TriShot 快速路径：构造单步 writer plan（跳过计划生成 LLM）。
    /// Call 1 路由合成器替代了计划生成的角色——此处仅产生一个最小 plan，
    /// 使 `execute_plan → execute_writer` 路径得以复用步骤参数注入。
    fn make_trishot_plan(context: &PlanContext) -> ExecutionPlan {
        let mut params = HashMap::new();
        params.insert(
            "story_id".to_string(),
            serde_json::Value::String(context.current_story_id.clone().unwrap_or_default()),
        );
        params.insert(
            "instruction".to_string(),
            serde_json::Value::String(context.user_input.clone()),
        );
        // 标记 mode=tri_shot，使 execute_writer 走 TriShot 分支
        params.insert(
            "mode".to_string(),
            serde_json::Value::String("tri_shot".to_string()),
        );
        if let Some(ref content_preview) = context.current_content_preview {
            params.insert(
                "current_content".to_string(),
                serde_json::Value::String(content_preview.clone()),
            );
        }
        // 标记为长任务，跳过 90s 步超时（受 smart_execute 180s 伞保护）
        params.insert("long_running".to_string(), serde_json::Value::Bool(true));

        ExecutionPlan {
            understanding: "TriShot 三击模式：智能合成 → 精修 → 生成".to_string(),
            steps: vec![PlanStep {
                step_id: "trishot_writer".to_string(),
                capability_id: "writer".to_string(),
                purpose: "TriShot 模式：Call 1 合成提示词 → Call 2(可选) 精修 → Call 3 Writer 生成"
                    .to_string(),
                parameters: params,
                depends_on: vec![],
                long_running: true,
            }],
            fallback_message: String::new(),
        }
    }

    pub async fn execute_plan(
        &self,
        plan: ExecutionPlan,
        plan_context: &PlanContext,
    ) -> PlanExecutionResult {
        let mut messages = Vec::new();
        let step_outputs = Arc::new(tokio::sync::Mutex::new(
            HashMap::<String, serde_json::Value>::new(),
        ));
        let mut steps_completed = 0;
        let mut final_content: Option<String> = None;
        let mut first_error: Option<AppError> = None;

        log::info!("[PlanExecutor] Understanding: {}", plan.understanding);
        log::info!("[PlanExecutor] Executing {} steps", plan.steps.len());

        // Phase 4: Agent Swarm - 拓扑排序确定执行批次
        let batches = crate::planner::swarm::topological_sort(&plan.steps);
        log::info!(
            "[PlanExecutor] Swarm batches: {} batches",
            batches.batches.len()
        );

        // 检测 Inspector→Writer 闭环模式
        let has_loop = crate::planner::swarm::detect_inspector_writer_loop(&plan.steps);
        if let Some((inspect_id, writer_id)) = &has_loop {
            log::info!(
                "[PlanExecutor] Detected Inspector→Writer loop: {} → {}",
                inspect_id,
                writer_id
            );
        }

        let total_steps = plan.steps.len();

        // plan 内所有 step_id 集合：依赖校验时区分真实 step_id 依赖与 LLM
        // 偶发写入的上下文名（如 "Story Context"），后者跳过校验（与
        // topological_sort 的处理一致），避免误杀整 plan。
        let plan_step_ids: std::collections::HashSet<&str> =
            plan.steps.iter().map(|s| s.step_id.as_str()).collect();

        // 按批次执行（同批次内无依赖的步骤并行执行）
        for (batch_idx, batch) in batches.batches.iter().enumerate() {
            log::info!(
                "[PlanExecutor] Executing batch {}/{} with {} steps",
                batch_idx + 1,
                batches.batches.len(),
                batch.len()
            );

            // 1) 在当前 batch 开始前统一检查依赖（同 batch 内步骤互相无依赖）
            let mut batch_steps: Vec<PlanStep> = Vec::with_capacity(batch.len());
            for step_id in batch {
                let step = match plan.steps.iter().find(|s| s.step_id == *step_id) {
                    Some(s) => s.clone(),
                    None => {
                        messages.push(format!("Step {} not found in plan", step_id));
                        continue;
                    }
                };

                let outputs = step_outputs.lock().await;
                let mut deps_ok = true;
                for dep in &step.depends_on {
                    // 与 topological_sort 一致：非 plan 内 step_id 的依赖
                    // （LLM 偶发写入的上下文名，如 "Story Context"）跳过校验，
                    // 避免误杀整 plan；仅校验真实 step_id 依赖是否已产出。
                    // 参数引用 {{step_id}} 由 resolve_parameters 兜底处理缺失键。
                    if !plan_step_ids.contains(dep.as_str()) {
                        log::warn!(
                            "[PlanExecutor] Step {} depends_on '{}' 不是 plan 内 step_id，跳过依赖校验",
                            step.step_id,
                            dep
                        );
                        continue;
                    }
                    if !outputs.contains_key(dep) {
                        let msg = format!("Step {} dependency {} not found", step.step_id, dep);
                        log::warn!("[PlanExecutor] {}", msg);
                        messages.push(msg);
                        deps_ok = false;
                        break;
                    }
                }
                if !deps_ok {
                    let _ = self.app_handle.emit(
                        "plan-executor-step",
                        PlanExecutorProgress {
                            step_id: step.step_id.clone(),
                            capability_id: step.capability_id.clone(),
                            status: "failed".to_string(),
                            message: "依赖步骤未满足，跳过".to_string(),
                            steps_completed,
                            total_steps,
                        },
                    );
                    continue;
                }
                batch_steps.push(step);
            }

            // 2) 同 batch 内步骤并行执行
            let step_futures = batch_steps.iter().map(|step| {
                let step = step.clone();
                let app_handle = self.app_handle.clone();
                let step_outputs = step_outputs.clone();
                let has_loop = has_loop.clone();
                let plan_context = plan_context;
                async move {
                    // 发送步骤开始进度事件
                    let _ = app_handle.emit(
                        "plan-executor-step",
                        PlanExecutorProgress {
                            step_id: step.step_id.clone(),
                            capability_id: step.capability_id.clone(),
                            status: "running".to_string(),
                            message: format!(
                                "正在执行: {}",
                                Self::capability_display_name(&step.capability_id)
                            ),
                            steps_completed,
                            total_steps,
                        },
                    );

                    // Phase 4: Swarm 闭环增强 — Inspector→Writer 之间注入质量反馈
                    let resolved_params = {
                        let outputs = step_outputs.lock().await;
                        let mut rp = Self::resolve_parameters(&step.parameters, &outputs);
                        if let Some((ref inspect_id, _)) = has_loop {
                            if step.capability_id == "writer"
                                && step.depends_on.contains(inspect_id)
                            {
                                if let Some(inspector_output) = outputs.get(inspect_id) {
                                    if let Some(feedback) =
                                        inspector_output.get("suggestions").and_then(|s| s.as_str())
                                    {
                                        log::info!(
                                            "[PlanExecutor] Injecting inspector feedback into writer step"
                                        );
                                        rp.insert(
                                            "inspector_feedback".to_string(),
                                            serde_json::Value::String(feedback.to_string()),
                                        );
                                    }
                                }
                            }
                        }

                        // v0.30.9: Inspector 兜底注入 draft -- LLM 生成的 plan 常遗漏
                        // "draft": "{{step_N}}" 参数，导致 inspector 收到空内容并返回
                        // 审查模板而非正文。当 draft 为空时，按 depends_on 顺序查找
                        // writer 步骤的 content，找不到则扫描全部 step_outputs。
                        if step.capability_id == "inspector" {
                            let injected = Self::inject_inspector_draft_fallback(
                                &mut rp,
                                &step.depends_on,
                                &outputs,
                            );
                            if injected {
                                log::info!(
                                    "[PlanExecutor] Inspector step {} draft 为空，自动注入依赖步骤的 writer 输出",
                                    step.step_id
                                );
                            } else {
                                let draft_empty = rp
                                    .get("draft")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.is_empty())
                                    .unwrap_or(true);
                                if draft_empty {
                                    log::warn!(
                                        "[PlanExecutor] Inspector step {} draft 为空且未找到任何 writer 步骤输出，inspector 将收到空内容",
                                        step.step_id
                                    );
                                }
                            }
                        }

                        // v0.30.10: style_mimic / plot_analyzer / builtin 技能 content 兜底注入。
                        // 与 inspector draft 兜底同理：LLM 常遗漏 "content": "{{step_N}}" 参数，
                        // 导致这些 capability 收到空内容并返回"请提供文本"模板而非实际处理结果。
                        let needs_content_fallback = step.capability_id == "style_mimic"
                            || step.capability_id == "plot_analyzer"
                            || step.capability_id.starts_with("builtin.");
                        if needs_content_fallback {
                            let current_content = plan_context
                                .current_content_preview
                                .as_deref();
                            let injected = Self::inject_content_fallback(
                                &mut rp,
                                &step.depends_on,
                                &outputs,
                                current_content,
                            );
                            if injected {
                                log::info!(
                                    "[PlanExecutor] {} step {} content 为空，自动注入 writer 输出或当前正文",
                                    step.capability_id,
                                    step.step_id
                                );
                            } else {
                                log::warn!(
                                    "[PlanExecutor] {} step {} content 为空且未找到任何可用文本",
                                    step.capability_id,
                                    step.step_id
                                );
                            }
                        }

                        rp
                    };

                    let step_start = std::time::Instant::now();
                    // v0.14.0: 单步超时 90 秒，防止某个 capability 卡死拖垮整个计划。
                    // 超时记为 step failed 但不中断后续批次（保持容错语义）。
                    // v0.23: long_running 步骤跳过步超时（如 TriShot 2~3 次 LLM），
                    // 仅受外层 smart_execute 180s 伞保护。
                    let step_timeout = if step.long_running {
                        None // 不设步级超时，依赖 smart_execute 外层 180s
                    } else {
                        let app_dir = self
                            .app_handle
                            .path()
                            .app_data_dir()
                            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
                        let secs = crate::config::AppConfig::load(&app_dir)
                            .map(|c| c.executor_step_timeout_secs)
                            .unwrap_or(90u64);
                        Some(std::time::Duration::from_secs(secs))
                    };

                    let result = if let Some(dur) = step_timeout {
                        match tokio::time::timeout(dur, self.execute_step(&step, &resolved_params, plan_context)).await {
                            Ok(r) => r,
                            Err(_) => {
                                log::error!(
                                    "[PlanExecutor] Step {} ({}) timed out after {:?}",
                                    step.step_id,
                                    step.capability_id,
                                    dur
                                );
                                Err(AppError::internal(format!(
                                    "步骤 {} 超时",
                                    Self::capability_display_name(&step.capability_id),
                                )))
                            }
                        }
                    } else {
                        self.execute_step(&step, &resolved_params, plan_context).await
                    };
                    let step_duration = step_start.elapsed().as_millis() as u64;

                    match &result {
                        Ok(_) => {
                            let _ = app_handle.emit(
                                "plan-executor-step",
                                PlanExecutorProgress {
                                    step_id: step.step_id.clone(),
                                    capability_id: step.capability_id.clone(),
                                    status: "completed".to_string(),
                                    message: format!(
                                        "{} 完成",
                                        Self::capability_display_name(&step.capability_id)
                                    ),
                                    steps_completed,
                                    total_steps,
                                },
                            );
                        }
                        Err(e) => {
                            log::warn!("[PlanExecutor] Step {} failed: {}", step.step_id, e);
                            let _ = app_handle.emit(
                                "plan-executor-step",
                                PlanExecutorProgress {
                                    step_id: step.step_id.clone(),
                                    capability_id: step.capability_id.clone(),
                                    status: "failed".to_string(),
                                    message: format!(
                                        "{} 失败: {}",
                                        Self::capability_display_name(&step.capability_id),
                                        e
                                    ),
                                    steps_completed,
                                    total_steps,
                                },
                            );
                        }
                    }

                    (step, result, step_duration)
                }
            });

            let batch_results = futures::future::join_all(step_futures).await;

            // 3) 合并本 batch 的执行结果（顺序处理，避免并发写 messages/records）
            for (step, result, step_duration) in batch_results {
                // Record execution result
                let record = ExecutionRecord {
                    capability_id: step.capability_id.clone(),
                    user_input: plan.understanding.clone(),
                    success: result.is_ok(),
                    user_feedback: None,
                    execution_time_ms: step_duration,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                };
                let _ = self.evolution_engine.record_execution(record);

                match result {
                    Ok(output) => {
                        step_outputs
                            .lock()
                            .await
                            .insert(step.step_id.clone(), output.clone());
                        messages.push(format!(
                            "Step {} completed: {}",
                            step.step_id, step.capability_id
                        ));
                        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
                            final_content = Some(content.to_string());
                        }
                        steps_completed += 1;
                    }
                    Err(e) => {
                        messages.push(format!("Step {} failed: {}", step.step_id, e));
                        if first_error.is_none() {
                            first_error = Some(e.clone());
                        }
                    }
                }
            }
        }

        // Phase 4: Swarm 质量闭环 — 如果最终内容是 writer 产出且前面有 inspector，
        // 尝试自动触发一轮轻量 inspector 检查
        if let Some((_, ref writer_id)) = has_loop {
            let outputs = step_outputs.lock().await;
            if let Some(writer_output) = outputs.get(writer_id) {
                if let Some(content) = writer_output.get("content").and_then(|c| c.as_str()) {
                    if content.len() > 100 {
                        log::info!(
                            "[PlanExecutor] Swarm loop complete, content length: {}",
                            content.len()
                        );
                    }
                }
            }
        }

        let success = steps_completed > 0
            && steps_completed
                >= plan
                    .steps
                    .iter()
                    .filter(|s| s.depends_on.is_empty())
                    .count();

        // Record successful plan as template
        if success {
            if let Ok(mut library) = self.template_library.lock() {
                library.record_success(&plan.understanding, plan.clone());
            }
        }

        // 发送执行完成事件
        let _ = self.app_handle.emit(
            "plan-executor-step",
            PlanExecutorProgress {
                step_id: "__complete__".to_string(),
                capability_id: "__complete__".to_string(),
                status: if success {
                    "completed".to_string()
                } else {
                    "failed".to_string()
                },
                message: if success {
                    "计划执行完成".to_string()
                } else {
                    "计划执行失败".to_string()
                },
                steps_completed,
                total_steps,
            },
        );

        // v0.11.5-hotfix: 禁用计划执行后自动触发能力进化，避免每次创作完成后
        // 在后台发起长时间 LLM 调用。能力进化改为通过 `evolve_capabilities` 手动触发。

        PlanExecutionResult {
            success,
            steps_completed,
            final_content,
            messages,
            error: first_error,
        }
    }

    /// 将 capability_id 转换为用户友好的中文名称
    fn capability_display_name(capability_id: &str) -> String {
        match capability_id {
            "writer" => "写作助手".to_string(),
            "inspector" => "质检员".to_string(),
            "outline_planner" => "大纲规划师".to_string(),
            "style_mimic" => "风格模仿师".to_string(),
            "plot_analyzer" => "情节分析师".to_string(),
            "create_story" => "创建故事".to_string(),
            "create_chapter" => "创建章节".to_string(),
            "create_character" => "创建角色".to_string(),
            "update_character" => "更新角色".to_string(),
            "update_world_building" => "更新世界观".to_string(),
            "update_scene" => "更新场景".to_string(),
            "query_knowledge_graph" => "查询知识图谱".to_string(),
            id if id.starts_with("builtin.") => {
                let skill_name = id.strip_prefix("builtin.").unwrap_or(id);
                match skill_name {
                    "style_enhancer" => "风格增强".to_string(),
                    "character_voice" => "角色声音".to_string(),
                    "emotion_pacing" => "情感节奏".to_string(),
                    "plot_twist" => "情节转折".to_string(),
                    "text_formatter" => "文本格式化".to_string(),
                    _ => format!("技能:{}", skill_name),
                }
            }
            id if id.starts_with("mcp.") => "外部工具".to_string(),
            _ => capability_id.to_string(),
        }
    }

    async fn execute_step(
        &self,
        step: &PlanStep,
        params: &HashMap<String, serde_json::Value>,
        plan_context: &PlanContext,
    ) -> Result<serde_json::Value, AppError> {
        match step.capability_id.as_str() {
            "create_story" => self.execute_create_story(params).await,
            "create_chapter" => self.execute_create_chapter(params).await,
            "create_character" => self.execute_create_character(params).await,
            "writer" => self.execute_writer(params, plan_context).await,
            "inspector" => self.execute_inspector(params, plan_context).await,
            "outline_planner" => self.execute_outline_planner(params, plan_context).await,
            "style_mimic" => self.execute_style_mimic(params, plan_context).await,
            "plot_analyzer" => self.execute_plot_analyzer(params, plan_context).await,
            "update_character" => self.execute_update_character(params).await,
            "update_world_building" => self.execute_update_world_building(params).await,
            "update_scene" => self.execute_update_scene(params).await,
            "query_knowledge_graph" => self.execute_query_knowledge_graph(params).await,
            skill_id if skill_id.starts_with("builtin.") => {
                self.execute_skill(skill_id, params, plan_context).await
            }
            skill_id if skill_id.starts_with("mcp.") => {
                self.execute_mcp_tool(skill_id, params).await
            }
            _ => Err(AppError::internal(format!(
                "Unknown capability: {}",
                step.capability_id
            ))),
        }
    }

    /// Build a rich AgentContext using StoryContextBuilder instead of the
    /// minimal stub.
    async fn build_agent_context(
        &self,
        story_id: &str,
        current_content: Option<String>,
        selected_text: Option<String>,
    ) -> Result<crate::domain::agent_context::AgentContext, AppError> {
        if story_id.is_empty() {
            return Ok(crate::domain::agent_context::AgentContext::minimal(
                story_id.to_string(),
                String::new(),
            ));
        }

        let pool = self.app_handle.state::<crate::db::DbPool>();
        let builder =
            crate::creative_engine::context_builder::StoryContextBuilder::new(pool.inner().clone());

        // Resolve current scene number from DB (latest scene for the story)
        let scene_number = self.get_current_scene_number(story_id).unwrap_or(None);

        Ok(builder
            .build(story_id, scene_number, current_content, selected_text)
            .await?)
    }

    fn get_current_scene_number(&self, story_id: &str) -> Result<Option<i32>, AppError> {
        let pool = self.app_handle.state::<crate::db::DbPool>();
        let repo = crate::db::repositories::SceneRepository::new(pool.inner().clone());
        let scenes = repo.get_by_story(story_id).map_err(AppError::from)?;
        Ok(scenes
            .iter()
            .max_by_key(|s| s.sequence_number)
            .map(|s| s.sequence_number))
    }

    fn resolve_parameters(
        params: &HashMap<String, serde_json::Value>,
        outputs: &HashMap<String, serde_json::Value>,
    ) -> HashMap<String, serde_json::Value> {
        let mut resolved = params.clone();

        for (key, value) in params.iter() {
            if let Some(ref_str) = value.as_str() {
                let mut result = ref_str.to_string();
                for (step_id, output) in outputs.iter() {
                    let placeholder = format!("{{{{{}}}}}", step_id);
                    if result.contains(&placeholder) {
                        let replacement =
                            output.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        result = result.replace(&placeholder, replacement);
                    }
                }
                if result != ref_str {
                    resolved.insert(key.clone(), serde_json::Value::String(result));
                }
            }
        }

        resolved
    }

    /// v0.30.9: Inspector draft 兜底注入 -- LLM 生成的 plan 常遗漏
    /// `"draft": "{{step_N}}"` 参数，导致 inspector 收到空内容并返回审查模板
    /// 而非正文。当 inspector 的 draft 为空时，按 depends_on 顺序查找 writer
    /// 步骤的 content，找不到则扫描全部 step_outputs。返回 true 表示已注入。
    fn inject_inspector_draft_fallback(
        rp: &mut HashMap<String, serde_json::Value>,
        depends_on: &[String],
        outputs: &HashMap<String, serde_json::Value>,
    ) -> bool {
        let draft_empty = rp
            .get("draft")
            .and_then(|v| v.as_str())
            .map(|s| s.is_empty())
            .unwrap_or(true);
        if !draft_empty {
            return false;
        }

        let mut found_content: Option<String> = None;
        // 优先从 depends_on 指定的 step_id 查找
        for dep in depends_on {
            if let Some(out) = outputs.get(dep) {
                if let Some(c) = out.get("content").and_then(|v| v.as_str()) {
                    if !c.is_empty() {
                        found_content = Some(c.to_string());
                        break;
                    }
                }
            }
        }
        // 兜底：扫描全部 step_outputs 找非空 content
        if found_content.is_none() {
            for (_sid, out) in outputs.iter() {
                if let Some(c) = out.get("content").and_then(|v| v.as_str()) {
                    if !c.is_empty() {
                        found_content = Some(c.to_string());
                        break;
                    }
                }
            }
        }

        if let Some(content) = found_content {
            rp.insert("draft".to_string(), serde_json::Value::String(content));
            true
        } else {
            false
        }
    }

    /// v0.30.10: style_mimic / plot_analyzer / builtin 技能的 content
    /// 兜底注入。 与 inspector draft 兜底同理：LLM 常遗漏 `"content":
    /// "{{step_N}}"` 参数， 导致这些 capability
    /// 收到空内容并返回"请提供文本"模板。当 content 为空时， 按 depends_on
    /// 顺序查找 writer 步骤的 content，找不到则扫描全部 step_outputs，
    /// 最后兜底用 plan_context.current_content_preview（当前编辑器正文）。
    fn inject_content_fallback(
        rp: &mut HashMap<String, serde_json::Value>,
        depends_on: &[String],
        outputs: &HashMap<String, serde_json::Value>,
        current_content: Option<&str>,
    ) -> bool {
        let content_empty = rp
            .get("content")
            .and_then(|v| v.as_str())
            .map(|s| s.is_empty())
            .unwrap_or(true);
        if !content_empty {
            return false;
        }

        let mut found: Option<String> = None;
        for dep in depends_on {
            if let Some(out) = outputs.get(dep) {
                if let Some(c) = out.get("content").and_then(|v| v.as_str()) {
                    if !c.is_empty() {
                        found = Some(c.to_string());
                        break;
                    }
                }
            }
        }
        if found.is_none() {
            for (_sid, out) in outputs.iter() {
                if let Some(c) = out.get("content").and_then(|v| v.as_str()) {
                    if !c.is_empty() {
                        found = Some(c.to_string());
                        break;
                    }
                }
            }
        }
        if found.is_none() {
            if let Some(cc) = current_content {
                if !cc.is_empty() {
                    found = Some(cc.to_string());
                }
            }
        }

        if let Some(content) = found {
            rp.insert("content".to_string(), serde_json::Value::String(content));
            true
        } else {
            false
        }
    }

    async fn execute_create_story(
        &self,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, AppError> {
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("未命名作品")
            .to_string();
        let description = params
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let genre = params
            .get("genre")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let pool = self.app_handle.state::<crate::db::DbPool>();
        let repo = crate::db::repositories::StoryRepository::new(pool.inner().clone());
        let story = repo
            .create(crate::db::CreateStoryRequest {
                title,
                description,
                genre,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .map_err(AppError::from)?;

        // Emit event to refresh frontstage
        let _ = crate::window::WindowManager::send_to_frontstage(
            &self.app_handle,
            crate::window::FrontstageEvent::DataRefresh {
                entity: "stories".to_string(),
            },
        );

        Ok(serde_json::json!({
            "story_id": story.id,
            "title": story.title,
            "content": format!("Created story: {}", story.title),
        }))
    }

    async fn execute_create_chapter(
        &self,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .ok_or("story_id required")?
            .to_string();
        let chapter_number = params
            .get("chapter_number")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as i32;
        let title = params
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let pool = self.app_handle.state::<crate::db::DbPool>();
        let repo = crate::db::ChapterRepository::new(pool.inner().clone());
        let chapter = repo
            .create(crate::db::CreateChapterRequest {
                story_id: story_id.clone(),
                chapter_number,
                title: title.clone(),
                outline: None,
                content: None,
            })
            .map_err(AppError::from)?;

        Ok(serde_json::json!({
            "chapter_id": chapter.id,
            "story_id": story_id,
            "chapter_number": chapter_number,
            "title": title.unwrap_or_default(),
            "content": format!("Created chapter {}", chapter_number),
        }))
    }

    async fn execute_create_character(
        &self,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .ok_or("story_id required")?
            .to_string();
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("name required")?
            .to_string();
        let background = params
            .get("background")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let pool = self.app_handle.state::<crate::db::DbPool>();
        let repo = crate::db::repositories::CharacterRepository::new(pool.inner().clone());
        let character = repo
            .create(crate::db::CreateCharacterRequest {
                story_id,
                name,
                background,
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,

                source: None,
                is_auto_generated: None,
            })
            .map_err(AppError::from)?;

        Ok(serde_json::json!({
            "character_id": character.id,
            "name": character.name,
            "content": format!("Created character: {}", character.name),
        }))
    }

    async fn execute_writer(
        &self,
        params: &HashMap<String, serde_json::Value>,
        plan_context: &PlanContext,
    ) -> Result<serde_json::Value, AppError> {
        log::info!("[PlanExecutor::execute_writer] START");
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let instruction = params
            .get("instruction")
            .and_then(|v| v.as_str())
            .unwrap_or("Continue the story")
            .to_string();
        let current_content = params
            .get("current_content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| plan_context.current_content_preview.clone());

        let service = crate::agents::service::AgentService::new(self.app_handle.clone());
        let sw = (plan_context.style_weight as f32 / 100.0).clamp(0.0, 1.0);
        let app_dir = self.app_handle.path().app_data_dir().unwrap_or_default();
        log::info!("[PlanExecutor::execute_writer] Loading AppConfig...");
        let app_config = crate::config::AppConfig::load(&app_dir).unwrap_or_default();
        let app_config_mode = app_config.generation_mode.clone();
        let mut config = crate::agents::orchestrator::WorkflowConfig::from_app_config(&app_config);
        config.style_weight = sw;
        config.narrative_weight = 1.0 - sw;
        let orchestrator = crate::agents::orchestrator::AgentOrchestrator::new(
            service,
            config,
            self.app_handle.clone(),
        );
        let selected_text = plan_context.selected_text.clone();
        // v0.14.3: 保留副本用于后续场景路由判断
        let has_selected_text = selected_text.is_some();
        let current_content_len = current_content.as_deref().map(|s| s.len()).unwrap_or(0);
        log::info!(
            "[PlanExecutor::execute_writer] Calling build_agent_context (story_id={})...",
            story_id
        );
        let t_ctx = std::time::Instant::now();
        let mut context = self
            .build_agent_context(&story_id, current_content, selected_text)
            .await?;
        log::info!(
            "[PlanExecutor::execute_writer] build_agent_context done in {:?}",
            t_ctx.elapsed()
        );
        // v0.8.0: 使用 PlanContext 中的章节号（用户当前编辑的场景），而非最新场景
        context.narrative.chapter_number = plan_context.chapter_number.max(1) as u32;

        // Phase 5: 将 PlanContext 中的结构信息注入到 AgentTask 参数
        let mut enriched_params = params.clone();
        enriched_params.insert(
            "story_progress".to_string(),
            serde_json::Value::String(plan_context.story_progress.clone()),
        );
        if let Some(ref stage) = plan_context.current_scene_stage {
            enriched_params.insert(
                "current_scene_stage".to_string(),
                serde_json::Value::String(stage.clone()),
            );
        }
        if plan_context.scene_count > 0 {
            enriched_params.insert(
                "scene_count".to_string(),
                serde_json::Value::Number(plan_context.scene_count.into()),
            );
        }
        if plan_context.total_word_count > 0 {
            enriched_params.insert(
                "total_word_count".to_string(),
                serde_json::Value::Number(plan_context.total_word_count.into()),
            );
        }

        // v0.17.1: 把智能后台预访谈推断出的中文叙事四元组注入 task.parameters，
        // 让 build_writer_prompt 在末尾追加 prompt 片段。
        if let Some(ref selected) = plan_context.selected_strategy {
            if let Ok(quartet) =
                crate::strategy::quartet_inference::serialize_quartet_for_prompt(selected)
            {
                if !quartet.is_null() {
                    enriched_params.insert("narrative_quartet".to_string(), quartet);
                }
            }

            // Phase 4: 将复合题材的次要 genre_profile_ids 透传给 TimeSliced 路径
            if let Some(secondary) = selected.parameters.get("secondary_genre_profile_ids") {
                let ids: Vec<String> = secondary
                    .as_str()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .or_else(|| {
                        secondary.as_array().map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                    })
                    .unwrap_or_default();
                if !ids.is_empty() {
                    enriched_params.insert(
                        "secondary_genre_profile_ids".to_string(),
                        serde_json::to_value(&ids).unwrap_or_default(),
                    );
                }
            }
        }

        // v0.22.5: Phase C - 注入追读力债务与本章追读力目标
        if let Some(pool_state) = self.app_handle.try_state::<crate::db::DbPool>() {
            let pool = pool_state.inner().clone();

            // 加载待偿还追读力债务
            let debt_repo = crate::db::ChaseDebtRepository::new(pool.clone());
            let (debt_count, debts_text) = match debt_repo.get_active_by_story(&story_id) {
                Ok(debts) => {
                    let count = debts.len();
                    let text = if debts.is_empty() {
                        "无".to_string()
                    } else {
                        debts
                            .iter()
                            .enumerate()
                            .map(|(i, d)| {
                                format!(
                                    "{}. 类型：{}，当前金额：{:.1}，到期章节：{}，来源章节：{}",
                                    i + 1,
                                    d.debt_type,
                                    d.current_amount,
                                    d.due_chapter,
                                    d.source_chapter
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    };
                    (count, text)
                }
                Err(e) => {
                    log::warn!("[PlanExecutor] 加载追读力债务失败: {}", e);
                    (0, "无".to_string())
                }
            };
            enriched_params.insert(
                "chase_debt_count".to_string(),
                serde_json::Value::String(debt_count.to_string()),
            );
            enriched_params.insert(
                "chase_debts".to_string(),
                serde_json::Value::String(debts_text),
            );

            // 加载最近一章追读力作为目标参考
            let rp_repo = crate::db::ChapterReadingPowerRepository::new(pool);
            let (hook_type, hook_strength, foreshadowing_list, micropayoff_count) =
                match rp_repo.get_by_story(&story_id, 1) {
                    Ok(items) if !items.is_empty() => {
                        let item = &items[0];
                        let micropayoffs: Vec<String> = item
                            .micropayoffs_json
                            .as_ref()
                            .and_then(|s| serde_json::from_str(s).ok())
                            .unwrap_or_default();
                        (
                            item.hook_type
                                .clone()
                                .unwrap_or_else(|| "（延续）".to_string()),
                            item.hook_strength.clone(),
                            if micropayoffs.is_empty() {
                                "无".to_string()
                            } else {
                                micropayoffs.join("、")
                            },
                            micropayoffs.len().to_string(),
                        )
                    }
                    _ => (
                        "（未指定）".to_string(),
                        "medium".to_string(),
                        "无".to_string(),
                        "1-2".to_string(),
                    ),
                };
            enriched_params.insert(
                "reading_power_hook_type".to_string(),
                serde_json::Value::String(hook_type),
            );
            enriched_params.insert(
                "reading_power_hook_strength".to_string(),
                serde_json::Value::String(hook_strength),
            );
            enriched_params.insert(
                "reading_power_foreshadowing_list".to_string(),
                serde_json::Value::String(foreshadowing_list),
            );
            enriched_params.insert(
                "reading_power_micropayoff_count".to_string(),
                serde_json::Value::String(micropayoff_count),
            );
        }

        // v0.26.0: 生成前约束门——在调用 Writer 前做轻量规则检查
        let gate = crate::agents::pre_generation_gate::check(&context);
        if !gate.can_proceed {
            let issues: Vec<String> = gate
                .warnings
                .iter()
                .chain(gate.constraints.iter())
                .cloned()
                .collect();
            return Err(crate::error::AppError::PreflightFailed {
                message: "生成前检查未通过".to_string(),
                issues,
            });
        }
        let instruction = if gate.constraints.is_empty() && gate.warnings.is_empty() {
            instruction
        } else {
            format!("{}{}", instruction, gate.render_constraints())
        };

        // v0.30.11: 透传 LLM 分类的 detected_genre / task_type_hint 到
        // task.parameters， 供 build_writer_prompt（题材覆盖，替代
        // extract_genre 子串匹配）与 from_instruction_and_context（task_type
        // hint，替代指令子串启发式）使用。 分类来源：smart_execute 入口的
        // classify_writing_intent，经 PlanContext 贯穿。
        if let Some(classification) = &plan_context.intent_classification {
            if let Some(ref genre) = classification.detected_genre {
                enriched_params.insert(
                    "detected_genre".to_string(),
                    serde_json::Value::String(genre.clone()),
                );
            }
            enriched_params.insert(
                "task_type_hint".to_string(),
                serde_json::to_value(&classification.task_type).unwrap_or(serde_json::Value::Null),
            );
        }

        let task = crate::domain::agent_types::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::domain::agent_types::AgentType::Writer,
            context,
            input: instruction,
            parameters: enriched_params,
            tier: None,
        };

        // v0.14.3: 场景智能路由——续写默认走 TimeSliced（快速生成 30-60s），
        // 重写选中文本走 Full（含 Inspector 质检），用户可在设置中显式覆盖。
        // 修复 v0.13.0 设计文档（time-sliced-intervention-design.md:456）的实现遗漏：
        // 该文档明确指定 smart_execute 默认 TimeSliced，但实施时漏改了 PlanExecutor
        // 路径。 模式选择优先级：plan 参数 > AppConfig.generation_mode >
        // 场景智能路由
        let mode_str = params
            .get("mode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| app_config_mode.clone());

        let mode = match mode_str.as_str() {
            "full" => crate::agents::orchestrator::GenerationMode::Full,
            "fast" => crate::agents::orchestrator::GenerationMode::Fast,
            "time_sliced" | "timesliced" => crate::agents::orchestrator::GenerationMode::TimeSliced,
            // v0.23 TriShot：弹性 2~3 次 LLM，资产任务下沉后台
            "tri_shot" | "trishot" => crate::agents::orchestrator::GenerationMode::TriShot,
            _ => {
                // "auto" 或其他：场景智能路由
                if has_selected_text {
                    // 重写选中文本：用户明确要求改写，需要质检循环
                    crate::agents::orchestrator::GenerationMode::Full
                } else {
                    // 续写或新章节首段：速度优先，问题靠后台审计修正
                    crate::agents::orchestrator::GenerationMode::TimeSliced
                }
            }
        };

        log::info!(
            "[PlanExecutor::execute_writer] Calling orchestrator.generate({:?}) (selected_text={}, current_content_len={})...",
            mode,
            has_selected_text,
            current_content_len
        );
        let t_gen = std::time::Instant::now();
        let workflow_result = orchestrator.generate(task, mode).await?;
        log::info!(
            "[PlanExecutor::execute_writer] orchestrator.generate({:?}) done in {:?} (score={})",
            mode,
            t_gen.elapsed(),
            workflow_result.final_score
        );
        Ok(serde_json::json!({
            "content": workflow_result.final_content,
            "score": Some(workflow_result.final_score as f64),
            "request_id": workflow_result.request_id,
        }))
    }

    async fn execute_inspector(
        &self,
        params: &HashMap<String, serde_json::Value>,
        plan_context: &PlanContext,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let draft = params
            .get("draft")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let current_content = params
            .get("current_content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| plan_context.current_content_preview.clone());

        let service = crate::agents::service::AgentService::new(self.app_handle.clone());
        let context = self
            .build_agent_context(&story_id, current_content, None)
            .await?;
        let task = crate::domain::agent_types::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::domain::agent_types::AgentType::Inspector,
            context,
            input: draft,
            parameters: params.clone(),
            tier: None,
        };

        let result = service.execute_task(task).await?;
        Ok(serde_json::json!({
            "content": result.content,
            "score": result.score,
            "suggestions": result.suggestions,
        }))
    }

    async fn execute_outline_planner(
        &self,
        params: &HashMap<String, serde_json::Value>,
        _plan_context: &PlanContext,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let premise = params
            .get("premise")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let service = crate::agents::service::AgentService::new(self.app_handle.clone());
        let context = self.build_agent_context(&story_id, None, None).await?;
        let task = crate::domain::agent_types::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::domain::agent_types::AgentType::OutlinePlanner,
            context,
            input: premise,
            parameters: params.clone(),
            tier: None,
        };

        let result = service.execute_task(task).await?;
        Ok(serde_json::json!({
            "content": result.content,
            "outline": result.content,
        }))
    }

    async fn execute_style_mimic(
        &self,
        params: &HashMap<String, serde_json::Value>,
        _plan_context: &PlanContext,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let service = crate::agents::service::AgentService::new(self.app_handle.clone());
        let mut task_params = params.clone();
        task_params.insert(
            "style_sample".to_string(),
            params
                .get("style_sample")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );

        let context = self.build_agent_context(&story_id, None, None).await?;
        let task = crate::domain::agent_types::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::domain::agent_types::AgentType::StyleMimic,
            context,
            input: content,
            parameters: task_params,
            tier: None,
        };

        let result = service.execute_task(task).await?;
        Ok(serde_json::json!({"content": result.content}))
    }

    async fn execute_plot_analyzer(
        &self,
        params: &HashMap<String, serde_json::Value>,
        _plan_context: &PlanContext,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let content = params
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let service = crate::agents::service::AgentService::new(self.app_handle.clone());
        let context = self.build_agent_context(&story_id, None, None).await?;
        let task = crate::domain::agent_types::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::domain::agent_types::AgentType::PlotAnalyzer,
            context,
            input: content,
            parameters: params.clone(),
            tier: None,
        };

        let result = service.execute_task(task).await?;
        Ok(serde_json::json!({
            "content": result.content,
            "score": result.score,
            "suggestions": result.suggestions,
        }))
    }

    async fn execute_skill(
        &self,
        skill_id: &str,
        params: &HashMap<String, serde_json::Value>,
        _plan_context: &PlanContext,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut params = params.clone();
        params.insert(
            "story_id".to_string(),
            serde_json::Value::String(story_id.clone()),
        );

        let skill_manager = crate::skills::SkillManager::from_app_handle(&self.app_handle);

        let agent_context = self.build_agent_context(&story_id, None, None).await?;

        let result = skill_manager
            .execute_skill(skill_id, &agent_context, params)
            .await?;

        if !result.success {
            return Err(AppError::internal(
                result.error.unwrap_or("Skill execution failed".to_string()),
            ));
        }

        Ok(result.data)
    }

    // ==================== 设定修改执行器 ====================

    async fn execute_update_character(
        &self,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let character_id = params
            .get("character_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let changes = params
            .get("changes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let pool = self.app_handle.state::<crate::db::DbPool>();
        let char_repo = crate::db::repositories::CharacterRepository::new(pool.inner().clone());

        // 先尝试按ID查找，失败则按名称查找
        let character = if let Ok(Some(c)) = char_repo.get_by_id(&character_id) {
            c
        } else {
            let all = char_repo.get_by_story(&story_id).map_err(AppError::from)?;
            all.into_iter()
                .find(|c| c.name == character_id)
                .ok_or_else(|| format!("Character '{}' not found", character_id))?
        };

        // 使用LLM解析修改意图并生成新属性值
        let llm_service = crate::llm::LlmService::new(self.app_handle.clone());
        // v0.21.0: 从 PromptRegistry 读取（支持用户覆盖）
        let prompt = {
            let default_tpl = || {
                r#"你是一位角色编辑助手。请根据用户的修改要求，为角色生成新的属性值。

角色名：{{character_name}}
当前属性：{{current_attributes}}
用户要求：{{user_request}}

请用 JSON 格式回复更新后的角色属性。只输出 JSON。"#
                    .to_string()
            };
            let tpl =
                crate::prompts::registry::resolve_prompt(&self.pool, "planner_edit_character")
                    .unwrap_or_else(|_| {
                        crate::prompts::registry::resolve_prompt_default("planner_edit_character")
                            .unwrap_or_else(default_tpl)
                    });
            let mut vars = std::collections::HashMap::new();
            vars.insert("character_name".to_string(), character.name.clone());
            vars.insert(
                "current_attributes".to_string(),
                format!(
                    "姓名：{}\n背景：{}\n性格：{}\n目标：{}",
                    character.name,
                    character.background.as_deref().unwrap_or("未设定"),
                    character.personality.as_deref().unwrap_or("未设定"),
                    character.goals.as_deref().unwrap_or("未设定"),
                ),
            );
            vars.insert("user_request".to_string(), changes.replace('"', "'"));
            crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
        };

        let response = llm_service
            .generate_for_task(
                TaskType::Editing,
                prompt,
                Some(1024),
                Some(0.3),
                Some("update_character"),
            )
            .await?;
        let content = response.content.trim();
        let json_str = if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
            &content[start..=end]
        } else {
            content
        };

        let updates: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse character update JSON: {}", e))?;

        let new_name = updates
            .get("name")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let new_background = updates
            .get("background")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let new_personality = updates
            .get("personality")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());
        let new_goals = updates
            .get("goals")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        char_repo
            .update(
                &character.id,
                new_name.map(|s| s.to_string()),
                new_background.map(|s| s.to_string()),
                new_personality.map(|s| s.to_string()),
                new_goals.map(|s| s.to_string()),
                None,
                None,
                None,
            )
            .map_err(AppError::from)?;

        Ok(serde_json::json!({
            "character_id": character.id,
            "name": new_name.unwrap_or(&character.name),
            "message": format!("角色 '{}' 已更新", character.name),
        }))
    }

    async fn execute_update_world_building(
        &self,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let changes = params
            .get("changes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let pool = self.app_handle.state::<crate::db::DbPool>();
        let wb_repo = crate::db::repositories::WorldBuildingRepository::new(pool.inner().clone());

        let wb = wb_repo
            .get_by_story(&story_id)
            .map_err(AppError::from)?
            .ok_or_else(|| "World building not found for this story".to_string())?;

        // 使用LLM解析修改意图
        let llm_service = crate::llm::LlmService::new(self.app_handle.clone());
        // v0.21.0: 优先从 PromptRegistry 读取（支持用户覆盖）
        if let Ok(tpl) = crate::prompts::registry::resolve_prompt(&self.pool, "planner_edit_world")
        {
            let mut vars = std::collections::HashMap::new();
            vars.insert("current_world".to_string(), wb.concept.as_str().to_string());
            vars.insert("user_request".to_string(), changes.replace('"', "'"));
            let prompt =
                crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars);
            let response = llm_service
                .generate_for_task(
                    TaskType::Editing,
                    prompt,
                    Some(1024),
                    Some(0.3),
                    Some("update_world_building"),
                )
                .await?;
            return Ok(serde_json::from_str(&response.content)?);
        }
        let prompt = format!(
            r#"你是一位世界观编辑助手。请根据用户的修改要求，生成新的世界观设定。

当前世界观：
- 核心概念：{}
- 规则：{}
- 历史：{}

用户修改要求："{}"

请用 JSON 格式回复：
{{{{
  "concept": "新概念（如不需要修改则留空或省略）",
  "rules_to_add": [{{"name": "新规则名", "description": "规则描述", "rule_type": "physical|magic|social|historical", "importance": 8}}],
  "history_update": "历史补充或修改（如不需要则留空或省略）"
}}}}

注意：只输出 JSON，不要其他内容。"#,
            wb.concept,
            wb.rules
                .iter()
                .map(|r| format!("{}: {}", r.name, r.description.as_deref().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("; "),
            wb.history.as_deref().unwrap_or("未设定"),
            changes.replace('"', "'")
        );

        let response = llm_service
            .generate_for_task(
                TaskType::WorldBuilding,
                prompt,
                Some(2048),
                Some(0.3),
                Some("update_world_building"),
            )
            .await?;
        let content = response.content.trim();
        let json_str = if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
            &content[start..=end]
        } else {
            content
        };

        let updates: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse world building update JSON: {}", e))?;

        let new_concept = updates
            .get("concept")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        // 解析新规则
        let mut all_rules = wb.rules.clone();
        if let Some(new_rules) = updates.get("rules_to_add").and_then(|v| v.as_array()) {
            for rule_val in new_rules {
                if let (Some(name), Some(desc), Some(rule_type), Some(importance)) = (
                    rule_val.get("name").and_then(|v| v.as_str()),
                    rule_val.get("description").and_then(|v| v.as_str()),
                    rule_val.get("rule_type").and_then(|v| v.as_str()),
                    rule_val.get("importance").and_then(|v| v.as_i64()),
                ) {
                    use crate::db::models::{RuleType, WorldRule};
                    all_rules.push(WorldRule {
                        id: Uuid::new_v4().to_string(),
                        name: name.to_string(),
                        description: Some(desc.to_string()),
                        rule_type: match rule_type {
                            "physical" => RuleType::Physical,
                            "magic" => RuleType::Magic,
                            "social" => RuleType::Social,
                            "historical" => RuleType::Historical,
                            _ => RuleType::Custom,
                        },
                        importance: importance as i32,
                    });
                }
            }
        }

        let history_update = updates.get("history_update").and_then(|v| v.as_str());
        let new_history = if let Some(update) = history_update {
            Some(format!(
                "{}\n\n【更新】{}",
                wb.history.as_deref().unwrap_or(""),
                update
            ))
        } else {
            wb.history.clone()
        };

        wb_repo
            .update(
                &wb.id,
                new_concept,
                Some(&all_rules),
                new_history.as_deref(),
                None,
            )
            .map_err(AppError::from)?;

        Ok(serde_json::json!({
            "world_building_id": wb.id,
            "message": "世界观设定已更新",
        }))
    }

    async fn execute_update_scene(
        &self,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let scene_id = params
            .get("scene_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let changes = params
            .get("changes")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let pool = self.app_handle.state::<crate::db::DbPool>();
        let scene_repo = crate::db::repositories::SceneRepository::new(pool.inner().clone());

        // 按ID或sequence_number查找场景
        let scene = if let Ok(Some(s)) = scene_repo.get_by_id(&scene_id) {
            s
        } else {
            let all = scene_repo.get_by_story(&story_id).map_err(AppError::from)?;
            if let Ok(seq) = scene_id.parse::<i32>() {
                all.into_iter()
                    .find(|s| s.sequence_number == seq)
                    .ok_or_else(|| format!("Scene '{}' not found", scene_id))?
            } else {
                return Err(AppError::internal(format!(
                    "Scene '{}' not found",
                    scene_id
                )));
            }
        };

        // 使用LLM解析修改意图
        let llm_service = crate::llm::LlmService::new(self.app_handle.clone());
        // v0.21.0: 优先从 PromptRegistry 读取（支持用户覆盖）
        if let Ok(tpl) = crate::prompts::registry::resolve_prompt(&self.pool, "planner_edit_scene")
        {
            let mut vars = std::collections::HashMap::new();
            vars.insert(
                "current_scene".to_string(),
                scene.title.as_deref().unwrap_or("未设定").to_string(),
            );
            vars.insert("user_request".to_string(), changes.replace('"', "'"));
            let prompt =
                crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars);
            let response = llm_service
                .generate_for_task(
                    TaskType::Editing,
                    prompt,
                    Some(1024),
                    Some(0.3),
                    Some("update_scene"),
                )
                .await?;
            return Ok(serde_json::from_str(&response.content)?);
        }
        let prompt = format!(
            r#"你是一位场景编辑助手。请根据用户的修改要求，生成新的场景属性。

当前场景：
- 标题：{}
- 戏剧目标：{}
- 外部压力：{}
- 地点：{}
- 时间：{}

用户修改要求："{}"

请用 JSON 格式回复，只包含需要修改的字段：
{{{{
  "title": "新标题（如不需要修改则留空或省略）",
  "dramatic_goal": "新戏剧目标（如不需要修改则留空或省略）",
  "external_pressure": "新外部压力（如不需要修改则留空或省略）",
  "setting_location": "新地点（如不需要修改则留空或省略）",
  "setting_time": "新时间（如不需要修改则留空或省略）"
}}}}

注意：只输出 JSON，不要其他内容。"#,
            scene.title.as_deref().unwrap_or("未设定"),
            scene.dramatic_goal.as_deref().unwrap_or("未设定"),
            scene.external_pressure.as_deref().unwrap_or("未设定"),
            scene.setting_location.as_deref().unwrap_or("未设定"),
            scene.setting_time.as_deref().unwrap_or("未设定"),
            changes.replace('"', "'")
        );

        let response = llm_service
            .generate_for_task(
                TaskType::Editing,
                prompt,
                Some(1024),
                Some(0.3),
                Some("update_scene"),
            )
            .await?;
        let content = response.content.trim();
        let json_str = if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
            &content[start..=end]
        } else {
            content
        };

        let updates: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse scene update JSON: {}", e))?;

        let mut scene_update = crate::db::repositories::SceneUpdate {
            title: updates
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            dramatic_goal: updates
                .get("dramatic_goal")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            external_pressure: updates
                .get("external_pressure")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            setting_location: updates
                .get("setting_location")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            setting_time: updates
                .get("setting_time")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            ..Default::default()
        };

        // 如果修改了关键设定，标记场景可能需要重写
        if scene_update.dramatic_goal.is_some() || scene_update.setting_location.is_some() {
            scene_update.execution_stage = Some("needs_rewrite".to_string());
        }

        scene_repo
            .update(&scene.id, &scene_update)
            .map_err(AppError::from)?;

        Ok(serde_json::json!({
            "scene_id": scene.id,
            "message": format!("场景 '{}' 已更新", scene.title.as_deref().unwrap_or("未命名")),
        }))
    }

    async fn execute_query_knowledge_graph(
        &self,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, AppError> {
        let story_id = params
            .get("story_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let pool = self.app_handle.state::<crate::db::DbPool>();
        let kg_repo = crate::db::repositories::KnowledgeGraphRepository::new(pool.inner().clone());

        // 简化查询：获取所有实体，由LLM筛选
        let entities = kg_repo
            .get_entities_by_story(&story_id)
            .map_err(AppError::from)?;

        let relevant: Vec<serde_json::Value> = entities
            .into_iter()
            .filter(|e| {
                let search_text = format!(
                    "{} {}",
                    e.name,
                    e.attributes
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                );
                query
                    .split_whitespace()
                    .any(|kw| search_text.to_lowercase().contains(&kw.to_lowercase()))
            })
            .take(10)
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "name": e.name,
                    "entity_type": e.entity_type,
                    "attributes": e.attributes,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "query": query,
            "results": relevant,
            "count": relevant.len(),
        }))
    }

    async fn execute_mcp_tool(
        &self,
        capability_id: &str,
        params: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value, AppError> {
        // capability_id格式: "mcp.{server_id}.{tool_name}"
        let parts: Vec<&str> = capability_id.splitn(3, '.').collect();
        if parts.len() != 3 {
            return Err(AppError::internal(format!(
                "Invalid MCP capability ID: {}",
                capability_id
            )));
        }
        let server_id = parts[1];
        let tool_name = parts[2];

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        // W2-B8: 支持内置 MCP 工具（server_id == "builtin"）
        if server_id == "builtin" {
            let config = crate::mcp::McpServerConfig {
                id: "builtin".to_string(),
                name: "Built-in Tools".to_string(),
                command: String::new(),
                args: vec![],
                env: std::collections::HashMap::new(),
                timeout_seconds: 30,
            };
            let server = crate::mcp::McpServer::new(config);
            server.start().await.map_err(AppError::from)?;
            let result = server
                .execute_tool(tool_name, arguments)
                .await
                .map_err(AppError::from)?;
            return Ok(result);
        }

        let mut connections = crate::MCP_CONNECTIONS.lock().await;
        let client = connections
            .get_mut(server_id)
            .ok_or_else(|| AppError::internal(format!("MCP server {} not connected", server_id)))?;

        let result = client
            .call_tool(tool_name, arguments)
            .await
            .map_err(|e| AppError::internal(format!("MCP tool call failed: {}", e)))?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_parameters_simple() {
        let mut params = HashMap::new();
        params.insert(
            "key1".to_string(),
            serde_json::Value::String("value1".to_string()),
        );

        let outputs = HashMap::new();
        let resolved = PlanExecutor::resolve_parameters(&params, &outputs);
        assert_eq!(resolved.get("key1").unwrap().as_str().unwrap(), "value1");
    }

    #[test]
    fn test_resolve_parameters_with_placeholder() {
        let mut params = HashMap::new();
        params.insert(
            "instruction".to_string(),
            serde_json::Value::String("基于{{step_1}}继续".to_string()),
        );

        let mut outputs = HashMap::new();
        let mut step_output = serde_json::Map::new();
        step_output.insert(
            "content".to_string(),
            serde_json::Value::String("前文内容".to_string()),
        );
        outputs.insert("step_1".to_string(), serde_json::Value::Object(step_output));

        let resolved = PlanExecutor::resolve_parameters(&params, &outputs);
        assert_eq!(
            resolved.get("instruction").unwrap().as_str().unwrap(),
            "基于前文内容继续"
        );
    }

    #[test]
    fn test_resolve_parameters_multiple_placeholders() {
        let mut params = HashMap::new();
        params.insert(
            "combined".to_string(),
            serde_json::Value::String("{{a}} and {{b}}".to_string()),
        );

        let mut outputs = HashMap::new();
        let mut out_a = serde_json::Map::new();
        out_a.insert(
            "content".to_string(),
            serde_json::Value::String("Alpha".to_string()),
        );
        outputs.insert("a".to_string(), serde_json::Value::Object(out_a));

        let mut out_b = serde_json::Map::new();
        out_b.insert(
            "content".to_string(),
            serde_json::Value::String("Beta".to_string()),
        );
        outputs.insert("b".to_string(), serde_json::Value::Object(out_b));

        let resolved = PlanExecutor::resolve_parameters(&params, &outputs);
        assert_eq!(
            resolved.get("combined").unwrap().as_str().unwrap(),
            "Alpha and Beta"
        );
    }

    #[test]
    fn test_resolve_parameters_missing_placeholder() {
        let mut params = HashMap::new();
        params.insert(
            "text".to_string(),
            serde_json::Value::String("{{missing}}".to_string()),
        );

        let outputs = HashMap::new();
        let resolved = PlanExecutor::resolve_parameters(&params, &outputs);
        // 当依赖步骤不存在时，保留原始占位符（不静默删除，便于调试）
        assert_eq!(
            resolved.get("text").unwrap().as_str().unwrap(),
            "{{missing}}"
        );
    }

    // v0.30.9: Inspector draft 兜底注入测试

    fn make_step_output(content: &str) -> serde_json::Value {
        let mut obj = serde_json::Map::new();
        obj.insert(
            "content".to_string(),
            serde_json::Value::String(content.to_string()),
        );
        serde_json::Value::Object(obj)
    }

    #[test]
    fn test_inspector_draft_fallback_injects_from_depends_on() {
        let mut rp = HashMap::new();
        // inspector 没有 draft 参数
        let mut outputs = HashMap::new();
        outputs.insert("step_1".to_string(), make_step_output("第一章正文内容"));

        let depends_on = vec!["step_1".to_string()];
        let injected =
            PlanExecutor::inject_inspector_draft_fallback(&mut rp, &depends_on, &outputs);

        assert!(injected);
        assert_eq!(rp.get("draft").unwrap().as_str().unwrap(), "第一章正文内容");
    }

    #[test]
    fn test_inspector_draft_fallback_skips_when_draft_present() {
        let mut rp = HashMap::new();
        rp.insert(
            "draft".to_string(),
            serde_json::Value::String("已有草稿".to_string()),
        );
        let outputs = HashMap::new();

        let injected = PlanExecutor::inject_inspector_draft_fallback(&mut rp, &[], &outputs);

        assert!(!injected);
        assert_eq!(rp.get("draft").unwrap().as_str().unwrap(), "已有草稿");
    }

    #[test]
    fn test_inspector_draft_fallback_scans_all_outputs_when_dep_not_found() {
        let mut rp = HashMap::new();
        let mut outputs = HashMap::new();
        // depends_on 指向不存在的 step_99，但 step_1 有内容
        outputs.insert("step_1".to_string(), make_step_output("扫描兜底内容"));

        let depends_on = vec!["step_99".to_string()];
        let injected =
            PlanExecutor::inject_inspector_draft_fallback(&mut rp, &depends_on, &outputs);

        assert!(injected);
        assert_eq!(rp.get("draft").unwrap().as_str().unwrap(), "扫描兜底内容");
    }

    #[test]
    fn test_inspector_draft_fallback_no_content_returns_false() {
        let mut rp = HashMap::new();
        let outputs = HashMap::new();

        let injected = PlanExecutor::inject_inspector_draft_fallback(&mut rp, &[], &outputs);

        assert!(!injected);
        assert!(rp.get("draft").is_none());
    }

    #[test]
    fn test_inspector_draft_fallback_skips_empty_content() {
        let mut rp = HashMap::new();
        let mut outputs = HashMap::new();
        outputs.insert("step_1".to_string(), make_step_output(""));

        let injected = PlanExecutor::inject_inspector_draft_fallback(
            &mut rp,
            &["step_1".to_string()],
            &outputs,
        );

        assert!(!injected);
        assert!(rp.get("draft").is_none());
    }

    // v0.30.10: content 兜底注入测试（style_mimic / plot_analyzer / builtin 技能）

    #[test]
    fn test_content_fallback_injects_from_depends_on() {
        let mut rp = HashMap::new();
        let mut outputs = HashMap::new();
        outputs.insert("step_1".to_string(), make_step_output("第一章正文内容"));

        let injected =
            PlanExecutor::inject_content_fallback(&mut rp, &["step_1".to_string()], &outputs, None);

        assert!(injected);
        assert_eq!(
            rp.get("content").unwrap().as_str().unwrap(),
            "第一章正文内容"
        );
    }

    #[test]
    fn test_content_fallback_skips_when_content_present() {
        let mut rp = HashMap::new();
        rp.insert(
            "content".to_string(),
            serde_json::Value::String("已有内容".to_string()),
        );
        let outputs = HashMap::new();

        let injected = PlanExecutor::inject_content_fallback(&mut rp, &[], &outputs, None);

        assert!(!injected);
        assert_eq!(rp.get("content").unwrap().as_str().unwrap(), "已有内容");
    }

    #[test]
    fn test_content_fallback_uses_current_content_when_no_outputs() {
        let mut rp = HashMap::new();
        let outputs = HashMap::new();

        let injected =
            PlanExecutor::inject_content_fallback(&mut rp, &[], &outputs, Some("编辑器当前正文"));

        assert!(injected);
        assert_eq!(
            rp.get("content").unwrap().as_str().unwrap(),
            "编辑器当前正文"
        );
    }

    #[test]
    fn test_content_fallback_no_content_returns_false() {
        let mut rp = HashMap::new();
        let outputs = HashMap::new();

        let injected = PlanExecutor::inject_content_fallback(&mut rp, &[], &outputs, None);

        assert!(!injected);
        assert!(rp.get("content").is_none());
    }

    #[test]
    fn test_content_fallback_prefers_outputs_over_current_content() {
        let mut rp = HashMap::new();
        let mut outputs = HashMap::new();
        outputs.insert("step_1".to_string(), make_step_output("writer输出"));

        let injected = PlanExecutor::inject_content_fallback(
            &mut rp,
            &["step_1".to_string()],
            &outputs,
            Some("编辑器当前正文"),
        );

        assert!(injected);
        // 优先用 step_outputs 的 content，不用 current_content
        assert_eq!(rp.get("content").unwrap().as_str().unwrap(), "writer输出");
    }
}
