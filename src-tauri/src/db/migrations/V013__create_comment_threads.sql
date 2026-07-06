-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE comment_threads (
                    id TEXT PRIMARY KEY,
                    scene_id TEXT,
                    chapter_id TEXT,
                    version_id TEXT,
                    anchor_type TEXT NOT NULL,
                    from_pos INTEGER,
                    to_pos INTEGER,
                    selected_text TEXT,
                    status TEXT NOT NULL DEFAULT 'Open',
                    created_at TEXT NOT NULL,
                    resolved_at TEXT,
                    FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE,
                    FOREIGN KEY (chapter_id) REFERENCES chapters(id) ON DELETE CASCADE,
                    FOREIGN KEY (version_id) REFERENCES scene_versions(id) ON DELETE CASCADE
                );
CREATE INDEX idx_comment_threads_scene ON comment_threads(scene_id);
CREATE INDEX idx_comment_threads_chapter ON comment_threads(chapter_id);
CREATE TABLE comment_messages (
                    id TEXT PRIMARY KEY,
                    thread_id TEXT NOT NULL,
                    author_id TEXT NOT NULL,
                    author_name TEXT,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY (thread_id) REFERENCES comment_threads(id) ON DELETE CASCADE
                );
CREATE INDEX idx_comment_messages_thread ON comment_messages(thread_id);
