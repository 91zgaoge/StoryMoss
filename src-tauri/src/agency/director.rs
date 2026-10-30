use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    agency::{
        budget::DIRECTOR_LLM_SEM,
        chat_repository::ChatRepository,
        crew::{CrewAlert, CrewSnapshot},
        director_repository::DirectorRepository,
        models::{AgentRole, ChatMessage, DirectorAction},
        scheduler::TaskScheduler,
    },
    db::DbPool,
    error::AppError,
    llm::LlmService,
    router::TaskType,
};

/// 导演计划：严格 JSON 的运行时结构。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectorPlan {
    pub reply: String,
    #[serde(default)]
    pub intent: String,
    #[serde(default)]
    pub actions: Vec<DirectorActionSpec>,
    #[serde(default)]
    pub needs_confirmation: bool,
    #[serde(default)]
    pub confirmation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DirectorActionSpec {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(rename = "type", default)]
    pub action_type: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub instruction: Option<String>,
    #[serde(default)]
    pub target_id: Option<String>,
    #[serde(default)]
    pub corpus_ids: Vec<String>,
    #[serde(default)]
    pub depends_on_action: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
}

const DIRECTOR_FALLBACK_SYSTEM: &str = "你是「导演」，代理工作室总控。你的职责是巡检、调度、修复、汇报；不写正文/资产。只输出严格 JSON：{\"reply\":\"...\",\"intent\":\"inspect|dispatch|repair|chat|corpus|none\",\"actions\":[{\"id\":\"a1\",\"type\":\"dispatch\",\"role\":\"producer\",\"instruction\":\"...\",\"corpus_ids\":[],\"depends_on_action\":null,\"reason\":\"...\"}],\"needs_confirmation\":false,\"confirmation\":null}。覆盖手写资产、主创续写/修订、批量、取消/恢复必须 needs_confirmation=true。";

/// 导演规划：巡检快照 + 用户消息 + 会话历史 → 严格 JSON。
pub struct DirectorPlanner;

impl DirectorPlanner {
    pub async fn plan(
        pool: &DbPool,
        app: &AppHandle,
        snapshot: &CrewSnapshot,
        user_message: &str,
        history: &[ChatMessage],
        corpus_brief: &str,
        corpus_ids: &[String],
    ) -> Result<DirectorPlan, AppError> {
        let _permit = DIRECTOR_LLM_SEM
            .acquire()
            .await
            .map_err(|_| AppError::from("导演许可已关闭"))?;
        let system = resolve_director_prompt(pool, &snapshot.story_title).await;
        let user =
            render_director_user_prompt(snapshot, user_message, history, corpus_brief, corpus_ids);
        let llm = LlmService::new(app.clone());
        let (_rid, result) = llm
            .generate_for_task_with_system_prompt(
                TaskType::Analysis,
                user,
                Some(4096),
                Some(0.4),
                Some("agency_director"),
                Some(system),
                None,
            )
            .await;
        let response = result.map_err(AppError::from)?;
        let mut plan: DirectorPlan =
            parse_loose(&response.content).unwrap_or_else(|| DirectorPlan {
                reply: format!(
                    "我先看了一遍现场：{} 个任务、{} 个告警。导演这次没听懂，请换个说法。",
                    snapshot.tasks.len(),
                    snapshot.alerts.len()
                ),
                intent: "inspect".into(),
                actions: Vec::new(),
                needs_confirmation: false,
                confirmation: None,
            });
        plan.actions.truncate(3);
        if !plan.needs_confirmation {
            plan.needs_confirmation = plan.actions.iter().any(action_requires_confirmation);
        }
        Ok(plan)
    }
}

/// 执行器：dispatch/retry/cancel/materialize/ensure/recheck/escalate。
/// 返回派发的 task_ids（立即返回，不等待子 agent）。
pub struct DirectorExecutor;

impl DirectorExecutor {
    #[allow(clippy::too_many_arguments)]
    pub async fn execute(
        pool: &DbPool,
        story_id: &str,
        session_id: &str,
        message_id: &str,
        premise: &str,
        actions: &[DirectorActionSpec],
        scheduler: &TaskScheduler,
    ) -> Result<Vec<String>, AppError> {
        let repo = DirectorRepository::new(pool.clone());
        let mut task_by_action: HashMap<String, String> = HashMap::new();
        let mut dispatched = Vec::new();

        for (idx, action) in actions.iter().enumerate() {
            let action_id = action.id.clone().unwrap_or_else(|| format!("a{}", idx + 1));
            let params = serde_json::to_string(action).unwrap_or_default();
            let record = repo.insert_action(
                story_id,
                Some(session_id),
                Some(message_id),
                None,
                &action.action_type,
                action.role.as_deref(),
                action.target_id.as_deref(),
                &params,
                "executing",
                action.reason.as_deref(),
            )?;

            let result = match action.action_type.as_str() {
                "dispatch" => {
                    let role = action
                        .role
                        .as_deref()
                        .and_then(AgentRole::from_str)
                        .ok_or_else(|| {
                            AppError::validation_failed("dispatch 缺少合法 role", Some("role"))
                        })?;
                    let instruction = action.instruction.as_deref().ok_or_else(|| {
                        AppError::validation_failed(
                            "dispatch 缺少 instruction",
                            Some("instruction"),
                        )
                    })?;
                    let depends_on = action
                        .depends_on_action
                        .as_ref()
                        .and_then(|a| task_by_action.get(a).cloned());
                    let meta = serde_json::json!({
                        "chat_session_id": session_id,
                        "chat_message_id": message_id,
                        "append_mode": "follow_up",
                        "premise": premise,
                        "corpus_ids": action.corpus_ids,
                    })
                    .to_string();
                    scheduler
                        .dispatch_sub_agent(
                            story_id,
                            &format!("chat:{}", session_id),
                            role,
                            premise,
                            instruction,
                            depends_on,
                            None,
                            meta,
                        )
                        .await
                        .map(Some)
                }
                "retry_task" => {
                    let target = action.target_id.as_deref().ok_or_else(|| {
                        AppError::validation_failed("retry_task 缺少 target_id", Some("target_id"))
                    })?;
                    scheduler.retry_task(target).await.map(Some)
                }
                "cancel_task" => {
                    let target = action.target_id.as_deref().unwrap_or("");
                    scheduler.cancel_task(target).map(|_| None)
                }
                "cancel_run" => {
                    let target = action.target_id.as_deref().unwrap_or("");
                    let hit = crate::agency::coordinator::cancel_agency_run(target);
                    Ok(if hit { Some(target.to_string()) } else { None })
                }
                "materialize_assets" | "ensure_assets" | "recheck_gate" => {
                    let role = if action.action_type == "recheck_gate" {
                        AgentRole::EditorAuditor
                    } else {
                        AgentRole::Producer
                    };
                    let instruction = action.instruction.as_deref().unwrap_or(
                        match action.action_type.as_str() {
                            "materialize_assets" => "把当前黑板资产物化落库，幂等，不覆盖手写内容",
                            "ensure_assets" => "补齐缺失资产，只补空，不覆盖已有人工设定",
                            _ => "复审当前草稿并重跑质量门",
                        },
                    );
                    let meta = serde_json::json!({
                        "chat_session_id": session_id,
                        "chat_message_id": message_id,
                        "append_mode": "follow_up",
                        "premise": premise,
                        "corpus_ids": action.corpus_ids,
                    })
                    .to_string();
                    scheduler
                        .dispatch_sub_agent(
                            story_id,
                            &format!("chat:{}", session_id),
                            role,
                            premise,
                            instruction,
                            None,
                            None,
                            meta,
                        )
                        .await
                        .map(Some)
                }
                "escalate" => {
                    let content = action
                        .instruction
                        .clone()
                        .unwrap_or_else(|| "需要作者处理".into());
                    let mut msg = ChatMessage::new(session_id, "system", content);
                    msg.status = "done".into();
                    msg.actions_json = Some(params.clone());
                    ChatRepository::new(pool.clone()).append_message(&msg)?;
                    Ok(None)
                }
                "record_debt" => Ok(None),
                other => Err(AppError::validation_failed(
                    format!("导演动作未实现: {other}"),
                    Some("action_type"),
                )),
            };

            match result {
                Ok(Some(task_id)) => {
                    repo.update_status(&record.id, "done", Some(&task_id), None)?;
                    task_by_action.insert(action_id, task_id.clone());
                    dispatched.push(task_id);
                }
                Ok(None) => {
                    repo.update_status(&record.id, "done", None, None)?;
                }
                Err(e) => {
                    repo.update_status(&record.id, "failed", None, Some(&e.to_string()))?;
                    if action.action_type == "dispatch"
                        || action.action_type == "materialize_assets"
                        || action.action_type == "ensure_assets"
                        || action.action_type == "recheck_gate"
                        || action.action_type == "retry_task"
                    {
                        return Err(e);
                    }
                }
            }
        }
        Ok(dispatched)
    }

    /// 确认后执行：把 proposed 动作转为 approved，再按同一执行器派发。
    pub async fn execute_confirmed(
        pool: &DbPool,
        story_id: &str,
        session_id: &str,
        message_id: &str,
        premise: &str,
        action_ids: &[String],
        scheduler: &TaskScheduler,
    ) -> Result<Vec<String>, AppError> {
        let repo = DirectorRepository::new(pool.clone());
        let proposed = repo.list_proposed(story_id, Some(session_id))?;
        let allowed: HashSet<&str> = action_ids.iter().map(|s| s.as_str()).collect();
        let selected: Vec<DirectorAction> = if allowed.is_empty() {
            proposed
        } else {
            proposed
                .into_iter()
                .filter(|a| allowed.contains(a.id.as_str()))
                .collect()
        };
        for a in &selected {
            repo.update_status(&a.id, "approved", None, None)?;
        }
        let actions: Vec<DirectorActionSpec> = selected
            .iter()
            .map(|a| serde_json::from_str::<DirectorActionSpec>(&a.params_json).unwrap_or_default())
            .collect();
        let dispatched = DirectorExecutor::execute(
            pool, story_id, session_id, message_id, premise, &actions, scheduler,
        )
        .await?;
        if !selected.is_empty() {
            // 审批卡已执行：清空 actions_json，避免前端重复显示「确认执行」。
            let _ = ChatRepository::new(pool.clone()).update_message_payload(
                message_id,
                "done",
                None,
                None,
                Some("[]"),
                None,
            );
        }
        Ok(dispatched)
    }
}

fn action_requires_confirmation(action: &DirectorActionSpec) -> bool {
    match action.action_type.as_str() {
        "dispatch" => {
            let role = action.role.as_deref().unwrap_or("");
            role == "lead_writer" || action.overwrite
        }
        "cancel_task" | "cancel_run" | "resume_run" | "ensure_assets" | "recheck_gate" => true,
        _ => false,
    }
}

async fn resolve_director_prompt(pool: &DbPool, premise: &str) -> String {
    let mut vars = HashMap::new();
    vars.insert("premise".to_string(), premise.to_string());
    let pool = pool.clone();
    let resolved = tokio::task::spawn_blocking(move || {
        crate::prompts::registry::resolve_prompt_with_vars(&pool, "agency_director_system", &vars)
    })
    .await
    .ok()
    .and_then(|r| r.ok());
    resolved.unwrap_or_else(|| DIRECTOR_FALLBACK_SYSTEM.to_string())
}

fn render_director_user_prompt(
    snapshot: &CrewSnapshot,
    user_message: &str,
    history: &[ChatMessage],
    corpus_brief: &str,
    corpus_ids: &[String],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("故事：{}\n", snapshot.story_title));
    out.push_str("Runs：\n");
    for r in snapshot.runs.iter().take(8) {
        out.push_str(&format!(
            "- {} lane={} status={} phase={}{}\n",
            r.id.chars().take(8).collect::<String>(),
            r.lane,
            r.status,
            r.phase,
            r.error_message
                .as_deref()
                .map(|e| format!(" error={}", e.chars().take(80).collect::<String>()))
                .unwrap_or_default()
        ));
    }
    out.push_str("Tasks：\n");
    for t in snapshot.tasks.iter().take(12) {
        out.push_str(&format!(
            "- {} {} status={} hb={} task={}\n",
            t.role.as_str(),
            t.id.chars().take(8).collect::<String>(),
            t.status,
            t.heartbeat_at.as_deref().unwrap_or("-"),
            t.task.chars().take(80).collect::<String>()
        ));
    }
    out.push_str("Leases：\n");
    for l in snapshot.leases.iter().take(6) {
        out.push_str(&format!(
            "- {} owner={:?} expires={}\n",
            l.resource, l.owner_task_id, l.expires_at
        ));
    }
    out.push_str("Alerts：\n");
    for a in snapshot.alerts.iter().take(10) {
        out.push_str(&format!("- [{}] {} {}\n", a.severity, a.kind, a.message));
    }
    out.push_str(&format!("语料：{} 条\n", snapshot.corpus_count));
    if !corpus_brief.is_empty() {
        out.push_str(&format!(
            "语料摘要：{}\n",
            corpus_brief.chars().take(1200).collect::<String>()
        ));
    }
    if !corpus_ids.is_empty() {
        out.push_str(&format!("用户引用语料：{}\n", corpus_ids.join(", ")));
    }
    out.push_str("会话历史：\n");
    for m in history
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        out.push_str(&format!(
            "{}: {}\n",
            m.role,
            m.content.chars().take(200).collect::<String>()
        ));
    }
    out.push_str(&format!("用户消息：{}\n", user_message));
    out
}

fn parse_loose<T: serde::de::DeserializeOwned>(raw: &str) -> Option<T> {
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(v) = serde_json::from_str::<T>(cleaned) {
        return Some(v);
    }
    let start = cleaned.find('{')?;
    let end = cleaned.rfind('}')?;
    serde_json::from_str::<T>(&cleaned[start..=end]).ok()
}

/// 导演回合入口：inspect → plan → dispatch/approval。异步，不等待子 agent。
#[allow(clippy::too_many_arguments)]
pub async fn run_director_turn(
    pool: &DbPool,
    app: &AppHandle,
    scheduler: &TaskScheduler,
    story_id: &str,
    session_id: &str,
    message_id: &str,
    user_message: &str,
    corpus_ids: &[String],
    corpus_brief: &str,
    premise: &str,
) -> Result<(), AppError> {
    let snapshot = crate::agency::crew::CrewInspector::inspect(pool, story_id)?;
    let history = ChatRepository::new(pool.clone()).list_messages(session_id, 8)?;
    let plan = DirectorPlanner::plan(
        pool,
        app,
        &snapshot,
        user_message,
        &history,
        corpus_brief,
        corpus_ids,
    )
    .await?;

    let chat = ChatRepository::new(pool.clone());
    if plan.needs_confirmation {
        let repo = DirectorRepository::new(pool.clone());
        let mut records = Vec::new();
        for (idx, action) in plan.actions.iter().enumerate() {
            let params = serde_json::to_string(action).unwrap_or_default();
            records.push(repo.insert_action(
                story_id,
                Some(session_id),
                Some(message_id),
                None,
                &action.action_type,
                action.role.as_deref(),
                action.target_id.as_deref(),
                &params,
                "proposed",
                action.reason.as_deref(),
            )?);
            let _ = idx;
        }
        let card = serde_json::json!({
            "confirmation": plan.confirmation,
            "actions": records,
        });
        let content = format!(
            "{}\n\n{}",
            plan.reply,
            plan.confirmation
                .clone()
                .unwrap_or_else(|| "需要你确认后才会派发。".into())
        );
        chat.update_message_payload(
            message_id,
            "done",
            Some(&content),
            None,
            Some(&card.to_string()),
            None,
        )?;
        return Ok(());
    }

    let task_ids = DirectorExecutor::execute(
        pool,
        story_id,
        session_id,
        message_id,
        premise,
        &plan.actions,
        scheduler,
    )
    .await?;
    if task_ids.is_empty() {
        chat.update_message_payload(message_id, "done", Some(&plan.reply), None, None, None)?;
    } else {
        let content = format!(
            "{}\n已派发 {} 个任务，完成后会追加回报。",
            plan.reply,
            task_ids.len()
        );
        chat.update_message_payload(
            message_id,
            "acting",
            Some(&content),
            Some(&serde_json::to_string(&task_ids).unwrap_or_default()),
            None,
            None,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_loose_and_confirmation() {
        let plan: DirectorPlan = parse_loose(
            "```json\n{\"reply\":\"收到\",\"actions\":[{\"id\":\"a1\",\"type\":\"dispatch\",\"role\":\"lead_writer\",\"instruction\":\"续写\"}],\"needs_confirmation\":false}\n```",
        )
        .unwrap();
        assert_eq!(plan.reply, "收到");
        assert!(action_requires_confirmation(&plan.actions[0]));
        assert!(action_requires_confirmation(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("producer".into()),
            overwrite: true,
            ..Default::default()
        }));
        assert!(!action_requires_confirmation(&DirectorActionSpec {
            action_type: "retry_task".into(),
            ..Default::default()
        }));
    }

    #[test]
    fn test_render_prompt_includes_alerts() {
        let snapshot = CrewSnapshot {
            story_id: "s".into(),
            story_title: "长夜".into(),
            runs: vec![],
            tasks: vec![],
            leases: vec![],
            corpus_count: 2,
            alerts: vec![CrewAlert {
                severity: "high".into(),
                kind: "StaleTask".into(),
                message: "编审卡住".into(),
                role: None,
                task_id: None,
                run_id: None,
            }],
        };
        let text = render_director_user_prompt(&snapshot, "巡检", &[], "", &[]);
        assert!(text.contains("编审卡住"));
        assert!(text.contains("语料：2 条"));
    }
}
