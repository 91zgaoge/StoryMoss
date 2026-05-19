//! 创作工作流引擎核心
//!
//! 串联所有创作阶段和 Agent，形成完整闭环：
//! Conception → Outlining → SceneDesign → Writing → Review → Iteration → Ingestion

use crate::agents::service::{AgentService, AgentTask, AgentType};
use crate::agents::{AgentContext, AgentResult};
use crate::db::DbPool;
use crate::db::repositories::ChapterRepository;
use crate::db::CreateChapterRequest;
use crate::db::repositories_v3::KnowledgeGraphRepository;
use crate::creative_engine::methodology::MethodologyConfig;
use super::{WorkflowExecutionResult, WorkflowProgressEvent, WorkflowStage, CreationMode};
use super::quality::QualityChecker;
use crate::llm::service::LlmService;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::Emitter;

/// 创作阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreationPhase {
    Conception,    // 构思：用户灵感 → OutlinePlanner → 故事种子
    Outlining,     // 大纲：故事种子 → 方法论 → 完整大纲
    SceneDesign,   // 场景设计：大纲章节 → 场景结构
    Writing,       // 写作：场景结构 + 记忆查询 → Writer → 初稿
    Review,        // 审校：初稿 → Inspector + ContinuityEngine → 问题列表
    Iteration,     // 迭代：问题列表 → Writer(改写) → 终稿
    Ingestion,     // 记忆：终稿 → IngestPipeline → 知识图谱更新
}

impl CreationPhase {
    pub fn name(&self) -> &'static str {
        match self {
            CreationPhase::Conception => "构思",
            CreationPhase::Outlining => "大纲",
            CreationPhase::SceneDesign => "场景设计",
            CreationPhase::Writing => "写作",
            CreationPhase::Review => "审校",
            CreationPhase::Iteration => "迭代",
            CreationPhase::Ingestion => "记忆",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            CreationPhase::Conception => "将用户灵感转化为结构化故事种子",
            CreationPhase::Outlining => "按方法论生成完整故事大纲",
            CreationPhase::SceneDesign => "为每章设计场景结构和戏剧目标",
            CreationPhase::Writing => "根据场景结构生成完整章节",
            CreationPhase::Review => "质检和内容一致性检查",
            CreationPhase::Iteration => "根据质检反馈改写优化",
            CreationPhase::Ingestion => "分析新内容并更新知识图谱",
        }
    }

    pub fn order(&self) -> u8 {
        match self {
            CreationPhase::Conception => 0,
            CreationPhase::Outlining => 1,
            CreationPhase::SceneDesign => 2,
            CreationPhase::Writing => 3,
            CreationPhase::Review => 4,
            CreationPhase::Iteration => 5,
            CreationPhase::Ingestion => 6,
        }
    }

    pub fn next(&self) -> Option<CreationPhase> {
        match self {
            CreationPhase::Conception => Some(CreationPhase::Outlining),
            CreationPhase::Outlining => Some(CreationPhase::SceneDesign),
            CreationPhase::SceneDesign => Some(CreationPhase::Writing),
            CreationPhase::Writing => Some(CreationPhase::Review),
            CreationPhase::Review => Some(CreationPhase::Iteration),
            CreationPhase::Iteration => Some(CreationPhase::Ingestion),
            CreationPhase::Ingestion => None,
        }
    }

    pub fn id_str(&self) -> &'static str {
        match self {
            CreationPhase::Conception => "conception",
            CreationPhase::Outlining => "outlining",
            CreationPhase::SceneDesign => "scene-design",
            CreationPhase::Writing => "writing",
            CreationPhase::Review => "review",
            CreationPhase::Iteration => "iteration",
            CreationPhase::Ingestion => "ingestion",
        }
    }
}

/// 阶段工作流配置
#[derive(Debug, Clone)]
pub struct PhaseWorkflow {
    pub phase: CreationPhase,
    /// 该阶段需要执行的 Agent 列表（按顺序）
    pub required_agents: Vec<AgentType>,
    /// 是否需要用户确认后才能进入下一阶段
    pub requires_user_confirmation: bool,
    /// 该阶段使用的方法论（如有）
    pub methodology: Option<MethodologyConfig>,
    /// 方法论 ID（注入 AgentContext）
    pub methodology_id: Option<String>,
    /// 方法论当前步骤（注入 AgentContext）
    pub methodology_step: Option<String>,
    /// 阶段特定的提示词补充
    pub prompt_extension: Option<String>,
}

impl PhaseWorkflow {
    pub fn new(phase: CreationPhase) -> Self {
        Self {
            phase,
            required_agents: vec![],
            requires_user_confirmation: false,
            methodology: None,
            methodology_id: None,
            methodology_step: None,
            prompt_extension: None,
        }
    }

    /// 设置该阶段使用的 Agent
    pub fn with_agents(mut self, agents: Vec<AgentType>) -> Self {
        self.required_agents = agents;
        self
    }

    /// 设置需要用户确认
    pub fn with_user_confirmation(mut self) -> Self {
        self.requires_user_confirmation = true;
        self
    }

    /// 设置方法论
    pub fn with_methodology(mut self, config: MethodologyConfig) -> Self {
        self.methodology = Some(config);
        self
    }

    /// 设置方法论 ID（简化接口，直接注入 AgentContext）
    pub fn with_methodology_id(mut self, id: &str) -> Self {
        self.methodology_id = Some(id.to_string());
        self
    }

    /// 设置方法论步骤
    pub fn with_methodology_step(mut self, step: &str) -> Self {
        self.methodology_step = Some(step.to_string());
        self
    }

    /// 设置提示词补充
    pub fn with_prompt_extension(mut self, ext: &str) -> Self {
        self.prompt_extension = Some(ext.to_string());
        self
    }
}

/// 工作流配置
#[derive(Debug, Clone)]
pub struct WorkflowConfig {
    pub mode: CreationMode,
    /// 是否自动执行（无需用户确认每个阶段）
    pub auto_execute: bool,
    /// 审校阈值（低于此分数进入迭代）
    pub review_threshold: f32,
    /// 最大迭代次数
    pub max_iterations: u32,
    /// 故事 ID
    pub story_id: String,
}

/// 工作流状态
#[derive(Debug, Clone)]
pub struct WorkflowState {
    pub workflow_id: String,
    pub current_phase: CreationPhase,
    pub completed_phases: Vec<CreationPhase>,
    /// 各阶段输出缓存
    pub phase_outputs: HashMap<String, String>,
    /// 质检评分
    pub review_score: Option<f32>,
    /// 迭代计数
    pub iteration_count: u32,
    /// 是否已暂停
    pub is_paused: bool,
}

impl WorkflowState {
    pub fn new(workflow_id: String) -> Self {
        Self {
            workflow_id,
            current_phase: CreationPhase::Conception,
            completed_phases: vec![],
            phase_outputs: HashMap::new(),
            review_score: None,
            iteration_count: 0,
            is_paused: false,
        }
    }

    pub fn progress(&self) -> f32 {
        let total = 7.0;
        let current = self.current_phase.order() as f32;
        let completed_bonus = self.completed_phases.len() as f32 * 0.1;
        ((current + completed_bonus) / total).min(1.0)
    }
}

fn phase_progress(phase: CreationPhase) -> f32 {
    match phase {
        CreationPhase::Conception => 0.0,
        CreationPhase::Outlining => 0.15,
        CreationPhase::SceneDesign => 0.30,
        CreationPhase::Writing => 0.50,
        CreationPhase::Review => 0.70,
        CreationPhase::Iteration => 0.85,
        CreationPhase::Ingestion => 1.0,
    }
}

/// 获取标准阶段工作流配置
///
/// 将各阶段的 AgentType、methodology、prompt_extension 从硬编码迁移到配置。
fn standard_phase_workflow(phase: CreationPhase, ctx: &AgentContext) -> PhaseWorkflow {
    let mut pw = match phase {
        CreationPhase::Conception => PhaseWorkflow::new(phase)
            .with_agents(vec![AgentType::OutlinePlanner]),
        CreationPhase::Outlining => PhaseWorkflow::new(phase)
            .with_agents(vec![AgentType::OutlinePlanner])
            .with_prompt_extension("请根据以下大纲设计场景结构："),
        CreationPhase::SceneDesign => PhaseWorkflow::new(phase)
            .with_agents(vec![AgentType::Writer])
            .with_prompt_extension("请根据以下大纲设计场景结构："),
        CreationPhase::Writing => PhaseWorkflow::new(phase)
            .with_agents(vec![AgentType::Writer]),
        CreationPhase::Review => PhaseWorkflow::new(phase)
            .with_agents(vec![AgentType::Inspector]),
        CreationPhase::Iteration => PhaseWorkflow::new(phase)
            .with_agents(vec![AgentType::Writer]),
        CreationPhase::Ingestion => PhaseWorkflow::new(phase)
            .with_agents(vec![]),
    };
    // 如果故事配置了创作方法论，覆盖默认硬编码
    if let Some(ref method_id) = ctx.methodology_id {
        if !method_id.is_empty() {
            pw = pw.with_methodology_id(method_id);
            if let Some(ref step) = ctx.methodology_step {
                pw = pw.with_methodology_step(step);
            }
        }
    }
    pw
}

/// 创作工作流引擎
pub struct CreationWorkflowEngine {
    agent_service: AgentService,
    pool: DbPool,
}

impl CreationWorkflowEngine {
    pub fn new(agent_service: AgentService, pool: DbPool) -> Self {
        Self { agent_service, pool }
    }

    /// 创建标准工作流配置
    pub fn create_standard_workflow(story_id: &str, mode: CreationMode) -> WorkflowConfig {
        WorkflowConfig {
            mode,
            auto_execute: mode == CreationMode::AiOnly,
            review_threshold: 0.75,
            max_iterations: 2,
            story_id: story_id.to_string(),
        }
    }

    /// 构建 AgentContext（使用 StoryContextBuilder）
    pub fn build_context(&self, story_id: &str) -> Result<AgentContext, String> {
        use crate::creative_engine::StoryContextBuilder;
        let builder = StoryContextBuilder::new(self.pool.clone());
        builder.build_quick(story_id)
    }

    /// 执行单阶段
    ///
    /// 根据 `PhaseWorkflow` 配置动态执行各阶段（P2-3 配置化）。
    pub async fn execute_phase(
        &self,
        phase: CreationPhase,
        context: &AgentContext,
        input: &str,
    ) -> Result<AgentResult, String> {
        let config = standard_phase_workflow(phase, context);
        let agent_type = config.required_agents.first().copied();

        match phase {
            CreationPhase::Ingestion => {
                let story_id = context.story_id.clone();
                let content = input.to_string();

                // 1. 保存内容到数据库（Chapter）
                let chapter_repo = ChapterRepository::new(self.pool.clone());
                let mut saved_info = String::new();

                match chapter_repo.get_by_story(&story_id) {
                    Ok(chapters) => {
                        if let Some(chapter) = chapters.into_iter().last() {
                            // 更新最后一个 chapter
                            match chapter_repo.update(&chapter.id, None, None, Some(content.clone()), None) {
                                Ok(_) => {
                                    saved_info.push_str(&format!(
                                        "已更新章节「{}」",
                                        chapter.title.unwrap_or_else(|| format!("第{}章", chapter.chapter_number))
                                    ));
                                }
                                Err(e) => {
                                    log::warn!("[Ingestion] 更新章节失败: {}", e);
                                }
                            }
                        } else {
                            // 创建新 chapter
                            let req = CreateChapterRequest {
                                story_id: story_id.clone(),
                                chapter_number: context.chapter_number as i32,
                                title: Some(format!("第{}章", context.chapter_number)),
                                outline: None,
                                content: Some(content.clone()),
                            };
                            match chapter_repo.create(req) {
                                Ok(chapter) => {
                                    saved_info.push_str(&format!(
                                        "已创建章节「{}」",
                                        chapter.title.unwrap_or_else(|| format!("第{}章", chapter.chapter_number))
                                    ));
                                }
                                Err(e) => {
                                    log::warn!("[Ingestion] 创建章节失败: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("[Ingestion] 获取章节列表失败: {}", e);
                    }
                }

                // 2. 简化版知识图谱更新：内容分块 + 提取实体存入 kg_entities
                let kg_repo = KnowledgeGraphRepository::new(self.pool.clone());
                let entities = Self::extract_simple_entities(&content);
                let mut entity_count = 0;
                for (name, entity_type) in entities {
                    let attrs = serde_json::json!({
                        "source": "ingestion",
                        "description": format!("从创作内容中提取的{}", entity_type),
                        "auto_extracted": true
                    });
                    if let Ok(_) = kg_repo.create_entity(&story_id, &name, &entity_type, &attrs, None) {
                        entity_count += 1;
                    }
                }

                if saved_info.is_empty() {
                    saved_info.push_str("内容已保存");
                }

                Ok(AgentResult {
                    content: format!("{}。知识图谱更新：提取 {} 个实体。", saved_info, entity_count),
                    score: Some(1.0),
                    suggestions: vec![],
                    request_id: None,
                })
            }
            _ => {
                // 通用阶段：使用 PhaseWorkflow 配置动态构建 AgentTask
                let agent_type = agent_type.ok_or_else(||
                    format!("阶段 {:?} 未配置 Agent", phase)
                )?;

                let task_input = if let Some(ref ext) = config.prompt_extension {
                    format!("{}\n\n{}", ext, input)
                } else {
                    input.to_string()
                };

                let mut task = AgentTask {
                    id: format!("{}-{}", phase.id_str(), context.story_id),
                    agent_type,
                    context: context.clone(),
                    input: task_input,
                    parameters: HashMap::new(),
                    tier: None,
                };

                // 注入方法论配置
                if config.methodology_id.is_some() {
                    task.context.methodology_id = config.methodology_id;
                }
                if config.methodology_step.is_some() {
                    task.context.methodology_step = config.methodology_step;
                }

                self.agent_service.execute_task(task).await
            }
        }
    }

    /// 执行完整工作流（一键创作）
    ///
    /// 根据 `PhaseWorkflow` 配置动态执行各阶段（P2-3），
    /// 并在每阶段完成后将关键产出回注 `AgentContext`（P2-4），
    /// 质检优先使用 LLM 评估（P2-2）。
    pub async fn execute_full_workflow(
        &self,
        config: &WorkflowConfig,
        initial_input: &str,
    ) -> Result<WorkflowExecutionResult, String> {
        let mut state = WorkflowState::new(format!("wf-{}", config.story_id));
        let mut current_input = initial_input.to_string();
        let mut context = self.build_context(&config.story_id)?;

        self.emit_progress(&state, WorkflowStage::Started, &format!("开始{}模式创作", config.mode.name()), 0.0);

        match config.mode {
            CreationMode::AiOnly => {
                // 全自动模式：执行所有阶段
                self.run_all_phases(config, &mut state, &mut context, &mut current_input).await?;
            }
            CreationMode::AiDraftHumanEdit => {
                // AI 初稿 + 人精修：执行到 Writing 后暂停
                let phase_workflows = vec![
                    standard_phase_workflow(CreationPhase::Conception, &context),
                    standard_phase_workflow(CreationPhase::Outlining, &context),
                    standard_phase_workflow(CreationPhase::SceneDesign, &context),
                    standard_phase_workflow(CreationPhase::Writing, &context),
                ];

                for pw in phase_workflows {
                    let phase = pw.phase;
                    if state.is_paused {
                        break;
                    }
                    state.current_phase = phase;
                    self.emit_progress(&state, WorkflowStage::InProgress, &format!("进入{}阶段", phase.name()), phase_progress(phase));
                    let result = self.execute_phase(phase, &context, &current_input).await?;
                    state.phase_outputs.insert(phase.name().to_string(), result.content.clone());

                    // 回注上下文（P2-4）
                    Self::update_context_after_phase(&mut context, phase, &result.content);

                    current_input = result.content;
                    state.completed_phases.push(phase);
                    let next_phase = phase.next().unwrap_or(CreationPhase::Ingestion);
                    self.emit_progress(&state, WorkflowStage::Completed, &format!("{}阶段完成", phase.name()), phase_progress(next_phase));
                }

                // Writing 完成后暂停，等待用户确认
                if !state.is_paused {
                    state.is_paused = true;
                    self.emit_progress(&state, WorkflowStage::WaitingForUser, "AI 初稿已完成，请在幕前编辑后继续", phase_progress(CreationPhase::Writing));
                }
            }
            CreationMode::HumanDraftAiPolish => {
                // 人初稿 + AI 润色：跳过前三个阶段，从 Review 开始
                state.completed_phases.push(CreationPhase::Conception);
                state.completed_phases.push(CreationPhase::Outlining);
                state.completed_phases.push(CreationPhase::SceneDesign);

                // initial_input 就是用户的草稿，直接进入 Review
                state.current_phase = CreationPhase::Review;
                self.emit_progress(&state, WorkflowStage::InProgress, "进入审校阶段", phase_progress(CreationPhase::Review));
                let review_result = self.execute_phase(CreationPhase::Review, &context, &current_input).await?;
                state.phase_outputs.insert("审校".to_string(), review_result.content.clone());
                state.review_score = review_result.score;
                state.completed_phases.push(CreationPhase::Review);
                self.emit_progress(&state, WorkflowStage::Completed, "审校阶段完成", phase_progress(CreationPhase::Iteration));

                // 根据审校结果决定是否迭代
                let score = review_result.score.unwrap_or(0.0);
                if score < config.review_threshold && state.iteration_count < config.max_iterations {
                    let feedback = if review_result.suggestions.is_empty() {
                        "请改进内容质量".to_string()
                    } else {
                        review_result.suggestions.join("\n")
                    };
                    let iteration_input = format!("【质检反馈】\n{}\n\n【原文】\n{}", feedback, current_input);
                    state.iteration_count += 1;
                    state.current_phase = CreationPhase::Iteration;
                    self.emit_progress(&state, WorkflowStage::InProgress, "进入迭代润色阶段", phase_progress(CreationPhase::Iteration));
                    let iteration_result = self.execute_phase(CreationPhase::Iteration, &context, &iteration_input).await?;
                    state.phase_outputs.insert("迭代".to_string(), iteration_result.content.clone());
                    current_input = iteration_result.content;
                    self.emit_progress(&state, WorkflowStage::Completed, "迭代润色阶段完成", phase_progress(CreationPhase::Ingestion));
                }

                // 最终 Ingestion
                state.current_phase = CreationPhase::Ingestion;
                self.emit_progress(&state, WorkflowStage::InProgress, "进入记忆阶段", phase_progress(CreationPhase::Ingestion));
                let _ = self.execute_phase(CreationPhase::Ingestion, &context, &current_input).await;
                state.completed_phases.push(CreationPhase::Ingestion);
                self.emit_progress(&state, WorkflowStage::Completed, "润色创作完成", 1.0);
            }
        }

        // 构建结果
        let final_output = state.phase_outputs.get("写作")
            .or(state.phase_outputs.get("迭代"))
            .or(state.phase_outputs.get("审校"))
            .cloned();

        // 生成质量报告：优先 LLM 评估，回退规则评估（P2-2）
        let quality_report = if let Some(ref content) = final_output {
            let llm_service = LlmService::new(self.agent_service.app_handle().clone());
            let checker = QualityChecker::new();
            match checker.check_with_llm(content, &llm_service).await {
                Ok(report) => Some(report),
                Err(e) => {
                    log::warn!("[Workflow] LLM 质量评估失败: {}，回退到规则评估", e);
                    Some(checker.check(content))
                }
            }
        } else {
            None
        };

        let success = match config.mode {
            CreationMode::AiOnly => !state.is_paused,
            CreationMode::AiDraftHumanEdit => state.completed_phases.contains(&CreationPhase::Writing),
            CreationMode::HumanDraftAiPolish => state.completed_phases.contains(&CreationPhase::Ingestion),
        };

        Ok(WorkflowExecutionResult {
            success,
            current_phase: state.current_phase.name().to_string(),
            completed_phases: state.completed_phases.iter().map(|p| p.name().to_string()).collect(),
            output: final_output,
            quality_report,
            error: None,
        })
    }

    /// 执行所有创作阶段（全自动模式）
    ///
    /// 根据 `PhaseWorkflow` 配置动态执行（P2-3），并回注上下文（P2-4）。
    async fn run_all_phases(
        &self,
        config: &WorkflowConfig,
        state: &mut WorkflowState,
        context: &mut AgentContext,
        current_input: &mut String,
    ) -> Result<(), String> {
        let phase_workflows = vec![
            standard_phase_workflow(CreationPhase::Conception, context),
            standard_phase_workflow(CreationPhase::Outlining, context),
            standard_phase_workflow(CreationPhase::SceneDesign, context),
            standard_phase_workflow(CreationPhase::Writing, context),
            standard_phase_workflow(CreationPhase::Review, context),
        ];

        for pw in phase_workflows {
            let phase = pw.phase;
            if state.is_paused {
                break;
            }

            state.current_phase = phase;
            self.emit_progress(&state, WorkflowStage::InProgress, &format!("进入{}阶段", phase.name()), phase_progress(phase));

            // 执行阶段
            let result = self.execute_phase(phase, context, current_input).await?;

            // 缓存输出
            state.phase_outputs.insert(phase.name().to_string(), result.content.clone());

            // 将关键产出回注 AgentContext（P2-4）
            Self::update_context_after_phase(context, phase, &result.content);

            // 处理阶段特定逻辑
            match phase {
                CreationPhase::Review => {
                    state.review_score = result.score;
                    let score = result.score.unwrap_or(0.0);

                    if score < config.review_threshold && state.iteration_count < config.max_iterations {
                        // 进入迭代阶段
                        let feedback = if result.suggestions.is_empty() {
                            "请改进内容质量".to_string()
                        } else {
                            result.suggestions.join("\n")
                        };
                        *current_input = format!("【质检反馈】\n{}\n\n【原文】\n{}",
                            feedback,
                            state.phase_outputs.get("写作").unwrap_or(&"".to_string())
                        );
                        state.iteration_count += 1;

                        state.current_phase = CreationPhase::Iteration;
                        self.emit_progress(&state, WorkflowStage::InProgress, "进入迭代阶段", phase_progress(CreationPhase::Iteration));

                        // 继续迭代
                        let iteration_result = self.execute_phase(CreationPhase::Iteration, context, current_input).await?;
                        state.phase_outputs.insert("迭代".to_string(), iteration_result.content.clone());
                        *current_input = iteration_result.content;

                        self.emit_progress(&state, WorkflowStage::Completed, "迭代阶段完成", phase_progress(CreationPhase::Ingestion));
                    } else {
                        *current_input = result.content;
                    }
                }
                CreationPhase::Writing => {
                    *current_input = result.content.clone();
                }
                _ => {
                    *current_input = result.content;
                }
            }

            state.completed_phases.push(phase);
            let next_phase = phase.next().unwrap_or(CreationPhase::Ingestion);
            self.emit_progress(&state, WorkflowStage::Completed, &format!("{}阶段完成", phase.name()), phase_progress(next_phase));
        }

        // 最终 Ingestion
        if !state.is_paused {
            let final_content = state.phase_outputs.get("写作")
                .or(state.phase_outputs.get("迭代"))
                .unwrap_or(current_input)
                .clone();

            state.current_phase = CreationPhase::Ingestion;
            self.emit_progress(&state, WorkflowStage::InProgress, "进入记忆阶段", phase_progress(CreationPhase::Ingestion));

            let _ = self.execute_phase(CreationPhase::Ingestion, context, &final_content).await;
            state.completed_phases.push(CreationPhase::Ingestion);
            self.emit_progress(&state, WorkflowStage::Completed, "一键创作完成", 1.0);
        }

        Ok(())
    }

    /// 阶段完成后将关键产出回注 AgentContext（P2-4）
    fn update_context_after_phase(context: &mut AgentContext, phase: CreationPhase, content: &str) {
        match phase {
            CreationPhase::Conception => {
                context.world_rules = Some(content.to_string());
            }
            CreationPhase::Outlining => {
                context.scene_structure = Some(content.to_string());
            }
            CreationPhase::SceneDesign => {
                let existing = context.world_rules.take().unwrap_or_default();
                context.world_rules = Some(
                    if existing.is_empty() {
                        format!("【场景结构】\n{}", content)
                    } else {
                        format!("{}\n\n【场景结构】\n{}", existing, content)
                    }
                );
            }
            CreationPhase::Writing => {
                context.current_content = Some(content.to_string());
            }
            _ => {}
        }
    }

    /// 执行单个创作阶段（分步模式）
    pub async fn execute_single_phase(
        &self,
        phase: CreationPhase,
        story_id: &str,
        input: &str,
    ) -> Result<AgentResult, String> {
        let mut context = self.build_context(story_id)?;
        let result = self.execute_phase(phase, &context, input).await?;
        Self::update_context_after_phase(&mut context, phase, &result.content);
        Ok(result)
    }

    /// 生成工作流进度事件
    pub fn emit_progress(&self, state: &WorkflowState, stage: WorkflowStage, message: &str, progress: f32) {
        let _ = self.agent_service.app_handle().emit(
            "workflow-progress",
            WorkflowProgressEvent {
                workflow_id: state.workflow_id.clone(),
                phase: state.current_phase.name().to_string(),
                stage,
                message: message.to_string(),
                progress,
            },
        );
    }

    /// 简化版实体提取：将内容分块，提取引号内文本作为潜在实体
    fn extract_simple_entities(text: &str) -> Vec<(String, String)> {
        let mut entities = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 1. 内容分块（每块约 200 字），每块作为记忆实体
        let chunk_size = 200;
        let chars: Vec<char> = text.chars().collect();
        for (i, chunk) in chars.chunks(chunk_size).enumerate() {
            let chunk_text: String = chunk.iter().collect();
            let name = format!("内容片段-{}", i + 1);
            if seen.insert(name.clone()) {
                entities.push((name, "ContentChunk".to_string()));
            }

            // 2. 从每块中提取引号内文本作为潜在实体
            let quote_pairs = [('「', '」'), ('『', '』')];
            for (open, close) in quote_pairs.iter() {
                let parts: Vec<&str> = chunk_text.split(*open).collect();
                for part in parts.iter().skip(1) {
                    if let Some(end) = part.find(*close) {
                        let quoted = &part[..end];
                        let len = quoted.chars().count();
                        if len >= 2 && len <= 20 && seen.insert(quoted.to_string()) {
                            entities.push((quoted.to_string(), "Concept".to_string()));
                        }
                    }
                }
            }
        }

        entities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation_phase_order() {
        assert_eq!(CreationPhase::Conception.order(), 0);
        assert_eq!(CreationPhase::Ingestion.order(), 6);
    }

    #[test]
    fn test_creation_phase_next() {
        assert_eq!(CreationPhase::Conception.next(), Some(CreationPhase::Outlining));
        assert_eq!(CreationPhase::Ingestion.next(), None);
    }

    #[test]
    fn test_workflow_state_progress() {
        let mut state = WorkflowState::new("test".to_string());
        assert_eq!(state.progress(), 0.0);

        state.current_phase = CreationPhase::Writing;
        state.completed_phases.push(CreationPhase::Conception);
        state.completed_phases.push(CreationPhase::Outlining);
        let p = state.progress();
        assert!(p > 0.0 && p < 1.0);
    }

    #[test]
    fn test_creation_mode() {
        assert_eq!(CreationMode::AiOnly.name(), "一键创作");
        assert_eq!(CreationMode::AiDraftHumanEdit.name(), "AI草稿+人修改");
    }

    #[test]
    fn test_phase_workflow_builder() {
        let wf = PhaseWorkflow::new(CreationPhase::Writing)
            .with_agents(vec![AgentType::Writer])
            .with_user_confirmation();

        assert_eq!(wf.phase, CreationPhase::Writing);
        assert_eq!(wf.required_agents.len(), 1);
        assert!(wf.requires_user_confirmation);
    }
}
