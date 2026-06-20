#![allow(dead_code)]
//! Agent Orchestrator - Agent 协作编排器
//!
//! 实现 Agent 间的协作工作流，支持反馈闭环：
//! Writer → Inspector → Writer(改写) → ...
//!
//! 幕后运行，幕前只呈现最终结果。

use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{timeout, Duration};

use super::{
    service::{AgentService, AgentTask, AgentType},
    AgentResult,
};
use crate::{
    creative_engine::style::{StyleChecker, StyleDNA},
    db::{repositories::StyleDnaRepository, DbPool},
    error::AppError,
    events::{emit_generation_status, GenerationPhase},
};

/// 生成模式 — 决定 Orchestrator 执行路径
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenerationMode {
    /// 快速模式：单轮 LLM，跳过 Inspector / StyleChecker
    /// 适用于 Ghost Text、实时补全等低延迟场景
    Fast,
    /// 分时模式：Writer 单轮 + 最小约束（WriteTimeBundle），立即返回正文；
    /// 后台异步触发审计（Inspector → annotation 回流）。
    /// 适用于普通生成、auto_write 等追求速度的场景。
    /// 设计依据：docs/plans/2026-06-14-time-sliced-intervention-design.md
    /// Phase 0 实测：最小约束 vs 全量资产平均差距 7.9%（< 30%
    /// 阈值），架构成立。
    TimeSliced,
    /// 完整模式：Writer → Inspector → Writer 反馈闭环（同步阻塞）
    /// 适用于向导首场景、Genesis、Planner、Workflow
    /// 等明确需要专业同步成品的场景
    Full,
}

impl GenerationMode {
    pub fn name(&self) -> &'static str {
        match self {
            GenerationMode::Fast => "快速",
            GenerationMode::TimeSliced => "分时",
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
    /// 综合分数达到该阈值时跳过改写闭环，直接返回结果（0.0 - 1.0，默认 0.90）
    ///
    /// 用于降低 Full 模式的平均等待时间：当 Inspector 已经给出较高评价时，
    /// 不再强制进行 Writer→Inspector→Rewrite 循环。
    pub skip_rewrite_threshold: f32,
    /// 候选生成阶段单个远程候选的 LLM 超时（秒，默认 120）
    pub candidate_timeout_seconds: u64,
    /// 候选生成阶段单个本地候选的 LLM 超时（秒，默认 60）
    pub candidate_timeout_local_seconds: u64,
    /// 候选生成阶段单个候选的最大重试次数（默认 0）
    ///
    /// 候选阶段本身已生成多个版本，单个候选失败应快速跳过，不应再重试，
    /// 否则 120s 超时 × 2 次 × 多个候选会迅速累积到 500s 以上。
    pub candidate_max_retries: u32,
    /// 本地模型是否在候选阶段串行生成（默认 false）
    ///
    /// 早期默认 true 是为了避免本地服务端排队，但实际导致候选 1 完全阻塞
    /// 候选 2，一旦候选 1 超时/挂起，用户就只能空等。改为默认并行，并
    /// 通过更短的本地超时（60s）来避免排队影响。
    /// v0.11.8: 该字段已弃用，候选阶段始终并行。
    pub candidate_local_sequential: bool,
    /// 候选生成阶段候选数量（默认 1，远端模型可在 1–2 之间配置）
    pub candidate_count: u32,
}

impl WorkflowConfig {
    /// 从应用配置构造工作流配置，确保用户设置优先于硬编码默认值。
    pub fn from_app_config(config: &crate::config::AppConfig) -> Self {
        Self {
            rewrite_threshold: config.rewrite_threshold,
            max_feedback_loops: config.max_feedback_loops,
            keep_revision_history: config.keep_revision_history,
            style_weight: config.style_weight,
            narrative_weight: config.narrative_weight,
            skip_rewrite_threshold: config.skip_rewrite_threshold,
            candidate_timeout_seconds: config.candidate_timeout_seconds,
            candidate_timeout_local_seconds: config.candidate_timeout_local_seconds,
            candidate_max_retries: config.candidate_max_retries,
            candidate_local_sequential: config.candidate_local_sequential,
            candidate_count: config.candidate_count.max(1).min(2),
        }
    }
}

impl Default for WorkflowConfig {
    fn default() -> Self {
        Self {
            rewrite_threshold: 0.75,
            max_feedback_loops: 2,
            keep_revision_history: true,
            style_weight: 0.5,
            narrative_weight: 0.5,
            skip_rewrite_threshold: 0.90,
            candidate_timeout_seconds: 120,
            candidate_timeout_local_seconds: 60,
            candidate_max_retries: 0,
            candidate_local_sequential: false,
            candidate_count: 1,
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

/// 结构化 generation trace 日志辅助。
/// v0.11.x (C2): 按 request_id/task_id 聚合各阶段耗时，info 输出总体，debug
/// 输出详细阶段。
#[derive(Clone)]
pub(crate) struct GenerationTrace {
    request_id: String,
}

impl GenerationTrace {
    fn new(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
        }
    }

    fn log_phase(&self, phase: &str, elapsed_ms: u128, details: Option<&str>) {
        log::debug!(
            target: "generation_trace",
            "{}",
            serde_json::json!({
                "event": "generation_trace",
                "request_id": self.request_id,
                "phase": phase,
                "elapsed_ms": elapsed_ms,
                "details": details.unwrap_or(""),
            })
        );
    }

    fn log_total(&self, elapsed_ms: u128, details: Option<&str>) {
        log::info!(
            target: "generation_trace",
            "{}",
            serde_json::json!({
                "event": "generation_trace",
                "request_id": self.request_id,
                "phase": "total",
                "elapsed_ms": elapsed_ms,
                "details": details.unwrap_or(""),
            })
        );
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
    /// v0.9.3: 增加 detail 字段，用于描述候选进度、回炉原因等更细粒度的状态
    fn emit_step_event(
        &self,
        task_id: &str,
        step_type: WorkflowStepType,
        loop_idx: Option<u32>,
        score: Option<f32>,
        detail: Option<&str>,
    ) {
        let mut event = serde_json::json!({
            "task_id": task_id,
            "step_type": step_type.name(),
            "loop_idx": loop_idx,
            "score": score.map(|s| (s * 100.0) as i32),
        });
        if let Some(d) = detail {
            event["detail"] = serde_json::Value::String(d.to_string());
        }
        let _ = self.app_handle.emit("orchestrator-step", event);
    }

    /// 发射工作流整体完成/失败事件，让前端 backendActivityStore 结束
    /// orchestrator 活动。
    fn emit_step_status_event(&self, task_id: &str, status: &str, message: &str) {
        let _ = self.app_handle.emit(
            "orchestrator-step",
            serde_json::json!({
                "task_id": task_id,
                "step_type": "完成",
                "status": status,
                "detail": message,
            }),
        );
    }

    /// 发射统一生成状态事件 `generation-status`
    ///
    /// 与 `orchestrator-step`
    /// 并存，供新版前端统一消费；旧版事件继续发射以保持兼容。
    fn emit_generation_status(
        &self,
        task_id: &str,
        phase: GenerationPhase,
        progress: f32,
        message: impl Into<String>,
        request_id: Option<String>,
    ) {
        emit_generation_status(
            &self.app_handle,
            task_id,
            phase,
            progress,
            message,
            request_id,
        );
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
        // C1: 记录任务开始并发射统一的准备阶段事件
        crate::events::record_generation_start(&task.id);
        self.emit_generation_status(
            &task.id,
            GenerationPhase::PreparingContext,
            0.05,
            "准备创作上下文...",
            None,
        );

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

        let trace = GenerationTrace::new(task.id.clone());
        let generation_start = std::time::Instant::now();

        let result = match mode {
            GenerationMode::Fast => self.execute_fast(task.clone(), &trace).await,
            GenerationMode::TimeSliced => self.execute_time_sliced(task.clone(), &trace).await,
            GenerationMode::Full => self.execute_full(task.clone(), &trace).await,
        };

        trace.log_total(
            generation_start.elapsed().as_millis(),
            Some(&format!("mode={:?}", mode)),
        );

        // v0.8.0: 自动写入记忆（创作完成后）
        // v0.9.5: 同时触发完整采摘（IngestPipeline → KG + 向量索引）
        // v0.11.x (C2): 增加 Semaphore 背压与 CancellationToken 取消传播。
        if let Ok(ref workflow_result) = result {
            self.emit_generation_status(
                &task.id,
                GenerationPhase::SavingMemory,
                0.95,
                "保存记忆并更新知识图谱...",
                workflow_result.request_id.clone(),
            );

            // 若用户已取消该次生成，则不再启动后台 ingest。
            if let Some(ref req_id) = workflow_result.request_id {
                if self.service.is_cancelled(req_id) {
                    log::info!(
                        "[AgentOrchestrator] Generation {} was cancelled, skipping background ingest",
                        req_id
                    );
                    return result;
                }
            }

            let pool = self.app_handle.state::<crate::db::DbPool>();
            let writer = crate::memory::writer::MemoryWriter::new(pool.inner().clone());
            let story_id = task.context.story.story_id.clone();
            let chapter_number = task.context.narrative.chapter_number as i32;
            let content = workflow_result.final_content.clone();
            let app_handle = self.app_handle.clone();
            let request_id = workflow_result.request_id.clone();

            // 注册后台 ingest 取消令牌；用户调用 cancel_generation(request_id)
            // 时可传播取消。
            let parent_token = request_id
                .as_ref()
                .map(|req_id| crate::memory::writer::register_ingest_cancel_token(req_id));

            tauri::async_runtime::spawn(async move {
                // 全局并发背压：最多同时运行 2 个 ingest 后台任务。
                let permit = crate::memory::writer::MEMORY_WRITER_SEMAPHORE
                    .acquire()
                    .await;
                if permit.is_err() {
                    log::warn!("[AgentOrchestrator] Failed to acquire ingest permit");
                    if let Some(ref req_id) = request_id {
                        crate::memory::writer::take_ingest_cancel_token(req_id);
                    }
                    return;
                }
                let _permit = permit.unwrap();

                let child_token = parent_token.as_ref().map(|t| t.child_token());
                let cancel_ref = child_token.as_ref();

                if cancel_ref.map(|t| t.is_cancelled()).unwrap_or(false) {
                    log::info!(
                        "[AgentOrchestrator] Background ingest cancelled before start for story {}",
                        story_id
                    );
                    if let Some(ref req_id) = request_id {
                        crate::memory::writer::take_ingest_cancel_token(req_id);
                    }
                    return;
                }

                match writer
                    .write_with_cancel(&story_id, chapter_number, &content, cancel_ref)
                    .await
                {
                    Ok(_) => {
                        log::info!("[AgentOrchestrator] Memory updated for story {}", story_id);

                        // 触发完整采摘
                        let pool_for_ingest = app_handle.state::<crate::db::DbPool>();
                        let llm_service = crate::llm::LlmService::new(app_handle.clone());
                        let pipeline = crate::memory::ingest::IngestPipeline::new(llm_service)
                            .with_pool(pool_for_ingest.inner().clone())
                            .with_app_handle(app_handle.clone());
                        let ingest_content = crate::memory::ingest::IngestContent {
                            text: content,
                            source: format!("smart_execute:chapter:{}", chapter_number),
                            story_id: story_id.clone(),
                            scene_id: None,
                        };

                        match pipeline
                            .ingest_with_cancel(&ingest_content, cancel_ref)
                            .await
                        {
                            Ok(ingest_result) => {
                                let kg_repo =
                                    crate::db::repositories::KnowledgeGraphRepository::new(
                                        pool_for_ingest.inner().clone(),
                                    );
                                if let Err(e) = kg_repo.save_entities_batch(&ingest_result.entities)
                                {
                                    log::warn!(
                                        "[AgentOrchestrator] Failed to save ingest entities: {}",
                                        e
                                    );
                                }
                                if let Err(e) =
                                    kg_repo.save_relations_batch(&ingest_result.relations)
                                {
                                    log::warn!(
                                        "[AgentOrchestrator] Failed to save ingest relations: {}",
                                        e
                                    );
                                }
                                log::info!(
                                    "[AgentOrchestrator] Ingest completed for story {}: {} entities, {} relations",
                                    story_id,
                                    ingest_result.entities.len(),
                                    ingest_result.relations.len()
                                );
                            }
                            Err(e) => {
                                log::warn!("[AgentOrchestrator] IngestPipeline failed: {}", e)
                            }
                        }
                    }
                    Err(e) => log::warn!("[AgentOrchestrator] Memory write failed: {}", e),
                }

                // 任务完成（无论成功与否），清理取消令牌注册表。
                if let Some(ref req_id) = request_id {
                    crate::memory::writer::take_ingest_cancel_token(req_id);
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

        // v0.11.2: 发出完成/失败状态事件，让前端 backendActivityStore 正确结束活动
        match &result {
            Ok(r) => {
                self.emit_step_status_event(&task.id, "completed", "创作完成");
                self.emit_generation_status(
                    &task.id,
                    GenerationPhase::Completed,
                    1.0,
                    "创作完成",
                    r.request_id.clone(),
                );
            }
            Err(e) => {
                self.emit_step_status_event(&task.id, "failed", &e.to_string());
                self.emit_generation_status(
                    &task.id,
                    GenerationPhase::Error,
                    0.0,
                    format!("创作失败: {}", e),
                    None,
                );
            }
        }

        result
    }

    /// Fast 模式：单轮 LLM 生成，跳过 Inspector / StyleChecker
    async fn execute_fast(
        &self,
        task: AgentTask,
        trace: &GenerationTrace,
    ) -> Result<WorkflowResult, AppError> {
        self.emit_step_event(&task.id, WorkflowStepType::Generation, None, None, None);
        self.emit_generation_status(
            &task.id,
            GenerationPhase::GeneratingCandidates,
            0.2,
            "正在生成内容...",
            None,
        );
        let writer_start = std::time::Instant::now();
        let writer_result = Box::pin(self.service.execute_writer_raw(task.clone())).await?;
        trace.log_phase(
            "writer",
            writer_start.elapsed().as_millis(),
            Some("fast mode single-pass writer"),
        );

        let steps = vec![WorkflowStepResult {
            step_type: WorkflowStepType::Generation,
            agent_type: AgentType::Writer,
            content: writer_result.content.clone(),
            score: writer_result.score,
            suggestions: writer_result.suggestions.clone(),
        }];

        let skill_start = std::time::Instant::now();
        let final_content = self
            .apply_writing_skills(&task.context, &writer_result.content)
            .await;
        trace.log_phase(
            "apply_writing_skills",
            skill_start.elapsed().as_millis(),
            None,
        );

        self.emit_generation_status(
            &task.id,
            GenerationPhase::FinalOutput,
            0.9,
            "整理最终输出...",
            writer_result.request_id.clone(),
        );

        Ok(WorkflowResult {
            final_content,
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

    /// TimeSliced 模式（分时介入）：最小约束 + 单轮 Writer + 跳过
    /// Inspector/Rewrite。
    ///
    /// 设计依据：docs/plans/2026-06-14-time-sliced-intervention-design.md
    /// 与 Fast 的区别：用 QuickPreflightChecker（仅角色非空），不触发
    /// auto_contract。 与 Full 的区别：跳过 Inspector 7 维审计、Rewrite
    /// 循环、apply_writing_skills。 审计在时间线 2（Phase 2 的
    /// AuditExecutor）异步进行，问题以 annotation 回流。
    ///
    /// 任务 1.6（做法 B）：用 WriteTimeBundle 构建精简 prompt，直接调
    /// generate_for_task， 绕过 execute_writer_raw（及其内嵌的 Full
    /// Preflight + auto_contract）。 Fast/Full 路径完全不受影响。
    async fn execute_time_sliced(
        &self,
        task: AgentTask,
        trace: &GenerationTrace,
    ) -> Result<WorkflowResult, AppError> {
        self.emit_step_event(
            &task.id,
            WorkflowStepType::Generation,
            None,
            None,
            Some("分时模式：最小约束快速生成"),
        );
        self.emit_generation_status(
            &task.id,
            GenerationPhase::PreparingContext,
            0.1,
            "快速预检...",
            None,
        );

        let pool = self.app_handle.state::<crate::db::DbPool>();

        // 时间线 1 预检：仅角色非空，失败直接报错，不触发 auto_contract
        let quick_check = crate::story_system::preflight::QuickPreflightChecker::check(
            pool.inner(),
            &task.context.story.story_id,
        )
        .await;
        if !quick_check.ready {
            log::info!(
                "[TimeSliced] QuickPreflight failed for story {}: {:?}",
                task.context.story.story_id,
                quick_check.blocking_issues
            );
            return Err(AppError::preflight_failed(
                "分时模式预检未通过（缺少角色）",
                quick_check.blocking_issues,
            ));
        }

        self.emit_generation_status(
            &task.id,
            GenerationPhase::PreparingContext,
            0.2,
            "加载写作约束...",
            None,
        );

        // 加载 WriteTimeBundle（最小约束包），全部 DB 查询在 spawn_blocking 内
        let story_id = task.context.story.story_id.clone();
        let chapter_number = task.context.narrative.chapter_number as i32;
        let pool_clone = pool.inner().clone();
        let bundle_start = std::time::Instant::now();
        let bundle = tokio::task::spawn_blocking(move || {
            crate::creative_engine::write_time_bundle::WriteTimeBundle::load_sync(
                &pool_clone,
                &story_id,
                chapter_number,
                None, // style_slice_override：任务 1.6 暂不接入 StyleDna，留空
            )
        })
        .await
        .map_err(|e| AppError::internal(format!("[TimeSliced] bundle 加载任务失败: {}", e)))??;
        trace.log_phase(
            "bundle_load",
            bundle_start.elapsed().as_millis(),
            Some("write-time-bundle spawn_blocking"),
        );

        // P1-1: 从 task.parameters 提取叙事四元组，注入 TimeSliced 路径。
        // 此前 TimeSliced 绕过 build_writer_prompt，四元组虽已序列化进 parameters
        // 却被忽略。现在接通，让 v0.17 核心资产在默认续写路径生效。
        let mut bundle = bundle;
        if let Some(quartet_val) = task.parameters.get("narrative_quartet") {
            if let Some(rendered) =
                crate::agents::service::render_narrative_quartet_section(quartet_val)
            {
                bundle.narrative_quartet = Some(rendered);
            }
        }

        // 构建精简 prompt：bundle 约束 + 用户指令
        let bundle_prompt = bundle.to_prompt();
        let user_instruction = if task.input.trim().is_empty() {
            "请续写下一段正文。".to_string()
        } else {
            task.input.clone()
        };
        let prompt = format!(
            "你是一名专业的小说作者。请根据以下设定写一段正文（800-1500字）。\n\n\
             {bundle_prompt}\n\n\
             【创作指令】\n{user_instruction}\n\n\
             请直接输出正文，不要写说明、标题或分章标记。"
        );

        self.emit_generation_status(
            &task.id,
            GenerationPhase::GeneratingCandidates,
            0.4,
            "正在生成内容...",
            None,
        );

        // 直接调 LLM（走路由器选 Writer 模型），绕过 execute_writer_raw
        let writer_start = std::time::Instant::now();
        let is_local = self.service.is_target_model_local(AgentType::Writer);
        let _writer_permit = self
            .service
            .llm_service_ref()
            .acquire_writer_permit(is_local)
            .await?;
        let gen_response = self
            .service
            .llm_service_ref()
            .generate_for_task(
                crate::router::TaskType::CreativeWriting,
                prompt,
                Some(2048),
                Some(0.75),
                Some("time-sliced-writer"),
            )
            .await?;
        trace.log_phase(
            "writer",
            writer_start.elapsed().as_millis(),
            Some("time-sliced direct generate_for_task"),
        );

        let content = gen_response.content;
        let request_id = gen_response.model.clone();

        let steps = vec![WorkflowStepResult {
            step_type: WorkflowStepType::Generation,
            agent_type: AgentType::Writer,
            content: content.clone(),
            score: None,
            suggestions: vec![],
        }];

        // 跳过 apply_writing_skills（Pro 专属）、Inspector、Rewrite
        self.emit_generation_status(
            &task.id,
            GenerationPhase::Completed,
            1.0,
            "生成完成",
            Some(request_id.clone()),
        );

        // 时间线 2：后台异步审计（不阻塞返回）。
        // 正文已生成，spawn AuditExecutor 跑 Inspector，问题以 annotation 回流。
        let audit_content = content.clone();
        let audit_story_id = task.context.story.story_id.clone();
        let audit_pool = pool.inner().clone();
        let audit_handle = self.app_handle.clone();
        let audit_chapter_number = task.context.narrative.chapter_number as i32;
        let audit_story_title = bundle.story_meta.title.clone();
        let audit_genre = bundle.story_meta.genre.clone();
        tokio::spawn(async move {
            let executor = crate::task_system::audit_executor::AuditExecutor {
                pool: audit_pool,
                app_handle: audit_handle,
            };
            executor
                .run_audit(crate::task_system::audit_executor::AuditPayload {
                    story_id: audit_story_id,
                    scene_id: None, // TODO: 任务 2.3 后续接入 scene_id
                    chapter_id: None,
                    chapter_number: audit_chapter_number,
                    content: audit_content,
                    story_title: Some(audit_story_title),
                    genre: audit_genre,
                })
                .await;
        });

        // 时间线 3：条件触发深度洞察（每 N 段，默认 5）。
        // 不阻塞返回，在后台异步跑，报告写入 story_summaries。
        let insight_pool = pool.inner().clone();
        let insight_story_id = task.context.story.story_id.clone();
        let insight_chapter = task.context.narrative.chapter_number as i32;
        let insight_handle = self.app_handle.clone();
        tokio::task::spawn_blocking(move || {
            let should = crate::task_system::insight_executor::InsightExecutor::should_trigger(
                &insight_pool,
                &insight_story_id,
                insight_chapter,
                5, // 默认每 5 段触发
            );
            (
                should,
                insight_pool,
                insight_story_id,
                insight_chapter,
                insight_handle,
            )
        })
        .await
        .ok()
        .map(|(should, ipool, istory, ichapter, ihandle)| {
            if should {
                tokio::spawn(async move {
                    let executor = crate::task_system::insight_executor::InsightExecutor {
                        pool: ipool,
                        app_handle: ihandle,
                    };
                    executor
                        .run_insight(crate::task_system::insight_executor::InsightPayload {
                            story_id: istory,
                            chapter_number: ichapter,
                            trend_window: 5,
                        })
                        .await;
                });
            }
        });

        Ok(WorkflowResult {
            final_content: content,
            final_score: 1.0,
            style_score: 0.0,
            narrative_score: 0.0,
            drift_details: Vec::new(),
            steps,
            was_rewritten: false,
            rewrite_count: 0,
            request_id: Some(request_id),
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
        trace: &GenerationTrace,
    ) -> Result<WorkflowResult, AppError> {
        // v0.13.4: 为 Full 模式设置整体时间预算（270 秒），避免多次 LLM 调用
        //（候选 + Inspector + Rewrite）累积超过前端 330 秒超时，导致前端先超时。
        const FULL_MODE_BUDGET_SECONDS: u64 = 270;
        let total_start = std::time::Instant::now();
        let remaining_budget_secs = || {
            FULL_MODE_BUDGET_SECONDS
                .saturating_sub(total_start.elapsed().as_secs())
                .max(1)
        };

        let mut steps: Vec<WorkflowStepResult> = Vec::new();
        let mut rewrite_count: u32 = 0;
        let mut was_rewritten = false;

        // 步骤1: Writer 生成初稿
        self.emit_step_event(&task.id, WorkflowStepType::Generation, None, None, None);
        self.emit_generation_status(
            &task.id,
            GenerationPhase::GeneratingCandidates,
            0.2,
            "正在生成初稿...",
            None,
        );

        // v0.7.8: 2 候选并行生成选优（续写场景且有风格指纹时启用）
        let candidate_start = std::time::Instant::now();
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
                let (r, req_id, content) = self.generate_candidates(&task, 2, trace).await?;
                (r, Some(req_id), content)
            } else {
                let result = Box::pin(self.service.execute_writer_raw(task.clone())).await?;
                let req_id = result.request_id.clone();
                let content = result.content.clone();
                (result, req_id, content)
            };
        trace.log_phase(
            "candidates",
            candidate_start.elapsed().as_millis(),
            Some("writer initial draft / candidate generation"),
        );

        steps.push(WorkflowStepResult {
            step_type: WorkflowStepType::Generation,
            agent_type: AgentType::Writer,
            content: current_content.clone(),
            score: writer_result.score,
            suggestions: writer_result.suggestions.clone(),
        });

        // 反馈循环
        for loop_idx in 0..self.config.max_feedback_loops {
            // v0.9.5: 协作式取消检查 —— 若上层已请求取消，则立即退出闭环
            if let Some(ref req_id) = request_id {
                if self.service.is_cancelled(req_id) {
                    log::info!("[AgentOrchestrator] Cancellation requested for request_id {}, stopping feedback loop", req_id);
                    self.emit_generation_status(
                        &task.id,
                        GenerationPhase::Cancelled,
                        0.0,
                        "生成已取消",
                        request_id.clone(),
                    );
                    return Ok(WorkflowResult {
                        final_content: current_content,
                        final_score: 0.0,
                        style_score: 0.0,
                        narrative_score: 0.0,
                        drift_details: vec![],
                        steps,
                        was_rewritten,
                        rewrite_count,
                        request_id: request_id.clone(),
                    });
                }
            }

            // v0.14.0: 预算检查——剩余时间不足 30 秒时跳过 Inspector/Rewrite，
            // 直接返回 Writer 结果，避免在预算耗尽后启动无法完成的 LLM 调用。
            if remaining_budget_secs() < 30 {
                log::info!(
                    "[AgentOrchestrator] Remaining budget {}s < 30s, skipping inspector loop {}",
                    remaining_budget_secs(),
                    loop_idx
                );
                break;
            }

            // v0.14.0: 预算检查——剩余时间不足 30 秒时跳过 Inspector/Rewrite，
            // 直接返回 Writer 结果，避免在预算耗尽后启动无法完成的 LLM 调用。
            if remaining_budget_secs() < 30 {
                log::info!(
                    "[AgentOrchestrator] Remaining budget {}s < 30s, skipping inspector loop {}",
                    remaining_budget_secs(),
                    loop_idx
                );
                break;
            }

            // 步骤2: Inspector 质检
            self.emit_step_event(
                &task.id,
                WorkflowStepType::Inspection,
                Some(loop_idx),
                None,
                Some("正在评估内容与风格一致性..."),
            );
            self.emit_generation_status(
                &task.id,
                GenerationPhase::Inspecting,
                0.5,
                format!("Inspector 审校（第 {} 轮）...", loop_idx + 1),
                request_id.clone(),
            );
            let inspect_task = AgentTask {
                id: format!("{}-inspect-{}", task.id, loop_idx),
                agent_type: AgentType::Inspector,
                context: task.context.clone(),
                input: current_content.clone(),
                parameters: task.parameters.clone(),
                tier: None,
            };

            let inspect_start = std::time::Instant::now();
            // v0.14.0: 激活 Full 模式预算——Inspector 调用受剩余时间约束
            let inspect_budget = remaining_budget_secs();
            let inspect_result = match tokio::time::timeout(
                std::time::Duration::from_secs(inspect_budget),
                Box::pin(self.service.execute_task(inspect_task)),
            )
            .await
            {
                Ok(r) => r?,
                Err(_) => {
                    log::warn!(
                        "[Orchestrator] Inspector timed out after {}s (budget exhausted), skipping remaining loops",
                        inspect_budget
                    );
                    // Inspector 超时：用当前内容作为最终结果，跳过后续改写
                    break;
                }
            };
            trace.log_phase(
                "inspector",
                inspect_start.elapsed().as_millis(),
                Some(&format!("loop {}", loop_idx)),
            );
            let base_inspect_score = inspect_result.score.unwrap_or(0.0);
            let mut style_issues = Vec::new();

            // v0.7.8: 双轨评分 — 从 Inspector JSON 响应中解析风格分数
            let (style_score, narrative_score, drift_details) =
                Self::parse_inspector_style_analysis(&inspect_result.content, base_inspect_score);

            // 综合分数 = 风格分 * style_weight + 叙事分 * narrative_weight
            let composite_score = style_score * self.config.style_weight
                + narrative_score * self.config.narrative_weight;

            // StyleChecker 验证（保留原有逻辑作为兜底）
            let db_start = std::time::Instant::now();
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
            trace.log_phase(
                "db_query_style_dna",
                db_start.elapsed().as_millis(),
                Some(&format!("loop {}", loop_idx)),
            );

            self.emit_step_event(
                &task.id,
                WorkflowStepType::Inspection,
                Some(loop_idx),
                Some(composite_score),
                None,
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

            // v0.9.5: 综合分数已足够高时，跳过改写闭环以降低等待时间
            let composite_good_enough = composite_score >= self.config.skip_rewrite_threshold;

            if style_ok && narrative_ok || composite_good_enough {
                if composite_good_enough && !(style_ok && narrative_ok) {
                    log::info!(
                        "[AgentOrchestrator] Composite score {:.2} >= skip_rewrite_threshold {:.2}, skipping rewrite loop",
                        composite_score, self.config.skip_rewrite_threshold
                    );
                }
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
            let rewrite_reason = format!(
                "质检未达标（风格 {:.0}%，叙事 {:.0}%），进入第 {} 轮改写优化",
                style_score * 100.0,
                narrative_score * 100.0,
                loop_idx + 1
            );
            self.emit_step_event(
                &task.id,
                WorkflowStepType::Rewrite,
                Some(loop_idx),
                None,
                Some(&rewrite_reason),
            );
            self.emit_generation_status(
                &task.id,
                GenerationPhase::Rewriting,
                0.7,
                format!("改写优化（第 {} 轮）...", loop_idx + 1),
                request_id.clone(),
            );
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

            let rewrite_start = std::time::Instant::now();
            // v0.14.0: Rewrite 同样受剩余时间预算约束
            let rewrite_budget = remaining_budget_secs();
            let rewrite_result = match tokio::time::timeout(
                std::time::Duration::from_secs(rewrite_budget),
                Box::pin(self.service.execute_task(rewrite_task)),
            )
            .await
            {
                Ok(r) => r?,
                Err(_) => {
                    log::warn!(
                        "[Orchestrator] Rewrite timed out after {}s (budget exhausted), keeping current content",
                        rewrite_budget
                    );
                    // Rewrite 超时：保留当前内容，跳过后续循环
                    break;
                }
            };
            trace.log_phase(
                "rewrite",
                rewrite_start.elapsed().as_millis(),
                Some(&format!("loop {}", loop_idx)),
            );
            current_content = rewrite_result.content.clone();

            self.emit_step_event(
                &task.id,
                WorkflowStepType::Rewrite,
                Some(loop_idx),
                rewrite_result.score,
                Some("改写优化完成"),
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

        let skill_start = std::time::Instant::now();
        let final_content = self
            .apply_writing_skills(&task.context, &current_content)
            .await;
        trace.log_phase(
            "apply_writing_skills",
            skill_start.elapsed().as_millis(),
            None,
        );

        self.emit_generation_status(
            &task.id,
            GenerationPhase::FinalOutput,
            0.9,
            "整理最终输出...",
            request_id.clone(),
        );

        Ok(WorkflowResult {
            final_content,
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

    /// v0.9.6: 在 Writer 输出后自动调用内置增强技能（情感节奏 + 文风润色）
    async fn apply_writing_skills(
        &self,
        context: &crate::agents::AgentContext,
        content: &str,
    ) -> String {
        let Some(manager) = crate::SKILL_MANAGER.get() else {
            return content.to_string();
        };
        let skill_manager = {
            let Ok(guard) = manager.lock() else {
                return content.to_string();
            };
            guard.clone()
        };

        let mut result = content.to_string();

        // 1. 情感节奏优化
        let emotion_params = std::collections::HashMap::from([
            (
                "content".to_string(),
                serde_json::Value::String(result.clone()),
            ),
            (
                "mode".to_string(),
                serde_json::Value::String("rewrite".to_string()),
            ),
        ]);
        match skill_manager
            .execute_skill("builtin.emotion_pacing", context, emotion_params)
            .await
        {
            Ok(skill_result) => {
                if skill_result.success {
                    if let Some(serde_json::Value::String(rewritten)) =
                        skill_result.data.get("content")
                    {
                        if !rewritten.trim().is_empty() {
                            result = rewritten.clone();
                        }
                    } else if let Some(rewritten) = skill_result.data.as_str() {
                        if !rewritten.trim().is_empty() {
                            result = rewritten.to_string();
                        }
                    }
                }
            }
            Err(e) => log::warn!("[AgentOrchestrator] emotion_pacing skill failed: {}", e),
        }

        // 2. 文风增强
        let style_params = std::collections::HashMap::from([(
            "content".to_string(),
            serde_json::Value::String(result.clone()),
        )]);
        match skill_manager
            .execute_skill("builtin.style_enhancer", context, style_params)
            .await
        {
            Ok(skill_result) => {
                if skill_result.success {
                    if let Some(serde_json::Value::String(rewritten)) =
                        skill_result.data.get("content")
                    {
                        if !rewritten.trim().is_empty() {
                            result = rewritten.clone();
                        }
                    } else if let Some(rewritten) = skill_result.data.as_str() {
                        if !rewritten.trim().is_empty() {
                            result = rewritten.to_string();
                        }
                    }
                }
            }
            Err(e) => log::warn!("[AgentOrchestrator] style_enhancer skill failed: {}", e),
        }

        // P1-2: 接通 3 个此前休眠的内置技能（审计报告发现 4.1.1）。
        // 按场景智能激活，避免对每段文本无差别调用（影响速度/成本）。

        // 3. 角色语音校准——当内容含密集对话时触发
        if Self::should_apply_character_voice(&result) {
            let voice_params = std::collections::HashMap::from([
                (
                    "content".to_string(),
                    serde_json::Value::String(result.clone()),
                ),
                (
                    "story_id".to_string(),
                    serde_json::Value::String(context.story.story_id.clone()),
                ),
            ]);
            match skill_manager
                .execute_skill("builtin.character_voice", context, voice_params)
                .await
            {
                Ok(skill_result) => {
                    if skill_result.success {
                        if let Some(rewritten) =
                            skill_result.data.get("content").and_then(|v| v.as_str())
                        {
                            if !rewritten.trim().is_empty() {
                                result = rewritten.to_string();
                            }
                        }
                    }
                }
                Err(e) => log::warn!("[AgentOrchestrator] character_voice skill failed: {}", e),
            }
        }

        // 4. 情节反转增强——当叙事阶段处于转折点时触发
        if Self::should_apply_plot_twist(context) {
            let twist_params = std::collections::HashMap::from([(
                "content".to_string(),
                serde_json::Value::String(result.clone()),
            )]);
            match skill_manager
                .execute_skill("builtin.plot_twist", context, twist_params)
                .await
            {
                Ok(skill_result) => {
                    if skill_result.success {
                        if let Some(rewritten) =
                            skill_result.data.get("content").and_then(|v| v.as_str())
                        {
                            if !rewritten.trim().is_empty() {
                                result = rewritten.to_string();
                            }
                        }
                    }
                }
                Err(e) => log::warn!("[AgentOrchestrator] plot_twist skill failed: {}", e),
            }
        }

        // 5. 文本排版——当内容含明显排版问题时触发
        if Self::should_apply_text_formatter(&result) {
            let fmt_params = std::collections::HashMap::from([(
                "content".to_string(),
                serde_json::Value::String(result.clone()),
            )]);
            match skill_manager
                .execute_skill("builtin.text_formatter", context, fmt_params)
                .await
            {
                Ok(skill_result) => {
                    if skill_result.success {
                        if let Some(rewritten) =
                            skill_result.data.get("content").and_then(|v| v.as_str())
                        {
                            if !rewritten.trim().is_empty() {
                                result = rewritten.to_string();
                            }
                        }
                    }
                }
                Err(e) => log::warn!("[AgentOrchestrator] text_formatter skill failed: {}", e),
            }
        }

        result
    }

    /// P1-2: 判断是否应调用 character_voice 技能——内容含密集对话时触发。
    /// 启发式：引号对数 >= 3 视为对话密集。
    fn should_apply_character_voice(content: &str) -> bool {
        let quote_pairs = content
            .chars()
            .filter(|&c| c == '"' || c == '\u{201C}' || c == '\u{201D}')
            .count();
        quote_pairs >= 6 // 每对 2 个引号，>=3 对
    }

    /// P1-2: 判断是否应调用 plot_twist 技能——叙事阶段处于转折点时触发。
    /// 当规范状态快照的叙事阶段为 Climax 或 Falling（高潮/回落）时，
    /// 或当前场景标记为关键转折时，激活反转增强。
    fn should_apply_plot_twist(context: &crate::agents::AgentContext) -> bool {
        // 检查 narrative_structure 中的 dramatic_function 是否标记转折
        if let Some(ref structure) = context.narrative.narrative_structure {
            let func = &structure.dramatic_function;
            let f = func.to_lowercase();
            if f.contains("转折") || f.contains("高潮") || f.contains("反转") {
                return true;
            }
        }
        false
    }

    /// P1-2: 判断是否应调用 text_formatter 技能——内容含明显排版问题时触发。
    /// 启发式：连续空行 >= 2 处，或单行过长（>500 字无换行）。
    fn should_apply_text_formatter(content: &str) -> bool {
        // 连续空行检测
        let mut blank_streak = 0u32;
        let mut excessive_blanks = false;
        for line in content.lines() {
            if line.trim().is_empty() {
                blank_streak += 1;
                if blank_streak >= 2 {
                    excessive_blanks = true;
                    break;
                }
            } else {
                blank_streak = 0;
            }
        }
        if excessive_blanks {
            return true;
        }
        // 单行过长检测（无换行的超长段落）
        content.split('\n').any(|l| l.chars().count() > 500)
    }

    /// 从 Inspector JSON 响应中解析风格分析（v0.7.8）和记忆分析（v0.8.0）
    fn parse_inspector_style_analysis(
        content: &str,
        fallback_score: f32,
    ) -> (f32, f32, Vec<String>) {
        // 尝试从 content 中提取 JSON
        let json_str = Self::extract_json_from_content(content);
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&json_str) {
            // v0.9.6: 从 dimension_scores 提取各维度分数
            let dimension_scores = json.get("dimension_scores");
            let ds = |key: &str| {
                dimension_scores
                    .and_then(|d| d.get(key))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32
            };
            let writing_score = ds("writing");
            let scene_score = ds("scene");

            let style_score = json
                .get("style_analysis")
                .and_then(|s| s.get("style_score"))
                .and_then(|s| s.as_f64())
                .map(|s| (s as f32 / 100.0).min(1.0))
                .unwrap_or_else(|| {
                    // 没有风格分析时，用文笔与场景丰富度作为风格分替代
                    if writing_score > 0.0 || scene_score > 0.0 {
                        ((writing_score + scene_score) / 40.0).min(1.0)
                    } else {
                        fallback_score
                    }
                });

            // v0.9.6: 叙事分基于升级后的七维评分（logic/character/writing/scene/plot/
            // pacing/world）
            let narrative_score = dimension_scores
                .and_then(|d| {
                    let logic = d.get("logic").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let character = d.get("character").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let writing = d.get("writing").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let scene = d.get("scene").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let plot = d.get("plot").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let pacing = d.get("pacing").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let world = d.get("world").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let total = logic + character + writing + scene + plot + pacing + world;
                    Some((total as f32 / 140.0).min(1.0)) // 7维度总分140，归一化到0-1
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
    /// v0.9.3: 默认候选数从 3 降到 2，平衡质量与本地模型响应时间
    /// v0.11.1: 候选阶段共享预准备上下文，使用专用短超时与失败降级。
    ///
    /// 每个候选使用不同的 temperature 产生多样性：
    /// - 候选1: 0.82 (更保守，接近训练分布)
    /// - 候选2: 1.0  (更发散，探索性)
    async fn generate_candidates(
        &self,
        task: &AgentTask,
        count: usize,
        trace: &GenerationTrace,
    ) -> Result<(AgentResult, String, String), AppError> {
        // v0.11.8: 候选阶段超时与并发重构
        // - 本地模型固定 1 候选；远端模型默认 1 候选，配置明确指定时才允许 2。
        // - 总超时硬上限 90s（取代原来的 180s/270s），避免用户长期无响应。
        const MAX_LOCAL_CANDIDATE_TIMEOUT: u64 = 60;
        const MAX_REMOTE_CANDIDATE_TIMEOUT: u64 = 120;
        const MAX_TOTAL_CANDIDATE_TIMEOUT: u64 = 90;

        let is_local = self.service.is_target_model_local(AgentType::Writer);
        let effective_count = if is_local {
            1usize
        } else {
            count.min(self.config.candidate_count.max(1).min(2) as usize)
        };
        let per_candidate_timeout = if is_local {
            self.config
                .candidate_timeout_local_seconds
                .min(MAX_LOCAL_CANDIDATE_TIMEOUT)
        } else {
            self.config
                .candidate_timeout_seconds
                .min(MAX_REMOTE_CANDIDATE_TIMEOUT)
        };
        let total_timeout_seconds = per_candidate_timeout
            .saturating_mul(effective_count as u64)
            .saturating_add(30)
            .min(MAX_TOTAL_CANDIDATE_TIMEOUT);

        log::info!(
            "[Orchestrator] Candidate strategy: local={}, effective_count={}, \
             per_candidate_timeout={}s, total_timeout={}s, task={}",
            is_local,
            effective_count,
            per_candidate_timeout,
            total_timeout_seconds,
            task.id
        );

        let candidate_start = std::time::Instant::now();
        let result = match timeout(
            Duration::from_secs(total_timeout_seconds),
            self.generate_candidates_inner(task, effective_count, trace),
        )
        .await
        {
            Ok(r) => r,
            Err(_) => {
                log::warn!(
                    "[Orchestrator] Candidate generation timed out after {}s for task {}",
                    total_timeout_seconds,
                    task.id
                );
                self.emit_step_event(
                    &task.id,
                    WorkflowStepType::Generation,
                    None,
                    None,
                    Some(&format!(
                        "候选生成阶段整体超时（{}s），请检查模型服务是否正常",
                        total_timeout_seconds
                    )),
                );
                Err(AppError::llm_timeout(total_timeout_seconds * 1000))
            }
        };
        trace.log_phase(
            "candidates",
            candidate_start.elapsed().as_millis(),
            Some(&format!("count={}", effective_count)),
        );
        result
    }

    /// - 候选1: 0.82 (更保守，接近训练分布)
    /// - 候选2: 1.0  (更发散，探索性)
    async fn generate_candidates_inner(
        &self,
        task: &AgentTask,
        count: usize,
        trace: &GenerationTrace,
    ) -> Result<(AgentResult, String, String), AppError> {
        let temps = [0.82_f32, 1.0_f32];

        log::info!(
            "[Orchestrator] Preparing shared writer context for {} candidates, task {}",
            count,
            task.id
        );
        self.emit_step_event(
            &task.id,
            WorkflowStepType::Generation,
            None,
            None,
            Some("正在准备候选生成上下文..."),
        );
        self.emit_generation_status(
            &task.id,
            GenerationPhase::PreparingContext,
            0.1,
            "准备候选生成上下文...",
            None,
        );

        // 1. 预准备共享上下文：预检、补齐、prompt 构建、策略计算只做一次
        let prepare_start = std::time::Instant::now();
        let prepared = self.service.prepare_writer_context(task).await?;
        trace.log_phase(
            "prepare_writer_context",
            prepare_start.elapsed().as_millis(),
            Some("candidate shared context"),
        );
        self.emit_step_event(
            &task.id,
            WorkflowStepType::Generation,
            None,
            None,
            Some("候选上下文准备完成，开始生成..."),
        );

        // 2. 候选阶段强制并行：无论用户旧配置是否保存了
        //    candidate_local_sequential=true，
        // 都不再走串行分支，避免候选 1 挂起时阻塞候选 2。
        // 同时给单个候选超时加硬上限，防止旧配置里的 600s 等值让失败候选挂死 500s+。
        const MAX_LOCAL_CANDIDATE_TIMEOUT: u64 = 60;
        const MAX_REMOTE_CANDIDATE_TIMEOUT: u64 = 120;
        let is_local = self.service.is_target_model_local(AgentType::Writer);
        let per_candidate_timeout = if is_local {
            self.config
                .candidate_timeout_local_seconds
                .min(MAX_LOCAL_CANDIDATE_TIMEOUT)
        } else {
            self.config
                .candidate_timeout_seconds
                .min(MAX_REMOTE_CANDIDATE_TIMEOUT)
        };
        let timeout_override = Some(per_candidate_timeout);
        // 候选阶段不重试：多候选本身就是冗余，重试只会让失败模型反复阻塞。
        let retries_override = Some(0u32);

        log::info!(
            "[Orchestrator] Candidate strategy: local={}, parallel=true, per_candidate_timeout={}s, retries=0",
            is_local,
            per_candidate_timeout
        );

        // 3. 构建候选任务列表，仅 temperature 不同
        let mut candidate_tasks = Vec::with_capacity(count);
        for i in 0..count {
            let mut candidate_task = task.clone();
            candidate_task.id = format!("{}-candidate-{}", task.id, i);
            candidate_task.parameters.insert(
                "temperature_override".to_string(),
                serde_json::json!(temps.get(i).copied().unwrap_or(0.9)),
            );
            candidate_tasks.push(candidate_task);
        }

        self.emit_step_event(
            &task.id,
            WorkflowStepType::Generation,
            None,
            None,
            Some(&format!("生成候选中（共 {} 个）", count)),
        );
        self.emit_generation_status(
            &task.id,
            GenerationPhase::GeneratingCandidates,
            0.25,
            format!("生成候选中（共 {} 个）...", count),
            None,
        );

        // 4. 执行候选：强制并行执行
        let results: Vec<Result<AgentResult, AppError>> =
            futures::future::join_all(candidate_tasks.into_iter().enumerate().map(
                |(i, candidate_task)| {
                    let this = self;
                    let prepared = prepared.clone();
                    async move {
                        this.emit_step_event(
                            &task.id,
                            WorkflowStepType::Generation,
                            None,
                            None,
                            Some(&format!("生成候选 {} / {}", i + 1, count)),
                        );
                        this.emit_generation_status(
                            &task.id,
                            GenerationPhase::GeneratingCandidates,
                            0.25 + 0.15 * ((i + 1) as f32 / count.max(1) as f32),
                            format!("生成候选 {} / {}...", i + 1, count),
                            None,
                        );
                        let result = this
                            .service
                            .execute_writer_prepared(
                                candidate_task,
                                prepared,
                                timeout_override,
                                retries_override,
                            )
                            .await;
                        if result.is_ok() {
                            this.emit_step_event(
                                &task.id,
                                WorkflowStepType::Generation,
                                None,
                                None,
                                Some(&format!("候选 {} / {} 生成完成", i + 1, count)),
                            );
                        } else {
                            this.emit_step_event(
                                &task.id,
                                WorkflowStepType::Generation,
                                None,
                                None,
                                Some(&format!(
                                    "候选 {} / {} 生成失败：{}",
                                    i + 1,
                                    count,
                                    result
                                        .as_ref()
                                        .err()
                                        .map(|e| e.to_string())
                                        .unwrap_or_default()
                                )),
                            );
                        }
                        result
                    }
                },
            ))
            .await;

        self.emit_step_event(
            &task.id,
            WorkflowStepType::Generation,
            None,
            None,
            Some("正在评估候选质量..."),
        );

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

        // 5. 失败降级：若所有候选均失败/超时，回退到单轮完整生成
        if candidates.is_empty() {
            log::warn!(
                "[Orchestrator] All candidates failed for task {}, falling back to single-pass writer",
                task.id
            );
            self.emit_step_event(
                &task.id,
                WorkflowStepType::Generation,
                None,
                None,
                Some("候选生成均失败，降级为单轮生成..."),
            );
            let result = self.service.execute_writer_raw(task.clone()).await?;
            let req_id = result.request_id.clone().unwrap_or_default();
            let content = result.content.clone();
            return Ok((result, req_id, content));
        }

        let best = candidates.swap_remove(best_idx);
        log::info!(
            "[Orchestrator] Selected candidate {} with score {:.2} (from {} valid)",
            best_idx,
            best_score,
            candidates.len() + 1
        );

        self.emit_step_event(
            &task.id,
            WorkflowStepType::Generation,
            None,
            Some(best_score),
            Some(&format!(
                "候选评估完成，选用最优结果（匹配度 {:.0}%）",
                best_score * 100.0
            )),
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
        assert_eq!(config.candidate_timeout_seconds, 120);
        assert_eq!(config.candidate_timeout_local_seconds, 60);
        assert_eq!(config.candidate_max_retries, 0);
        assert!(!config.candidate_local_sequential);
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

    // ==================== 阶段一候选超时/并发策略 ====================

    #[test]
    fn test_workflow_config_clamps_candidate_count() {
        let mut app_config = crate::config::AppConfig::default();
        app_config.candidate_count = 5;
        let config = WorkflowConfig::from_app_config(&app_config);
        assert_eq!(config.candidate_count, 2, "远端候选数应被限制在 2 以内");
    }

    #[test]
    fn test_candidate_total_timeout_never_exceeds_90s() {
        // 复现 generate_candidates 中的超时计算逻辑，确保默认配置下总超时 ≤ 90s。
        // 本地模型固定 1 候选、远端默认 1 候选；per-candidate 超时取硬上限。
        let config = WorkflowConfig::default();

        let local_per = config.candidate_timeout_local_seconds.min(60);
        let local_total = local_per.saturating_mul(1).saturating_add(30).min(90);
        assert_eq!(local_per, 60);
        assert_eq!(local_total, 90);

        let remote_per = config.candidate_timeout_seconds.min(120);
        let remote_total = remote_per.saturating_mul(1).saturating_add(30).min(90);
        assert_eq!(remote_per, 120);
        assert_eq!(remote_total, 90);

        // 远端 2 候选配置（用户显式设置）时，总超时仍被硬上限 90s 截断
        let remote_total_2 = remote_per.saturating_mul(2).saturating_add(30).min(90);
        assert_eq!(remote_total_2, 90);
    }
}
