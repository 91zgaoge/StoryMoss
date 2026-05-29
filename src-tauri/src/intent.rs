//! Intent Parser - 意图解析引擎
//!
//! 将创作者的自然语言输入解析为结构化意图，
//! 驱动 workflow::scheduler 调用正确的 Agent 执行创作任务。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use uuid::Uuid;

use crate::{
    agents::{
        service::{AgentService, AgentTask, AgentType},
        AgentContext, AgentResult,
    },
    llm::{GenerateResponse, LlmService},
};

/// 意图类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum IntentType {
    TextGenerate,
    TextRewrite,
    PlotSuggest,
    CharacterCheck,
    WorldConsistency,
    StyleShift,
    MemoryIngest,
    VisualGenerate,
    SceneReorder,
    OutlineExpand,
    Unknown,
}

/// 执行模式
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Serial,
    Parallel,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::Serial
    }
}

/// 反馈类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    DirectApply,
    SuggestionCard,
    DiffPreview,
    SystemNotice,
    VisualHighlight,
}

impl Default for FeedbackType {
    fn default() -> Self {
        FeedbackType::SuggestionCard
    }
}

/// 意图目标
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IntentTarget {
    pub target_type: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
}

/// 结构化意图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    #[serde(rename = "intent_type")]
    pub intent_type: IntentType,
    #[serde(default)]
    pub target: IntentTarget,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub required_agents: Vec<String>,
    #[serde(default)]
    pub execution_mode: ExecutionMode,
    #[serde(default)]
    pub feedback_type: FeedbackType,
    /// 原始用户输入（补充字段，不由LLM生成）
    #[serde(skip)]
    pub raw_input: String,
}

impl Intent {
    pub fn unknown(raw_input: impl Into<String>) -> Self {
        Self {
            intent_type: IntentType::Unknown,
            target: IntentTarget::default(),
            constraints: vec![],
            required_agents: vec![],
            execution_mode: ExecutionMode::default(),
            feedback_type: FeedbackType::default(),
            raw_input: raw_input.into(),
        }
    }
}

/// 意图解析器
pub struct IntentParser {
    llm_service: LlmService,
}

impl IntentParser {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            llm_service: LlmService::new(app_handle),
        }
    }

    /// 解析用户输入为结构化意图
    pub async fn parse(&self, user_input: &str) -> Result<Intent, String> {
        let prompt = Self::build_intent_prompt(user_input);

        match self
            .llm_service
            .generate(prompt, Some(512), Some(0.1))
            .await
        {
            Ok(GenerateResponse { content, .. }) => Self::parse_intent_json(&content, user_input),
            Err(e) => {
                log::error!("[IntentParser] LLM generation failed: {}", e);
                Ok(Intent::unknown(user_input))
            }
        }
    }

    fn build_intent_prompt(user_input: &str) -> String {
        format!(
            r#"你是一个专业的创作助手意图解析器。请将用户的输入解析为固定的 JSON 格式。

可识别的意图类型 (intent_type):
- text_generate: 文本续写、扩展内容、从头开始创作新内容。用户使用"写"、"创作"、"生成"、"续"、"扩"、"补"、"开篇"、"开头"等词时，必须识别为 text_generate
- text_rewrite: 改写、润色已有文本
- plot_suggest: 情节建议、反转设计、剧情推进
- character_check: 角色一致性检查、角色动机分析
- world_consistency: 世界设定一致性检查
- style_shift: 文风切换、文风模仿
- memory_ingest: 知识摄取、更新记忆
- visual_generate: 生成图像、概念图
- scene_reorder: 场景结构调整、排序
- outline_expand: 大纲扩展（仅在用户明确要求扩展大纲时使用，不要与 text_generate 混淆）
- unknown: 无法识别或闲聊

执行模式 (execution_mode):
- serial: 串行执行（默认）
- parallel: 并行执行

反馈类型 (feedback_type):
- direct_apply: 直接修改（适用于续写、创作）
- suggestion_card: 建议卡片（适用于情节建议）
- diff_preview: Diff预览（适用于改写）
- system_notice: 系统通知（适用于异步任务）
- visual_highlight: 可视化高亮（适用于检查结果）

可用 Agent (required_agents):
- writer: 写作助手，用于 text_generate/text_rewrite
- style_mimic: 风格模仿师
- plot_analyzer: 情节分析师
- outline_planner: 大纲规划师
- character_agent: 角色分析 Agent
- world_building_agent: 世界观 Agent
- memory_agent: 记忆 Agent
- inspector: 质检员

关键规则:
1. 必须且只能返回合法的 JSON，不要包含 markdown 代码块标记。
2. 用户说"写一篇..."、"创作一个..."、"生成..."等明确请求生成文字内容时，intent_type 必须是 text_generate，required_agents 必须包含 writer，feedback_type 必须是 direct_apply。
3. 用户说"帮我想个..."、"给点建议"等请求思路时，intent_type 是 plot_suggest，feedback_type 是 suggestion_card。
4. target 字段用于指明操作对象，如场景、角色等。target_type 可选值: scene, character, story, paragraph。
5. constraints 是用户对结果的具体约束条件列表。
6. 如果用户只是打招呼或闲聊，返回 intent_type: unknown。

JSON Schema:
{{
  "intent_type": "string",
  "target": {{
    "target_type": "string | null",
    "id": "string | null",
    "name": "string | null"
  }},
  "constraints": ["string"],
  "required_agents": ["string"],
  "execution_mode": "serial | parallel",
  "feedback_type": "direct_apply | suggestion_card | diff_preview | system_notice | visual_highlight"
}}

用户输入: "{}"

请直接输出 JSON:"#,
            user_input
        )
    }

    fn parse_intent_json(content: &str, user_input: &str) -> Result<Intent, String> {
        // 尝试清理可能存在的 markdown 代码块
        let json_str = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        match serde_json::from_str::<Intent>(json_str) {
            Ok(mut intent) => {
                intent.raw_input = user_input.to_string();
                Ok(intent)
            }
            Err(e) => {
                log::warn!(
                    "[IntentParser] Failed to parse JSON: {}. Raw content: {}",
                    e,
                    content
                );
                Ok(Intent::unknown(user_input))
            }
        }
    }
}

/// Agent 执行步骤结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStepResult {
    pub agent_name: String,
    pub success: bool,
    pub result: Option<AgentResult>,
    pub error: Option<String>,
}

/// 意图执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentExecutionResult {
    pub intent_type: IntentType,
    pub feedback_type: FeedbackType,
    pub execution_mode: ExecutionMode,
    pub steps: Vec<AgentStepResult>,
    pub summary: String,
}

/// 意图执行器 - 将解析后的意图调度到具体 Agent 执行
pub struct IntentExecutor {
    agent_service: AgentService,
}

impl IntentExecutor {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            agent_service: AgentService::new(app_handle),
        }
    }

    /// 执行意图对应的 Agent 任务
    pub async fn execute(
        &self,
        intent: Intent,
        story_id: String,
    ) -> Result<IntentExecutionResult, String> {
        let agents = Self::map_agents(&intent.required_agents);

        if agents.is_empty() {
            return Ok(IntentExecutionResult {
                intent_type: intent.intent_type.clone(),
                feedback_type: intent.feedback_type.clone(),
                execution_mode: intent.execution_mode.clone(),
                steps: vec![],
                summary: "暂无可执行的相关 Agent，已回退到对话模式。".to_string(),
            });
        }

        let context = Self::build_context(&story_id, &intent, self.agent_service.app_handle());
        let steps = match intent.execution_mode {
            ExecutionMode::Serial => self.execute_serial(agents, context, &intent).await,
            ExecutionMode::Parallel => self.execute_parallel(agents, context, &intent).await,
        };

        let summary = Self::build_summary(&intent, &steps);

        Ok(IntentExecutionResult {
            intent_type: intent.intent_type,
            feedback_type: intent.feedback_type,
            execution_mode: intent.execution_mode,
            steps,
            summary,
        })
    }

    /// 将 agent 名称字符串映射到 AgentType
    fn map_agents(agent_names: &[String]) -> Vec<AgentType> {
        agent_names
            .iter()
            .filter_map(|name| match name.as_str() {
                "writer" => Some(AgentType::Writer),
                "style_mimic" => Some(AgentType::StyleMimic),
                "plot_analyzer" => Some(AgentType::PlotAnalyzer),
                "outline_planner" => Some(AgentType::OutlinePlanner),
                "inspector" => Some(AgentType::Inspector),
                // 以下 agent 尚未实现独立类型，暂时映射到最接近的实现
                "character_agent" => Some(AgentType::Inspector),
                "world_building_agent" => Some(AgentType::Inspector),
                "memory_agent" => Some(AgentType::Writer),
                _ => None,
            })
            .collect()
    }

    /// 构建 Agent 执行上下文
    ///
    /// 使用 StoryContextBuilder 从数据库读取真实故事数据，
    /// 替代原有的硬编码默认值。
    fn build_context(
        story_id: &str,
        _intent: &Intent,
        app_handle: &tauri::AppHandle,
    ) -> AgentContext {
        use tauri::Manager;

        use crate::{creative_engine::StoryContextBuilder, db::DbPool};

        match app_handle.try_state::<DbPool>() {
            Some(pool_state) => {
                let pool = pool_state.inner().clone();
                let builder = StoryContextBuilder::new(pool);
                match builder.build_quick(story_id) {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        log::warn!(
                            "[IntentExecutor] Failed to build context from DB: {}, falling back \
                             to minimal",
                            e
                        );
                        AgentContext::minimal(story_id.to_string(), String::new())
                    }
                }
            }
            None => {
                log::warn!("[IntentExecutor] DbPool not available, using minimal context");
                AgentContext::minimal(story_id.to_string(), String::new())
            }
        }
    }

    /// 串行执行
    async fn execute_serial(
        &self,
        agents: Vec<AgentType>,
        context: AgentContext,
        intent: &Intent,
    ) -> Vec<AgentStepResult> {
        let mut steps = Vec::new();
        let mut current_input = intent.raw_input.clone();

        for agent in agents {
            let task = AgentTask {
                id: Uuid::new_v4().to_string(),
                agent_type: agent,
                context: context.clone(),
                input: current_input.clone(),
                parameters: Self::build_parameters(intent),
                tier: None,
            };

            match self.agent_service.execute_task(task).await {
                Ok(result) => {
                    current_input = result.content.clone();
                    steps.push(AgentStepResult {
                        agent_name: agent.name().to_string(),
                        success: true,
                        result: Some(result),
                        error: None,
                    });
                }
                Err(e) => {
                    steps.push(AgentStepResult {
                        agent_name: agent.name().to_string(),
                        success: false,
                        result: None,
                        error: Some(e.to_string()),
                    });
                    // 串行模式下遇到错误可选择中断，这里继续记录但停止传递输入
                    break;
                }
            }
        }

        steps
    }

    /// 并行执行
    async fn execute_parallel(
        &self,
        agents: Vec<AgentType>,
        context: AgentContext,
        intent: &Intent,
    ) -> Vec<AgentStepResult> {
        let mut handles = Vec::new();
        let service = self.agent_service.clone();

        for agent in agents {
            let task = AgentTask {
                id: Uuid::new_v4().to_string(),
                agent_type: agent,
                context: context.clone(),
                input: intent.raw_input.clone(),
                parameters: Self::build_parameters(intent),
                tier: None,
            };

            let service_clone = service.clone();
            let handle = tokio::spawn(async move {
                match service_clone.execute_task(task).await {
                    Ok(result) => AgentStepResult {
                        agent_name: agent.name().to_string(),
                        success: true,
                        result: Some(result),
                        error: None,
                    },
                    Err(e) => AgentStepResult {
                        agent_name: agent.name().to_string(),
                        success: false,
                        result: None,
                        error: Some(e.to_string()),
                    },
                }
            });
            handles.push(handle);
        }

        let mut steps = Vec::new();
        for handle in handles {
            if let Ok(step) = handle.await {
                steps.push(step);
            }
        }

        steps
    }

    /// 构建额外参数
    fn build_parameters(intent: &Intent) -> HashMap<String, serde_json::Value> {
        let mut params = HashMap::new();
        if let Some(target_type) = &intent.target.target_type {
            params.insert("target_type".to_string(), serde_json::json!(target_type));
        }
        if let Some(target_id) = &intent.target.id {
            params.insert("target_id".to_string(), serde_json::json!(target_id));
        }
        if let Some(target_name) = &intent.target.name {
            params.insert("target_name".to_string(), serde_json::json!(target_name));
        }
        if !intent.constraints.is_empty() {
            params.insert(
                "constraints".to_string(),
                serde_json::json!(intent.constraints),
            );
        }
        params
    }

    /// 构建执行结果摘要
    /// 优先返回最后一个成功 Agent 的实际生成内容，让用户看到有用的结果
    fn build_summary(intent: &Intent, steps: &[AgentStepResult]) -> String {
        let success_count = steps.iter().filter(|s| s.success).count();
        let total_count = steps.len();

        if total_count == 0 {
            return "未执行任何 Agent 任务。".to_string();
        }

        // 优先返回最后一个成功 Agent 的实际内容
        if let Some(last_success) = steps.iter().rev().find(|s| s.success) {
            if let Some(ref result) = last_success.result {
                if !result.content.is_empty() {
                    return result.content.clone();
                }
            }
        }

        // 回退到状态摘要
        if success_count == total_count {
            format!(
                "{} 意图已完全执行，共调用 {} 个 Agent。",
                Self::intent_display_name(&intent.intent_type),
                total_count
            )
        } else {
            format!(
                "{} 意图部分执行，成功 {}/{}。",
                Self::intent_display_name(&intent.intent_type),
                success_count,
                total_count
            )
        }
    }

    fn intent_display_name(intent_type: &IntentType) -> &'static str {
        match intent_type {
            IntentType::TextGenerate => "续写生成",
            IntentType::TextRewrite => "文本改写",
            IntentType::PlotSuggest => "情节建议",
            IntentType::CharacterCheck => "角色检查",
            IntentType::WorldConsistency => "世界观检查",
            IntentType::StyleShift => "文风切换",
            IntentType::MemoryIngest => "知识摄取",
            IntentType::VisualGenerate => "视觉生成",
            IntentType::SceneReorder => "场景调整",
            IntentType::OutlineExpand => "大纲扩展",
            IntentType::Unknown => "自由对话",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_intent_json() {
        let json = r#"{
            "intent_type": "text_rewrite",
            "target": {"target_type": "scene", "id": "scene_2", "name": null},
            "constraints": ["增强紧张感", "保持 K-7 语气"],
            "required_agents": ["writer", "style_mimic"],
            "execution_mode": "serial",
            "feedback_type": "diff_preview"
        }"#;

        let intent = IntentParser::parse_intent_json(json, "把 Scene 2 改得更紧张").unwrap();
        assert_eq!(intent.intent_type, IntentType::TextRewrite);
        assert_eq!(intent.target.target_type, Some("scene".to_string()));
        assert_eq!(intent.target.id, Some("scene_2".to_string()));
        assert_eq!(intent.constraints.len(), 2);
        assert_eq!(intent.required_agents, vec!["writer", "style_mimic"]);
        assert_eq!(intent.execution_mode, ExecutionMode::Serial);
        assert_eq!(intent.feedback_type, FeedbackType::DiffPreview);
    }

    #[test]
    fn test_parse_intent_json_with_markdown() {
        let json = "```json\n{\"intent_type\": \"plot_suggest\", \"target\": {}, \"constraints\": \
                    [], \"required_agents\": [\"plot_analyzer\"], \"execution_mode\": \"serial\", \
                    \"feedback_type\": \"suggestion_card\"}\n```";

        let intent = IntentParser::parse_intent_json(json, "帮我想个反转").unwrap();
        assert_eq!(intent.intent_type, IntentType::PlotSuggest);
    }

    #[test]
    fn test_parse_intent_json_fallback() {
        let invalid = "这不是 JSON";
        let intent = IntentParser::parse_intent_json(invalid, "你好").unwrap();
        assert_eq!(intent.intent_type, IntentType::Unknown);
    }

    #[test]
    fn test_map_agents() {
        let agents = IntentExecutor::map_agents(&vec![
            "writer".to_string(),
            "plot_analyzer".to_string(),
            "unknown_agent".to_string(),
        ]);
        assert_eq!(agents.len(), 2);
        assert!(matches!(agents[0], AgentType::Writer));
        assert!(matches!(agents[1], AgentType::PlotAnalyzer));
    }

    #[test]
    fn test_build_summary() {
        let intent = Intent::unknown("测试");
        let steps = vec![AgentStepResult {
            agent_name: "Writer".to_string(),
            success: true,
            result: None,
            error: None,
        }];
        let summary = IntentExecutor::build_summary(&intent, &steps);
        assert!(summary.contains("自由对话"));
        assert!(summary.contains("1 个 Agent"));
    }
}
