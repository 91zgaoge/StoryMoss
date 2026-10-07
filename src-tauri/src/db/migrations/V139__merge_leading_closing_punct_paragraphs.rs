use rusqlite::{params, Connection};

use crate::{db::migrations::RustMigration, utils::text::TextUtils};

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        139
    }

    fn description(&self) -> &'static str {
        "merge paragraph-leading closing punct in scene content"
    }

    /// 存量修复：句子切分在句末标点处断开，把紧随其后的收尾引号切给了下一句；
    /// 段落正好在那一处断开时，HTML 里落成 `<p>”\n正文…</p>`——孤引号独占一行
    /// （真机《帝国的烟火》第 2
    /// 章：`…还是你苏家的命。</p><p>”\n大堂内的空气…`）。
    /// V128 只覆盖「整段仅闭合标点」的形态，引号后面还跟着正文的漏掉了。
    ///
    /// 对所有含 `<p>` 的 scene 跑
    /// `TextUtils::merge_leading_closing_punct_paragraphs`（与前端
    /// `format.ts::mergeLeadingClosingPunctParagraphs` 同规则），有变化才
    /// UPDATE。 幂等：合并后段首不再有闭合标点，重跑无匹配；空库 no-op。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let tx = conn.transaction()?;
        let rows: Vec<(String, String)> = {
            let mut stmt = tx.prepare(
                "SELECT id, COALESCE(content, '') FROM scenes WHERE content LIKE '%</p>%'",
            )?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            out
        };

        let mut changed = 0usize;
        for (id, content) in rows {
            let merged = TextUtils::merge_leading_closing_punct_paragraphs(&content);
            if merged != content {
                tx.execute(
                    "UPDATE scenes SET content = ?1 WHERE id = ?2",
                    params![merged, id],
                )?;
                changed += 1;
            }
        }

        tx.commit()?;
        if changed > 0 {
            log::info!(
                "[V139] merged paragraph-leading closing punct in {} scenes",
                changed
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;

    fn insert_scene(conn: &Connection, id: &str, story_id: &str, seq: i32, content: &str) {
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, content,
             characters_present, character_conflicts, execution_stage, chapter_id,
             created_at, updated_at)
             VALUES (?1, ?2, ?3, '场景', ?4, '[]', '[]', 'drafting', NULL, ?5, ?5)",
            params![id, story_id, seq, content, now],
        )
        .unwrap();
    }

    fn scene_content(conn: &Connection, id: &str) -> String {
        conn.query_row(
            "SELECT COALESCE(content, '') FROM scenes WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn v139_merges_paragraph_leading_closing_punct() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s1', '测试', ?1, ?1)",
            [&now],
        )
        .unwrap();

        // 真机形态：段首引号 + 换行 + 正文
        insert_scene(
            &conn,
            "sc-leading",
            "s1",
            1,
            "<p>…还是你苏家的命。</p><p>”\n大堂内的空气仿佛凝固成了实质。</p>",
        );
        // 实体形态
        insert_scene(
            &conn,
            "sc-entity",
            "s1",
            2,
            "<p>甲。</p><p>&rdquo;\n乙丙丁。</p>",
        );
        // ASCII 直引号可能是开引号 → 不动
        insert_scene(&conn, "sc-ascii", "s1", 3, "<p>甲。</p><p>\"\n乙丙丁。</p>");
        // 正常段落不动
        insert_scene(
            &conn,
            "sc-good",
            "s1",
            4,
            "<p>正常段落。</p><p>另一段。</p>",
        );
        // 无 <p> 不动
        insert_scene(&conn, "sc-plain", "s1", 5, "纯文本内容\n没有段落标签");

        Migration.apply(&mut conn).unwrap();

        assert_eq!(
            scene_content(&conn, "sc-leading"),
            "<p>…还是你苏家的命。”</p><p>大堂内的空气仿佛凝固成了实质。</p>"
        );
        assert_eq!(
            scene_content(&conn, "sc-entity"),
            "<p>甲。&rdquo;</p><p>乙丙丁。</p>"
        );
        assert_eq!(
            scene_content(&conn, "sc-ascii"),
            "<p>甲。</p><p>\"\n乙丙丁。</p>"
        );
        assert_eq!(
            scene_content(&conn, "sc-good"),
            "<p>正常段落。</p><p>另一段。</p>"
        );
        assert_eq!(scene_content(&conn, "sc-plain"), "纯文本内容\n没有段落标签");

        // 幂等：重跑后逐字段对比相等
        let snapshot = |c: &Connection| {
            c.prepare("SELECT id, COALESCE(content, '') FROM scenes ORDER BY id")
                .unwrap()
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        let before = snapshot(&conn);
        Migration.apply(&mut conn).unwrap();
        assert_eq!(snapshot(&conn), before, "重跑 apply 不应改动任何场景");
    }

    /// V128 已并过的孤闭合标字段，V139 重跑不得再动。
    #[test]
    fn v139_is_noop_on_v128_result() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s1', '测试', ?1, ?1)",
            [&now],
        )
        .unwrap();
        let after_v128 =
            TextUtils::merge_lone_closing_punct_paragraphs("<p>他控制着局面。</p><p>”</p>");
        insert_scene(&conn, "sc-v128", "s1", 1, &after_v128);
        Migration.apply(&mut conn).unwrap();
        assert_eq!(scene_content(&conn, "sc-v128"), after_v128);
    }

    #[test]
    fn v139_empty_db_is_noop() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();
    }
}
