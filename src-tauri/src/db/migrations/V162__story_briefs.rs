use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        162
    }

    fn description(&self) -> &'static str {
        "story briefs: long-range recap (chapter facts, open threads, cast, canon terms)"
    }

    /// v0.99.1：续写长程记忆。训练资产重建/确定性兜底把前 10 章压成一份
    /// 结构化《前情提要》，writer 与 planner 全量注入；表随故事级联清理。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS story_briefs (\
                story_id TEXT PRIMARY KEY,\
                content_json TEXT NOT NULL,\
                source TEXT NOT NULL DEFAULT 'unknown',\
                created_at TEXT NOT NULL,\
                updated_at TEXT NOT NULL\
            );\
            CREATE INDEX IF NOT EXISTS idx_story_briefs_updated \
             ON story_briefs(updated_at);",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_story_briefs_table_idempotently() {
        let mut conn = Connection::open_in_memory().unwrap();
        Migration.apply(&mut conn).expect("首次建表");
        Migration.apply(&mut conn).expect("重复执行应幂等");

        let mut stmt = conn.prepare("PRAGMA table_info(story_briefs)").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut columns = 0;
        while rows.next().unwrap().is_some() {
            columns += 1;
        }
        assert_eq!(columns, 5, "故事簿表应有 5 列");
    }
}
