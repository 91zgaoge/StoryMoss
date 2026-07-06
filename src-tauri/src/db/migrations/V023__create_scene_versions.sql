-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE scene_versions (
                    id TEXT PRIMARY KEY,
                    scene_id TEXT NOT NULL,
                    version_number INTEGER NOT NULL,
                    title TEXT,
                    content TEXT,
                    dramatic_goal TEXT,
                    external_pressure TEXT,
                    conflict_type TEXT,
                    characters_present TEXT,
                    character_conflicts TEXT,
                    setting_location TEXT,
                    setting_time TEXT,
                    setting_atmosphere TEXT,
                    word_count INTEGER,
                    change_summary TEXT NOT NULL,
                    created_by TEXT NOT NULL,
                    model_used TEXT,
                    confidence_score REAL,
                    previous_version_id TEXT,
                    superseded_by TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                    FOREIGN KEY (previous_version_id) REFERENCES scene_versions(id),
                    FOREIGN KEY (superseded_by) REFERENCES scene_versions(id)
                );
CREATE INDEX idx_scene_versions_scene ON scene_versions(scene_id);
