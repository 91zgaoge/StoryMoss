//! V143：物料失效标记表。
//!
//! v0.64.11：编辑旧章后，从旧正文推出来的跨章物料（后续章节摘要、分层摘要/全书
//! 纲要、连续性快照）不会自动失效——此前连「哪里过期了」都无处记录。本表按
//! (story_id, kind) 记一行，`from_chapter` 取最早失效章（编辑第 9 章 → 从第 9
//! 章 起失效），供运行维护页展示与一键重算（`story_system::recompute`）。
//!
//! 幂等：表存在即跳过。

use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        143
    }

    fn description(&self) -> &'static str {
        "story material staleness (edited chapter invalidates derived material)"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS story_material_staleness (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                from_chapter INTEGER NOT NULL,
                reason TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(story_id, kind)
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_material_staleness_story \
             ON story_material_staleness(story_id)",
            [],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    #[test]
    fn v143_creates_staleness_table_idempotently() {
        let pool = create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();
        Migration.apply(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='story_material_staleness'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
