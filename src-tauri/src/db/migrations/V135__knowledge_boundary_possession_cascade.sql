-- V135: 三把尺子 —— 知识边界（三层信息分离）、物品归属账本、级联影响报告。
--
-- 背景：docs/plans/2026-10-06-p0-p3-roadmap-implementation.md 阶段 P0。
-- 语义约定：
--  * story_timeline_events 每条事件同时记录「世界真相 objective_fact」与
--    「读者此刻认知 reader_knowledge」，并以 reveal_status 状态机
--    （hidden / partial / revealed）追踪揭示进度。hidden 事件的 objective_fact
--    绝不能泄漏进正文（泄密探针据此判定）。
--  * character_knowledge_log 是角色知情的 append-only 审计流水，
--    支撑「谁在什么场景知道了什么」的复核与后续时间旅行查询。
--  * item_holdings 只登记跨章影响行动边界的关键资源及其持有者（玉佩账本），
--    遵循 ani-book-skill「个人短期状态回写角色档案，不建账本」的克制原则。
--  * cascade_impacts 是改稿级联影响报告：一个 batch_id = 一次改稿分析，
--    每行 = 一个受影响的目标场景；severity 由确定性打分或 LLM 冲突扫描给出；
--    decision 承载作者的三条动作（忽略 / 已请求改写 / 待处理）；
--    stale_flag 标记「该目标章的分析可能已失效」。
-- 全部为 additive 迁移，不改动既有表结构。

CREATE TABLE IF NOT EXISTS story_timeline_events (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    chapter_number INTEGER,
    scene_id TEXT,
    sequence_number INTEGER,
    objective_fact TEXT NOT NULL,
    reader_knowledge TEXT,
    reveal_status TEXT NOT NULL DEFAULT 'hidden',
    reveal_chapter INTEGER,
    participants TEXT NOT NULL DEFAULT '[]',
    source TEXT NOT NULL DEFAULT 'ingest',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_story_timeline_events_story ON story_timeline_events(story_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_story_timeline_events_reveal ON story_timeline_events(story_id, reveal_status);

CREATE TABLE IF NOT EXISTS character_knowledge_log (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    character_id TEXT NOT NULL,
    fact TEXT NOT NULL,
    change_type TEXT NOT NULL DEFAULT 'learned',
    source_scene_id TEXT,
    chapter_number INTEGER,
    evidence TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_character_knowledge_log_char ON character_knowledge_log(story_id, character_id, chapter_number);

CREATE TABLE IF NOT EXISTS item_holdings (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    item_name TEXT NOT NULL,
    item_entity_id TEXT,
    holder_name TEXT,
    holder_character_id TEXT,
    status TEXT NOT NULL DEFAULT 'held',
    acquired_chapter INTEGER,
    evidence TEXT,
    source_scene_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_item_holdings_story_item ON item_holdings(story_id, item_name);

CREATE TABLE IF NOT EXISTS cascade_impacts (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    batch_id TEXT NOT NULL,
    source_scene_id TEXT NOT NULL,
    source_chapter_number INTEGER,
    target_scene_id TEXT NOT NULL,
    target_chapter_number INTEGER,
    impact_score REAL NOT NULL DEFAULT 0,
    impact_kind TEXT NOT NULL DEFAULT 'mention',
    severity TEXT NOT NULL DEFAULT 'info',
    entity_ids TEXT NOT NULL DEFAULT '[]',
    detail TEXT,
    evidence TEXT,
    decision TEXT NOT NULL DEFAULT 'open',
    stale_flag INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(batch_id, target_scene_id),
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_cascade_impacts_story ON cascade_impacts(story_id, decision, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_cascade_impacts_target ON cascade_impacts(target_scene_id);
