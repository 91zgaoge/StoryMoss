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
    capabilities::get_capability_registry, error::AppError, llm::LlmService, router::TaskType,
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
9. If the user wants to improve/refine text and there IS content, use 'inspector' first then 'writer'. CRITICAL: When using 'inspector', you MUST pass the text to check as the "draft" parameter using {{{{step_id}}}} syntax (e.g. "draft": "{{{{step_1}}}}" to check the output of step_1). The inspector CANNOT function without a draft -- if you omit it, the inspector receives empty content and returns a request for input instead of an actual review.
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
21. DEFINITIVE PROSE CHECK: If the user input contains '写' / 'write' / '创作' followed by ANY story-related subject (novel/story/chapter/scene/正文/开篇/章节/网文), this is UNAMBIGUOUSLY a prose-generation request. Use 'writer'. Never use 'outline_planner' for these inputs."#,
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
        // outline_planner，强制替换为 writer
        if !plan.steps.is_empty() && plan.steps[0].capability_id == "outline_planner" {
            let input_lower = context.user_input.to_lowercase();
            let prose_keywords = [
                "写",
                "write",
                "创作",
                "开始写",
                "写小说",
                "写故事",
                "写一章",
                "写开篇",
                "写正文",
                "start writing",
                "write a novel",
                "write a story",
                "write chapter",
                "begin writing",
            ];
            let is_prose_request = prose_keywords.iter().any(|&kw| input_lower.contains(kw));
            if is_prose_request {
                log::warn!(
                    "[PlanGenerator] Force-correcting outline_planner → writer for prose request: \
                     {}",
                    context.user_input
                );
                plan.steps[0].capability_id = "writer".to_string();
                plan.steps[0].purpose = "Auto-corrected: user wants prose generation, not \
                                         structural planning"
                    .to_string();
                plan.understanding = format!(
                    "{} [auto-corrected: prose-generation keywords detected in user input, \
                     forcing writer instead of outline_planner]",
                    plan.understanding
                );
            }
        }

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
        };
        assert!(!ctx.has_story);
        assert_eq!(ctx.story_progress, "just_started");
    }
}
