//! V145：章节摘要的内容指纹列。
//!
//! v0.64.12：没有指纹时「从第 N 章起重算」必须把 N 以后每一章都重算一遍
//! （100 章的书 = 90+ 次模型调用），自动后台重算就不可接受。加一列记录
//! 「这条摘要对应的正文指纹」，重算时**只重算指纹变了的章**。
//!
//! 幂等：列存在即跳过。

use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        145
    }

    fn description(&self) -> &'static str {
        "scene_commits.summary_source_hash (recompute only chapters whose prose changed)"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(scene_commits)")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        if !cols.iter().any(|c| c == "summary_source_hash") {
            conn.execute(
                "ALTER TABLE scene_commits ADD COLUMN summary_source_hash TEXT",
                [],
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    #[test]
    fn v145_adds_hash_column_idempotently() {
        let pool = create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();
        Migration.apply(&mut conn).unwrap();
        let has: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('scene_commits') \
                 WHERE name = 'summary_source_hash'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(has, 1);
    }
}
