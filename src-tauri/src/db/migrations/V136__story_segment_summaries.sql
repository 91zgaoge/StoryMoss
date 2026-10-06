-- V136: 分层记忆金字塔（P1 阶段，v0.61.0）。
--
-- 背景：docs/plans/2026-10-06-p0-p3-roadmap-implementation.md 阶段 P1。
-- 此前只有「每章前 1000 字截断」的章节摘要，长篇的中远期情节只能靠向量检索
-- 概率召回。本迁移引入段级与全书级滚动摘要：
--   * level='segment'：每 SEGMENT_SIZE（10）章生成一条，覆盖 [start_chapter, end_chapter]
--   * level='book'：由全部段摘要再压缩出一条全书纲要（segment_index 恒为 0）
-- 三层（章 → 段 → 全书）配合自适应窗口注入续写上下文，给长篇一个确定性的
-- 远期纲要，而不是只靠检索。
-- 纯 additive。
CREATE TABLE IF NOT EXISTS story_segment_summaries (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    level TEXT NOT NULL DEFAULT 'segment',
    segment_index INTEGER NOT NULL,
    start_chapter INTEGER,
    end_chapter INTEGER,
    summary TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'llm',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(story_id, level, segment_index),
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_story_segment_summaries_story
    ON story_segment_summaries(story_id, level, segment_index);
