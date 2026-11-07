use rusqlite::{params, Connection};

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        156
    }

    fn description(&self) -> &'static str {
        "persist MCP server configs (stdio + streamable HTTP) for startup auto-connect"
    }

    /// v0.88.0：MCP 连接此前只存在于内存（前端 connect_mcp_server 手动触发），
    /// 重启即丢，定时任务（续写训练）也无法在后端直连。本迁移建
    /// `mcp_server_configs` 表持久化配置：stdio 与流式 HTTP（url + 自定义头，
    /// 如栗子读书 legado 的 Bearer 令牌）统一存 config_json。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mcp_server_configs (\
             id TEXT PRIMARY KEY,\
             name TEXT NOT NULL,\
             config_json TEXT NOT NULL,\
             enabled INTEGER NOT NULL DEFAULT 1,\
             created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),\
             updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime')))",
            [],
        )?;
        Ok(())
    }
}

/// 迁移自测：建表 + 唯一约束 + enabled 默认值
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_table_with_defaults() {
        let mut conn = Connection::open_in_memory().unwrap();
        Migration.apply(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO mcp_server_configs (id, name, config_json) VALUES (?1, ?2, ?3)",
            params![
                "legado",
                "栗子读书",
                r#"{"id":"legado","url":"http://127.0.0.1:1122/mcp"}"#
            ],
        )
        .unwrap();
        let enabled: i64 = conn
            .query_row(
                "SELECT enabled FROM mcp_server_configs WHERE id = 'legado'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(enabled, 1);
        // 主键幂等：INSERT OR REPLACE 不炸
        conn.execute(
            "INSERT OR REPLACE INTO mcp_server_configs (id, name, config_json) VALUES (?1, ?2, ?3)",
            params!["legado", "栗子读书2", "{}"],
        )
        .unwrap();
    }
}
