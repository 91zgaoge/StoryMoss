use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        163
    }

    fn description(&self) -> &'static str {
        "continuation training chapter brief summaries cache (book/chapter/prompt version)"
    }

    /// v0.99.2：逐章摘要缓存。训练资产重建拆成「逐章摘要 + 聚合 brief」
    /// 两阶段后，单章摘要可以按 (book_ref, chapter_index, prompt_version)
    /// 复用；同一本书反复训练不再重复调用 LLM。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS continuation_training_chapter_briefs (\
                book_ref TEXT NOT NULL,\
                chapter_index INTEGER NOT NULL,\
                prompt_version TEXT NOT NULL,\
                summary_json TEXT NOT NULL,\
                fetched_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),\
                PRIMARY KEY (book_ref, chapter_index, prompt_version)\
            );\
            CREATE INDEX IF NOT EXISTS idx_ct_chapter_briefs_fetched \
             ON continuation_training_chapter_briefs(fetched_at);",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_chapter_brief_cache_table_idempotently() {
        let mut conn = Connection::open_in_memory().unwrap();
        Migration.apply(&mut conn).expect("首次建表");
        Migration.apply(&mut conn).expect("重复执行应幂等");

        let mut stmt = conn
            .prepare("PRAGMA table_info(continuation_training_chapter_briefs)")
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut columns = 0;
        while rows.next().unwrap().is_some() {
            columns += 1;
        }
        assert_eq!(columns, 5, "逐章摘要缓存表应有 5 列");
    }
}
