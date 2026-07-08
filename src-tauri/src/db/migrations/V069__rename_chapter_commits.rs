use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        69
    }

    fn description(&self) -> &'static str {
        "rename chapter commits"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        let has_old_table: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='chapter_commits'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
            > 0;

        if has_old_table {
            // 如果 Migration 48 已经创建了空的 scene_commits（旧数据库升级场景），先删除它
            let has_new_table: bool = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND \
                 name='scene_commits'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0)
                > 0;
            if has_new_table {
                conn.execute("DROP TABLE scene_commits", [])?;
            }
            conn.execute("ALTER TABLE chapter_commits RENAME TO scene_commits", [])?;
            // SQLite RENAME TABLE 会自动更新大部分索引引用，
            // 但含有旧表名的索引名需要删除后重建
            conn.execute("DROP INDEX IF EXISTS idx_chapter_commits_story", [])?;
            conn.execute("DROP INDEX IF EXISTS idx_chapter_commits_scene", [])?;
            conn.execute("DROP INDEX IF EXISTS idx_chapter_commits_number", [])?;
            conn.execute("DROP INDEX IF EXISTS idx_chapter_commits_chapter", [])?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_scene_commits_story ON scene_commits(story_id)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_scene_commits_scene ON scene_commits(scene_id)",
                [],
            )?;
            conn.execute(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_scene_commits_number ON \
             scene_commits(story_id, chapter_number)",
                [],
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_scene_commits_chapter ON scene_commits(chapter_id)",
                [],
            )?;
        }
        Ok(())
    }
}
