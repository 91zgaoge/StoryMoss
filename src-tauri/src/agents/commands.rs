#![allow(dead_code)]
//! Agent Commands
//!
//! Tauri commands for agent execution
#![allow(unused_imports)]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Emitter, Manager};
use uuid::Uuid;

use super::service::{AgentService, AgentTask, AgentType};
use crate::{
    db::{
        repositories::{SceneRepository, SceneUpdate},
        DbPool,
    },
    domain::creative_engine::CreativeEnginePort,
    error::AppError,
    subscription::{identity, SubscriptionService, SubscriptionTier},
};

/// 获取当前用户订阅层级（同步）
fn get_user_tier_sync(app_handle: &AppHandle) -> SubscriptionTier {
    if let Some(pool) = app_handle.try_state::<DbPool>() {
        let user_id = identity::resolve_user_id(app_handle, pool.inner());
        if !user_id.is_empty() {
            let service = SubscriptionService::new(pool.inner().clone());
            if let Ok(status) = service.get_or_create_subscription(&user_id) {
                return status.tier.parse().unwrap_or(SubscriptionTier::Free);
            }
        }
    }
    SubscriptionTier::Free
}

static TASK_HANDLES: Lazy<Mutex<HashMap<String, tokio::task::AbortHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// 执行Agent请求
#[derive(Debug, Deserialize)]
pub struct ExecuteAgentRequest {
    pub agent_type: AgentType,
    pub story_id: String,
    pub chapter_number: Option<u32>,
    pub input: String,
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

/// 开始流式Agent执行（通过事件推送进度）
#[command]
pub async fn agent_execute_stream(
    request: ExecuteAgentRequest,
    app_handle: AppHandle,
) -> Result<String, AppError> {
    let task_id = Uuid::new_v4().to_string();

    // 构建上下文
    let context = build_agent_context(&app_handle, &request).await?;

    let tier = get_user_tier_sync(&app_handle);
    let task = AgentTask {
        id: task_id.clone(),
        agent_type: request.agent_type.clone(),
        context,
        input: request.input.clone(),
        parameters: request.parameters.unwrap_or_default(),
        tier: Some(tier),
    };

    // 在后台执行
    let service = AgentService::from_app_handle(app_handle.clone());
    let task_id_clone = task_id.clone();

    let handle = tokio::spawn(async move {
        match service.execute_task(task).await {
            Ok(result) => {
                let _ = app_handle.emit(&format!("agent-complete-{}", task_id_clone), result);
            }
            Err(e) => {
                let _ = app_handle.emit(&format!("agent-error-{}", task_id_clone), e);
            }
        }
        // 完成后清理句柄
        let _ = TASK_HANDLES.lock().unwrap().remove(&task_id_clone);
    });

    TASK_HANDLES
        .lock()
        .unwrap()
        .insert(task_id.clone(), handle.abort_handle());

    Ok(task_id)
}

/// 取消Agent任务
#[command]
pub async fn agent_cancel_task(task_id: String) -> Result<(), AppError> {
    let mut handles = TASK_HANDLES.lock().unwrap();
    if let Some(handle) = handles.remove(&task_id) {
        handle.abort();
        log::info!("[Agent] Task {} aborted", task_id);
    } else {
        log::info!("[Agent] No active task found for {} to cancel", task_id);
    }
    Ok(())
}

/// 取消所有正在执行的 Agent 任务（前端“取消生成”兜底）
#[command]
pub async fn agent_cancel_all_tasks() -> Result<(), AppError> {
    let mut handles = TASK_HANDLES.lock().unwrap();
    let count = handles.len();
    for (task_id, handle) in handles.drain() {
        handle.abort();
        log::info!("[Agent] Task {} aborted via cancel_all", task_id);
    }
    log::info!("[Agent] Cancelled {} active task(s)", count);
    Ok(())
}

// ==================== 文思泉涌：自动续写 ====================

/// 自动续写请求
#[derive(Debug, Deserialize)]
pub struct AutoWriteRequest {
    pub story_id: String,
    pub chapter_id: String,
    pub target_chars: i32,
    pub chars_per_loop: i32,
    /// 外部参考文本（可选），用于风格指纹提取
    #[serde(default)]
    pub reference_text: Option<String>,
    /// 风格权重 0-100（0=叙事优先，100=风格优先）
    #[serde(default = "default_style_weight")]
    pub style_weight: i32,
}

fn default_style_weight() -> i32 {
    50
}

/// 自动续写响应
#[derive(Debug, Serialize)]
pub struct AutoWriteResponse {
    pub task_id: String,
    pub actual_chars: i32,
    pub loops: i32,
    pub status: String,
}

/// 自动续写进度事件
#[derive(Debug, Clone, Serialize)]
pub struct AutoWriteProgressEvent {
    pub task_id: String,
    pub current_chars: i32,
    pub target_chars: i32,
    pub percentage: i32,
    pub current_loop: i32,
    pub status: String,
    // v0.7.8: 风格一致性评分
    pub style_score: f32,
    pub drift_details: Vec<String>,
}

/// 开始自动续写（循环调用 WriterAgent，直到达到目标字数或用户取消）
#[command]
pub async fn auto_write(
    request: AutoWriteRequest,
    app_handle: AppHandle,
) -> Result<AutoWriteResponse, AppError> {
    let task_id = Uuid::new_v4().to_string();

    let pool = app_handle.state::<DbPool>();
    let _user_id = identity::resolve_user_id(&app_handle, pool.inner());
    let scene_repo = SceneRepository::new(pool.inner().clone());

    // v0.8.0: 在调用 coordinator 前捕获当前场景内容，避免生成期间用户编辑导致竞态
    let current_content = scene_repo
        .get_by_id(&request.chapter_id)
        .map_err(AppError::from)?
        .map(|scene| scene.content.unwrap_or_default())
        .unwrap_or_default();

    let task_id_clone = task_id.clone();
    let app_handle_clone = app_handle.clone();
    let story_id = request.story_id.clone();
    let chapter_id = request.chapter_id.clone();
    let target_chars = request.target_chars;
    let chars_per_loop = request.chars_per_loop;
    let reference_text = request.reference_text.clone();
    let style_weight = request.style_weight;
    let current_content_for_task = current_content.clone();

    // 在后台委托 agency coordinator 续写
    let handle = tokio::spawn(async move {
        let pool = app_handle_clone.state::<DbPool>();
        let coordinator = crate::agency::coordinator::AgencyCoordinator::new(
            app_handle_clone.clone(),
            pool.inner().clone(),
        );

        let _ = app_handle_clone.emit(
            &format!("auto-write-progress-{}", task_id_clone),
            AutoWriteProgressEvent {
                task_id: task_id_clone.clone(),
                current_chars: 0,
                target_chars,
                percentage: 0,
                current_loop: 0,
                status: "writing".to_string(),
                style_score: 0.0,
                drift_details: Vec::new(),
            },
        );

        // 构建携带请求参数的任务描述
        let mut task_description = format!(
            "请继续续写以下内容。目标字数：{}，每次续写参考字数：{}。请直接输出续写内容，不要重复前文，保持故事连贯性和风格一致性。",
            target_chars, chars_per_loop
        );
        if let Some(ref rt) = reference_text {
            if !rt.is_empty() {
                task_description.push_str(&format!("\n\n【参考文本】\n{}", rt));
            }
        }
        if style_weight != 50 {
            task_description.push_str(&format!(
                "\n【风格权重】{}（0=叙事优先，100=风格优先）",
                style_weight
            ));
        }

        match coordinator
            .run_role_task(
                &task_id_clone,
                &story_id,
                crate::agency::models::AgentRole::Writer,
                &current_content_for_task,
                &task_description,
            )
            .await
        {
            Ok(result) => {
                let new_content = current_content_for_task + &result.output;

                let pool = app_handle_clone.state::<DbPool>();
                let scene_repo = SceneRepository::new(pool.inner().clone());
                if let Err(e) = scene_repo.update(
                    &chapter_id,
                    &SceneUpdate {
                        title: None,
                        content: Some(new_content.clone()),
                        ..Default::default()
                    },
                ) {
                    log::error!("[auto_write] Failed to update scene {}: {}", chapter_id, e);
                    let _ = app_handle_clone.emit(
                        &format!("auto-write-error-{}", task_id_clone),
                        AppError::from(e),
                    );
                    return;
                }

                let content_len = new_content.chars().count() as i32;
                let _ = app_handle_clone.emit(
                    &format!("auto-write-complete-{}", task_id_clone),
                    AutoWriteProgressEvent {
                        task_id: task_id_clone.clone(),
                        current_chars: content_len,
                        target_chars,
                        percentage: 100,
                        current_loop: 1,
                        status: "completed".to_string(),
                        style_score: 0.0,
                        drift_details: Vec::new(),
                    },
                );
                log::info!(
                    "[auto_write] Saved {} chars to scene {}",
                    content_len,
                    chapter_id
                );
            }
            Err(e) => {
                let msg = format!("[auto_write] agency run_role_task failed: {}", e);
                log::error!("{}", msg);
                let _ = app_handle_clone.emit(
                    &format!("auto-write-error-{}", task_id_clone),
                    AppError::internal(msg),
                );
            }
        }

        // 清理句柄
        let _ = TASK_HANDLES.lock().unwrap().remove(&task_id_clone);
    });

    TASK_HANDLES
        .lock()
        .unwrap()
        .insert(task_id.clone(), handle.abort_handle());

    Ok(AutoWriteResponse {
        task_id,
        actual_chars: 0,
        loops: 0,
        status: "started".to_string(),
    })
}

// ==================== 文思泉涌：自动修改 ====================

/// 自动修改请求
#[derive(Debug, Deserialize)]
pub struct AutoReviseRequest {
    pub story_id: String,
    pub chapter_id: Option<String>,
    pub scope: String, // "full" | "chapter" | "selection"
    pub selected_text: Option<String>,
    pub revision_type: String, /* "style" | "plot" | "dialogue" | "description" |
                                * "comprehensive" | "editor_qc" */
    /// v0.59.0：编辑审计（后台质检）给出的具体问题清单，逐条注入修订要求。
    /// 缺省（旧调用方）不影响既有行为。
    #[serde(default)]
    pub extra_instruction: Option<String>,
}

/// 自动修改响应
#[derive(Debug, Serialize)]
pub struct AutoReviseResponse {
    pub task_id: String,
    pub revised_text: String,
    pub status: String,
}

/// 自动修改进度事件
#[derive(Debug, Serialize, Clone)]
pub struct AutoReviseProgressEvent {
    pub task_id: String,
    pub stage: String,
    pub progress: f32,
    pub message: String,
    pub revised_text: Option<String>,
}

/// 自动修改指令映射
fn get_revision_instruction(revision_type: &str) -> &'static str {
    match revision_type {
        "style" => "优化语言风格，提升文学性和节奏感，让文字更流畅优美。",
        "plot" => "强化情节张力，增加伏笔和转折，让故事更加引人入胜。",
        "dialogue" => "让人物对话更生动立体，加入动作神态描写，避免干巴巴的对话。",
        "description" => "增加感官细节，让画面更具体可感，调动读者的五感。",
        // v0.59.0：编辑审计闭环——按下述问题逐条修正，只改问题涉及处，
        // 其余文字保持原样（避免整章重写把没问题的段落也改坏）。
        "editor_qc" => {
            "严格按下述「编辑审计问题」逐条修正：被点名的问题必须解决，未点名处保持原样，不得顺手扩写或改动无关段落。"
        }
        _ => "综合以上所有方面进行全面修改，提升整体质量。",
    }
}

/// 构建自动修改任务描述
fn build_revise_task_description(
    scope: &str,
    target_text: &str,
    story_id: &str,
    chapter_id: Option<&str>,
    revision_type: &str,
    extra_instruction: Option<&str>,
) -> String {
    let revision_instruction = get_revision_instruction(revision_type);
    let extra = match extra_instruction.map(str::trim) {
        Some(s) if !s.is_empty() => format!("\n【编辑审计问题（逐条修正）】\n{}\n", s),
        _ => String::new(),
    };
    if scope == "selection" {
        format!(
            "请修订以下选中文本：\n{}\n该文本来自故事 {} 第 {} 章。修改要求：{}。{}请输出修改后的完整文本。",
            target_text,
            story_id,
            chapter_id.unwrap_or("未知"),
            revision_instruction,
            extra
        )
    } else {
        format!(
            "请修订故事 {} 第 {} 章（scope={}）。修改要求：{}。{}请输出修改后的完整文本。",
            story_id,
            chapter_id.unwrap_or("未知"),
            scope,
            revision_instruction,
            extra
        )
    }
}

/// 执行自动修改
#[command]
pub async fn auto_revise(
    request: AutoReviseRequest,
    app_handle: AppHandle,
) -> Result<AutoReviseResponse, AppError> {
    let task_id = Uuid::new_v4().to_string();

    // 预估算文本长度用于配额检查
    let _text_len = match request.scope.as_str() {
        "selection" => request
            .selected_text
            .as_ref()
            .map(|s| s.chars().count() as i32)
            .unwrap_or(0),
        "chapter" | "scene" => {
            if let Some(ref sid) = request.chapter_id {
                let pool = app_handle.state::<DbPool>();
                let scene_repo = SceneRepository::new(pool.inner().clone());
                scene_repo
                    .get_by_id(sid)
                    .map_err(AppError::from)?
                    .map(|s| s.content.unwrap_or_default().chars().count() as i32)
                    .unwrap_or(0)
            } else {
                0
            }
        }
        _ => {
            let pool = app_handle.state::<DbPool>();
            let scene_repo = SceneRepository::new(pool.inner().clone());
            let scenes = scene_repo
                .get_by_story(&request.story_id)
                .map_err(AppError::from)?;
            scenes
                .into_iter()
                .filter_map(|s| s.content)
                .map(|c| c.chars().count() as i32)
                .sum()
        }
    };

    let task_id_clone = task_id.clone();
    let app_handle_clone = app_handle.clone();
    let story_id = request.story_id.clone();
    let chapter_id = request.chapter_id.clone();
    let scope = request.scope.clone();
    let selected_text = request.selected_text.clone();
    let revision_type = request.revision_type.clone();
    let request_extra_instruction = request.extra_instruction.clone();

    // 在后台委托 agency coordinator 审阅/修订
    let handle = tokio::spawn(async move {
        let _ = app_handle_clone.emit(
            &format!("auto-revise-progress-{}", task_id_clone),
            AutoReviseProgressEvent {
                task_id: task_id_clone.clone(),
                stage: "preparing".to_string(),
                progress: 0.1,
                message: "读取目标文本...".to_string(),
                revised_text: None,
            },
        );

        if !TASK_HANDLES.lock().unwrap().contains_key(&task_id_clone) {
            return;
        }

        let pool = app_handle_clone.state::<DbPool>();
        let scene_repo = SceneRepository::new(pool.inner().clone());
        let target_text = match scope.as_str() {
            "chapter" | "scene" => {
                if let Some(ref sid) = chapter_id {
                    scene_repo
                        .get_by_id(sid)
                        .map(|s| {
                            s.map(|scene| scene.content.unwrap_or_default())
                                .unwrap_or_default()
                        })
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            }
            "selection" => selected_text.unwrap_or_default(),
            _ => {
                let scenes = scene_repo.get_by_story(&story_id).unwrap_or_default();
                scenes
                    .into_iter()
                    .filter_map(|s| s.content)
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
        };

        if target_text.is_empty() {
            let _ = app_handle_clone.emit(
                &format!("auto-revise-error-{}", task_id_clone),
                AppError::internal("目标文本为空".to_string()),
            );
            return;
        }

        let _ = app_handle_clone.emit(
            &format!("auto-revise-progress-{}", task_id_clone),
            AutoReviseProgressEvent {
                task_id: task_id_clone.clone(),
                stage: "revising".to_string(),
                progress: 0.3,
                message: "AI 正在修改文本...".to_string(),
                revised_text: None,
            },
        );

        let task_description = build_revise_task_description(
            &scope,
            &target_text,
            &story_id,
            chapter_id.as_deref(),
            &revision_type,
            request_extra_instruction.as_deref(),
        );

        let pool = app_handle_clone.state::<DbPool>();
        let coordinator = crate::agency::coordinator::AgencyCoordinator::new(
            app_handle_clone.clone(),
            pool.inner().clone(),
        );

        match coordinator
            .run_role_task(
                &task_id_clone,
                &story_id,
                crate::agency::models::AgentRole::Writer,
                &target_text,
                &task_description,
            )
            .await
        {
            Ok(result) => {
                let revised_text = result.output;

                let _ = app_handle_clone.emit(
                    &format!("auto-revise-progress-{}", task_id_clone),
                    AutoReviseProgressEvent {
                        task_id: task_id_clone.clone(),
                        stage: "saving".to_string(),
                        progress: 0.8,
                        message: "保存修改结果...".to_string(),
                        revised_text: None,
                    },
                );

                if let Some(ref sid) = chapter_id {
                    if scope == "chapter" || scope == "scene" {
                        let pool = app_handle_clone.state::<DbPool>();
                        let scene_repo = SceneRepository::new(pool.inner().clone());
                        let _ = scene_repo.update(
                            sid,
                            &SceneUpdate {
                                title: None,
                                content: Some(revised_text.clone()),
                                ..Default::default()
                            },
                        );
                        log::info!("[auto_revise] Saved revised content to scene {}", sid);
                    }
                }

                let _ = app_handle_clone.emit(
                    &format!("auto-revise-complete-{}", task_id_clone),
                    AutoReviseProgressEvent {
                        task_id: task_id_clone.clone(),
                        stage: "completed".to_string(),
                        progress: 1.0,
                        message: "修改完成".to_string(),
                        revised_text: Some(revised_text),
                    },
                );
            }
            Err(e) => {
                let msg = format!("[auto_revise] agency run_role_task failed: {}", e);
                log::error!("{}", msg);
                let _ = app_handle_clone.emit(
                    &format!("auto-revise-error-{}", task_id_clone),
                    AppError::internal(msg),
                );
            }
        }

        // 清理句柄
        let _ = TASK_HANDLES.lock().unwrap().remove(&task_id_clone);
    });

    TASK_HANDLES
        .lock()
        .unwrap()
        .insert(task_id.clone(), handle.abort_handle());

    Ok(AutoReviseResponse {
        task_id,
        revised_text: String::new(),
        status: "started".to_string(),
    })
}

/// 构建Agent上下文
///
/// 使用 ContextOptimizer (L0/L1/L2) 从数据库读取真实故事数据，
/// 为Agent提供完整且紧凑的创作上下文。
/// L0: 静态元数据 | L1: 结构化知识 | L2: 动态工具检索
pub(crate) async fn build_agent_context(
    app_handle: &AppHandle,
    request: &ExecuteAgentRequest,
) -> Result<super::AgentContext, AppError> {
    use tauri::Manager;

    use crate::{
        agents::context_optimizer::{default_writing_tools, ContextOptimizer},
        db::DbPool,
    };

    let pool = app_handle.state::<DbPool>();
    let vector_store = app_handle.state::<std::sync::Arc<dyn crate::ports::VectorStore>>();
    let creative_engine = app_handle
        .state::<Arc<dyn CreativeEnginePort>>()
        .inner()
        .clone();
    let story_id = request.story_id.clone();
    let chapter_number = request.chapter_number.unwrap_or(1);

    let optimizer = ContextOptimizer::new(
        pool.inner().clone(),
        vector_store.inner().clone(),
        creative_engine,
    );

    // 根据 Agent 类型选择默认 L2 工具
    let l2_tools = match request.agent_type {
        super::service::AgentType::Writer => default_writing_tools(chapter_number),
        super::service::AgentType::Inspector => {
            crate::agents::context_optimizer::default_inspection_tools(
                &request.input,
                chapter_number,
            )
        }
        _ => vec![],
    };

    let mut context = match optimizer
        .build_full_context(&story_id, chapter_number, None, None, l2_tools)
        .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            log::warn!(
                "[build_agent_context] ContextOptimizer failed: {}, falling back to minimal",
                e
            );
            let _ = app_handle.emit(
                "context-degraded",
                serde_json::json!({
                    "story_id": story_id,
                    "reason": format!("ContextOptimizer failed: {}", e),
                    "fallback": "minimal",
                }),
            );
            return Ok(super::AgentContext::minimal(story_id, String::new()));
        }
    };

    // 注入未解决的伏笔提示到世界观规则中
    // v0.14.0: spawn_blocking 包裹同步 DB，避免阻塞 tokio worker
    {
        let engine = app_handle
            .state::<Arc<dyn CreativeEnginePort>>()
            .inner()
            .clone();
        let story_id_for_hints = story_id.clone();
        let hints = tokio::task::spawn_blocking(move || {
            engine.get_foreshadowing_hints(&story_id_for_hints, 5)
        })
        .await
        .map_err(|e| AppError::internal(format!("foreshadowing hints task failed: {}", e)))?;
        match hints {
            Ok(hints) if !hints.is_empty() => {
                let hints_text = format!("\n\n【伏笔提醒】\n{}", hints.join("\n"));
                context.world.world_rules =
                    Some(context.world.world_rules.unwrap_or_default() + &hints_text);
                log::info!(
                    "[build_agent_context] Injected {} foreshadowing hints",
                    hints.len()
                );
            }
            Ok(_) => {}
            Err(e) => log::warn!("[build_agent_context] ForeshadowingTracker failed: {}", e),
        }
    }
    if request.input.len() >= 10 {
        match crate::knowledge_base::kb_search(
            vector_store.inner().as_ref(),
            &story_id,
            &request.input,
            5,
            None,
            "hybrid",
        )
        .await
        {
            Ok(results) if !results.is_empty() => {
                let lines: Vec<String> = results
                    .iter()
                    .map(|r| format!("[第{}章 相似度{:.2}] {}", r.chapter_number, r.score, r.text))
                    .collect();
                let semantic_text = format!("\n\n【相关记忆检索】\n{}", lines.join("\n"));
                context.world.scene_structure =
                    Some(context.world.scene_structure.unwrap_or_default() + &semantic_text);
                log::info!(
                    "[build_agent_context] Injected {} semantic search results",
                    results.len()
                );
            }
            Ok(_) => {}
            Err(e) => {
                log::warn!(
                    "[build_agent_context] Semantic search failed: {}, skipping",
                    e
                );
            }
        }
    }

    // 注入 story 的 style_dna_id
    // v0.14.0: spawn_blocking 包裹同步 DB
    {
        let pool_for_story = pool.inner().clone();
        let story_id_for_story = story_id.clone();
        let story_opt = tokio::task::spawn_blocking(move || {
            let story_repo = crate::db::repositories::StoryRepository::new(pool_for_story);
            story_repo.get_by_id(&story_id_for_story)
        })
        .await
        .map_err(|e| AppError::internal(format!("story lookup task failed: {}", e)))?;
        if let Ok(Some(story)) = story_opt {
            context.style.style_dna_id = story.style_dna_id;
            if context.style.style_dna_id.is_some() {
                log::info!(
                    "[build_agent_context] Using style_dna_id: {:?}",
                    context.style.style_dna_id
                );
            }
            // 注入方法论配置
            context.world.methodology_id = story.methodology_id.clone();
            context.world.methodology_step = story.methodology_step.map(|s| s.to_string());
            if context.world.methodology_id.is_some() {
                log::info!(
                    "[build_agent_context] Using methodology_id: {:?}, step: {:?}",
                    context.world.methodology_id,
                    context.world.methodology_step
                );
            }
        }
    }

    // 注入规范状态快照
    // v0.14.0: spawn_blocking 包裹多表聚合同步查询
    {
        let pool_for_snapshot = pool.inner().clone();
        let story_id_for_snapshot = story_id.clone();
        let snapshot_result = tokio::task::spawn_blocking(move || {
            let cs_manager = crate::canonical_state::CanonicalStateManager::new(pool_for_snapshot);
            cs_manager.get_snapshot_sync(&story_id_for_snapshot)
        })
        .await
        .map_err(|e| AppError::internal(format!("canonical snapshot task failed: {}", e)))?;
        match snapshot_result {
            Ok(snapshot) => {
                // 追加世界观事实和伏笔到 world_rules
                let mut world_parts = Vec::new();
                if let Some(ref existing) = context.world.world_rules {
                    world_parts.push(existing.clone());
                }

                if !snapshot.world_facts.is_empty() {
                    world_parts.push("【世界观事实】".to_string());
                    for fact in snapshot.world_facts.iter().take(10) {
                        world_parts.push(format!("- [{}] {}", fact.fact_type, fact.content));
                    }
                }

                if !snapshot.story_context.pending_payoffs.is_empty() {
                    world_parts.push("【待回收伏笔】".to_string());
                    for payoff in snapshot.story_context.pending_payoffs.iter().take(5) {
                        world_parts.push(format!(
                            "- [重要度{}] {}",
                            payoff.importance, payoff.content
                        ));
                    }
                }

                if !snapshot.story_context.overdue_payoffs.is_empty() {
                    world_parts.push("【逾期伏笔】".to_string());
                    for payoff in snapshot.story_context.overdue_payoffs.iter().take(5) {
                        world_parts.push(format!(
                            "- [重要度{}] {}",
                            payoff.importance, payoff.content
                        ));
                    }
                }

                if world_parts.len() > 1 {
                    context.world.world_rules = Some(world_parts.join("\n"));
                }

                // 追加叙事阶段和时间线到 scene_structure
                let mut scene_parts = Vec::new();
                if let Some(ref existing) = context.world.scene_structure {
                    scene_parts.push(existing.clone());
                }

                scene_parts.push(format!(
                    "【叙事阶段】{}\n{}",
                    snapshot.narrative_phase,
                    snapshot.narrative_phase.writer_guidance()
                ));

                if !snapshot.timeline.is_empty() {
                    let recent_events: Vec<String> = snapshot
                        .timeline
                        .iter()
                        .rev()
                        .take(5)
                        .rev()
                        .map(|e| format!("场景{}: {}", e.sequence_number, e.event_summary))
                        .collect();
                    scene_parts.push(format!("【近期时间线】\n{}", recent_events.join("\n")));
                }

                if !snapshot.story_context.active_conflicts.is_empty() {
                    let conflicts: Vec<String> = snapshot
                        .story_context
                        .active_conflicts
                        .iter()
                        .take(5)
                        .map(|c| {
                            format!(
                                "- [{}] {} (涉及: {})",
                                c.conflict_type,
                                c.stakes,
                                c.parties.join(", ")
                            )
                        })
                        .collect();
                    scene_parts.push(format!("【活跃冲突】\n{}", conflicts.join("\n")));
                }

                context.world.scene_structure = Some(scene_parts.join("\n"));

                log::info!(
                    "[build_agent_context] CanonicalState injected: phase={}, facts={}, \
                     pending={}, overdue={}",
                    snapshot.narrative_phase,
                    snapshot.world_facts.len(),
                    snapshot.story_context.pending_payoffs.len(),
                    snapshot.story_context.overdue_payoffs.len()
                );
            }
            Err(e) => {
                log::warn!(
                    "[build_agent_context] CanonicalStateManager failed: {}, skipping",
                    e
                );
            }
        }
    }

    // current_content 和 selected_text 由调用方在返回后填充
    //（参见 auto_write、auto_revise 等调用点）

    Ok(context)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn auto_write_request_deserialization() {
        let value = json!({
            "story_id": "story-1",
            "chapter_id": "chapter-1",
            "target_chars": 1000,
            "chars_per_loop": 200,
            "reference_text": "参考",
            "style_weight": 80
        });
        let req: AutoWriteRequest = serde_json::from_value(value).unwrap();
        assert_eq!(req.story_id, "story-1");
        assert_eq!(req.chapter_id, "chapter-1");
        assert_eq!(req.target_chars, 1000);
        assert_eq!(req.chars_per_loop, 200);
        assert_eq!(req.reference_text, Some("参考".to_string()));
        assert_eq!(req.style_weight, 80);
    }

    #[test]
    fn auto_write_request_uses_default_style_weight() {
        let value = json!({
            "story_id": "story-1",
            "chapter_id": "chapter-1",
            "target_chars": 1000,
            "chars_per_loop": 200
        });
        let req: AutoWriteRequest = serde_json::from_value(value).unwrap();
        assert_eq!(req.style_weight, 50);
    }

    #[test]
    fn auto_revise_request_deserialization() {
        let value = json!({
            "story_id": "story-1",
            "chapter_id": "chapter-1",
            "scope": "selection",
            "selected_text": "选中段落",
            "revision_type": "style"        });
        let req: AutoReviseRequest = serde_json::from_value(value).unwrap();
        assert_eq!(req.story_id, "story-1");
        assert_eq!(req.chapter_id, Some("chapter-1".to_string()));
        assert_eq!(req.scope, "selection");
        assert_eq!(req.selected_text, Some("选中段落".to_string()));
        assert_eq!(req.revision_type, "style");
    }

    #[test]
    fn auto_write_response_serializes() {
        let resp = AutoWriteResponse {
            task_id: "task-1".to_string(),
            actual_chars: 500,
            loops: 2,
            status: "completed".to_string(),
        };
        let value = serde_json::to_value(&resp).unwrap();
        assert_eq!(value["task_id"], "task-1");
        assert_eq!(value["actual_chars"], 500);
        assert_eq!(value["loops"], 2);
        assert_eq!(value["status"], "completed");
    }

    #[test]
    fn auto_revise_response_serializes() {
        let resp = AutoReviseResponse {
            task_id: "task-1".to_string(),
            revised_text: "修订后文本".to_string(),
            status: "completed".to_string(),
        };
        let value = serde_json::to_value(&resp).unwrap();
        assert_eq!(value["task_id"], "task-1");
        assert_eq!(value["revised_text"], "修订后文本");
        assert_eq!(value["status"], "completed");
    }

    #[test]
    fn revise_task_description_selection_includes_selected_text() {
        let desc = build_revise_task_description(
            "selection",
            "选中段落",
            "story-1",
            Some("chapter-1"),
            "style",
            None,
        );
        assert!(desc.contains("请修订以下选中文本"));
        assert!(desc.contains("选中段落"));
        assert!(desc.contains("story-1"));
        assert!(desc.contains("chapter-1"));
    }

    #[test]
    fn revise_task_description_chapter_uses_chapter_scope() {
        let desc = build_revise_task_description(
            "chapter",
            "章节全文",
            "story-1",
            Some("chapter-1"),
            "plot",
            None,
        );
        assert!(desc.contains("请修订故事 story-1 第 chapter-1 章"));
        assert!(desc.contains("scope=chapter"));
        // 旧调用方（无 extra_instruction）不得出现审计问题段
        assert!(!desc.contains("编辑审计问题"));
    }

    /// v0.59.0 续写质检闭环：editor_qc 类型必须逐条注入审计问题，且要求
    /// 只改被点名处（避免整章重写把没问题的段落也改坏）。
    #[test]
    fn revise_task_description_injects_editor_qc_issues() {
        let desc = build_revise_task_description(
            "chapter",
            "章节全文",
            "story-1",
            Some("scene-1"),
            "editor_qc",
            Some("1. 明成公主已死却再次开口\n2. 场景重复复述上一拍"),
        );
        assert!(desc.contains("未点名处保持原样"));
        assert!(desc.contains("【编辑审计问题（逐条修正）】"));
        assert!(desc.contains("明成公主已死却再次开口"));
        assert!(desc.contains("场景重复复述上一拍"));
    }

    /// 空白 extra_instruction 视同未提供，不产生空标题段。
    #[test]
    fn revise_task_description_ignores_blank_extra_instruction() {
        let desc = build_revise_task_description(
            "chapter",
            "章节全文",
            "story-1",
            Some("scene-1"),
            "editor_qc",
            Some("   \n  "),
        );
        // editor_qc 的指令本身提到「编辑审计问题」，这里断言的是问题段标题未生成
        assert!(!desc.contains("【编辑审计问题（逐条修正）】"));
    }

    /// 旧前端（缺 extra_instruction 字段）反序列化必须成功且为 None。
    #[test]
    fn auto_revise_request_extra_instruction_defaults_to_none() {
        let value = json!({
            "story_id": "story-1",
            "chapter_id": "scene-1",
            "scope": "chapter",
            "revision_type": "editor_qc"
        });
        let req: AutoReviseRequest = serde_json::from_value(value).unwrap();
        assert_eq!(req.extra_instruction, None);
    }
}
