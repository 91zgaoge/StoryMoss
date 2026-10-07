use rusqlite::Connection;

use crate::db::{character_identity, migrations::RustMigration};

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        141
    }

    fn description(&self) -> &'static str {
        "merge character rows that are the same person (title forms)"
    }

    /// 存量修复：`characters`
    /// 里同一人物的不同称呼被建成了多行（真机《帝国的烟火》：
    /// `景亲王` 与 `景亲王曹元寿`、`镇北王苏会山` 与 `苏会山` 各占一行），
    /// 关系表、场景关联与提示词随之处处出现"两个同一个人"。
    ///
    /// 这里只做**确定性**的合并：
    /// 名字互为称号形态（`continue_director::same_person`）
    /// 的两行并成一行，保留信息更全的一行（简介更长者优先，其次先建的），
    /// 被并掉的称呼登记为别称。称呼表（字/号/官称这类无形态规律的可变称呼）由
    /// ingest 抽到的 aliases 在运行时归并，迁移不猜。
    ///
    /// 幂等：合并后同一人物只剩一行，重跑无配对。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let tx = conn.transaction()?;
        let stories: Vec<String> = {
            let mut stmt = tx.prepare("SELECT id FROM stories")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let mut merged_pairs = 0usize;
        for story_id in &stories {
            // (id, name, background) —— 背景更长者视为信息更全
            let mut chars: Vec<(String, String, String)> = {
                let mut stmt = tx.prepare(
                    "SELECT id, name, COALESCE(background, '') FROM characters \
                     WHERE story_id = ?1 ORDER BY created_at ASC, id ASC",
                )?;
                let rows = stmt.query_map([story_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            if chars.len() < 2 {
                continue;
            }

            let mut keep: Vec<(String, String, String)> = Vec::new();
            for (id, name, bg) in chars.drain(..) {
                let mut absorbed = false;
                for slot in keep.iter_mut() {
                    if !crate::agency::continue_director::same_person(&slot.1, &name) {
                        continue;
                    }
                    // 保留信息更全的一行（简介更长者），被并者登记为别称
                    let (from_id, from_name, to_id, to_name) =
                        if bg.chars().count() > slot.2.chars().count() {
                            let prev = (slot.0.clone(), slot.1.clone());
                            slot.0 = id.clone();
                            slot.1 = name.clone();
                            slot.2 = bg.clone();
                            (prev.0, prev.1, id.clone(), name.clone())
                        } else {
                            (id.clone(), name.clone(), slot.0.clone(), slot.1.clone())
                        };
                    let report =
                        character_identity::merge_characters(&tx, story_id, &from_id, &to_id)?;
                    if report.any_change() || from_id != to_id {
                        merged_pairs += 1;
                        log::info!(
                            "[V141] 合并同一人物「{}」→「{}」(story={})",
                            from_name,
                            to_name,
                            story_id
                        );
                    }
                    absorbed = true;
                    break;
                }
                if !absorbed {
                    keep.push((id, name, bg));
                }
            }
        }

        tx.commit()?;
        if merged_pairs > 0 {
            log::info!(
                "[V141] merged {} same-person character row(s)",
                merged_pairs
            );
        }
        Ok(())
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

    fn add_character(conn: &Connection, story: &str, name: &str, bg: &str) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, source, is_auto_generated, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'ingest', 1, ?5, ?5)",
            params![id, story, name, bg, now],
        )
        .unwrap();
        id
    }

    /// 真机形态：`景亲王` 与 `景亲王曹元寿` 是一个人的两种称呼。
    #[test]
    fn v141_merges_title_form_rows() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        seed_story(&conn, "s1");
        let short = add_character(&conn, "s1", "景亲王", "皇上之弟");
        let long = add_character(
            &conn,
            "s1",
            "景亲王曹元寿",
            "景亲王，太上皇贵妃琮妃之子，琬公主曹元佩的同胞哥哥",
        );

        Migration.apply(&mut conn).unwrap();

        let chars: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM characters WHERE story_id = 's1'")
                .unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            rows.collect::<Result<Vec<_>, _>>().unwrap()
        };
        assert_eq!(chars.len(), 1, "只留一行 chars={chars:?}");
        assert_eq!(chars[0], "景亲王曹元寿", "保留信息更全的一行");
        // 被并掉的称呼成为别称：此后按它解析仍回到本人
        let resolved = character_identity::resolve_character_id(&conn, "s1", "景亲王")
            .unwrap()
            .unwrap();
        assert_eq!(resolved, long, "别名应指向保留行");
        assert_ne!(resolved, short);

        // 幂等
        let before: Vec<String> = chars.clone();
        Migration.apply(&mut conn).unwrap();
        let after: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM characters WHERE story_id = 's1'")
                .unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            rows.collect::<Result<Vec<_>, _>>().unwrap()
        };
        assert_eq!(after, before);
    }

    /// 不同人物不得被误并（父子各有本名）。
    #[test]
    fn v141_keeps_distinct_people() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        seed_story(&conn, "s1");
        add_character(&conn, "s1", "苏会山", "镇北王");
        add_character(&conn, "s1", "苏亦铁", "苏会山长子");
        add_character(&conn, "s1", "苏亦俭", "苏会山次子");

        Migration.apply(&mut conn).unwrap();

        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM characters WHERE story_id = 's1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 3, "父子兄弟各是本名，不得合并");
    }
}
