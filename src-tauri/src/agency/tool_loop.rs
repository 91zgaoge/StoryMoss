use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    agency::{
        models::AgentRole,
        tools::{ToolContext, ToolRegistry},
    },
    error::AppError,
    router::TaskType,
};

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

    /// 带计量的完成：返回 (content, tokens_used,
    /// cost)。默认实现不计费（mock/测试用）。
    async fn complete_metered(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<(String, i32, f64), AppError> {
        self.complete(system_prompt, user_prompt, task, max_tokens)
            .await
            .map(|s| (s, 0, 0.0))
    }

    /// JSON mode 完成：请求结构化输出（OpenAI `{"type":"json_object"}` /
    /// Ollama `format:"json"`）。默认回退 complete（mock/不支持 JSON
    /// mode 的实现无需感知）。
    async fn complete_json(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<String, AppError> {
        self.complete(system_prompt, user_prompt, task, max_tokens)
            .await
    }

    /// 带计量的 JSON mode 完成：返回 (content, tokens_used, cost)。
    /// 默认回退 complete_json 不计费（mock/测试用零改动）。
    async fn complete_json_metered(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        task: TaskType,
        max_tokens: i32,
    ) -> Result<(String, i32, f64), AppError> {
        self.complete_json(system_prompt, user_prompt, task, max_tokens)
            .await
            .map(|s| (s, 0, 0.0))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LoopAction {
    Tool {
        name: String,
        #[serde(default)]
        args: serde_json::Value,
    },
    Final {
        content: String,
    },
}

/// 多 action 数组截断提示：模型一次输出多个 action 时只执行第一个，
/// 该提示追加进 observation 引导其后续单 action 输出。
const MULTI_ACTION_HINT: &str =
    "（检测到多个 action，只执行了第一个。一次只输出一个 JSON action，不要数组。）";

/// 解析模型输出为 action。容错策略：截取首个 '{' 与末个 '}' 之间的子串解析。
pub fn parse_action(raw: &str) -> Result<LoopAction, AppError> {
    parse_action_full(raw).map(|(action, _)| action)
}

/// Value → LoopAction：先试标准反序列化，失败则启发式归类——
/// object 含 `name` 字段视为 Tool（args 缺省 {}）；无 `name` 仅 `content` 视为
/// Final。覆盖本地模型的 `{"type":"board_write","name":..,"args":..}` 变体。
fn action_from_value(v: serde_json::Value) -> Result<LoopAction, AppError> {
    if let Ok(action) = serde_json::from_value::<LoopAction>(v.clone()) {
        return Ok(action);
    }
    if let Some(obj) = v.as_object() {
        if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
            let args = obj
                .get("args")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            return Ok(LoopAction::Tool {
                name: name.to_string(),
                args,
            });
        }
        if let Some(content) = obj.get("content").and_then(|c| c.as_str()) {
            return Ok(LoopAction::Final {
                content: content.to_string(),
            });
        }
    }
    Err(AppError::validation_failed(
        "action 形态无法识别",
        None::<String>,
    ))
}

/// parse_action 完整版：返回 (action, 附加提示)。三层容错：
/// 1) 标准解析（截取首个 '{' 至末个 '}'）；2) 数组解包（'[' 在首个 '{'
/// 之前时按 Vec<Value> 解析，单元素解包、多元素取首个并附提示）；
/// 3) 启发式判定（见 action_from_value）。
fn parse_action_full(raw: &str) -> Result<(LoopAction, Option<&'static str>), AppError> {
    let start = raw.find('{');
    let end = raw.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if e > s => {
            let slice = &raw[s..=e];
            // 1) 标准解析
            let std_err = match serde_json::from_str::<LoopAction>(slice) {
                Ok(action) => return Ok((action, None)),
                Err(err) => err,
            };
            // 2) 数组解包：'[' 位于首个 '{' 之前 → 多动作数组形态
            if let Some(b) = raw.find('[') {
                if b < s {
                    if let Some(ae) = raw.rfind(']') {
                        if ae > b {
                            if let Ok(items) =
                                serde_json::from_str::<Vec<serde_json::Value>>(&raw[b..=ae])
                            {
                                let mut iter = items.into_iter();
                                if let Some(first) = iter.next() {
                                    let hint = if iter.next().is_some() {
                                        Some(MULTI_ACTION_HINT)
                                    } else {
                                        None
                                    };
                                    if let Ok(action) = action_from_value(first) {
                                        return Ok((action, hint));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // 3) 启发式判定：截取子串按 Value 解析后归类
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(slice) {
                if let Ok(action) = action_from_value(v) {
                    return Ok((action, None));
                }
            }
            Err(AppError::validation_failed(
                format!("action JSON 解析失败: {}", std_err),
                None::<String>,
            ))
        }
        _ => Err(AppError::validation_failed(
            "输出中未找到 JSON action",
            None::<String>,
        )),
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

/// 会话窗口：超预算时保留头部（工具目录+任务）与尾部最近对话（ECC
/// 注入预算模式）。
pub fn truncate_conversation(head: &str, tail: &str, budget_chars: usize) -> String {
    let marker = "\n…(早期对话已截断)…\n";
    let total = head.chars().count() + tail.chars().count();
    if total <= budget_chars {
        return format!("{}{}", head, tail);
    }
    let tail_budget = budget_chars.saturating_sub(head.chars().count() + marker.chars().count());
    let kept: String = {
        let mut chars: Vec<char> = tail.chars().collect();
        let start = chars.len().saturating_sub(tail_budget);
        chars.drain(start..).collect()
    };
    format!("{}{}{}", head, marker, kept)
}

/// LoopTurn 记录的原始响应截断（P1 终审转 P3）。
pub fn truncate_raw(raw: &str) -> String {
    if raw.chars().count() > 4000 {
        format!("{}…(已截断)", raw.chars().take(4000).collect::<String>())
    } else {
        raw.to_string()
    }
}

pub struct ToolLoop {
    llm: Arc<dyn LoopLlm>,
    registry: Arc<ToolRegistry>,
    max_turns: usize,
}

impl ToolLoop {
    pub fn new(llm: Arc<dyn LoopLlm>, registry: Arc<ToolRegistry>) -> Self {
        Self {
            llm,
            registry,
            max_turns: 8,
        }
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
        // head = 工具目录 + 任务（恒定），tail =
        // 逐轮追加的对话；每轮调用前按角色预算截断
        let head = format!(
            "{}\n\n你只能输出一个 JSON action，不要输出其他内容：\n\
             - 调用工具: {{\"type\":\"tool\",\"name\":\"<工具名>\",\"args\":{{...}}}}\n\
             - 完成任务: {{\"type\":\"final\",\"content\":\"<最终产出>\"}}\n\n任务：\n{}",
            self.registry.catalog_for_role(role),
            task
        );
        let mut tail = String::new();
        let mut turns: Vec<LoopTurn> = Vec::new();
        let mut parse_failures = 0usize;

        for _ in 0..self.max_turns {
            let conversation = truncate_conversation(&head, &tail, ctx.max_context_chars());
            let raw = self
                .llm
                .complete(
                    system_prompt,
                    &conversation,
                    ctx.task_type(),
                    ctx.max_output_tokens(),
                )
                .await?;
            match parse_action_full(&raw) {
                Ok((LoopAction::Final { content }, _)) => {
                    turns.push(LoopTurn {
                        raw_response: truncate_raw(&raw),
                        action: Some(LoopAction::Final {
                            content: content.clone(),
                        }),
                        observation: None,
                    });
                    return Ok(LoopResult {
                        output: content,
                        turns,
                        aborted: false,
                    });
                }
                Ok((LoopAction::Tool { name, args }, hint)) => {
                    parse_failures = 0;
                    let mut observation = match self.registry.get_for_role(role, &name) {
                        Some(tool) => match tool.execute(ctx, args.clone()).await {
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
                    // 多 action 数组截断提示：引导模型后续单 action 输出
                    if let Some(hint) = hint {
                        observation.push_str(hint);
                    }
                    tail.push_str(&format!(
                        "\n\n你的上一步：{}\n观察结果：{}",
                        raw, observation
                    ));
                    // 保留真实 args（P4 trace 回放需要），不再置 Null
                    turns.push(LoopTurn {
                        raw_response: truncate_raw(&raw),
                        action: Some(LoopAction::Tool { name, args }),
                        observation: Some(observation),
                    });
                }
                Err(e) => {
                    parse_failures += 1;
                    let observation = format!("格式错误（{}）。请只输出一个 JSON action。", e);
                    tail.push_str(&format!(
                        "\n\n你的上一步：{}\n观察结果：{}",
                        raw, observation
                    ));
                    turns.push(LoopTurn {
                        raw_response: truncate_raw(&raw),
                        action: None,
                        observation: Some(observation),
                    });
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
    use std::{collections::VecDeque, sync::Mutex};

    use super::*;
    use crate::{
        agency::{
            board::BlackboardService, models::*, repository::AgencyRepository, tools::ToolRegistry,
        },
        db::create_test_pool,
    };

    struct MockLlm {
        responses: Mutex<VecDeque<String>>,
    }

    impl MockLlm {
        fn scripted(lines: Vec<&str>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(lines.into_iter().map(String::from).collect()),
            })
        }
    }

    #[async_trait::async_trait]
    impl LoopLlm for MockLlm {
        async fn complete(
            &self,
            _s: &str,
            _u: &str,
            _t: TaskType,
            _m: i32,
        ) -> Result<String, AppError> {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| AppError::validation_failed("mock exhausted", None::<String>))
        }
    }

    fn setup() -> (ToolContext, Arc<ToolRegistry>) {
        let pool = create_test_pool().unwrap();
        AgencyRepository::new(pool.clone())
            .create_run(&AgencyRun::new("r1", "前提"))
            .unwrap();
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
        let tool =
            parse_action(r#"{"type":"tool","name":"board_read","args":{"zone":"asset"}}"#).unwrap();
        assert_eq!(
            tool,
            LoopAction::Tool {
                name: "board_read".into(),
                args: serde_json::json!({"zone":"asset"})
            }
        );
        let final_ = parse_action(r#"{"type":"final","content":"完成"}"#).unwrap();
        assert_eq!(
            final_,
            LoopAction::Final {
                content: "完成".into()
            }
        );
        // 容错：模型前后带解释文字
        let noisy = parse_action(
            "好的，我来调用工具。\n{\"type\":\"final\",\"content\":\"提取成功\"}\n以上",
        )
        .unwrap();
        assert_eq!(
            noisy,
            LoopAction::Final {
                content: "提取成功".into()
            }
        );
        assert!(parse_action("完全没有 JSON").is_err());
    }

    #[test]
    fn test_parse_action_single_element_array_unwrapped() {
        // 本地模型常把单个 action 包成数组
        let tool = parse_action(r#"[{"type":"tool","name":"board_read","args":{"zone":"asset"}}]"#)
            .unwrap();
        assert_eq!(
            tool,
            LoopAction::Tool {
                name: "board_read".into(),
                args: serde_json::json!({"zone":"asset"})
            }
        );
    }

    #[test]
    fn test_parse_action_multi_element_array_takes_first_with_hint() {
        let (action, hint) = parse_action_full(
            r#"[{"type":"tool","name":"board_read","args":{"zone":"asset"}},{"type":"final","content":"完成"}]"#,
        )
        .unwrap();
        assert_eq!(
            action,
            LoopAction::Tool {
                name: "board_read".into(),
                args: serde_json::json!({"zone":"asset"})
            }
        );
        assert_eq!(hint, Some(MULTI_ACTION_HINT));
    }

    #[test]
    fn test_parse_action_tool_name_as_type_variant() {
        // 变体形态：{"type":"board_write","name":"board_write","args":{...}}
        let tool = parse_action(
            r#"{"type":"board_write","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星"}}"#,
        )
        .unwrap();
        assert_eq!(
            tool,
            LoopAction::Tool {
                name: "board_write".into(),
                args: serde_json::json!({"zone":"asset","item_type":"world","key":"世界观","content":"双星"})
            }
        );
        // 无 type 标签、仅 name → Tool（args 缺省 {}）
        let no_args = parse_action(r#"{"name":"board_read"}"#).unwrap();
        assert_eq!(
            no_args,
            LoopAction::Tool {
                name: "board_read".into(),
                args: serde_json::json!({})
            }
        );
        // 无 name、仅 content → Final
        let final_ = parse_action(r#"{"content":"收工"}"#).unwrap();
        assert_eq!(
            final_,
            LoopAction::Final {
                content: "收工".into()
            }
        );
    }

    #[test]
    fn test_parse_action_prose_wrapped_array() {
        let (action, hint) = parse_action_full(
            "我先给出两个动作：\n[{\"type\":\"tool\",\"name\":\"story_info\",\"args\":{}},{\"type\":\"final\",\"content\":\"完\"}]\n以上。",
        )
        .unwrap();
        assert_eq!(
            action,
            LoopAction::Tool {
                name: "story_info".into(),
                args: serde_json::json!({})
            }
        );
        assert_eq!(hint, Some(MULTI_ACTION_HINT));
    }

    #[test]
    fn test_parse_action_non_json_still_errors() {
        assert!(parse_action("完全没有 JSON").is_err());
        assert!(parse_action("{不是合法 json}").is_err());
    }

    #[tokio::test]
    async fn test_loop_multi_action_array_hint_in_observation() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec![
            r#"[{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}},{"type":"tool","name":"board_read","args":{}}]"#,
            r#"{"type":"final","content":"done"}"#,
        ]);
        let lp = ToolLoop::new(llm, registry);
        let result = lp
            .run(AgentRole::Producer, &ctx, "系统", "任务")
            .await
            .unwrap();
        assert!(!result.aborted);
        // 多 action 数组：只执行第一个，observation 带截断提示
        let obs = result.turns[0].observation.as_ref().unwrap();
        assert!(
            obs.contains("只执行了第一个"),
            "observation 应含提示: {}",
            obs
        );
        // 数组首元素的 board_write 确实生效
        let snap = ctx.board.snapshot("r1").unwrap();
        assert_eq!(snap.assets.len(), 1);
    }

    #[tokio::test]
    async fn test_conversation_window_truncation() {
        let (ctx, registry) = setup();
        let big_observation = "x".repeat(30_000); // 工具结果超长（observation 截断 4000 仍累计）
        let llm = MockLlm::scripted(vec![
            &format!(r#"{{"type":"tool","name":"story_info","args":{{}}}}"#),
            r#"{"type":"final","content":"done"}"#,
        ]);
        let lp = ToolLoop::new(llm, registry).with_max_turns(4);
        let result = lp
            .run(AgentRole::EditorAuditor, &ctx, "系统", "任务")
            .await
            .unwrap();
        assert!(!result.aborted);
        // 会话窗口逻辑存在性验证：EditorAuditor 预算 10000 字符，截断函数行为单测
        let windowed = truncate_conversation("头部任务\n", &"中".repeat(20000), 10000);
        assert!(windowed.chars().count() <= 10050);
        assert!(windowed.contains("头部任务"));
        assert!(windowed.contains("…(早期对话已截断)…"));
        let _ = big_observation;
        let _ = result;
    }

    #[test]
    fn test_raw_response_truncated_in_turns() {
        // parse/记录层：LoopTurn.raw_response 超 4000 字符被截断
        let raw = format!("{}...", "y".repeat(5000));
        let truncated = truncate_raw(&raw);
        assert_eq!(
            truncated.chars().count(),
            4000 + "…(已截断)".chars().count()
        );
    }

    #[tokio::test]
    async fn test_loop_tool_then_final() {
        let (ctx, registry) = setup();
        let llm = MockLlm::scripted(vec![
            r#"{"type":"tool","name":"board_write","args":{"zone":"asset","item_type":"world","key":"世界观","content":"双星","summary":"双星"}}"#,
            r#"{"type":"final","content":"资产已写入"}"#,
        ]);
        let lp = ToolLoop::new(llm, registry);
        let result = lp
            .run(AgentRole::Producer, &ctx, "系统", "生产世界观")
            .await
            .unwrap();
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
        let result = lp
            .run(AgentRole::Producer, &ctx, "系统", "任务")
            .await
            .unwrap();
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
        let result = lp
            .run(AgentRole::Producer, &ctx, "系统", "任务")
            .await
            .unwrap();
        assert!(!result.aborted);
        assert_eq!(result.output, "改用合法路径");
        assert!(result.turns[0]
            .observation
            .as_ref()
            .unwrap()
            .contains("不可用"));
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
        let result = lp
            .run(AgentRole::Producer, &ctx, "系统", "任务")
            .await
            .unwrap();
        assert!(result.aborted);
        assert_eq!(result.turns.len(), 2);
    }
}
