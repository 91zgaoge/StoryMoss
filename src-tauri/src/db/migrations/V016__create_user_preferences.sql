-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE user_preferences (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    preference_type TEXT,
                    preference_key TEXT,
                    preference_value TEXT,
                    confidence REAL,
                    evidence_count INTEGER,
                    updated_at TEXT
                );
CREATE INDEX idx_user_preferences_story ON user_preferences(story_id);
