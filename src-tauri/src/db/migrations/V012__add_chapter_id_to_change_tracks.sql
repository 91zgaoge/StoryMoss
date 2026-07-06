-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

ALTER TABLE change_tracks ADD COLUMN chapter_id TEXT REFERENCES chapters(id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS idx_change_tracks_chapter ON change_tracks(chapter_id);
