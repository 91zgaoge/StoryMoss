#![allow(dead_code)]
//! Plan Generator - 智能执行计划生成器
//!
//! 将用户的自然语言输入转化为结构化的执行计划，
//! 替代旧的 IntentParser + IntentExecutor 分类标签方式。
//! 核心设计：LLM 自由理解用户意图，自主选择能力组合，无预设分类。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    capabilities::get_capability_registry,
    creative_engine::asset_capability_manifest::AssetTaskType, error::AppError,
    intent::WritingIntentClassification, llm::LlmService, router::TaskType,
};

pub mod bootstrap;
pub mod executor;
pub mod swarm;
pub mod template_learning;
pub use executor::{PlanExecutionResult, PlanExecutor};
#[allow(unused_imports)]
pub use template_learning::PlanTemplate;
pub use template_learning::PlanTemplateLibrary;

/// 执行计划中的单个步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_id: String,
    pub capability_id: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub parameters: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// v0.23 TriShot：标记为长任务，跳过 PlanExecutor 90s 步超时
    #[serde(default)]
    pub long_running: bool,
}

/// 完整的执行计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlan {
    #[serde(default)]
    pub understanding: String,
    #[serde(default)]
    pub steps: Vec<PlanStep>,
    #[serde(default)]
    pub fallback_message: String,
}

/// 场景结构摘要（用于计划生成）
#[derive(Debug, Clone)]
pub struct SceneStructureSummary {
    pub scene_id: String,
    pub sequence_number: i32,
    pub title: Option<String>,
    pub execution_stage: Option<String>,
    pub has_content: bool,
    pub word_count: usize,
}

/// 生成计划所需的上下文
#[derive(Debug, Clone)]
pub struct PlanContext {
    pub current_story_id: Option<String>,
    pub has_story: bool,
    pub has_chapters: bool,
    pub chapter_count: usize,
    pub current_content_preview: Option<String>,
    pub user_input: String,
    // Phase 3: 场景/章节结构感知
    pub scene_count: usize,
    pub scenes_summary: Vec<SceneStructureSummary>,
    pub current_scene_id: Option<String>,
    pub current_scene_stage: Option<String>,
    pub total_word_count: usize,
    pub latest_chapter_word_count: usize,
    pub story_progress: String, /* "just_started" | "developing" | "midpoint" | "climax" |
                                 * "resolution" */
    // Phase 4: 增强上下文 - 世界观、角色、伏笔、风格、MCP
    pub world_building_summary: Option<String>,
    pub character_list: Vec<String>,
    pub foreshadowing_status: Vec<String>,
    pub style_dna_info: Option<String>,
    pub mcp_tools_available: Vec<String>,
    /// v0.22.5: 最新深度洞察摘要，供 Planner 生成分阶段干预计划
    pub deep_insight_summary: Option<String>,
    // W3-F3: 支持选中文本（Inline Suggestion 统一路径）
    pub selected_text: Option<String>,
    // v0.7.8: 风格权重（0-100，默认50）
    pub style_weight: i32,
    // v0.8.0: 当前章节号（用于记忆构建）
    pub chapter_number: i32,
    // v0.10.0: 当前故事的创作策略（模型选择或用户锁定）
    pub selected_strategy: Option<crate::domain::strategy::SelectedStrategy>,
    /// v0.30.11: LLM 写作意图路由分类（由
    /// `IntentParser::classify_writing_intent` 产出）。
    /// 贯穿管线替代各处朴素子串匹配。None 表示未分类，各站点走兜底启发式。
    pub intent_classification: Option<WritingIntentClassification>,
}

/// 计划生成器
pub struct PlanGenerator {
    llm_service: LlmService,
    app_handle: Option<AppHandle>,
}

impl PlanGenerator {
    pub fn new(llm_service: LlmService) -> Self {
        Self {
            llm_service,
            app_handle: None,
        }
    }

    pub fn with_app_handle(mut self, app_handle: AppHandle) -> Self {
        self.app_handle = Some(app_handle);
        self
    }

    fn emit_progress(&self, stage: &str, message: &str) {
        if let Some(ref app) = self.app_handle {
            let _ = app.emit(
                "plan-generator-progress",
                serde_json::json!({
                    "stage": stage,
                    "message": message,
                }),
            );
        }
    }

    /// 防线 2 决策：首步 capability 是否应被强制改为 writer。
    ///
    /// v0.30.11: 用 LLM 分类的 `is_prose_request` 替代单字
    /// `contains('写'/'创')` 朴素匹配（"大纲里写明主角动机"会命中"写"误改
    /// outline->writer）。分类经 `PlanContext` 贯穿；缺失时兜底
    /// `true`（force-to-writer 安全默认：误改 outline->writer
    /// 可恢复，反之返回空模板灾难）。
    ///
    /// v0.30.12: 新增 `inspector` 处理。续写请求被误路由到质检员会返回**审查
    /// 报告**而非正文（用户报告"继续写当前这部小说"得到"总体评分 0.85 / 具体
    /// 问题清单"）。`inspector`
    /// 仅保留给显式审查（`Audit`）或改写润色（`Rewrite`，
    /// Rule 9 的 inspector->writer 流）；续写（`is_continuation`）/ 创世 /
    /// 无分类 一律强制 writer。
    fn should_force_correct_to_writer(
        first_cap: &str,
        classification: Option<&WritingIntentClassification>,
    ) -> bool {
        let needs_correction = first_cap == "outline_planner"
            || first_cap == "style_mimic"
            || first_cap == "plot_analyzer"
            || first_cap == "inspector"
            || first_cap.starts_with("builtin.style_enhancer")
            || first_cap.starts_with("builtin.text_formatter")
            || first_cap.starts_with("builtin.character_voice")
            || first_cap.starts_with("builtin.emotion_pacing");
        if !needs_correction {
            return false;
        }
        if first_cap == "inspector" {
            return match classification {
                // 续写绝不该返回审查报告。
                Some(c) if c.is_continuation => true,
                Some(c) => match c.task_type {
                    // 显式审查（非 prose）保留 inspector；若同时判为 prose 请求，
                    // 说明分类矛盾（如"继续写"被误判 Audit），强制 writer。
                    AssetTaskType::Audit => c.is_prose_request,
                    // 改写润色保留 inspector（Rule 9 inspector->writer 流，最终输出是 writer
                    // 正文）。
                    AssetTaskType::Rewrite => false,
                    // Continuation/Genesis/Other 强制 writer。
                    _ => true,
                },
                // 无分类安全默认 writer（续写误路由代价 >> 审查被改 writer 的代价）。
                None => true,
            };
        }
        classification.map(|c| c.is_prose_request).unwrap_or(true)
    }

    /// 防线 2 的强制修正动作：当 `should_force_correct_to_writer`
    /// 判定需要修正时， 把 plan 首步的 capability_id 改为 writer 并标注
    /// understanding/purpose。
    ///
    /// v0.30.13: 提取为可复用方法，使其可在 **plan 执行咽喉点**
    /// （`PlanExecutor::execute_with_context`--所有 plan 来源 SING /
    /// PlanGenerator / fallback 的必经之路）统一施加，修补 SING 路径直接返回
    /// plan 绕过 `generate_plan` 内 force-correction 的漏洞（续写被 SING 路由到
    /// `builtin.style_enhancer`
    /// 等返回"请提供需要增强的原始文本"模板而非正文）。 幂等：已为 writer
    /// 的首步不会被改动，故 generate_plan 与咽喉点重复调用安全。
    pub(crate) fn force_correct_first_step_to_writer(
        plan: &mut ExecutionPlan,
        classification: Option<&WritingIntentClassification>,
        user_input: &str,
    ) {
        if plan.steps.is_empty() {
            return;
        }
        let first_cap = plan.steps[0].capability_id.clone();
        if !Self::should_force_correct_to_writer(&first_cap, classification) {
            return;
        }
        log::warn!(
            "[PlanGenerator] Force-correcting {} -> writer for prose/continuation request: {}",
            first_cap,
            user_input
        );
        plan.steps[0].capability_id = "writer".to_string();
        plan.steps[0].purpose = "Auto-corrected: user wants prose generation/continuation, not style enhancement, analysis, or review"
            .to_string();
        plan.understanding = format!(
            "{} [auto-corrected: prose/continuation keywords detected, forcing writer instead of {}]",
            plan.understanding, first_cap
        );
    }

    /// v0.30.14 防线 3：prose 请求计划净化。
    ///
    /// force-correction（防线 2）只修正**首步**，无法拦截多步 plan 中**尾部**的
    /// `style_enhancer`/`inspector` 等非 writer 步骤。`execute_plan`
    /// 用**最后产出 `content` 的步骤**作为
    /// `final_content`（executor.rs:685-687），故尾部的 `style_enhancer`
    /// 会用"请提供需要增强的原始文本"模板覆盖 writer 已产出的正文，
    /// `inspector` 会用审查报告覆盖--用户看到模板/报告而非正文。这是该误路由
    /// bug 第 5 次复发的根因（v0.30.10/11/12/13
    /// 各堵一条路径，但多步尾部漏网）。
    ///
    /// 本方法在咽喉点对所有 `is_prose_request` plan 统一净化：
    /// 1. 移除 `builtin.style_enhancer`/`text_formatter`/`character_voice`/
    ///    `emotion_pacing` 等绝不产出可用正文的技能步骤（产出模板/元文本）。
    /// 2. 续写（`is_continuation`）塌缩为单 writer
    ///    步（续写本质单步，多步必为误路由）。
    /// 3. 其余 prose 请求（改写/增强等）：弹出尾部非 writer 步骤，保证末步为
    ///    writer （`final_content` = 正文）。保留 `[inspector, writer]` 等 Rule
    ///    9 合法流。
    /// 4. 净化后若为空，补一个 writer 步。
    ///
    /// 非 prose 请求（显式审查 `Audit`/`is_prose_request=false`）不净化，保留
    /// inspector 等合法用途。
    pub(crate) fn sanitize_plan_for_prose_request(
        plan: &mut ExecutionPlan,
        classification: Option<&WritingIntentClassification>,
        context: &PlanContext,
    ) {
        let cls = match classification {
            Some(c) if c.is_prose_request => c,
            _ => return,
        };

        // 1. 移除非 prose 技能步骤
        let before = plan.steps.len();
        plan.steps
            .retain(|s| !Self::is_non_prose_skill(&s.capability_id));
        if plan.steps.len() != before {
            log::warn!(
                "[PlanGenerator] Sanitize: removed {} non-prose skill step(s) from prose request: {}",
                before - plan.steps.len(),
                context.user_input
            );
        }

        // 2. 续写塌缩为单 writer
        if cls.is_continuation {
            if plan.steps.len() != 1 || plan.steps[0].capability_id != "writer" {
                log::warn!(
                    "[PlanGenerator] Sanitize: collapsing continuation plan ({} steps) to single writer: {}",
                    plan.steps.len(),
                    context.user_input
                );
                plan.steps = vec![Self::make_sanitized_writer_step(context)];
                plan.understanding = format!(
                    "{} [sanitized: continuation collapsed to single writer]",
                    plan.understanding
                );
            }
            return;
        }

        // 3. 弹出尾部非 writer 步骤，保证末步为 writer（final_content = 正文）
        while let Some(last) = plan.steps.last() {
            if last.capability_id == "writer" {
                break;
            }
            let removed = plan.steps.pop().expect("last() confirmed non-empty");
            log::warn!(
                "[PlanGenerator] Sanitize: popping trailing non-writer step '{}' ({}) from prose plan",
                removed.step_id,
                removed.capability_id
            );
        }

        // 4. 空则补 writer
        if plan.steps.is_empty() {
            log::warn!(
                "[PlanGenerator] Sanitize: prose plan empty after sanitize, adding writer step: {}",
                context.user_input
            );
            plan.steps.push(Self::make_sanitized_writer_step(context));
        }
    }

    /// 判定 capability 是否为"绝不产出可用正文"的技能（模板/元文本）。
    /// 这些步骤在 prose 请求中无论作为首步、中间步还是尾步都无益--产出会被
    /// `execute_plan` 当作 content
    /// 覆盖正文。inspector/outline_planner/style_mimic/ plot_analyzer
    /// 不在此列：它们可作为中间步（其 content 若为末步才需由 `sanitize`
    /// 的尾部弹出处理）。
    fn is_non_prose_skill(cap_id: &str) -> bool {
        cap_id.starts_with("builtin.style_enhancer")
            || cap_id.starts_with("builtin.text_formatter")
            || cap_id.starts_with("builtin.character_voice")
            || cap_id.starts_with("builtin.emotion_pacing")
    }

    /// 构造一个净化用的 writer 步（带 story_id/instruction/current_content）。
    fn make_sanitized_writer_step(context: &PlanContext) -> PlanStep {
        let mut params = HashMap::new();
        if let Some(ref story_id) = context.current_story_id {
            params.insert(
                "story_id".to_string(),
                serde_json::Value::String(story_id.clone()),
            );
        }
        params.insert(
            "instruction".to_string(),
            serde_json::Value::String(context.user_input.clone()),
        );
        if let Some(ref preview) = context.current_content_preview {
            params.insert(
                "current_content".to_string(),
                serde_json::Value::String(preview.clone()),
            );
        }
        PlanStep {
            step_id: "sanitized_writer".to_string(),
            capability_id: "writer".to_string(),
            purpose: "Sanitized writer step: prose request must yield prose".to_string(),
            parameters: params,
            depends_on: vec![],
            long_running: true,
        }
    }

    /// 根据用户输入和系统状态生成执行计划
    ///
    /// 外层套 60 秒整体超时：计划生成只应消耗几百 tokens，若卡住可快速失败，
    /// 避免前端"系统正在处理中..."长期不消失。
    pub async fn generate_plan(&self, context: &PlanContext) -> Result<ExecutionPlan, AppError> {
        let start = std::time::Instant::now();
        match tokio::time::timeout(
            std::time::Duration::from_secs(60),
            self.generate_plan_inner(context),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                log::error!(
                    "[PlanGenerator] generate_plan timed out after {}ms",
                    start.elapsed().as_millis()
                );
                Err(AppError::internal(
                    "执行计划生成超时（60秒），请检查模型配置后重试".to_string(),
                ))
            }
        }
    }

    async fn generate_plan_inner(&self, context: &PlanContext) -> Result<ExecutionPlan, AppError> {
        self.emit_progress("context", "正在分析故事上下文...");
        let registry_context = get_capability_registry().to_llm_context();

        // Sanitize inputs to prevent prompt injection / format breakage
        fn sanitize_for_prompt(s: &str) -> String {
            s.replace('"', "'")
                .replace('\n', " ")
                .replace('\r', "")
                .replace("{{", "〔")
                .replace("}}", "〕")
        }

        let preview = context.current_content_preview.as_deref().unwrap_or("none");
        let user_input_clean = sanitize_for_prompt(&context.user_input);
        let preview_clean = sanitize_for_prompt(preview);
        let registry_clean = sanitize_for_prompt(&registry_context);

        // Build scene structure summary for prompt —
        // 截断到最近10个场景，减少大故事的prompt长度
        let scenes_summary = if context.scenes_summary.is_empty() {
            "No scenes yet".to_string()
        } else {
            let total = context.scenes_summary.len();
            let truncated: Vec<_> = context.scenes_summary.iter().rev().take(10).collect();
            let mut lines: Vec<String> = truncated
                .iter()
                .map(|s| {
                    let stage = s.execution_stage.as_deref().unwrap_or("unknown");
                    let title = s.title.as_deref().unwrap_or("Untitled");
                    let content_flag = if s.has_content { "✓" } else { "○" };
                    format!(
                        "  #{} [{}] {} {} ({} words)",
                        s.sequence_number, stage, title, content_flag, s.word_count
                    )
                })
                .collect();
            if total > 10 {
                lines.insert(0, format!("  ... ({} earlier scenes omitted)", total - 10));
            }
            lines.reverse();
            lines.join("\n")
        };

        let current_scene_info = if let Some(ref id) = context.current_scene_id {
            format!(
                "Current scene ID: {} (stage: {})",
                id,
                context.current_scene_stage.as_deref().unwrap_or("unknown")
            )
        } else {
            "No current scene".to_string()
        };

        // 构建增强上下文信息 — 截断超长文本，减少token消耗
        let world_building_text = context
            .world_building_summary
            .as_deref()
            .map(|s| {
                if s.chars().count() > 200 {
                    format!("{}...(truncated)", s.chars().take(200).collect::<String>())
                } else {
                    s.to_string()
                }
            })
            .unwrap_or_else(|| "No world building yet".to_string());
        let characters_text = if context.character_list.is_empty() {
            "No characters yet".to_string()
        } else {
            let total = context.character_list.len();
            let shown: Vec<_> = context.character_list.iter().take(5).cloned().collect();
            let mut text = format!("Characters: {}", shown.join(", "));
            if total > 5 {
                text.push_str(&format!(" (+{} more)", total - 5));
            }
            text
        };
        let foreshadowing_text = if context.foreshadowing_status.is_empty() {
            "No active foreshadowing".to_string()
        } else {
            format!(
                "Active foreshadowing:\n{}",
                context
                    .foreshadowing_status
                    .iter()
                    .map(|f| format!("  - {}", f))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        let deep_insight_text = context
            .deep_insight_summary
            .as_deref()
            .map(|s| format!("Deep insight (latest):\n{}", s))
            .unwrap_or_else(|| "No deep insight report yet".to_string());
        let style_dna_text = context
            .style_dna_info
            .as_deref()
            .unwrap_or("No style DNA configured");
        let strategy_text = context
            .selected_strategy
            .as_ref()
            .map(|s| {
                let mut lines = vec![format!("rationale: {}", s.rationale)];
                if let Some(id) = &s.genre_profile_id {
                    lines.push(format!("genre_profile_id: {}", id));
                }
                if let Some(id) = &s.methodology_id {
                    lines.push(format!("methodology_id: {}", id));
                }
                if !s.style_dna_ids.is_empty() {
                    lines.push(format!("style_dna_ids: {}", s.style_dna_ids.join(", ")));
                }
                if !s.skill_ids.is_empty() {
                    lines.push(format!("skill_ids: {}", s.skill_ids.join(", ")));
                }
                format!("Selected creative strategy:\n{}", lines.join("\n"))
            })
            .unwrap_or_else(|| "No creative strategy selected".to_string());
        let mcp_tools_text = if context.mcp_tools_available.is_empty() {
            "No MCP tools available".to_string()
        } else {
            let total = context.mcp_tools_available.len();
            let shown: Vec<_> = context
                .mcp_tools_available
                .iter()
                .take(5)
                .cloned()
                .collect();
            let mut lines = shown
                .iter()
                .map(|t| format!("  - {}", t))
                .collect::<Vec<_>>();
            if total > 5 {
                lines.push(format!("  ... ({} more tools)", total - 5));
            }
            format!("Available MCP tools:\n{}", lines.join("\n"))
        };

        // 简化 Capability Registry — 当资产过多时保留核心能力，避免 prompt 爆炸
        let registry_clean = if registry_clean.chars().count() > 4000 {
            let core_caps = [
                "writer: 生成故事正文",
                "inspector: 质检内容",
                "outline_planner: 规划大纲",
                "create_chapter: 创建章节",
                "create_character: 创建角色",
                "update_character: 修改角色",
                "update_world_building: 修改世界观",
                "update_scene: 修改场景",
                "builtin.style_enhancer: 风格增强",
                "builtin.character_voice: 角色声音",
                "builtin.emotion_pacing: 情感节奏",
                "mcp.*: 外部工具",
                "methodology.*: 创作方法论（只读上下文）",
                "genre_profile.*: 体裁画像（只读上下文）",
                "style_dna.*: 风格 DNA（只读上下文）",
            ];
            format!(
                "Available capabilities (simplified):\n{}",
                core_caps.join("\n")
            )
        } else {
            registry_clean
        };

        self.emit_progress("planning", "正在生成执行计划...");

        // v0.21.0: 检查 PromptRegistry 是否有 planner_generator 覆盖
        // 有覆盖时用覆盖内容（支持 {{user_input}} {{capabilities}} 等变量），
        // 无覆盖时用原有动态拼接逻辑
        let prompt = if let Some(app) = &self.app_handle {
            app.try_state::<crate::db::DbPool>().and_then(|s| {
                crate::prompts::registry::resolve_prompt(s.inner(), "planner_generator").ok()
            })
        } else {
            None
        }
        .or_else(|| crate::prompts::registry::resolve_prompt_default("planner_generator"));

        let prompt = if let Some(template) = prompt {
            // 用户覆盖了 PlanGenerator prompt，渲染模板变量
            let mut vars = std::collections::HashMap::new();
            vars.insert("user_input".to_string(), context.user_input.clone());
            vars.insert("capabilities".to_string(), registry_clean.clone());
            vars.insert(
                "story_context".to_string(),
                format!(
                    "Has story: {}\nChapter count: {}\nTotal word count: {}",
                    context.has_story, context.chapter_count, context.total_word_count
                ),
            );
            vars.insert(
                "deep_insight_summary".to_string(),
                context
                    .deep_insight_summary
                    .clone()
                    .unwrap_or_else(|| "No deep insight report yet".to_string()),
            );
            crate::prompts::engine::TemplateEngine::render_with_conditions(&template, &vars)
        } else {
            // 默认动态拼接逻辑（原代码）
            format!(
                r#"You are an intelligent orchestrator for a creative writing application.

Current system state:
- Has story: {}
- Story ID: {}
- Has chapters: {}
- Chapter count: {}
- Total word count: {}
- Latest chapter words: {}
- Story progress: {}
- Scene count: {}
{}

Scene structure (last 10 shown):
{}

World building:
{}

{}

{}

Deep insight (latest):
{}

Style: {}

{}

{}

Current content preview: {}

User input: "{}"

{}

Your task: Analyze the user's intent and generate an execution plan.

Respond with JSON:
{{
  "understanding": "Your understanding of what the user wants (free text, not categories)",
  "steps": [
    {{
      "step_id": "step_1",
      "capability_id": "writer",
      "purpose": "Why this capability is chosen",
      "parameters": {{"story_id": "...", "instruction": "..."}},
      "depends_on": []
    }},
    {{
      "step_id": "step_2",
      "capability_id": "inspector",
      "purpose": "Quality check the writer output",
      "parameters": {{"story_id": "...", "draft": "{{step_1}}"}},
      "depends_on": ["step_1"]
    }}
  ],
  "fallback_message": "If the plan fails, tell the user this..."
}}

Rules:
1. Do NOT use classification labels or keyword matching in your reasoning.
2. Choose capabilities based on what the user actually needs.
3. Use depends_on to order steps when one step needs another's output. depends_on MUST ONLY contain step_id values of OTHER steps in this same plan (e.g. "step_1"). NEVER put context names, capability names, or free text (e.g. "Story Context", "writer") in depends_on -- such values are not step outputs and will be ignored.
4. step_id must be unique within the plan.
5. fallback_message should be helpful if execution fails.
6. For parameters, you can reference output from a previous step using {{step_id}} syntax in string values.
7. Available capability_id values include:
   - Agents: writer, inspector, outline_planner, style_mimic, plot_analyzer
   - System: create_story, create_chapter, create_character, update_character, update_world_building, update_scene, query_knowledge_graph
   - Skills: builtin.style_enhancer, builtin.plot_twist, builtin.text_formatter, builtin.character_voice, builtin.emotion_pacing
   - MCP: mcp.{{server_id}}.{{tool_name}} (use only when external data is needed)
8. CRITICAL: If the user wants to continue writing and the current scene has no content or is in 'planning'/'outline' stage, use 'writer' to generate draft content.
9. If the user EXPLICITLY wants to review/audit/critique/refine EXISTING text (e.g. 检查/审查/评估/润色/改进 this text), use 'inspector' first then 'writer'. CRITICAL: 'continue writing' / '继续写' / '续写' / '往下写' is a CONTINUATION, NOT a refine request -- it MUST use 'writer' directly (see Rule 21), never 'inspector' (inspector returns a review report, not prose). When using 'inspector', you MUST pass the text to check as the "draft" parameter using {{{{step_id}}}} syntax (e.g. "draft": "{{{{step_1}}}}" to check the output of step_1). The inspector CANNOT function without a draft -- if you omit it, the inspector receives empty content and returns a request for input instead of an actual review.
10. If story progress is 'just_started' and user asks for next chapter/scene, use 'create_chapter' or 'outline_planner' first.
11. If scenes are stuck in 'planning' or 'outline' stage, prioritize 'writer' to move them to 'drafting'.
12. If user asks to modify a character, use 'update_character' with character_id and changes parameters.
13. If user asks to modify world rules or setting, use 'update_world_building' with changes parameter.
14. If user asks to modify a scene structure, use 'update_scene' with scene_id and changes parameters.
15. If you need external information (research, facts, current events), use MCP tools: mcp.{{server_id}}.{{tool_name}}.
16. After updating story elements (character/world/scene), if the current content might be affected, add a 'writer' step to rewrite content with the new settings.
17. If user requests style enhancement, dialogue improvement, or emotional pacing, prefer using builtin skills over raw writer.
18. Consider active foreshadowing when planning writing steps - reference unresolved setup items to create payoff moments.
19. CRITICAL — HIGHEST PRIORITY: When the user explicitly asks to 'write a novel', 'write a story', 'start writing', '写小说', '写故事', '开始写', '写一部', or any clear prose-generation request, ALWAYS use 'writer' to generate actual prose content. Do NOT use 'outline_planner' or return conversational greetings. This rule OVERRIDES Rule 10 — even if story progress is 'just_started', a direct writing request means the user wants to see story text immediately, not planning advice.
20. If a style blend configuration is active (multiple style DNAs with weights), the writer must follow the blend rules: dominant style sets the overall tone, secondary styles permeate specific scenes (dialogue/rhythm/psychological depth/atmosphere). Do NOT ignore the blend weights.
21. DEFINITIVE PROSE CHECK: If the user input contains '写' / 'write' / '创作' / '继续' / '续写' followed by ANY story-related subject (novel/story/chapter/scene/正文/开篇/章节/网文/这部/当前), this is UNAMBIGUOUSLY a prose-generation request. Use 'writer'. Never use 'outline_planner', 'style_mimic', 'plot_analyzer', 'inspector', or 'builtin.style_enhancer' for these inputs -- 'inspector' returns a review report instead of prose, and the others return empty-content templates."#,
                context.has_story,
                context.current_story_id.as_deref().unwrap_or("none"),
                context.has_chapters,
                context.chapter_count,
                context.total_word_count,
                context.latest_chapter_word_count,
                context.story_progress,
                context.scene_count,
                current_scene_info,
                scenes_summary,
                world_building_text,
                characters_text,
                foreshadowing_text,
                deep_insight_text,
                style_dna_text,
                strategy_text,
                mcp_tools_text,
                preview_clean,
                user_input_clean,
                registry_clean
            )
        }; // 结束 else（默认动态拼接）

        // 计划生成JSON通常只需要几百tokens，1024足够，减少等待时间
        let response = self
            .llm_service
            .generate_for_task(
                TaskType::Analysis,
                prompt,
                Some(1024),
                Some(0.3),
                Some("plan_generation"),
            )
            .await?;
        self.emit_progress("parsing", "正在解析执行计划...");

        // Robust JSON extraction: find first '{' and last '}'
        let content = response.content.trim();
        let json_str = if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
            &content[start..=end]
        } else {
            // Fallback to markdown code block stripping
            content
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim()
        };

        let mut plan: ExecutionPlan = serde_json::from_str(json_str).map_err(|e| {
            AppError::validation_failed(
                format!(
                    "Failed to parse plan JSON: {}. Extracted JSON: {}",
                    e, json_str
                ),
                None::<String>,
            )
        })?;
        self.emit_progress("validating", "正在验证执行计划...");

        // 验证计划：确保所有 capability_id 在注册表中存在
        {
            let registry = get_capability_registry();
            plan.steps.retain(|step| {
                if registry.get_by_id(&step.capability_id).is_none() {
                    log::warn!(
                        "[PlanGenerator] Removing step '{}' with unknown capability '{}'",
                        step.step_id,
                        step.capability_id
                    );
                    false
                } else {
                    true
                }
            });
        }

        // 防线 2：强制修正 — 如果用户输入明确是写作请求但 LLM 选择了
        // outline_planner/style_enhancer/inspector 等，强制替换为 writer。
        // v0.30.13: 提取为 force_correct_first_step_to_writer；咽喉点
        // （execute_with_context）也调用同一方法，修补 SING 路径绕过。
        Self::force_correct_first_step_to_writer(
            &mut plan,
            context.intent_classification.as_ref(),
            &context.user_input,
        );

        Ok(plan)
    }
}

/// smart_execute 统一进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartExecuteProgress {
    pub stage: String,
    pub message: String,
    pub step_number: usize,
    pub total_steps: usize,
}

/// PlanExecutor 步骤级进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanExecutorProgress {
    pub step_id: String,
    pub capability_id: String,
    pub status: String, // running | completed | failed
    pub message: String,
    pub steps_completed: usize,
    pub total_steps: usize,
}

#[cfg(test)]
#[allow(deprecated)] // conservative_fallback() 在测试中用作 fixture
mod tests {
    use super::*;

    #[test]
    fn test_plan_step_creation() {
        let step = PlanStep {
            step_id: "step_1".to_string(),
            capability_id: "writer".to_string(),
            purpose: "Generate opening".to_string(),
            parameters: HashMap::new(),
            depends_on: vec![],
            long_running: false,
        };
        assert_eq!(step.step_id, "step_1");
        assert_eq!(step.capability_id, "writer");
    }

    #[test]
    fn test_execution_plan_default() {
        let plan: ExecutionPlan = serde_json::from_str(r#"{"steps": []}"#).unwrap();
        assert!(plan.steps.is_empty());
        assert!(plan.understanding.is_empty());
        assert!(plan.fallback_message.is_empty());
    }

    #[test]
    fn test_scene_structure_summary_has_content() {
        let summary = SceneStructureSummary {
            scene_id: "s1".to_string(),
            sequence_number: 1,
            title: Some("开篇".to_string()),
            execution_stage: Some("drafting".to_string()),
            has_content: true,
            word_count: 1500,
        };
        assert_eq!(summary.sequence_number, 1);
        assert!(summary.has_content);
    }

    #[test]
    fn test_plan_context_defaults() {
        let ctx = PlanContext {
            current_story_id: None,
            has_story: false,
            has_chapters: false,
            chapter_count: 0,
            current_content_preview: None,
            user_input: "test".to_string(),
            scene_count: 0,
            scenes_summary: vec![],
            current_scene_id: None,
            current_scene_stage: None,
            total_word_count: 0,
            latest_chapter_word_count: 0,
            story_progress: "just_started".to_string(),
            selected_text: None,
            world_building_summary: None,
            character_list: vec![],
            foreshadowing_status: vec![],
            style_dna_info: None,
            mcp_tools_available: vec![],
            deep_insight_summary: None,
            style_weight: 50,
            chapter_number: 1,
            selected_strategy: None,
            intent_classification: None,
        };
        assert!(!ctx.has_story);
        assert_eq!(ctx.story_progress, "just_started");
    }

    #[test]
    fn test_force_correct_inspector_continuation_forced_to_writer() {
        // v0.30.12 回归：续写被误路由到 inspector 必须强制改 writer
        // （用户报告"继续写当前这部小说"得到审查报告）。
        let cls = WritingIntentClassification::conservative_fallback();
        assert!(PlanGenerator::should_force_correct_to_writer(
            "inspector",
            Some(&cls)
        ));
    }

    #[test]
    fn test_force_correct_inspector_audit_kept() {
        // 显式审查请求（非 prose）保留 inspector（Rule 9 合法用途）。
        let cls = WritingIntentClassification {
            is_continuation: false,
            task_type: AssetTaskType::Audit,
            is_prose_request: false,
            ..WritingIntentClassification::conservative_fallback()
        };
        assert!(!PlanGenerator::should_force_correct_to_writer(
            "inspector",
            Some(&cls)
        ));
    }

    #[test]
    fn test_force_correct_inspector_audit_with_prose_forced_to_writer() {
        // v0.30.12 防误判：分类矛盾时（task_type=Audit 但 is_prose_request=true，
        // 如本地模型把"继续写"误判为 Audit），强制 writer--续写绝不该返回审查报告。
        let cls = WritingIntentClassification {
            is_continuation: false,
            task_type: AssetTaskType::Audit,
            is_prose_request: true,
            ..WritingIntentClassification::conservative_fallback()
        };
        assert!(PlanGenerator::should_force_correct_to_writer(
            "inspector",
            Some(&cls)
        ));
    }

    #[test]
    fn test_force_correct_inspector_rewrite_kept() {
        // 改写润色（Rule 9 inspector->writer 流）保留 inspector。
        let cls = WritingIntentClassification {
            is_continuation: false,
            task_type: AssetTaskType::Rewrite,
            ..WritingIntentClassification::conservative_fallback()
        };
        assert!(!PlanGenerator::should_force_correct_to_writer(
            "inspector",
            Some(&cls)
        ));
    }

    #[test]
    fn test_force_correct_inspector_genesis_forced_to_writer() {
        // 创世不该走 inspector。
        let cls = WritingIntentClassification {
            is_continuation: false,
            task_type: AssetTaskType::Genesis,
            ..WritingIntentClassification::conservative_fallback()
        };
        assert!(PlanGenerator::should_force_correct_to_writer(
            "inspector",
            Some(&cls)
        ));
    }

    #[test]
    fn test_force_correct_inspector_no_classification_forced_to_writer() {
        // 无分类安全默认 writer（续写误路由代价 >> 审查被改 writer 的代价）。
        assert!(PlanGenerator::should_force_correct_to_writer(
            "inspector",
            None
        ));
    }

    #[test]
    fn test_force_correct_outline_prose_request_forced_to_writer() {
        // 回归 v0.30.11：is_prose_request=true 时 outline_planner -> writer。
        let cls = WritingIntentClassification::conservative_fallback();
        assert!(PlanGenerator::should_force_correct_to_writer(
            "outline_planner",
            Some(&cls)
        ));
    }

    #[test]
    fn test_force_correct_writer_not_corrected() {
        // writer 不该被修正。
        let cls = WritingIntentClassification::conservative_fallback();
        assert!(!PlanGenerator::should_force_correct_to_writer(
            "writer",
            Some(&cls)
        ));
    }

    #[test]
    fn test_force_correct_method_sing_style_enhancer_corrected() {
        // v0.30.13 回归：SING 路径产生的 builtin.style_enhancer 首步经咽喉点
        // force_correct_first_step_to_writer 修正为 writer（用户报告"继续写"得到
        // "请提供需要增强的原始文本"模板，根因是 SING 绕过 generate_plan 内防线）。
        let mut plan = ExecutionPlan {
            understanding: "sing plan: enhance style".to_string(),
            steps: vec![PlanStep {
                step_id: "ig_step_1".to_string(),
                capability_id: "builtin.style_enhancer".to_string(),
                purpose: "enhance style".to_string(),
                parameters: HashMap::new(),
                depends_on: vec![],
                long_running: false,
            }],
            fallback_message: String::new(),
        };
        let cls = WritingIntentClassification {
            is_continuation: true,
            is_prose_request: true,
            ..WritingIntentClassification::conservative_fallback()
        };
        PlanGenerator::force_correct_first_step_to_writer(&mut plan, Some(&cls), "继续写");
        assert_eq!(plan.steps[0].capability_id, "writer");
        assert!(plan.understanding.contains("auto-corrected"));
        assert!(plan.understanding.contains("builtin.style_enhancer"));
    }

    #[test]
    fn test_force_correct_method_writer_plan_untouched() {
        // v0.30.13：已为 writer 的首步幂等不改动（understanding 不变）。
        let mut plan = ExecutionPlan {
            understanding: "writer plan".to_string(),
            steps: vec![PlanStep {
                step_id: "s1".to_string(),
                capability_id: "writer".to_string(),
                purpose: "write prose".to_string(),
                parameters: HashMap::new(),
                depends_on: vec![],
                long_running: false,
            }],
            fallback_message: String::new(),
        };
        PlanGenerator::force_correct_first_step_to_writer(&mut plan, None, "继续写");
        assert_eq!(plan.steps[0].capability_id, "writer");
        assert_eq!(plan.understanding, "writer plan");
    }

    #[test]
    fn test_force_correct_method_empty_plan_noop() {
        // v0.30.13：空 plan 不 panic。
        let mut plan = ExecutionPlan {
            understanding: "empty".to_string(),
            steps: vec![],
            fallback_message: String::new(),
        };
        PlanGenerator::force_correct_first_step_to_writer(&mut plan, None, "继续写");
        assert!(plan.steps.is_empty());
    }

    #[test]
    fn test_force_correct_method_inspector_continuation_corrected() {
        // v0.30.13：咽喉点也覆盖 inspector 误路由（v0.30.12 场景在 SING 路径复现）。
        let mut plan = ExecutionPlan {
            understanding: "sing plan: inspect".to_string(),
            steps: vec![PlanStep {
                step_id: "ig_step_1".to_string(),
                capability_id: "inspector".to_string(),
                purpose: "review".to_string(),
                parameters: HashMap::new(),
                depends_on: vec![],
                long_running: false,
            }],
            fallback_message: String::new(),
        };
        let cls = WritingIntentClassification {
            is_continuation: true,
            ..WritingIntentClassification::conservative_fallback()
        };
        PlanGenerator::force_correct_first_step_to_writer(&mut plan, Some(&cls), "继续写");
        assert_eq!(plan.steps[0].capability_id, "writer");
    }

    // ---- v0.30.14 防线 3：sanitize_plan_for_prose_request 回归 ----

    fn make_sanitize_ctx(user_input: &str, cls: WritingIntentClassification) -> PlanContext {
        PlanContext {
            current_story_id: Some("story_1".to_string()),
            has_story: true,
            has_chapters: true,
            chapter_count: 2,
            current_content_preview: Some("已有正文...".to_string()),
            user_input: user_input.to_string(),
            scene_count: 1,
            scenes_summary: vec![],
            current_scene_id: None,
            current_scene_stage: None,
            total_word_count: 1000,
            latest_chapter_word_count: 500,
            story_progress: "in_progress".to_string(),
            selected_text: None,
            world_building_summary: None,
            character_list: vec![],
            foreshadowing_status: vec![],
            style_dna_info: None,
            mcp_tools_available: vec![],
            deep_insight_summary: None,
            style_weight: 50,
            chapter_number: 2,
            selected_strategy: None,
            intent_classification: Some(cls),
        }
    }

    fn make_step(step_id: &str, cap: &str) -> PlanStep {
        PlanStep {
            step_id: step_id.to_string(),
            capability_id: cap.to_string(),
            purpose: cap.to_string(),
            parameters: HashMap::new(),
            depends_on: vec![],
            long_running: false,
        }
    }

    fn make_plan(steps: Vec<PlanStep>) -> ExecutionPlan {
        ExecutionPlan {
            understanding: "test plan".to_string(),
            steps,
            fallback_message: String::new(),
        }
    }

    #[test]
    fn test_sanitize_inspector_then_style_enhancer_prose() {
        // v0.30.14 核心回归：用户报告"增强第二章"得到 [inspector, style_enhancer]
        // 多步 plan，尾部 style_enhancer 用"请提供原始文本"模板覆盖正文。净化后末步
        // 必须为 writer。
        let cls = WritingIntentClassification {
            is_continuation: false,
            is_prose_request: true,
            task_type: AssetTaskType::Other,
            ..WritingIntentClassification::conservative_fallback()
        };
        let ctx = make_sanitize_ctx("增强第二章的文学性", cls);
        let mut plan = make_plan(vec![
            make_step("s1", "inspector"),
            make_step("s2", "builtin.style_enhancer"),
        ]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        assert!(!plan.steps.is_empty());
        assert_eq!(plan.steps.last().unwrap().capability_id, "writer");
        // style_enhancer 必须被移除
        assert!(plan
            .steps
            .iter()
            .all(|s| !s.capability_id.contains("style_enhancer")));
    }

    #[test]
    fn test_sanitize_style_enhancer_only_prose() {
        let cls = WritingIntentClassification::conservative_fallback();
        let ctx = make_sanitize_ctx("继续写", cls);
        let mut plan = make_plan(vec![make_step("s1", "builtin.style_enhancer")]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        // 续写 -> 塌缩单 writer
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].capability_id, "writer");
    }

    #[test]
    fn test_sanitize_writer_then_style_enhancer_prose() {
        // [writer, style_enhancer]：移除尾部 style_enhancer，保留 writer。
        let cls = WritingIntentClassification {
            is_continuation: false,
            is_prose_request: true,
            task_type: AssetTaskType::Other,
            ..WritingIntentClassification::conservative_fallback()
        };
        let ctx = make_sanitize_ctx("增强第二章", cls);
        let mut plan = make_plan(vec![
            make_step("s1", "writer"),
            make_step("s2", "builtin.style_enhancer"),
        ]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].capability_id, "writer");
    }

    #[test]
    fn test_sanitize_inspector_writer_rewrite_preserved() {
        // Rule 9 合法流 [inspector, writer]（改写：先审后写，末步 writer）必须保留。
        let cls = WritingIntentClassification {
            is_continuation: false,
            is_prose_request: true,
            task_type: AssetTaskType::Rewrite,
            ..WritingIntentClassification::conservative_fallback()
        };
        let ctx = make_sanitize_ctx("改写第二章", cls);
        let mut plan = make_plan(vec![
            make_step("s1", "inspector"),
            make_step("s2", "writer"),
        ]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].capability_id, "inspector");
        assert_eq!(plan.steps[1].capability_id, "writer");
    }

    #[test]
    fn test_sanitize_continuation_multi_step_collapsed() {
        // 续写本质单步；多步必为误路由 -> 塌缩单 writer。
        let cls = WritingIntentClassification::conservative_fallback(); // is_continuation=true
        let ctx = make_sanitize_ctx("继续写当前这部小说", cls);
        let mut plan = make_plan(vec![
            make_step("s1", "inspector"),
            make_step("s2", "writer"),
        ]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].capability_id, "writer");
        assert!(plan.understanding.contains("collapsed"));
    }

    #[test]
    fn test_sanitize_continuation_single_writer_unchanged() {
        let cls = WritingIntentClassification::conservative_fallback();
        let ctx = make_sanitize_ctx("继续写", cls);
        let mut plan = make_plan(vec![make_step("s1", "writer")]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].capability_id, "writer");
    }

    #[test]
    fn test_sanitize_audit_not_purged() {
        // 显式审查（is_prose_request=false）保留 inspector--审查报告是用户想要的。
        let cls = WritingIntentClassification {
            is_continuation: false,
            is_prose_request: false,
            task_type: AssetTaskType::Audit,
            ..WritingIntentClassification::conservative_fallback()
        };
        let ctx = make_sanitize_ctx("检查第二章的逻辑问题", cls);
        let mut plan = make_plan(vec![make_step("s1", "inspector")]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].capability_id, "inspector");
    }

    #[test]
    fn test_sanitize_outline_writer_preserved() {
        // [outline_planner, writer]：末步 writer，保留（outline 作为中间步合法）。
        let cls = WritingIntentClassification {
            is_continuation: false,
            is_prose_request: true,
            task_type: AssetTaskType::Other,
            ..WritingIntentClassification::conservative_fallback()
        };
        let ctx = make_sanitize_ctx("写第二章", cls);
        let mut plan = make_plan(vec![
            make_step("s1", "outline_planner"),
            make_step("s2", "writer"),
        ]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[1].capability_id, "writer");
    }

    #[test]
    fn test_sanitize_outline_only_prose_becomes_writer() {
        // [outline_planner]：末步非 writer -> 弹出 -> 空 -> 补 writer。
        let cls = WritingIntentClassification {
            is_continuation: false,
            is_prose_request: true,
            task_type: AssetTaskType::Other,
            ..WritingIntentClassification::conservative_fallback()
        };
        let ctx = make_sanitize_ctx("写第二章", cls);
        let mut plan = make_plan(vec![make_step("s1", "outline_planner")]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].capability_id, "writer");
    }

    #[test]
    fn test_sanitize_empty_prose_gets_writer() {
        let cls = WritingIntentClassification::conservative_fallback();
        let ctx = make_sanitize_ctx("继续写", cls);
        let mut plan = make_plan(vec![]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].capability_id, "writer");
    }

    #[test]
    fn test_sanitize_no_classification_unchanged() {
        // 无分类不净化（保守，交由 force-correction 兜底）。
        let mut plan = make_plan(vec![
            make_step("s1", "inspector"),
            make_step("s2", "builtin.style_enhancer"),
        ]);
        let ctx = make_sanitize_ctx(
            "继续写",
            WritingIntentClassification::conservative_fallback(),
        );
        PlanGenerator::sanitize_plan_for_prose_request(&mut plan, None, &ctx);
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[1].capability_id, "builtin.style_enhancer");
    }

    #[test]
    fn test_sanitize_sanitized_writer_step_has_instruction() {
        // 净化补的 writer 步必须带 instruction（用户输入）+ current_content。
        let cls = WritingIntentClassification::conservative_fallback();
        let ctx = make_sanitize_ctx("继续写这部小说", cls);
        let mut plan = make_plan(vec![]);
        PlanGenerator::sanitize_plan_for_prose_request(
            &mut plan,
            ctx.intent_classification.as_ref(),
            &ctx,
        );
        let w = &plan.steps[0];
        assert_eq!(w.capability_id, "writer");
        assert_eq!(
            w.parameters.get("instruction").and_then(|v| v.as_str()),
            Some("继续写这部小说")
        );
        assert_eq!(
            w.parameters.get("current_content").and_then(|v| v.as_str()),
            Some("已有正文...")
        );
        assert_eq!(
            w.parameters.get("story_id").and_then(|v| v.as_str()),
            Some("story_1")
        );
    }
}
