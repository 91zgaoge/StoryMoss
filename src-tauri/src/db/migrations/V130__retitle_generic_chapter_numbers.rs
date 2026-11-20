use chrono::Local;
use rusqlite::{params, Connection};

use crate::db::{
    chapter_split::{canonical_chapter_title, is_generic_chapter_title},
    migrations::RustMigration,
};

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        130
    }

    fn description(&self) -> &'static str {
        "retitle generic 第N章 / 第一章 to match chapter_number after split renumber"
    }

    /// 自动分章重排只 +1 章号、不改派生标题，列表会出现「第7章」后还挂
    /// 「第6章」。把「第N章」「第一章」这类标题改成当前章号。手写标题不动。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let now = Local::now().to_rfc3339();
        let tx = conn.transaction()?;
        let rows: Vec<(String, i32, Option<String>)> = {
            let mut stmt = tx.prepare("SELECT id, chapter_number, title FROM chapters")?;
            let mapped = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
            let mut out = Vec::new();
            for row in mapped {
                out.push(row?);
            }
            out
        };

        let mut changed = 0usize;
        for (id, number, title) in rows {
            let raw = title.unwrap_or_default();
            if !is_generic_chapter_title(&raw) && !raw.trim().is_empty() {
                continue;
            }
            let new_title = canonical_chapter_title(number);
            if raw.trim() == new_title {
                continue;
            }
            tx.execute(
                "UPDATE chapters SET title = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, new_title, now],
            )?;
            tx.execute(
                "UPDATE scenes SET title = ?2, updated_at = ?3 WHERE chapter_id = ?1 \
                 AND (title IS NULL OR TRIM(title) = '' OR title = ?4)",
                params![id, new_title, now, raw],
            )?;
            changed += 1;
        }

        tx.commit()?;
        if changed > 0 {
            log::info!("[V130] retitled {changed} generic chapter titles to match chapter_number");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::*;
    use crate::db::migrations::RustMigration;

    fn insert_chapter(conn: &Connection, id: &str, story_id: &str, number: i32, title: &str) {
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title, outline, word_count,
             model_used, cost, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, NULL, 0, NULL, 0.0, ?5, ?5)",
            params![id, story_id, number, title, now],
        )
        .unwrap();
    }

    fn chapter_title(conn: &Connection, id: &str) -> String {
        conn.query_row("SELECT title FROM chapters WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap()
    }

    #[test]
    fn v130_retitles_stale_generic_and_keeps_custom() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s1', '测试', ?1, ?1)",
            [&now],
        )
        .unwrap();

        insert_chapter(&conn, "c1", "s1", 1, "第一章");
        insert_chapter(&conn, "c6", "s1", 8, "第6章");
        insert_chapter(&conn, "c7", "s1", 7, "第7章");
        insert_chapter(&conn, "cc", "s1", 3, "临江夜雨");

        Migration.apply(&mut conn).unwrap();

        assert_eq!(chapter_title(&conn, "c1"), "第一章");
        assert_eq!(chapter_title(&conn, "c6"), "第八章");
        assert_eq!(chapter_title(&conn, "c7"), "第七章");
        assert_eq!(chapter_title(&conn, "cc"), "临江夜雨");

        Migration.apply(&mut conn).unwrap();
        assert_eq!(chapter_title(&conn, "c6"), "第八章");
        assert_eq!(chapter_title(&conn, "cc"), "临江夜雨");
    }
}
