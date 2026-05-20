//! LLM Tauri Commands
//!
//! 提供给前端调用的LLM相关命令
#![allow(dead_code)]

use super::service::{init_llm_service, LlmService};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle};

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

    service.generate(
        request.prompt,
        request.max_tokens,
        request.temperature,
    ).await
}

/// 开始流式生成
#[command]
pub async fn llm_generate_stream(
    request: StreamGenerateRequest,
    app_handle: AppHandle,
) -> Result<(), AppError> {
    let service = LlmService::new(app_handle);

    service.generate_stream(
        request.request_id,
        request.prompt,
        request.context,
        request.max_tokens,
        request.temperature,
    ).await
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

/// 初始化LLM服务（在应用启动时调用）
#[command]
pub fn init_llm(app_handle: AppHandle) -> Result<(), AppError> {
    init_llm_service(app_handle);
    log::info!("[LLM] Service initialized");
    Ok(())
}
