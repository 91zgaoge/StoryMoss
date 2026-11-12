use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        159
    }

    fn description(&self) -> &'static str {
        "editor_gate_records.run_id: 让 gate 裁决成为 run QC 的单一权威"
    }

    /// v0.97.0：gate 裁决记录原先只有 story_id/chapter_number，无法回答
    /// “这次 run 的最终 verdict 是什么”。run.result.verdict 是第二份副本，
    /// 后台 QC 完成时可能仍是 pending。这里给记录加 run_id，使它成为
    /// run QC 的单一权威：finish_run / QC 收尾都从这里回填 run.result。
    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let has_run_id: bool = {
            let mut stmt = conn.prepare("PRAGMA table_info(editor_gate_records)")?;
            let mut rows = stmt.query([])?;
            let mut found = false;
            while let Some(row) = rows.next()? {
                let name: String = row.get(1)?;
                if name == "run_id" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_run_id {
            conn.execute("ALTER TABLE editor_gate_records ADD COLUMN run_id TEXT", [])?;
        }
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_editor_gate_records_run \
             ON editor_gate_records(run_id, chapter_number, created_at DESC)",
            [],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_run_id_and_index_idempotently() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE editor_gate_records (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                chapter_number INTEGER NOT NULL,
                round INTEGER NOT NULL,
                outcome TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
             );",
        )
        .unwrap();
        Migration.apply(&mut conn).unwrap();
        // 第二次 apply 不应因 column exists 报错（PRAGMA 检查）。
        Migration.apply(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO editor_gate_records \
             (id, story_id, chapter_number, round, outcome, run_id) \
             VALUES ('g1','s1',1,1,'pass','r1')",
            [],
        )
        .unwrap();
        let run_id: String = conn
            .query_row(
                "SELECT run_id FROM editor_gate_records WHERE id='g1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(run_id, "r1");
    }
}
