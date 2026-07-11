//! IntentionGraphPlanner - SING 意图图驱动的计划生成器
//!
//! 将 SING 意图图理论集成到 StoryMoss 的计划生成流程中：
//! 1. 意图合成（IntentSynthesisPipeline）→ 原子意图
//! 2. 分层发现（LayeredDiscovery）→ 相关资产
//! 3. 执行图构建（ExecutionGraphBuilder）→ ExecutionPlan
//! 4. 回退机制：若意图图失败，自动回退到原有 PlanGenerator
//!
//! 设计原则：零回归风险——保留所有现有 PlanGenerator 行为，
//! 仅在意图图置信度足够高时启用新路径。

use std::collections::HashMap;

use tauri::{AppHandle, Emitter, Manager};

use crate::{
    error::AppError,
    intention_graph::{
        builder::IntentSynthesisPipeline,
        context::IntentContext,
        discovery::LayeredDiscovery,
        graph::IntentionGraphRepository,
        models::{
            AssetDiscoveryResult, AssetNode, AssetType, ExecutionGraph, ExecutionGraphStatus,
            ExecutionNode, ExecutionNodeStatus, IntentSynthesisResult,
        },
        reactor::{DynamicReactor, ReActAction},
        scorer::{GraphScorer, PprConfig},
    },
    llm::LlmService,
    planner::{ExecutionPlan, PlanContext, PlanGenerator, PlanStep},
};

/// 意图图计划生成器
///
/// 包装原有 PlanGenerator，在意图图可用时优先使用意图图路径。
pub struct IntentionGraphPlanner {
    llm_service: LlmService,
    app_handle: Option<AppHandle>,
    graph_repo: Option<IntentionGraphRepository>,
    fallback_generator: PlanGenerator,
    /// 启用意图图的最小置信度阈值
    min_confidence_threshold: f64,
    /// 是否强制使用回退（用于调试或降级）
    force_fallback: bool,
}

impl IntentionGraphPlanner {
    pub fn new(llm_service: LlmService, app_handle: Option<AppHandle>) -> Self {
        let fallback_generator = PlanGenerator::new(llm_service.clone());
        Self {
            llm_service: llm_service.clone(),
            app_handle: app_handle.clone(),
            graph_repo: None,
            fallback_generator,
            min_confidence_threshold: 0.5,
            force_fallback: false,
        }
    }

    pub fn with_app_handle(mut self, app_handle: AppHandle) -> Self {
        self.app_handle = Some(app_handle.clone());
        self.fallback_generator = self.fallback_generator.with_app_handle(app_handle);
        self
    }

    pub fn with_graph_repo(mut self, repo: IntentionGraphRepository) -> Self {
        self.graph_repo = Some(repo);
        self
    }

    pub fn with_min_confidence(mut self, threshold: f64) -> Self {
        self.min_confidence_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn force_fallback(mut self, force: bool) -> Self {
        self.force_fallback = force;
        self
    }

    fn emit_progress(&self, stage: &str, message: &str) {
        if let Some(ref app) = self.app_handle {
            let _ = app.emit(
                "intention-graph-planner-progress",
                serde_json::json!({
                    "stage": stage,
                    "message": message,
                }),
            );
        }
    }

    /// 生成执行计划——意图图优先路径，带自动回退
    ///
    /// 流程：
    /// 1. 意图合成（三阶段）
    /// 2. 置信度检查 → 低置信度直接回退
    /// 3. 分层发现（Server-level PPR + Tool-level 融合）
    /// 4. 执行图构建 → 转换为 ExecutionPlan
    /// 5. 若任何环节失败，回退到原有 PlanGenerator
    pub async fn generate_plan(&self, context: &PlanContext) -> Result<ExecutionPlan, AppError> {
        // 若强制回退或意图图不可用，直接走原有路径
        if self.force_fallback || self.graph_repo.is_none() {
            log::info!("[IntentionGraphPlanner] Fallback mode: using PlanGenerator");
            return self.fallback_generator.generate_plan(context).await;
        }

        let start = std::time::Instant::now();
        self.emit_progress("synthesis", "正在合成创作意图...");

        // Step 1: 意图合成
        let synthesis_result = match self.synthesize_intent(context).await {
            Ok(result) => result,
            Err(e) => {
                log::warn!(
                    "[IntentionGraphPlanner] Intent synthesis failed ({}), falling back to PlanGenerator",
                    e
                );
                return self.fallback_generator.generate_plan(context).await;
            }
        };

        log::info!(
            "[IntentionGraphPlanner] Synthesis confidence: {:.2}, root_intent: {} {}",
            synthesis_result.confidence,
            synthesis_result.root_intention.verb,
            synthesis_result.root_intention.object
        );

        // Step 2: 置信度检查
        if synthesis_result.confidence < self.min_confidence_threshold {
            log::info!(
                "[IntentionGraphPlanner] Confidence {:.2} below threshold {:.2}, falling back",
                synthesis_result.confidence,
                self.min_confidence_threshold
            );
            return self.fallback_generator.generate_plan(context).await;
        }

        self.emit_progress("discovery", "正在发现相关创作资产...");

        // Step 3: 分层发现
        let discovered_assets = match self
            .discover_assets(&synthesis_result, &context.user_input)
            .await
        {
            Ok(assets) => assets,
            Err(e) => {
                log::warn!(
                    "[IntentionGraphPlanner] Asset discovery failed ({}), falling back",
                    e
                );
                return self.fallback_generator.generate_plan(context).await;
            }
        };

        log::info!(
            "[IntentionGraphPlanner] Discovered {} assets",
            discovered_assets.len()
        );

        // Step 4: 执行图构建 → ExecutionPlan
        self.emit_progress("building_plan", "正在构建执行计划...");
        match self
            .build_execution_plan(&synthesis_result, &discovered_assets, context)
            .await
        {
            Ok(plan) => {
                let elapsed = start.elapsed().as_millis();
                log::info!(
                    "[IntentionGraphPlanner] Plan generated in {}ms ({} steps, intention-graph path)",
                    elapsed,
                    plan.steps.len()
                );
                self.emit_progress("complete", "执行计划构建完成");
                Ok(plan)
            }
            Err(e) => {
                log::warn!(
                    "[IntentionGraphPlanner] Plan building failed ({}), falling back",
                    e
                );
                self.fallback_generator.generate_plan(context).await
            }
        }
    }

    // ------------------------------------------------------------------
    // 内部步骤
    // ------------------------------------------------------------------

    /// 意图合成：将用户输入转化为原子意图
    async fn synthesize_intent(
        &self,
        context: &PlanContext,
    ) -> Result<IntentSynthesisResult, AppError> {
        let pipeline = IntentSynthesisPipeline::new(self.llm_service.clone());

        // 构建意图上下文
        let mut intent_context = IntentContext::new()
            .with_story_id(context.current_story_id.clone().unwrap_or_default());

        // 将会话参数存入 context
        intent_context.set_param("user_input", &context.user_input);
        if let Some(ref preview) = context.current_content_preview {
            intent_context.set_param("current_content", preview);
        }
        intent_context.set_param("story_progress", &context.story_progress);
        intent_context.set_param("scene_count", context.scene_count);
        intent_context.set_param("has_chapters", context.has_chapters);
        intent_context.set_param("chapter_count", context.chapter_count);
        if let Some(ref selected) = context.selected_text {
            intent_context.set_param("selected_text", selected);
        }
        intent_context.add_input(context.user_input.clone());

        // v0.20.1: LLM 增强合成（失败时内部回退到规则匹配）
        let pool = self
            .app_handle
            .as_ref()
            .and_then(|app| app.try_state::<crate::db::DbPool>())
            .map(|s| s.inner().clone());
        let result = pipeline
            .synthesize_full(&context.user_input, &intent_context, pool.as_ref())
            .await?;
        Ok(result)
    }

    /// 分层发现：基于意图发现相关资产
    async fn discover_assets(
        &self,
        synthesis: &IntentSynthesisResult,
        user_input: &str,
    ) -> Result<Vec<AssetDiscoveryResult>, AppError> {
        let graph_repo = self
            .graph_repo
            .as_ref()
            .ok_or_else(|| AppError::internal("Graph repository not available".to_string()))?;

        let scorer = crate::intention_graph::GraphScorer::new(
            crate::intention_graph::scorer::PprConfig::default(),
        );
        let discovery = LayeredDiscovery::new(scorer);

        // 使用根意图进行发现
        let max_results = 10;
        let mut results = discovery.discover(&synthesis.root_intention, graph_repo, max_results)?;

        // Phase 2: 对用户输入做 GenreResolver 复合题材解析，补充相关 genre_profile 资产
        if let Some(pool) = self
            .app_handle
            .as_ref()
            .and_then(|app| app.try_state::<crate::db::DbPool>())
        {
            let repo = crate::db::GenreProfileRepository::new(pool.inner().clone());
            let resolver = crate::strategy::GenreResolver::new();
            match resolver.resolve_from_text(user_input, &repo) {
                Ok(matches) if !matches.is_empty() => {
                    let existing_ids: std::collections::HashSet<String> =
                        results.iter().map(|r| r.asset.id.clone()).collect();
                    for m in matches {
                        let asset_id = format!("genre_profile.{}", m.profile_id);
                        if existing_ids.contains(&asset_id) {
                            continue;
                        }
                        if let Some(asset) = graph_repo.get_asset(&asset_id)? {
                            results.push(AssetDiscoveryResult {
                                asset,
                                score: m.score.min(1.0),
                                semantic_score: m.score,
                                intent_score: m.score,
                                ppr_score: 0.0,
                                collab_score: 0.0,
                                reason: format!("GenreResolver 复合题材解析: {}", m.reason),
                            });
                        }
                    }
                    // 按分数降序，保持前 max_results
                    results.sort_by(|a, b| {
                        b.score
                            .partial_cmp(&a.score)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    results.truncate(max_results);
                }
                _ => {}
            }
        }

        Ok(results)
    }

    /// 构建执行计划：将发现的资产转换为 ExecutionPlan
    async fn build_execution_plan(
        &self,
        synthesis: &IntentSynthesisResult,
        discovered_assets: &[AssetDiscoveryResult],
        context: &PlanContext,
    ) -> Result<ExecutionPlan, AppError> {
        let mut steps = Vec::new();
        let mut step_index = 0;

        // 按资产类型分组和排序
        let mut agent_steps: Vec<PlanStep> = Vec::new();
        let mut skill_steps: Vec<PlanStep> = Vec::new();
        let mut system_steps: Vec<PlanStep> = Vec::new();
        let mut mcp_steps: Vec<PlanStep> = Vec::new();

        for discovery in discovered_assets {
            let asset = &discovery.asset;
            let step = self.asset_to_plan_step(asset, context, step_index)?;
            step_index += 1;

            match asset.asset_type {
                AssetType::Agent => agent_steps.push(step),
                AssetType::Skill => skill_steps.push(step),
                AssetType::SystemCommand => system_steps.push(step),
                AssetType::McpTool => mcp_steps.push(step),
                _ => {
                    // 方法论、风格DNA等作为上下文注入，不生成独立步骤
                    log::debug!(
                        "[IntentionGraphPlanner] Asset {} ({:?}) injected as context",
                        asset.name,
                        asset.asset_type
                    );
                }
            }
        }

        // 组装步骤：系统命令 → Agent → 技能 → MCP
        // 依赖关系：技能可能依赖 Agent 的输出
        steps.extend(system_steps);
        steps.extend(agent_steps.clone());

        // 技能步骤添加对前一个 Agent 的依赖
        for (_idx, mut skill_step) in skill_steps.into_iter().enumerate() {
            if !agent_steps.is_empty() {
                let last_agent = &agent_steps[agent_steps.len() - 1];
                skill_step.depends_on.push(last_agent.step_id.clone());
            }
            steps.push(skill_step);
        }

        // MCP 步骤添加依赖
        for mut mcp_step in mcp_steps {
            if !steps.is_empty() {
                mcp_step
                    .depends_on
                    .push(steps[steps.len() - 1].step_id.clone());
            }
            steps.push(mcp_step);
        }

        // 如果没有发现任何可执行步骤，回退失败
        if steps.is_empty() {
            return Err(AppError::internal(
                "No executable assets discovered from intention graph".to_string(),
            ));
        }

        // 构建理解文本
        let understanding = format!(
            "[意图图] {} {} → 发现 {} 个资产（置信度: {:.2}）",
            synthesis.root_intention.verb,
            synthesis.root_intention.object,
            discovered_assets.len(),
            synthesis.confidence
        );

        Ok(ExecutionPlan {
            understanding,
            steps,
            fallback_message: "意图图计划执行失败，请重试".to_string(),
        })
    }

    /// 将资产节点转换为 PlanStep
    fn asset_to_plan_step(
        &self,
        asset: &AssetNode,
        context: &PlanContext,
        index: usize,
    ) -> Result<PlanStep, AppError> {
        let step_id = format!("ig_step_{}", index + 1);

        // 资产类型 → capability_id 映射
        let capability_id = match asset.asset_type {
            AssetType::Agent => {
                // Agent 资产：writer, inspector, outline_planner 等
                asset
                    .capability_id
                    .clone()
                    .unwrap_or_else(|| asset.name.to_lowercase().replace(' ', "_"))
            }
            AssetType::Skill => {
                // 技能资产：builtin.style_enhancer 等
                asset.capability_id.clone().unwrap_or_else(|| {
                    format!("builtin.{}", asset.name.to_lowercase().replace(' ', "_"))
                })
            }
            AssetType::SystemCommand => {
                // 系统命令：create_chapter, update_character 等
                asset
                    .capability_id
                    .clone()
                    .unwrap_or_else(|| asset.name.to_lowercase().replace(' ', "_"))
            }
            AssetType::McpTool => {
                // MCP 工具：mcp.server_id.tool_name
                asset.capability_id.clone().unwrap_or_else(|| {
                    format!("mcp.{}", asset.name.to_lowercase().replace(' ', "_"))
                })
            }
            _ => asset.name.to_lowercase().replace(' ', "_"),
        };

        // 构建参数
        let mut parameters = HashMap::new();

        // 注入上下文参数
        if let Some(ref story_id) = context.current_story_id {
            parameters.insert(
                "story_id".to_string(),
                serde_json::Value::String(story_id.clone()),
            );
        }
        if let Some(ref preview) = context.current_content_preview {
            parameters.insert(
                "current_content".to_string(),
                serde_json::Value::String(preview.clone()),
            );
        }
        if let Some(ref selected) = context.selected_text {
            parameters.insert(
                "selected_text".to_string(),
                serde_json::Value::String(selected.clone()),
            );
        }

        // 注入资产元数据参数
        if let Some(ref metadata) = asset.metadata {
            if let Some(obj) = metadata.as_object() {
                for (key, value) in obj {
                    parameters.insert(key.clone(), value.clone());
                }
            }
        }

        // Phase 2/3: 把资产标签与资产 ID 单独注入，便于 AgentService 透传给模型网关
        let tags: Vec<String> = asset.tags();
        if !tags.is_empty() {
            parameters.insert(
                "asset_tags".to_string(),
                serde_json::to_value(&tags).unwrap_or_default(),
            );
        }
        parameters.insert(
            "discovered_asset_ids".to_string(),
            serde_json::to_value(vec![asset.id.clone()]).unwrap_or_default(),
        );

        // 用户输入作为 instruction
        parameters.insert(
            "instruction".to_string(),
            serde_json::Value::String(context.user_input.clone()),
        );

        Ok(PlanStep {
            step_id,
            capability_id,
            purpose: format!("[意图图] {}: {}", asset.name, asset.description),
            parameters,
            depends_on: vec![],
            long_running: false,
        })
    }

    /// 将 ExecutionPlan 转换为 ExecutionGraph 并运行动态 ReAct 循环
    ///
    /// 这是 SING 动态执行的核心：计划不是静态的，而是在执行过程中
    /// 根据输出动态发现新意图和资产。
    /// 将 ExecutionPlan 转换为 ExecutionGraph 并运行动态 ReAct 循环
    ///
    /// 这是 SING 动态执行的核心：计划不是静态的，而是在执行过程中
    /// 根据输出动态发现新意图和资产。
    ///
    /// v0.20.1: `invoke_fn` 回调让每个 Invoke 动作真正执行对应的 PlanStep，
    /// 而非硬编码假输出。执行图在循环结束后持久化到数据库供诊断面板查询。
    pub async fn execute_with_react<F, Fut>(
        &self,
        plan: &ExecutionPlan,
        request_id: &str,
        story_id: Option<&str>,
        invoke_fn: F,
    ) -> Result<(Vec<ReActAction>, serde_json::Value), AppError>
    where
        F: Fn(&str, &serde_json::Value) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value, AppError>>,
    {
        let mut graph = ExecutionGraph {
            id: format!("eg_{}", request_id),
            request_id: request_id.to_string(),
            story_id: story_id.map(|s| s.to_string()),
            user_input: plan.understanding.clone(),
            root_intention_id: None,
            status: ExecutionGraphStatus::Building,
            plan_json: Some(serde_json::to_string(plan).unwrap_or_default()),
            result_json: None,
            created_at: chrono::Local::now(),
            completed_at: None,
            execution_time_ms: None,
        };

        // 将 PlanStep 转换为 ExecutionNode
        let mut nodes: Vec<ExecutionNode> = plan
            .steps
            .iter()
            .map(|step| ExecutionNode {
                id: step.step_id.clone(),
                graph_id: graph.id.clone(),
                intention_id: Some(step.capability_id.clone()),
                asset_id: Some(step.capability_id.clone()),
                status: ExecutionNodeStatus::Pending,
                parameters: Some(serde_json::to_value(&step.parameters).unwrap_or_default()),
                depends_on: if step.depends_on.is_empty() {
                    None
                } else {
                    Some(step.depends_on.clone())
                },
                outputs: None,
                discovered_from: crate::intention_graph::models::DiscoverySource::Synthesis,
                execution_time_ms: None,
                created_at: chrono::Local::now(),
                completed_at: None,
            })
            .collect();

        let reactor = DynamicReactor::new(20, 50);
        let mut actions = Vec::new();
        let mut iteration = 0;
        let start = std::time::Instant::now();

        loop {
            let (action, should_continue) = reactor.step(&mut graph, &mut nodes, iteration)?;
            actions.push(action.clone());

            match &action {
                ReActAction::Invoke {
                    node_id,
                    parameters,
                } => {
                    log::info!("[IntentionGraphPlanner] ReAct Invoke: {}", node_id);
                    let node_start = std::time::Instant::now();

                    // v0.20.1: 真正执行步骤——调用 invoke_fn 回调
                    let exec_result = invoke_fn(node_id, parameters).await;
                    if let Some(node) = nodes.iter_mut().find(|n| n.id == *node_id) {
                        match exec_result {
                            Ok(outputs) => {
                                node.status = ExecutionNodeStatus::Completed;
                                node.outputs = Some(outputs);
                                node.execution_time_ms =
                                    Some(node_start.elapsed().as_millis() as i64);
                                node.completed_at = Some(chrono::Local::now());
                            }
                            Err(e) => {
                                log::warn!(
                                    "[IntentionGraphPlanner] Step {} failed: {}",
                                    node_id,
                                    e
                                );
                                node.status = ExecutionNodeStatus::Failed;
                                node.outputs = Some(serde_json::json!({
                                    "error": e.to_string()
                                }));
                                node.completed_at = Some(chrono::Local::now());
                            }
                        }
                    }
                }
                ReActAction::Discover {
                    source_node_id,
                    reason,
                } => {
                    log::info!(
                        "[IntentionGraphPlanner] ReAct Discover from {}: {}",
                        source_node_id,
                        reason
                    );
                }
                ReActAction::Respond {
                    message,
                    final_outputs,
                } => {
                    log::info!(
                        "[IntentionGraphPlanner] ReAct Respond: {} (outputs: {})",
                        message,
                        final_outputs
                    );
                    break;
                }
            }

            if !should_continue {
                break;
            }

            iteration += 1;
        }

        let final_outputs = reactor.aggregate_outputs(&nodes);

        // v0.20.1: 持久化执行图到数据库，供前端诊断面板查询
        graph.status = ExecutionGraphStatus::Completed;
        graph.completed_at = Some(chrono::Local::now());
        graph.execution_time_ms = Some(start.elapsed().as_millis() as i64);
        graph.result_json = Some(serde_json::to_string(&final_outputs).unwrap_or_default());

        if let Some(ref repo) = self.graph_repo {
            if let Err(e) = repo.create_execution_graph(&graph) {
                log::warn!(
                    "[IntentionGraphPlanner] Failed to persist execution graph: {}",
                    e
                );
            }
            // 持久化执行节点
            for node in &nodes {
                if let Err(e) = repo.create_execution_node(node) {
                    log::warn!(
                        "[IntentionGraphPlanner] Failed to persist node {}: {}",
                        node.id,
                        e
                    );
                }
            }
        }

        Ok((actions, final_outputs))
    }

    /// 记录执行图到数据库（用于分析和诊断）
    pub async fn record_execution_graph(
        &self,
        request_id: &str,
        story_id: Option<&str>,
        user_input: &str,
        root_intention_id: Option<&str>,
        plan_json: &str,
    ) -> Result<(), AppError> {
        if let Some(ref repo) = self.graph_repo {
            let graph = ExecutionGraph {
                id: format!("eg_{}", request_id),
                request_id: request_id.to_string(),
                story_id: story_id.map(|s| s.to_string()),
                user_input: user_input.to_string(),
                root_intention_id: root_intention_id.map(|s| s.to_string()),
                status: ExecutionGraphStatus::Building,
                plan_json: Some(plan_json.to_string()),
                result_json: None,
                created_at: chrono::Local::now(),
                completed_at: None,
                execution_time_ms: None,
            };
            repo.create_execution_graph(&graph)?;
        }
        Ok(())
    }
}

// ==================== 便捷构造函数 ====================

impl IntentionGraphPlanner {
    /// 从 AppHandle 和数据库连接创建规划器
    ///
    /// 优先复用 lib.rs setup 阶段注册的
    /// IntentionGraphRepository（共享预热缓存），
    /// 若不可用则降级为新建（空缓存，查询将回查 SQLite）。
    pub fn from_app_handle(app_handle: AppHandle) -> Result<Self, AppError> {
        let llm_service = LlmService::new(app_handle.clone());
        let mut planner = Self::new(llm_service, Some(app_handle.clone()));

        // 优先复用 setup 阶段注册的 repository（共享预热 cache）
        if let Some(repo_state) = app_handle.try_state::<IntentionGraphRepository>() {
            planner = planner.with_graph_repo(repo_state.inner().clone());
        } else if let Some(pool_state) = app_handle.try_state::<crate::db::DbPool>() {
            // 降级：DbPool 可用但意图图未注册（理论上不会发生，setup 总会注册）
            let pool = pool_state.inner().clone();
            let graph_repo = IntentionGraphRepository::new(pool);
            planner = planner.with_graph_repo(graph_repo);
            log::warn!("[IntentionGraphPlanner] IntentionGraphRepository not managed, created new instance (cache not warmed)");
        } else {
            log::warn!("[IntentionGraphPlanner] DbPool not available, intention graph disabled");
        }

        Ok(planner)
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intention_graph::models::IntentionNode;

    /// 创建测试用的 PlanContext
    fn test_context() -> PlanContext {
        PlanContext {
            current_story_id: Some("story_1".to_string()),
            has_story: true,
            has_chapters: true,
            chapter_count: 3,
            current_content_preview: Some("测试内容".to_string()),
            user_input: "续写".to_string(),
            scene_count: 3,
            scenes_summary: vec![],
            current_scene_id: Some("scene_1".to_string()),
            current_scene_stage: Some("drafting".to_string()),
            total_word_count: 3000,
            latest_chapter_word_count: 1000,
            story_progress: "developing".to_string(),
            world_building_summary: None,
            character_list: vec![],
            foreshadowing_status: vec![],
            style_dna_info: None,
            mcp_tools_available: vec![],
            deep_insight_summary: None,
            selected_text: None,
            style_weight: 50,
            chapter_number: 1,
            selected_strategy: None,
        }
    }

    #[test]
    fn test_asset_to_capability_id_mapping() {
        let context = test_context();

        // Agent 资产
        let agent_asset =
            AssetNode::new(AssetType::Agent, "writer", "生成故事正文", Some("writer"));
        let step = asset_to_plan_step_static(&agent_asset, &context, 0).unwrap();
        assert_eq!(step.capability_id, "writer");
        assert_eq!(step.step_id, "ig_step_1");
        assert!(step.parameters.contains_key("story_id"));
        assert!(step.parameters.contains_key("instruction"));

        // Skill 资产
        let skill_asset = AssetNode::new(
            AssetType::Skill,
            "style enhancer",
            "增强文本风格",
            Some("builtin.style_enhancer"),
        );
        let step = asset_to_plan_step_static(&skill_asset, &context, 1).unwrap();
        assert_eq!(step.capability_id, "builtin.style_enhancer");

        // SystemCommand 资产
        let sys_asset = AssetNode::new(
            AssetType::SystemCommand,
            "create chapter",
            "创建新章节",
            Some("create_chapter"),
        );
        let step = asset_to_plan_step_static(&sys_asset, &context, 2).unwrap();
        assert_eq!(step.capability_id, "create_chapter");

        // MCP 工具资产
        let mcp_asset = AssetNode::new(
            AssetType::McpTool,
            "duckduckgo search",
            "搜索网络资料",
            None,
        );
        let step = asset_to_plan_step_static(&mcp_asset, &context, 3).unwrap();
        assert_eq!(step.capability_id, "mcp.duckduckgo_search");

        // 方法论资产（无 capability_id，应使用默认映射）
        let method_asset = AssetNode::new(
            AssetType::Methodology,
            "snowflake method",
            "雪花法创作方法论",
            None,
        );
        let step = asset_to_plan_step_static(&method_asset, &context, 4).unwrap();
        assert_eq!(step.capability_id, "snowflake_method");
    }

    #[test]
    fn test_asset_to_plan_step_parameters() {
        let mut context = test_context();
        context.selected_text = Some("选中段落".to_string());

        let asset = AssetNode::new(AssetType::Agent, "writer", "生成故事正文", Some("writer"));
        let step = asset_to_plan_step_static(&asset, &context, 0).unwrap();

        // 验证参数注入
        assert_eq!(
            step.parameters.get("story_id").unwrap().as_str(),
            Some("story_1")
        );
        assert_eq!(
            step.parameters.get("current_content").unwrap().as_str(),
            Some("测试内容")
        );
        assert_eq!(
            step.parameters.get("selected_text").unwrap().as_str(),
            Some("选中段落")
        );
        assert_eq!(
            step.parameters.get("instruction").unwrap().as_str(),
            Some("续写")
        );
    }

    #[test]
    fn test_confidence_threshold_clamping() {
        // 直接测试阈值截断逻辑
        assert_eq!(0.8_f64.clamp(0.0, 1.0), 0.8);
        assert_eq!(1.5_f64.clamp(0.0, 1.0), 1.0);
        assert_eq!((-0.5_f64).clamp(0.0, 1.0), 0.0);
    }

    #[test]
    fn test_empty_discovered_assets_fails() {
        let context = test_context();
        let synthesis = IntentSynthesisResult {
            root_intention: IntentionNode::atomic("generate", "prose", "generate prose"),
            confidence: 0.9,
            sub_intentions: vec![],
            chain_expansion: vec!["generate prose".to_string(), "inspect quality".to_string()],
        };

        let empty_assets: Vec<AssetDiscoveryResult> = vec![];

        // 使用 tokio::runtime::Runtime 来运行 async 测试
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(async {
            build_execution_plan_static(&synthesis, &empty_assets, &context).await
        });

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("No executable assets"));
    }

    /// 静态版本的 asset_to_plan_step，不依赖 self
    fn asset_to_plan_step_static(
        asset: &AssetNode,
        context: &PlanContext,
        index: usize,
    ) -> Result<PlanStep, AppError> {
        let step_id = format!("ig_step_{}", index + 1);

        let capability_id = match asset.asset_type {
            AssetType::Agent => asset
                .capability_id
                .clone()
                .unwrap_or_else(|| asset.name.to_lowercase().replace(' ', "_")),
            AssetType::Skill => asset.capability_id.clone().unwrap_or_else(|| {
                format!("builtin.{}", asset.name.to_lowercase().replace(' ', "_"))
            }),
            AssetType::SystemCommand => asset
                .capability_id
                .clone()
                .unwrap_or_else(|| asset.name.to_lowercase().replace(' ', "_")),
            AssetType::McpTool => asset
                .capability_id
                .clone()
                .unwrap_or_else(|| format!("mcp.{}", asset.name.to_lowercase().replace(' ', "_"))),
            _ => asset.name.to_lowercase().replace(' ', "_"),
        };

        let mut parameters = HashMap::new();

        if let Some(ref story_id) = context.current_story_id {
            parameters.insert(
                "story_id".to_string(),
                serde_json::Value::String(story_id.clone()),
            );
        }
        if let Some(ref preview) = context.current_content_preview {
            parameters.insert(
                "current_content".to_string(),
                serde_json::Value::String(preview.clone()),
            );
        }
        if let Some(ref selected) = context.selected_text {
            parameters.insert(
                "selected_text".to_string(),
                serde_json::Value::String(selected.clone()),
            );
        }

        if let Some(ref metadata) = asset.metadata {
            if let Some(obj) = metadata.as_object() {
                for (key, value) in obj {
                    parameters.insert(key.clone(), value.clone());
                }
            }
        }

        parameters.insert(
            "instruction".to_string(),
            serde_json::Value::String(context.user_input.clone()),
        );

        Ok(PlanStep {
            step_id,
            capability_id,
            purpose: format!("[意图图] {}: {}", asset.name, asset.description),
            parameters,
            depends_on: vec![],
            long_running: false,
        })
    }

    /// 静态版本的 build_execution_plan，不依赖 self
    async fn build_execution_plan_static(
        synthesis: &IntentSynthesisResult,
        discovered_assets: &[AssetDiscoveryResult],
        context: &PlanContext,
    ) -> Result<ExecutionPlan, AppError> {
        let mut steps = Vec::new();
        let mut step_index = 0;

        let mut agent_steps: Vec<PlanStep> = Vec::new();
        let mut skill_steps: Vec<PlanStep> = Vec::new();
        let mut system_steps: Vec<PlanStep> = Vec::new();
        let mut mcp_steps: Vec<PlanStep> = Vec::new();

        for discovery in discovered_assets {
            let asset = &discovery.asset;
            let step = asset_to_plan_step_static(asset, context, step_index)?;
            step_index += 1;

            match asset.asset_type {
                AssetType::Agent => agent_steps.push(step),
                AssetType::Skill => skill_steps.push(step),
                AssetType::SystemCommand => system_steps.push(step),
                AssetType::McpTool => mcp_steps.push(step),
                _ => {}
            }
        }

        steps.extend(system_steps);
        steps.extend(agent_steps.clone());

        for (_i, mut skill_step) in skill_steps.into_iter().enumerate() {
            if !agent_steps.is_empty() {
                let last_agent = &agent_steps[agent_steps.len() - 1];
                skill_step.depends_on.push(last_agent.step_id.clone());
            }
            steps.push(skill_step);
        }

        for mut mcp_step in mcp_steps {
            if !steps.is_empty() {
                mcp_step
                    .depends_on
                    .push(steps[steps.len() - 1].step_id.clone());
            }
            steps.push(mcp_step);
        }

        if steps.is_empty() {
            return Err(AppError::internal(
                "No executable assets discovered from intention graph".to_string(),
            ));
        }

        let understanding = format!(
            "[意图图] {} {} → 发现 {} 个资产（置信度: {:.2}）",
            synthesis.root_intention.verb,
            synthesis.root_intention.object,
            discovered_assets.len(),
            synthesis.confidence
        );

        Ok(ExecutionPlan {
            understanding,
            steps,
            fallback_message: "意图图计划执行失败，请重试".to_string(),
        })
    }
}
