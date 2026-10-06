-- V138: P3 阶段（v0.63.0）工程纪律——质量债台账、故事检查点、待确认队列。
--
-- 1) quality_debts（P3-A）：质检 fail-open 降级（salvage）或存在未解决问题时入账，
--    避免「降级放行」变成「静默消失」。含建议回收窗口，作者可在后续版本批量处理。
-- 2) story_checkpoints（P3-C）：每 10 章（与段摘要同频）写一份连续性资产快照，
--    支撑「某章时点」回溯与事故恢复。
-- 3) pending_reviews（P3-B）：ingest 自动新增的「规则类」资产（世界观硬规则等）
--    进入待确认队列，未经作者确认不作为硬约束使用。
-- 全部 additive。
CREATE TABLE IF NOT EXISTS quality_debts (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    scene_id TEXT,
    chapter_number INTEGER,
    source TEXT NOT NULL DEFAULT 'editor_qc',
    severity TEXT NOT NULL DEFAULT 'warning',
    detail TEXT NOT NULL,
    suggested_window TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(story_id, chapter_number, detail),
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_quality_debts_story
    ON quality_debts(story_id, status, created_at DESC);

CREATE TABLE IF NOT EXISTS story_checkpoints (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    chapter_number INTEGER NOT NULL,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(story_id, chapter_number),
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_story_checkpoints_story
    ON story_checkpoints(story_id, chapter_number DESC);

CREATE TABLE IF NOT EXISTS pending_reviews (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'world_rule',
    subject TEXT NOT NULL,
    detail TEXT,
    source TEXT NOT NULL DEFAULT 'ingest',
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(story_id, kind, subject),
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_pending_reviews_story
    ON pending_reviews(story_id, status);
