use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        70
    }

    fn description(&self) -> &'static str {
        "drop chapters scene id"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let chapter_columns_m71: Vec<String> = conn
            .prepare("PRAGMA table_info(chapters)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        if chapter_columns_m71.iter().any(|c| c == "scene_id") {
            // 必须先删除索引，再删除列（SQLite DROP COLUMN 不能删除有索引的列）
            // 使用显式事务包裹，避免 SQLite schema 缓存导致 drop column
            // 时仍能看到已删除的索引
            let tx = conn.transaction()?;
            tx.execute("DROP INDEX IF EXISTS idx_chapters_scene", [])?;
            tx.execute("ALTER TABLE chapters DROP COLUMN scene_id", [])?;
            tx.commit()?;
        }
        Ok(())
    }
}
