use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        96
    }

    fn description(&self) -> &'static str {
        "genre profiles recommended assets"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        // 检查表是否存在（迁移 52 可能因版本跳跃未执行）
        let table_exists: bool = conn
            .prepare(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='genre_profiles'",
            )?
            .query_row([], |row| row.get::<_, i32>(0))
            .map(|count| count > 0)
            .unwrap_or(false);

        if table_exists {
            let genre_cols: Vec<String> = conn
                .prepare("PRAGMA table_info(genre_profiles)")?
                .query_map([], |row| {
                    let name: String = row.get(1)?;
                    Ok(name)
                })?
                .collect::<Result<Vec<_>, _>>()?;

            if !genre_cols.contains(&"recommended_style_dna_ids".to_string()) {
                conn.execute(
                    "ALTER TABLE genre_profiles ADD COLUMN recommended_style_dna_ids TEXT",
                    [],
                )?;
            }
            if !genre_cols.contains(&"recommended_methodology_id".to_string()) {
                conn.execute(
                    "ALTER TABLE genre_profiles ADD COLUMN recommended_methodology_id TEXT",
                    [],
                )?;
            }
            if !genre_cols.contains(&"recommended_skill_ids".to_string()) {
                conn.execute(
                    "ALTER TABLE genre_profiles ADD COLUMN recommended_skill_ids TEXT",
                    [],
                )?;
            }
            if !genre_cols.contains(&"min_quality_tier".to_string()) {
                conn.execute(
                    "ALTER TABLE genre_profiles ADD COLUMN min_quality_tier TEXT DEFAULT 'low'",
                    [],
                )?;
            }
        }
        Ok(())
    }
}
