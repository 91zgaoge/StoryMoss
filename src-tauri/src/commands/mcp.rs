//! Mcp commands

use std::collections::HashMap;

use serde::Serialize;
use tauri::State;

use crate::{
    db::DbPool,
    error::AppError,
    mcp::{repository::SavedMcpServer, McpClient, McpServerConfig},
    BUILTIN_MCP_SERVER, MCP_CONNECTIONS,
};

/// 已保存 MCP 服务器的实时连接状态（前端自动连接页使用）。
#[derive(Debug, Clone, Serialize)]
pub struct McpConnectionStatus {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub connected: bool,
    pub tools: usize,
    pub resources: usize,
    pub mapping_version: Option<String>,
    pub source_name: Option<String>,
    pub source_version: Option<String>,
    pub permissions: Option<serde_json::Value>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// 连接一台 MCP 服务器并注册能力（命令与启动自动连接共用）。
pub async fn connect_server(config: McpServerConfig) -> Result<Vec<crate::mcp::McpTool>, AppError> {
    // 重连同一 id 前先释放旧 transport，避免子进程/连接泄漏；能力注册会在
    // 下方重新填充（同 id 覆盖）。
    disconnect_server(&config.id).await;

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

    // 先登记连接，再注册能力：能力注册后若立刻被 PlanGenerator/执行器
    // 查询，连接必须已经可用；顺序反过来会产生“能力已注册但连接不可用”
    // 的窗口。
    {
        let mut connections = MCP_CONNECTIONS.lock().await;
        connections.insert(config.id.clone(), client);
    }
    log::info!(
        "[MCP] Connected to server {} ({}), {} tools available",
        config.name,
        config.id,
        tools.len()
    );

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
        crate::mcp::repository::set_enabled(pool.inner(), &server_id, false)?;
        return Ok(());
    }
    crate::mcp::repository::set_enabled(pool.inner(), &server_id, true)?;
    // 启用即自动连接；连接失败保留 enabled=true（下次打开 MCP 页会重试）。
    let config = crate::mcp::repository::list_servers(pool.inner())?
        .into_iter()
        .find(|server| server.id == server_id)
        .map(|server| server.config);
    if let Some(config) = config {
        connect_server(config).await?;
    }
    Ok(())
}

fn connection_status(
    saved: &SavedMcpServer,
    client: Option<&McpClient>,
    error: Option<&AppError>,
) -> McpConnectionStatus {
    let mapping = client.and_then(|client| client.get_mapping_snapshot());
    McpConnectionStatus {
        id: saved.id.clone(),
        name: saved.name.clone(),
        enabled: saved.enabled,
        connected: client.is_some(),
        tools: client.map(|client| client.get_tools().len()).unwrap_or(0),
        resources: client
            .map(|client| client.get_resources().len())
            .unwrap_or(0),
        mapping_version: mapping.map(|mapping| mapping.mapping_version.clone()),
        source_name: mapping.map(|mapping| mapping.source_name.clone()),
        source_version: mapping.map(|mapping| mapping.source_version.clone()),
        permissions: mapping.and_then(|mapping| serde_json::to_value(&mapping.permissions).ok()),
        error_code: error.map(|error| error.code().to_string()),
        error_message: error.map(|error| error.message()),
    }
}

/// 自动连接所有 enabled 的已保存服务器（启动、MCP 页、训练页共用）。
///
/// 已连接的服务器直接复用；单台失败只记录状态，不阻塞其余服务器。stdio
/// 服务器也按保存的 command/args/env 尝试拉起（不再需要手动连接）。
pub async fn ensure_saved_server_connections(
    pool: &DbPool,
) -> Result<Vec<McpConnectionStatus>, AppError> {
    let servers = crate::mcp::repository::list_servers(pool)?;
    let mut errors: HashMap<String, AppError> = HashMap::new();

    for server in &servers {
        if !server.enabled {
            continue;
        }
        let already_connected = {
            let connections = MCP_CONNECTIONS.lock().await;
            connections.contains_key(&server.id)
        };
        if already_connected {
            continue;
        }
        if let Err(error) = connect_server(server.config.clone()).await {
            log::warn!(
                "[MCP] 自动连接 {}（{}）失败: {}",
                server.name,
                server.id,
                error
            );
            errors.insert(server.id.clone(), error);
        }
    }

    let connections = MCP_CONNECTIONS.lock().await;
    Ok(servers
        .iter()
        .map(|server| {
            connection_status(server, connections.get(&server.id), errors.get(&server.id))
        })
        .collect())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn mcp_ensure_connections(
    pool: State<'_, DbPool>,
) -> Result<Vec<McpConnectionStatus>, AppError> {
    ensure_saved_server_connections(pool.inner()).await
}

/// 启动时自动连接所有 enabled 的 MCP 服务器（lib.rs setup 末尾调用）。
/// 单台失败只记日志，不阻塞其余连接。
pub async fn auto_connect_saved_servers(pool: crate::db::DbPool) {
    match ensure_saved_server_connections(&pool).await {
        Ok(statuses) => {
            let connected = statuses.iter().filter(|status| status.connected).count();
            log::info!(
                "[MCP] 启动自动连接扫描完成: {connected}/{} 台服务器已连接",
                statuses.len()
            );
            for status in statuses.iter().filter(|status| !status.connected) {
                if status.enabled {
                    log::warn!(
                        "[MCP] {}（{}）未连接: {}",
                        status.name,
                        status.id,
                        status
                            .error_message
                            .as_deref()
                            .unwrap_or("服务器可能未开机")
                    );
                }
            }
        }
        Err(error) => log::warn!("[MCP] 启动自动连接读取配置失败: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn live_config(token: String) -> McpServerConfig {
        McpServerConfig {
            id: "legado-test".into(),
            name: "栗子读书（测试）".into(),
            command: String::new(),
            args: vec![],
            env: HashMap::new(),
            timeout_seconds: 30,
            url: Some("http://127.0.0.1:1122/mcp".into()),
            headers: HashMap::from([("Authorization".to_string(), format!("Bearer {token}"))]),
        }
    }

    #[tokio::test]
    #[ignore = "需要本机 Legado MCP 在线与 LEGADO_MCP_TOKEN"]
    async fn live_ensure_saved_server_auto_connects() {
        let token = std::env::var("LEGADO_MCP_TOKEN").expect("LEGADO_MCP_TOKEN 未设置");
        let pool = crate::db::create_test_pool().expect("测试 DB 初始化失败");
        let config = live_config(token);
        crate::mcp::repository::save_server(&pool, &config, true).expect("保存配置失败");

        let statuses = ensure_saved_server_connections(&pool)
            .await
            .expect("自动连接失败");
        let status = statuses
            .iter()
            .find(|status| status.id == config.id)
            .expect("缺少连接状态");
        assert!(status.connected, "enabled 服务器应自动连接");
        assert!(
            status.tools >= 10,
            "Legado 至少暴露 10 个基础工具，实际 {}",
            status.tools
        );
        assert!(status.error_code.is_none());
        disconnect_server(&config.id).await;
    }
}
