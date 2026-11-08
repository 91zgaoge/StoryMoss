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

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::String(Uuid::new_v4().to_string())),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": bare_name,
                "arguments": arguments
            })),
        };

        let response = {
            let transport = self.transport.as_mut().ok_or(McpError::NotConnected)?;
            transport.send(request).await?;
            transport.receive().await?
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
                            .unwrap_or(McpPermission::Read);
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

fn is_tool_missing_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("tool not found")
        || lower.contains("unknown tool")
        || lower.contains("不存在")
        || lower.contains("未知工具")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

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
