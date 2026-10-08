#![allow(dead_code)]
use std::sync::Arc;

use super::types::*;
use crate::{
    db::{BlueprintRepository, CharacterRepository, CharacterState, DbPool},
    llm::LlmService,
    ports::VectorStore,
    router::TaskType,
};

/// 构建定稿后处理步骤列表
pub fn build_finalize_steps(
    _story_id: &str,
    _chapter_number: i32,
    chapter_title: &str,
    _draft_content: &str,
) -> Vec<PostProcessStepDef> {
    vec![
        PostProcessStepDef {
            key: "kb_import".to_string(),
            label: "📚 导入知识库".to_string(),
            critical: true,
        },
        PostProcessStepDef {
            key: "chapter_notes".to_string(),
            label: format!("📋 章节剧情要点提取 — {}", chapter_title),
            critical: true,
        },
        PostProcessStepDef {
            key: "character_cards".to_string(),
            label: "🎭 角色状态更新".to_string(),
            critical: false,
        },
        PostProcessStepDef {
            key: "style_analysis".to_string(),
            label: "🎨 文风自动学习".to_string(),
            critical: false,
        },
    ]
}

/// 执行单个后处理步骤
pub async fn run_post_process_step(
    story_id: &str,
    chapter_number: i32,
    draft_content: &str,
    step: &PostProcessStepDef,
    pool: &DbPool,
    llm_service: &LlmService,
    vector_store: &dyn VectorStore,
) -> Result<(), PipelineError> {
    match step.key.as_str() {
        "kb_import" => {
            run_kb_import(story_id, chapter_number, draft_content, pool, vector_store).await
        }
        "chapter_notes" => {
            run_chapter_notes(story_id, chapter_number, draft_content, pool, llm_service).await
        }
        "character_cards" => {
            run_character_cards(story_id, chapter_number, draft_content, pool, llm_service).await
        }
        "style_analysis" => {
            run_style_analysis(story_id, chapter_number, draft_content, pool, llm_service).await
        }
        _ => {
            log::warn!("[post_process] 未知步骤: {}", step.key);
            Ok(())
        }
    }
}

/// 执行后处理步骤（批量）
///
/// 遍历步骤列表，依次执行。关键步骤失败会中断管线。
pub async fn run_post_process(
    story_id: &str,
    chapter_number: i32,
    draft_content: &str,
    steps: &[PostProcessStepDef],
    pool: &DbPool,
    llm_service: &LlmService,
    vector_store: &dyn VectorStore,
    _callbacks: &dyn PipelineCallbacks,
    _options: Option<&PostProcessOptions>,
) -> Result<(), PipelineError> {
    for step in steps {
        let result = run_post_process_step(
            story_id,
            chapter_number,
            draft_content,
            step,
            pool,
            llm_service,
            vector_store,
        )
        .await;

        if let Err(e) = result {
            if step.critical {
                return Err(PipelineError {
                    phase: format!("post_process:{}", step.key),
                    message: e.message,
                    recoverable: false,
                });
            } else {
                log::warn!("[post_process] 非关键步骤 {} 失败: {}", step.key, e.message);
            }
        }
    }

    Ok(())
}

/// 导入知识库 — 将章节内容分块并嵌入 LanceDB
async fn run_kb_import(
    story_id: &str,
    chapter_number: i32,
    draft_content: &str,
    _pool: &DbPool,
    vector_store: &dyn VectorStore,
) -> Result<(), PipelineError> {
    log::info!(
        "[post_process] kb_import: story_id={}, chapter={}",
        story_id,
        chapter_number
    );

    match crate::knowledge_base::import_text(
        vector_store,
        story_id,
        chapter_number,
        draft_content,
        &format!("第{}章", chapter_number),
    )
    .await
    {
        Ok(result) => {
            log::info!(
                "[post_process] kb_import: 成功导入 {} chunks, {} vectors",
                result.chunks_imported,
                result.vectors_indexed
            );
        }
        Err(e) => {
            log::error!("[post_process] kb_import: 失败: {}", e);
            return Err(PipelineError {
                phase: "post_process:kb_import".to_string(),
                message: format!("知识库导入失败: {}", e),
                recoverable: false,
            });
        }
    }

    Ok(())
}

/// 提取章节剧情要点并更新 blueprint.notes
async fn run_chapter_notes(
    story_id: &str,
    chapter_number: i32,
    draft_content: &str,
    pool: &DbPool,
    llm_service: &LlmService,
) -> Result<(), PipelineError> {
    log::info!(
        "[post_process] chapter_notes: story_id={}, chapter={}",
        story_id,
        chapter_number
    );

    let content_preview = if draft_content.len() > 8000 {
        &draft_content[..draft_content.floor_char_boundary(8000)]
    } else {
        draft_content
    };

    // v0.21.0: 从 PromptRegistry 读取（支持用户覆盖）
    let prompt = {
        let tpl = crate::prompts::registry::resolve_prompt(pool, "pipeline_post_process_plot")
            .unwrap_or_else(|_| {
                crate::prompts::registry::resolve_prompt_default("pipeline_post_process_plot")
                    .unwrap_or_else(|| default_plot_extraction_prompt().to_string())
            });
        let mut vars = std::collections::HashMap::new();
        vars.insert("content".to_string(), content_preview.to_string());
        crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
    };

    let notes = match llm_service
        .generate_for_task(
            TaskType::Summarization,
            prompt,
            Some(1024),
            Some(0.3),
            Some("chapter_notes"),
        )
        .await
    {
        Ok(resp) => resp.content.trim().to_string(),
        Err(e) => {
            log::warn!("[post_process] chapter_notes LLM 调用失败，使用占位: {}", e);
            format!(
                "[第{}章] 剧情要点待提取\n- 核心事件待提取\n- 关键对话待提取\n- 伏笔和悬念待提取",
                chapter_number
            )
        }
    };

    let blueprint_repo = BlueprintRepository::new(pool.clone());
    if let Ok(Some(bp)) = blueprint_repo.get_by_chapter(story_id, chapter_number) {
        let req = crate::db::UpdateBlueprintRequest {
            notes: Some(notes),
            ..Default::default()
        };
        let _ = blueprint_repo.update(&bp.id, req);
    }

    Ok(())
}

/// 更新角色动态状态 — 使用 LLM 解析章节内容中的角色状态变化
async fn run_character_cards(
    story_id: &str,
    chapter_number: i32,
    draft_content: &str,
    pool: &DbPool,
    llm_service: &LlmService,
) -> Result<(), PipelineError> {
    log::info!(
        "[post_process] character_cards: story_id={}, chapter={}",
        story_id,
        chapter_number
    );

    let char_repo = CharacterRepository::new(pool.clone());
    let all_chars = char_repo
        .get_by_story(story_id)
        .map_err(|e| PipelineError {
            phase: "post_process:character_cards".to_string(),
            message: format!("读取角色失败: {}", e),
            recoverable: true,
        })?;

    // v0.64.7：已死角色的身体状态不得被 LLM 改回活人——标记与持久列单调。
    let dead_cards = crate::story_system::life_status::dead_marker_map(pool, story_id);

    if all_chars.is_empty() {
        log::info!("[post_process] character_cards: 故事无角色，跳过");
        return Ok(());
    }

    let states = char_repo
        .get_character_states_by_story(story_id)
        .map_err(|e| {
            log::warn!("[post_process] 加载角色状态失败: {}", e);
            e
        })
        .unwrap_or_default();

    let content_preview = if draft_content.len() > 6000 {
        &draft_content[..draft_content.floor_char_boundary(6000)]
    } else {
        draft_content
    };

    // 构建角色上下文
    let mut char_context_parts = Vec::new();
    for c in &all_chars {
        let mut parts = vec![format!("【{}】", c.name)];
        if let Some(ref bg) = c.background {
            parts.push(format!("背景: {}", bg));
        }
        let state = states.get(&c.id);
        if let Some(ref loc) = state.and_then(|s| s.location.as_ref()) {
            parts.push(format!("当前位置: {}", loc));
        }
        if let Some(ref phys) = state.and_then(|s| s.physical_state.as_ref()) {
            parts.push(format!("身体状态: {}", phys));
        }
        if let Some(ref mental) = state.and_then(|s| s.mental_state.as_ref()) {
            parts.push(format!("心理状态: {}", mental));
        }
        if let Some(ref items) = state.and_then(|s| s.key_items.as_ref()) {
            parts.push(format!("持有物品: {}", items));
        }
        if let Some(ref recent) = state.and_then(|s| s.recent_events.as_ref()) {
            parts.push(format!("近期事件: {}", recent));
        }
        char_context_parts.push(parts.join(" | "));
    }
    let char_context = char_context_parts.join("\n");

    // v0.21.0: 从 PromptRegistry 读取（支持用户覆盖）
    let prompt = {
        let tpl =
            crate::prompts::registry::resolve_prompt(pool, "pipeline_post_process_character_state")
                .unwrap_or_else(|_| {
                    crate::prompts::registry::resolve_prompt_default(
                        "pipeline_post_process_character_state",
                    )
                    .unwrap_or_else(|| default_character_state_prompt().to_string())
                });
        let mut vars = std::collections::HashMap::new();
        vars.insert("content".to_string(), content_preview.to_string());
        crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars)
    };

    let mut updated_count = 0;

    match llm_service
        .generate_for_task(
            TaskType::Analysis,
            prompt,
            Some(2048),
            Some(0.2),
            Some("character_cards"),
        )
        .await
    {
        Ok(resp) => {
            let text = resp.content.trim();
            // 尝试提取 JSON 数组（LLM 可能包裹在 markdown 代码块中）
            let json_str = if let Some(start) = text.find('[') {
                if let Some(end) = text.rfind(']') {
                    &text[start..=end]
                } else {
                    text
                }
            } else {
                text
            };

            match serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
                Ok(updates) => {
                    for update in updates {
                        let name = update
                            .get("character_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if name.is_empty() {
                            continue;
                        }

                        if let Some(character) = all_chars.iter().find(|c| c.name == name) {
                            let new_location = update
                                .get("location")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let new_physical = update
                                .get("physical_state")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let new_mental = update
                                .get("mental_state")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let new_items = update
                                .get("key_items")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            let new_recent = update
                                .get("recent_events")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());

                            let existing = states.get(&character.id);
                            let composed_physical =
                                new_physical.or(existing.and_then(|s| s.physical_state.clone()));
                            let state = CharacterState {
                                location: new_location
                                    .or(existing.and_then(|s| s.location.clone())),
                                power_level: existing.and_then(|s| s.power_level.clone()),
                                physical_state: match dead_cards.get(&character.name) {
                                    Some(chapter) => {
                                        Some(crate::db::character_life::annotate_physical_state(
                                            composed_physical.as_deref(),
                                            *chapter,
                                        ))
                                    }
                                    None => composed_physical,
                                },
                                mental_state: new_mental
                                    .or(existing.and_then(|s| s.mental_state.clone())),
                                key_items: new_items.or(existing.and_then(|s| s.key_items.clone())),
                                recent_events: new_recent
                                    .or(Some(format!("[第{}章] 出场", chapter_number))),
                                updated_at_chapter: Some(chapter_number),
                                cs_json: existing.and_then(|s| s.cs_json.clone()),
                                state_transitions_json: None,
                                arc_type: None,
                            };

                            if let Ok(count) =
                                char_repo.update_character_state(&character.id, &state)
                            {
                                if count > 0 {
                                    updated_count += 1;
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[post_process] character_cards JSON 解析失败: {}. Raw: {}",
                        e,
                        json_str
                    );
                }
            }
        }
        Err(e) => {
            log::warn!("[post_process] character_cards LLM 调用失败: {}", e);
        }
    }

    // 兜底：简单扫描内容中提到的角色名，为未更新的角色标记出场
    for character in &all_chars {
        if content_preview.contains(&character.name) {
            // 原 LLM 调用在 Ok/Err 分支均返回 false，属于冗余调用，直接保留
            // false 语义
            let already_updated = false;
            if already_updated {
                continue;
            }

            // 如果角色没有被 LLM 更新过，至少标记出场
            let existing = states.get(&character.id);
            let carried_physical = existing.and_then(|s| s.physical_state.clone());
            let state = CharacterState {
                location: existing.and_then(|s| s.location.clone()),
                power_level: existing.and_then(|s| s.power_level.clone()),
                physical_state: match dead_cards.get(&character.name) {
                    Some(chapter) => Some(crate::db::character_life::annotate_physical_state(
                        carried_physical.as_deref(),
                        *chapter,
                    )),
                    None => carried_physical,
                },
                mental_state: existing.and_then(|s| s.mental_state.clone()),
                key_items: existing.and_then(|s| s.key_items.clone()),
                recent_events: Some(format!("[第{}章] 出场", chapter_number)),
                updated_at_chapter: Some(chapter_number),
                cs_json: existing.and_then(|s| s.cs_json.clone()),
                state_transitions_json: None,
                arc_type: None,
            };
            if let Ok(count) = char_repo.update_character_state(&character.id, &state) {
                if count > 0 {
                    updated_count += 1;
                }
            }
        }
    }

    log::info!(
        "[post_process] character_cards: 更新 {} 个角色状态",
        updated_count
    );
    Ok(())
}

/// 文风自动学习 — 每5章触发
async fn run_style_analysis(
    story_id: &str,
    _chapter_number: i32,
    _draft_content: &str,
    pool: &DbPool,
    _llm_service: &LlmService,
) -> Result<(), PipelineError> {
    use crate::{
        db::repositories::{WritingStyleRepository, WritingStyleUpdate},
        pipeline::style_analysis,
    };

    // 检查是否应触发
    let should_trigger =
        style_analysis::should_trigger_style_analysis(story_id, pool).map_err(|e| {
            PipelineError {
                phase: "style_analysis".to_string(),
                message: format!("风格分析触发检查失败: {}", e),
                recoverable: true,
            }
        })?;

    if !should_trigger {
        log::info!("[post_process] style_analysis: 跳过（条件不满足）");
        return Ok(());
    }

    log::info!("[post_process] style_analysis: story_id={}", story_id);

    // 执行风格分析
    let result =
        style_analysis::analyze_style_for_story(story_id, pool).map_err(|e| PipelineError {
            phase: "style_analysis".to_string(),
            message: format!("风格分析失败: {}", e),
            recoverable: true,
        })?;

    log::info!(
        "[post_process] style_analysis: 第{}-{}章分析完成，句长={:.1}, 对话比={:.2}, \
         比喻密度={:.2}, 内心独白={:.2}, 情感密度={:.2}, 节奏={:.2}",
        result.chapter_range.0,
        result.chapter_range.1,
        result.metrics.sentence_length,
        result.metrics.dialogue_ratio,
        result.metrics.metaphor_density,
        result.metrics.inner_monologue_ratio,
        result.metrics.emotion_density,
        result.metrics.rhythm_score,
    );

    // 更新 story 的 writing_style（如果存在）
    let style_repo = WritingStyleRepository::new(pool.clone());
    if let Ok(Some(existing)) = style_repo.get_by_story(story_id) {
        let update = WritingStyleUpdate {
            description: Some(format!(
                "自动分析（第{}-{}章）：句长{:.0}字，对话占比{:.0}%，比喻{:.1}个/千字，情感密度{:.\
                 3}",
                result.chapter_range.0,
                result.chapter_range.1,
                result.metrics.sentence_length,
                result.metrics.dialogue_ratio * 100.0,
                result.metrics.metaphor_density,
                result.metrics.emotion_density,
            )),
            ..Default::default()
        };
        if let Err(e) = style_repo.update(&existing.id, &update) {
            log::warn!(
                "[post_process] style_analysis: 更新 writing_style 失败: {}",
                e
            );
        } else {
            log::info!("[post_process] style_analysis: writing_style 已更新");
        }
    }

    Ok(())
}

/// v0.21.0: 剧情要点提取内置默认提示词
fn default_plot_extraction_prompt() -> &'static str {
    r#"请阅读以下小说章节内容，提取核心剧情要点（3-5条）。每条要点用一句话概括，使用中文。

章节内容：
{{content}}

请仅输出要点列表，每条一行，以"- "开头。不要输出任何额外解释。"#
}

/// v0.21.0: 角色状态追踪内置默认提示词
fn default_character_state_prompt() -> &'static str {
    r#"你是一位专业的小说角色状态追踪器。请根据以下小说章节内容，分析每个出场角色的状态变化。

章节内容：
{{content}}

请严格按以下 JSON 格式输出每个角色的状态更新。只有状态确实发生变化的角色才需要包含在输出中。如果没有变化，输出空数组 []。

输出格式示例：
[
  {
    "character_name": "角色名",
    "location": "新位置（如有变化）",
    "physical_state": "新的身体状态（如有变化）",
    "mental_state": "新的心理状态（如有变化）",
    "key_items": "新的持有物品（如有变化）",
    "recent_events": "本章发生的关键事件（1-2句）"
  }
]

要求：
1. 只输出 JSON 数组，不要任何额外文字
2. 只包含状态确实发生变化的角色
3. recent_events 必须概括角色在本章经历的关键事件
4. 如果某个字段没有变化，不要包含该字段或设为 null"#
}
