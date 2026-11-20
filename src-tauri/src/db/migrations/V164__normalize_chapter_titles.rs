//! v0.99.8：统一旧库章节标题。
//!
//! 历史数据里同时存在：
//! - `第 134 章`（训练沙盒旧格式，丢了原书标题正文）
//! - `第12章`（自动取名未完成的系统占位）
//! - `香炉边的银镯与铜钱`（裸标题）
//! - `第二章 铜钱与脚印`（中文数字）
//!
//! 一次性收敛为「第<中文数字>章 <标题>」；训练沙盒优先从正文首行
//! 「第134章 魑魅魍魉横行」恢复原书标题。

use rusqlite::{params, Connection};

use crate::{
    db::migrations::RustMigration, utils::chapter_title::normalize_existing_chapter_title,
};

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        164
    }

    fn description(&self) -> &'static str {
        "normalize chapter/scene titles to chinese-numbered chapter title + body"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let chapters: Vec<(String, i32, Option<String>)> = {
            let mut stmt = conn.prepare("SELECT id, chapter_number, title FROM chapters")?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for (id, chapter_number, title) in chapters {
            let content: String = conn
                .query_row(
                    "SELECT COALESCE(content, '') FROM scenes \
                     WHERE chapter_id = ?1 ORDER BY sequence_number LIMIT 1",
                    [&id],
                    |row| row.get(0),
                )
                .unwrap_or_default();
            let raw = title.unwrap_or_default();
            let normalized =
                normalize_existing_chapter_title(chapter_number.max(0) as u32, &raw, &content);
            if normalized != raw {
                conn.execute(
                    "UPDATE chapters SET title = ?2 WHERE id = ?1",
                    params![&id, &normalized],
                )?;
            }
            conn.execute(
                "UPDATE scenes SET title = ?2 WHERE chapter_id = ?1 \
                 AND (title IS NULL OR TRIM(title) = '' OR TRIM(title) = ?3)",
                params![&id, &normalized, &raw],
            )?;
        }

        // 未关联章节的独立场景（导演/手写场景）：按自身序号统一。
        let scenes: Vec<(String, i32, Option<String>, String)> = {
            let mut stmt = conn.prepare(
                "SELECT id, sequence_number, title, COALESCE(content, '') \
                 FROM scenes WHERE chapter_id IS NULL",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for (id, sequence_number, title, content) in scenes {
            let raw = title.unwrap_or_default();
            let normalized =
                normalize_existing_chapter_title(sequence_number.max(0) as u32, &raw, &content);
            if normalized != raw {
                conn.execute(
                    "UPDATE scenes SET title = ?2 WHERE id = ?1",
                    params![&id, &normalized],
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE chapters (\
                id TEXT PRIMARY KEY, story_id TEXT, chapter_number INTEGER, title TEXT\
             );\
             CREATE TABLE scenes (\
                id TEXT PRIMARY KEY, story_id TEXT, sequence_number INTEGER, title TEXT,\
                content TEXT, chapter_id TEXT\
             );",
        )
        .unwrap();
    }

    #[test]
    fn normalizes_mixed_titles_and_recovers_source_title() {
        let mut conn = Connection::open_in_memory().unwrap();
        setup(&conn);
        conn.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title) VALUES ('c1','s',1,'第 134 章')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, content, chapter_id) \
             VALUES ('sc1','s',1,'第 134 章','第134章 魑魅魍魉横行\n正文。','c1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title) VALUES ('c2','s',2,'雨夜叩门')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, content, chapter_id) \
             VALUES ('sc2','s',1,'第一场 雨夜叩门','雨声。','c2')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title) VALUES ('c3','s',12,'第12章')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title) VALUES ('c4','s',13,'香炉边的银镯与铜钱')",
            [],
        )
        .unwrap();

        Migration.apply(&mut conn).unwrap();

        let title = |id: &str| -> String {
            conn.query_row("SELECT title FROM chapters WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .unwrap()
        };
        assert_eq!(title("c1"), "第一百三十四章 魑魅魍魉横行");
        assert_eq!(title("c2"), "第二章 雨夜叩门");
        assert_eq!(title("c3"), "第十二章");
        assert_eq!(title("c4"), "第十三章 香炉边的银镯与铜钱");
        let scene_title: String = conn
            .query_row("SELECT title FROM scenes WHERE id='sc1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(scene_title, "第一百三十四章 魑魅魍魉横行");
        // 多场章节里的具名场景标题不被章节标题覆盖。
        let sc2_title: String = conn
            .query_row("SELECT title FROM scenes WHERE id='sc2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sc2_title, "第一场 雨夜叩门");
    }
}
