use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub mime_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    /// stdio 模式的可执行文件；HTTP 模式（url 存在）时为空串。
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub timeout_seconds: u64,
    /// v0.88.0: 流式 HTTP 传输端点（如栗子读书 legado `http://127.0.0.1:1122/mcp`）。
    /// 存在时优先于 stdio。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// v0.88.0: 随请求发送的自定义 HTTP 头（如 `Authorization: Bearer
    /// <token>`）。
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,
}

impl McpServerConfig {
    /// HTTP（流式）模式判定：配置了 url 即走 HttpTransport。
    pub fn is_http(&self) -> bool {
        self.url
            .as_deref()
            .map(str::trim)
            .map(|u| !u.is_empty())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone)]
pub enum McpError {
    NotConnected,
    RpcError(String),
    TransportError(String),
    SerializationError(String),
    Timeout,
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpError::NotConnected => write!(f, "Not connected to MCP server"),
            McpError::RpcError(msg) => write!(f, "RPC error: {}", msg),
            McpError::TransportError(msg) => write!(f, "Transport error: {}", msg),
            McpError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            McpError::Timeout => write!(f, "Operation timed out"),
        }
    }
}

impl std::error::Error for McpError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}
