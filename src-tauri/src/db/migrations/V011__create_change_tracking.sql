-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE change_tracks (
                    id TEXT PRIMARY KEY,
                    scene_id TEXT,
                    chapter_id TEXT,
                    version_id TEXT,
                    author_id TEXT NOT NULL,
                    author_name TEXT,
                    change_type TEXT NOT NULL,
                    from_pos INTEGER NOT NULL,
                    to_pos INTEGER NOT NULL,
                    content TEXT,
                    status TEXT NOT NULL DEFAULT 'Pending',
                    created_at TEXT NOT NULL,
                    resolved_at TEXT,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE,
                    FOREIGN KEY (version_id) REFERENCES scene_versions(id) ON DELETE CASCADE
                );
CREATE INDEX idx_change_tracks_scene ON change_tracks(scene_id);
CREATE INDEX idx_change_tracks_chapter ON change_tracks(chapter_id);
CREATE INDEX idx_change_tracks_status ON change_tracks(status);
