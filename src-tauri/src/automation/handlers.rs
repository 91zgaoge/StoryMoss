//! 自动化处理器定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Wry};

use crate::db::DbPool;
use super::triggers::TriggerEvent;

/// 处理器上下文
#[derive(Debug, Clone)]
pub struct HandlerContext {
    pub event: TriggerEvent,
    pub db_pool: DbPool,
    pub app_handle: AppHandle<Wry>,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// 处理器执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// 处理器动作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HandlerAction {
    /// 执行数据库操作
    DatabaseOperation {
        operation: String,
        table: String,
        data: serde_json::Value,
    },
    /// 发送通知
    SendNotification {
        title: String,
        message: String,
        level: String,
    },
    /// 执行工作流
    ExecuteWorkflow {
        workflow_id: String,
        parameters: HashMap<String, serde_json::Value>,
    },
    /// 调用外部API
    CallExternalApi {
        url: String,
        method: String,
        headers: HashMap<String, String>,
        body: Option<serde_json::Value>,
    },
}

/// 自动化处理器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationHandler {
    pub name: String,
    pub description: String,
    pub handler_type: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub enabled: bool,
}