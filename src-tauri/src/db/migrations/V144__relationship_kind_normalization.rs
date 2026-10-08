//! V144：关系类型归一列。
//!
//! v0.64.11：真机 46 行关系里有 28
//! 种自由写法（复合/斜杠/括注），确定性消费者只能
//! 子串匹配。新增 `relation_kind`（受控词表）+
//! `relation_flags`（位标志：敌意/血亲/ 配偶/师徒/主仆/盟友/竞争/交易），并按
//! `db::relation_kind::RelationClass` 回填 存量；`relationship_type`
//! 原样保留（作者可见标签）。
//!
//! 幂等：列已存在则跳过；回填为覆盖写（分类是纯函数）。

use rusqlite::{params, Connection};

use crate::db::{migrations::RustMigration, relation_kind::RelationClass};

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        144
    }

    fn description(&self) -> &'static str {
        "normalize character relationship types (kind + flags)"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let cols: Vec<String> = {
            let mut stmt = conn.prepare("PRAGMA table_info(character_relationships)")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        if !cols.iter().any(|c| c == "relation_kind") {
            conn.execute(
                "ALTER TABLE character_relationships ADD COLUMN relation_kind TEXT",
                [],
            )?;
        }
        if !cols.iter().any(|c| c == "relation_flags") {
            conn.execute(
                "ALTER TABLE character_relationships ADD COLUMN relation_flags INTEGER",
                [],
            )?;
        }

        // 回填存量（只认词表内的写法；认不出的写「其他」，仍保留原标签）
        let rows: Vec<(String, String, Option<String>)> = {
            let mut stmt = conn.prepare(
                "SELECT id, relationship_type, emotional_bond FROM character_relationships",
            )?;
            let mapped = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?;
            mapped.collect::<Result<Vec<_>, _>>()?
        };
        let mut unknown = 0usize;
        for (id, raw, bond) in &rows {
            let class = RelationClass::classify(raw, bond.as_deref());
            if !class.is_supported_kind() {
                unknown += 1;
            }
            conn.execute(
                "UPDATE character_relationships SET relation_kind = ?2, relation_flags = ?3 \
                 WHERE id = ?1",
                params![id, class.kind, class.flags()],
            )?;
        }
        if !rows.is_empty() {
            log::warn!(
                "[V144] 关系类型归一：{} 行已回填，其中 {} 行未认出受控类型（记为「其他」，原标签保留）",
                rows.len(),
                unknown
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    #[test]
    fn v144_backfills_kind_and_flags_idempotently() {
        let pool = create_test_pool().unwrap();
        let story_id = uuid::Uuid::new_v4().to_string();
        let (a, b) = (
            uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4().to_string(),
        );
        let now = chrono::Local::now().to_rfc3339();
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '归一', ?2, ?2)",
                params![&story_id, &now],
            )
            .unwrap();
            for (id, name) in [(&a, "甲"), (&b, "乙")] {
                conn.execute(
                    "INSERT INTO characters (id, story_id, name, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![id, &story_id, name, &now],
                )
                .unwrap();
            }
            conn.execute(
                "INSERT INTO character_relationships \
                 (id, story_id, source_character_id, target_character_id, relationship_type, \
                  created_at) VALUES ('r1', ?1, ?2, ?3, '夫妻（名分）／仇敌', ?4)",
                params![&story_id, &a, &b, &now],
            )
            .unwrap();
        }

        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();
        Migration.apply(&mut conn).unwrap();

        let (kind, flags): (String, i64) = conn
            .query_row(
                "SELECT relation_kind, relation_flags FROM character_relationships WHERE id='r1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(kind, "夫妻");
        let class = RelationClass::from_flags(&kind, Some(flags));
        assert!(class.spouse && class.hostile, "{class:?}");
        // 原标签保留
        let raw: String = conn
            .query_row(
                "SELECT relationship_type FROM character_relationships WHERE id='r1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(raw, "夫妻（名分）／仇敌");
    }
}
