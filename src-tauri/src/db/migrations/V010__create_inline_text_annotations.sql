-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE text_annotations (
                    id TEXT PRIMARY KEY,
                    story_id TEXT NOT NULL,
                    scene_id TEXT,
                    chapter_id TEXT,
                    content TEXT NOT NULL,
                    annotation_type TEXT NOT NULL DEFAULT 'note',
                    from_pos INTEGER NOT NULL,
                    to_pos INTEGER NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    resolved_at TEXT,
                    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
                );
CREATE INDEX idx_text_annotations_story ON text_annotations(story_id);
CREATE INDEX idx_text_annotations_scene ON text_annotations(scene_id);
