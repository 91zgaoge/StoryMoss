use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 映射层权限分类。权限来自源系统工具面/annotations，不等同于 StoryMoss
/// 的账号权限；用于把“源系统权限变化”转成可查询、可报错的工具可用性状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpPermission {
    /// 保守默认：工具没有 annotations 或没有任何可判别 hint 时，权限未知。
    /// 未知权限可以调用（工具确实存在），但绝不能在瞬时故障后重发。
    Unknown,
    Read,
    Write,
    Network,
    Private,
}

impl McpPermission {
    pub fn as_str(self) -> &'static str {
        match self {
            McpPermission::Unknown => "unknown",
            McpPermission::Read => "read",
            McpPermission::Write => "write",
            McpPermission::Network => "network",
            McpPermission::Private => "private",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            McpPermission::Unknown => "未知",
            McpPermission::Read => "读取",
            McpPermission::Write => "写入",
            McpPermission::Network => "联网",
            McpPermission::Private => "隐私",
        }
    }
}

impl std::fmt::Display for McpPermission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label_zh())
    }
}

/// MCP 标准工具声明。标准字段是 `inputSchema`，StoryMoss 内部沿用的字段名是
/// `parameters`；映射层固定用 alias 兼容，不再静默丢弃整批工具。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, alias = "inputSchema")]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub annotations: Option<McpToolAnnotations>,
}

/// MCP 工具行为注解（camelCase 为协议字段）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpToolAnnotations {
    #[serde(default, rename = "readOnlyHint")]
    pub read_only_hint: Option<bool>,
    #[serde(default, rename = "openWorldHint")]
    pub open_world_hint: Option<bool>,
    #[serde(default, rename = "destructiveHint")]
    pub destructive_hint: Option<bool>,
    #[serde(default, rename = "idempotentHint")]
    pub idempotent_hint: Option<bool>,
}

impl McpToolAnnotations {
    /// 写权限判定：显式只读为 false，或标记 destructive。
    pub fn is_write(&self) -> bool {
        self.read_only_hint == Some(false) || self.destructive_hint == Some(true)
    }

    /// 联网权限判定：工具声明 openWorldHint。
    pub fn is_network(&self) -> bool {
        self.open_world_hint == Some(true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, alias = "mimeType")]
    pub mime_type: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// initialize 响应里的服务器元信息（映射契约的版本输入）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpServerInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpServerMeta {
    #[serde(default)]
    pub protocol_version: String,
    #[serde(default)]
    pub server_info: McpServerInfo,
    #[serde(default)]
    pub instructions: Option<String>,
}

impl McpServerMeta {
    /// 从 initialize 的 `result` 读取协议版本与 serverInfo；缺字段时保留空串，
    /// 由映射契约决定是拒绝还是走通用契约。
    pub fn from_initialize_result(result: &serde_json::Value) -> Self {
        let server_info = result
            .get("serverInfo")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        Self {
            protocol_version: result
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim()
                .to_string(),
            server_info: McpServerInfo {
                name: server_info
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                title: server_info
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                version: server_info
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            },
            instructions: result
                .get("instructions")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        }
    }

    pub fn display_name(&self) -> String {
        self.server_info
            .title
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.server_info.name.clone())
    }
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
    /// initialize 报出的协议版本不在映射契约白名单
    ProtocolUnsupported {
        requested: String,
        supported: Vec<String>,
    },
    /// 源系统版本不在映射契约支持范围（如 Legado 0.15.x）
    MappingUnsupported {
        source: String,
        version: String,
        mapping_version: String,
        supported: String,
    },
    /// 映射结构/基础工具损坏，整台服务器的映射不可用
    MappingInvalid {
        reason: String,
    },
    /// 单个工具字段漂移；连接可继续，查询该工具时按此报错
    SchemaDrift {
        tool: String,
        missing: Vec<String>,
    },
    /// 源系统权限撤销后，查询对应工具
    PermissionDenied {
        permission: McpPermission,
        tool: String,
    },
    /// 调用时发现权限快照变化（如写/联网开关在会话中切换）
    PermissionChanged {
        permission: McpPermission,
        tool: String,
    },
    /// 工具从未提供或已移除
    ToolUnavailable {
        tool: String,
    },
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpError::NotConnected => write!(f, "未连接到 MCP 服务器"),
            McpError::RpcError(msg) => write!(f, "RPC 错误: {}", msg),
            McpError::TransportError(msg) => write!(f, "传输错误: {}", msg),
            McpError::SerializationError(msg) => write!(f, "序列化错误: {}", msg),
            McpError::Timeout => write!(f, "操作超时"),
            McpError::ProtocolUnsupported {
                requested,
                supported,
            } => write!(
                f,
                "MCP 协议版本 {} 不受支持（支持: {}）",
                requested,
                supported.join(" / ")
            ),
            McpError::MappingUnsupported {
                source,
                version,
                mapping_version,
                supported,
            } => write!(
                f,
                "MCP 源 {} 版本 {} 超出映射 {} 支持范围（{}）",
                source, version, mapping_version, supported
            ),
            McpError::MappingInvalid { reason } => write!(f, "MCP 映射无效: {}", reason),
            McpError::SchemaDrift { tool, missing } => {
                write!(f, "MCP 工具 {} 字段漂移: 缺少 {}", tool, missing.join(", "))
            }
            McpError::PermissionDenied { permission, tool } => write!(
                f,
                "MCP 权限拒绝: {}权限下的工具 {} 不可用",
                permission.label_zh(),
                tool
            ),
            McpError::PermissionChanged { permission, tool } => write!(
                f,
                "MCP 权限已变化: {}权限下的工具 {} 当前不可用",
                permission.label_zh(),
                tool
            ),
            McpError::ToolUnavailable { tool } => {
                write!(f, "MCP 工具 {} 不可用或未提供", tool)
            }
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
