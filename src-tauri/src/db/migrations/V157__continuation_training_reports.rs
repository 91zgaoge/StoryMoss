use rusqlite::{params, Connection};

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        157
    }

    fn description(&self) -> &'static str {
        "continuation training reports: compare generated chapters against ground-truth chapters"
    }

    /// v0.88.0：续写训练定时任务（取原书若干章 → 灌沙盒 → 续写 → 与真实
    /// 后续章对比打分）的报告表。每轮任务一行报告；下一轮执行时读取最近
    /// 报告的 improvement 建议，注入续写指令形成自动闭环。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS continuation_training_reports (\
             id TEXT PRIMARY KEY,\
             task_id TEXT NOT NULL,\
             book_title TEXT NOT NULL,\
             source_kind TEXT NOT NULL,\
             story_id TEXT NOT NULL,\
             run_id TEXT,\
             start_chapter INTEGER NOT NULL,\
             context_chapters INTEGER NOT NULL,\
             generated_chapter INTEGER,\
             truth_content TEXT,\
             scores_json TEXT,\
             report_md TEXT NOT NULL,\
             improvement_md TEXT,\
             style_dna_applied INTEGER NOT NULL DEFAULT 0,\
             created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')))",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_training_reports_book \
             ON continuation_training_reports(book_title, created_at DESC)",
            [],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_table_and_index() {
        let mut conn = Connection::open_in_memory().unwrap();
        Migration.apply(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO continuation_training_reports \
             (id, task_id, book_title, source_kind, story_id, start_chapter, context_chapters, report_md) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params!["r1", "t1", "阴脉先生", "mcp", "s1", 10, 10, "报告"],
        )
        .unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM continuation_training_reports",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }
}
