-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE scene_annotations (
                        id TEXT PRIMARY KEY,
                        scene_id TEXT NOT NULL,
                        story_id TEXT NOT NULL,
                        content TEXT NOT NULL,
                        annotation_type TEXT NOT NULL DEFAULT 'note',
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL,
                        resolved_at TEXT,
                        FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                        FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                    );
CREATE INDEX idx_scene_annotations_scene ON scene_annotations(scene_id);
CREATE INDEX idx_scene_annotations_story ON scene_annotations(story_id);
