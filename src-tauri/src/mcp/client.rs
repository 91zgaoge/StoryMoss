#![allow(dead_code)]
use uuid::Uuid;

use super::{
    mapping::{
        build_snapshot, check_tool_available, parse_resources_payload, parse_tools_payload,
        resolve_contract, MappingContract, MappingSnapshot,
    },
    transport::{HttpTransport, StdioTransport, Transport},
    types::*,
};

pub struct McpClient {
    config: McpServerConfig,
    transport: Option<Box<dyn Transport + Send>>,
    tools: Vec<McpTool>,
    resources: Vec<McpResource>,
    server_meta: Option<McpServerMeta>,
    mapping_contract: Option<MappingContract>,
    mapping: Option<MappingSnapshot>,
    previous_mapping: Option<MappingSnapshot>,
}

impl McpClient {
    pub fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            transport: None,
            tools: Vec::new(),
            resources: Vec::new(),
            server_meta: None,
            mapping_contract: None,
            mapping: None,
            previous_mapping: None,
        }
    }

    pub async fn connect(&mut self) -> Result<(), McpError> {
        // v0.88.0: 配置 url 时走流式 HTTP（栗子读书等远程 MCP），否则 stdio
        // 子进程
        let mut transport: Box<dyn Transport + Send> = if self.config.is_http() {
            Box::new(HttpTransport::new(
                self.config.url.as_deref().unwrap_or_default(),
                &self.config.headers,
                self.config.timeout_seconds,
            ))
        } else {
            Box::new(StdioTransport::new(
                &self.config.command,
                &self.config.args,
                &self.config.env,
            ))
        };

        transport.connect().await?;

        let init_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "cinema-ai",
                    "version": "2.0.0"
                }
            })),
        };

        transport.send(init_request).await?;
        let init_response = transport.receive().await?;
        if let Some(error) = init_response.error {
            return Err(McpError::RpcError(error.message));
        }
        let init_result = init_response
            .result
            .ok_or_else(|| McpError::MappingInvalid {
                reason: "initialize 响应缺少 result".to_string(),
            })?;

        let server_meta = McpServerMeta::from_initialize_result(&init_result);
        let contract = resolve_contract(&server_meta)?;
        log::info!(
            "[MCP] 映射契约已解析: {} v{} · source={} {} · protocol={}",
            contract.mapping_id,
            contract.mapping_version,
            server_meta.display_name(),
            server_meta.server_info.version,
            server_meta.protocol_version
        );
        self.server_meta = Some(server_meta);
        self.mapping_contract = Some(contract);

        // 流式 HTTP：initialize 后必须补发 initialized 通知（无 id，
        // 服务端回 202）；stdio 服务器也不要求读回执，统一发送即可。
        if self.config.is_http() {
            let initialized = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: None,
                method: "notifications/initialized".to_string(),
                params: None,
            };
            transport.send(initialized).await?;
        }

        self.transport = Some(transport);

        self.refresh_tools().await?;
        self.refresh_resources().await?;

        Ok(())
    }

    async fn refresh_tools(&mut self) -> Result<(), McpError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = {
            let transport = self.transport.as_mut().ok_or(McpError::NotConnected)?;
            transport.send(request).await?;
            transport.receive().await?
        };

        if let Some(error) = response.error {
            return Err(McpError::RpcError(error.message));
        }
        let result = response.result.ok_or_else(|| McpError::MappingInvalid {
            reason: "tools/list 响应缺少 result".to_string(),
        })?;

        let parsed = parse_tools_payload(&result)?;
        let contract = self
            .mapping_contract
            .as_ref()
            .ok_or_else(|| McpError::MappingInvalid {
                reason: "映射契约尚未解析".to_string(),
            })?;
        let meta = self.server_meta.as_ref().cloned().unwrap_or_default();
        let snapshot = build_snapshot(contract, &meta, &parsed.tools, parsed.drifted.clone())?;

        if let Some(previous) = self.mapping.take() {
            let removed = snapshot.removed_tools(&previous);
            if !removed.is_empty() {
                log::info!(
                    "[MCP] 源系统工具面变化，以下工具暂不可用: {}",
                    removed.join(", ")
                );
            }
            for (permission, state) in &snapshot.permissions {
                if !state.granted
                    && previous
                        .permissions
                        .get(permission)
                        .map(|previous_state| previous_state.granted)
                        .unwrap_or(false)
                {
                    log::warn!(
                        "[MCP] 源系统权限撤销: {} · tools={:?}",
                        permission.as_str(),
                        state.tools
                    );
                }
            }
            self.previous_mapping = Some(previous);
        }

        self.mapping = Some(snapshot);
        self.tools = parsed.tools;
        Ok(())
    }

    async fn refresh_resources(&mut self) -> Result<(), McpError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "resources/list".to_string(),
            params: None,
        };

        let response = {
            let transport = self.transport.as_mut().ok_or(McpError::NotConnected)?;
            transport.send(request).await?;
            transport.receive().await?
        };

        if let Some(error) = response.error {
            return Err(McpError::RpcError(error.message));
        }
        let result = response.result.ok_or_else(|| McpError::MappingInvalid {
            reason: "resources/list 响应缺少 result".to_string(),
        })?;
        self.resources = parse_resources_payload(&result)?;
        Ok(())
    }

    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        let bare_name = self.normalize_tool_name(tool_name);
        if let (Some(snapshot), Some(contract)) = (&self.mapping, &self.mapping_contract) {
            check_tool_available(snapshot, contract, &bare_name)?;
        }

        // v0.93.15：只读工具遇到瞬时传输故障（栗子读书回环 HTTP 偶发
        // Connection reset）时重发；写/隐私工具绝不重试，避免重复副作用。
        // 续写训练一次取 n+1 章，单次网络抖动曾把整条训练判死。
        let retry_safe = permission_is_retry_safe(
            self.mapping
                .as_ref()
                .and_then(|snapshot| snapshot.permission_for(&bare_name)),
        );
        let mut attempt = 0usize;
        let response = loop {
            let request = JsonRpcRequest {
                jsonrpc: "2.0".to_string(),
                id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
                method: "tools/call".to_string(),
                params: Some(serde_json::json!({
                    "name": bare_name,
                    "arguments": arguments.clone()
                })),
            };

            let outcome = {
                let transport = self.transport.as_mut().ok_or(McpError::NotConnected)?;
                match transport.send(request).await {
                    Ok(()) => transport.receive().await,
                    Err(error) => Err(error),
                }
            };

            match outcome {
                Ok(response) => break response,
                Err(error)
                    if retry_safe
                        && attempt < TOOL_CALL_RETRY_LIMIT
                        && is_transient_transport_error(&error) =>
                {
                    log::warn!(
                        "[MCP] 只读工具调用瞬时网络故障，重试 {}/{}: tool={} error={}",
                        attempt + 1,
                        TOOL_CALL_RETRY_LIMIT,
                        bare_name,
                        error
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(
                        TOOL_CALL_RETRY_BACKOFF_MS[attempt],
                    ))
                    .await;
                    attempt += 1;
                }
                Err(error) => return Err(error),
            }
        };

        if let Some(error) = response.error {
            let error_code = error.code;
            let error_message = error.message;
            // 源系统权限可能在会话中切换（如 Legado 联网/写开关）。工具突然
            // 不存在时刷新一次工具面，再给出可诊断的权限错误。
            if error_code == -32601 || is_tool_missing_message(&error_message) {
                let previous = self.mapping.clone();
                if let Err(refresh_error) = self.refresh_tools().await {
                    log::warn!("[MCP] 工具调用失败后刷新工具面也失败: {}", refresh_error);
                }
                if let (Some(previous), Some(current)) = (previous, self.mapping.as_ref()) {
                    if previous.tool_names.contains(&bare_name)
                        && !current.tool_names.contains(&bare_name)
                    {
                        let permission = previous
                            .permission_for(&bare_name)
                            .unwrap_or(McpPermission::Unknown);
                        return Err(McpError::PermissionChanged {
                            permission,
                            tool: bare_name,
                        });
                    }
                    if !current.tool_names.contains(&bare_name) {
                        if let Some(contract) = self.mapping_contract.as_ref() {
                            if let Some(permission) = contract.has_optional_tool(&bare_name) {
                                if current
                                    .permissions
                                    .get(&permission)
                                    .map(|state| !state.granted)
                                    .unwrap_or(false)
                                {
                                    return Err(McpError::PermissionDenied {
                                        permission,
                                        tool: bare_name,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            return Err(McpError::RpcError(error_message));
        }

        Ok(response.result.unwrap_or(serde_json::Value::Null))
    }

    pub async fn read_resource(&mut self, uri: &str) -> Result<serde_json::Value, McpError> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "resources/read".to_string(),
            params: Some(serde_json::json!({
                "uri": uri
            })),
        };

        if let Some(ref mut transport) = self.transport {
            transport.send(request).await?;
            let response = transport.receive().await?;

            if let Some(error) = response.error {
                return Err(McpError::RpcError(error.message));
            }

            return Ok(response.result.unwrap_or(serde_json::Value::Null));
        }

        Err(McpError::NotConnected)
    }

    pub fn get_tools(&self) -> &Vec<McpTool> {
        &self.tools
    }

    pub fn get_resources(&self) -> &Vec<McpResource> {
        &self.resources
    }

    pub fn get_mapping_snapshot(&self) -> Option<&MappingSnapshot> {
        self.mapping.as_ref()
    }

    pub fn get_server_meta(&self) -> Option<&McpServerMeta> {
        self.server_meta.as_ref()
    }

    /// UI / capability registry 传入的 `mcp.{server_id}.{tool}` 前缀名在进
    /// server 之前还原为协议裸工具名。
    fn normalize_tool_name(&self, tool_name: &str) -> String {
        let prefix = format!("mcp.{}.", self.config.id);
        tool_name
            .strip_prefix(&prefix)
            .unwrap_or(tool_name)
            .to_string()
    }

    pub async fn disconnect(&mut self) {
        if let Some(mut transport) = self.transport.take() {
            let _ = transport.close().await;
        }
    }
}

/// 只读工具瞬时故障重试上限（初次 + 2 次重试）。
const TOOL_CALL_RETRY_LIMIT: usize = 2;
/// 重试退避：先短后长，避免连续打满本地源。
const TOOL_CALL_RETRY_BACKOFF_MS: [u64; 2] = [150, 400];

/// 瞬时传输故障判定：连接重置/拒绝/断开属于网络抖动，可重试；
/// 协议、序列化与 RPC 错误不可重试。
fn is_transient_transport_error(error: &McpError) -> bool {
    match error {
        McpError::TransportError(message) => {
            let lower = message.to_ascii_lowercase();
            [
                "connection reset",
                "connection refused",
                "broken pipe",
                "os error 54",
                "os error 61",
                "os error 32",
                "读取响应失败",
                "写入请求失败",
            ]
            .iter()
            .any(|needle| lower.contains(needle))
        }
        McpError::Timeout => true,
        _ => false,
    }
}

/// 只有显式声明为只读/联网的工具在瞬时故障后可安全重发；写、隐私与
/// 未知权限工具绝不重试，避免连接重置后把同一副作用执行两次。
fn permission_is_retry_safe(permission: Option<McpPermission>) -> bool {
    matches!(
        permission,
        Some(McpPermission::Read) | Some(McpPermission::Network)
    )
}

fn is_tool_missing_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("tool not found")
        || lower.contains("unknown tool")
        || lower.contains("不存在")
        || lower.contains("未知工具")
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, VecDeque},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    use super::*;

    fn fake_config() -> McpServerConfig {
        McpServerConfig {
            id: "fake".into(),
            name: "fake".into(),
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            timeout_seconds: 5,
            url: Some("http://127.0.0.1:9/mcp".into()),
            headers: HashMap::new(),
        }
    }

    fn fake_meta() -> McpServerMeta {
        McpServerMeta {
            protocol_version: "2025-06-18".into(),
            server_info: McpServerInfo {
                name: "fake".into(),
                title: None,
                version: "1.0".into(),
            },
            instructions: None,
        }
    }

    fn fake_contract() -> MappingContract {
        resolve_contract(&fake_meta()).expect("通用契约应可解析")
    }

    fn fake_snapshot(tools: Vec<McpTool>) -> MappingSnapshot {
        let meta = fake_meta();
        let contract = resolve_contract(&meta).unwrap();
        build_snapshot(&contract, &meta, &tools, HashMap::new()).expect("快照应可构建")
    }

    fn fake_tool(name: &str, destructive: bool) -> McpTool {
        McpTool {
            name: name.into(),
            description: String::new(),
            parameters: serde_json::json!({ "type": "object" }),
            annotations: Some(McpToolAnnotations {
                read_only_hint: Some(!destructive),
                open_world_hint: Some(false),
                destructive_hint: Some(destructive),
                idempotent_hint: None,
            }),
        }
    }

    /// 首次 send 模拟回环 HTTP 被对端重置，之后正常返回。
    struct ResetOnceTransport {
        attempts: Arc<AtomicUsize>,
        pending: VecDeque<JsonRpcResponse>,
        payload: serde_json::Value,
    }

    impl ResetOnceTransport {
        fn new(attempts: Arc<AtomicUsize>, payload: serde_json::Value) -> Self {
            Self {
                attempts,
                pending: VecDeque::new(),
                payload,
            }
        }
    }

    #[async_trait::async_trait]
    impl Transport for ResetOnceTransport {
        async fn connect(&mut self) -> Result<(), McpError> {
            Ok(())
        }

        async fn send(&mut self, request: JsonRpcRequest) -> Result<(), McpError> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt == 1 {
                return Err(McpError::TransportError(
                    "读取响应失败: Connection reset by peer (os error 54)".into(),
                ));
            }
            self.pending.push_back(JsonRpcResponse {
                jsonrpc: "2.0".into(),
                id: request.id,
                result: Some(self.payload.clone()),
                error: None,
            });
            Ok(())
        }

        async fn receive(&mut self) -> Result<JsonRpcResponse, McpError> {
            self.pending
                .pop_front()
                .ok_or_else(|| McpError::TransportError("no pending response".into()))
        }

        async fn close(&mut self) -> Result<(), McpError> {
            Ok(())
        }
    }

    fn client_with_transport(tool: McpTool, transport: ResetOnceTransport) -> McpClient {
        let mut client = McpClient::new(fake_config());
        client.mapping_contract = Some(fake_contract());
        client.mapping = Some(fake_snapshot(vec![tool]));
        client.transport = Some(Box::new(transport));
        client
    }

    #[test]
    fn transient_transport_errors_are_classified() {
        assert!(is_transient_transport_error(&McpError::TransportError(
            "读取响应失败: Connection reset by peer (os error 54)".into()
        )));
        assert!(is_transient_transport_error(&McpError::TransportError(
            "写入请求失败: Broken pipe (os error 32)".into()
        )));
        assert!(is_transient_transport_error(&McpError::Timeout));
        assert!(!is_transient_transport_error(
            &McpError::SerializationError("响应不是合法 JSON-RPC".into())
        ));
        assert!(!is_transient_transport_error(&McpError::RpcError(
            "tool not found".into()
        )));
    }

    #[test]
    fn only_read_and_network_permissions_are_retry_safe() {
        assert!(permission_is_retry_safe(Some(McpPermission::Read)));
        assert!(permission_is_retry_safe(Some(McpPermission::Network)));
        assert!(!permission_is_retry_safe(Some(McpPermission::Unknown)));
        assert!(!permission_is_retry_safe(Some(McpPermission::Write)));
        assert!(!permission_is_retry_safe(Some(McpPermission::Private)));
        assert!(!permission_is_retry_safe(None));
    }

    /// 无 annotations 的工具被保守判为 Unknown，瞬时故障时不得重发。
    #[tokio::test]
    async fn unknown_permission_tool_is_not_retried_after_connection_reset() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let tool = McpTool {
            name: "get_chapter_content".into(),
            description: String::new(),
            parameters: serde_json::json!({ "type": "object" }),
            annotations: None,
        };
        let mut client = client_with_transport(
            tool,
            ResetOnceTransport::new(attempts.clone(), serde_json::json!({})),
        );

        let error = client
            .call_tool("get_chapter_content", serde_json::json!({}))
            .await
            .expect_err("未知权限工具不应重试");
        assert!(matches!(error, McpError::TransportError(_)));
        assert_eq!(attempts.load(Ordering::SeqCst), 1, "不得重发");
    }

    /// v0.93.15 回归：只读工具首次 Connection reset，重试后成功返回。
    #[tokio::test]
    async fn read_only_tool_retries_after_connection_reset() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let mut client = client_with_transport(
            fake_tool("get_chapter_content", false),
            ResetOnceTransport::new(
                attempts.clone(),
                serde_json::json!({ "content": [{ "text": "第1章正文" }] }),
            ),
        );

        let result = client
            .call_tool("get_chapter_content", serde_json::json!({ "index": 1 }))
            .await
            .expect("只读工具应在瞬时重置后重试成功");
        assert_eq!(result["content"][0]["text"], "第1章正文");
        assert_eq!(attempts.load(Ordering::SeqCst), 2, "应恰好重试一次");
    }

    /// 写工具遇到连接重置必须立即失败，不重复执行副作用。
    #[tokio::test]
    async fn write_tool_is_not_retried_after_connection_reset() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let mut client = client_with_transport(
            fake_tool("fake_delete", true),
            ResetOnceTransport::new(attempts.clone(), serde_json::json!({})),
        );

        let error = client
            .call_tool("fake_delete", serde_json::json!({}))
            .await
            .expect_err("写工具不应重试");
        assert!(matches!(error, McpError::TransportError(_)));
        assert_eq!(attempts.load(Ordering::SeqCst), 1, "不得重发");
    }

    fn live_config(token: String) -> McpServerConfig {
        McpServerConfig {
            id: "legado".into(),
            name: "栗子读书".into(),
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            timeout_seconds: 30,
            url: Some(
                std::env::var("LEGADO_MCP_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:1122/mcp".into()),
            ),
            headers: HashMap::from([("Authorization".to_string(), format!("Bearer {token}"))]),
        }
    }

    #[test]
    fn normalizes_prefixed_tool_names() {
        let mut client = McpClient::new(live_config("x".into()));
        assert_eq!(
            client.normalize_tool_name("mcp.legado.get_chapter_content"),
            "get_chapter_content"
        );
        assert_eq!(
            client.normalize_tool_name("get_chapter_content"),
            "get_chapter_content"
        );
        assert_eq!(
            client.normalize_tool_name("mcp.other.get_chapter_content"),
            "mcp.other.get_chapter_content"
        );
    }

    /// 真机探针，显式运行：
    /// LEGADO_MCP_TOKEN=<token> cargo test --lib live_legado_mcp_tools_list --
    /// --ignored
    #[tokio::test]
    #[ignore = "需要本机 Legado MCP 在线与 LEGADO_MCP_TOKEN"]
    async fn live_legado_mcp_tools_list() {
        let token = std::env::var("LEGADO_MCP_TOKEN").expect("LEGADO_MCP_TOKEN 未设置");
        let mut client = McpClient::new(live_config(token));
        client.connect().await.expect("连接 Legado MCP 失败");

        let snapshot = client.get_mapping_snapshot().expect("缺少映射快照");
        assert_eq!(snapshot.mapping_id, "legado-macos");
        assert!(client.get_tools().len() >= 10);

        let chapter_tool = client
            .get_tools()
            .iter()
            .find(|tool| tool.name == "get_chapter_content")
            .expect("缺少 get_chapter_content");
        assert!(
            chapter_tool.parameters.get("properties").is_some(),
            "章节正文工具应带 JSON Schema 参数"
        );

        let result = client
            .call_tool("list_groups", serde_json::json!({}))
            .await
            .expect("调用 list_groups 失败");
        assert!(result.is_object(), "list_groups 应返回 JSON 对象");
    }
}
