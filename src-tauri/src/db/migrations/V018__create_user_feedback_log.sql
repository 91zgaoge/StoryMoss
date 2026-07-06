-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE user_feedback_log (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    scene_id TEXT,
                    chapter_id TEXT,
                    feedback_type TEXT NOT NULL,    -- accept / reject / modify
                    agent_type TEXT,                -- writer / inspector / etc
                    original_ai_text TEXT,          -- AI 生成的原始文本
                    final_text TEXT,                -- 用户最终接受的文本
                    ai_score REAL,                  -- AI 自评分数
                    user_satisfaction INTEGER,      -- 用户满意度 1-5（如提供）
                    metadata TEXT,                  -- JSON: 额外上下文
                    created_at TEXT NOT NULL
                );
CREATE INDEX idx_feedback_story ON user_feedback_log(story_id);
CREATE INDEX idx_feedback_type ON user_feedback_log(feedback_type);
CREATE INDEX idx_feedback_created ON user_feedback_log(created_at);
