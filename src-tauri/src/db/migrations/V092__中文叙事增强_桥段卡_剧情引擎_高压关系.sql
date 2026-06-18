-- v0.17.0 中文叙事增强 —— 桥段卡 / 剧情引擎 / 高压关系三类新资产 + reader_promise 字段。
--
-- 注意：genre_profiles.reader_promise 字段的 ALTER TABLE 已由 connection.rs 中的
-- Migration 92 Rust 逻辑处理（含列存在性检查），此处不重复 ALTER 以避免 SQLite
-- "duplicate column name" 错误，也兼容测试中 create_test_pool 仅运行 .sql 文件
-- 但未创建 genre_profiles 表的场景。

-- 1) 桥段卡表（30+ 经典叙事功能模板，可被 outline / writer 引用）
CREATE TABLE IF NOT EXISTS beat_cards (
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
CREATE INDEX IF NOT EXISTS idx_beat_cards_category ON beat_cards(category);
CREATE INDEX IF NOT EXISTS idx_beat_cards_builtin ON beat_cards(is_builtin);

-- 2) 剧情引擎表（21 种正交叙事引擎，可组合 2-4 个）
CREATE TABLE IF NOT EXISTS story_engines (
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
CREATE INDEX IF NOT EXISTS idx_story_engines_builtin ON story_engines(is_builtin);

-- 3) 高压关系表（13 种角色对位关系）
CREATE TABLE IF NOT EXISTS pressure_relationships (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    pressure_source TEXT NOT NULL,
    works_with      TEXT,
    tags_json       TEXT,
    is_builtin      INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_pressure_relationships_builtin ON pressure_relationships(is_builtin);
