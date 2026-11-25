//! v0.99.24：训练报告通用规则提升层。
//!
//! 训练报告中的维度短板/探针问题会泛化为通用规则（不包含具体书名/人名/
//! 情节），按 rule_key 去重累计；生产续写按活跃规则注入 writer/gate，并
//! 由生产回归探针核对。规则表跨重启保留，训练越跑越完整。

use rusqlite::Connection;

pub struct Migration;

impl crate::db::migrations::RustMigration for Migration {
    fn version(&self) -> i32 {
        165
    }

    fn description(&self) -> &'static str {
        "training report promotion rules (generic, no book-specific names/plot)"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS training_rule_promotions (\
                id TEXT PRIMARY KEY,\
                rule_key TEXT NOT NULL UNIQUE,\
                dimension TEXT NOT NULL,\
                probe_kind TEXT NOT NULL,\
                rule_text TEXT NOT NULL,\
                source_report_id TEXT,\
                score REAL,\
                promotion_count INTEGER NOT NULL DEFAULT 1,\
                active INTEGER NOT NULL DEFAULT 1,\
                created_at TEXT NOT NULL,\
                updated_at TEXT NOT NULL\
            );\
            CREATE INDEX IF NOT EXISTS idx_training_rule_active \
             ON training_rule_promotions(active, promotion_count DESC);",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::RustMigration;

    #[test]
    fn migration_creates_rule_table_and_is_idempotent() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        Migration.apply(&mut conn).unwrap();
        Migration.apply(&mut conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM training_rule_promotions", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
