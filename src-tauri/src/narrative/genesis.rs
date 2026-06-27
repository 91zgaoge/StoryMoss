//! GenesisPipeline — 正向/创世流程
//!
//! 替代 planner/bootstrap.rs，基于统一的 NarrativePipeline 框架。
//! 输入：用户概念 premise
//! 输出：NarrativeBundle（包含故事的全部结构要素）

use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::{
    elements::*,
    pipeline::*,
    progress::*,
    prompts::{PromptMode, *},
};
use crate::{
    db::{
        models::{ConflictType, RuleType},
        repositories::{
            ChapterRepository, CharacterRelationshipRepository, CharacterRepository,
            KnowledgeGraphRepository, SceneRepository, SceneUpdate, StoryOutlineRepository,
            StoryRepository, WorldBuildingRepository,
        },
        CreateCharacterRequest, CreateStoryRequest, DbPool, UpdateStoryRequest,
    },
    llm::{service::PipelineContext as LlmPipelineContext, LlmService},
    ports::VectorStore,
    router::{Complexity, Priority, RoutingRequest, TaskType},
    story_system::StorySystemEngine,
    strategy::{load_all_assets, SelectionContext, StrategySelector},
};

// ==================== GenesisContext ====================

/// 创世流水线上下文
///
/// 在流水线执行过程中，各步骤通过此上下文共享数据和状态。
pub struct GenesisContext {
    pub story_id: String,
    pub session_id: String,
    pub user_premise: String,
    /// 叙事元素集合，使用 Arc<RwLock<>> 支持后台阶段分组并行写入
    pub bundle: Arc<tokio::sync::RwLock<NarrativeBundle>>,
    pub current_step: String,
    pub app_handle: AppHandle,
    pub pool: DbPool,
    pub vector_store: Arc<dyn VectorStore>,
    /// 第一章正文内容（用于返回给前端）
    pub first_chapter_content: Option<String>,
    /// 模型为当前故事选择的创作策略
    pub selected_strategy: Option<crate::domain::strategy::SelectedStrategy>,
}

impl StepContext for GenesisContext {
    fn story_id(&self) -> Option<&str> {
        Some(&self.story_id)
    }

    fn set_current_step(&mut self, step_name: &str) {
        self.current_step = step_name.to_string();
    }

    fn current_step(&self) -> &str {
        &self.current_step
    }
}

impl GenesisContext {
    pub fn new(app_handle: AppHandle, user_premise: String) -> Self {
        let pool = app_handle.state::<DbPool>().inner().clone();
        let vector_store = app_handle.state::<Arc<dyn VectorStore>>().inner().clone();
        Self {
            story_id: String::new(),
            session_id: Uuid::new_v4().to_string(),
            user_premise,
            bundle: Arc::new(tokio::sync::RwLock::new(NarrativeBundle::new())),
            current_step: String::new(),
            app_handle,
            pool,
            vector_store,
            first_chapter_content: None,
            selected_strategy: None,
        }
    }

    /// 创建用于后台阶段的上下文（继承即时阶段的结果）
    pub fn for_background(
        app_handle: AppHandle,
        story_id: String,
        session_id: String,
        user_premise: String,
        bundle: NarrativeBundle,
        selected_strategy: Option<crate::domain::strategy::SelectedStrategy>,
    ) -> Self {
        let pool = app_handle.state::<DbPool>().inner().clone();
        let vector_store = app_handle.state::<Arc<dyn VectorStore>>().inner().clone();
        Self {
            story_id,
            session_id,
            user_premise,
            bundle: Arc::new(tokio::sync::RwLock::new(bundle)),
            current_step: String::new(),
            app_handle,
            pool,
            vector_store,
            first_chapter_content: None,
            selected_strategy,
        }
    }

    fn llm_pipeline_ctx(
        &self,
        step_name: &str,
        step_number: usize,
        total_steps: usize,
        action: &str,
    ) -> LlmPipelineContext {
        LlmPipelineContext {
            step_name: step_name.to_string(),
            step_number,
            total_steps,
            action: action.to_string(),
        }
    }
}

/// 根据已选策略和体裁画像构建写作指令中的策略注解
fn build_strategy_notes(ctx: &GenesisContext, genre: &str) -> String {
    let strategy = match &ctx.selected_strategy {
        Some(s) => s,
        None => return format!("（未选择策略，按题材 '{}' 自由发挥）", genre),
    };

    let mut notes = Vec::new();

    if let Some(profile_id) = &strategy.genre_profile_id {
        let repo = crate::db::GenreProfileRepository::new(ctx.pool.clone());
        if let Ok(Some(profile)) = repo.get_by_id(profile_id) {
            notes.push(format!(
                "体裁画像：{}（{}）",
                profile.genre_name, profile.canonical_name
            ));
            if let Some(tone) = &profile.core_tone {
                notes.push(format!("核心基调：{}", tone));
            }
            if let Some(pacing) = &profile.pacing_strategy {
                notes.push(format!("节奏策略：\n{}", pacing));
            }
            if let Some(anti_patterns) = &profile.anti_patterns_json {
                if let Ok(list) = serde_json::from_str::<Vec<String>>(anti_patterns) {
                    if !list.is_empty() {
                        notes.push(format!("应避免的反套路：\n- {}", list.join("\n- ")));
                    }
                }
            }
            if let Some(reference_tables) = &profile.reference_tables_json {
                notes.push(format!("元素参考表：\n{}", reference_tables));
            }
            if let Some(typical_structure) = &profile.typical_structure_json {
                notes.push(format!("典型结构：\n{}", typical_structure));
            }
        } else {
            notes.push(format!("体裁画像 ID：{}（未找到详细内容）", profile_id));
        }
    }

    if let Some(methodology_id) = &strategy.methodology_id {
        if let Some(content) = resolve_methodology_prompt(methodology_id, None) {
            notes.push(format!("\n应遵循的方法论：{}\n{}", methodology_id, content));
        } else {
            notes.push(format!("\n应遵循的方法论：{}", methodology_id));
        }
    }

    if !strategy.style_dna_ids.is_empty() {
        notes.push(format!(
            "\n参考风格 DNA：{}",
            strategy.style_dna_ids.join(", ")
        ));
    }

    if !strategy.skill_ids.is_empty() {
        notes.push(format!(
            "\n建议激活的技能：{}",
            strategy.skill_ids.join(", ")
        ));
    }

    if notes.is_empty() {
        format!("（按题材 '{}' 自由发挥）", genre)
    } else {
        notes.join("\n")
    }
}

/// 从 PromptRegistry 读取指定方法论的当前 prompt 内容（不引入新的硬编码文本）
fn resolve_methodology_prompt(methodology_id: &str, step: Option<&str>) -> Option<String> {
    let prompt_id = match methodology_id {
        "snowflake" => format!("methodology_snowflake_step{}", step.unwrap_or("1")),
        "hero_journey" => "methodology_hero_journey".to_string(),
        "scene_structure" => "methodology_scene_structure".to_string(),
        "character_depth" => "methodology_character_depth".to_string(),
        "high_density_world_building" => {
            let phase = step.unwrap_or("1");
            match phase {
                "1" | "seed" => "methodology_hdwb_seed",
                "2" | "expansion" => "methodology_hdwb_expansion",
                "3" | "convergence" => "methodology_hdwb_convergence",
                "4" | "iteration" => "methodology_hdwb_iteration",
                _ => "methodology_hdwb_seed",
            }
            .to_string()
        }
        _ => return None,
    };
    crate::prompts::registry::resolve_prompt_default(&prompt_id)
}

/// 将已选策略中的中文叙事四元组渲染为 prompt 可注入文本
fn build_narrative_quartet(ctx: &GenesisContext) -> Option<String> {
    let strategy = ctx.selected_strategy.as_ref()?;
    let value = crate::strategy::quartet_inference::serialize_quartet_for_prompt(strategy).ok()?;
    if value.is_null() {
        return None;
    }
    Some(value.to_string())
}

// ==================== GenesisPipeline 构建器 ====================

pub struct GenesisPipeline;

impl GenesisPipeline {
    /// 快速阶段：故事概念 + 第一章正文，目标 30-60 秒返回给用户
    pub fn quick_phase_steps() -> Vec<Box<dyn PipelineStep<GenesisContext>>> {
        vec![
            Box::new(ConceptGenerationStep),
            Box::new(FirstChapterGenerationStep),
        ]
    }

    /// 后台阶段：策略选择 + 世界观/大纲/角色/场景/伏笔/知识图谱 + 合同播种
    /// v0.23.14: FirstChapterGenerationStep 已移至快速阶段，后台不再包含。
    pub fn background_steps() -> Vec<Box<dyn PipelineStep<GenesisContext>>> {
        vec![
            Box::new(StrategySelectionStep),
            Box::new(ParallelWorldOutlineCharacterStep),
            Box::new(SceneGenerationStep),
            Box::new(ForeshadowingGenerationStep),
            Box::new(KnowledgeGraphGenerationStep),
            Box::new(ContractSeedingStep),
        ]
    }
}

// ==================== Step 1: 概念生成 ====================

struct ConceptGenerationStep;

impl PipelineStep<GenesisContext> for ConceptGenerationStep {
    fn name(&self) -> &'static str {
        "构思故事"
    }
    fn description(&self) -> &'static str {
        "生成故事概念（标题、简介、题材）"
    }
    fn step_number(&self) -> usize {
        1
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 2,
                status: StepStatus::Running,
                message: "正在调用AI生成故事概念...".to_string(),
                progress_percent: 10,
                elapsed_seconds: 0,
                metadata: None,
            });

            let app_dir = ctx.app_handle.path().app_data_dir().unwrap_or_default();
            let (concept_max_tokens, concept_temperature) =
                crate::config::AppConfig::load(&app_dir)
                    .map(|c| {
                        let profile_id = c.active_llm_profile.as_deref();
                        let profile = profile_id.and_then(|id| c.llm_profiles.get(id));
                        (
                            profile.map(|p| p.max_tokens).unwrap_or(512),
                            profile.map(|p| p.temperature).unwrap_or(0.7),
                        )
                    })
                    .unwrap_or((512, 0.7));

            let genre_repo = crate::db::GenreProfileRepository::new(ctx.pool.clone());
            let available_profiles = genre_repo.get_all().unwrap_or_default();
            let prompt = story_concept_prompt(
                PromptMode::Generate,
                &ctx.user_premise,
                Some(&available_profiles),
                Some(&ctx.pool),
            );
            let pipeline_ctx =
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 2, "生成故事概念");
            let request = RoutingRequest {
                task: TaskType::WorldBuilding,
                complexity: Complexity::Medium,
                budget_priority: Priority::Low,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            };
            let response = llm
                .generate_for_request_with_context_and_pipeline(
                    request,
                    prompt,
                    Some(concept_max_tokens),
                    Some(concept_temperature),
                    Some("生成故事概念"),
                    Some(pipeline_ctx),
                )
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            log::warn!(
                "[GenesisDiag] ConceptGenerationStep: LLM 返回，content_len={}，开始解析 JSON",
                response.content.len()
            );

            let content = response.content.trim();
            let json_str = super::extract_and_sanitize_json(content)
                .map_err(|e| PipelineError::ParseError(e))?;
            log::warn!(
                "[GenesisDiag] ConceptGenerationStep: JSON 提取成功，len={}，开始反序列化",
                json_str.len()
            );
            let meta: StoryMetaElement = serde_json::from_str(&json_str).or_else(|e| {
                // v0.23.55: serde 解析失败时，用正则逐字段提取作为兜底。
                // 根因：本地模型（如 MN-Oblivion）经常在 JSON 字符串值里放
                // 未转义的双引号或特殊字符，导致 serde "expected `,` or `}`" 错误。
                // 正则提取不依赖严格的 JSON 语法，容错性更强。
                log::warn!("[GenesisDiag] serde 解析失败: {}，尝试正则兜底提取", e);
                super::extract_story_meta_fallback(&json_str)
                    .ok_or_else(|| PipelineError::ParseError(format!("解析故事概念失败: {}", e)))
            })?;
            log::warn!(
                "[GenesisDiag] ConceptGenerationStep: 反序列化成功 title={}，开始创建 Story 记录",
                meta.title
            );

            // 创建 Story 记录；若 LLM 已返回标准化 genre_profile_ids，优先使用首个
            let primary_genre_profile_id = meta.genre_profile_ids.first().cloned();
            // v0.23.15: spawn_blocking 包裹 sync DB 操作，防止连接池满/DB 锁阻塞
            // tokio worker 线程，导致 smart_execute 的 tokio::time::timeout 无法触发。
            let pool = ctx.pool.clone();
            let req = CreateStoryRequest {
                title: meta.title.clone(),
                description: Some(meta.description.clone()),
                genre: Some(meta.genre.clone()),
                style_dna_id: None,
                genre_profile_id: primary_genre_profile_id,
                methodology_id: None,
                reference_book_id: None,
            };
            let story = tokio::task::spawn_blocking(move || {
                let story_repo = StoryRepository::new(pool);
                story_repo.create(req)
            })
            .await
            .map_err(|e| PipelineError::StorageError(format!("spawn_blocking 失败: {}", e)))?
            .map_err(|e| PipelineError::StorageError(e.to_string()))?;
            log::warn!(
                "[GenesisDiag] ConceptGenerationStep: Story 创建成功 id={}，写入 ctx",
                story.id
            );

            ctx.story_id = story.id.clone();
            let title = meta.title.clone();
            {
                let mut bundle = ctx.bundle.write().await;
                *bundle = bundle.clone().with_story_meta(StoryMetaElement {
                    id: story.id.clone(),
                    ..meta
                });
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 2,
                status: StepStatus::Completed,
                message: format!("故事概念已生成：《{}", title),
                progress_percent: 50,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 2: 策略选择 ====================

struct StrategySelectionStep;

impl PipelineStep<GenesisContext> for StrategySelectionStep {
    fn name(&self) -> &'static str {
        "选择创作策略"
    }
    fn description(&self) -> &'static str {
        "根据故事概念自动选择体裁画像、方法论、风格 DNA 与技能"
    }
    fn step_number(&self) -> usize {
        2
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 2,
                status: StepStatus::Running,
                message: "正在为故事匹配最优创作策略...".to_string(),
                progress_percent: 55,
                elapsed_seconds: 0,
                metadata: None,
            });

            let (genre, preferred_genre_profile_ids) = {
                let bundle = ctx.bundle.read().await;
                bundle
                    .story_meta
                    .as_ref()
                    .map(|m| (m.genre.clone(), m.genre_profile_ids.clone()))
                    .unwrap_or_default()
            };

            let app_dir = ctx.app_handle.path().app_data_dir().unwrap_or_default();
            let word_count_target = crate::config::AppConfig::load(&app_dir)
                .map(|c| c.genesis_first_chapter_word_count_target)
                .unwrap_or(2000);

            let genre_repo = crate::db::GenreProfileRepository::new(ctx.pool.clone());
            let skills =
                crate::skills::SkillManager::from_app_handle(&ctx.app_handle).get_all_skills();

            let assets =
                load_all_assets(&genre_repo, &skills).map_err(|e| PipelineError::StepFailed {
                    step_name: self.name().to_string(),
                    reason: format!("加载创作资产失败: {}", e),
                })?;

            let selector = StrategySelector::new(llm.clone(), ctx.pool.clone());
            let selection_ctx = SelectionContext {
                user_input: ctx.user_premise.clone(),
                genre_hint: Some(genre.clone()),
                preferred_genre_profile_ids,
                word_count_target: Some(word_count_target),
                story_progress: "just_started".to_string(),
                has_story: true,
                story_id: Some(ctx.story_id.clone()),
                ..Default::default()
            };

            let strategy = selector
                .select_strategy(&selection_ctx, &assets, Some(&genre_repo), None)
                .await
                .map_err(|e| PipelineError::StepFailed {
                    step_name: self.name().to_string(),
                    reason: format!("策略选择失败: {}", e),
                })?;

            // 保存选择结果到 story 表
            let story_repo = StoryRepository::new(ctx.pool.clone());
            let update_req = UpdateStoryRequest {
                title: None,
                description: None,
                genre: None,
                tone: None,
                pacing: None,
                style_dna_id: strategy.style_dna_ids.first().cloned(),
                genre_profile_id: strategy.genre_profile_id.clone(),
                methodology_id: strategy.methodology_id.clone(),
                methodology_step: None,
                reference_book_id: None,
            };
            if let Err(e) = story_repo.update(&ctx.story_id, &update_req) {
                log::warn!("[GenesisPipeline] 保存策略到 story 表失败: {}", e);
            }

            let strategy_summary = format!(
                "体裁画像: {}, 方法论: {}, 风格 DNA: [{}], 技能: [{}]",
                strategy.genre_profile_id.as_deref().unwrap_or("无"),
                strategy.methodology_id.as_deref().unwrap_or("无"),
                strategy.style_dna_ids.join(", "),
                strategy.skill_ids.join(", ")
            );
            ctx.selected_strategy = Some(strategy);

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 2,
                status: StepStatus::Completed,
                message: format!("已选择创作策略：{}", strategy_summary),
                progress_percent: 60,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 3: 第一章生成 ====================

struct FirstChapterGenerationStep;

impl PipelineStep<GenesisContext> for FirstChapterGenerationStep {
    fn name(&self) -> &'static str {
        "撰写开篇"
    }
    fn description(&self) -> &'static str {
        "生成第一章正文（用户立即可见）"
    }
    fn step_number(&self) -> usize {
        3
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        _llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let meta = {
                let bundle = ctx.bundle.read().await;
                bundle
                    .story_meta
                    .clone()
                    .ok_or_else(|| PipelineError::StepFailed {
                        step_name: self.name().to_string(),
                        reason: "故事概念未生成".to_string(),
                    })?
            };

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 2,
                status: StepStatus::Running,
                message: "正在构建写作指令...".to_string(),
                progress_percent: 60,
                elapsed_seconds: 0,
                metadata: None,
            });

            // v0.22.3: 一次性加载 AppConfig，避免同一函数内原先 3 次 load()；
            // 配合钥匙串内存缓存，大幅减少 macOS 钥匙串访问。
            let app_dir = ctx.app_handle.path().app_data_dir().unwrap_or_default();
            let app_config = crate::config::AppConfig::load(&app_dir).unwrap_or_default();
            log::warn!(
                "[GenesisDiag] FirstChapterGenerationStep: 开始，story_id={}",
                ctx.story_id
            );

            let app_config = crate::config::AppConfig::load(&app_dir)
                .map(|c| c)
                .unwrap_or_default();
            let word_count_target = app_config.genesis_first_chapter_word_count_target;
            let writing_strategy = app_config.writing_strategy.clone();
            let orchestrator_config =
                crate::agents::orchestrator::WorkflowConfig::from_app_config(&app_config);

            // 通过 AgentService 生成第一章
            // v0.23.15: TriShot 模式的预检失败会自动触发 auto-fill 补齐角色，
            log::warn!(
                "[GenesisDiag] FirstChapterGenerationStep: 开始构建 StoryContext story_id={}",
                ctx.story_id
            );
            let builder =
                crate::creative_engine::context_builder::StoryContextBuilder::new(ctx.pool.clone());
            let agent_context = builder
                .build(&ctx.story_id, Some(1), None, None)
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;
            log::warn!(
                "[GenesisDiag] FirstChapterGenerationStep: StoryContext 构建完成，开始生成第一章"
            );

            // 构建策略注解：将模型选择的体裁画像、方法论等注入写作指令
            let strategy_notes = build_strategy_notes(ctx, &meta.genre);
            let quartet_section = build_narrative_quartet(ctx)
                .map(|q| format!("\n\n【中文叙事四元组】\n{}\n", q))
                .unwrap_or_default();

            let service = crate::agents::service::AgentService::new(ctx.app_handle.clone());
            let task = crate::domain::agent_types::AgentTask {
                id: Uuid::new_v4().to_string(),
                agent_type: crate::domain::agent_types::AgentType::Writer,
                context: agent_context,
                input: format!(
                    "请撰写《{}》的第一章开头（目标字数：{}字，允许±15%）。\n\n【故事概念】\n题材：{}\n基调：\
                     {}\n节奏：{}\n简介：{}\n主题：{}\n\n【创作策略】\n{}{}\n\n【写作策略】\n模式：{}\n冲突强度：{}/100\n叙事节奏：{}\nAI自由度：{}\n\n【用户原始要求】\n{}\n\n这是故事的开篇，\
                     需要：\n1. 迅速建立世界观和氛围\n2. 引入主角，展示其性格和目标\n3. \
                     埋下至少一个伏笔\n4. \
                     在第一幕结尾制造一个冲突或悬念\n\n重要：\
                     必须严格遵循用户原始要求中的题材设定，不得偏离。",
                    meta.title,
                    word_count_target,
                    meta.genre,
                    meta.tone,
                    meta.pacing,
                    meta.description,
                    meta.themes.join(", "),
                    strategy_notes,
                    quartet_section,
                    writing_strategy.run_mode,
                    writing_strategy.conflict_level,
                    writing_strategy.pace,
                    writing_strategy.ai_freedom,
                    ctx.user_premise
                ),
                parameters: HashMap::new(),
                tier: None,
            };

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 2,
                status: StepStatus::Running,
                message: "AI正在撰写第一章...".to_string(),
                progress_percent: 75,
                elapsed_seconds: 0,
                metadata: None,
            });

            // v0.23.30: Genesis 始终走 genesis_default（TriShot），不随用户设置变化。
            // 原因：Genesis 是一辈子一次的操作，需要资产选择 + 快速出章，
            // 用户模式设置影响日常续写/改写，不影响创世。
            let genesis_mode = crate::agents::orchestrator::GenerationMode::genesis_default();
            let orchestrator = crate::agents::orchestrator::AgentOrchestrator::new(
                service,
                orchestrator_config,
                ctx.app_handle.clone(),
            );
            let result = match orchestrator.generate(task, genesis_mode).await {
                Ok(workflow_result) => crate::domain::agent_types::AgentResult {
                    content: workflow_result.final_content,
                    score: Some(workflow_result.final_score),
                    suggestions: vec![],
                    request_id: None,
                },
                Err(e) => return Err(PipelineError::LlmError(e.to_string())),
            };

            // [DEBUG] Bug A 关键日志：第一章生成完成，记录实际内容
            if let Some(logger) = ctx
                .app_handle
                .try_state::<std::sync::Arc<crate::workflow_logger::WorkflowLogger>>()
            {
                logger.info(
                    "genesis.first_chapter.generated",
                    "第一章正文生成完成",
                    Some(serde_json::json!({
                        "content_len": result.content.len(),
                        "content_preview": &result.content[..result.content.len().min(120)],
                        "score": result.score,
                    })),
                );
            }

            // 保存到 Chapter（自动补齐可能已创建 chapter_number=1 的 Chapter，需要检查）
            // v0.23.29: spawn_blocking 包裹同步 DB 操作，防连接池满时阻塞 tokio worker。
            let save_content = result.content.clone();
            let save_story_id = ctx.story_id.clone();
            let save_pool = ctx.pool.clone();
            if let Some(logger) = ctx
                .app_handle
                .try_state::<std::sync::Arc<crate::workflow_logger::WorkflowLogger>>()
            {
                logger.info(
                    "genesis.chapter.save.start",
                    "保存第一章到 DB",
                    Some(serde_json::json!({
                        "story_id": save_story_id,
                        "content_len": save_content.len(),
                    })),
                );
            }
            let (chapter_id, chapter_number) = tokio::task::spawn_blocking(move || {
                let chapter_repo = ChapterRepository::new(save_pool);
                let existing = chapter_repo
                    .get_by_story(&save_story_id)
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?
                    .into_iter()
                    .find(|c| c.chapter_number == 1);
                if let Some(ref ch) = existing {
                    chapter_repo
                        .update(
                            &ch.id,
                            Some("第一章".to_string()),
                            None,
                            Some(save_content),
                            None,
                        )
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                    Ok::<_, PipelineError>((ch.id.clone(), ch.chapter_number))
                } else {
                    let ch = chapter_repo
                        .create(crate::db::CreateChapterRequest {
                            story_id: save_story_id,
                            chapter_number: 1,
                            title: Some("第一章".to_string()),
                            outline: None,
                            content: Some(save_content),
                        })
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                    Ok((ch.id, ch.chapter_number))
                }
            })
            .await
            .map_err(|e| PipelineError::StepFailed {
                step_name: "撰写开篇".to_string(),
                reason: format!("章节保存 spawn_blocking 失败: {}", e),
            })??;

            tracing::info!(
                "[FirstChapterGenerationStep] Chapter saved: chapter_id={}, chapter_content_len={}",
                chapter_id,
                result.content.chars().count()
            );

            // v0.22.5: Genesis 完成后主动触发一次 commit/ingest 管线，
            // 让叙事分析、追读力评估、投影写入在第一章就有数据。
            // 在后台任务中执行，避免阻塞 Genesis 完成事件。
            let commit_story_id = ctx.story_id.clone();
            let commit_app_handle = ctx.app_handle.clone();
            let commit_pool = ctx.pool.clone();
            let commit_vector_store = ctx.vector_store.clone();
            let commit_chapter_id = chapter_id.clone();
            let commit_chapter_number = chapter_number;
            let commit_content = result.content.clone();
            tauri::async_runtime::spawn(async move {
                let service = crate::story_system::SceneCommitService::new(commit_pool.clone());
                match service
                    .auto_commit(
                        &commit_story_id,
                        None,
                        Some(&commit_chapter_id),
                        commit_chapter_number,
                        Some(&commit_content),
                        None,
                        Some(commit_app_handle.clone()),
                        Some(commit_vector_store.as_ref()),
                    )
                    .await
                {
                    Ok(()) => {
                        tracing::info!(
                            "[FirstChapterGenerationStep] Genesis 后自动 commit 成功: story_id={}, chapter_number={}",
                            commit_story_id,
                            commit_chapter_number
                        );
                        // commit 成功后触发一次深度洞察（首次 Genesis 强制 interval=1）
                        if crate::task_system::insight_executor::InsightExecutor::should_trigger(
                            &commit_pool,
                            &commit_story_id,
                            commit_chapter_number,
                            1,
                        ) {
                            let executor = crate::task_system::insight_executor::InsightExecutor {
                                pool: commit_pool,
                                app_handle: commit_app_handle,
                            };
                            executor
                                .run_insight(crate::task_system::insight_executor::InsightPayload {
                                    story_id: commit_story_id,
                                    chapter_number: commit_chapter_number,
                                    trend_window: 1,
                                })
                                .await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "[FirstChapterGenerationStep] Genesis 后自动 commit 失败（非阻塞）: {}",
                            e
                        );
                    }
                }
            });

            // 发送 ChapterSwitch 事件
            // [DEBUG] Bug A 关键日志：ChapterSwitch 事件发送时的内容
            if let Some(logger) = ctx
                .app_handle
                .try_state::<std::sync::Arc<crate::workflow_logger::WorkflowLogger>>()
            {
                logger.info(
                    "genesis.chapter_switch.sent",
                    "ChapterSwitch 事件发送到前端",
                    Some(serde_json::json!({
                        "story_id": &ctx.story_id,
                        "chapter_id": &chapter_id,
                        "content_len": result.content.len(),
                        "content_preview": &result.content[..result.content.len().min(120)],
                    })),
                );
            }
            match crate::window::WindowManager::send_to_frontstage(
                &ctx.app_handle,
                crate::window::FrontstageEvent::ChapterSwitch {
                    story_id: ctx.story_id.clone(),
                    chapter_id: chapter_id.clone(),
                    title: "第一章".to_string(),
                    content: Some(result.content.clone()),
                },
            ) {
                Ok(()) => tracing::info!(
                    "[FirstChapterGenerationStep] ChapterSwitch event sent: story_id={}, \
                     chapter_id={}",
                    ctx.story_id,
                    chapter_id
                ),
                Err(e) => tracing::error!(
                    "[FirstChapterGenerationStep] Failed to send ChapterSwitch event: {}",
                    e
                ),
            }

            ctx.first_chapter_content = Some(result.content.clone());
            let content_len = result.content.chars().count();
            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 2,
                status: StepStatus::Completed,
                message: format!("第一章已完成！{}字", content_len),
                progress_percent: 100,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 3: 世界观/大纲/角色并行生成 ====================
/// 原后台阶段的世界观、大纲、角色三步互相独立（均只依赖故事概念），
/// 合并为一个步骤后内部使用 tokio::join! 并行调用 LLM，减少整体等待时间。
struct ParallelWorldOutlineCharacterStep;

impl PipelineStep<GenesisContext> for ParallelWorldOutlineCharacterStep {
    fn name(&self) -> &'static str {
        "构建世界与骨架"
    }
    fn description(&self) -> &'static str {
        "并行生成世界观、故事大纲和主要角色"
    }
    fn step_number(&self) -> usize {
        1
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let meta = {
                let bundle = ctx.bundle.read().await;
                bundle
                    .story_meta
                    .clone()
                    .ok_or_else(|| PipelineError::StepFailed {
                        step_name: self.name().to_string(),
                        reason: "故事概念未生成".to_string(),
                    })?
            };

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Running,
                message: "正在并行构建世界观、大纲与角色...".to_string(),
                progress_percent: 5,
                elapsed_seconds: 0,
                metadata: None,
            });

            let session_id = ctx.session_id.clone();
            let story_id = ctx.story_id.clone();
            let pool = ctx.pool.clone();
            let bundle = ctx.bundle.clone();
            let llm = llm.clone();
            let strategy_notes = build_strategy_notes(ctx, &meta.genre);
            let narrative_quartet = build_narrative_quartet(ctx);

            let world_future = {
                let meta = meta.clone();
                let session_id = session_id.clone();
                let story_id = story_id.clone();
                let pool = pool.clone();
                let llm = llm.clone();
                let progress = progress.clone();
                let strategy_notes = strategy_notes.clone();
                let narrative_quartet = narrative_quartet.clone();
                async move {
                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "构建世界".to_string(),
                        step_number: 1,
                        total_steps: 4,
                        status: StepStatus::Running,
                        message: "正在调用AI生成世界观...".to_string(),
                        progress_percent: 5,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    let prompt = world_building_prompt(
                        PromptMode::Generate,
                        &meta.title,
                        &meta.genre,
                        &meta.description,
                        Some(&strategy_notes),
                        narrative_quartet.as_deref(),
                        Some(&pool),
                    );
                    let pipeline_ctx = LlmPipelineContext {
                        step_name: "构建世界".to_string(),
                        step_number: 1,
                        total_steps: 4,
                        action: "生成世界观设定".to_string(),
                    };
                    let request = RoutingRequest {
                        task: TaskType::WorldBuilding,
                        complexity: Complexity::Medium,
                        budget_priority: Priority::Low,
                        speed_priority: Priority::Low,
                        estimated_input_tokens: 0,
                        constraints: vec![],
                    };
                    let response = llm
                        .generate_for_request_with_context_and_pipeline(
                            request,
                            prompt,
                            Some(2048),
                            Some(0.6),
                            Some("生成世界观设定"),
                            Some(pipeline_ctx),
                        )
                        .await
                        .map_err(|e| PipelineError::LlmError(e.to_string()))?;

                    let content = response.content.trim();
                    let json_str = super::extract_and_sanitize_json(content)
                        .map_err(|e| PipelineError::ParseError(e))?;
                    let wb: WorldBuildingElement = serde_json::from_str(&json_str)
                        .map_err(|e| PipelineError::ParseError(format!("解析世界观失败: {}", e)))?;

                    let repo = WorldBuildingRepository::new(pool.clone());
                    let world_building = repo
                        .create(&story_id, &wb.concept)
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?;

                    let rules: Vec<crate::db::models::WorldRule> = wb
                        .rules
                        .iter()
                        .map(|r| crate::db::models::WorldRule {
                            id: Uuid::new_v4().to_string(),
                            name: r.name.clone(),
                            description: Some(r.description.clone()),
                            rule_type: match r.rule_type.as_str() {
                                "physical" => RuleType::Physical,
                                "magic" => RuleType::Magic,
                                "social" => RuleType::Social,
                                "historical" => RuleType::Historical,
                                "technology" => RuleType::Technology,
                                "biological" => RuleType::Biological,
                                "cultural" => RuleType::Cultural,
                                _ => RuleType::Custom,
                            },
                            importance: r.importance,
                        })
                        .collect();

                    let _ = repo.update(
                        &world_building.id,
                        None,
                        Some(&rules),
                        Some(&wb.history),
                        None,
                    );

                    let element = WorldBuildingElement {
                        id: world_building.id,
                        story_id: story_id.clone(),
                        ..wb
                    };

                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "构建世界".to_string(),
                        step_number: 1,
                        total_steps: 4,
                        status: StepStatus::Completed,
                        message: "世界观设定已生成".to_string(),
                        progress_percent: 15,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    Ok::<WorldBuildingElement, PipelineError>(element)
                }
            };

            let outline_future = {
                let meta = meta.clone();
                let session_id = session_id.clone();
                let story_id = story_id.clone();
                let pool = pool.clone();
                let llm = llm.clone();
                let progress = progress.clone();
                let strategy_notes = strategy_notes.clone();
                let narrative_quartet = narrative_quartet.clone();
                async move {
                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "故事大纲".to_string(),
                        step_number: 1,
                        total_steps: 4,
                        status: StepStatus::Running,
                        message: "正在调用AI设计故事大纲...".to_string(),
                        progress_percent: 20,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    let prompt = outline_prompt(
                        PromptMode::Generate,
                        &meta.title,
                        &meta.genre,
                        &meta.description,
                        Some(&strategy_notes),
                        narrative_quartet.as_deref(),
                        Some(&pool),
                    );
                    let pipeline_ctx = LlmPipelineContext {
                        step_name: "故事大纲".to_string(),
                        step_number: 1,
                        total_steps: 4,
                        action: "生成故事大纲".to_string(),
                    };
                    let request = RoutingRequest {
                        task: TaskType::WorldBuilding,
                        complexity: Complexity::Medium,
                        budget_priority: Priority::Low,
                        speed_priority: Priority::Low,
                        estimated_input_tokens: 0,
                        constraints: vec![],
                    };
                    let response = llm
                        .generate_for_request_with_context_and_pipeline(
                            request,
                            prompt,
                            Some(2048),
                            Some(0.6),
                            Some("生成故事大纲"),
                            Some(pipeline_ctx),
                        )
                        .await
                        .map_err(|e| PipelineError::LlmError(e.to_string()))?;

                    let content = response.content.trim();
                    let json_str = super::extract_and_sanitize_json(content)
                        .map_err(|e| PipelineError::ParseError(e))?;
                    let outline: OutlineElement = serde_json::from_str(&json_str)
                        .map_err(|e| PipelineError::ParseError(format!("解析大纲失败: {}", e)))?;

                    let repo = StoryOutlineRepository::new(pool.clone());
                    let structure_json = serde_json::to_string(&outline.acts)
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                    let content_summary = outline
                        .acts
                        .iter()
                        .map(|a| format!("第{}幕 {}：{}", a.act_number, a.title, a.summary))
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    let total_scenes: i32 = outline.acts.iter().map(|a| a.estimated_scenes).sum();

                    let _ = repo
                        .create(
                            &story_id,
                            &content_summary,
                            Some(&structure_json),
                            outline.acts.len() as i32,
                            Some(total_scenes),
                        )
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?;

                    let element = OutlineElement {
                        id: Uuid::new_v4().to_string(),
                        story_id: story_id.clone(),
                        ..outline
                    };

                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "故事大纲".to_string(),
                        step_number: 1,
                        total_steps: 4,
                        status: StepStatus::Completed,
                        message: "故事大纲已生成".to_string(),
                        progress_percent: 30,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    Ok::<OutlineElement, PipelineError>(element)
                }
            };

            let character_future = {
                let meta = meta.clone();
                let story_id = story_id.clone();
                let pool = pool.clone();
                let bundle = bundle.clone();
                let llm = llm.clone();
                let progress = progress.clone();
                let strategy_notes = strategy_notes.clone();
                let narrative_quartet = narrative_quartet.clone();
                async move {
                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "塑造角色".to_string(),
                        step_number: 1,
                        total_steps: 4,
                        status: StepStatus::Running,
                        message: "正在调用AI设计角色...".to_string(),
                        progress_percent: 35,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    let world = {
                        let b = bundle.read().await;
                        b.world_building
                            .as_ref()
                            .map(|w| w.concept.clone())
                            .unwrap_or_default()
                    };

                    let prompt = character_prompt(
                        PromptMode::Generate,
                        &meta.title,
                        &meta.genre,
                        &world,
                        &meta.description,
                        Some(&strategy_notes),
                        narrative_quartet.as_deref(),
                        Some(&pool),
                    );
                    let pipeline_ctx = LlmPipelineContext {
                        step_name: "塑造角色".to_string(),
                        step_number: 1,
                        total_steps: 4,
                        action: "生成角色".to_string(),
                    };
                    let request = RoutingRequest {
                        task: TaskType::WorldBuilding,
                        complexity: Complexity::Medium,
                        budget_priority: Priority::Low,
                        speed_priority: Priority::Low,
                        estimated_input_tokens: 0,
                        constraints: vec![],
                    };
                    let response = llm
                        .generate_for_request_with_context_and_pipeline(
                            request,
                            prompt,
                            Some(3000),
                            Some(0.7),
                            Some("生成角色"),
                            Some(pipeline_ctx),
                        )
                        .await
                        .map_err(|e| PipelineError::LlmError(e.to_string()))?;

                    let content = response.content.trim();
                    let json_str = super::extract_and_sanitize_json(content)
                        .map_err(|e| PipelineError::ParseError(e))?;

                    #[derive(Debug, Deserialize)]
                    struct CharacterResponse {
                        #[serde(default)]
                        characters: Vec<CharacterElement>,
                    }
                    let char_data: CharacterResponse =
                        serde_json::from_str(&json_str).map_err(|e| {
                            log::warn!("角色 JSON 解析失败: {}\n原始 JSON:\n{}", e, json_str);
                            PipelineError::ParseError(format!("解析角色失败: {}", e))
                        })?;

                    let repo = CharacterRepository::new(pool.clone());
                    let rel_repo = CharacterRelationshipRepository::new(pool.clone());
                    let mut name_to_id: HashMap<String, String> = HashMap::new();
                    let mut generated = Vec::new();

                    for c in char_data.characters {
                        let character = repo
                            .create(CreateCharacterRequest {
                                story_id: story_id.clone(),
                                name: c.name.clone(),
                                background: Some(c.background.clone()),
                                personality: Some(c.personality.clone()),
                                goals: Some(c.goals.clone()),
                                appearance: Some(c.appearance.clone()),
                                gender: Some(c.gender.clone()),
                                age: Some(c.age),
                            })
                            .map_err(|e| PipelineError::StorageError(e.to_string()))?;

                        name_to_id.insert(c.name.clone(), character.id.clone());

                        generated.push(CharacterElement {
                            id: character.id,
                            story_id: story_id.clone(),
                            ..c
                        });
                    }

                    for c in &generated {
                        for rel in &c.relationships {
                            if let (Some(source_id), Some(target_id)) =
                                (name_to_id.get(&c.name), name_to_id.get(&rel.target_name))
                            {
                                let _ = rel_repo.create(
                                    &story_id,
                                    source_id,
                                    target_id,
                                    &rel.relation_type,
                                    rel.description.as_deref(),
                                    None,
                                );
                            }
                        }
                    }

                    let count = generated.len();

                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "塑造角色".to_string(),
                        step_number: 1,
                        total_steps: 4,
                        status: StepStatus::Completed,
                        message: format!("已生成 {} 个角色", count),
                        progress_percent: 50,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    Ok::<Vec<CharacterElement>, PipelineError>(generated)
                }
            };

            let (world_res, outline_res, characters_res) =
                tokio::join!(world_future, outline_future, character_future);

            {
                let mut bundle_guard = bundle.write().await;
                if let Ok(ref wb) = world_res {
                    *bundle_guard = bundle_guard.clone().with_world_building(wb.clone());
                }
                if let Ok(ref outline) = outline_res {
                    *bundle_guard = bundle_guard.clone().with_outline(outline.clone());
                }
                if let Ok(ref characters) = characters_res {
                    for c in characters {
                        *bundle_guard = bundle_guard.clone().add_character(c.clone());
                    }
                }
            }

            // 任一失败都中断整个 pipeline（保持严格语义）
            world_res?;
            outline_res?;
            characters_res?;

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Completed,
                message: "世界观、大纲与角色已并行生成".to_string(),
                progress_percent: 50,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 4: 场景生成 ====================

struct SceneGenerationStep;

impl PipelineStep<GenesisContext> for SceneGenerationStep {
    fn name(&self) -> &'static str {
        "场景规划"
    }
    fn description(&self) -> &'static str {
        "生成核心场景大纲"
    }
    fn step_number(&self) -> usize {
        2
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let (meta, character_names) = {
                let bundle = ctx.bundle.read().await;
                let meta = bundle
                    .story_meta
                    .clone()
                    .ok_or_else(|| PipelineError::StepFailed {
                        step_name: self.name().to_string(),
                        reason: "故事概念未生成".to_string(),
                    })?;
                let character_names = bundle
                    .characters
                    .iter()
                    .map(|c| format!("{}({})", c.name, c.role_type))
                    .collect::<Vec<_>>()
                    .join(", ");
                (meta, character_names)
            };
            let strategy_notes = build_strategy_notes(ctx, &meta.genre);
            let narrative_quartet = build_narrative_quartet(ctx);

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Running,
                message: "正在调用AI设计场景...".to_string(),
                progress_percent: 60,
                elapsed_seconds: 0,
                metadata: None,
            });

            let prompt = scene_prompt(
                PromptMode::Generate,
                &meta.title,
                &meta.genre,
                &character_names,
                &meta.description,
                Some(&strategy_notes),
                narrative_quartet.as_deref(),
                Some(&ctx.pool),
            );
            let pipeline_ctx =
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 6, "生成场景大纲");
            let request = RoutingRequest {
                task: TaskType::WorldBuilding,
                complexity: Complexity::Medium,
                budget_priority: Priority::Low,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            };
            let response = llm
                .generate_for_request_with_context_and_pipeline(
                    request,
                    prompt,
                    Some(3000),
                    Some(0.6),
                    Some("生成场景大纲"),
                    Some(pipeline_ctx),
                )
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            let content = response.content.trim();
            let json_str = super::extract_and_sanitize_json(content)
                .map_err(|e| PipelineError::ParseError(e))?;

            #[derive(Debug, Deserialize)]
            struct SceneResponse {
                scenes: Vec<SceneElement>,
            }
            let scene_data: SceneResponse = serde_json::from_str(&json_str)
                .map_err(|e| PipelineError::ParseError(format!("解析场景失败: {}", e)))?;

            // 保存到数据库
            let repo = SceneRepository::new(ctx.pool.clone());
            let mut generated = Vec::new();

            // 查询已有场景，处理重试或LLM返回重复sequence_number的情况
            let existing_scenes = repo.get_by_story(&ctx.story_id).unwrap_or_default();
            let existing_by_seq: std::collections::HashMap<i32, String> = existing_scenes
                .iter()
                .map(|s| (s.sequence_number, s.id.clone()))
                .collect();
            let mut seen_seqs = std::collections::HashSet::new();

            for s in scene_data.scenes {
                // 跳过LLM返回的重复sequence_number
                if !seen_seqs.insert(s.sequence_number) {
                    log::warn!(
                        "[SceneGenerationStep] 跳过重复 sequence_number={} 的场景: {}",
                        s.sequence_number,
                        s.title
                    );
                    continue;
                }

                let scene = if let Some(existing_id) = existing_by_seq.get(&s.sequence_number) {
                    log::info!(
                        "[SceneGenerationStep] sequence_number={} 已存在，更新场景 {}",
                        s.sequence_number,
                        existing_id
                    );
                    repo.get_by_id(existing_id)
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?
                        .ok_or_else(|| {
                            PipelineError::StorageError(format!(
                                "找不到已存在的场景: {}",
                                existing_id
                            ))
                        })?
                } else {
                    repo.create(&ctx.story_id, s.sequence_number, Some(&s.title))
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?
                };

                let updates = SceneUpdate {
                    title: Some(s.title.clone()),
                    dramatic_goal: Some(s.dramatic_goal.clone()),
                    external_pressure: Some(s.external_pressure.clone()),
                    conflict_type: Some(parse_conflict_type(&s.conflict_type)),
                    characters_present: Some(s.characters_present.clone()),
                    character_conflicts: None,
                    setting_location: Some(s.setting_location.clone()),
                    setting_time: Some(s.setting_time.clone()),
                    setting_atmosphere: None,
                    content: None,
                    previous_scene_id: None,
                    next_scene_id: None,
                    confidence_score: Some(0.8),
                    execution_stage: Some("planning".to_string()),
                    outline_content: Some(s.summary.clone()),
                    draft_content: None,
                    style_blend_override: None,
                    foreshadowing_ids: None,
                };
                let _ = repo.update(&scene.id, &updates);

                generated.push(SceneElement {
                    id: scene.id,
                    story_id: ctx.story_id.clone(),
                    ..s
                });
            }

            let count = generated.len();
            {
                let mut bundle = ctx.bundle.write().await;
                for s in generated {
                    *bundle = bundle.clone().add_scene(s);
                }
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Completed,
                message: format!("已生成 {} 个场景", count),
                progress_percent: 70,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 7: 伏笔生成 ====================

struct ForeshadowingGenerationStep;

impl PipelineStep<GenesisContext> for ForeshadowingGenerationStep {
    fn name(&self) -> &'static str {
        "埋设伏笔"
    }
    fn description(&self) -> &'static str {
        "埋设核心伏笔"
    }
    fn step_number(&self) -> usize {
        3
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let (meta, outline_summary, first_scene_id) = {
                let bundle = ctx.bundle.read().await;
                let meta = bundle
                    .story_meta
                    .clone()
                    .ok_or_else(|| PipelineError::StepFailed {
                        step_name: self.name().to_string(),
                        reason: "故事概念未生成".to_string(),
                    })?;
                let outline_summary = bundle
                    .outline
                    .as_ref()
                    .map(|o| {
                        o.acts
                            .iter()
                            .map(|a| format!("第{}幕 {}：{}", a.act_number, a.title, a.summary))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_else(|| "暂无大纲".to_string());
                let first_scene_id = bundle.scenes.first().map(|s| s.id.clone());
                (meta, outline_summary, first_scene_id)
            };
            let strategy_notes = build_strategy_notes(ctx, &meta.genre);
            let narrative_quartet = build_narrative_quartet(ctx);

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Running,
                message: "正在埋设伏笔...".to_string(),
                progress_percent: 80,
                elapsed_seconds: 0,
                metadata: None,
            });

            let prompt = foreshadowing_prompt(
                PromptMode::Generate,
                &meta.title,
                &meta.genre,
                &outline_summary,
                "",
                Some(&strategy_notes),
                narrative_quartet.as_deref(),
                Some(&ctx.pool),
            );
            let pipeline_ctx = ctx.llm_pipeline_ctx(self.name(), self.step_number(), 6, "生成伏笔");
            let request = RoutingRequest {
                task: TaskType::WorldBuilding,
                complexity: Complexity::Medium,
                budget_priority: Priority::Low,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            };
            let response = llm
                .generate_for_request_with_context_and_pipeline(
                    request,
                    prompt,
                    Some(1024),
                    Some(0.7),
                    Some("生成伏笔"),
                    Some(pipeline_ctx),
                )
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            let content = response.content.trim();
            let json_str = super::extract_and_sanitize_json(content)
                .map_err(|e| PipelineError::ParseError(e))?;

            #[derive(Debug, Deserialize)]
            struct ForeshadowingResponse {
                foreshadowings: Vec<ForeshadowingElement>,
            }
            let fw_data: ForeshadowingResponse = serde_json::from_str(&json_str)
                .map_err(|e| PipelineError::ParseError(format!("解析伏笔失败: {}", e)))?;

            // 保存到数据库
            let tracker =
                crate::creative_engine::foreshadowing::ForeshadowingTracker::new(ctx.pool.clone());
            let mut generated = Vec::new();

            for (idx, fw) in fw_data.foreshadowings.into_iter().enumerate() {
                let setup_scene = if idx == 0 {
                    first_scene_id.as_deref()
                } else {
                    None
                };
                let id = tracker
                    .add_foreshadowing(&ctx.story_id, &fw.content, setup_scene, fw.importance)
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?;

                generated.push(ForeshadowingElement {
                    id,
                    story_id: ctx.story_id.clone(),
                    ..fw
                });
            }

            let count = generated.len();
            {
                let mut bundle = ctx.bundle.write().await;
                for fw in generated {
                    *bundle = bundle.clone().add_foreshadowing(fw);
                }
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Completed,
                message: format!("已埋设 {} 处伏笔", count),
                progress_percent: 85,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 8: 知识图谱生成 ====================

struct KnowledgeGraphGenerationStep;

impl PipelineStep<GenesisContext> for KnowledgeGraphGenerationStep {
    fn name(&self) -> &'static str {
        "知识图谱"
    }
    fn description(&self) -> &'static str {
        "构建知识图谱"
    }
    fn step_number(&self) -> usize {
        4
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        _llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Running,
                message: "正在构建知识图谱...".to_string(),
                progress_percent: 95,
                elapsed_seconds: 0,
                metadata: None,
            });

            let kg_repo = KnowledgeGraphRepository::new(ctx.pool.clone());
            let mut entity_id_map: HashMap<String, String> = HashMap::new();

            let (characters, scenes) = {
                let bundle = ctx.bundle.read().await;
                (bundle.characters.clone(), bundle.scenes.clone())
            };

            // 创建角色实体
            for c in &characters {
                let attrs = serde_json::json!({"role": c.role_type, "personality": c.personality});
                let entity = kg_repo
                    .create_entity(&ctx.story_id, &c.name, "Character", &attrs, None)
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                entity_id_map.insert(format!("char:{}", c.id), entity.id);
            }

            // 创建场景实体
            for s in &scenes {
                let attrs =
                    serde_json::json!({"sequence_number": s.sequence_number, "summary": s.summary});
                let entity = kg_repo
                    .create_entity(&ctx.story_id, &s.title, "Event", &attrs, None)
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                entity_id_map.insert(format!("scene:{}", s.id), entity.id);
            }

            // 创建关系：角色 -> 场景
            for c in &characters {
                for s in &scenes {
                    let scene_text = format!("{} {}", s.title, s.summary);
                    if scene_text.contains(&c.name) {
                        if let (Some(char_entity), Some(scene_entity)) = (
                            entity_id_map.get(&format!("char:{}", c.id)),
                            entity_id_map.get(&format!("scene:{}", s.id)),
                        ) {
                            let _ = kg_repo.create_relation(
                                &ctx.story_id,
                                char_entity,
                                scene_entity,
                                "ParticipatesIn",
                                0.7,
                            );
                        }
                    }
                }
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Completed,
                message: "知识图谱已构建".to_string(),
                progress_percent: 100,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 9: 合同播种 ====================

struct ContractSeedingStep;

impl PipelineStep<GenesisContext> for ContractSeedingStep {
    fn name(&self) -> &'static str {
        "播种故事合同"
    }
    fn description(&self) -> &'static str {
        "根据 Genesis 产出创建 MASTER_SETTING 和 CHAPTER_1 合同"
    }
    fn step_number(&self) -> usize {
        5
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        _llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Running,
                message: "正在为故事建立合同真源...".to_string(),
                progress_percent: 95,
                elapsed_seconds: 0,
                metadata: None,
            });

            if let Err(e) = seed_contracts_from_genesis(ctx).await {
                log::warn!(
                    "[GenesisPipeline] Contract seeding failed (non-blocking): {}",
                    e
                );
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 4,
                status: StepStatus::Completed,
                message: "故事合同已建立".to_string(),
                progress_percent: 100,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

/// 从 Genesis 产物生成 MASTER_SETTING 与 CHAPTER_1 合同。
/// 失败时返回 Err，但调用方已标记为 non-blocking。
async fn seed_contracts_from_genesis(ctx: &GenesisContext) -> Result<(), PipelineError> {
    let (story_meta, world_building, characters, scenes, foreshadowings, genre_profile_id) = {
        let bundle = ctx.bundle.read().await;
        let meta = bundle
            .story_meta
            .clone()
            .ok_or_else(|| PipelineError::StepFailed {
                step_name: "播种故事合同".to_string(),
                reason: "故事概念未生成".to_string(),
            })?;
        let gpid = meta.genre_profile_ids.first().cloned();
        (
            meta,
            bundle.world_building.clone(),
            bundle.characters.clone(),
            bundle.scenes.clone(),
            bundle.foreshadowings.clone(),
            gpid,
        )
    };

    // 加载体裁画像：优先用 genre_profile_id，否则按 genre 名称回退
    let profile = {
        let repo = crate::db::GenreProfileRepository::new(ctx.pool.clone());
        let by_id = if let Some(id) = &genre_profile_id {
            repo.get_by_id(id).ok().flatten()
        } else {
            None
        };
        by_id.or_else(|| repo.get_by_name(&story_meta.genre).ok().flatten())
    };

    let core_tone = profile
        .as_ref()
        .and_then(|p| p.core_tone.clone())
        .unwrap_or_else(|| story_meta.tone.clone());
    let pacing_strategy = profile
        .as_ref()
        .and_then(|p| p.pacing_strategy.clone())
        .unwrap_or_else(|| story_meta.pacing.clone());

    let anti_patterns: Vec<String> = profile
        .as_ref()
        .and_then(|p| p.anti_patterns_json.as_deref())
        .and_then(|json| serde_json::from_str::<Vec<String>>(json).ok())
        .unwrap_or_default();

    let world_rules: Vec<String> = world_building
        .as_ref()
        .map(|wb| {
            wb.rules
                .iter()
                .map(|r| format!("{}: {}", r.name, r.description))
                .collect()
        })
        .unwrap_or_default();

    let engine = StorySystemEngine::new(ctx.pool.clone());

    // 创建 MASTER_SETTING 合同
    engine
        .create_master_setting(
            &ctx.story_id,
            &story_meta.genre,
            &core_tone,
            &pacing_strategy,
            &anti_patterns,
            &world_rules,
        )
        .map_err(|e| PipelineError::StorageError(format!("创建 MASTER_SETTING 合同失败: {}", e)))?;

    // 准备 CHAPTER_1 合同数据
    let first_scene = scenes.first();
    let first_foreshadowing = foreshadowings.first();

    let goal = first_scene
        .map(|s| s.dramatic_goal.clone())
        .unwrap_or_else(|| "建立世界观与主角，引入核心冲突".to_string());

    let mut must_cover_nodes = Vec::new();
    if let Some(scene) = first_scene {
        must_cover_nodes.push(format!("场景：{}", scene.title));
        if !scene.setting_location.is_empty() {
            must_cover_nodes.push(format!("地点：{}", scene.setting_location));
        }
    }
    if let Some(fw) = first_foreshadowing {
        must_cover_nodes.push(format!("伏笔：{}", fw.content));
    }
    for c in characters.iter().take(3) {
        must_cover_nodes.push(format!("角色：{}({})", c.name, c.role_type));
    }

    let mut forbidden_zones = anti_patterns.clone();
    forbidden_zones.extend(world_rules.iter().map(|r| format!("不可违反：{}", r)));

    let time_anchor = first_scene.map(|s| s.setting_time.as_str());
    let chapter_span = first_scene.map(|s| s.setting_location.as_str());

    engine
        .create_chapter_contract(
            &ctx.story_id,
            1,
            &goal,
            &must_cover_nodes,
            &forbidden_zones,
            time_anchor,
            chapter_span,
        )
        .map_err(|e| PipelineError::StorageError(format!("创建 CHAPTER_1 合同失败: {}", e)))?;

    Ok(())
}

// ==================== 辅助函数 ====================

fn parse_conflict_type(s: &str) -> ConflictType {
    match s {
        "man_vs_man" => ConflictType::ManVsMan,
        "man_vs_self" => ConflictType::ManVsSelf,
        "man_vs_society" => ConflictType::ManVsSociety,
        "man_vs_nature" => ConflictType::ManVsNature,
        "man_vs_technology" => ConflictType::ManVsTechnology,
        "man_vs_fate" => ConflictType::ManVsFate,
        "man_vs_supernatural" => ConflictType::ManVsSupernatural,
        "man_vs_time" => ConflictType::ManVsTime,
        "man_vs_morality" => ConflictType::ManVsMorality,
        "man_vs_identity" => ConflictType::ManVsIdentity,
        "faction_vs_faction" => ConflictType::FactionVsFaction,
        _ => ConflictType::ManVsMan,
    }
}

#[cfg(test)]
mod contract_seeding_tests {
    use super::*;

    #[test]
    fn background_steps_include_contract_seeding() {
        let steps = GenesisPipeline::background_steps();
        let names: Vec<&str> = steps.iter().map(|s| s.name()).collect();
        assert!(names.contains(&"播种故事合同"));
        assert_eq!(names.len(), 6);
    }
}
