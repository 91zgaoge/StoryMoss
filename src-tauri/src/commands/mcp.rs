//! Mcp commands

use tauri::State;

use crate::{
    db::DbPool,
    error::AppError,
    mcp::{McpClient, McpServerConfig},
    BUILTIN_MCP_SERVER, MCP_CONNECTIONS,
};

/// 连接一台 MCP 服务器并注册能力（命令与启动自动连接共用）。
pub async fn connect_server(config: McpServerConfig) -> Result<Vec<crate::mcp::McpTool>, AppError> {
    let mut client = McpClient::new(config.clone());
    client.connect().await.map_err(AppError::from)?;
    let tools = client.get_tools().clone();
    let mapping = client.get_mapping_snapshot().cloned();

    if let Some(mapping) = &mapping {
        log::info!(
            "[MCP] 映射快照 {} v{} · source={} {} · tool_names={} · permissions={:?}",
            mapping.mapping_id,
            mapping.mapping_version,
            mapping.source_name,
            mapping.source_version,
            mapping.tool_names.len(),
            mapping.permissions
        );
    }

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
pub async fn connect_mcp_server(
    config: McpServerConfig,
    _pool: State<'_, DbPool>,
) -> Result<Vec<crate::mcp::McpTool>, AppError> {
    connect_server(config).await
}

/// 断开一台 MCP 服务器并注销能力（命令与删除配置共用）。
pub async fn disconnect_server(server_id: &str) {
    {
        let mut connections = MCP_CONNECTIONS.lock().await;
        if let Some(mut client) = connections.remove(server_id) {
            client.disconnect().await;
            log::info!("[MCP] Disconnected from server {server_id}");
        }
    }
    let mut registry = crate::capabilities::get_capability_registry();
    let prefix = format!("mcp.{server_id}.");
    let removed = registry.unregister_by_prefix(&prefix);
    if removed > 0 {
        log::info!(
            "[CapabilityRegistry] Unregistered {removed} MCP capabilities from server {server_id}"
        );
    }
}

/// UI / CapabilityRegistry 的 `mcp.{server_id}.{tool}` 前缀名还原为协议裸名。
pub(crate) fn normalize_external_tool_name(server_id: &str, tool_name: &str) -> String {
    let prefix = format!("mcp.{server_id}.");
    tool_name
        .strip_prefix(&prefix)
        .unwrap_or(tool_name)
        .to_string()
}

#[tauri::command(rename_all = "snake_case")]
pub async fn call_mcp_tool(
    server_id: String,
    tool_name: String,
    arguments: serde_json::Value,
    _pool: State<'_, DbPool>,
) -> Result<serde_json::Value, AppError> {
    let tool_name = normalize_external_tool_name(&server_id, &tool_name);
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
    disconnect_server(&server_id).await;
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
                "mapping": client
                    .get_mapping_snapshot()
                    .and_then(|mapping| serde_json::to_value(mapping).ok()),
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

// ═══════════════ v0.88.0: MCP 服务器配置持久化 + 启动自动连接 ═══════════════

#[tauri::command(rename_all = "snake_case")]
pub async fn mcp_list_saved_servers(
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::mcp::repository::SavedMcpServer>, AppError> {
    crate::mcp::repository::list_servers(pool.inner())
}

/// 保存（或更新）服务器配置；`connect=true` 时同时建立连接并注册能力。
#[tauri::command(rename_all = "snake_case")]
pub async fn mcp_save_server(
    pool: State<'_, DbPool>,
    config: McpServerConfig,
    enabled: bool,
    connect: Option<bool>,
) -> Result<Option<Vec<crate::mcp::McpTool>>, AppError> {
    crate::mcp::repository::save_server(pool.inner(), &config, enabled)?;
    if connect.unwrap_or(false) {
        let tools = connect_mcp_server(config, pool).await?;
        Ok(Some(tools))
    } else {
        Ok(None)
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn mcp_delete_server(pool: State<'_, DbPool>, server_id: String) -> Result<(), AppError> {
    // 先断开内存连接（幂等），再删配置
    disconnect_server(&server_id).await;
    crate::mcp::repository::delete_server(pool.inner(), &server_id)?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn mcp_set_server_enabled(
    pool: State<'_, DbPool>,
    server_id: String,
    enabled: bool,
) -> Result<(), AppError> {
    if !enabled {
        disconnect_server(&server_id).await;
    }
    crate::mcp::repository::set_enabled(pool.inner(), &server_id, enabled)
}

/// 启动时自动连接所有 enabled 的 MCP 服务器（lib.rs setup 末尾调用）。
/// 单台失败只记日志，不阻塞其余连接——MCP 服务器（如栗子读书手机端）
/// 可能暂时不在线，前端可手动重连。
pub async fn auto_connect_saved_servers(pool: crate::db::DbPool) {
    let configs = match crate::mcp::repository::list_enabled_configs(&pool) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[MCP] 启动自动连接读取配置失败: {e}");
            return;
        }
    };
    for config in configs {
        if !config.is_http() {
            // stdio 服务器需要拉起子进程，启动期不自动拉起（保留手动触发）
            continue;
        }
        match connect_server(config.clone()).await {
            Ok(tools) => {
                log::info!(
                    "[MCP] 自动连接 {}（{}）成功，{} 个工具",
                    config.name,
                    config.id,
                    tools.len()
                );
            }
            Err(e) => {
                log::warn!(
                    "[MCP] 自动连接 {}（{}）失败（服务器可能未开机）：{e}",
                    config.name,
                    config.id
                );
            }
        }
    }
}
