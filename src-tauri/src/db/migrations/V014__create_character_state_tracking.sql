-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE character_states (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    character_id TEXT NOT NULL,
                    current_location TEXT,
                    current_emotion TEXT,
                    active_goal TEXT,
                    secrets_known TEXT,
                    secrets_unknown TEXT,
                    arc_progress REAL,
                    last_updated TEXT,
                    FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
                );
CREATE INDEX idx_character_states_story ON character_states(story_id);
CREATE INDEX idx_character_states_character ON character_states(character_id);
