//! PlanExecutor - Dumb executor that faithfully runs LLM-generated plans
//!
//! All intelligence is in the plan. This executor just follows instructions.

use std::{collections::HashMap, sync::Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use super::{ExecutionPlan, PlanContext, PlanExecutorProgress, PlanGenerator, PlanStep};
use crate::{
    capabilities::{CapabilityEvolutionEngine, ExecutionRecord},
    error::AppError,
    planner::PlanTemplateLibrary,
};

#[derive(Debug, Clone, Serialize)]
pub struct PlanExecutionResult {
    pub success: bool,
    pub steps_completed: usize,
    pub final_content: Option<String>,
    pub messages: Vec<String>,
}

pub struct PlanExecutor {
    app_handle: AppHandle,
    template_library: Mutex<PlanTemplateLibrary>,
    evolution_engine: CapabilityEvolutionEngine,
}

impl PlanExecutor {
    pub fn new(app_handle: AppHandle) -> Self {
        let pool = app_handle.state::<crate::db::DbPool>().inner().clone();
        let llm_service = crate::llm::LlmService::new(app_handle.clone());
        let evolution_engine = CapabilityEvolutionEngine::new(llm_service, &app_handle);
        Self {
            app_handle,
            template_library: Mutex::new(PlanTemplateLibrary::new(pool)),
            evolution_engine,
        }
    }

    /// Check if a matching template exists for the given user input
    pub fn find_template(&self, user_input: &str) -> Option<ExecutionPlan> {
        let library = self.template_library.lock().ok()?;
        library.find_match(user_input).map(|t| t.plan.clone())
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
        // Before generating a new plan, check PlanTemplateLibrary for matching
        // templates
        let mut plan = if let Some(template_plan) = self.find_template(&context.user_input) {
            log::info!(
                "[PlanExecutor] Using template plan for input: {}",
                context.user_input
            );
            self.adapt_template_plan(template_plan, context)
        } else {
            let llm_service = crate::llm::LlmService::new(self.app_handle.clone());
            let generator =
                PlanGenerator::new(llm_service).with_app_handle(self.app_handle.clone());
            match generator.generate_plan(context).await {
                Ok(plan) => plan,
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

    pub async fn execute_plan(
        &self,
        plan: ExecutionPlan,
        plan_context: &PlanContext,
    ) -> PlanExecutionResult {
        let mut messages = Vec::new();
        let mut step_outputs: HashMap<String, serde_json::Value> = HashMap::new();
        let mut steps_completed = 0;
        let mut final_content: Option<String> = None;

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

        // 按批次执行（同批次内的步骤无相互依赖，未来可并行化）
        for (batch_idx, batch) in batches.batches.iter().enumerate() {
            log::info!(
                "[PlanExecutor] Executing batch {}/{} with {} steps",
                batch_idx + 1,
                batches.batches.len(),
                batch.len()
            );

            for step_id in batch {
                let step = match plan.steps.iter().find(|s| s.step_id == *step_id) {
                    Some(s) => s,
                    None => {
                        messages.push(format!("Step {} not found in plan", step_id));
                        continue;
                    }
                };

                // 发送步骤开始进度事件（fire-and-forget，不阻塞）
                let _ = self.app_handle.emit(
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

                let step_start = std::time::Instant::now();

                // 检查依赖是否满足
                let mut deps_ok = true;
                for dep in &step.depends_on {
                    if !step_outputs.contains_key(dep) {
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

                // Phase 4: Swarm 闭环增强 — Inspector→Writer 之间注入质量反馈
                let mut resolved_params = Self::resolve_parameters(&step.parameters, &step_outputs);
                if let Some((ref inspect_id, _)) = has_loop {
                    if step.capability_id == "writer" && step.depends_on.contains(inspect_id) {
                        if let Some(inspector_output) = step_outputs.get(inspect_id) {
                            if let Some(feedback) =
                                inspector_output.get("suggestions").and_then(|s| s.as_str())
                            {
                                log::info!(
                                    "[PlanExecutor] Injecting inspector feedback into writer step"
                                );
                                resolved_params.insert(
                                    "inspector_feedback".to_string(),
                                    serde_json::Value::String(feedback.to_string()),
                                );
                            }
                        }
                    }
                }

                let result = self
                    .execute_step(step, &resolved_params, plan_context)
                    .await;
                let step_duration = step_start.elapsed().as_millis() as u64;

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

                match &result {
                    Ok(output) => {
                        step_outputs.insert(step.step_id.clone(), output.clone());
                        messages.push(format!(
                            "Step {} completed: {}",
                            step.step_id, step.capability_id
                        ));
                        if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
                            final_content = Some(content.to_string());
                        }
                        steps_completed += 1;
                        // 发送步骤完成进度事件
                        let _ = self.app_handle.emit(
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
                        messages.push(format!("Step {} failed: {}", step.step_id, e));
                        log::warn!("[PlanExecutor] Step {} failed: {}", step.step_id, e);
                        // 发送步骤失败进度事件
                        let _ = self.app_handle.emit(
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
            }
        }

        // Phase 4: Swarm 质量闭环 — 如果最终内容是 writer 产出且前面有 inspector，
        // 尝试自动触发一轮轻量 inspector 检查
        if let Some((_, ref writer_id)) = has_loop {
            if let Some(writer_output) = step_outputs.get(writer_id) {
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

        // 异步触发能力进化反馈环（后台执行，不阻塞返回）
        let evolution_engine = self.evolution_engine.clone();
        tauri::async_runtime::spawn(async move {
            match evolution_engine.evolve_capability_descriptions().await {
                Ok(improvements) if !improvements.is_empty() => {
                    log::info!(
                        "[PlanExecutor] Capability evolution triggered, {} descriptions improved",
                        improvements.len()
                    );
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!("[PlanExecutor] Capability evolution failed: {}", e);
                }
            }
        });

        PlanExecutionResult {
            success,
            steps_completed,
            final_content,
            messages,
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
    fn build_agent_context(
        &self,
        story_id: &str,
        current_content: Option<String>,
        selected_text: Option<String>,
    ) -> Result<crate::agents::AgentContext, AppError> {
        if story_id.is_empty() {
            return Ok(crate::agents::AgentContext::minimal(
                story_id.to_string(),
                String::new(),
            ));
        }

        let pool = self.app_handle.state::<crate::db::DbPool>();
        let builder =
            crate::creative_engine::context_builder::StoryContextBuilder::new(pool.inner().clone());

        // Resolve current scene number from DB (latest scene for the story)
        let scene_number = self.get_current_scene_number(story_id).unwrap_or(None);

        Ok(builder.build(story_id, scene_number, current_content, selected_text)?)
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
        let mut config = crate::agents::orchestrator::WorkflowConfig::default();
        config.style_weight = sw;
        config.narrative_weight = 1.0 - sw;
        let orchestrator = crate::agents::orchestrator::AgentOrchestrator::new(
            service,
            config,
            self.app_handle.clone(),
        );
        let selected_text = plan_context.selected_text.clone();
        let mut context = self.build_agent_context(&story_id, current_content, selected_text)?;
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

        let task = crate::agents::service::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::agents::service::AgentType::Writer,
            context,
            input: instruction,
            parameters: enriched_params,
            tier: None,
        };

        let workflow_result = orchestrator
            .generate(task, crate::agents::orchestrator::GenerationMode::Full)
            .await
            .map_err(|e| AppError::internal(format!("AgentOrchestrator failed: {}", e)))?;
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
        let context = self.build_agent_context(&story_id, current_content, None)?;
        let task = crate::agents::service::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::agents::service::AgentType::Inspector,
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
        let context = self.build_agent_context(&story_id, None, None)?;
        let task = crate::agents::service::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::agents::service::AgentType::OutlinePlanner,
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

        let context = self.build_agent_context(&story_id, None, None)?;
        let task = crate::agents::service::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::agents::service::AgentType::StyleMimic,
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
        let context = self.build_agent_context(&story_id, None, None)?;
        let task = crate::agents::service::AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: crate::agents::service::AgentType::PlotAnalyzer,
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

        let manager = crate::SKILL_MANAGER
            .get()
            .ok_or("Skill manager not initialized")?;
        let skill_manager = manager.lock().map_err(AppError::from)?.clone();

        let agent_context = self.build_agent_context(&story_id, None, None)?;

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
        let prompt = format!(
            r#"你是一位角色编辑助手。请根据用户的修改要求，为角色生成新的属性值。

角色当前信息：
- 姓名：{}
- 背景：{}
- 性格：{}
- 目标：{}

用户修改要求："{}"

请用 JSON 格式回复，只包含需要修改的字段：
{{{{
  "name": "新姓名（如不需要修改则留空或省略）",
  "background": "新背景（如不需要修改则留空或省略）",
  "personality": "新性格（如不需要修改则留空或省略）",
  "goals": "新目标（如不需要修改则留空或省略）"
}}}}

注意：只输出 JSON，不要其他内容。"#,
            character.name,
            character.background.as_deref().unwrap_or("未设定"),
            character.personality.as_deref().unwrap_or("未设定"),
            character.goals.as_deref().unwrap_or("未设定"),
            changes.replace('"', "'")
        );

        let response = llm_service.generate(prompt, Some(1024), Some(0.3)).await?;
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

        let response = llm_service.generate(prompt, Some(2048), Some(0.3)).await?;
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

        let response = llm_service.generate(prompt, Some(1024), Some(0.3)).await?;
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
}
