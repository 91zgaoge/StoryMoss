use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        161
    }

    fn description(&self) -> &'static str {
        "continuation training source chapter cache: reuse fetched MCP chapters across runs"
    }

    /// v0.98.5：同一本书反复训练时会重复通过 MCP 取同样的章节。章节正文在
    /// 训练上下文中是只读、稳定的源材料，按 (server_id, book_ref,
    /// chapter_index) 缓存到本地 SQLite，下次训练直接命中，
    /// 避免重复读取与回环 HTTP 竞态。 `fetched_at`
    /// 仅用于诊断/后续清理，不参与命中判定。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS continuation_training_chapter_cache (\
                server_id TEXT NOT NULL,\
                book_ref TEXT NOT NULL,\
                chapter_index INTEGER NOT NULL,\
                content TEXT NOT NULL,\
                char_count INTEGER NOT NULL DEFAULT 0,\
                fetched_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),\
                PRIMARY KEY (server_id, book_ref, chapter_index)\
            );\
            CREATE INDEX IF NOT EXISTS idx_ct_chapter_cache_fetched \
             ON continuation_training_chapter_cache(fetched_at);",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_chapter_cache_table_idempotently() {
        let mut conn = Connection::open_in_memory().unwrap();
        Migration.apply(&mut conn).expect("首次建表");
        Migration.apply(&mut conn).expect("重复执行应幂等");

        let mut stmt = conn
            .prepare("PRAGMA table_info(continuation_training_chapter_cache)")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut columns = 0;
        while rows.next().unwrap().is_some() {
            columns += 1;
        }
        assert_eq!(columns, 6, "缓存表应有 6 列");
    }
}
