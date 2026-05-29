//! AnalysisPipeline — 逆向/分析流程
//!
//! 增强版拆书功能，基于统一的 NarrativePipeline 框架。
//! 输入：小说文本（分块后的文本）
//! 输出：NarrativeBundle（包含从文本中提取的全部结构要素）
//!
//! 相比原版拆书，新增：
//! - 伏笔提取（ForeshadowingExtractionStep）
//! - 知识图谱构建（KnowledgeGraphExtractionStep）
//! - 结构化世界观（WorldBuildingExtractionStep 输出 JSON 而非纯文本）

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc,
    },
};

use serde::Deserialize;
use tokio::sync::Semaphore;
// use tauri::AppHandle;
use uuid::Uuid;

use super::{
    elements::*,
    pipeline::*,
    progress::*,
    prompts::{PromptMode, *},
};
use crate::llm::{service::PipelineContext as LlmPipelineContext, LlmService};

// ==================== 文本分块 ====================

#[derive(Debug, Clone)]
pub struct TextChunk {
    pub index: usize,
    pub title: Option<String>,
    pub content: String,
    pub word_count: usize,
}

// ==================== AnalysisContext ====================

/// 分析流水线上下文
pub struct AnalysisContext {
    pub book_id: String,
    pub story_id: String,
    pub chunks: Vec<TextChunk>,
    pub total_word_count: usize,
    pub bundle: NarrativeBundle,
    pub current_step: String,
    pub concurrency: usize,
    pub semaphore: Arc<Semaphore>,
    pub active_requests: Arc<AtomicI32>,
    pub pool: crate::db::DbPool,
}

impl StepContext for AnalysisContext {
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

impl AnalysisContext {
    pub fn new(
        book_id: String,
        story_id: String,
        chunks: Vec<TextChunk>,
        total_word_count: usize,
        pool: crate::db::DbPool,
    ) -> Self {
        let concurrency = 3; // 默认并发数
        Self {
            book_id,
            story_id,
            chunks,
            total_word_count,
            bundle: NarrativeBundle::new(),
            current_step: String::new(),
            concurrency,
            semaphore: Arc::new(Semaphore::new(concurrency)),
            active_requests: Arc::new(AtomicI32::new(0)),
            pool,
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

    fn sample_text(&self, max_chars: usize) -> String {
        let combined: String = self
            .chunks
            .iter()
            .map(|c| c.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n");
        if combined.chars().count() > max_chars {
            combined.chars().take(max_chars).collect()
        } else {
            combined
        }
    }
}

// ==================== AnalysisPipeline 构建器 ====================

pub struct AnalysisPipeline;

impl AnalysisPipeline {
    pub fn steps() -> Vec<Box<dyn PipelineStep<AnalysisContext>>> {
        vec![
            Box::new(MetadataExtractionStep),
            Box::new(WorldBuildingExtractionStep),
            Box::new(CharacterExtractionStep),
            Box::new(SceneExtractionStep),
            Box::new(StoryArcExtractionStep),
            Box::new(ForeshadowingExtractionStep),
            Box::new(KnowledgeGraphExtractionStep),
        ]
    }
}

// ==================== Step 1: 元信息提取 ====================

struct MetadataExtractionStep;

impl PipelineStep<AnalysisContext> for MetadataExtractionStep {
    fn name(&self) -> &'static str {
        "提取元信息"
    }
    fn description(&self) -> &'static str {
        "从文本中提取标题、作者、题材等元信息"
    }
    fn step_number(&self) -> usize {
        1
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut AnalysisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let sample = ctx.sample_text(3000);

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Running,
                message: "正在提取故事元信息...".to_string(),
                progress_percent: 5,
                elapsed_seconds: 0,
                metadata: None,
            });

            let prompt = story_concept_prompt(PromptMode::Extract, &sample);
            let pipeline_ctx =
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 7, "提取元信息");
            let response = llm
                .generate_with_context_and_pipeline(
                    prompt,
                    Some(512),
                    Some(0.3),
                    Some("提取元信息"),
                    Some(pipeline_ctx),
                )
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            let content = response.content.trim();
            let json_str = extract_json(content).map_err(|e| PipelineError::ParseError(e))?;
            let meta: StoryMetaElement = serde_json::from_str(&json_str)
                .map_err(|e| PipelineError::ParseError(format!("解析元信息失败: {}", e)))?;

            ctx.bundle = ctx.bundle.clone().with_story_meta(StoryMetaElement {
                id: ctx.story_id.clone(),
                source: ElementSource::Extracted,
                source_ref_id: Some(ctx.book_id.clone()),
                ..meta
            });

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Completed,
                message: format!(
                    "元信息提取完成：《{}",
                    ctx.bundle.story_meta.as_ref().unwrap().title
                ),
                progress_percent: 10,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 2: 世界观提取 ====================

struct WorldBuildingExtractionStep;

impl PipelineStep<AnalysisContext> for WorldBuildingExtractionStep {
    fn name(&self) -> &'static str {
        "提取世界观"
    }
    fn description(&self) -> &'static str {
        "从文本中提取世界观设定（结构化）"
    }
    fn step_number(&self) -> usize {
        2
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut AnalysisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let sample = if ctx.total_word_count <= 100_000 {
                ctx.sample_text(15000)
            } else {
                // 中长篇：均匀采样
                let sample_size = ctx.chunks.len().min(10);
                let step = ctx.chunks.len() / sample_size.max(1);
                let mut samples = Vec::new();
                for i in 0..sample_size {
                    let idx = i * step;
                    if idx < ctx.chunks.len() {
                        samples.push(
                            ctx.chunks[idx]
                                .content
                                .chars()
                                .take(1500)
                                .collect::<String>(),
                        );
                    }
                }
                samples.join("\n\n---\n\n")
            };

            let meta = ctx.bundle.story_meta.as_ref();
            let title = meta.map(|m| m.title.as_str()).unwrap_or("未知");
            let genre = meta.map(|m| m.genre.as_str()).unwrap_or("未知");

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Running,
                message: "正在提取世界观设定...".to_string(),
                progress_percent: 15,
                elapsed_seconds: 0,
                metadata: None,
            });

            let prompt = world_building_prompt(PromptMode::Extract, title, genre, &sample);
            let pipeline_ctx =
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 7, "提取世界观");
            let response = llm
                .generate_with_context_and_pipeline(
                    prompt,
                    Some(2048),
                    Some(0.5),
                    Some("提取世界观"),
                    Some(pipeline_ctx),
                )
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            let content = response.content.trim();
            let json_str = extract_json(content).map_err(|e| PipelineError::ParseError(e))?;
            let wb: WorldBuildingElement = serde_json::from_str(&json_str)
                .map_err(|e| PipelineError::ParseError(format!("解析世界观失败: {}", e)))?;

            ctx.bundle = ctx
                .bundle
                .clone()
                .with_world_building(WorldBuildingElement {
                    id: Uuid::new_v4().to_string(),
                    story_id: ctx.story_id.clone(),
                    source: ElementSource::Extracted,
                    source_ref_id: Some(ctx.book_id.clone()),
                    ..wb
                });

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Completed,
                message: "世界观设定提取完成".to_string(),
                progress_percent: 25,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 3: 角色提取 ====================

struct CharacterExtractionStep;

impl PipelineStep<AnalysisContext> for CharacterExtractionStep {
    fn name(&self) -> &'static str {
        "提取角色"
    }
    fn description(&self) -> &'static str {
        "从文本中提取所有人物角色"
    }
    fn step_number(&self) -> usize {
        3
    }
    fn estimated_llm_calls(&self) -> usize {
        3 // 逐块并行，可能有多次调用
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut AnalysisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let meta = ctx.bundle.story_meta.as_ref();
            let title = meta.map(|m| m.title.as_str()).unwrap_or("未知");
            let genre = meta.map(|m| m.genre.as_str()).unwrap_or("未知");
            let total = ctx.chunks.len();

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Running,
                message: format!("开始提取角色，共 {} 个文本块...", total),
                progress_percent: 30,
                elapsed_seconds: 0,
                metadata: None,
            });

            let mut character_results: Vec<Vec<CharacterElement>> = Vec::new();

            for (i, chunk) in ctx.chunks.iter().enumerate() {
                let sample = if chunk.content.chars().count() > 4000 {
                    chunk.content.chars().take(4000).collect()
                } else {
                    chunk.content.clone()
                };

                let prompt = character_prompt(PromptMode::Extract, title, genre, "", &sample);
                let pipeline_ctx = ctx.llm_pipeline_ctx(
                    self.name(),
                    self.step_number(),
                    7,
                    &format!("提取角色 ({}/{})", i + 1, total),
                );

                let _permit = ctx
                    .semaphore
                    .acquire()
                    .await
                    .map_err(|e| PipelineError::LlmError(format!("并发控制错误: {}", e)))?;
                ctx.active_requests.fetch_add(1, Ordering::Relaxed);

                let response = llm
                    .generate_with_context_and_pipeline(
                        prompt,
                        Some(1000),
                        Some(0.3),
                        Some(&format!("提取角色 {}/{}", i + 1, total)),
                        Some(pipeline_ctx),
                    )
                    .await;

                ctx.active_requests.fetch_sub(1, Ordering::Relaxed);
                drop(_permit);

                match response {
                    Ok(resp) => {
                        let content = resp.content.trim();
                        if let Ok(json_str) = extract_json(content) {
                            #[derive(Debug, Deserialize)]
                            struct CharacterResponse {
                                characters: Vec<CharacterElement>,
                            }
                            if let Ok(result) = serde_json::from_str::<CharacterResponse>(&json_str)
                            {
                                character_results.push(result.characters);
                            } else {
                                character_results.push(Vec::new());
                            }
                        } else {
                            character_results.push(Vec::new());
                        }
                    }
                    Err(e) => {
                        log::warn!("[AnalysisPipeline] 角色提取块 {} 失败: {}", i, e);
                        character_results.push(Vec::new());
                    }
                }

                let progress_pct = 30 + ((i + 1) * 15 / total.max(1)) as i32;
                progress(PipelineProgressEvent {
                    pipeline_id: ctx.book_id.clone(),
                    pipeline_type: PipelineType::Analysis,
                    step_name: self.name().to_string(),
                    step_number: self.step_number(),
                    total_steps: 7,
                    status: StepStatus::Running,
                    message: format!(
                        "正在提取角色 ({}/{}) — 活跃线程 {}/{}",
                        i + 1,
                        total,
                        ctx.active_requests.load(Ordering::Relaxed),
                        ctx.concurrency
                    ),
                    progress_percent: progress_pct,
                    elapsed_seconds: 0,
                    metadata: None,
                });
            }

            // 合并去重
            let merged = merge_characters(character_results);
            for c in merged {
                ctx.bundle = ctx.bundle.clone().add_character(CharacterElement {
                    id: Uuid::new_v4().to_string(),
                    story_id: ctx.story_id.clone(),
                    source: ElementSource::Extracted,
                    source_ref_id: Some(ctx.book_id.clone()),
                    ..c
                });
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Completed,
                message: format!(
                    "角色提取完成，共识别 {} 个角色",
                    ctx.bundle.characters.len()
                ),
                progress_percent: 45,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 4: 场景提取 ====================

struct SceneExtractionStep;

impl PipelineStep<AnalysisContext> for SceneExtractionStep {
    fn name(&self) -> &'static str {
        "提取场景"
    }
    fn description(&self) -> &'static str {
        "从文本中提取所有场景/章节"
    }
    fn step_number(&self) -> usize {
        4
    }
    fn estimated_llm_calls(&self) -> usize {
        3
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut AnalysisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let meta = ctx.bundle.story_meta.as_ref();
            let title = meta.map(|m| m.title.as_str()).unwrap_or("未知");
            let genre = meta.map(|m| m.genre.as_str()).unwrap_or("未知");
            let total = ctx.chunks.len();

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Running,
                message: format!("开始提取场景，共 {} 个文本块...", total),
                progress_percent: 50,
                elapsed_seconds: 0,
                metadata: None,
            });

            let mut scenes = Vec::new();

            for (i, chunk) in ctx.chunks.iter().enumerate() {
                let sample = if chunk.content.chars().count() > 5000 {
                    chunk.content.chars().take(5000).collect()
                } else {
                    chunk.content.clone()
                };

                let prompt = scene_prompt(PromptMode::Extract, title, genre, "", &sample);
                let pipeline_ctx = ctx.llm_pipeline_ctx(
                    self.name(),
                    self.step_number(),
                    7,
                    &format!("提取场景 ({}/{})", i + 1, total),
                );

                let _permit = ctx
                    .semaphore
                    .acquire()
                    .await
                    .map_err(|e| PipelineError::LlmError(format!("并发控制错误: {}", e)))?;
                ctx.active_requests.fetch_add(1, Ordering::Relaxed);

                let response = llm
                    .generate_with_context_and_pipeline(
                        prompt,
                        Some(1000),
                        Some(0.3),
                        Some(&format!("提取场景 {}/{}", i + 1, total)),
                        Some(pipeline_ctx),
                    )
                    .await;

                ctx.active_requests.fetch_sub(1, Ordering::Relaxed);
                drop(_permit);

                match response {
                    Ok(resp) => {
                        let content = resp.content.trim();
                        if let Ok(json_str) = extract_json(content) {
                            #[derive(Debug, Deserialize)]
                            struct SceneResponse {
                                scenes: Vec<SceneElement>,
                            }
                            if let Ok(result) = serde_json::from_str::<SceneResponse>(&json_str) {
                                for s in result.scenes {
                                    scenes.push(SceneElement {
                                        id: Uuid::new_v4().to_string(),
                                        story_id: ctx.story_id.clone(),
                                        source: ElementSource::Extracted,
                                        source_ref_id: Some(ctx.book_id.clone()),
                                        ..s
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!("[AnalysisPipeline] 场景提取块 {} 失败: {}", i, e);
                    }
                }

                let progress_pct = 50 + ((i + 1) * 10 / total.max(1)) as i32;
                progress(PipelineProgressEvent {
                    pipeline_id: ctx.book_id.clone(),
                    pipeline_type: PipelineType::Analysis,
                    step_name: self.name().to_string(),
                    step_number: self.step_number(),
                    total_steps: 7,
                    status: StepStatus::Running,
                    message: format!(
                        "正在提取场景 ({}/{}) — 已处理 {} 章",
                        i + 1,
                        total,
                        scenes.len()
                    ),
                    progress_percent: progress_pct,
                    elapsed_seconds: 0,
                    metadata: None,
                });
            }

            for s in scenes {
                ctx.bundle = ctx.bundle.clone().add_scene(s);
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Completed,
                message: format!("场景提取完成，共 {} 章", ctx.bundle.scenes.len()),
                progress_percent: 60,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 5: 故事线提取 ====================

struct StoryArcExtractionStep;

impl PipelineStep<AnalysisContext> for StoryArcExtractionStep {
    fn name(&self) -> &'static str {
        "提取故事线"
    }
    fn description(&self) -> &'static str {
        "从场景概要中提取故事线结构"
    }
    fn step_number(&self) -> usize {
        5
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut AnalysisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let meta = ctx.bundle.story_meta.as_ref();
            let title = meta.map(|m| m.title.as_str()).unwrap_or("未知");

            let summaries: Vec<String> = ctx
                .bundle
                .scenes
                .iter()
                .map(|s| format!("第{} {}: {}", s.sequence_number, s.title, s.summary))
                .collect();
            let combined = summaries.join("\n");
            let sample = if combined.chars().count() > 8000 {
                combined.chars().take(8000).collect()
            } else {
                combined
            };

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Running,
                message: "正在提取故事线结构...".to_string(),
                progress_percent: 65,
                elapsed_seconds: 0,
                metadata: None,
            });

            let prompt = story_arc_prompt(PromptMode::Extract, title, &sample);
            let pipeline_ctx =
                ctx.llm_pipeline_ctx(self.name(), self.step_number(), 7, "提取故事线");
            let response = llm
                .generate_with_context_and_pipeline(
                    prompt,
                    Some(1000),
                    Some(0.5),
                    Some("提取故事线"),
                    Some(pipeline_ctx),
                )
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            let content = response.content.trim();
            let json_str = extract_json(content).map_err(|e| PipelineError::ParseError(e))?;

            #[derive(Debug, Deserialize)]
            struct ArcResponse {
                main_arc: String,
                sub_arcs: Vec<String>,
                climaxes: Vec<String>,
                turning_points: Vec<String>,
            }
            let _arc: ArcResponse = serde_json::from_str(&json_str)
                .map_err(|e| PipelineError::ParseError(format!("解析故事线失败: {}", e)))?;

            // 故事线不直接存储为 NarrativeElement，而是作为 Outline 的补充信息
            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Completed,
                message: "故事线提取完成".to_string(),
                progress_percent: 75,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 6: 伏笔提取（新增） ====================

struct ForeshadowingExtractionStep;

impl PipelineStep<AnalysisContext> for ForeshadowingExtractionStep {
    fn name(&self) -> &'static str {
        "提取伏笔"
    }
    fn description(&self) -> &'static str {
        "从文本中提取伏笔线索"
    }
    fn step_number(&self) -> usize {
        6
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut AnalysisContext,
        llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            let meta = ctx.bundle.story_meta.as_ref();
            let title = meta.map(|m| m.title.as_str()).unwrap_or("未知");
            let genre = meta.map(|m| m.genre.as_str()).unwrap_or("未知");
            let sample = ctx.sample_text(8000);

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Running,
                message: "正在提取伏笔线索...".to_string(),
                progress_percent: 80,
                elapsed_seconds: 0,
                metadata: None,
            });

            let prompt = foreshadowing_prompt(PromptMode::Extract, title, genre, "", &sample);
            let pipeline_ctx = ctx.llm_pipeline_ctx(self.name(), self.step_number(), 7, "提取伏笔");
            let response = llm
                .generate_with_context_and_pipeline(
                    prompt,
                    Some(1024),
                    Some(0.7),
                    Some("提取伏笔"),
                    Some(pipeline_ctx),
                )
                .await
                .map_err(|e| PipelineError::LlmError(e.to_string()))?;

            let content = response.content.trim();
            let json_str = extract_json(content).map_err(|e| PipelineError::ParseError(e))?;

            #[derive(Debug, Deserialize)]
            struct ForeshadowingResponse {
                foreshadowings: Vec<ForeshadowingElement>,
            }
            let fw_data: ForeshadowingResponse = serde_json::from_str(&json_str)
                .map_err(|e| PipelineError::ParseError(format!("解析伏笔失败: {}", e)))?;

            for fw in fw_data.foreshadowings {
                ctx.bundle = ctx.bundle.clone().add_foreshadowing(ForeshadowingElement {
                    id: Uuid::new_v4().to_string(),
                    story_id: ctx.story_id.clone(),
                    source: ElementSource::Extracted,
                    source_ref_id: Some(ctx.book_id.clone()),
                    status: ForeshadowingStatus::Setup,
                    ..fw
                });
            }

            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Completed,
                message: format!(
                    "伏笔提取完成，共识别 {} 处伏笔",
                    ctx.bundle.foreshadowings.len()
                ),
                progress_percent: 90,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== Step 7: 知识图谱提取（新增） ====================

struct KnowledgeGraphExtractionStep;

impl PipelineStep<AnalysisContext> for KnowledgeGraphExtractionStep {
    fn name(&self) -> &'static str {
        "构建知识图谱"
    }
    fn description(&self) -> &'static str {
        "从文本中提取实体和关系，构建知识图谱"
    }
    fn step_number(&self) -> usize {
        7
    }

    fn execute<'a>(
        &'a self,
        ctx: &'a mut AnalysisContext,
        _llm: &'a LlmService,
        progress: std::sync::Arc<dyn Fn(PipelineProgressEvent) + Send + Sync>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), PipelineError>> + Send + 'a>>
    {
        Box::pin(async move {
            progress(PipelineProgressEvent {
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Running,
                message: "正在从文本构建知识图谱...".to_string(),
                progress_percent: 92,
                elapsed_seconds: 0,
                metadata: None,
            });
            let kg_repo = crate::db::repositories::KnowledgeGraphRepository::new(ctx.pool.clone());
            let story_id = ctx.story_id.clone();
            let mut entity_id_map: HashMap<String, String> = HashMap::new();

            // 创建角色实体
            for c in &ctx.bundle.characters {
                let attrs = serde_json::json!({
                    "role": c.role_type,
                    "personality": c.personality,
                    "background": c.background,
                });
                match kg_repo.create_entity(&story_id, &c.name, "Character", &attrs, None) {
                    Ok(entity) => {
                        entity_id_map.insert(format!("char:{}", c.id), entity.id);
                    }
                    Err(e) => {
                        log::warn!(
                            "[KnowledgeGraphExtractionStep] Failed to create character entity for \
                             {}: {}",
                            c.name,
                            e
                        );
                    }
                }
            }

            // 创建场景实体
            for s in &ctx.bundle.scenes {
                let attrs = serde_json::json!({
                    "sequence_number": s.sequence_number,
                    "summary": s.summary,
                });
                match kg_repo.create_entity(&story_id, &s.title, "Event", &attrs, None) {
                    Ok(entity) => {
                        entity_id_map.insert(format!("scene:{}", s.id), entity.id);
                    }
                    Err(e) => {
                        log::warn!(
                            "[KnowledgeGraphExtractionStep] Failed to create scene entity for {}: \
                             {}",
                            s.title,
                            e
                        );
                    }
                }
            }

            // 创建伏笔实体
            for (idx, f) in ctx.bundle.foreshadowings.iter().enumerate() {
                let attrs = serde_json::json!({
                    "content": f.content,
                    "importance": f.importance,
                });
                match kg_repo.create_entity(
                    &story_id,
                    &format!("伏笔{}", idx + 1),
                    "PlotDevice",
                    &attrs,
                    None,
                ) {
                    Ok(entity) => {
                        entity_id_map.insert(format!("fw:{}", idx), entity.id);
                    }
                    Err(e) => {
                        log::warn!(
                            "[KnowledgeGraphExtractionStep] Failed to create foreshadowing \
                             entity: {}",
                            e
                        );
                    }
                }
            }

            // 创建关系：角色 -> 场景 (participates_in)
            for c in &ctx.bundle.characters {
                for s in &ctx.bundle.scenes {
                    let scene_text = format!("{} {}", s.title, s.summary);
                    if scene_text.contains(&c.name) {
                        if let (Some(char_entity), Some(scene_entity)) = (
                            entity_id_map.get(&format!("char:{}", c.id)),
                            entity_id_map.get(&format!("scene:{}", s.id)),
                        ) {
                            let _ = kg_repo.create_relation(
                                &story_id,
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
                pipeline_id: ctx.book_id.clone(),
                pipeline_type: PipelineType::Analysis,
                step_name: self.name().to_string(),
                step_number: self.step_number(),
                total_steps: 7,
                status: StepStatus::Completed,
                message: format!("知识图谱构建完成（{} 实体）", entity_id_map.len()),
                progress_percent: 100,
                elapsed_seconds: 0,
                metadata: None,
            });

            Ok(())
        })
    }
}

// ==================== 辅助函数 ====================

fn extract_json(content: &str) -> Result<String, String> {
    super::extract_and_sanitize_json(content)
}

fn merge_characters(results: Vec<Vec<CharacterElement>>) -> Vec<CharacterElement> {
    let mut merged: HashMap<String, CharacterElement> = HashMap::new();
    for batch in results {
        for c in batch {
            if let Some(existing) = merged.get_mut(&c.name) {
                // 合并信息：优先保留更详细的描述
                if existing.personality.len() < c.personality.len() {
                    existing.personality = c.personality;
                }
                if existing.background.len() < c.background.len() {
                    existing.background = c.background;
                }
                existing.importance_score = existing.importance_score.max(c.importance_score);
            } else {
                merged.insert(c.name.clone(), c);
            }
        }
    }
    merged.into_values().collect()
}
