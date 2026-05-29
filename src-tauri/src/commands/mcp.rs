//! Mcp commands

use tauri::State;

use crate::{
    db::DbPool,
    error::AppError,
    mcp::{McpClient, McpServerConfig},
    BUILTIN_MCP_SERVER, MCP_CONNECTIONS,
};

#[tauri::command(rename_all = "snake_case")]
pub async fn connect_mcp_server(
    config: McpServerConfig,
    _pool: State<'_, DbPool>,
) -> Result<Vec<crate::mcp::McpTool>, AppError> {
    let mut client = McpClient::new(config.clone());
    client.connect().await.map_err(AppError::from)?;
    let tools = client.get_tools().clone();

    // W4-B2: 动态注册到 CapabilityRegistry
    {
        let mut registry = crate::capabilities::get_capability_registry();
        for tool in &tools {
            let cap = crate::capabilities::Capability::from_mcp_tool(&config.id, tool);
            registry.register(cap);
        }
        log::info!(
            "[CapabilityRegistry] Registered {} MCP tools from server {}",
            tools.len(),
            config.id
        );
    }

    let mut connections = MCP_CONNECTIONS.lock().await;
    connections.insert(config.id.clone(), client);
    log::info!(
        "[MCP] Connected to server {} ({}), {} tools available",
        config.name,
        config.id,
        tools.len()
    );
    Ok(tools)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn call_mcp_tool(
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
    _pool: State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    let mut connections = MCP_CONNECTIONS.lock().await;
    let client = connections
        .get_mut(&server_id)
        .ok_or_else(|| AppError::internal(format!("MCP server {} not connected", server_id)))?;
    client
        .call_tool(&tool_name, arguments)
        .await
        .map_err(AppError::from)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn disconnect_mcp_server(
    server_id: String,
    _pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    {
        let mut connections = MCP_CONNECTIONS.lock().await;
        if let Some(mut client) = connections.remove(&server_id) {
            client.disconnect().await;
            log::info!("[MCP] Disconnected from server {}", server_id);
        }
    }

    // W4-B2: 从 CapabilityRegistry 注销该服务器的所有能力
    {
        let mut registry = crate::capabilities::get_capability_registry();
        let prefix = format!("mcp.{server_id}.");
        let removed = registry.unregister_by_prefix(&prefix);
        if removed > 0 {
            log::info!(
                "[CapabilityRegistry] Unregistered {} MCP capabilities from server {}",
                removed,
                server_id
            );
        }
    }

    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_mcp_connections(
    _pool: State<'_, DbPool>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let connections = MCP_CONNECTIONS.lock().await;
    let result: Vec<serde_json::Value> = connections
        .iter()
        .map(|(id, client)| {
            serde_json::json!({
                "id": id,
                "tools": client.get_tools().len(),
                "resources": client.get_resources().len(),
            })
        })
        .collect();
    Ok(result)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_mcp_tools(
    _pool: State<'_, DbPool>,
) -> Result<Vec<crate::mcp::McpTool>, AppError> {
    // 1. 收集内置工具（含动态注册的）
    let server = BUILTIN_MCP_SERVER.lock().await;
    let mut all_tools = server.get_tools();

    // 2. 收集外部连接工具（W2-B8: MCP 工具动态注册）
    // 外部工具名称前缀为 mcp.{server_id}.{tool_name}，便于前端区分来源
    let connections = MCP_CONNECTIONS.lock().await;
    for (server_id, client) in connections.iter() {
        for tool in client.get_tools() {
            let mut t = tool.clone();
            t.name = format!("mcp.{}.{}", server_id, tool.name);
            all_tools.push(t);
        }
    }

    Ok(all_tools)
}

#[tauri::command(rename_all = "snake_case")]
pub async fn execute_mcp_tool(
    tool_name: String,
    arguments: serde_json::Value,
    _pool: State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    // W2-B8: 统一入口 — 支持内置工具和外部工具
    // 如果 tool_name 以 mcp.{server_id}. 开头，路由到外部连接
    if let Some(rest) = tool_name.strip_prefix("mcp.") {
        let parts: Vec<&str> = rest.splitn(2, '.').collect();
        if parts.len() == 2 {
            let server_id = parts[0];
            let actual_tool_name = parts[1];
            let mut connections = MCP_CONNECTIONS.lock().await;
            let client = connections.get_mut(server_id).ok_or_else(|| {
                AppError::internal(format!("MCP server {} not connected", server_id))
            })?;
            let result = client
                .call_tool(actual_tool_name, arguments)
                .await
                .map_err(|e| AppError::internal(format!("MCP tool call failed: {}", e)))?;
            return Ok(result);
        }
    }

    // 否则作为内置工具执行（使用全局实例）
    let server = BUILTIN_MCP_SERVER.lock().await;
    server.start().await.map_err(AppError::from)?;
    let result = server
        .execute_tool(&tool_name, arguments)
        .await
        .map_err(AppError::from)?;

    Ok(result)
}

/// 动态注册内置 MCP 工具到 CapabilityRegistry（W2-B8）
///
/// 注意：此命令仅将工具注册到 CapabilityRegistry，使其可被 PlanGenerator
/// 发现和调度。 实际执行 handler 仍需在 mcp/server.rs 中实现并硬编码注册到
/// McpServer。
#[tauri::command(rename_all = "snake_case")]
pub async fn register_mcp_tool(
    tool: crate::mcp::McpTool,
    _pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    let mut registry = crate::capabilities::get_capability_registry();
    let cap = crate::capabilities::Capability::from_mcp_tool("builtin", &tool);
    registry.register(cap);

    log::info!(
        "[MCP] Dynamically registered built-in tool to CapabilityRegistry: {}",
        tool.name
    );
    Ok(())
}

/// 动态注销内置 MCP 工具（W2-B8）
#[tauri::command(rename_all = "snake_case")]
pub async fn unregister_mcp_tool(
    tool_name: String,
    _pool: State<'_, DbPool>,
) -> Result<(), AppError> {
    // 1. 从 CapabilityRegistry 注销
    let mut registry = crate::capabilities::get_capability_registry();
    let cap_id = format!("mcp.builtin.{}", tool_name);
    registry.unregister(&cap_id);

    log::info!(
        "[MCP] Dynamically unregistered built-in tool: {}",
        tool_name
    );
    Ok(())
}
