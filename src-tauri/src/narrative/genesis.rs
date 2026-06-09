//! GenesisPipeline — 正向/创世流程
//!
//! 替代 planner/bootstrap.rs，基于统一的 NarrativePipeline 框架。
//! 输入：用户概念 premise
//! 输出：NarrativeBundle（包含故事的全部结构要素）

use std::collections::HashMap;

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
        CreateCharacterRequest, CreateStoryRequest, DbPool,
    },
    llm::{service::PipelineContext as LlmPipelineContext, LlmService},
};

// ==================== GenesisContext ====================

/// 创世流水线上下文
///
/// 在流水线执行过程中，各步骤通过此上下文共享数据和状态。
pub struct GenesisContext {
    pub story_id: String,
    pub session_id: String,
    pub user_premise: String,
    pub bundle: NarrativeBundle,
    pub current_step: String,
    pub app_handle: AppHandle,
    pub pool: DbPool,
    /// 第一章正文内容（用于返回给前端）
    pub first_chapter_content: Option<String>,
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
        Self {
            story_id: String::new(),
            session_id: Uuid::new_v4().to_string(),
            user_premise,
            bundle: NarrativeBundle::new(),
            current_step: String::new(),
            app_handle,
            pool,
            first_chapter_content: None,
        }
    }

    /// 创建用于后台阶段的上下文（继承即时阶段的结果）
    pub fn for_background(
        app_handle: AppHandle,
        story_id: String,
        session_id: String,
        user_premise: String,
        bundle: NarrativeBundle,
    ) -> Self {
        let pool = app_handle.state::<DbPool>().inner().clone();
        Self {
            story_id,
            session_id,
            user_premise,
            bundle,
            current_step: String::new(),
            app_handle,
            pool,
            first_chapter_content: None,
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

// ==================== GenesisPipeline 构建器 ====================

pub struct GenesisPipeline;

impl GenesisPipeline {
    pub fn quick_phase_steps() -> Vec<Box<dyn PipelineStep<GenesisContext>>> {
        vec![
            Box::new(ConceptGenerationStep),
            Box::new(FirstChapterGenerationStep),
        ]
    }

    pub fn background_phase_steps() -> Vec<Box<dyn PipelineStep<GenesisContext>>> {
        vec![
            Box::new(WorldBuildingGenerationStep),
            Box::new(OutlineGenerationStep),
            Box::new(CharacterGenerationStep),
            Box::new(SceneGenerationStep),
            Box::new(ForeshadowingGenerationStep),
            Box::new(KnowledgeGraphGenerationStep),
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

            let prompt = story_concept_prompt(PromptMode::Generate, &ctx.user_premise);
            let pipeline_ctx =
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 2, "生成故事概念");
            let response = llm
                .generate_with_context_and_pipeline(
                    prompt,
                    Some(512),
                    Some(0.7),
                    Some("生成故事概念"),
                    Some(pipeline_ctx),
                )
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            let content = response.content.trim();
            let json_str = super::extract_and_sanitize_json(content)
                .map_err(|e| PipelineError::ParseError(e))?;
            let meta: StoryMetaElement = serde_json::from_str(&json_str)
                .map_err(|e| PipelineError::ParseError(format!("解析故事概念失败: {}", e)))?;

            // 创建 Story 记录
            let story_repo = StoryRepository::new(ctx.pool.clone());
            let story = story_repo
                .create(CreateStoryRequest {
                    title: meta.title.clone(),
                    description: Some(meta.description.clone()),
                    genre: Some(meta.genre.clone()),
                    style_dna_id: None,
                })
                .map_err(|e| PipelineError::StorageError(e.to_string()))?;

            ctx.story_id = story.id.clone();
            ctx.bundle = ctx.bundle.clone().with_story_meta(StoryMetaElement {
                id: story.id.clone(),
                ..meta
            });

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 2,
                status: StepStatus::Completed,
                message: format!(
                    "故事概念已生成：《{}",
                    ctx.bundle.story_meta.as_ref().unwrap().title
                ),
                progress_percent: 50,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 2: 第一章生成 ====================

struct FirstChapterGenerationStep;

impl PipelineStep<GenesisContext> for FirstChapterGenerationStep {
    fn name(&self) -> &'static str {
        "撰写开篇"
    }
    fn description(&self) -> &'static str {
        "生成第一章正文（用户立即可见）"
    }
    fn step_number(&self) -> usize {
        2
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        _llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let meta = ctx
                .bundle
                .story_meta
                .as_ref()
                .ok_or_else(|| PipelineError::StepFailed {
                    step_name: self.name().to_string(),
                    reason: "故事概念未生成".to_string(),
                })?;

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

            // 通过 AgentService 生成第一章
            // auto-fill 已支持自动补齐角色和场景，preflight 不会再阻塞
            let builder =
                crate::creative_engine::context_builder::StoryContextBuilder::new(ctx.pool.clone());
            let agent_context = builder
                .build(&ctx.story_id, Some(1), None, None)
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            let service = crate::agents::service::AgentService::new(ctx.app_handle.clone());
            let task = crate::agents::service::AgentTask {
                id: Uuid::new_v4().to_string(),
                agent_type: crate::agents::service::AgentType::Writer,
                context: agent_context,
                input: format!(
                    "请撰写《{}》的第一章开头（1500-2500字）。\n\n【故事概念】\n题材：{}\n基调：\
                     {}\n节奏：{}\n简介：{}\n主题：{}\n\n【用户原始要求】\n{}\n\n这是故事的开篇，\
                     需要：\n1. 迅速建立世界观和氛围\n2. 引入主角，展示其性格和目标\n3. \
                     埋下至少一个伏笔\n4. \
                     在第一幕结尾制造一个冲突或悬念\n\n重要：\
                     必须严格遵循用户原始要求中的题材设定，不得偏离。",
                    meta.title,
                    meta.genre,
                    meta.tone,
                    meta.pacing,
                    meta.description,
                    meta.themes.join(", "),
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

            // W2-B2: Bootstrap 初稿走 Orchestrator Fast 模式（单轮生成，跳过 Inspector）
            let orchestrator = crate::agents::orchestrator::AgentOrchestrator::with_default_config(
                service,
                ctx.app_handle.clone(),
            );
            let result = match orchestrator
                .generate(task, crate::agents::orchestrator::GenerationMode::Fast)
                .await
            {
                Ok(workflow_result) => crate::agents::AgentResult {
                    content: workflow_result.final_content,
                    score: Some(workflow_result.final_score),
                    suggestions: vec![],
                    request_id: None,
                },
                Err(e) => return Err(PipelineError::LlmError(e.to_string())),
            };

            // 保存到 Chapter（自动补齐可能已创建 chapter_number=1 的 Chapter，需要检查）
            let chapter_repo = ChapterRepository::new(ctx.pool.clone());
            let content_len = result.content.chars().count();
            tracing::info!(
                "[FirstChapterGenerationStep] Saving chapter: story_id={}, content_len={}",
                ctx.story_id,
                content_len
            );

            // 检查是否已有 chapter_number=1 的 Chapter（由 auto-fill 创建）
            let existing_chapters = chapter_repo
                .get_by_story(&ctx.story_id)
                .map_err(|e| PipelineError::StorageError(e.to_string()))?;
            let existing_chapter = existing_chapters
                .into_iter()
                .find(|c| c.chapter_number == 1);

            let chapter = if let Some(ch) = existing_chapter {
                tracing::info!(
                    "[FirstChapterGenerationStep] Existing chapter found: chapter_id={}, updating",
                    ch.id
                );
                chapter_repo
                    .update(
                        &ch.id,
                        Some("第一章".to_string()),
                        None,
                        Some(result.content.clone()),
                        Some(content_len as i32),
                    )
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                ch
            } else {
                let ch = chapter_repo
                    .create(crate::db::CreateChapterRequest {
                        story_id: ctx.story_id.clone(),
                        chapter_number: 1,
                        title: Some("第一章".to_string()),
                        outline: None,
                        content: Some(result.content.clone()),
                    })
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                ch
            };

            tracing::info!(
                "[FirstChapterGenerationStep] Chapter saved: chapter_id={}, chapter_content_len={}",
                chapter.id,
                chapter
                    .content
                    .as_ref()
                    .map(|c| c.chars().count())
                    .unwrap_or(0)
            );

            // 发送 ChapterSwitch 事件
            match crate::window::WindowManager::send_to_frontstage(
                &ctx.app_handle,
                crate::window::FrontstageEvent::ChapterSwitch {
                    story_id: ctx.story_id.clone(),
                    chapter_id: chapter.id.clone(),
                    title: "第一章".to_string(),
                    content: Some(result.content.clone()),
                },
            ) {
                Ok(()) => tracing::info!(
                    "[FirstChapterGenerationStep] ChapterSwitch event sent: story_id={}, \
                     chapter_id={}",
                    ctx.story_id,
                    chapter.id
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

// ==================== Step 3: 世界观生成 ====================

struct WorldBuildingGenerationStep;

impl PipelineStep<GenesisContext> for WorldBuildingGenerationStep {
    fn name(&self) -> &'static str {
        "构建世界"
    }
    fn description(&self) -> &'static str {
        "生成世界观设定"
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
            let meta = ctx
                .bundle
                .story_meta
                .as_ref()
                .ok_or_else(|| PipelineError::StepFailed {
                    step_name: self.name().to_string(),
                    reason: "故事概念未生成".to_string(),
                })?;

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
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
            );
            let pipeline_ctx =
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 6, "生成世界观设定");
            let response = llm
                .generate_with_context_and_pipeline(
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

            // 保存到数据库
            let repo = WorldBuildingRepository::new(ctx.pool.clone());
            let world_building = repo
                .create(&ctx.story_id, &wb.concept)
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

            ctx.bundle = ctx
                .bundle
                .clone()
                .with_world_building(WorldBuildingElement {
                    id: world_building.id,
                    story_id: ctx.story_id.clone(),
                    ..wb
                });

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
                status: StepStatus::Completed,
                message: "世界观设定已生成".to_string(),
                progress_percent: 15,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 4: 大纲生成 ====================

struct OutlineGenerationStep;

impl PipelineStep<GenesisContext> for OutlineGenerationStep {
    fn name(&self) -> &'static str {
        "故事大纲"
    }
    fn description(&self) -> &'static str {
        "生成三幕式故事大纲"
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
            let meta = ctx
                .bundle
                .story_meta
                .as_ref()
                .ok_or_else(|| PipelineError::StepFailed {
                    step_name: self.name().to_string(),
                    reason: "故事概念未生成".to_string(),
                })?;

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
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
            );
            let pipeline_ctx =
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 6, "生成故事大纲");
            let response = llm
                .generate_with_context_and_pipeline(
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

            // 保存到数据库
            let repo = StoryOutlineRepository::new(ctx.pool.clone());
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
                    &ctx.story_id,
                    &content_summary,
                    Some(&structure_json),
                    outline.acts.len() as i32,
                    Some(total_scenes),
                )
                .map_err(|e| PipelineError::StorageError(e.to_string()))?;

            ctx.bundle = ctx.bundle.clone().with_outline(OutlineElement {
                id: Uuid::new_v4().to_string(),
                story_id: ctx.story_id.clone(),
                ..outline
            });

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
                status: StepStatus::Completed,
                message: "故事大纲已生成".to_string(),
                progress_percent: 30,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 5: 角色生成 ====================

struct CharacterGenerationStep;

impl PipelineStep<GenesisContext> for CharacterGenerationStep {
    fn name(&self) -> &'static str {
        "塑造角色"
    }
    fn description(&self) -> &'static str {
        "生成主要角色"
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
            let meta = ctx
                .bundle
                .story_meta
                .as_ref()
                .ok_or_else(|| PipelineError::StepFailed {
                    step_name: self.name().to_string(),
                    reason: "故事概念未生成".to_string(),
                })?;
            let world = ctx
                .bundle
                .world_building
                .as_ref()
                .map(|w| w.concept.clone())
                .unwrap_or_default();

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
                status: StepStatus::Running,
                message: "正在调用AI设计角色...".to_string(),
                progress_percent: 35,
                elapsed_seconds: 0,
                metadata: None,
            });

            let prompt = character_prompt(
                PromptMode::Generate,
                &meta.title,
                &meta.genre,
                &world,
                &meta.description,
            );
            let pipeline_ctx = ctx.llm_pipeline_ctx(self.name(), self.step_number(), 6, "生成角色");
            let response = llm
                .generate_with_context_and_pipeline(
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
                characters: Vec<CharacterElement>,
            }
            let char_data: CharacterResponse = serde_json::from_str(&json_str).map_err(|e| {
                log::warn!("角色 JSON 解析失败: {}\n原始 JSON:\n{}", e, json_str);
                PipelineError::ParseError(format!("解析角色失败: {}", e))
            })?;

            // 保存到数据库
            let repo = CharacterRepository::new(ctx.pool.clone());
            let rel_repo = CharacterRelationshipRepository::new(ctx.pool.clone());
            let mut name_to_id: HashMap<String, String> = HashMap::new();
            let mut generated = Vec::new();

            for c in char_data.characters {
                let character = repo
                    .create(CreateCharacterRequest {
                        story_id: ctx.story_id.clone(),
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
                    story_id: ctx.story_id.clone(),
                    ..c
                });
            }

            // 创建角色关系
            for c in &generated {
                for rel in &c.relationships {
                    if let (Some(source_id), Some(target_id)) =
                        (name_to_id.get(&c.name), name_to_id.get(&rel.target_name))
                    {
                        let _ = rel_repo.create(
                            &ctx.story_id,
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
            for c in generated {
                ctx.bundle = ctx.bundle.clone().add_character(c);
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
                status: StepStatus::Completed,
                message: format!("已生成 {} 个角色", count),
                progress_percent: 50,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 6: 场景生成 ====================

struct SceneGenerationStep;

impl PipelineStep<GenesisContext> for SceneGenerationStep {
    fn name(&self) -> &'static str {
        "场景规划"
    }
    fn description(&self) -> &'static str {
        "生成核心场景大纲"
    }
    fn step_number(&self) -> usize {
        4
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let meta = ctx
                .bundle
                .story_meta
                .as_ref()
                .ok_or_else(|| PipelineError::StepFailed {
                    step_name: self.name().to_string(),
                    reason: "故事概念未生成".to_string(),
                })?;
            let character_names = ctx
                .bundle
                .characters
                .iter()
                .map(|c| format!("{}({})", c.name, c.role_type))
                .collect::<Vec<_>>()
                .join(", ");

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
                status: StepStatus::Running,
                message: "正在调用AI设计场景...".to_string(),
                progress_percent: 55,
                elapsed_seconds: 0,
                metadata: None,
            });

            let prompt = scene_prompt(
                PromptMode::Generate,
                &meta.title,
                &meta.genre,
                &character_names,
                &meta.description,
            );
            let pipeline_ctx =
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 6, "生成场景大纲");
            let response = llm
                .generate_with_context_and_pipeline(
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
            for s in generated {
                ctx.bundle = ctx.bundle.clone().add_scene(s);
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
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
        5
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut GenesisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let meta = ctx
                .bundle
                .story_meta
                .as_ref()
                .ok_or_else(|| PipelineError::StepFailed {
                    step_name: self.name().to_string(),
                    reason: "故事概念未生成".to_string(),
                })?;
            let outline_summary = ctx
                .bundle
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
            let first_scene_id = ctx.bundle.scenes.first().map(|s| s.id.as_str());

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
                status: StepStatus::Running,
                message: "正在埋设伏笔...".to_string(),
                progress_percent: 75,
                elapsed_seconds: 0,
                metadata: None,
            });

            let prompt = foreshadowing_prompt(
                PromptMode::Generate,
                &meta.title,
                &meta.genre,
                &outline_summary,
                "",
            );
            let pipeline_ctx = ctx.llm_pipeline_ctx(self.name(), self.step_number(), 6, "生成伏笔");
            let response = llm
                .generate_with_context_and_pipeline(
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
                let setup_scene = if idx == 0 { first_scene_id } else { None };
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
            for fw in generated {
                ctx.bundle = ctx.bundle.clone().add_foreshadowing(fw);
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.session_id.clone(),
                pipeline_type: PipelineType::Genesis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 6,
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
        6
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
                total_steps: 6,
                status: StepStatus::Running,
                message: "正在构建知识图谱...".to_string(),
                progress_percent: 90,
                elapsed_seconds: 0,
                metadata: None,
            });

            let kg_repo = KnowledgeGraphRepository::new(ctx.pool.clone());
            let mut entity_id_map: HashMap<String, String> = HashMap::new();

            // 创建角色实体
            for c in &ctx.bundle.characters {
                let attrs = serde_json::json!({"role": c.role_type, "personality": c.personality});
                let entity = kg_repo
                    .create_entity(&ctx.story_id, &c.name, "Character", &attrs, None)
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                entity_id_map.insert(format!("char:{}", c.id), entity.id);
            }

            // 创建场景实体
            for s in &ctx.bundle.scenes {
                let attrs =
                    serde_json::json!({"sequence_number": s.sequence_number, "summary": s.summary});
                let entity = kg_repo
                    .create_entity(&ctx.story_id, &s.title, "Event", &attrs, None)
                    .map_err(|e| PipelineError::StorageError(e.to_string()))?;
                entity_id_map.insert(format!("scene:{}", s.id), entity.id);
            }

            // 创建关系：角色 -> 场景
            for c in &ctx.bundle.characters {
                for s in &ctx.bundle.scenes {
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
                total_steps: 6,
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
