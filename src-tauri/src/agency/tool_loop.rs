use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::agency::models::AgentRole;
use crate::agency::tools::{ToolContext, ToolRegistry};
use crate::error::AppError;
use crate::router::TaskType;

/// 工具循环所需的极简 LLM 抽象（可 mock）。
/// 生产实现见 coordinator.rs 的 AgencyLlm。
#[async_trait::async_trait]
pub trait LoopLlm: Send + Sync {
    async fn complete(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopAction {
    Tool { name: String, #[serde(default)] args: serde_json::Value },
    Final { content: String },
}

/// 解析模型输出为 action。容错策略：截取首个 '{' 与末个 '}' 之间的子串解析。
pub fn parse_action(raw: &str) -> Result<LoopAction, AppError> {
    let start = raw.find('{');
    let end = raw.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if e > s => {
            serde_json::from_str::<LoopAction>(&raw[s..=e])
                .map_err(|err| AppError::validation_failed(format!("action JSON 解析失败: {}", err), None::<String>))
        }
        _ => Err(AppError::validation_failed("输出中未找到 JSON action", None::<String>)),
    }
}

#[derive(Debug, Clone)]
pub struct LoopTurn {
    pub raw_response: String,
    pub action: Option<LoopAction>,
    pub observation: Option<String>,
}

#[derive(Debug)]
pub struct LoopResult {
    pub output: String,
    pub turns: Vec<LoopTurn>,
    pub aborted: bool,
}

const MAX_CONSECUTIVE_PARSE_FAILURES: usize = 3;

pub struct ToolLoop {
    llm: Arc<dyn LoopLlm>,
    registry: Arc<ToolRegistry>,
    max_turns: usize,
}

impl ToolLoop {
    pub fn new(llm: Arc<dyn LoopLlm>, registry: Arc<ToolRegistry>) -> Self {
        Self { llm, registry, max_turns: 8 }
    }

    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// ReAct 循环：模型每轮输出一个 JSON action（tool 调用或 final），
    /// 工具执行结果作为 observation 回灌对话，直到 final 或熔断。
    pub async fn run(
        &self,
        role: AgentRole,
        ctx: &ToolContext,
        system_prompt: &str,
        task: &str,
    ) -> Result<LoopResult, AppError> {
        let mut conversation = format!(
            "{}\n\n你只能输出一个 JSON action，不要输出其他内容：\n\
             - 调用工具: {{\"type\":\"tool\",\"name\":\"<工具名>\",\"args\":{{...}}}}\n\
             - 完成任务: {{\"type\":\"final\",\"content\":\"<最终产出>\"}}\n\n任务：\n{}",
            self.registry.catalog_for_role(role),
            task
        );
        let mut turns: Vec<LoopTurn> = Vec::new();
        let mut parse_failures = 0usize;

        for _ in 0..self.max_turns {
            let raw = self.llm
                .complete(system_prompt, &conversation, ctx.task_type(), ctx.max_output_tokens())
                .await?;
            match parse_action(&raw) {
                Ok(LoopAction::Final { content }) => {
                    turns.push(LoopTurn { raw_response: raw, action: Some(LoopAction::Final { content: content.clone() }), observation: None });
                    return Ok(LoopResult { output: content, turns, aborted: false });
                }
                Ok(LoopAction::Tool { name, args }) => {
                    parse_failures = 0;
                    let observation = match self.registry.get_for_role(role, &name) {
                        Some(tool) => match tool.execute(ctx, args).await {
                            Ok(out) => {
                                // observation 摘要化：防止超长工具结果爆上下文
                                if out.chars().count() > 4000 {
                                    out.chars().take(4000).collect::<String>() + "\n... (已截断)"
                                } else {
                                    out
                                }
                            }
                            Err(e) => format!("工具 {} 执行失败: {}", name, e),
                        },
                        None => format!("工具 {} 对你的角色不可用或不存在，请改用可用工具", name),
                    };
                    conversation.push_str(&format!("\n\n你的上一步：{}\n观察结果：{}", raw, observation));
                    turns.push(LoopTurn { raw_response: raw, action: Some(LoopAction::Tool { name, args: serde_json::Value::Null }), observation: Some(observation) });
                }
                Err(e) => {
                    parse_failures += 1;
                    let observation = format!("格式错误（{}）。请只输出一个 JSON action。", e);
                    conversation.push_str(&format!("\n\n你的上一步：{}\n观察结果：{}", raw, observation));
                    turns.push(LoopTurn { raw_response: raw, action: None, observation: Some(observation) });
                    if parse_failures >= MAX_CONSECUTIVE_PARSE_FAILURES {
                        return Ok(LoopResult {
                            output: "（代理连续输出非法格式，已熔断）".to_string(),
                            turns,
                            aborted: true,
                        });
                    }
                }
            }
        }
        Ok(LoopResult {
            output: "（达到最大轮数，已熔断）".to_string(),
            turns,
            aborted: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::board::BlackboardService;
    use crate::agency::models::*;
    use crate::agency::repository::AgencyRepository;
    use crate::agency::tools::ToolRegistry;
    use crate::db::create_test_pool;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MockLlm {
        responses: Mutex<VecDeque<String>>,
    }

    impl MockLlm {
        fn scripted(lines: Vec<&str>) -> Arc<Self> {
            Arc::new(Self { responses: Mutex::new(lines.into_iter().map(String::from).collect()) })
        }
    }

    #[async_trait::async_trait]
    impl LoopLlm for MockLlm {
        async fn complete(&self, _s: &str, _u: &str, _t: TaskType, _m: i32) -> Result<String, AppError> {
            self.responses.lock().unwrap().pop_front()
                .ok_or_else(|| AppError::validation_failed("mock exhausted", None::<String>))
        }
    }

    fn setup() -> (ToolContext, Arc<ToolRegistry>) {
        let pool = create_test_pool().unwrap();
        AgencyRepository::new(pool.clone()).create_run(&AgencyRun::new("r1", "前提")).unwrap();
        let ctx = ToolContext {
            run_id: "r1".into(),
            story_id: "s1".into(),
            role: AgentRole::Producer,
            board: BlackboardService::new(pool.clone()),
            pool,
        };
        (ctx, Arc::new(ToolRegistry::agency_default()))
    }

    #[test]
    fn test_parse_action_tool_and_final() {
        let tool = parse_action(r#"{"type":"tool","name":"board_read","args":{"zone":"asset"}}"#).unwrap();
        assert_eq!(tool, LoopAction::Tool { name: "board_read".into(), args: serde_json::json!({"zone":"asset"}) });
        let final_ = parse_action(r#"{"type":"final","content":"完成"}"#).unwrap();
        assert_eq!(final_, LoopAction::Final { content: "完成".into() });
        // 容错：模型前后带解释文字
        let noisy = parse_action("好的，我来调用工具。\n{\"type\":\"final\",\"content\":\"提取成功\"}\n以上").unwrap();
        assert_eq!(noisy, LoopAction::Final { content: "提取成功".into() });
        assert!(parse_action("完全没有 JSON").is_err());
    }

    #[tokio::test]
    async fn test_loop_tool_then_final() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec![
            r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#,
            r#"{"type":"final","content":"资产已写入"}"#,
        ]);
        let lp = ToolLoop::new(llm, registry);
        let result = lp.run(AgentRole::Producer, &ctx, "系统", "生产世界观").await.unwrap();
        assert!(!result.aborted);
        assert_eq!(result.output, "资产已写入");
        assert_eq!(result.turns.len(), 2);
        // 黑板确实被写入
        let snap = ctx.board.snapshot("r1").unwrap();
        assert_eq!(snap.assets.len(), 1);
    }

    #[tokio::test]
    async fn test_loop_aborts_on_repeated_parse_failure() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec!["不是 JSON", "还不是", "依然不是"]);
        let lp = ToolLoop::new(llm, registry);
        let result = lp.run(AgentRole::Producer, &ctx, "系统", "任务").await.unwrap();
        assert!(result.aborted);
    }

    #[tokio::test]
    async fn test_loop_rejects_unwhitelisted_tool_then_recovers() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec![
            r#"{"type":"tool","name":"delete_story","args":{}}"#,
            r#"{"type":"final","content":"改用合法路径"}"#,
        ]);
        let lp = ToolLoop::new(llm, registry);
        let result = lp.run(AgentRole::Producer, &ctx, "系统", "任务").await.unwrap();
        assert!(!result.aborted);
        assert_eq!(result.output, "改用合法路径");
        assert!(result.turns[0].observation.as_ref().unwrap().contains("不可用"));
    }

    #[tokio::test]
    async fn test_loop_max_turns() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec![
            r#"{"type":"tool","name":"story_info","args":{}}"#,
            r#"{"type":"tool","name":"story_info","args":{}}"#,
            r#"{"type":"tool","name":"story_info","args":{}}"#,
        ]);
        let lp = ToolLoop::new(llm, registry).with_max_turns(2);
        let result = lp.run(AgentRole::Producer, &ctx, "系统", "任务").await.unwrap();
        assert!(result.aborted);
        assert_eq!(result.turns.len(), 2);
    }
}
