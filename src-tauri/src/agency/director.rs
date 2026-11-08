use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    agency::{
        budget::DIRECTOR_LLM_SEM,
        chat_repository::ChatRepository,
        crew::{CrewAlert, CrewSnapshot},
        director_repository::DirectorRepository,
        models::{AgentRole, ChatMessage, DirectorAction},
        scheduler::TaskScheduler,
        task_repository::TaskRepository,
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
    /// v0.80.6：主创修订既有章的目标章号（1-based，对应
    /// scenes.sequence_number）。 仅 `dispatch lead_writer`
    /// 可用；缺省表示续写下一章。
    #[serde(default)]
    pub target_chapter: Option<i32>,
    /// v0.82.2：主创修订指定场景的目标场景序号（1-based，对应
    /// scenes.sequence_number）。仅 `dispatch lead_writer` 可用；
    /// 与 target_chapter 互斥（同时只能指定一种修订目标）。
    #[serde(default)]
    pub target_scene: Option<i32>,
    /// v0.86.0：主创连续续写到该章（含）后自动停止。仅 `dispatch
    /// lead_writer` 可用；单次批量最多 5 章，更长区间由导演用
    /// depends_on_action 串行分批。与 target_chapter / target_scene 互斥。
    #[serde(default)]
    pub target_end_chapter: Option<i32>,
}

const DIRECTOR_FALLBACK_SYSTEM: &str = "你是「导演」，代理工作室总控。你的职责是巡检、调度、修复、汇报；不写正文/资产。只输出严格 JSON：{\"reply\":\"...\",\"intent\":\"inspect|dispatch|repair|chat|corpus|none\",\"actions\":[{\"id\":\"a1\",\"type\":\"dispatch\",\"role\":\"producer\",\"instruction\":\"...\",\"corpus_ids\":[],\"depends_on_action\":null,\"reason\":\"...\"}],\"needs_confirmation\":false,\"confirmation\":null}。target_id 只能来自巡检快照 tasks[].id / runs[].id；没有对应 id 就不要生成动作。target_chapter 仅用于 lead_writer 修订既有章：填已有章号则修订该章，留空则续写下一章。target_scene 仅用于 lead_writer 修订指定场景：填巡检快照 Scenes 列表里的场景序号（scenes.sequence_number）则只修订该场景，与 target_chapter 只能二选一。target_end_chapter 仅用于 lead_writer 连续续写到该章（含）自动停止：单次最多 5 章，更长区间用 depends_on_action 串行分批；与 target_chapter/target_scene 互斥。覆盖手写资产、主创续写/修订、批量、取消/恢复必须 needs_confirmation=true。";

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
        let mut valid = Vec::new();
        let mut rejected = Vec::new();
        for action in plan.actions.drain(..) {
            if let Some(reason) = validate_action(&action) {
                rejected.push(reason);
            } else {
                valid.push(action);
            }
        }
        plan.actions = valid;
        if !rejected.is_empty() {
            plan.reply = format!(
                "{}\n\n导演丢弃了 {} 个不完整动作：{}",
                plan.reply,
                rejected.len(),
                rejected.join("；")
            );
        }
        if plan.actions.is_empty() {
            plan.needs_confirmation = false;
        } else if !plan.needs_confirmation {
            plan.needs_confirmation = plan.actions.iter().any(action_requires_confirmation);
        }
        Ok(plan)
    }
}

/// 计划动作的最小字段校验：宁可丢弃，也不把缺 target_id 的动作做成审批卡。
fn validate_action(action: &DirectorActionSpec) -> Option<String> {
    let at = action.action_type.as_str();
    match at {
        "dispatch" => {
            let role = action.role.as_deref().and_then(AgentRole::from_str)?;
            if role == AgentRole::Director {
                return Some("导演不能被派成子 agent".to_string());
            }
            if action
                .instruction
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                return Some("dispatch 缺少 instruction".to_string());
            }
            if let Some(ch) = action.target_chapter {
                if role != AgentRole::LeadWriter {
                    return Some("target_chapter 仅用于 lead_writer 修订".to_string());
                }
                if ch < 1 {
                    return Some("target_chapter 必须 >= 1".to_string());
                }
            }
            if let Some(sc) = action.target_scene {
                if role != AgentRole::LeadWriter {
                    return Some("target_scene 仅用于 lead_writer 修订".to_string());
                }
                if sc < 1 {
                    return Some("target_scene 必须 >= 1".to_string());
                }
                if action.target_chapter.is_some() {
                    return Some("target_chapter 与 target_scene 只能二选一".to_string());
                }
            }
            if let Some(end) = action.target_end_chapter {
                if role != AgentRole::LeadWriter {
                    return Some("target_end_chapter 仅用于 lead_writer 续写".to_string());
                }
                if end < 1 {
                    return Some("target_end_chapter 必须 >= 1".to_string());
                }
                if action.target_chapter.is_some() || action.target_scene.is_some() {
                    return Some(
                        "target_end_chapter 与 target_chapter/target_scene 互斥".to_string(),
                    );
                }
            }
            None
        }
        "retry_task" | "cancel_task" => {
            if action.target_id.as_deref().unwrap_or("").trim().is_empty() {
                Some(format!("{at} 缺少 task_id"))
            } else {
                None
            }
        }
        "cancel_run" | "resume_run" | "materialize_assets" | "recheck_gate" => {
            if action.target_id.as_deref().unwrap_or("").trim().is_empty() {
                Some(format!("{at} 缺少 run_id"))
            } else {
                None
            }
        }
        "ensure_assets" | "escalate" => None,
        "record_debt" => {
            if action
                .instruction
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                Some("record_debt 缺少 instruction（债务描述）".to_string())
            } else {
                None
            }
        }
        other => Some(format!("未知动作类型 {other}")),
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
        // v0.86.0：单动作失败只记录，不再中断整回合；所有动作都失败才把
        // 回合判死，部分失败以系统消息追加说明。
        let mut failures: Vec<String> = Vec::new();
        let mut succeeded: usize = 0;

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

            // v0.86.0：每个 arm 包 async 块，`?` 只在块内短路、进 result，
            // 由下方统一收集失败——不允许任何 arm 直接把整回合炸掉。
            let result = match action.action_type.as_str() {
                "dispatch" => {
                    async {
                        let role = action
                            .role
                            .as_deref()
                            .and_then(AgentRole::from_str)
                            .ok_or_else(|| {
                                AppError::validation_failed("dispatch 缺少合法 role", Some("role"))
                            })?;
                        if role == AgentRole::Director {
                            return Err(AppError::validation_failed(
                                "导演不能把自己再派成子 agent",
                                Some("role"),
                            ));
                        }
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
                        let mut meta_obj = serde_json::json!({
                            "chat_session_id": session_id,
                            "chat_message_id": message_id,
                            "append_mode": "follow_up",
                            "premise": premise,
                            "corpus_ids": action.corpus_ids,
                        });
                        // v0.80.5: 导演派发的主创任务走「章号分配 → 续写 →
                        // GateRunner → 装配」；黑板 ToolLoop 只用于 Producer/
                        // Editor 等资产/审查角色。
                        if role == AgentRole::LeadWriter {
                            meta_obj["directed_chapter"] = serde_json::Value::Bool(true);
                            if let Some(ch) = action.target_chapter {
                                meta_obj["target_chapter"] = serde_json::Value::from(ch);
                            }
                            if let Some(sc) = action.target_scene {
                                meta_obj["target_scene"] = serde_json::Value::from(sc);
                            }
                            // v0.86.0：连续续写到第 N 章（含）自动停止
                            if let Some(end) = action.target_end_chapter {
                                meta_obj["target_end_chapter"] = serde_json::Value::from(end);
                            }
                        }
                        let meta = meta_obj.to_string();
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
                    .await
                }
                "retry_task" => {
                    async {
                        let target = action.target_id.as_deref().ok_or_else(|| {
                            AppError::validation_failed(
                                "retry_task 缺少 target_id",
                                Some("target_id"),
                            )
                        })?;
                        // v0.86.0：target_id 允许是巡检快照的截断/短 ID——先在本
                        // 故事任务表里按「全等 → 唯一前缀」解析成完整 ID。
                        let resolved = Self::resolve_task_target(pool, story_id, target).await?;
                        scheduler.retry_task(&resolved).await.map(Some)
                    }
                    .await
                }
                "cancel_task" => {
                    async {
                        let target = action.target_id.as_deref().unwrap_or("");
                        if target.is_empty() {
                            return scheduler.cancel_task(target).map(|_| None);
                        }
                        let resolved = Self::resolve_task_target(pool, story_id, target).await?;
                        scheduler.cancel_task(&resolved).map(|_| None)
                    }
                    .await
                }
                "cancel_run" => {
                    async {
                        let target = action.target_id.as_deref().unwrap_or("");
                        let hit = crate::agency::coordinator::cancel_agency_run(target);
                        if hit {
                            return Ok(Some(target.to_string()));
                        }
                        // v0.86.0：精确未命中时按前缀解析 run id，仍找不到才算
                        // 失败（此前是静默 Ok(None)，导演会误以为已取消）。
                        let resolved = Self::resolve_run_target(pool, story_id, target).await?;
                        let hit = crate::agency::coordinator::cancel_agency_run(&resolved);
                        Ok(if hit { Some(resolved) } else { None })
                    }
                    .await
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
                    async {
                        let content = action
                            .instruction
                            .clone()
                            .unwrap_or_else(|| "需要作者处理".into());
                        let mut msg = ChatMessage::new(session_id, "system", content);
                        msg.status = "done".into();
                        msg.actions_json = Some(params.clone());
                        ChatRepository::new(pool.clone())
                            .append_message(&msg)
                            .map(|_| None)
                    }
                    .await
                }
                "record_debt" => {
                    let detail = action
                        .instruction
                        .as_deref()
                        .or(action.reason.as_deref())
                        .unwrap_or("")
                        .trim();
                    if detail.is_empty() {
                        Err(AppError::validation_failed(
                            "record_debt 缺少 instruction（债务描述）",
                            Some("instruction"),
                        ))
                    } else {
                        let pool_c = pool.clone();
                        let sid = story_id.to_string();
                        let detail_c = detail.chars().take(500).collect::<String>();
                        let chapter = action.target_chapter;
                        let scene = action.target_id.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            crate::story_system::quality_debt::record_debt(
                                &pool_c,
                                &sid,
                                scene.as_deref(),
                                chapter,
                                "director",
                                "warning",
                                &detail_c,
                            )
                            .map_err(AppError::from)
                        })
                        .await
                        .map_err(|e| AppError::from(format!("record_debt join error: {e}")))?;
                        result.map(|_| None)
                    }
                }
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
                    succeeded += 1;
                }
                Ok(None) => {
                    repo.update_status(&record.id, "done", None, None)?;
                    succeeded += 1;
                }
                Err(e) => {
                    // v0.86.0：失败只记录该动作，不再炸掉整回合——此前
                    // retry_task 一个坏 target_id（如快照截断 ID）会把整个
                    // 导演回合判死，作者只看到「导演回合失败」。
                    repo.update_status(&record.id, "failed", None, Some(&e.to_string()))?;
                    failures.push(format!("{}（{}）", action.action_type, e));
                }
            }
        }
        if succeeded == 0 && !failures.is_empty() {
            // 所有动作都失败：保留回合级失败语义，聊天消息标记 failed。
            return Err(AppError::from(format!(
                "导演动作全部失败：{}",
                failures.join("；")
            )));
        }
        if !failures.is_empty() {
            // 部分成功：以系统消息追加失败明细，不打断已派发任务的回报。
            let mut msg = ChatMessage::new(
                session_id,
                "system",
                format!(
                    "有 {} 个导演动作执行失败：{}",
                    failures.len(),
                    failures.join("；")
                ),
            );
            msg.status = "done".into();
            ChatRepository::new(pool.clone()).append_message(&msg)?;
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

    /// v0.86.0：把巡检快照里的截断/短 task id 解析成完整 id。
    /// 全等优先；未命中时按前缀匹配，恰好一个命中才通过，
    /// 多个命中报歧义，零命中给可操作错误（不再让调度器炸整回合）。
    async fn resolve_task_target(
        pool: &DbPool,
        story_id: &str,
        raw: &str,
    ) -> Result<String, AppError> {
        let pool_c = pool.clone();
        let sid = story_id.to_string();
        let raw_s = raw.to_string();
        let tasks = tokio::task::spawn_blocking(move || {
            TaskRepository::new(pool_c)
                .list_for_story(&sid, 200)
                .map_err(AppError::from)
        })
        .await
        .map_err(|e| AppError::from(format!("resolve task join error: {e}")))??;
        if tasks.iter().any(|t| t.id == raw_s) {
            return Ok(raw_s);
        }
        let hits: Vec<&str> = tasks
            .iter()
            .filter(|t| t.id.starts_with(&raw_s))
            .map(|t| t.id.as_str())
            .collect();
        match hits.len() {
            1 => Ok(hits[0].to_string()),
            0 => Err(AppError::validation_failed(
                format!("任务不存在: {raw_s}（target_id 必须来自巡检快照的完整任务 ID）"),
                Some("target_id"),
            )),
            _ => Err(AppError::validation_failed(
                format!(
                    "任务 ID 前缀歧义: {raw_s} 命中 {} 个任务（{}…）",
                    hits.len(),
                    &hits[0][..hits[0].len().min(12)]
                ),
                Some("target_id"),
            )),
        }
    }

    /// v0.86.0：cancel_run 的 run id 前缀解析（口径同 resolve_task_target）。
    async fn resolve_run_target(
        pool: &DbPool,
        story_id: &str,
        raw: &str,
    ) -> Result<String, AppError> {
        let pool_c = pool.clone();
        let sid = story_id.to_string();
        let runs = tokio::task::spawn_blocking(move || {
            crate::agency::crew::CrewInspector::inspect(&pool_c, &sid)
                .map(|s| s.runs.into_iter().map(|r| r.id).collect::<Vec<_>>())
                .map_err(AppError::from)
        })
        .await
        .map_err(|e| AppError::from(format!("resolve run join error: {e}")))??;
        if runs.iter().any(|id| id == raw) {
            return Ok(raw.to_string());
        }
        let hits: Vec<&str> = runs
            .iter()
            .filter(|id| id.starts_with(raw))
            .map(|id| id.as_str())
            .collect();
        match hits.len() {
            1 => Ok(hits[0].to_string()),
            0 => Err(AppError::validation_failed(
                format!("运行不存在: {raw}（target_id 必须来自巡检快照的 run ID）"),
                Some("target_id"),
            )),
            _ => Err(AppError::validation_failed(
                format!("运行 ID 前缀歧义: {raw} 命中 {} 个 run", hits.len()),
                Some("target_id"),
            )),
        }
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

pub(crate) async fn resolve_director_prompt(pool: &DbPool, premise: &str) -> String {
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

pub(crate) fn render_director_user_prompt(
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
            // v0.86.0：完整 ID（此前截前 8 位，导演照抄去 retry_task/cancel_run
            // 必然「任务不存在」）。36 字符 × 20 行成本可忽略。
            "- {} lane={} status={} phase={}{}\n",
            r.id,
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
            t.id,
            t.status,
            t.heartbeat_at.as_deref().unwrap_or("-"),
            t.task.chars().take(80).collect::<String>()
        ));
    }
    out.push_str("Scenes（场景序号 | 所属章 | 标题，可作 target_scene）：\n");
    for sc in snapshot.scenes.iter().take(24) {
        let chapter = sc
            .chapter_number
            .map(|n| format!("第{n}章"))
            .unwrap_or_else(|| "-".to_string());
        let title = if sc.title.trim().is_empty() {
            "-".to_string()
        } else {
            sc.title.chars().take(24).collect::<String>()
        };
        out.push_str(&format!(
            "- {} | {} | {} | {}字\n",
            sc.sequence_number, chapter, title, sc.word_count
        ));
    }
    out.push_str("Leases：\n");
    for l in snapshot.leases.iter().take(6) {
        out.push_str(&format!(
            "- {} owner={:?} expires={}\n",
            l.resource, l.owner_task_id, l.expires_at
        ));
    }
    // v0.93.0 D2：数据洞察摘要（追读债/标注），导演据此决定派发方向。
    if !snapshot.insights.is_empty() {
        out.push_str("Insights（数据洞察摘要）：\n");
        for line in snapshot.insights.iter().take(4) {
            out.push_str(&format!("- {line}\n"));
        }
    }
    // v0.91.0 B1：未结清质量债进巡检（导演可派发修订并按需要登记新债）。
    if !snapshot.debts.is_empty() {
        out.push_str("Debts（未结清质量债，可据此派发修订或 escalate）：\n");
        for d in snapshot.debts.iter().take(5) {
            let chapter = d
                .chapter_number
                .map(|n| format!("第{n}章"))
                .unwrap_or_else(|| "-".to_string());
            out.push_str(&format!(
                "- {} [{}] {} source={} window={} detail={}\n",
                d.id,
                d.severity,
                chapter,
                d.source,
                d.suggested_window.as_deref().unwrap_or("-"),
                d.detail.chars().take(80).collect::<String>()
            ));
        }
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

pub(crate) fn parse_loose<T: serde::de::DeserializeOwned>(raw: &str) -> Option<T> {
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
    // A1: 导演单发回合也开短 trace（planner 阶段一步，生成链路页可见）
    let trace_id = crate::tracing::new_trace_id();
    let trace_store = app
        .try_state::<crate::tracing::TraceStore>()
        .map(|s| s.inner().clone());
    if let Some(store) = &trace_store {
        let _ = store.start_trace(
            &trace_id,
            Some(message_id.to_string()),
            Some(story_id.to_string()),
            Some(user_message.chars().take(200).collect::<String>()),
        );
        let _ = store.add_step(&trace_id, "director.plan", "agency");
    }
    let plan_result = DirectorPlanner::plan(
        pool,
        app,
        &snapshot,
        user_message,
        &history,
        corpus_brief,
        corpus_ids,
    )
    .await;
    if let Some(store) = &trace_store {
        match &plan_result {
            Ok(_) => {
                let _ = store.finish_last_step(&trace_id, "completed", None);
            }
            Err(e) => {
                let _ = store.finish_last_step(&trace_id, "failed", Some(e.to_string()));
            }
        }
    }
    let plan = plan_result?;

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
            scenes: vec![crate::agency::crew::CrewScene {
                scene_id: "sc-12".into(),
                sequence_number: 12,
                chapter_number: Some(8),
                title: "雨夜观测台".into(),
                word_count: 640,
            }],
            debts: vec![],
            insights: vec![],
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
        assert!(
            text.contains("12 | 第8章 | 雨夜观测台") && text.contains("Scenes"),
            "导演提示词应列出场景序号供 target_scene 使用: {text}"
        );
    }

    #[test]
    fn test_validate_action_rejects_missing_target_and_director() {
        // dispatch producer 合法
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("producer".into()),
            instruction: Some("补资产".into()),
            ..Default::default()
        })
        .is_none());
        // 导演不能派自己
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("director".into()),
            instruction: Some("巡检".into()),
            ..Default::default()
        })
        .is_some());
        // retry_task 缺 target_id（真机审批卡报错根因）
        assert!(validate_action(&DirectorActionSpec {
            action_type: "retry_task".into(),
            ..Default::default()
        })
        .is_some());
        // cancel_run 缺 run_id
        assert!(validate_action(&DirectorActionSpec {
            action_type: "cancel_run".into(),
            ..Default::default()
        })
        .is_some());
        // 有 target_id 的 retry_task 合法
        assert!(validate_action(&DirectorActionSpec {
            action_type: "retry_task".into(),
            target_id: Some("task-1".into()),
            ..Default::default()
        })
        .is_none());
        // v0.80.6：lead_writer 修订既有章可带 target_chapter
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("lead_writer".into()),
            instruction: Some("修订第9章".into()),
            target_chapter: Some(9),
            ..Default::default()
        })
        .is_none());
        // v0.82.2：lead_writer 修订指定场景可带 target_scene
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("lead_writer".into()),
            instruction: Some("修订第12个场景".into()),
            target_scene: Some(12),
            ..Default::default()
        })
        .is_none());
        // target_scene 只允许 lead_writer
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("producer".into()),
            instruction: Some("补资产".into()),
            target_scene: Some(12),
            ..Default::default()
        })
        .is_some());
        // 场景序号必须 >= 1，且与 target_chapter 互斥
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("lead_writer".into()),
            instruction: Some("修订".into()),
            target_scene: Some(0),
            ..Default::default()
        })
        .is_some());
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("lead_writer".into()),
            instruction: Some("修订".into()),
            target_chapter: Some(9),
            target_scene: Some(12),
            ..Default::default()
        })
        .is_some());
        // target_chapter 只允许 lead_writer
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("producer".into()),
            instruction: Some("补资产".into()),
            target_chapter: Some(9),
            ..Default::default()
        })
        .is_some());
        // 章号必须 >= 1
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("lead_writer".into()),
            instruction: Some("修订第0章".into()),
            target_chapter: Some(0),
            ..Default::default()
        })
        .is_some());
    }

    /// v0.86.0：target_end_chapter 校验——仅 lead_writer、>= 1、
    /// 与 target_chapter/target_scene 互斥。
    #[test]
    fn test_validate_action_target_end_chapter() {
        // lead_writer 续写到第 60 章合法
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("lead_writer".into()),
            instruction: Some("续写到第一阶段结束".into()),
            target_end_chapter: Some(60),
            ..Default::default()
        })
        .is_none());
        // 只允许 lead_writer
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("producer".into()),
            instruction: Some("补资产".into()),
            target_end_chapter: Some(60),
            ..Default::default()
        })
        .is_some());
        // 必须 >= 1
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("lead_writer".into()),
            instruction: Some("续写".into()),
            target_end_chapter: Some(0),
            ..Default::default()
        })
        .is_some());
        // 与 target_chapter 互斥
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("lead_writer".into()),
            instruction: Some("续写".into()),
            target_end_chapter: Some(60),
            target_chapter: Some(9),
            ..Default::default()
        })
        .is_some());
        // 与 target_scene 互斥
        assert!(validate_action(&DirectorActionSpec {
            action_type: "dispatch".into(),
            role: Some("lead_writer".into()),
            instruction: Some("续写".into()),
            target_end_chapter: Some(60),
            target_scene: Some(12),
            ..Default::default()
        })
        .is_some());
    }

    /// v0.86.0：巡检快照必须输出完整 task/run ID——此前截前 8 位，
    /// 导演照抄去 retry_task 必然「任务不存在: 8c0629c1」。
    #[test]
    fn test_render_prompt_emits_full_ids() {
        let run_id = "ec0796cd-1111-2222-3333-444455556666".to_string();
        let task_id = "8c0629c1-aaaa-bbbb-cccc-ddddeeeeffff".to_string();
        let mut run = crate::agency::models::AgencyRun::new(&run_id, "前提");
        run.status = "failed".into();
        run.phase = "writing".into();
        run.lane = "write".into();
        run.error_message = Some("process exited".into());
        let mut task = crate::agency::models::AgencyAgentTask::new(
            &run_id,
            "s",
            crate::agency::models::AgentRole::LeadWriter,
            "续写第49章",
            None,
            "{}",
        );
        task.id = task_id.clone();
        task.status = "failed".into();
        let snapshot = CrewSnapshot {
            story_id: "s".into(),
            story_title: "超梦神探".into(),
            runs: vec![run],
            tasks: vec![task],
            leases: vec![],
            corpus_count: 0,
            scenes: vec![],
            debts: vec![],
            insights: vec![],
            alerts: vec![],
        };
        let text = render_director_user_prompt(&snapshot, "续写", &[], "", &[]);
        assert!(text.contains(&run_id), "Runs 区块应输出完整 run ID: {text}");
        assert!(
            text.contains(&task_id),
            "Tasks 区块应输出完整 task ID: {text}"
        );
    }

    #[test]
    fn test_render_prompt_includes_debts() {
        let snapshot = CrewSnapshot {
            story_id: "s".into(),
            story_title: "长夜".into(),
            runs: vec![],
            tasks: vec![],
            leases: vec![],
            corpus_count: 0,
            scenes: vec![],
            debts: vec![crate::agency::crew::CrewDebt {
                id: "debt-abc".into(),
                severity: "critical".into(),
                source: "gate".into(),
                detail: "第三章缺少具名阻力".into(),
                chapter_number: Some(3),
                suggested_window: Some("第4章前（尽快）".into()),
            }],
            insights: vec![],
            alerts: vec![],
        };
        let text = render_director_user_prompt(&snapshot, "年报", &[], "", &[]);
        assert!(
            text.contains("Debts"),
            "快照 prompt 应含 Debts 区块: {text}"
        );
        assert!(text.contains("debt-abc"), "Debts 应输出完整债务 ID: {text}");
        assert!(
            text.contains("第三章缺少具名阻力"),
            "Debts 应输出细节: {text}"
        );
    }

    #[test]
    fn test_render_prompt_includes_insights() {
        let snapshot = CrewSnapshot {
            story_id: "s".into(),
            story_title: "长夜".into(),
            runs: vec![],
            tasks: vec![],
            leases: vec![],
            corpus_count: 0,
            scenes: vec![],
            debts: vec![],
            insights: vec!["追读债：active 2 条 / 逾期 1 条 / 合计强度 3.5".into()],
            alerts: vec![],
        };
        let text = render_director_user_prompt(&snapshot, "巡检", &[], "", &[]);
        assert!(text.contains("Insights"), "prompt 应含洞察区块: {text}");
        assert!(text.contains("追读债"), "洞察应输出追读债摘要: {text}");
    }
}
