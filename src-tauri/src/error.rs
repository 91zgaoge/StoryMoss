//! AppError - 统一错误枚举
//!
//! W2-B8: 取代遍布代码库的 `Result<T, String>`，提供结构化错误信息。
//! 前端根据 `code` 渲染不同恢复 UI。

use serde::{Deserialize, Serialize};

/// 应用级错误枚举
///
/// 每个变体对应一种可恢复或不可恢复的错误场景，携带结构化上下文。
#[derive(Debug, Clone, Deserialize)]
pub enum AppError {
    /// 功能需要订阅解锁（v0.7.3：商业模式从模型配额制改为功能订阅制）
    SubscriptionRequired {
        message: String,
        feature_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        current_tier: Option<String>,
    },
    /// LLM 调用超时
    LlmTimeout {
        message: String,
        elapsed_ms: u64,
    },
    /// 数据库锁定（并发写入冲突）
    DbLocked {
        message: String,
    },
    /// 上下文不可用（StoryContextBuilder 空返回等）
    ContextUnavailable {
        message: String,
        context_type: String,
    },
    /// 输入验证失败
    ValidationFailed {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        field: Option<String>,
    },
    /// 网络离线（平台模型不可用时）
    NetworkOffline {
        message: String,
    },
    /// 操作被取消
    Cancellation {
        message: String,
    },
    /// 资源未找到
    NotFound {
        resource: String,
        id: String,
    },
    /// 预检失败（写作前阻塞性问题）
    PreflightFailed {
        message: String,
        issues: Vec<String>,
    },
    /// 内部错误（兜底）
    Internal {
        message: String,
    },
}

impl AppError {
    /// 错误代码（前端用于路由恢复 UI）
    pub fn code(&self) -> &'static str {
        match self {
            AppError::SubscriptionRequired { .. } => "SUBSCRIPTION_REQUIRED",
            AppError::LlmTimeout { .. } => "LLM_TIMEOUT",
            AppError::DbLocked { .. } => "DB_LOCKED",
            AppError::ContextUnavailable { .. } => "CONTEXT_UNAVAILABLE",
            AppError::ValidationFailed { .. } => "VALIDATION_FAILED",
            AppError::NetworkOffline { .. } => "NETWORK_OFFLINE",
            AppError::Cancellation { .. } => "CANCELLATION",
            AppError::NotFound { .. } => "NOT_FOUND",
            AppError::PreflightFailed { .. } => "PREFLIGHT_FAILED",
            AppError::Internal { .. } => "INTERNAL_ERROR",
        }
    }

    /// 人类可读的错误消息
    pub fn message(&self) -> String {
        match self {
            AppError::SubscriptionRequired { message, .. } => message.clone(),
            AppError::LlmTimeout { message, .. } => message.clone(),
            AppError::DbLocked { message } => message.clone(),
            AppError::ContextUnavailable { message, .. } => message.clone(),
            AppError::ValidationFailed { message, .. } => message.clone(),
            AppError::NetworkOffline { message } => message.clone(),
            AppError::Cancellation { message } => message.clone(),
            AppError::NotFound { resource, id } => format!("{} '{}' not found", resource, id),
            AppError::PreflightFailed { message, issues } => format!("{}: {}", message, issues.join("; ")),
            AppError::Internal { message } => message.clone(),
        }
    }

    /// 构造 IPC 响应对象
    pub fn to_response(&self) -> ErrorResponse {
        ErrorResponse {
            code: self.code().to_string(),
            message: self.message(),
            data: None,
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code(), self.message())
    }
}

impl std::error::Error for AppError {}

// ==================== 兼容转换（过渡期） ====================

impl From<String> for AppError {
    fn from(msg: String) -> Self {
        AppError::Internal { message: msg }
    }
}

impl From<&str> for AppError {
    fn from(msg: &str) -> Self {
        AppError::Internal { message: msg.to_string() }
    }
}

// ==================== 外部错误转换 ====================

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        let msg = err.to_string();
        if msg.contains("database is locked") {
            AppError::DbLocked { message: msg }
        } else {
            AppError::Internal { message: msg }
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Internal {
            message: format!("JSON error: {}", err),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Internal {
            message: format!("IO error: {}", err),
        }
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Internal {
            message: format!("HTTP error: {}", err),
        }
    }
}

impl From<r2d2::Error> for AppError {
    fn from(err: r2d2::Error) -> Self {
        AppError::Internal {
            message: format!("Connection pool error: {}", err),
        }
    }
}

impl From<Box<dyn std::error::Error>> for AppError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        AppError::Internal {
            message: format!("Error: {}", err),
        }
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        AppError::Internal {
            message: format!("Error: {}", err),
        }
    }
}

impl<T> From<std::sync::PoisonError<T>> for AppError {
    fn from(err: std::sync::PoisonError<T>) -> Self {
        AppError::Internal {
            message: format!("Lock poisoned: {}", err),
        }
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        AppError::Internal {
            message: format!("Tauri error: {}", err),
        }
    }
}

impl From<crate::book_deconstruction::models::ParseError> for AppError {
    fn from(err: crate::book_deconstruction::models::ParseError) -> Self {
        AppError::Internal {
            message: format!("Parse error: {}", err),
        }
    }
}

impl From<crate::mcp::types::McpError> for AppError {
    fn from(err: crate::mcp::types::McpError) -> Self {
        AppError::Internal {
            message: format!("MCP error: {}", err),
        }
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(err: serde_yaml::Error) -> Self {
        AppError::Internal {
            message: format!("YAML error: {}", err),
        }
    }
}

impl From<std::time::SystemTimeError> for AppError {
    fn from(err: std::time::SystemTimeError) -> Self {
        AppError::Internal {
            message: format!("System time error: {}", err),
        }
    }
}

// ==================== IPC 序列化格式 ====================

/// 前端消费的标准错误响应
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// 便捷构造函数
impl AppError {
    pub fn subscription_required(feature_id: impl Into<String>, message: impl Into<String>) -> Self {
        AppError::SubscriptionRequired {
            feature_id: feature_id.into(),
            message: message.into(),
            current_tier: None,
        }
    }

    pub fn llm_timeout(elapsed_ms: u64) -> Self {
        AppError::LlmTimeout {
            message: format!("LLM call timed out after {}ms", elapsed_ms),
            elapsed_ms,
        }
    }

    pub fn db_locked(message: impl Into<String>) -> Self {
        AppError::DbLocked {
            message: message.into(),
        }
    }

    pub fn context_unavailable(context_type: impl Into<String>, message: impl Into<String>) -> Self {
        AppError::ContextUnavailable {
            context_type: context_type.into(),
            message: message.into(),
        }
    }

    pub fn validation_failed(message: impl Into<String>, field: Option<impl Into<String>>) -> Self {
        AppError::ValidationFailed {
            message: message.into(),
            field: field.map(|f| f.into()),
        }
    }

    pub fn network_offline(message: impl Into<String>) -> Self {
        AppError::NetworkOffline {
            message: message.into(),
        }
    }

    pub fn cancelled(message: impl Into<String>) -> Self {
        AppError::Cancellation {
            message: message.into(),
        }
    }

    pub fn not_found(resource: impl Into<String>, id: impl Into<String>) -> Self {
        AppError::NotFound {
            resource: resource.into(),
            id: id.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        AppError::Internal {
            message: message.into(),
        }
    }

    pub fn preflight_failed(message: impl Into<String>, issues: Vec<String>) -> Self {
        AppError::PreflightFailed {
            message: message.into(),
            issues,
        }
    }
}

// ==================== 手动 Serialize（统一 IPC 格式 { code, message, data }） ====================

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("code", self.code())?;
        map.serialize_entry("message", &self.message())?;

        let data: Option<serde_json::Value> = match self {
            AppError::SubscriptionRequired { feature_id, current_tier, .. } => {
                Some(serde_json::json!({ "feature_id": feature_id, "current_tier": current_tier }))
            }
            AppError::LlmTimeout { elapsed_ms, .. } => {
                Some(serde_json::json!({ "elapsed_ms": elapsed_ms }))
            }
            AppError::ContextUnavailable { context_type, .. } => {
                Some(serde_json::json!({ "context_type": context_type }))
            }
            AppError::ValidationFailed { field, .. } => {
                Some(serde_json::json!({ "field": field }))
            }
            AppError::NotFound { resource, id } => {
                Some(serde_json::json!({ "resource": resource, "id": id }))
            }
            AppError::PreflightFailed { issues, .. } => {
                Some(serde_json::json!({ "issues": issues }))
            }
            _ => None,
        };

        if let Some(d) = data {
            map.serialize_entry("data", &d)?;
        }

        map.end()
    }
}
