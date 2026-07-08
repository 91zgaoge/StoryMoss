use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        83
    }

    fn description(&self) -> &'static str {
        "drop redundant litseg tables"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        // 删除 narrative_events 表（功能已合并到 scenes 表）
        conn.execute("DROP TABLE IF EXISTS narrative_events", [])?;
        // 删除 narrative_threads 表（功能已拆分到
        // foreshadowing_tracker/character_states/conflict_escalations）
        conn.execute("DROP TABLE IF EXISTS narrative_threads", [])?;
        // 删除 narrative_structure 表（功能已合并到
        // story_outlines.analyzed_structure_json）
        conn.execute("DROP TABLE IF EXISTS narrative_structure", [])?;
        // 清理相关索引
        conn.execute("DROP INDEX IF EXISTS idx_narrative_events_story", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_events_chapter", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_events_type", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_threads_story", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_threads_type", [])?;
        conn.execute("DROP INDEX IF EXISTS idx_narrative_structure_story", [])?;
        Ok(())
    }
}
