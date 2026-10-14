//! V147：每拍手法落库（作者裁决回灌手法池的归因基底，P1-5，v0.69.0）。
//!
//! 缘起 docs/audits/2026-10-14-sherpa-benchmark-analysis.md：Sherpa 用行为数据
//! 训练散文引擎；StoryMoss 的对位是把作者裁决（Tab 接受 / Esc 放弃）变成每拍
//! 决策的权重。要归因就必须先知道「哪一拍用了哪条手法」——本表每拍一行。
//! 反馈侧（`user_feedback_log`）不带 scene_id，归因按时间窗在应用层完成
//! （见 `agency::technique_feedback`）。
//!
//! 幂等：表存在即跳过。

use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        147
    }

    fn description(&self) -> &'static str {
        "beat_technique_log (per-beat technique attribution for author-verdict replay)"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS beat_technique_log (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                scene_id TEXT,
                chapter_number INTEGER NOT NULL,
                beat_no INTEGER NOT NULL,
                technique_id TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_beat_technique_story_time \
             ON beat_technique_log(story_id, created_at)",
            [],
        )?;
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_beat_technique_story_scene \
             ON beat_technique_log(story_id, scene_id)",
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
    fn v147_creates_beat_technique_log_idempotently() {
        let pool = create_test_pool().unwrap();
        let mut conn = pool.get().unwrap();
        Migration.apply(&mut conn).unwrap();
        Migration.apply(&mut conn).unwrap();
        let has: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' \
                 AND name = 'beat_technique_log'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(has, 1);
    }
}
