#![allow(dead_code)]
//! LLM Tauri Commands
//!
//! 提供给前端调用的LLM相关命令

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, State};

use super::service::LlmService;
use crate::{
    creative_engine::context_prioritizer::ContextHealthMetrics,
    diagnostics::{DiagnosticStore, LastLlmPrompt},
    error::AppError,
};

/// 生成请求
#[derive(Debug, Deserialize)]
pub struct GenerateRequestPayload {
    pub prompt: String,
    pub context: Option<String>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
}

/// 流式生成请求
#[derive(Debug, Deserialize)]
pub struct StreamGenerateRequest {
    pub request_id: String,
    pub prompt: String,
    pub context: Option<String>,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
}

/// 同步生成文本
#[command]
pub async fn llm_generate(
    request: GenerateRequestPayload,
    app_handle: AppHandle,
) -> Result<super::adapter::GenerateResponse, AppError> {
    let service = LlmService::new(app_handle);

    service
        .generate(request.prompt, request.max_tokens, request.temperature)
        .await
}

/// 开始流式生成
#[command]
pub async fn llm_generate_stream(
    request: StreamGenerateRequest,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let service = LlmService::new(app_handle);

    service
        .generate_stream(
            request.request_id,
            request.prompt,
            request.context,
            request.max_tokens,
            request.temperature,
        )
        .await
}

/// 测试LLM连接
#[command]
pub async fn llm_test_connection(app_handle: AppHandle) -> Result<TestConnectionResult, AppError> {
    let service = LlmService::new(app_handle);

    match service.test_connection().await {
        Ok((success, latency)) => Ok(TestConnectionResult {
            success,
            latency_ms: latency,
            message: if success {
                format!("连接成功，延迟 {}ms", latency)
            } else {
                "连接失败".to_string()
            },
        }),
        Err(e) => Ok(TestConnectionResult {
            success: false,
            latency_ms: 0,
            message: e.to_string(),
        }),
    }
}

/// 连接测试结果
#[derive(Debug, Serialize)]
pub struct TestConnectionResult {
    pub success: bool,
    pub latency_ms: u64,
    pub message: String,
}

/// 取消生成
#[command]
pub async fn llm_cancel_generation(
    request_id: String,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let service = LlmService::new(app_handle);
    service.cancel_generation(&request_id);
    Ok(())
}

/// v0.14.0: 取消所有进行中的 LLM 生成。
///
/// 在前端超时或用户主动取消时调用，确保不会留下孤儿 LLM 任务。
#[command(rename_all = "snake_case")]
pub async fn llm_cancel_all_generations(app_handle: AppHandle) -> Result<(), AppError> {
    let service = LlmService::new(app_handle);
    service.cancel_all_generations();
    Ok(())
}

/// v0.23.8: 获取最近一次实际发给 LLM 的提示词全文（诊断用）。
#[command(rename_all = "snake_case")]
pub async fn get_last_llm_prompt(
    store: State<'_, Arc<DiagnosticStore>>,
) -> Result<Option<LastLlmPrompt>, AppError> {
    Ok(store.get_last_llm_prompt())
}

/// v0.25.0: 获取最近一次 Writer 系统提示词的上下文健康度指标（诊断用）。
#[command(rename_all = "snake_case")]
pub async fn get_context_health(
    store: State<'_, Arc<DiagnosticStore>>,
) -> Result<Option<ContextHealthMetrics>, AppError> {
    Ok(store.get_context_health())
}

/// v0.23.12: 获取智能创作流程最近 N 条日志（诊断用）。
#[command(rename_all = "snake_case")]
pub async fn get_workflow_logs(
    logger: State<'_, Arc<crate::workflow_logger::WorkflowLogger>>,
    count: Option<usize>,
) -> Result<Vec<String>, AppError> {
    logger.tail(count.unwrap_or(50))
}

/// v0.23.57: 获取解析后的工作流日志（前端直接用字段渲染，无需二次
/// JSON.parse）。
#[command(rename_all = "snake_case")]
pub async fn get_workflow_logs_parsed(
    logger: State<'_, Arc<crate::workflow_logger::WorkflowLogger>>,
    count: Option<usize>,
) -> Result<Vec<crate::workflow_logger::WorkflowLogEvent>, AppError> {
    let lines = logger.tail(count.unwrap_or(100))?;
    let parsed: Vec<_> = lines
        .into_iter()
        .rev() // tail 返回 newest-first，反转为 oldest-first 方便阅读
        .filter_map(|line| {
            serde_json::from_str::<crate::workflow_logger::WorkflowLogEvent>(&line).ok()
        })
        .collect();
    Ok(parsed)
}

/// v0.23.12: 获取智能创作流程日志文件路径（诊断用）。
#[command(rename_all = "snake_case")]
pub async fn get_workflow_log_path(
    logger: State<'_, Arc<crate::workflow_logger::WorkflowLogger>>,
) -> Result<String, AppError> {
    Ok(logger.path().to_string_lossy().to_string())
}

/// v0.23.42: 前端写入 WorkflowLogger（诊断卡片自动收集）。
/// 让前端关键路径（setContent/selectChapter/ChapterSwitch）也能记录到
/// creative_workflow.log。
#[command(rename_all = "snake_case")]
pub async fn log_frontend_event(
    logger: State<'_, Arc<crate::workflow_logger::WorkflowLogger>>,
    phase: String,
    message: String,
    details: Option<serde_json::Value>,
) -> Result<(), AppError> {
    logger.info(&phase, &message, details);
    Ok(())
}

/// v0.23.20: 获取数据库连接池状态（前端状态栏 + 诊断卡片用）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DbPoolStatus {
    pub max_size: u32,
    pub connections: u32,
    pub idle: u32,
    pub in_use: u32,
    pub connection_timeout_secs: u64,
}

#[command(rename_all = "snake_case")]
pub async fn get_db_pool_status(pool: State<'_, DbPool>) -> Result<DbPoolStatus, AppError> {
    let p = pool.inner();
    let st = p.state();
    Ok(DbPoolStatus {
        max_size: p.max_size(),
        connections: st.connections,
        idle: st.idle_connections,
        in_use: st.connections - st.idle_connections,
        connection_timeout_secs: p.connection_timeout().as_secs(),
    })
}

/// 初始化LLM服务（在应用启动时调用）
///
/// 自 v0.23.0 起，LLM 服务在 app setup() 中通过 Tauri State 统一注入。
/// 该命令保留为前端兼容入口，实际触发共享实例的复用。
#[command]
pub fn init_llm(app_handle: AppHandle) -> Result<(), AppError> {
    let _service = LlmService::new(app_handle);
    log::info!("[LLM] Shared service ready via Tauri State");
    Ok(())
}

// ==================== LLM Call 统计命令 ====================

use crate::db::{DbPool, LlmCall, LlmCallRepository};

/// 获取故事的 LLM 调用记录
#[command(rename_all = "snake_case")]
pub async fn get_story_llm_calls(
    story_id: String,
    limit: i64,
    pool: State<'_, DbPool>,
) -> Result<Vec<LlmCall>, AppError> {
    let repo = LlmCallRepository::new(pool.inner().clone());
    repo.get_by_story(&story_id, limit).map_err(AppError::from)
}

/// 获取最近的 LLM 调用记录
#[command(rename_all = "snake_case")]
pub async fn get_recent_llm_calls(
    limit: i64,
    pool: State<'_, DbPool>,
) -> Result<Vec<LlmCall>, AppError> {
    let repo = LlmCallRepository::new(pool.inner().clone());
    repo.get_recent(limit).map_err(AppError::from)
}

/// LLM 调用统计
#[derive(serde::Serialize)]
pub struct LlmCallStats {
    pub count: i64,
    pub total_tokens: i64,
    pub total_cost: f64,
}

/// 获取故事的 LLM 调用统计
#[command(rename_all = "snake_case")]
pub async fn get_llm_call_stats(
    story_id: String,
    pool: State<'_, DbPool>,
) -> Result<LlmCallStats, AppError> {
    let repo = LlmCallRepository::new(pool.inner().clone());
    let (count, total_tokens, total_cost) =
        repo.get_stats_by_story(&story_id).map_err(AppError::from)?;
    Ok(LlmCallStats {
        count,
        total_tokens,
        total_cost,
    })
}
