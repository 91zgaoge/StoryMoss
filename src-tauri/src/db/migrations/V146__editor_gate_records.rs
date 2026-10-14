//! V146：编辑审计的 gate 裁决记录（判官一致率的数据源，P1-4，v0.68.0）。
//!
//! 缘起 docs/audits/2026-10-14-sherpa-benchmark-analysis.md：Sherpa 用「与人类
//! 审阅者 80–90% 匹配率」证明其判官可用；StoryMoss 的 editor 裁决（含
//! `blocking_issues[].evidence` 原文引用）此前在 gate 处被压平成字符串（只进
//! 黑板与 llm_calls 文本），没有可查询的判定数据源——一致率无从计算。本表每次
//! gate 判定落一行：findings_json 原样保留 `{issue, evidence, impact, fix}`，
//! 供「引用是否在被作者改过的成稿里幸存」这一确定性判定消费。
//!
//! 幂等：表存在即跳过。

use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        146
    }

    fn description(&self) -> &'static str {
        "editor_gate_records (judge agreement: per-gate verdict with evidence findings)"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS editor_gate_records (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                chapter_number INTEGER NOT NULL,
                round INTEGER NOT NULL DEFAULT 1,
                outcome TEXT NOT NULL,
                verdict TEXT,
                score REAL,
                weighted REAL,
                dimension_scores_json TEXT,
                findings_json TEXT,
                source TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_editor_gate_records_story_chapter \
             ON editor_gate_records(story_id, chapter_number)",
            [],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    #[test]
    fn v146_creates_editor_gate_records_idempotently() {
        let pool = create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();
        Migration.apply(&mut conn).unwrap();
        let has: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' \
                 AND name = 'editor_gate_records'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(has, 1);
    }
}
