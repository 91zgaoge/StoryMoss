//! Agent Commands
//!
//! Tauri commands for agent execution
#![allow(dead_code)]
#![allow(unused_imports)]

use super::service::{AgentService, AgentTask, AgentType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use tauri::{command, AppHandle, Emitter, Manager, State};
use uuid::Uuid;
use crate::db::{DbPool, CreateStoryRequest};
use crate::error::AppError;
use crate::db::repositories::{StoryRepository};
use crate::db::repositories_v3::{SceneRepository, SceneUpdate};
use crate::subscription::{SubscriptionService, SubscriptionTier};
use crate::state_sync::StateSync;

/// 获取当前用户订阅层级（同步）
fn get_user_tier_sync(app_handle: &AppHandle) -> SubscriptionTier {
    let app_dir = match app_handle.path().app_data_dir() {
        Ok(d) => d,
        Err(_) => return SubscriptionTier::Free,
    };
    let machine_id_path = app_dir.join(".machine_id");
    let user_id = if machine_id_path.exists() {
        std::fs::read_to_string(&machine_id_path).unwrap_or_default().trim().to_string()
    } else {
        return SubscriptionTier::Free;
    };
    if user_id.is_empty() {
        return SubscriptionTier::Free;
    }
    if let Some(pool) = app_handle.try_state::<DbPool>() {
        let service = SubscriptionService::new(pool.inner().clone());
        if let Ok(status) = service.get_or_create_subscription(&user_id) {
            return status.tier.parse().unwrap_or(SubscriptionTier::Free);
        }
    }
    SubscriptionTier::Free
}

/// 获取用户 ID
fn get_user_id(app_handle: &AppHandle) -> String {
    let app_dir = app_handle.path().app_data_dir().unwrap_or_default();
    let machine_id_path = app_dir.join(".machine_id");
    if machine_id_path.exists() {
        std::fs::read_to_string(&machine_id_path).unwrap_or_default().trim().to_string()
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        let _ = std::fs::create_dir_all(&app_dir);
        let _ = std::fs::write(&machine_id_path, &id);
        id
    }
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

/// Agent执行响应
#[derive(Debug, Serialize)]
pub struct ExecuteAgentResponse {
    pub task_id: String,
    pub result: Option<super::AgentResult>,
    pub error: Option<String>,
}

/// 同步执行Agent（所有功能已免费，不限制配额）
#[command]
pub async fn agent_execute(
    request: ExecuteAgentRequest,
    app_handle: AppHandle,
) -> Result<ExecuteAgentResponse, AppError> {
    let task_id = Uuid::new_v4().to_string();
    
    // 构建上下文
    let context = build_agent_context(&app_handle, &request).await?;
    
    let tier = get_user_tier_sync(&app_handle);
    let task = AgentTask {
        id: task_id.clone(),
        agent_type: request.agent_type,
        context,
        input: request.input,
        parameters: request.parameters.unwrap_or_default(),
        tier: Some(tier),
    };
    
    let service = AgentService::new(app_handle.clone());
    
    match service.execute_task(task).await {
        Ok(result) => {
            Ok(ExecuteAgentResponse {
                task_id,
                result: Some(result),
                error: None,
            })
        }
        Err(e) => Ok(ExecuteAgentResponse {
            task_id,
            result: None,
            error: Some(e.to_string()),
        }),
    }
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
    let service = AgentService::new(app_handle.clone());
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

    TASK_HANDLES.lock().unwrap().insert(task_id.clone(), handle.abort_handle());

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

/// 获取Agent执行状态
#[command]
pub fn agent_get_status(task_id: String) -> String {
    let handles = TASK_HANDLES.lock().unwrap();
    if handles.contains_key(&task_id) {
        "running".to_string()
    } else {
        "completed_or_not_found".to_string()
    }
}

/// 正文助手(WriterAgent)专用请求
#[derive(Debug, Deserialize)]
pub struct WriterAgentRequest {
    pub story_id: String,
    pub chapter_number: Option<u32>,
    pub current_content: String,
    pub selected_text: Option<String>,
    pub instruction: String,
}

/// 正文助手执行响应
#[derive(Debug, Serialize)]
pub struct WriterAgentResponse {
    pub content: String,
    pub story_id: Option<String>,
    pub chapter_id: Option<String>,
    pub task_id: String,
}

/// 执行正文助手任务（手工续写 — 已免费开放，不限制配额）
#[command]
pub async fn writer_agent_execute(
    request: WriterAgentRequest,
    app_handle: AppHandle,
    automation_service: State<'_, crate::automation::service::AutomationService>,
) -> Result<WriterAgentResponse, AppError> {
    let mut story_id = request.story_id.clone();
    let mut chapter_number = request.chapter_number.unwrap_or(1);
    let mut created_chapter_id: Option<String> = None;

    // 如果没有 story_id，自动创建新作品和第一场景
    if story_id.is_empty() {
        let pool = app_handle.state::<DbPool>();
        let story_repo = StoryRepository::new(pool.inner().clone());
        let scene_repo = SceneRepository::new(pool.inner().clone());

        let story = story_repo.create(CreateStoryRequest {
            title: "未命名作品".to_string(),
            description: Some(request.instruction.clone()),
            genre: Some("小说".to_string()),
            style_dna_id: None,
        }).map_err(AppError::from)?;

        let scene = scene_repo.create(&story.id, 1, Some("第一场景")).map_err(AppError::from)?;

        story_id = story.id.clone();
        chapter_number = 1;
        created_chapter_id = Some(scene.id.clone());

        // 发射 StateSync 事件
        StateSync::emit_story_created(&app_handle, &story.id, &story.title);
        StateSync::emit_scene_created(&app_handle, &story.id, &scene.id, Some("第一场景"));

        // 通知幕前切换到新场景
        let event = crate::window::FrontstageEvent::ChapterSwitch {
            story_id: story_id.clone(),
            chapter_id: scene.id.clone(),
            title: "第一场景".to_string(),
            content: scene.content.clone(),
        };
        let _ = crate::window::WindowManager::send_to_frontstage(&app_handle, event);

        // 通知幕后作品列表刷新
        let _ = crate::window::WindowManager::send_to_backstage(
            &app_handle,
            crate::window::BackstageEvent::DataRefresh { entity: "stories".to_string() }
        );
    }

    // v0.8.0: 触发 SceneGenerationRequested 事件
    if let Some(ref scene_id) = created_chapter_id {
        let _ = automation_service.trigger_event(
            crate::automation::triggers::TriggerEvent::SceneGenerationRequested {
                story_id: story_id.clone(),
                scene_id: scene_id.clone(),
            }
        ).await;
    }

    let mut context = build_agent_context(
        &app_handle,
        &ExecuteAgentRequest {
            agent_type: AgentType::Writer,
            story_id: story_id.clone(),
            chapter_number: Some(chapter_number),
            input: request.instruction.clone(),
            parameters: None,
        },
    ).await?;

    context.current_content = Some(request.current_content);
    context.selected_text = request.selected_text;

    let tier = get_user_tier_sync(&app_handle);
    let task = AgentTask {
        id: Uuid::new_v4().to_string(),
        agent_type: AgentType::Writer,
        context,
        input: request.instruction,
        parameters: std::collections::HashMap::new(),
        tier: Some(tier),
    };

    let task_id = task.id.clone();
    let service = AgentService::new(app_handle.clone());

    // 读取 AgentOrchestrator 配置
    let app_dir = app_handle.path().app_data_dir().unwrap_or_default();
    let orchestrator_config = crate::config::AppConfig::load(&app_dir)
        .map(|c| super::orchestrator::WorkflowConfig {
            rewrite_threshold: c.rewrite_threshold,
            max_feedback_loops: c.max_feedback_loops,
            keep_revision_history: true,
        })
        .unwrap_or_default();

    // 使用 AgentOrchestrator 执行 Writer → Inspector → Writer 闭环优化
    let orchestrator = super::orchestrator::AgentOrchestrator::new(service, orchestrator_config, app_handle.clone());

    match orchestrator.generate(task, super::orchestrator::GenerationMode::Full).await {
        Ok(workflow_result) => {
            log::info!("[writer_agent_execute] Orchestrator completed: score={:.2}, rewritten={}", 
                workflow_result.final_score, workflow_result.was_rewritten);
            
            // v0.8.0: 触发 SceneGenerated 事件
            if let Some(ref scene_id) = created_chapter_id {
                let _ = automation_service.trigger_event(
                    crate::automation::triggers::TriggerEvent::SceneGenerated {
                        story_id: story_id.clone(),
                        scene_id: scene_id.clone(),
                    }
                ).await;
            }

            // 如果创建了新场景，把生成的内容保存到数据库
            if let Some(ref scene_id) = created_chapter_id {
                let pool = app_handle.state::<DbPool>();
                let scene_repo = SceneRepository::new(pool.inner().clone());
                let _ = scene_repo.update(
                    scene_id,
                    &SceneUpdate {
                        title: Some("第一场景".to_string()),
                        content: Some(workflow_result.final_content.clone()),
                        ..Default::default()
                    },
                );

                // 同时推送内容更新事件到幕前
                let event = crate::window::FrontstageEvent::ContentUpdate {
                    text: workflow_result.final_content.clone(),
                    chapter_id: scene_id.clone(),
                };
                let _ = crate::window::WindowManager::send_to_frontstage(&app_handle, event);
            }

            Ok(WriterAgentResponse {
                content: workflow_result.final_content,
                story_id: Some(story_id),
                task_id,
                chapter_id: created_chapter_id,
            })
        },
        Err(e) => Err(e),
    }
}

// ==================== 文思泉涌：自动续写 ====================

/// 自动续写请求
#[derive(Debug, Deserialize)]
pub struct AutoWriteRequest {
    pub story_id: String,
    pub chapter_id: String,
    pub target_chars: i32,
    pub chars_per_loop: i32,
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
}

/// 开始自动续写（循环调用 WriterAgent，直到达到目标字数或用户取消）
#[command]
pub async fn auto_write(
    request: AutoWriteRequest,
    app_handle: AppHandle,
) -> Result<AutoWriteResponse, AppError> {
    let task_id = Uuid::new_v4().to_string();
    let _user_id = get_user_id(&app_handle);

    let pool = app_handle.state::<DbPool>();
    let scene_repo = SceneRepository::new(pool.inner().clone());

    // 读取当前场景内容作为上下文
    let current_content = scene_repo.get_by_id(&request.chapter_id)
        .map_err(AppError::from)?
        .map(|s| s.content.unwrap_or_default())
        .unwrap_or_default();

    let task_id_clone = task_id.clone();
    let app_handle_clone = app_handle.clone();
    let story_id = request.story_id.clone();
    let chapter_id = request.chapter_id.clone();
    let target_chars = request.target_chars;
    let chars_per_loop = request.chars_per_loop;

    // 在后台执行循环续写
    let handle = tokio::spawn(async move {
        let mut total_written = 0i32;
        let mut loop_count = 0i32;
        let service = AgentService::new(app_handle_clone.clone());
        let mut accumulated_content = current_content;

        while total_written < target_chars {
            // 检查是否被取消
            if !TASK_HANDLES.lock().unwrap().contains_key(&task_id_clone) {
                log::info!("[auto_write] Task {} cancelled", task_id_clone);
                break;
            }

            let remaining = target_chars - total_written;
            let this_loop_chars = chars_per_loop.min(remaining);

            // 构建续写 prompt
            let instruction = format!("请继续续写以下内容，续写约 {} 字，保持故事连贯性和风格一致性。请直接输出续写内容，不要重复前文。", this_loop_chars);

            let mut context = build_agent_context(
                &app_handle_clone,
                &ExecuteAgentRequest {
                    agent_type: AgentType::Writer,
                    story_id: story_id.clone(),
                    chapter_number: None,
                    input: instruction.clone(),
                    parameters: None,
                },
            ).await.unwrap_or_else(|_| super::AgentContext::minimal(story_id.clone(), String::new()));

            // 注入当前已积累的上下文内容
            context.current_content = Some(accumulated_content.clone());

            let task = AgentTask {
                id: Uuid::new_v4().to_string(),
                agent_type: AgentType::Writer,
                context,
                input: instruction,
                parameters: std::collections::HashMap::new(),
                tier: Some(get_user_tier_sync(&app_handle_clone)),
            };

            let orchestrator = crate::agents::orchestrator::AgentOrchestrator::with_default_config(
                service.clone(),
                app_handle_clone.clone(),
            );
            match orchestrator.generate(task, crate::agents::orchestrator::GenerationMode::Full).await {
                Ok(workflow_result) => {
                    let generated = workflow_result.final_content;
                    let generated_len = generated.chars().count() as i32;
                    total_written += generated_len;
                    loop_count += 1;

                    // 将生成内容追加到积累上下文，供下一轮使用
                    accumulated_content.push_str(&generated);

                    // 推送内容追加事件到幕前
                    let event = crate::window::FrontstageEvent::AppendContent {
                        text: generated,
                        chapter_id: chapter_id.clone(),
                    };
                    let _ = crate::window::WindowManager::send_to_frontstage(&app_handle_clone, event);

                    // 推送进度事件
                    let percentage = ((total_written as f32 / target_chars as f32) * 100.0) as i32;
                    let progress = AutoWriteProgressEvent {
                        task_id: task_id_clone.clone(),
                        current_chars: total_written,
                        target_chars,
                        percentage,
                        current_loop: loop_count,
                        status: "writing".to_string(),
                    };
                    let _ = app_handle_clone.emit(&format!("auto-write-progress-{}", task_id_clone), progress);
                }
                Err(e) => {
                    log::error!("[auto_write] Loop {} failed: {}", loop_count, e);
                    let _ = app_handle_clone.emit(&format!("auto-write-error-{}", task_id_clone), e);
                    break;
                }
            }
        }

        // 推送完成事件
        let _ = app_handle_clone.emit(&format!("auto-write-complete-{}", task_id_clone), AutoWriteProgressEvent {
            task_id: task_id_clone.clone(),
            current_chars: total_written,
            target_chars,
            percentage: 100,
            current_loop: loop_count,
            status: "completed".to_string(),
        });

        // 保存最终内容到数据库
        let pool = app_handle_clone.state::<DbPool>();
        let scene_repo = SceneRepository::new(pool.inner().clone());
        let _ = scene_repo.update(
            &chapter_id,
            &SceneUpdate {
                title: None,
                content: Some(accumulated_content.clone()),
                ..Default::default()
            },
        );
        log::info!("[auto_write] Saved {} chars to scene {}", accumulated_content.chars().count(), chapter_id);

        // 后台触发知识图谱 Ingest
        let story_id_for_ingest = story_id.clone();
        let chapter_id_for_ingest = chapter_id.clone();
        let accumulated_for_ingest = accumulated_content.clone();
        let app_for_ingest = app_handle_clone.clone();
        let pool_for_ingest = app_handle_clone.state::<DbPool>().inner().clone();
        tokio::spawn(async move {
            let llm_service = crate::llm::LlmService::new(app_for_ingest.clone());
            let pipeline = crate::memory::ingest::IngestPipeline::new(llm_service)
                .with_pool(pool_for_ingest.clone())
                .with_app_handle(app_for_ingest);
            let ingest_content = crate::memory::ingest::IngestContent {
                text: accumulated_for_ingest,
                source: format!("auto_write:{}", chapter_id_for_ingest),
                story_id: story_id_for_ingest.clone(),
                scene_id: Some(chapter_id_for_ingest.clone()),
            };

            match pipeline.ingest(&ingest_content).await {
                Ok(ingest_result) => {
                    let kg_repo = crate::db::repositories_v3::KnowledgeGraphRepository::new(pool_for_ingest);
                    let mut saved_entities = 0usize;
                    let mut saved_relations = 0usize;

                    for entity in &ingest_result.entities {
                        if let Ok(_) = kg_repo.create_entity(
                            &story_id_for_ingest,
                            &entity.name,
                            &entity.entity_type.to_string(),
                            &entity.attributes,
                            entity.embedding.clone(),
                        ) {
                            saved_entities += 1;
                        }
                    }

                    let entity_name_to_id: std::collections::HashMap<String, String> = ingest_result.entities
                        .iter()
                        .map(|e| (e.name.clone(), e.id.clone()))
                        .collect();

                    for relation in &ingest_result.relations {
                        let source_id = entity_name_to_id.get(&relation.source_id).unwrap_or(&relation.source_id);
                        let target_id = entity_name_to_id.get(&relation.target_id).unwrap_or(&relation.target_id);
                        if let Ok(_) = kg_repo.create_relation(
                            &story_id_for_ingest,
                            source_id,
                            target_id,
                            &relation.relation_type.to_string(),
                            relation.strength,
                        ) {
                            saved_relations += 1;
                        }
                    }

                    log::info!("[auto_write] Ingest complete: {} entities, {} relations saved", saved_entities, saved_relations);
                }
                Err(e) => {
                    log::warn!("[auto_write] Ingest failed: {}", e);
                }
            }
        });

        // 清理句柄
        let _ = TASK_HANDLES.lock().unwrap().remove(&task_id_clone);
    });

    TASK_HANDLES.lock().unwrap().insert(task_id.clone(), handle.abort_handle());

    Ok(AutoWriteResponse {
        task_id,
        actual_chars: 0,
        loops: 0,
        status: "started".to_string(),
    })
}

/// 取消自动续写
#[command]
pub async fn auto_write_cancel(task_id: String) -> Result<(), AppError> {
    let mut handles = TASK_HANDLES.lock().unwrap();
    if let Some(handle) = handles.remove(&task_id) {
        handle.abort();
        log::info!("[auto_write] Task {} cancelled by user", task_id);
    }
    Ok(())
}

// ==================== 文思泉涌：自动修改 ====================

/// 自动修改请求
#[derive(Debug, Deserialize)]
pub struct AutoReviseRequest {
    pub story_id: String,
    pub chapter_id: Option<String>,
    pub scope: String,          // "full" | "chapter" | "selection"
    pub selected_text: Option<String>,
    pub revision_type: String,  // "style" | "plot" | "dialogue" | "description" | "comprehensive"
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
        _ => "综合以上所有方面进行全面修改，提升整体质量。",
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
    let text_len = match request.scope.as_str() {
        "selection" => request.selected_text.as_ref().map(|s| s.chars().count() as i32).unwrap_or(0),
        "chapter" | "scene" => {
            if let Some(ref sid) = request.chapter_id {
                let pool = app_handle.state::<DbPool>();
                let scene_repo = SceneRepository::new(pool.inner().clone());
                scene_repo.get_by_id(sid)
                    .map_err(AppError::from)?
                    .map(|s| s.content.unwrap_or_default().chars().count() as i32)
                    .unwrap_or(0)
            } else { 0 }
        }
        _ => {
            let pool = app_handle.state::<DbPool>();
            let scene_repo = SceneRepository::new(pool.inner().clone());
            let scenes = scene_repo.get_by_story(&request.story_id)
                .map_err(AppError::from)?;
            scenes.into_iter()
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

    // 在后台执行修改
    let handle = tokio::spawn(async move {
        // 阶段 1: 准备中
        let _ = app_handle_clone.emit(&format!("auto-revise-progress-{}", task_id_clone), AutoReviseProgressEvent {
            task_id: task_id_clone.clone(),
            stage: "preparing".to_string(),
            progress: 0.1,
            message: "读取目标文本...".to_string(),
            revised_text: None,
        });

        // 检查是否被取消
        if !TASK_HANDLES.lock().unwrap().contains_key(&task_id_clone) {
            return;
        }

        let pool = app_handle_clone.state::<DbPool>();
        let scene_repo = SceneRepository::new(pool.inner().clone());

        let target_text = match scope.as_str() {
            "chapter" | "scene" => {
                if let Some(ref sid) = chapter_id {
                    scene_repo.get_by_id(sid)
                        .map(|s| s.map(|scene| scene.content.unwrap_or_default()).unwrap_or_default())
                        .unwrap_or_default()
                } else { String::new() }
            }
            "selection" => selected_text.unwrap_or_default(),
            _ => {
                let scenes = scene_repo.get_by_story(&story_id).unwrap_or_default();
                scenes.into_iter()
                    .filter_map(|s| s.content)
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
        };

        if target_text.is_empty() {
            let _ = app_handle_clone.emit(&format!("auto-revise-error-{}", task_id_clone), "目标文本为空".to_string());
            return;
        }

        // 阶段 2: 修改中
        let _ = app_handle_clone.emit(&format!("auto-revise-progress-{}", task_id_clone), AutoReviseProgressEvent {
            task_id: task_id_clone.clone(),
            stage: "revising".to_string(),
            progress: 0.3,
            message: "AI 正在修改文本...".to_string(),
            revised_text: None,
        });

        let revision_instruction = get_revision_instruction(&revision_type);
        let instruction = format!(
            "你是一个专业的小说编辑。请根据以下要求对文本进行修改：\n\n【修改要求】{}\n\n【原文】\n{}\n\n请输出修改后的完整文本。保持原文结构和段落，只修改需要改进的地方。",
            revision_instruction,
            target_text
        );

        let context = match build_agent_context(
            &app_handle_clone,
            &ExecuteAgentRequest {
                agent_type: AgentType::Writer,
                story_id: story_id.clone(),
                chapter_number: None,
                input: instruction.clone(),
                parameters: None,
            },
        ).await {
            Ok(c) => c,
            Err(e) => {
                let _ = app_handle_clone.emit(&format!("auto-revise-error-{}", task_id_clone), e);
                return;
            }
        };

        let task = AgentTask {
            id: Uuid::new_v4().to_string(),
            agent_type: AgentType::Writer,
            context,
            input: instruction,
            parameters: std::collections::HashMap::new(),
            tier: Some(get_user_tier_sync(&app_handle_clone)),
        };

        let service = AgentService::new(app_handle_clone.clone());
        let orchestrator = crate::agents::orchestrator::AgentOrchestrator::with_default_config(
            service,
            app_handle_clone.clone(),
        );

        match orchestrator.generate(task, crate::agents::orchestrator::GenerationMode::Full).await {
            Ok(workflow_result) => {
                let result = crate::agents::AgentResult {
                    content: workflow_result.final_content,
                    score: Some(workflow_result.final_score),
                    suggestions: workflow_result.steps.iter().flat_map(|s| s.suggestions.clone()).collect(),
                    request_id: None,
                };
                // 阶段 3: 保存中
                let _ = app_handle_clone.emit(&format!("auto-revise-progress-{}", task_id_clone), AutoReviseProgressEvent {
                    task_id: task_id_clone.clone(),
                    stage: "saving".to_string(),
                    progress: 0.8,
                    message: "保存修改结果...".to_string(),
                    revised_text: None,
                });

                // 保存到数据库
                if let Some(ref sid) = chapter_id {
                    if scope == "chapter" || scope == "scene" {
                        let pool = app_handle_clone.state::<DbPool>();
                        let scene_repo = SceneRepository::new(pool.inner().clone());
                        let _ = scene_repo.update(
                            sid,
                            &SceneUpdate {
                                title: None,
                                content: Some(result.content.clone()),
                                ..Default::default()
                            },
                        );
                        log::info!("[auto_revise] Saved revised content to scene {}", sid);
                    }
                }

                // 阶段 4: 完成
                let _ = app_handle_clone.emit(&format!("auto-revise-complete-{}", task_id_clone), AutoReviseProgressEvent {
                    task_id: task_id_clone.clone(),
                    stage: "completed".to_string(),
                    progress: 1.0,
                    message: "修改完成".to_string(),
                    revised_text: Some(result.content.clone()),
                });
            }
            Err(e) => {
                let _ = app_handle_clone.emit(&format!("auto-revise-error-{}", task_id_clone), e);
            }
        }

        // 清理句柄
        let _ = TASK_HANDLES.lock().unwrap().remove(&task_id_clone);
    });

    TASK_HANDLES.lock().unwrap().insert(task_id.clone(), handle.abort_handle());

    Ok(AutoReviseResponse {
        task_id,
        revised_text: String::new(),
        status: "started".to_string(),
    })
}

/// 取消自动修改
#[command]
pub async fn auto_revise_cancel(task_id: String) -> Result<(), AppError> {
    let mut handles = TASK_HANDLES.lock().unwrap();
    if let Some(handle) = handles.remove(&task_id) {
        handle.abort();
        log::info!("[auto_revise] Task {} cancelled by user", task_id);
    }
    Ok(())
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
    use crate::db::DbPool;
    use crate::agents::context_optimizer::{ContextOptimizer, default_writing_tools};
    use tauri::Manager;

    let pool = app_handle.state::<DbPool>();
    let story_id = request.story_id.clone();
    let chapter_number = request.chapter_number.unwrap_or(1);

    let optimizer = ContextOptimizer::new(pool.inner().clone());

    // 根据 Agent 类型选择默认 L2 工具
    let l2_tools = match request.agent_type {
        super::service::AgentType::Writer => default_writing_tools(chapter_number),
        super::service::AgentType::Inspector => {
            crate::agents::context_optimizer::default_inspection_tools(&request.input, chapter_number)
        }
        _ => vec![],
    };

    let mut context = match optimizer.build_full_context(
        &story_id,
        chapter_number,
        None,
        None,
        l2_tools,
    ).await {
        Ok(ctx) => ctx,
        Err(e) => {
            log::warn!("[build_agent_context] ContextOptimizer failed: {}, falling back to minimal", e);
            let _ = app_handle.emit("context-degraded", serde_json::json!({
                "story_id": story_id,
                "reason": format!("ContextOptimizer failed: {}", e),
                "fallback": "minimal",
            }));
            return Ok(super::AgentContext::minimal(story_id, String::new()));
        }
    };

    // 注入未解决的伏笔提示到世界观规则中
    {
        let tracker = crate::creative_engine::foreshadowing::ForeshadowingTracker::new(pool.inner().clone());
        match tracker.get_writing_hints(&story_id, 5) {
            Ok(hints) if !hints.is_empty() => {
                let hints_text = format!("\n\n【伏笔提醒】\n{}", hints.join("\n"));
                context.world_rules = Some(context.world_rules.unwrap_or_default() + &hints_text);
                log::info!("[build_agent_context] Injected {} foreshadowing hints", hints.len());
            }
            Ok(_) => {}
            Err(e) => log::warn!("[build_agent_context] ForeshadowingTracker failed: {}", e),
        }
    }

    // v0.8.0: 语义检索增强 — 根据用户输入智能检索相关知识
    if request.input.len() >= 10 {
        if let Some(store) = crate::VECTOR_STORE.get() {
            match crate::knowledge_base::kb_search(
                store,
                &story_id,
                &request.input,
                5,
                None,
                "hybrid",
            ).await {
                Ok(results) if !results.is_empty() => {
                    let lines: Vec<String> = results
                        .iter()
                        .map(|r| format!("[第{}章 相似度{:.2}] {}", r.chapter_number, r.score, r.text))
                        .collect();
                    let semantic_text = format!("\n\n【相关记忆检索】\n{}", lines.join("\n"));
                    context.scene_structure = Some(
                        context.scene_structure.unwrap_or_default() + &semantic_text
                    );
                    log::info!("[build_agent_context] Injected {} semantic search results", results.len());
                }
                Ok(_) => {}
                Err(e) => {
                    log::warn!("[build_agent_context] Semantic search failed: {}, skipping", e);
                }
            }
        }
    }

    // 注入 story 的 style_dna_id
    {
        let story_repo = crate::db::repositories::StoryRepository::new(pool.inner().clone());
        if let Ok(Some(story)) = story_repo.get_by_id(&story_id) {
            context.style_dna_id = story.style_dna_id;
            if context.style_dna_id.is_some() {
                log::info!("[build_agent_context] Using style_dna_id: {:?}", context.style_dna_id);
            }
            // 注入方法论配置
            context.methodology_id = story.methodology_id.clone();
            context.methodology_step = story.methodology_step.map(|s| s.to_string());
            if context.methodology_id.is_some() {
                log::info!(
                    "[build_agent_context] Using methodology_id: {:?}, step: {:?}",
                    context.methodology_id,
                    context.methodology_step
                );
            }
        }
    }

    // 注入规范状态快照
    {
        let cs_manager = crate::canonical_state::CanonicalStateManager::new(pool.inner().clone());
        match cs_manager.get_snapshot(&story_id).await {
            Ok(snapshot) => {
                // 追加世界观事实和伏笔到 world_rules
                let mut world_parts = Vec::new();
                if let Some(ref existing) = context.world_rules {
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
                        world_parts.push(format!("- [重要度{}] {}", payoff.importance, payoff.content));
                    }
                }

                if !snapshot.story_context.overdue_payoffs.is_empty() {
                    world_parts.push("【逾期伏笔】".to_string());
                    for payoff in snapshot.story_context.overdue_payoffs.iter().take(5) {
                        world_parts.push(format!("- [重要度{}] {}", payoff.importance, payoff.content));
                    }
                }

                if world_parts.len() > 1 {
                    context.world_rules = Some(world_parts.join("\n"));
                }

                // 追加叙事阶段和时间线到 scene_structure
                let mut scene_parts = Vec::new();
                if let Some(ref existing) = context.scene_structure {
                    scene_parts.push(existing.clone());
                }

                scene_parts.push(format!("【叙事阶段】{}\n{}", snapshot.narrative_phase, snapshot.narrative_phase.writer_guidance()));

                if !snapshot.timeline.is_empty() {
                    let recent_events: Vec<String> = snapshot.timeline.iter().rev().take(5).rev().map(|e| {
                        format!("场景{}: {}", e.sequence_number, e.event_summary)
                    }).collect();
                    scene_parts.push(format!("【近期时间线】\n{}", recent_events.join("\n")));
                }

                if !snapshot.story_context.active_conflicts.is_empty() {
                    let conflicts: Vec<String> = snapshot.story_context.active_conflicts.iter().take(5).map(|c| {
                        format!("- [{}] {} (涉及: {})", c.conflict_type, c.stakes, c.parties.join(", "))
                    }).collect();
                    scene_parts.push(format!("【活跃冲突】\n{}", conflicts.join("\n")));
                }

                context.scene_structure = Some(scene_parts.join("\n"));

                log::info!(
                    "[build_agent_context] CanonicalState injected: phase={}, facts={}, pending={}, overdue={}",
                    snapshot.narrative_phase,
                    snapshot.world_facts.len(),
                    snapshot.story_context.pending_payoffs.len(),
                    snapshot.story_context.overdue_payoffs.len()
                );
            }
            Err(e) => {
                log::warn!("[build_agent_context] CanonicalStateManager failed: {}, skipping", e);
            }
        }
    }

    // current_content 和 selected_text 由调用方在返回后填充
    //（参见 writer_agent_execute、auto_write 等调用点）

    Ok(context)
}
