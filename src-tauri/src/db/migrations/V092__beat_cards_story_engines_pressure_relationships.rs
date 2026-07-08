use rusqlite::Connection;

use crate::db::migrations::RustMigration;

pub struct Migration;

impl RustMigration for Migration {
    fn version(&self) -> i32 {
        92
    }

    fn description(&self) -> &'static str {
        "beat cards story engines pressure relationships"
    }

    fn apply(&self, conn: &mut Connection) -> Result<(), rusqlite::Error> {
        // genre_profiles 新增 reader_promise 字段（爽 / 甜 / 虐 / 恨 / 惊 /
        // 燃 / 怕 / 痛 / 治愈 等读者主情绪承诺，用于注入 Writer prompt）
        let gp_cols_92: Vec<String> = conn
            .prepare("PRAGMA table_info(genre_profiles)")?
            .query_map([], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if !gp_cols_92.iter().any(|c| c == "reader_promise") {
            conn.execute(
                "ALTER TABLE genre_profiles ADD COLUMN reader_promise TEXT",
                [],
            )?;
        }

        // 桥段卡表（beat_cards）—— 经典叙事功能模板，可被 outline / writer 引用
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS beat_cards (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL UNIQUE,
            category        TEXT NOT NULL,
            function        TEXT NOT NULL,
            when_to_use     TEXT NOT NULL,
            remix_hint      TEXT,
            avoid           TEXT,
            tags_json       TEXT,
            is_builtin      INTEGER NOT NULL DEFAULT 0,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_beat_cards_category
            ON beat_cards(category);
        CREATE INDEX IF NOT EXISTS idx_beat_cards_builtin
            ON beat_cards(is_builtin);
        ",
        )?;

        // 剧情引擎表（story_engines）—— 21 种正交叙事引擎，可组合 2-4 个
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS story_engines (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL UNIQUE,
            payoff          TEXT NOT NULL,
            best_payoff     TEXT,
            avoid           TEXT,
            pairs_well_with TEXT,
            tags_json       TEXT,
            is_builtin      INTEGER NOT NULL DEFAULT 0,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_story_engines_builtin
            ON story_engines(is_builtin);
        ",
        )?;

        // 高压关系表（pressure_relationships）—— 13 种角色对位关系
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pressure_relationships (
            id              TEXT PRIMARY KEY,
            name            TEXT NOT NULL UNIQUE,
            pressure_source TEXT NOT NULL,
            works_with      TEXT,
            tags_json       TEXT,
            is_builtin      INTEGER NOT NULL DEFAULT 0,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_pressure_relationships_builtin
            ON pressure_relationships(is_builtin);
        ",
        )?;

        Ok(())
    }
}
