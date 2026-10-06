-- V137: P2 阶段（v0.62.0）——伏笔增强与作者风格偏好。
--
-- 1) foreshadowing_tracker 增强（P2-C）：
--    * evidence：埋设时的原文证据句（供「证据锚定」展示与复核）；
--    * strength / subtlety：强度与隐藏度（1-10，默认 5），供回收排序与提醒策略；
--    * related_foreshadow_ids：伏笔链（JSON 数组，长线伏笔的分次回收/呼应关系）。
-- 2) style_preferences（P2-B）：作者手改 AI 文本时逆向提炼的文风偏好，
--    按故事累积，注入续写上下文，让「越改越像你自己」。
-- 全部 additive。
ALTER TABLE foreshadowing_tracker ADD COLUMN evidence TEXT;
ALTER TABLE foreshadowing_tracker ADD COLUMN strength INTEGER DEFAULT 5;
ALTER TABLE foreshadowing_tracker ADD COLUMN subtlety INTEGER DEFAULT 5;
ALTER TABLE foreshadowing_tracker ADD COLUMN related_foreshadow_ids TEXT DEFAULT '[]';

CREATE TABLE IF NOT EXISTS style_preferences (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    pattern TEXT NOT NULL,
    evidence TEXT,
    source TEXT NOT NULL DEFAULT 'user_edit',
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(story_id, pattern),
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_style_preferences_story
    ON style_preferences(story_id, status);
