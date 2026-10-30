use rusqlite::{params, Connection};

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        148
    }

    fn description(&self) -> &'static str {
        "dedupe duplicate character entities in kg_entities by name"
    }

    /// 存量修复：v0.73.13 及更早版本里，`update_character` 的编辑后 ingest
    /// 会对已链接的实体调用 `create_entity`（重新生成 UUID），于是每编辑一次
    /// 角色（尤其改名）就在 `kg_entities` 多出一行同名角色。角色卡因此重复，
    /// 续写提示词里也会出现两个同名人。
    ///
    /// 这里按 `(story_id, name, entity_type='Character')` 合并：优先保留
    /// `characters` 表里有对应行的实体（用户创建路径 id 与 kg 一致），否则
    /// 保留最早出现的；把重复行的属性填进保留行的空字段，关系与
    /// `memory_items.kg_entity_id` 改线到保留行后删除重复行。
    ///
    /// 幂等：合并后同名组只剩一行，重跑无配对。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let tx = conn.transaction()?;
        let stories: Vec<String> = {
            let mut stmt = tx.prepare("SELECT id FROM stories")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let mut removed = 0usize;
        for story_id in &stories {
            let duplicate_names: Vec<String> = {
                let mut stmt = tx.prepare(
                    "SELECT name FROM kg_entities \
                     WHERE story_id = ?1 AND entity_type = 'Character' AND is_archived = 0 \
                     GROUP BY name HAVING COUNT(*) > 1",
                )?;
                let rows = stmt.query_map([story_id], |r| r.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };

            for name in duplicate_names {
                // (id, attributes, first_seen)，最早出现的排在最前
                let mut rows: Vec<(String, String, String)> = {
                    let mut stmt = tx.prepare(
                        "SELECT id, COALESCE(attributes, '{}'), first_seen FROM kg_entities \
                         WHERE story_id = ?1 AND entity_type = 'Character' AND name = ?2 \
                           AND is_archived = 0 \
                         ORDER BY first_seen ASC, id ASC",
                    )?;
                    let rows = stmt.query_map(params![story_id, &name], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    })?;
                    rows.collect::<Result<Vec<_>, _>>()?
                };
                if rows.len() < 2 {
                    continue;
                }

                // 保留与 legacy `characters` 行同 id 的实体（用户创建路径），
                // 否则保留最早出现的那行。
                let keeper_idx = rows
                    .iter()
                    .position(|(id, _, _)| {
                        tx.query_row(
                            "SELECT 1 FROM characters WHERE story_id = ?1 AND id = ?2 LIMIT 1",
                            params![story_id, id],
                            |_| Ok(()),
                        )
                        .is_ok()
                    })
                    .unwrap_or(0);
                let (keeper_id, keeper_attrs, _) = rows.remove(keeper_idx);
                let mut merged_attrs: serde_json::Value =
                    serde_json::from_str(&keeper_attrs).unwrap_or_else(|_| serde_json::json!({}));
                if !merged_attrs.is_object() {
                    merged_attrs = serde_json::json!({});
                }

                for (dup_id, dup_attrs, _) in rows {
                    // 关系先改线，再删实体（kg_relations 对 source/target 有
                    // FK）
                    tx.execute(
                        "UPDATE kg_relations SET source_id = ?3 \
                         WHERE story_id = ?1 AND source_id = ?2",
                        params![story_id, &dup_id, &keeper_id],
                    )?;
                    tx.execute(
                        "UPDATE kg_relations SET target_id = ?3 \
                         WHERE story_id = ?1 AND target_id = ?2",
                        params![story_id, &dup_id, &keeper_id],
                    )?;
                    // memory_items.kg_entity_id 有 FK 且无 CASCADE，先改线
                    tx.execute(
                        "UPDATE memory_items SET kg_entity_id = ?2 WHERE kg_entity_id = ?1",
                        params![&dup_id, &keeper_id],
                    )?;
                    // 场景提及也挂到保留实体（entity_mentions 对 kg_entities
                    // 有 ON DELETE CASCADE，不改线会随重复行一起丢掉）
                    tx.execute(
                        "UPDATE entity_mentions SET entity_id = ?2 WHERE entity_id = ?1",
                        params![&dup_id, &keeper_id],
                    )?;
                    if let Ok(dup_json) = serde_json::from_str::<serde_json::Value>(&dup_attrs) {
                        merge_missing_attrs(&mut merged_attrs, &dup_json);
                    }
                    tx.execute("DELETE FROM kg_entities WHERE id = ?1", [&dup_id])?;
                    removed += 1;
                }

                // 合并后可能出现自指关系，清掉
                tx.execute(
                    "DELETE FROM kg_relations \
                     WHERE story_id = ?1 AND source_id = ?2 AND target_id = ?2",
                    params![story_id, &keeper_id],
                )?;
                tx.execute(
                    "UPDATE kg_entities SET attributes = ?2, last_updated = ?3 WHERE id = ?1",
                    params![
                        &keeper_id,
                        merged_attrs.to_string(),
                        chrono::Local::now().to_rfc3339()
                    ],
                )?;
                log::info!(
                    "[V148] 合并同名角色「{}」的重复实体（保留 {}，story={}）",
                    name,
                    keeper_id,
                    story_id
                );
            }
        }

        tx.commit()?;
        if removed > 0 {
            log::info!("[V148] removed {} duplicate character entities", removed);
        }
        Ok(())
    }
}

/// 把 `extra` 中缺失或为空的字段补进 `base`，不覆盖已有非空值。
fn merge_missing_attrs(base: &mut serde_json::Value, extra: &serde_json::Value) {
    let (Some(base_map), Some(extra_map)) = (base.as_object_mut(), extra.as_object()) else {
        return;
    };
    for (key, value) in extra_map {
        match base_map.get(key) {
            None => {
                base_map.insert(key.clone(), value.clone());
            }
            Some(existing) if attr_is_empty(existing) && !attr_is_empty(value) => {
                base_map.insert(key.clone(), value.clone());
            }
            _ => {}
        }
    }
}

fn attr_is_empty(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.trim().is_empty(),
        serde_json::Value::Array(a) => a.is_empty(),
        serde_json::Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;
    use rusqlite::params;

    use super::*;

    fn seed_story(conn: &Connection, id: &str) {
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '测试', ?2, ?2)",
            params![id, now],
        )
        .unwrap();
    }

    fn add_entity(conn: &Connection, story: &str, id: &str, name: &str, attrs: &str) -> String {
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO kg_entities (id, story_id, name, entity_type, attributes, first_seen, \
             last_updated, is_archived) VALUES (?1, ?2, ?3, 'Character', ?4, ?5, ?5, 0)",
            params![id, story, name, attrs, now],
        )
        .unwrap();
        id.to_string()
    }

    /// 真机形态：同一角色被编辑两次后，kg_entities 里出现三行同名实体，
    /// 其中一行与 characters 表同 id。迁移后只留这一行，属性/关系/记忆挂载
    /// 都并过去。
    #[test]
    fn v148_merges_duplicate_character_entities() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        seed_story(&conn, "s1");
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, source, is_auto_generated, \
             created_at, updated_at) VALUES ('keeper', 's1', '林夏', '档案室', 'user_created', 0, \
             ?1, ?1)",
            params![now],
        )
        .unwrap();
        add_entity(&conn, "s1", "keeper", "林夏", r#"{"background":"档案室"}"#);
        add_entity(
            &conn,
            "s1",
            "phantom-1",
            "林夏",
            r#"{"personality":"冷静强势","background":""}"#,
        );
        add_entity(&conn, "s1", "phantom-2", "林夏", r#"{"goals":"查清真相"}"#);
        add_entity(&conn, "s1", "other", "谢慈", r#"{"background":"医生"}"#);
        conn.execute(
            "INSERT INTO kg_relations (id, story_id, source_id, target_id, relation_type, \
             strength, first_seen) VALUES ('r1', 's1', 'phantom-1', 'other', 'knows', 0.8, ?1)",
            params![now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO memory_items (id, story_id, category, subject, value, status, \
             updated_at, kg_entity_id) VALUES ('m1', 's1', 'character', '林夏', '冷静', 'active', \
             ?1, 'phantom-2')",
            params![now],
        )
        .unwrap();

        Migration.apply(&mut conn).unwrap();

        let ids: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT id FROM kg_entities WHERE story_id = 's1' AND name = '林夏' \
                     ORDER BY id",
                )
                .unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.collect::<Result<Vec<_>, _>>().unwrap()
        };
        assert_eq!(
            ids,
            vec!["keeper".to_string()],
            "只保留 characters 同 id 的实体"
        );

        let attrs: String = conn
            .query_row(
                "SELECT attributes FROM kg_entities WHERE id = 'keeper'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let attrs: serde_json::Value = serde_json::from_str(&attrs).unwrap();
        assert_eq!(attrs["background"], "档案室", "既有非空属性不被覆盖");
        assert_eq!(attrs["personality"], "冷静强势", "缺失属性从重复行补齐");
        assert_eq!(attrs["goals"], "查清真相", "缺失属性从重复行补齐");

        let source: String = conn
            .query_row(
                "SELECT source_id FROM kg_relations WHERE id = 'r1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(source, "keeper", "关系改线到保留实体");
        let memory_link: String = conn
            .query_row(
                "SELECT kg_entity_id FROM memory_items WHERE id = 'm1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(memory_link, "keeper", "记忆挂载改线到保留实体");

        let other: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_entities WHERE story_id = 's1' AND name = '谢慈'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(other, 1, "不同名角色不得被合并");

        // 幂等
        Migration.apply(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_entities WHERE story_id = 's1' AND name = '林夏'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
