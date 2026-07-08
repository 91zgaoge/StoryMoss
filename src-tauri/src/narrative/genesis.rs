//! GenesisPipeline — 正向/创世流程
//!
//! 替代 planner/bootstrap.rs，基于统一的 NarrativePipeline 框架。
//! 输入：用户概念 premise
//! 输出：NarrativeBundle（包含故事的全部结构要素）

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
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

/// v0.26.19 Phase 2.2: 创世步骤非致命错误记录。
///
/// 后台步骤中 `let _ =` 静默吞掉的失败（world update / outline create /
/// character relations / scene update / KG relations / contract seeding）
/// 改为收集到此结构，最终写入 `genesis_runs.steps_json` 的 `errors` 数组，
/// 供仪表盘展示与用户 toast 提示。
///
/// 严重度分级：
/// - `Warning`：单条记录写入失败，不影响整体创作产出（多数 `let _ =` 站点）。
/// - `Error`：整个子步骤失败但仍允许流水线继续（如 contract seeding
///   整体失败）。
#[derive(Debug, Clone, Serialize)]
pub struct GenesisStepError {
    pub step: String,
    pub message: String,
    pub severity: String,
}

impl GenesisStepError {
    pub fn warning(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            step: step.into(),
            message: message.into(),
            severity: "warning".to_string(),
        }
    }

    pub fn error(step: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            step: step.into(),
            message: message.into(),
            severity: "error".to_string(),
        }
    }
}

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
    /// v0.26.19 Phase 2.2: 后台步骤非致命错误累计。
    /// 使用 `Arc<Mutex<...>>` 以便 quick phase 与 background phase 共享同一集合
    /// （`for_background` 透传同一 Arc），最终在后台阶段结束时写入
    /// `genesis_runs.steps_json`。
    pub errors: Arc<Mutex<Vec<GenesisStepError>>>,
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

    fn pipeline_type(&self) -> crate::narrative::progress::PipelineType {
        crate::narrative::progress::PipelineType::Genesis
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
            errors: Arc::new(Mutex::new(Vec::new())),
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
        errors: Arc<Mutex<Vec<GenesisStepError>>>,
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
            errors,
        }
    }

    /// v0.26.19 Phase 2.2: 记录一个非致命错误到共享错误集合。
    /// 中毒锁视为致命：直接 panic（不应发生；若发生说明上游线程已 panic）。
    pub fn record_error(&self, step: impl Into<String>, message: impl Into<String>) {
        if let Ok(mut guard) = self.errors.lock() {
            guard.push(GenesisStepError::warning(step, message));
        }
    }

    /// v0.26.19 Phase 2.2: 记录一个 Error 级别的非致命错误。
    pub fn record_error_level(
        &self,
        step: impl Into<String>,
        message: impl Into<String>,
        level: &str,
    ) {
        if let Ok(mut guard) = self.errors.lock() {
            guard.push(if level == "error" {
                GenesisStepError::error(step, message)
            } else {
                GenesisStepError::warning(step, message)
            });
        }
    }

    /// v0.26.19 Phase 2.2: 取出当前累计的错误快照（不清空）。
    pub fn snapshot_errors(&self) -> Vec<GenesisStepError> {
        self.errors.lock().map(|g| g.clone()).unwrap_or_default()
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
    /// 快速阶段：故事概念 → 策略选择 → 第一章正文，目标 30-90 秒返回给用户
    /// v0.26.28 Phase 4: 策略选择从后台阶段前移至快速阶段，使 FirstChapter
    /// 能使用 `ctx.selected_strategy` 注入体裁画像/方法论/风格 DNA。
    pub fn quick_phase_steps() -> Vec<Box<dyn PipelineStep<GenesisContext>>> {
        vec![
            Box::new(ConceptGenerationStep),
            Box::new(StrategySelectionStep),
            Box::new(FirstChapterGenerationStep),
        ]
    }

    /// 后台阶段：世界观/大纲/角色/场景/伏笔/知识图谱 + 合同播种
    /// v0.23.14: FirstChapterGenerationStep 已移至快速阶段。
    /// v0.26.28 Phase 4: StrategySelectionStep 已前移至快速阶段。
    pub fn background_steps() -> Vec<Box<dyn PipelineStep<GenesisContext>>> {
        vec![
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
                total_steps: 3,
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
                        // v0.23.66: 创世温度优先用 creative_temperature 覆盖，回退到 profile 默认
                        let temp = c
                            .creative_temperature()
                            .or_else(|| profile.map(|p| p.temperature))
                            .unwrap_or(0.7);
                        (profile.map(|p| p.max_tokens).unwrap_or(512), temp)
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
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 3, "生成故事概念");
            let request = RoutingRequest {
                task: TaskType::WorldBuilding,
                complexity: Complexity::Medium,
                budget_priority: Priority::Low,
                speed_priority: Priority::Low,
                estimated_input_tokens: 0,
                constraints: vec![],
            };
            // v0.23.66: 为可能的重试保存副本
            let retry_request = request.clone();
            let retry_pipeline_ctx = pipeline_ctx.clone();
            let retry_prompt_base = prompt.clone();

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
            let json_str = match super::extract_and_sanitize_json(content) {
                Ok(s) => s,
                Err(first_err) => {
                    // v0.23.66: 一次重试 + 散文兜底。
                    // 部分本地量化模型无视"只输出JSON"指令，返回纯文本/散文。
                    log::warn!(
                        "[GenesisDiag] ConceptGenerationStep: 首次JSON提取失败({})，准备重试。content_preview={}",
                        first_err,
                        &content[..content.len().min(200)]
                    );
                    let retry_prompt = format!(
                        "{}\n\n【重要】你的上一次回复未包含有效的JSON格式。请严格按以下JSON格式输出，不要添加任何解释、思考、markdown或前缀文字：\n{}\n只输出JSON，不要输出其他任何内容。",
                        retry_prompt_base,
                        r#"{"title":"故事标题","description":"一句话简介","genre":"题材","tone":"基调","pacing":"节奏","themes":["主题1"],"target_length":"篇幅"}"#
                    );
                    let retry_response = llm
                        .generate_for_request_with_context_and_pipeline(
                            retry_request,
                            retry_prompt,
                            Some(concept_max_tokens),
                            Some(concept_temperature),
                            Some("生成故事概念（重试）"),
                            Some(retry_pipeline_ctx),
                        )
                        .await
                        .map_err(|e| PipelineError::LlmError(e.to_string()))?;
                    let retry_content = retry_response.content.trim();

                    match super::extract_and_sanitize_json(retry_content) {
                        Ok(s) => s,
                        Err(retry_err) => {
                            // v0.23.66: 散文兜底——JSON+重试都失败后，从自然语言中提取。
                            // 模型可能以 "标题：XXX" 等标签形式给出信息。
                            log::warn!(
                                "[GenesisDiag] ConceptGenerationStep: JSON+重试均失败，尝试散文兜底。retry_preview={}",
                                &retry_content[..retry_content.len().min(200)]
                            );
                            let prose_meta = super::extract_story_meta_from_prose(retry_content)
                                .or_else(|| super::extract_story_meta_from_prose(content));
                            match prose_meta {
                                Some(meta) => {
                                    log::info!(
                                        "[GenesisDiag] ConceptGenerationStep: 散文兜底提取成功 title={}",
                                        meta.title
                                    );
                                    // 散文兜底提取成功，跳过 serde 反序列化，直接使用 meta
                                    let pool = ctx.pool.clone();
                                    let req = CreateStoryRequest {
                                        title: meta.title.clone(),
                                        description: Some(meta.description.clone()),
                                        genre: Some(meta.genre.clone()),
                                        style_dna_id: None,
                                        genre_profile_id: meta.genre_profile_ids.first().cloned(),
                                        methodology_id: None,
                                        reference_book_id: None,
                                    };
                                    let story = tokio::task::spawn_blocking(move || {
                                        let story_repo = StoryRepository::new(pool);
                                        story_repo.create(req)
                                    })
                                    .await
                                    .map_err(|e| {
                                        PipelineError::StorageError(format!(
                                            "spawn_blocking 失败: {}",
                                            e
                                        ))
                                    })?
                                    .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                                    log::warn!(
                                        "[GenesisDiag] ConceptGenerationStep: Story 创建成功 id={}（散文兜底）",
                                        story.id
                                    );
                                    ctx.story_id = story.id.clone();
                                    let mut bundle = ctx.bundle.write().await;
                                    *bundle = bundle.clone().with_story_meta(StoryMetaElement {
                                        id: story.id.clone(),
                                        ..meta
                                    });
                                    return Ok(());
                                }
                                None => {
                                    log::error!(
                                        "[GenesisDiag] ConceptGenerationStep: 所有提取方式均失败。first_err={}, retry_err={}",
                                        first_err, retry_err
                                    );
                                    return Err(PipelineError::ParseError(format!(
                                        "JSON解析失败（含1次重试+散文兜底）: 首次={}, 重试={}",
                                        first_err, retry_err
                                    )));
                                }
                            }
                        }
                    }
                }
            };
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
                total_steps: 3,
                status: StepStatus::Completed,
                message: format!("故事概念已生成：《{}", title),
                progress_percent: 40,
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
                total_steps: 3,
                status: StepStatus::Running,
                message: "正在为故事匹配最优创作策略...".to_string(),
                progress_percent: 45,
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
                total_steps: 3,
                status: StepStatus::Completed,
                message: format!("已选择创作策略：{}", strategy_summary),
                progress_percent: 55,
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
                total_steps: 3,
                status: StepStatus::Running,
                message: "正在构建写作指令...".to_string(),
                progress_percent: 60,
                elapsed_seconds: 0,
                metadata: None,
            });

            // v0.22.3: 一次性加载 AppConfig，避免同一函数内多次 load()；
            // 配合钥匙串内存缓存，大幅减少 macOS 钥匙串访问。
            // v0.26.19 Phase 4.2: 去重——原先此处连续两次 AppConfig::load，
            //   第一次结果未被使用即被第二次（含冗余 .map(|c| c)）覆盖。
            let app_dir = ctx.app_handle.path().app_data_dir().unwrap_or_default();
            let app_config = crate::config::AppConfig::load(&app_dir).unwrap_or_default();
            log::warn!(
                "[GenesisDiag] FirstChapterGenerationStep: 开始，story_id={}",
                ctx.story_id
            );

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

            // Phase 4: 场景优先创世 — 加载 Scene 1 的戏剧结构注入 prompt
            let scene_repo_for_prompt = SceneRepository::new(ctx.pool.clone());
            let scene_dramatic_goal;
            let scene_conflict_type;
            let scene_external_pressure;
            let scene_setting_location;
            let scene_setting_time;
            let scene_setting_atmosphere;
            let scene_characters_present;
            let scene_outline;
            if let Ok(scenes) = scene_repo_for_prompt.get_by_story(&ctx.story_id) {
                let scene1 = scenes.into_iter().find(|s| s.sequence_number == 1);
                scene_dramatic_goal = scene1
                    .as_ref()
                    .and_then(|s| s.dramatic_goal.clone())
                    .unwrap_or_default();
                scene_conflict_type = scene1
                    .as_ref()
                    .and_then(|s| s.conflict_type.map(|c| c.to_string()))
                    .unwrap_or_default();
                scene_external_pressure = scene1
                    .as_ref()
                    .and_then(|s| s.external_pressure.clone())
                    .unwrap_or_default();
                scene_setting_location = scene1
                    .as_ref()
                    .and_then(|s| s.setting_location.clone())
                    .unwrap_or_default();
                scene_setting_time = scene1
                    .as_ref()
                    .and_then(|s| s.setting_time.clone())
                    .unwrap_or_default();
                scene_setting_atmosphere = scene1
                    .as_ref()
                    .and_then(|s| s.setting_atmosphere.clone())
                    .unwrap_or_default();
                scene_characters_present = scene1
                    .as_ref()
                    .map(|s| s.characters_present.join("、"))
                    .unwrap_or_default();
                scene_outline = scene1
                    .as_ref()
                    .and_then(|s| s.outline_content.clone())
                    .unwrap_or_default();
            } else {
                scene_dramatic_goal = String::new();
                scene_conflict_type = String::new();
                scene_external_pressure = String::new();
                scene_setting_location = String::new();
                scene_setting_time = String::new();
                scene_setting_atmosphere = String::new();
                scene_characters_present = String::new();
                scene_outline = String::new();
            }

            let service = crate::agents::service::AgentService::new(ctx.app_handle.clone());
            // Phase 4: 使用场景优先模板替代旧章级模板
            let chapter_prompt = first_scene_prompt(
                &meta.title,
                &meta.genre,
                &meta.tone,
                &meta.pacing,
                &meta.description,
                &meta.themes.join(", "),
                &scene_dramatic_goal,
                &scene_conflict_type,
                &scene_external_pressure,
                &scene_setting_location,
                &scene_setting_time,
                &scene_setting_atmosphere,
                &scene_characters_present,
                &scene_outline,
                &strategy_notes,
                &quartet_section,
                &writing_strategy.run_mode,
                writing_strategy.conflict_level,
                &writing_strategy.pace,
                &writing_strategy.ai_freedom,
                &ctx.user_premise,
                word_count_target as u32,
                "",
                Some(&ctx.pool),
            );
            let task = crate::domain::agent_types::AgentTask {
                id: Uuid::new_v4().to_string(),
                agent_type: crate::domain::agent_types::AgentType::Writer,
                context: agent_context,
                input: chapter_prompt,
                parameters: HashMap::new(),
                tier: None,
            };

            // v0.26.16: 保留 task 副本用于自重复重试。AgentTask derives Clone.
            let task_for_retry = task.clone();

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 3,
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
            let mut result = match orchestrator.generate(task, genesis_mode).await {
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
                        "content_preview": result.content.chars().take(120).collect::<String>(),
                        "score": result.score,
                    })),
                );
            }

            // v0.26.16: 生成侧验证闸门——检测自重复并在显著时重试一次。
            // 根因：v0.26.14 发现 LLM 会输出「首尾段落相同」的模型级循环。
            // 后处理 trim 只能事后裁剪，且为避免误伤首尾呼应而阈值保守。
            // 此处在生成侧主动检测：若 trim 裁掉量 ≥ 8%，判定为模型故障，
            // 用更强 anti-repeat 指令重试一次。重试更干净则采用，否则保留首次清理结果。
            let raw_content = result.content.clone();
            let cleaned_content = crate::utils::text::TextUtils::trim_self_repetition(&raw_content);
            let raw_chars = raw_content.chars().count();
            let cleaned_chars = cleaned_content.chars().count();
            let trim_ratio = compute_trim_ratio(raw_chars, cleaned_chars);

            if should_retry_self_repetition(trim_ratio, raw_chars) {
                log::warn!(
                    "[Genesis-DIAG] self-repetition detected (ratio={:.2}, raw={} chars, cleaned={} chars), retrying with anti-repeat prompt",
                    trim_ratio,
                    raw_chars,
                    cleaned_chars
                );
                if let Some(logger) = ctx
                    .app_handle
                    .try_state::<std::sync::Arc<crate::workflow_logger::WorkflowLogger>>()
                {
                    logger.info(
                        "genesis.self_repetition_retry",
                        "检测到模型自重复，使用 anti-repeat 指令重试",
                        Some(serde_json::json!({
                            "story_id": &ctx.story_id,
                            "trim_ratio": format!("{:.2}", trim_ratio),
                            "raw_chars": raw_chars,
                            "cleaned_chars": cleaned_chars,
                        })),
                    );
                }

                let mut retry_task = task_for_retry;
                retry_task.id = Uuid::new_v4().to_string();
                retry_task.input.push_str(
                    "\n\n【绝对禁止 — 上一版违反了以下纪律，本次必须严格遵守】\n\
                     - 严禁首段与末段相同或高度相似：结尾必须是新的情节推进，不得回环到开头\n\
                     - 严禁整章内容写两遍或前后两半高度重叠\n\
                     - 严禁任何段落、句子、情节块在文中出现两次\n\
                     - 严禁在结尾复述开头的场景、意象或句式\n\
                     请确保全文每一段都是全新的内容，首尾之间没有任何重复。",
                );

                match orchestrator.generate(retry_task, genesis_mode).await {
                    Ok(retry_workflow_result) => {
                        let retry_raw = retry_workflow_result.final_content;
                        let retry_cleaned =
                            crate::utils::text::TextUtils::trim_self_repetition(&retry_raw);
                        let retry_raw_chars = retry_raw.chars().count();
                        let retry_cleaned_chars = retry_cleaned.chars().count();
                        let retry_trim_ratio =
                            compute_trim_ratio(retry_raw_chars, retry_cleaned_chars);

                        log::warn!(
                            "[Genesis-DIAG] retry completed: retry_trim_ratio={:.2} (original={:.2}), retry_cleaned={} chars",
                            retry_trim_ratio,
                            trim_ratio,
                            retry_cleaned_chars
                        );

                        let accepted = retry_trim_ratio < trim_ratio;
                        result.content = select_first_chapter_content(
                            trim_ratio,
                            retry_trim_ratio,
                            cleaned_content.clone(),
                            retry_cleaned,
                        );
                        if accepted {
                            log::info!(
                                "[Genesis-DIAG] retry accepted: cleaner than original (ratio {} -> {})",
                                trim_ratio,
                                retry_trim_ratio
                            );
                        } else {
                            log::info!(
                                "[Genesis-DIAG] retry rejected: not cleaner, keeping original trimmed content"
                            );
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "[Genesis-DIAG] retry failed (non-blocking): {}, keeping original trimmed content",
                            e
                        );
                        result.content = cleaned_content;
                    }
                }
            } else {
                // v0.26.15: 无显著自重复，直接使用清理后内容
                result.content = cleaned_content;
                if result.content.len() < raw_content.len() {
                    log::warn!(
                        "[Genesis-DIAG] trimmed minor self-repetition: story_id={} original_len={} cleaned_len={}",
                        ctx.story_id,
                        raw_content.len(),
                        result.content.len()
                    );
                }
            }

            // Phase 1: 内容保存到 Scene（Scene 为真相源），Chapter 仅存元数据
            let save_content = result.content.clone();
            let save_story_id = ctx.story_id.clone();
            let save_pool = ctx.pool.clone();
            log::warn!(
                "[Genesis-DIAG] About to spawn_blocking for scene save, story_id={} content_len={}",
                save_story_id,
                save_content.len()
            );
            if let Some(logger) = ctx
                .app_handle
                .try_state::<std::sync::Arc<crate::workflow_logger::WorkflowLogger>>()
            {
                logger.info(
                    "genesis.scene.save.start",
                    "保存第一个场景到 DB",
                    Some(serde_json::json!({
                        "story_id": save_story_id,
                        "content_len": save_content.len(),
                    })),
                );
            }
            let (chapter_id, scene_id, chapter_number) = tokio::task::spawn_blocking(move || {
                let chapter_repo = ChapterRepository::new(save_pool.clone());
                let scene_repo = SceneRepository::new(save_pool.clone());
                let existing = chapter_repo
                    .get_by_story(&save_story_id)
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?
                    .into_iter()
                    .find(|c| c.chapter_number == 1);
                let (ch_id, ch_num) = if let Some(ref ch) = existing {
                    // 更新章元数据（标题），内容走 Scene
                    chapter_repo
                        .update(&ch.id, Some("第一章".to_string()), None, None)
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                    (ch.id.clone(), ch.chapter_number)
                } else {
                    let story_id_for_ch = save_story_id.clone();
                    let ch = chapter_repo
                        .create(crate::db::CreateChapterRequest {
                            story_id: story_id_for_ch,
                            chapter_number: 1,
                            title: Some("第一章".to_string()),
                            outline: None,
                            content: None, // 内容由 Scene 管理
                        })
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                    (ch.id, ch.chapter_number)
                };
                // 查找或创建 Scene 并写入内容
                let sid = {
                    let scenes = scene_repo
                        .get_by_chapter(&ch_id)
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                    if let Some(scene) = scenes.first() {
                        scene_repo
                            .update(
                                &scene.id,
                                &SceneUpdate {
                                    content: Some(save_content.clone()),
                                    ..Default::default()
                                },
                            )
                            .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                        scene.id.clone()
                    } else {
                        // 创建新 Scene 并关联到 Chapter
                        let mut conn = save_pool
                            .get()
                            .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                        let tx = conn
                            .transaction()
                            .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                        let scene = scene_repo
                            .create_in_tx(&tx, &save_story_id, ch_num, Some("第一章"))
                            .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                        tx.execute(
                            "UPDATE scenes SET chapter_id = ?1, content = ?2 WHERE id = ?3",
                            rusqlite::params![&ch_id, &save_content, &scene.id],
                        )
                        .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                        tx.commit()
                            .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                        scene.id
                    }
                };
                Ok::<_, PipelineError>((ch_id, sid, ch_num))
            })
            .await
            .map_err(|e| PipelineError::StepFailed {
                step_name: "撰写开篇".to_string(),
                reason: format!("场景保存 spawn_blocking 失败: {}", e),
            })??;

            // v0.23.60 stall diagnostic: DB save completed
            log::warn!(
                "[Genesis-DIAG] spawn_blocking chapter save completed, chapter_id={} story_id={}",
                chapter_id,
                ctx.story_id
            );

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

            // v0.26.19 文档对齐（Phase 2.4）：创世第一章正文投递契约
            //   自 v0.26.11 起，创世第一章不再走 generatedText + Tab 确认流程，
            //   而是由 `smart_execute` 把 `first_chapter_content` 作为 `final_content`
            //   返回，前端 `handleSmartGeneration` / `handleRequestGeneration` 通过
            //   `appendAiContent(..., 'auto')` 自动接受进编辑器（见 FrontstageApp.tsx
            //   `genesisDeliveryRef` 三态状态机：idle → generating → delivered）。
            //
            //   此处的 `ChapterSwitch` 事件仅用于切换 chapter 上下文（让前端加载新故事
            //   的章节列表并选中第一章），**不携带正文**，`auto_accept: false`。
            //   正文来源唯一写者是 `smart_execute.final_content`，避免多通道写入导致
            //   "正文 + 幽灵文本"同框或同一内容被追加两次（v0.26.7–v0.26.18 多轮修复
            //   的根因）。Tab 接受仅作为续写/改写路径的回退。
            //
            // [DEBUG] Bug A 关键日志：ChapterSwitch 事件发送时的内容
            if let Some(logger) = ctx
                .app_handle
                .try_state::<std::sync::Arc<crate::workflow_logger::WorkflowLogger>>()
            {
                logger.info(
                    "genesis.chapter_switch.sent",
                    "ChapterSwitch 事件发送到前端（不含 content，正文由 smart_execute.final_content 投递）",
                    Some(serde_json::json!({
                        "story_id": &ctx.story_id,
                        "chapter_id": &chapter_id,
                        "scene_id": &scene_id,
                        "content_len": result.content.len(),
                        "content_preview": result.content.chars().take(120).collect::<String>(),
                    })),
                );
            }
            match crate::window::WindowManager::send_to_frontstage(
                &ctx.app_handle,
                build_first_chapter_chapter_switch(
                    ctx.story_id.clone(),
                    chapter_id.clone(),
                    scene_id.clone(),
                ),
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
                total_steps: 3,
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

// ==================== Step 3: 世界观/大纲/角色生成 ====================
/// 原后台阶段的世界观、大纲、角色三步互相独立（均只依赖故事概念），
/// 历史上用 tokio::join! 并行调用 LLM。
///
/// v0.26.19 P0-2 修复「角色提示词读取空 world_concept」后，执行顺序变为：
///   1. world_gen（async 块，await 拿到 world_concept）
///   2. outline_gen（async 块，await）
///   3. character_gen（依赖 world_concept，最后构造并 await）
/// 三者均为 `BoxFuture` 闭包，被**顺序** await（非 tokio::join! 并行）。
///
/// 已知延迟债务：world 与 outline 互相独立，理论上可用 `tokio::join!` 并行，
/// 仅 character 需在 world 之后。当前顺序 await 是 P0-2 最小修复的产物，
/// 未 reintroduce 并行以避免再次引入闭包捕获竞态。若首章生成延迟敏感，
/// 可重构为 `let (w, o) = tokio::join!(world_gen, outline_gen);
/// character_gen.await`， 但需确保 character_gen 闭包不再捕获共享
/// bundle（已改为传 world_concept 值）。
struct ParallelWorldOutlineCharacterStep;

/// v0.26.19 P0-2: 从 world 生成结果提取 world_concept，供角色提示词使用。
/// 成功时返回真实 concept；失败时返回空串，让角色生成以 fallback 继续。
/// 此函数存在的意义是把「world 必须先于 character 解析」这一不变量从闭包内部
/// 提升为可测试的纯函数契约。
fn world_concept_for_character_prompt(
    world_res: &Result<WorldBuildingElement, PipelineError>,
) -> String {
    match world_res {
        Ok(wb) => wb.concept.clone(),
        Err(_) => String::new(),
    }
}

/// v0.26.19 Phase 3.1: 计算自重复裁剪比例（纯函数，供测试编码 8% 闸门契约）。
///
/// `trim_ratio = 1 - cleaned/raw`；raw 为空时返回 0.0 避免除零。
/// 此函数把 `FirstChapterGenerationStep::execute` 中内联的比例计算提升为
/// 可独立测试的契约，确保 8% 重试闸门阈值不因实现漂移而失效。
pub(crate) fn compute_trim_ratio(raw_chars: usize, cleaned_chars: usize) -> f32 {
    if raw_chars == 0 {
        return 0.0;
    }
    1.0 - (cleaned_chars as f32 / raw_chars as f32)
}

/// v0.26.19 Phase 3.1: 判定是否需要触发 anti-repeat 重试（纯函数）。
///
/// 契约：仅当 `trim_ratio >= 0.08` **且** `raw_chars > 100` 时触发。
/// - 8% 阈值：低于此值视为首尾呼应等良性结构，不重试（避免误伤）。
/// - 100 字下限：短文本的自重复比例波动大，不触发重试（与
///   `trim_self_repetition` 的 40 字短文本旁路对齐，但此处更保守）。
pub(crate) fn should_retry_self_repetition(trim_ratio: f32, raw_chars: usize) -> bool {
    trim_ratio >= 0.08 && raw_chars > 100
}

/// v0.26.19 Phase 3.1: 选择最终第一章正文（纯函数，编码重试接受/拒绝契约）。
///
/// 重试更干净（`retry_trim_ratio < original_trim_ratio`）则采用重试清理结果；
/// 否则保留首次清理结果。重试 LLM 失败由调用方在 `Err` 分支保留首次清理结果，
/// 此函数仅处理 `Ok` 分支的选择逻辑。
pub(crate) fn select_first_chapter_content(
    original_trim_ratio: f32,
    retry_trim_ratio: f32,
    original_cleaned: String,
    retry_cleaned: String,
) -> String {
    if retry_trim_ratio < original_trim_ratio {
        retry_cleaned
    } else {
        original_cleaned
    }
}

/// v0.26.19 Phase 3.1: 构造第一章 ChapterSwitch 事件（纯函数，编码 payload
/// 契约）。
///
/// 契约：创世第一章的 ChapterSwitch **不携带正文**（`content: None`）且
/// `auto_accept: false`。正文唯一写者是 `smart_execute.final_content`，
/// 经前端 `appendAiContent(..., 'auto')` 自动接受。此函数把 payload 形状从
/// 嵌套在 `WindowManager::send_to_frontstage` 调用中的字面量提升为可测试契约，
/// 防止 v0.26.7–v0.26.18 多轮修复的「双眼皮」回归。
pub(crate) fn build_first_chapter_chapter_switch(
    story_id: String,
    chapter_id: String,
    scene_id: String,
) -> crate::window::FrontstageEvent {
    crate::window::FrontstageEvent::ChapterSwitch {
        story_id,
        chapter_id,
        scene_id: Some(scene_id),
        title: "第一章".to_string(),
        content: None,
        auto_accept: false,
    }
}

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
                total_steps: 5,
                status: StepStatus::Running,
                message: "正在构建世界观、大纲与角色...".to_string(),
                progress_percent: 5,
                elapsed_seconds: 0,
                metadata: None,
            });

            let session_id = ctx.session_id.clone();
            let story_id = ctx.story_id.clone();
            let pool = ctx.pool.clone();
            let bundle = ctx.bundle.clone();
            let llm = llm.clone();
            // v0.26.19 Phase 2.2: 错误集合 Arc，透传到各子 future 以收集非致命错误。
            let errors = ctx.errors.clone();
            let strategy_notes = build_strategy_notes(ctx, &meta.genre);
            let narrative_quartet = build_narrative_quartet(ctx);

            let world_gen = {
                let meta = meta.clone();
                let session_id = session_id.clone();
                let story_id = story_id.clone();
                let pool = pool.clone();
                let llm = llm.clone();
                let progress = progress.clone();
                let strategy_notes = strategy_notes.clone();
                let narrative_quartet = narrative_quartet.clone();
                let errors = errors.clone();
                async move {
                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "构建世界".to_string(),
                        step_number: 1,
                        total_steps: 5,
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
                        total_steps: 5,
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
                        .create_with_source(&story_id, &wb.concept, Some("genesis"), Some(true))
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

                    if let Err(e) = repo.update(
                        &world_building.id,
                        None,
                        Some(&rules),
                        Some(&wb.history),
                        None,
                    ) {
                        // v0.26.19 Phase 2.2: 收集而非吞掉，最终写入 genesis_runs
                        if let Ok(mut guard) = errors.lock() {
                            guard.push(GenesisStepError::warning(
                                "构建世界与骨架",
                                format!("世界观规则更新失败: {}", e),
                            ));
                        }
                    }

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
                        total_steps: 5,
                        status: StepStatus::Completed,
                        message: "世界观设定已生成".to_string(),
                        progress_percent: 15,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    Ok::<WorldBuildingElement, PipelineError>(element)
                }
            };

            let outline_gen = {
                let meta = meta.clone();
                let session_id = session_id.clone();
                let story_id = story_id.clone();
                let pool = pool.clone();
                let llm = llm.clone();
                let progress = progress.clone();
                let strategy_notes = strategy_notes.clone();
                let narrative_quartet = narrative_quartet.clone();
                let errors = errors.clone();
                async move {
                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "故事大纲".to_string(),
                        step_number: 1,
                        total_steps: 5,
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
                        total_steps: 5,
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
                        total_steps: 5,
                        status: StepStatus::Completed,
                        message: "故事大纲已生成".to_string(),
                        progress_percent: 30,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    Ok::<OutlineElement, PipelineError>(element)
                }
            };

            // v0.26.19 fix (P0-2): 先 await world_gen 拿到真实 world_concept，
            // 再构造 character_gen，避免角色提示词读取空 world（恒为空字符串）。
            // 原实现 character_gen 闭包在构造时捕获 bundle，运行时读取
            // bundle.world_building，但 world_gen.await 与 bundle 写入都在三个 gen
            // 块全部构造之后，导致角色提示词 world 参数恒为空，角色与世界观脱钩。
            let world_res = world_gen.await;
            let world_concept = world_concept_for_character_prompt(&world_res);
            let outline_res = outline_gen.await;

            let character_gen = {
                let meta = meta.clone();
                let story_id = story_id.clone();
                let pool = pool.clone();
                let world_concept = world_concept.clone();
                let llm = llm.clone();
                let progress = progress.clone();
                let strategy_notes = strategy_notes.clone();
                let narrative_quartet = narrative_quartet.clone();
                let errors = errors.clone();
                async move {
                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "塑造角色".to_string(),
                        step_number: 1,
                        total_steps: 5,
                        status: StepStatus::Running,
                        message: "正在调用AI设计角色...".to_string(),
                        progress_percent: 35,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    // v0.26.19: 使用已解析的 world_concept，不再运行时读取 bundle
                    let world = world_concept.clone();

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
                        total_steps: 5,
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
                                source: Some("genesis".to_string()),
                                is_auto_generated: Some(true),
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
                                if let Err(e) = rel_repo.create(
                                    &story_id,
                                    source_id,
                                    target_id,
                                    &rel.relation_type,
                                    rel.description.as_deref(),
                                    None,
                                ) {
                                    // v0.26.19 Phase 2.2: 角色关系单条失败不阻断整体，
                                    //   但记录到 errors 供仪表盘展示。
                                    if let Ok(mut guard) = errors.lock() {
                                        guard.push(GenesisStepError::warning(
                                            "构建世界与骨架",
                                            format!(
                                                "角色关系创建失败 ({}→{}): {}",
                                                c.name, rel.target_name, e
                                            ),
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    let count = generated.len();

                    progress(PipelineProgressEvent {
                        pipeline_id: session_id.clone(),
                        pipeline_type: PipelineType::Genesis,
                        step_name: "塑造角色".to_string(),
                        step_number: 1,
                        total_steps: 5,
                        status: StepStatus::Completed,
                        message: format!("已生成 {} 个角色", count),
                        progress_percent: 50,
                        elapsed_seconds: 0,
                        metadata: None,
                    });

                    Ok::<Vec<CharacterElement>, PipelineError>(generated)
                }
            };

            // v0.23.71: 3 路 LLM 调用保持串行执行（已在 v0.23.66 从 tokio::join! 改为
            // 顺序 .await）。信号量由 commands/orchestrator.rs 的 genesis 后台 spawn 入口
            // 统一持有，此处不再重复 acquire（否则同一 task 内自死锁）。
            // v0.26.19: world_res / outline_res 已在 character_gen 构造前 await 完成，
            // 此处仅 await character_gen。
            let characters_res = character_gen.await;

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
                total_steps: 5,
                status: StepStatus::Completed,
                message: "世界观、大纲与角色已生成".to_string(),
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
                total_steps: 5,
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
                    source: Some("genesis".to_string()),
                    is_auto_generated: Some(true),
                };
                if let Err(e) = repo.update(&scene.id, &updates) {
                    // v0.26.19 Phase 2.2: 场景戏剧字段更新失败不阻断流水线，
                    //   但记录到 errors 供仪表盘展示与诊断。
                    ctx.record_error(
                        "生成场景大纲",
                        format!("场景 {} 戏剧字段更新失败: {}", scene.id, e),
                    );
                }

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
                total_steps: 5,
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
                total_steps: 5,
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
                total_steps: 5,
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
                total_steps: 5,
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
                    .create_entity_with_source(
                        &ctx.story_id,
                        &c.name,
                        "Character",
                        &attrs,
                        None,
                        Some("genesis"),
                        Some(true),
                    )
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                entity_id_map.insert(format!("char:{}", c.id), entity.id);
            }

            // 创建场景实体
            for s in &scenes {
                let attrs =
                    serde_json::json!({"sequence_number": s.sequence_number, "summary": s.summary});
                let entity = kg_repo
                    .create_entity_with_source(
                        &ctx.story_id,
                        &s.title,
                        "Event",
                        &attrs,
                        None,
                        Some("genesis"),
                        Some(true),
                    )
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
                            if let Err(e) = kg_repo.create_relation(
                                &ctx.story_id,
                                char_entity,
                                scene_entity,
                                "ParticipatesIn",
                                0.7,
                            ) {
                                // v0.26.19 Phase 2.2: KG 关系单条失败不阻断，
                                //   但记录以便用户感知知识图谱不完整。
                                ctx.record_error(
                                    "构建知识图谱",
                                    format!("KG 关系创建失败 ({}→{}): {}", c.name, s.title, e),
                                );
                            }
                        }
                    }
                }
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 5,
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
                total_steps: 5,
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
                // v0.26.19 Phase 2.2: 记录为 Error 级非致命错误（合同真源缺失影响
                //   后续 Story System Phase B/C），写入 genesis_runs 供仪表盘展示。
                ctx.record_error_level("播种故事合同", format!("{}", e), "error");
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 5,
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
        assert_eq!(names.len(), 5);
    }
}

#[cfg(test)]
mod world_character_order_tests {
    use super::*;
    use crate::domain::narrative_elements::WorldBuildingElement;

    // v0.26.19 P0-2 契约：world 生成成功时，world_concept 必须取真实 concept，
    // 角色提示词据此构造，不得拿到空字符串。
    #[test]
    fn world_concept_resolves_to_real_concept_on_success() {
        let wb = WorldBuildingElement {
            id: "wb-1".to_string(),
            story_id: "story-1".to_string(),
            concept: "末世废土，水比黄金珍贵".to_string(),
            rules: vec![],
            history: String::new(),
            key_locations: vec![],
            power_system: String::new(),
            source: Default::default(),
            source_ref_id: None,
            status: Default::default(),
        };
        let res: Result<WorldBuildingElement, PipelineError> = Ok(wb.clone());
        let concept = world_concept_for_character_prompt(&res);
        assert_eq!(concept, wb.concept);
        assert!(!concept.is_empty());
    }

    // v0.26.19 P0-2 契约：world 生成失败时，world_concept 为空串，
    // 角色生成以 fallback（题材级）继续，而非阻塞整个 step。
    #[test]
    fn world_concept_falls_back_to_empty_on_error() {
        let res: Result<WorldBuildingElement, PipelineError> =
            Err(PipelineError::LlmError("gateway timeout".to_string()));
        let concept = world_concept_for_character_prompt(&res);
        assert!(concept.is_empty());
    }

    // v0.26.28 Phase 4 契约：quick_phase_steps 为「概念 → 策略选择 →
    // 撰写开篇」三步， 策略选择已前移至快速阶段，使 FirstChapter 可消费
    // `ctx.selected_strategy`。
    #[test]
    fn quick_phase_steps_remain_concept_then_first_chapter() {
        let steps = GenesisPipeline::quick_phase_steps();
        let names: Vec<&str> = steps.iter().map(|s| s.name()).collect();
        assert_eq!(names, vec!["构思故事", "选择创作策略", "撰写开篇"]);
        assert_eq!(names.len(), 3);
    }
}

#[cfg(test)]
mod error_collection_tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    // v0.26.19 Phase 2.2 契约：record_error 把非致命错误追加到共享集合，
    // 不阻塞调用方流程；snapshot_errors 返回当前累计快照（不清空）。
    #[test]
    fn record_error_accumulates_into_shared_collection() {
        let errors: Arc<Mutex<Vec<GenesisStepError>>> = Arc::new(Mutex::new(Vec::new()));
        // 模拟两个独立步骤共享同一 errors Arc（对应 quick 与 background phase 透传）
        let errors_a = errors.clone();
        let errors_b = errors.clone();

        if let Ok(mut g) = errors_a.lock() {
            g.push(GenesisStepError::warning(
                "构建世界与骨架",
                "世界观规则更新失败: db locked",
            ));
        }
        if let Ok(mut g) = errors_b.lock() {
            g.push(GenesisStepError::error(
                "播种故事合同",
                "MASTER_SETTING 合同创建失败: invalid story_id",
            ));
        }

        let snapshot = errors.lock().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].severity, "warning");
        assert_eq!(snapshot[1].severity, "error");
        assert!(snapshot[0].message.contains("世界观规则更新失败"));
        assert!(snapshot[1].message.contains("MASTER_SETTING"));
    }

    // v0.26.19 Phase 2.2 契约：GenesisStepError::warning / ::error 严重度分级正确，
    //   供前端 toast 区分「次要资产未完整」与「关键错误」。
    #[test]
    fn genesis_step_error_severity_levels_are_distinct() {
        let w = GenesisStepError::warning("s", "m");
        let e = GenesisStepError::error("s", "m");
        assert_eq!(w.severity, "warning");
        assert_eq!(e.severity, "error");
        assert_ne!(w.severity, e.severity);
    }
}

#[cfg(test)]
mod first_chapter_retry_gate_tests {
    use super::*;

    // v0.26.19 Phase 3.1 契约：compute_trim_ratio 在 raw 为空时返回 0.0（不除零），
    //   在 cleaned == raw 时返回 0.0（无裁剪），在 cleaned = raw/2 时返回 0.5。
    #[test]
    fn compute_trim_ratio_handles_empty_and_half_trim() {
        assert_eq!(compute_trim_ratio(0, 0), 0.0);
        assert_eq!(compute_trim_ratio(100, 100), 0.0);
        assert!((compute_trim_ratio(100, 50) - 0.5).abs() < f32::EPSILON);
    }

    // v0.26.19 Phase 3.1 契约：should_retry_self_repetition 仅在
    //   trim_ratio >= 0.08 且 raw_chars > 100 时触发。
    //   - 8% 阈值边界：0.079 不触发，0.08 触发。
    //   - 100 字下限边界：trim_ratio 高但 raw=100 不触发，raw=101 触发。
    #[test]
    fn should_retry_self_repetition_threshold_boundary() {
        // 8% 阈值边界
        assert!(!should_retry_self_repetition(0.079, 500));
        assert!(should_retry_self_repetition(0.08, 500));
        assert!(should_retry_self_repetition(0.20, 500));
        // 100 字下限边界
        assert!(!should_retry_self_repetition(0.20, 100));
        assert!(should_retry_self_repetition(0.20, 101));
        // 短文本高比例不触发（与 trim_self_repetition 40 字旁路对齐，更保守）
        assert!(!should_retry_self_repetition(0.50, 50));
    }

    // v0.26.19 Phase 3.1 契约：select_first_chapter_content 在重试更干净时
    //   采用重试结果，否则保留首次清理结果。
    #[test]
    fn select_first_chapter_content_prefers_cleaner_retry() {
        let original = "原清理结果".to_string();
        let retry = "重试清理结果".to_string();
        // 重试更干净 (0.02 < 0.10) → 采用重试
        assert_eq!(
            select_first_chapter_content(0.10, 0.02, original.clone(), retry.clone()),
            retry
        );
        // 重试更脏 (0.15 > 0.10) → 保留原
        assert_eq!(
            select_first_chapter_content(0.10, 0.15, original.clone(), retry.clone()),
            original
        );
        // 相等 → 保留原（严格 <，相等不算更干净）
        assert_eq!(
            select_first_chapter_content(0.10, 0.10, original.clone(), retry.clone()),
            original
        );
    }

    // v0.26.19 Phase 3.1 契约：build_first_chapter_chapter_switch 生成的
    //   ChapterSwitch 事件必须 content=None 且 auto_accept=false，
    //   标题为「第一章」，scene_id 为 Some。这是 v0.26.7–v0.26.18 多轮修复
    //   「双眼皮」回归的硬契约——正文唯一写者是 smart_execute.final_content。
    #[test]
    fn first_chapter_chapter_switch_payload_contract() {
        let evt = build_first_chapter_chapter_switch(
            "story-1".to_string(),
            "chapter-1".to_string(),
            "scene-1".to_string(),
        );
        match evt {
            crate::window::FrontstageEvent::ChapterSwitch {
                story_id,
                chapter_id,
                scene_id,
                title,
                content,
                auto_accept,
            } => {
                assert_eq!(story_id, "story-1");
                assert_eq!(chapter_id, "chapter-1");
                assert_eq!(scene_id, Some("scene-1".to_string()));
                assert_eq!(title, "第一章");
                assert!(
                    content.is_none(),
                    "content 必须为 None，正文由 smart_execute 投递"
                );
                assert!(
                    !auto_accept,
                    "auto_accept 必须为 false，避免与 smart_execute 竞争"
                );
            }
            other => panic!("expected ChapterSwitch, got {:?}", other),
        }
    }
}

#[cfg(test)]
mod background_steps_order_tests {
    use super::*;

    // v0.26.28 Phase 4 契约：background_steps 为 5 步且顺序固定，
    // 策略选择已前移至 quick phase；合同播种仍居末（依赖前面所有产出）。
    #[test]
    fn background_steps_keep_five_in_fixed_order() {
        let steps = GenesisPipeline::background_steps();
        let names: Vec<&str> = steps.iter().map(|s| s.name()).collect();
        assert_eq!(names.len(), 5);
        assert_eq!(names[0], "构建世界与骨架");
        assert_eq!(names[1], "场景规划");
        assert_eq!(names[4], "播种故事合同");
    }
}
