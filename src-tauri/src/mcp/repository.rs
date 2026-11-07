//! MCP 服务器配置持久化（V156）
//!
//! stdio 与流式 HTTP 配置统一存
//! `mcp_server_configs.config_json`（McpServerConfig serde 序列化）。启动时
//! `lib.rs` 读取 enabled 的配置自动连接（写入 `MCP_CONNECTIONS`），使定时任务/
//! 训练执行器可在后端直连 MCP 工具。

use rusqlite::params;

use crate::db::DbPool;

#[derive(Debug, Clone, serde::Serialize)]
pub struct SavedMcpServer {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub config: crate::mcp::McpServerConfig,
    pub created_at: String,
    pub updated_at: String,
}

pub fn list_servers(pool: &DbPool) -> Result<Vec<SavedMcpServer>, crate::error::AppError> {
    let conn = pool
        .get()
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, config_json, enabled, created_at, updated_at \
             FROM mcp_server_configs ORDER BY created_at ASC",
        )
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;

    let mut out = Vec::new();
    for row in rows {
        let (id, name, config_json, enabled, created_at, updated_at) =
            row.map_err(|e| crate::error::AppError::internal(e.to_string()))?;
        let config: crate::mcp::McpServerConfig = serde_json::from_str(&config_json)
            .map_err(|e| crate::error::AppError::internal(format!("配置解析失败 {id}: {e}")))?;
        out.push(SavedMcpServer {
            id,
            name,
            enabled: enabled != 0,
            config,
            created_at,
            updated_at,
        });
    }
    Ok(out)
}

pub fn save_server(
    pool: &DbPool,
    config: &crate::mcp::McpServerConfig,
    enabled: bool,
) -> Result<(), crate::error::AppError> {
    let conn = pool
        .get()
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    let json = serde_json::to_string(config)
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    conn.execute(
        "INSERT INTO mcp_server_configs (id, name, config_json, enabled, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, datetime('now','localtime'), datetime('now','localtime')) \
         ON CONFLICT(id) DO UPDATE SET name = excluded.name, config_json = excluded.config_json, \
         enabled = excluded.enabled, updated_at = datetime('now','localtime')",
        params![config.id, config.name, json, enabled as i64],
    )
    .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    Ok(())
}

pub fn delete_server(pool: &DbPool, id: &str) -> Result<usize, crate::error::AppError> {
    let conn = pool
        .get()
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    conn.execute("DELETE FROM mcp_server_configs WHERE id = ?1", params![id])
        .map_err(|e| crate::error::AppError::internal(e.to_string()))
}

pub fn set_enabled(pool: &DbPool, id: &str, enabled: bool) -> Result<(), crate::error::AppError> {
    let conn = pool
        .get()
        .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    conn.execute(
        "UPDATE mcp_server_configs SET enabled = ?2, updated_at = datetime('now','localtime') WHERE id = ?1",
        params![id, enabled as i64],
    )
    .map_err(|e| crate::error::AppError::internal(e.to_string()))?;
    Ok(())
}

/// 读取所有 enabled 配置（启动自动连接用）
pub fn list_enabled_configs(
    pool: &DbPool,
) -> Result<Vec<crate::mcp::McpServerConfig>, crate::error::AppError> {
    Ok(list_servers(pool)?
        .into_iter()
        .filter(|s| s.enabled)
        .map(|s| s.config)
        .collect())
}
