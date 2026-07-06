-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE foreshadowing_tracker (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    content TEXT NOT NULL,
                    setup_scene_id TEXT,
                    payoff_scene_id TEXT,
                    status TEXT NOT NULL DEFAULT 'setup',
                    importance INTEGER,
                    created_at TEXT NOT NULL,
                    resolved_at TEXT
                );
CREATE INDEX idx_foreshadowing_story ON foreshadowing_tracker(story_id);
CREATE INDEX idx_foreshadowing_status ON foreshadowing_tracker(status);
