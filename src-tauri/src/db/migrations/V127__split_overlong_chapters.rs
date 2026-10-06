use rusqlite::{Connection, OptionalExtension};

use crate::{
    config::DEFAULT_CHAPTER_SPLIT_MAX_CHARS,
    db::{
        chapter_split::{plan_split, split_chapter_in_tx, ChapterSplitMode},
        migrations::RustMigration,
    },
    utils::text::TextUtils,
};

/// 事务内聚合一章的全部场景内容（与 `split_latest_chapter_once` 同序）。
fn concat_scene_content(
    tx: &rusqlite::Transaction,
    chapter_id: &str,
) -> Result<String, rusqlite::Error> {
    let mut stmt = tx.prepare(
        "SELECT COALESCE(content, '') FROM scenes WHERE chapter_id = ?1 \
         ORDER BY sequence_number",
    )?;
    let rows = stmt.query_map([chapter_id], |r| r.get::<_, String>(0))?;
    let mut parts = Vec::new();
    for row in rows {
        parts.push(row?);
    }
    Ok(parts.concat())
}

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        127
    }

    fn description(&self) -> &'static str {
        "split overlong chapters and renumber following chapters"
    }

    /// 存量修复：此前自动分章只允许切"最新章"，续写持续写入非最新章时
    /// 该章会无限增长（真实数据出现单章 19,492 字）。找出所有内容字数超过
    /// 3000 字（`TextUtils::chinese_word_count`，聚合其 scenes 内容）的章，
    /// 用与 `find_split_offset` 相同的逻辑（WordCount 模式，段落/句末边界）
    /// 循环切分 + 重排后续章号，直到每章不超过 3000 字。全程单事务，失败回滚。
    /// 幂等：修复后所有章均不超过 3000 字，重跑无匹配。
    ///
    /// 多场景守卫（I-3）：章关联 scene 数 > 1 时跳过该章（不截断、不新建），
    /// 不阻断其他章修复——章内容是全场景拼接，截断只写首场景会造成正文重复。
    ///
    /// 不依赖 AppConfig：固定默认阈值 3000 / WordCount 模式；新章标题回退
    /// `第{N}章`（切分后章号 N+1 必无合约，见 `split_chapter_in_tx`）。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        const MAX_CHARS: usize = DEFAULT_CHAPTER_SPLIT_MAX_CHARS;
        const MAX_ITER: usize = 50; // 与 chapter_splitter::MAX_SPLIT_ITERATIONS
                                    // 对齐

        let tx = conn.transaction()?;
        let chapters: Vec<(String, String)> = {
            let mut stmt =
                tx.prepare("SELECT id, story_id FROM chapters ORDER BY story_id, chapter_number")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            out
        };

        let mut total_splits = 0usize;
        for (chapter_id, story_id) in chapters {
            let mut current_id = chapter_id;
            let mut splits = 0usize;
            for _ in 0..MAX_ITER {
                let Some(number) = tx
                    .query_row(
                        "SELECT chapter_number FROM chapters WHERE id = ?1",
                        [&current_id],
                        |r| r.get::<_, i32>(0),
                    )
                    .optional()?
                else {
                    break;
                };
                let content = concat_scene_content(&tx, &current_id)?;
                if TextUtils::chinese_word_count(&content) <= MAX_CHARS {
                    break;
                }
                let Some(plan) = plan_split(&content, ChapterSplitMode::WordCount, MAX_CHARS)
                else {
                    // 找不到切分点（无边界且 keep 为 0 字等）：放弃该章
                    log::warn!(
                        "[V127] chapter {} (story {}) 超阈值但找不到切分点，跳过",
                        current_id,
                        story_id
                    );
                    break;
                };
                let Some(scene_id) = tx
                    .query_row(
                        "SELECT id FROM scenes WHERE chapter_id = ?1 \
                         ORDER BY sequence_number LIMIT 1",
                        [&current_id],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?
                else {
                    break;
                };
                // None = 多场景章被守卫跳过（不截断、不新建；守卫内已告警），
                // 放弃该章但不阻断其他章修复
                let Some((new_id, _, _)) =
                    split_chapter_in_tx(&tx, &story_id, &current_id, number, &plan, &scene_id)?
                else {
                    break;
                };
                total_splits += 1;
                splits += 1;
                current_id = new_id;
            }
            // M-4：达迭代上限仍未压到阈值内 → 明示哪章残留多少字
            if splits == MAX_ITER {
                let remaining = concat_scene_content(&tx, &current_id)?;
                let wc = TextUtils::chinese_word_count(&remaining);
                if wc > MAX_CHARS {
                    log::warn!(
                        "[V127] chapter {} (story {}) 达迭代上限 {} 仍残留 {} 字（> {}），未完全修复",
                        current_id,
                        story_id,
                        MAX_ITER,
                        wc,
                        MAX_CHARS
                    );
                }
            }
        }

        tx.commit()?;
        if total_splits > 0 {
            log::info!("[V127] split overlong chapters: {} splits", total_splits);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;
    use rusqlite::params;

    use super::*;
    use crate::utils::text::TextUtils;

    fn insert_chapter_with_content(
        conn: &Connection,
        id: &str,
        story_id: &str,
        number: i32,
        title: &str,
        content: &str,
    ) {
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title, outline, word_count,
             model_used, cost, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, NULL, 0, NULL, 0.0, ?5, ?5)",
            params![id, story_id, number, title, now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, content,
             characters_present, character_conflicts, execution_stage, chapter_id,
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, '[]', '[]', 'drafting', ?6, ?7, ?7)",
            params![
                format!("sc-{}", id),
                story_id,
                number,
                title,
                content,
                id,
                now
            ],
        )
        .unwrap();
    }

    fn chapter_numbers(conn: &Connection, story_id: &str) -> Vec<(String, i32)> {
        conn.prepare(
            "SELECT id, chapter_number FROM chapters WHERE story_id = ?1 ORDER BY chapter_number",
        )
        .unwrap()
        .query_map([story_id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    }

    fn chapter_content(conn: &Connection, chapter_id: &str) -> String {
        conn.prepare(
            "SELECT COALESCE(content, '') FROM scenes WHERE chapter_id = ?1 ORDER BY sequence_number",
        )
        .unwrap()
        .query_map([chapter_id], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .concat()
    }

    /// 全量快照：所有章（id, 章号, 标题, 字数）+ 所有场景（id, 序号, 所属章,
    /// 内容）， 供幂等断言逐字段对比（M-1：替代旧的恒真式断言）。
    fn full_snapshot(
        conn: &Connection,
    ) -> (
        Vec<(String, i32, Option<String>, i32)>,
        Vec<(String, i32, Option<String>, String)>,
    ) {
        let chapters = conn
            .prepare("SELECT id, chapter_number, title, word_count FROM chapters ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let scenes = conn
            .prepare(
                "SELECT id, sequence_number, chapter_id, COALESCE(content, '') FROM scenes \
                 ORDER BY id",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        (chapters, scenes)
    }

    #[test]
    fn v127_splits_overlong_chapter_and_renumbers_followers() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s1', '测试', ?1, ?1)",
            [&now],
        )
        .unwrap();

        // 真实事故现场：单章 19,492 字（10 段×1800 + 尾段 1492），其后还有一章
        let para = "文".repeat(1800);
        let tail = "尾".repeat(1492);
        let mut parts: Vec<&str> = vec![para.as_str(); 10];
        parts.push(tail.as_str());
        let big = parts.join("\n\n");
        assert_eq!(TextUtils::chinese_word_count(&big), 19492);
        insert_chapter_with_content(&conn, "c1", "s1", 1, "第1章", &big);
        insert_chapter_with_content(&conn, "c2", "s1", 2, "第2章", "后续章内容");

        Migration.apply(&mut conn).unwrap();

        let chapters = chapter_numbers(&conn, "s1");
        assert!(
            chapters.len() >= 8,
            "19492 字应切出多章, got {}",
            chapters.len()
        );

        // 每章 ≤ 3000 字
        for (cid, number) in &chapters {
            let wc = TextUtils::chinese_word_count(&chapter_content(&conn, cid));
            assert!(wc <= 3000, "chapter {} exceeds threshold: {}", number, wc);
        }

        // 旧第2章顺延到队尾，章号连续 1..=N
        let c2_number = chapters.iter().find(|(id, _)| id == "c2").unwrap().1;
        assert_eq!(c2_number, chapters.len() as i32);
        let numbers: Vec<i32> = chapters.iter().map(|(_, n)| *n).collect();
        assert_eq!(numbers, (1..=chapters.len() as i32).collect::<Vec<_>>());

        // scenes.sequence_number 与章号对齐
        let scene_seqs: Vec<i32> = conn
            .prepare(
                "SELECT sequence_number FROM scenes WHERE story_id = 's1' ORDER BY sequence_number",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(scene_seqs, (1..=chapters.len() as i32).collect::<Vec<_>>());

        // 幂等（M-1）：重跑前快照全部章 + 场景，重跑后逐字段对比相等
        let before = full_snapshot(&conn);
        Migration.apply(&mut conn).unwrap();
        assert_eq!(
            full_snapshot(&conn),
            before,
            "重跑 apply 不应改动任何章/场景"
        );
    }

    #[test]
    fn v127_leaves_within_threshold_chapters_untouched() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s2', '测试', ?1, ?1)",
            [&now],
        )
        .unwrap();
        insert_chapter_with_content(&conn, "c1", "s2", 1, "第1章", &"短".repeat(1000));
        insert_chapter_with_content(&conn, "c2", "s2", 2, "第2章", &"章".repeat(2000));

        Migration.apply(&mut conn).unwrap();

        assert_eq!(
            chapter_numbers(&conn, "s2"),
            vec![("c1".to_string(), 1), ("c2".to_string(), 2)]
        );
        assert_eq!(chapter_content(&conn, "c1"), "短".repeat(1000));
    }

    /// 多场景守卫（I-3）：多场景超长章跳过（不截断、不新建、
    /// 不阻断其他章修复）。
    #[test]
    fn v127_skips_multi_scene_chapter_but_fixes_others() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        for sid in ["s-multi", "s-single"] {
            conn.execute(
                "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '测试', ?2, ?2)",
                params![sid, now],
            )
            .unwrap();
        }

        // s-multi：cm1 两个场景（2000 + 2000 = 4000 字 > 3000）
        insert_chapter_with_content(&conn, "cm1", "s-multi", 1, "第1章", &"首".repeat(2000));
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, content,
             characters_present, character_conflicts, execution_stage, chapter_id,
             created_at, updated_at)
             VALUES ('sc-cm1-b', 's-multi', 2, '第1章', ?1, '[]', '[]', 'drafting', 'cm1', ?2, ?2)",
            params!["续".repeat(2000), now],
        )
        .unwrap();

        // s-single：cs1 单场景 3600 字（两段落，可切）
        let big = format!("{}\n\n{}", "甲".repeat(1800), "乙".repeat(1800));
        insert_chapter_with_content(&conn, "cs1", "s-single", 1, "第1章", &big);

        Migration.apply(&mut conn).unwrap();

        // 多场景章原样保留：未截断、未新建
        assert_eq!(
            chapter_numbers(&conn, "s-multi"),
            vec![("cm1".to_string(), 1)],
            "多场景章不应切出新章"
        );
        let contents: Vec<String> = conn
            .prepare(
                "SELECT COALESCE(content, '') FROM scenes WHERE chapter_id = 'cm1' \
                 ORDER BY sequence_number",
            )
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(contents, vec!["首".repeat(2000), "续".repeat(2000)]);
        assert!(
            TextUtils::chinese_word_count(&chapter_content(&conn, "cm1")) > 3000,
            "守卫是跳过而非修复，聚合内容仍超阈值"
        );

        // 其他 story 的单场景超长章正常修复
        let chapters = chapter_numbers(&conn, "s-single");
        assert_eq!(chapters.len(), 2, "3600 字应切出 1 章");
        for (cid, number) in &chapters {
            let wc = TextUtils::chinese_word_count(&chapter_content(&conn, cid));
            assert!(wc <= 3000, "chapter {} exceeds threshold: {}", number, wc);
        }
    }
}
