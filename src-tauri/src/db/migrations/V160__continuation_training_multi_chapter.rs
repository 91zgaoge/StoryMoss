use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        160
    }

    fn description(&self) -> &'static str {
        "continuation training multi-chapter: continuation_chapters + per-chapter reviews/truth"
    }

    /// v0.98.0：续写训练从「续写 1 章 vs 1 章基准」升级为「连续续写 N 章
    /// （默认 10）vs 原书后 N 章」，报告需要保存逐章编审结果与真实基准章。
    /// 列补齐后旧报告仍以 continuation_chapters=1 读取（趋势/历史兼容）。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        for (column, definition) in [
            (
                "continuation_chapters",
                "continuation_chapters INTEGER NOT NULL DEFAULT 1",
            ),
            ("truth_chapters_json", "truth_chapters_json TEXT"),
            ("chapter_reviews_json", "chapter_reviews_json TEXT"),
        ] {
            let exists: bool = {
                let mut stmt = conn.prepare("PRAGMA table_info(continuation_training_reports)")?;
                let mut rows = stmt.query([])?;
                let mut found = false;
                while let Some(row) = rows.next()? {
                    let name: String = row.get(1)?;
                    if name == column {
                        found = true;
                        break;
                    }
                }
                found
            };
            if !exists {
                conn.execute(
                    &format!("ALTER TABLE continuation_training_reports ADD COLUMN {definition}"),
                    [],
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::V157__continuation_training_reports;

    #[test]
    fn adds_multi_chapter_columns_idempotently() {
        let mut conn = Connection::open_in_memory().unwrap();
        V157__continuation_training_reports::Migration
            .apply(&mut conn)
            .unwrap();
        // 再 apply 两次不应因列已存在报错（PRAGMA 检查）。
        Migration.apply(&mut conn).unwrap();
        Migration.apply(&mut conn).unwrap();
        // 旧报告不带 continuation_chapters 时默认 1（历史数据兼容）。
        conn.execute(
            "INSERT INTO continuation_training_reports \
             (id, task_id, book_title, source_kind, story_id, start_chapter, context_chapters, \
              report_md) VALUES ('old','t1','阴脉先生','mcp','s1',10,10,'旧报告')",
            [],
        )
        .unwrap();
        let continuation: i64 = conn
            .query_row(
                "SELECT continuation_chapters FROM continuation_training_reports WHERE id='old'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(continuation, 1);
        conn.execute(
            "INSERT INTO continuation_training_reports \
             (id, task_id, book_title, source_kind, story_id, start_chapter, context_chapters, \
              continuation_chapters, chapter_reviews_json, truth_chapters_json, report_md) \
             VALUES ('r1','t1','阴脉先生','mcp','s1',10,10,10,'[]','[]','报告')",
            [],
        )
        .unwrap();
        let reviews: String = conn
            .query_row(
                "SELECT chapter_reviews_json FROM continuation_training_reports WHERE id='r1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reviews, "[]");
    }
}
