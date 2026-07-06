-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

ALTER TABLE kg_entities ADD COLUMN is_archived INTEGER NOT NULL DEFAULT 0;
ALTER TABLE kg_entities ADD COLUMN archived_at TEXT;
CREATE INDEX IF NOT EXISTS idx_kg_entities_archived ON kg_entities(is_archived);
