-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE ai_usage_logs (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    story_id TEXT,
                    chapter_id TEXT,
                    agent_type TEXT NOT NULL,
                    instruction TEXT,
                    prompt_tokens INTEGER,
                    completion_tokens INTEGER,
                    model_used TEXT,
                    cost REAL,
                    duration_ms INTEGER,
                    tier_at_time TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
CREATE INDEX idx_usage_logs_user ON ai_usage_logs(user_id);
CREATE INDEX idx_usage_logs_created ON ai_usage_logs(created_at);
