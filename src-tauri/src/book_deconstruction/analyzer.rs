//! Book Analyzer - LLM 分析编排器
//!
//! 5步分析 Pipeline：元信息 → 世界观 → 人物 → 章节概要 → 故事线

use std::sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    Arc,
};

use chrono::Local;
use serde_json;
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;

use super::{chunker::extract_sample, models::*};
use crate::{db::DbPool, llm::LlmService};

pub struct BookAnalyzer {
    llm_service: LlmService,
    app_handle: AppHandle,
    pool: DbPool,
    semaphore: Arc<Semaphore>,
    /// 当前活跃的 LLM 请求数
    active_requests: Arc<AtomicI32>,
    /// 最大并发数
    concurrency: usize,
    /// 最近一次进度值，用于心跳不覆盖进度
    last_progress: Arc<AtomicI32>,
}

impl BookAnalyzer {
    pub fn new(
        llm_service: LlmService,
        app_handle: AppHandle,
        pool: DbPool,
        concurrency: usize,
    ) -> Self {
        let concurrency = concurrency.max(1).min(100);
        log::info!(
            "[BookAnalyzer] Initialized with concurrency: {}",
            concurrency
        );
        Self {
            llm_service,
            app_handle,
            pool,
            semaphore: Arc::new(Semaphore::new(concurrency)),
            active_requests: Arc::new(AtomicI32::new(0)),
            concurrency,
            last_progress: Arc::new(AtomicI32::new(0)),
        }
    }

    /// 执行完整分析 Pipeline
    ///
    /// `heartbeat_callback`: 可选的心跳回调，每完成一个主要步骤调用一次
    /// `cancel_check`: 可选的取消检查回调，返回 true 表示应取消
    pub async fn analyze(
        &self,
        book_id: &str,
        chunks: &[TextChunk],
        total_word_count: usize,
        heartbeat_callback: Option<Box<dyn Fn() + Send + Sync>>,
        cancel_check: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    ) -> Result<BookAnalysisResult, AnalysisError> {
        log::info!(
            "[BookAnalyzer] Starting analysis: {} chunks, {} words",
            chunks.len(),
            total_word_count
        );

        let check_cancel = || -> Result<(), AnalysisError> {
            if let Some(ref cb) = cancel_check {
                if cb() {
                    return Err(AnalysisError::Cancelled("用户取消分析".to_string()));
                }
            }
            Ok(())
        };

        // 启动后台定时推送：每 500ms  emit 一次活跃线程数
        let book_id_for_heartbeat = book_id.to_string();
        let active_reqs = self.active_requests.clone();
        let app_handle = self.app_handle.clone();
        let concurrency = self.concurrency;
        let last_progress_for_heartbeat = self.last_progress.clone();
        let stop_heartbeat = Arc::new(AtomicBool::new(false));
        let stop_flag = stop_heartbeat.clone();
        let heartbeat_handle = tokio::task::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
            loop {
                interval.tick().await;
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                let active = active_reqs.load(Ordering::Relaxed);
                let last_progress = last_progress_for_heartbeat.load(Ordering::Relaxed);
                let event = BookAnalysisProgressEvent {
                    book_id: book_id_for_heartbeat.clone(),
                    status: "analyzing".to_string(),
                    progress: last_progress, // 保持当前进度，不覆盖
                    current_step: format!("LLM 调用中，活跃线程 {}/{}", active, concurrency),
                    message: Some(format!("活跃线程 {}/{}", active, concurrency)),
                    active_threads: active,
                    total_chunks: concurrency as i32,
                    processed_chunks: 0,
                };
                let _ = app_handle.emit("book-analysis-progress", event);
            }
        });

        // 将分析逻辑放在一个块中，确保无论成功失败都停止心跳
        let analysis_result: Result<BookAnalysisResult, AnalysisError> = async {
            // Step 1: 元信息识别 (5% -> 12%)
            self.emit_progress(book_id, "extracting", 5, "正在准备文本样本...")
                .await;
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;
            let sample_text = extract_sample(
                &chunks
                    .first()
                    .map(|c| c.content.clone())
                    .unwrap_or_default(),
                3000,
            );
            self.emit_progress(book_id, "extracting", 7, "正在调用LLM识别元信息...")
                .await;
            let metadata = self.extract_metadata(&sample_text).await?;
            self.emit_progress(
                book_id,
                "analyzing",
                12,
                &format!(
                    "识别完成：{} / {}",
                    metadata.title.as_deref().unwrap_or("未知"),
                    metadata.genre.as_deref().unwrap_or("未知类型")
                ),
            )
            .await;
            log::info!(
                "[BookAnalyzer] Step {} completed: {} / {}",
                "metadata",
                metadata.title.as_deref().unwrap_or("未知"),
                metadata.genre.as_deref().unwrap_or("未知类型")
            );
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;

            // Step 2: 世界观提取 (15% -> 28%)
            self.emit_progress(book_id, "analyzing", 15, "正在准备世界观分析样本...")
                .await;
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;
            let world_setting = self
                .extract_world_setting(book_id, chunks, total_word_count)
                .await?;
            self.emit_progress(book_id, "analyzing", 28, "世界观设定分析完成")
                .await;
            log::info!(
                "[BookAnalyzer] Step {} completed: {}",
                "world_setting",
                "世界观设定分析完成"
            );
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;

            // Step 3: 人物拆解 (30% -> 52%)
            self.emit_progress(
                book_id,
                "analyzing",
                30,
                &format!("开始拆解人物角色，共 {} 个文本块...", chunks.len()),
            )
            .await;
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;
            let characters = self
                .extract_characters(book_id, chunks, cancel_check.as_deref())
                .await?;
            self.emit_progress(
                book_id,
                "analyzing",
                52,
                &format!("人物拆解完成，共识别 {} 个角色", characters.len()),
            )
            .await;
            log::info!(
                "[BookAnalyzer] Step {} completed: 共识别 {} 个角色",
                "characters",
                characters.len()
            );
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;

            // Step 4: 章节概要 (55% -> 78%)
            self.emit_progress(
                book_id,
                "analyzing",
                55,
                &format!("开始生成章节概要，共 {} 个文本块...", chunks.len()),
            )
            .await;
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;
            let scenes = self
                .extract_scene_summaries(book_id, chunks, cancel_check.as_deref())
                .await?;
            self.emit_progress(
                book_id,
                "analyzing",
                78,
                &format!("章节概要完成，共 {} 章", scenes.len()),
            )
            .await;
            log::info!(
                "[BookAnalyzer] Step {} completed: 共 {} 章",
                "scenes",
                scenes.len()
            );
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;

            // Step 5: 故事线生成 (80% -> 92%)
            self.emit_progress(book_id, "analyzing", 80, "正在准备故事线素材...")
                .await;
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;
            let story_arc = self.extract_story_arc(book_id, &scenes).await?;
            self.emit_progress(
                book_id,
                "analyzing",
                92,
                &format!(
                    "故事线生成完成：主线{}、支线{}条、高潮{}处",
                    if story_arc.main_arc.is_empty() {
                        "待补充"
                    } else {
                        "已提取"
                    },
                    story_arc.sub_arcs.len(),
                    story_arc.climaxes.len()
                ),
            )
            .await;
            log::info!(
                "[BookAnalyzer] Step {} completed: 主线{}、支线{}条、高潮{}处",
                "story_arc",
                if story_arc.main_arc.is_empty() {
                    "待补充"
                } else {
                    "已提取"
                },
                story_arc.sub_arcs.len(),
                story_arc.climaxes.len()
            );
            if let Some(ref cb) = heartbeat_callback {
                cb();
            }
            check_cancel()?;

            // 构建结果
            let book = self.build_book_result(
                book_id,
                metadata,
                world_setting,
                plot_summary_from_scenes(&scenes),
                story_arc,
            )?;

            self.emit_progress(book_id, "analyzing", 100, "分析完成")
                .await;

            Ok(BookAnalysisResult {
                book,
                characters,
                scenes,
            })
        }
        .await;

        // 停止后台心跳推送
        stop_heartbeat.store(true, Ordering::Relaxed);
        let _ = heartbeat_handle.await;

        analysis_result
    }

    // ==================== Step 1: 元信息识别 ====================

    async fn extract_metadata(
        &self,
        sample_text: &str,
    ) -> Result<ExtractedMetadata, AnalysisError> {
        let prompt = format!(
            r#"请分析以下小说开头，提取基本信息。只输出 JSON，不要有任何其他文字。

要求：
1. title: 小说标题（如无法确定则为null）
2. author: 作者名（如无法确定则为null）
3. genre: 主要类型（如：玄幻、都市、穿越、志怪、言情、科幻、武侠、历史等）
4. genre_tags: 类型标签数组（如：["系统流","无敌流","爽文"]）
5. estimated_word_count: 估计总字数（仅数字）

文本样本：
{}

JSON格式：
{{"title":"...","author":"...","genre":"...","genre_tags":["..."],"estimated_word_count":12345}}"#,
            sample_text
        );

        let response = self.call_llm(prompt, Some(500), Some(0.3)).await?;
        let metadata: LlmMetadataResponse = parse_json_response(&response)?;

        Ok(ExtractedMetadata {
            title: metadata.title,
            author: metadata.author,
            genre: metadata.genre,
            genre_tags: metadata.genre_tags.unwrap_or_default(),
            estimated_word_count: metadata.estimated_word_count,
        })
    }

    // ==================== Step 2: 世界观提取 ====================

    async fn extract_world_setting(
        &self,
        book_id: &str,
        chunks: &[TextChunk],
        total_word_count: usize,
    ) -> Result<String, AnalysisError> {
        let sample = if total_word_count <= 100_000 {
            // 短篇：取全文的前30%作为样本
            let combined: String = chunks
                .iter()
                .map(|c| c.content.clone())
                .collect::<Vec<_>>()
                .join("\n\n");
            extract_sample(&combined, 15000)
        } else {
            // 中长篇：均匀采样
            let sample_size = chunks.len().min(10);
            let step = chunks.len() / sample_size.max(1);
            let mut samples = Vec::new();
            for i in 0..sample_size {
                let idx = i * step;
                if idx < chunks.len() {
                    samples.push(extract_sample(&chunks[idx].content, 1500));
                }
            }
            samples.join("\n\n---\n\n")
        };

        self.emit_progress(book_id, "analyzing", 18, "正在调用LLM分析世界观...")
            .await;

        let prompt = format!(
            r#"请基于以下小说文本片段，提取世界观设定。用中文简洁描述，不超过500字。

需要包含：
1. 世界背景（古代/现代/未来/异世界等）
2. 力量体系（如有：修炼体系、魔法系统、科技水平等）
3. 社会结构（国家、门派、势力分布等）
4. 地理环境（主要场景设定）

文本片段：
{}

请直接输出世界观描述文本，不要输出 JSON。"#,
            sample
        );

        let response = self.call_llm(prompt, Some(800), Some(0.5)).await?;
        self.emit_progress(book_id, "analyzing", 24, "正在整理世界观设定...")
            .await;
        Ok(response.trim().to_string())
    }

    // ==================== Step 3: 人物拆解 ====================

    async fn extract_characters(
        &self,
        book_id: &str,
        chunks: &[TextChunk],
        cancel_check: Option<&(dyn Fn() -> bool + Send + Sync)>,
    ) -> Result<Vec<ReferenceCharacter>, AnalysisError> {
        // 对每个 chunk 并行提取人物
        let mut character_results: Vec<Vec<ExtractedCharacter>> = Vec::new();
        let total = chunks.len();

        for (i, chunk) in chunks.iter().enumerate() {
            // 检查取消
            if let Some(ref cb) = cancel_check {
                if cb() {
                    return Err(AnalysisError::Cancelled("用户取消分析".to_string()));
                }
            }

            let prompt = format!(
                r#"请分析以下小说章节，提取所有出现的人物角色。只输出 JSON，不要有任何其他文字。

要求：
1. name: 人物姓名
2. role_type: 角色定位（主角/反派/配角/龙套）
3. personality: 性格特征（简要描述）
4. appearance: 外貌描写（如有）
5. relationships: 与其他人物的关系数组 [{{"target":"姓名","type":"关系类型","description":"描述"}}]

注意：只提取本章出现的人物。如某人仅被提及但未出场，标记为"提及"。

章节内容：
{}

JSON格式：
{{"characters":[{{"name":"...","role_type":"...","personality":"...","appearance":"...","relationships":[]}}]}}"#,
                extract_sample(&chunk.content, 4000)
            );

            let response = self
                .call_llm_with_semaphore(prompt, Some(1000), Some(0.3))
                .await?;
            let parsed: Result<LlmCharacterResponse, _> = parse_json_response(&response);

            match parsed {
                Ok(result) => {
                    let extracted: Vec<ExtractedCharacter> = result
                        .characters
                        .into_iter()
                        .map(|c| ExtractedCharacter {
                            name: c.name,
                            role_type: c.role_type,
                            personality: c.personality,
                            appearance: c.appearance,
                            relationships: c
                                .relationships
                                .unwrap_or_default()
                                .into_iter()
                                .map(|r| CharacterRelationship {
                                    target_name: r.target,
                                    relation_type: r.relation_type,
                                    description: r.description,
                                })
                                .collect(),
                        })
                        .collect();
                    character_results.push(extracted);
                }
                Err(e) => {
                    log::warn!(
                        "[BookAnalyzer] Failed to parse characters for chunk {}: {}",
                        i,
                        e
                    );
                    character_results.push(Vec::new());
                }
            }

            // 发送进度
            let progress = 30 + ((i + 1) * 20 / total.max(1)) as i32;
            self.emit_progress(
                book_id,
                "analyzing",
                progress,
                &format!("正在拆解人物 ({}/{})", i + 1, total),
            )
            .await;
        }

        // 合并去重（按姓名）
        let merged = merge_characters(character_results);
        log::info!("[BookAnalyzer] Merged {} unique characters", merged.len());

        // 转换为 ReferenceCharacter
        let now = Local::now();
        let characters: Vec<ReferenceCharacter> = merged
            .into_iter()
            .enumerate()
            .map(|(i, c)| {
                let importance = calculate_importance(&c);
                ReferenceCharacter {
                    id: format!("{}_char_{}", book_id, i),
                    book_id: book_id.to_string(),
                    name: c.name,
                    role_type: c.role_type,
                    personality: c.personality,
                    appearance: c.appearance,
                    relationships: Some(
                        serde_json::to_string(&c.relationships).unwrap_or_default(),
                    ),
                    key_scenes: None,
                    importance_score: Some(importance),
                    created_at: now,
                }
            })
            .collect();

        Ok(characters)
    }

    // ==================== Step 4: 章节概要 ====================

    async fn extract_scene_summaries(
        &self,
        book_id: &str,
        chunks: &[TextChunk],
        cancel_check: Option<&(dyn Fn() -> bool + Send + Sync)>,
    ) -> Result<Vec<ReferenceScene>, AnalysisError> {
        let mut scenes = Vec::new();
        let total = chunks.len();
        let now = Local::now();

        for (i, chunk) in chunks.iter().enumerate() {
            // 检查取消
            if let Some(ref cb) = cancel_check {
                if cb() {
                    return Err(AnalysisError::Cancelled("用户取消分析".to_string()));
                }
            }

            let prompt = format!(
                r#"请总结以下小说章节的内容。只输出 JSON，不要有任何其他文字。

要求：
1. summary: 章节内容概要（100-200字）
2. characters_present: 出场人物名称数组
3. key_events: 关键事件数组
4. conflict_type: 冲突类型（如有：人与人/人与自我/人与社会/人与自然/人与命运）
5. emotional_tone: 情感基调（如：紧张/温馨/悲伤/激昂/悬疑）

章节内容：
{}

JSON格式：
{{"summary":"...","characters_present":["..."],"key_events":["..."],"conflict_type":"...","emotional_tone":"..."}}"#,
                extract_sample(&chunk.content, 5000)
            );

            self.emit_progress(
                book_id,
                "analyzing",
                57 + (i * 19 / total.max(1)) as i32,
                &format!("正在调用LLM总结第 {} 章...", i + 1),
            )
            .await;

            let response = self
                .call_llm_with_semaphore(prompt, Some(800), Some(0.3))
                .await?;
            let parsed: Result<LlmSceneSummaryResponse, _> = parse_json_response(&response);

            let scene = match parsed {
                Ok(result) => ReferenceScene {
                    id: format!("{}_scene_{}", book_id, i),
                    book_id: book_id.to_string(),
                    sequence_number: (i + 1) as i32,
                    title: chunk.title.clone(),
                    summary: Some(result.summary),
                    characters_present: Some(
                        serde_json::to_string(&result.characters_present).unwrap_or_default(),
                    ),
                    key_events: Some(serde_json::to_string(&result.key_events).unwrap_or_default()),
                    narrative_intensity: result.conflict_type.as_ref().map(|ct| {
                        crate::narrative::intensity_mapper::conflict_type_to_intensity(ct)
                    }),
                    narrative_sentiment: result.emotional_tone.as_ref().map(|et| {
                        crate::narrative::intensity_mapper::emotional_tone_to_sentiment(et)
                    }),
                    conflict_type: result.conflict_type,
                    emotional_tone: result.emotional_tone,
                    narrative_event_types: None,
                    act_number: None,
                    position_in_act: None,
                    created_at: now,
                },
                Err(e) => {
                    log::warn!(
                        "[BookAnalyzer] Failed to parse scene summary for chunk {}: {}",
                        i,
                        e
                    );
                    ReferenceScene {
                        id: format!("{}_scene_{}", book_id, i),
                        book_id: book_id.to_string(),
                        sequence_number: (i + 1) as i32,
                        title: chunk.title.clone(),
                        summary: Some("（解析失败）".to_string()),
                        characters_present: None,
                        key_events: None,
                        conflict_type: None,
                        emotional_tone: None,
                        narrative_intensity: None,
                        narrative_sentiment: None,
                        narrative_event_types: None,
                        act_number: None,
                        position_in_act: None,
                        created_at: now,
                    }
                }
            };

            scenes.push(scene);

            // 发送进度
            let progress = 59 + ((i + 1) * 17 / total.max(1)) as i32;
            self.emit_progress(
                book_id,
                "analyzing",
                progress,
                &format!(
                    "正在生成章节概要 ({}/{}) — 已处理 {} 章",
                    i + 1,
                    total,
                    scenes.len()
                ),
            )
            .await;
        }

        Ok(scenes)
    }

    // ==================== Step 5: 故事线生成 ====================

    async fn extract_story_arc(
        &self,
        book_id: &str,
        scenes: &[ReferenceScene],
    ) -> Result<ExtractedStoryArc, AnalysisError> {
        // 构建章节概要文本
        let summaries: Vec<String> = scenes
            .iter()
            .map(|s| {
                format!(
                    "第{}章 {}: {}",
                    s.sequence_number,
                    s.title.as_deref().unwrap_or(""),
                    s.summary.as_deref().unwrap_or("无概要")
                )
            })
            .collect();

        let combined = summaries.join("\n");
        let sample = extract_sample(&combined, 8000);

        let prompt = format!(
            r#"请基于以下章节概要，生成完整的故事线。只输出 JSON，不要有任何其他文字。

要求：
1. main_arc: 主线故事（简要概括）
2. sub_arcs: 重要支线数组
3. climaxes: 高潮点数组
4. turning_points: 转折点数组

章节概要：
{}

JSON格式：
{{"main_arc":"...","sub_arcs":["..."],"climaxes":["..."],"turning_points":["..."]}}"#,
            sample
        );

        self.emit_progress(book_id, "analyzing", 82, "正在调用LLM生成故事线...")
            .await;
        let response = self.call_llm(prompt, Some(1000), Some(0.5)).await?;
        self.emit_progress(book_id, "analyzing", 88, "正在解析故事线结构...")
            .await;
        let arc: LlmStoryArcResponse = parse_json_response(&response)?;

        Ok(ExtractedStoryArc {
            main_arc: arc.main_arc,
            sub_arcs: arc.sub_arcs,
            climaxes: arc.climaxes,
            turning_points: arc.turning_points,
        })
    }

    // ==================== 辅助方法 ====================

    async fn call_llm(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<String, AnalysisError> {
        match self
            .llm_service
            .generate(prompt, max_tokens, temperature)
            .await
        {
            Ok(response) => Ok(response.content),
            Err(e) => Err(AnalysisError::LlmError(e.to_string())),
        }
    }

    async fn call_llm_with_semaphore(
        &self,
        prompt: String,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<String, AnalysisError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| AnalysisError::LlmError(format!("Semaphore error: {}", e)));
        self.active_requests.fetch_add(1, Ordering::Relaxed);
        let result = self.call_llm(prompt, max_tokens, temperature).await;
        if let Err(ref e) = result {
            log::error!("[BookAnalyzer] LLM call failed: {}", e);
        }
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
        result
    }

    async fn emit_progress(&self, book_id: &str, status: &str, progress: i32, message: &str) {
        let active = self.active_requests.load(Ordering::Relaxed);
        let _max = self.semaphore.available_permits() as i32 + active;
        let event = BookAnalysisProgressEvent {
            book_id: book_id.to_string(),
            status: status.to_string(),
            progress,
            current_step: message.to_string(),
            message: Some(message.to_string()),
            active_threads: active,
            total_chunks: self.concurrency as i32,
            processed_chunks: 0,
        };
        let _ = self.app_handle.emit("book-analysis-progress", event);
    }

    fn build_book_result(
        &self,
        book_id: &str,
        metadata: ExtractedMetadata,
        world_setting: String,
        plot_summary: String,
        story_arc: ExtractedStoryArc,
    ) -> Result<ReferenceBook, AnalysisError> {
        let story_arc_json = serde_json::to_string(&story_arc)
            .map_err(|e| AnalysisError::StorageError(format!("JSON serialize error: {}", e)))?;

        Ok(ReferenceBook {
            id: book_id.to_string(),
            title: metadata.title.unwrap_or_else(|| "未命名".to_string()),
            author: metadata.author,
            genre: metadata.genre,
            word_count: metadata.estimated_word_count,
            file_format: None,
            file_hash: None,
            file_path: None,
            world_setting: Some(world_setting),
            plot_summary: Some(plot_summary),
            story_arc: Some(story_arc_json),
            analyzed_structure_json: None,
            analysis_status: AnalysisStatus::Completed,
            task_id: None,
            analysis_progress: 100,
            analysis_error: None,
            created_at: Local::now(),
            updated_at: Local::now(),
        })
    }
}

// ==================== 工具函数 ====================

/// 解析 LLM 返回的 JSON（带容错）
fn parse_json_response<T: serde::de::DeserializeOwned>(response: &str) -> Result<T, AnalysisError> {
    let trimmed = response.trim();

    // 尝试直接解析
    if let Ok(result) = serde_json::from_str::<T>(trimmed) {
        return Ok(result);
    }

    // 尝试提取 markdown 代码块
    if let Some(start) = trimmed.find("```json") {
        let after_start = &trimmed[start + 7..];
        if let Some(end) = after_start.find("```") {
            let json_str = after_start[..end].trim();
            if let Ok(result) = serde_json::from_str::<T>(json_str) {
                return Ok(result);
            }
        }
    }

    // 尝试提取任意 ``` 代码块
    if let Some(start) = trimmed.find("```") {
        let after_start = &trimmed[start + 3..];
        if let Some(end) = after_start.find("```") {
            let json_str = after_start[..end].trim();
            if let Ok(result) = serde_json::from_str::<T>(json_str) {
                return Ok(result);
            }
        }
    }

    // 尝试提取第一个 { 到最后一个 }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            let json_str = &trimmed[start..=end];
            if let Ok(result) = serde_json::from_str::<T>(json_str) {
                return Ok(result);
            }
        }
    }

    Err(AnalysisError::ParseError(format!(
        "Failed to parse JSON from LLM response: {}",
        &trimmed[..trimmed.len().min(200)]
    )))
}

/// 合并去重人物
fn merge_characters(all_results: Vec<Vec<ExtractedCharacter>>) -> Vec<ExtractedCharacter> {
    use std::collections::HashMap;

    let mut merged: HashMap<String, ExtractedCharacter> = HashMap::new();

    for batch in all_results {
        for character in batch {
            let name = character.name.clone();
            if let Some(existing) = merged.get_mut(&name) {
                // 合并信息：优先保留非空字段
                if existing.role_type.is_none() && character.role_type.is_some() {
                    existing.role_type = character.role_type;
                }
                if existing.personality.is_none() && character.personality.is_some() {
                    existing.personality = character.personality;
                }
                if existing.appearance.is_none() && character.appearance.is_some() {
                    existing.appearance = character.appearance;
                }
                // 合并关系
                for rel in character.relationships {
                    if !existing
                        .relationships
                        .iter()
                        .any(|r| r.target_name == rel.target_name)
                    {
                        existing.relationships.push(rel);
                    }
                }
            } else {
                merged.insert(name, character);
            }
        }
    }

    merged.into_values().collect()
}

/// 从场景生成剧情概要
fn plot_summary_from_scenes(scenes: &[ReferenceScene]) -> String {
    let summaries: Vec<String> = scenes
        .iter()
        .take(20)
        .map(|s| s.summary.clone().unwrap_or_default())
        .collect();
    summaries.join("；")
}

/// 计算人物重要度分数
fn calculate_importance(character: &ExtractedCharacter) -> f32 {
    let mut score = 0.0;

    // 角色定位权重
    if let Some(ref role) = character.role_type {
        score += match role.as_str() {
            "主角" | "主人公" => 1.0,
            "反派" | "反派角色" => 0.8,
            "配角" | "重要配角" => 0.6,
            "龙套" | "路人" => 0.2,
            _ => 0.4,
        };
    }

    // 性格描述越详细越重要
    if character.personality.is_some() {
        score += 0.1;
    }

    // 外貌描述
    if character.appearance.is_some() {
        score += 0.05;
    }

    // 关系越多越重要
    score += (character.relationships.len() as f32 * 0.05).min(0.3);

    score.min(1.0)
}
