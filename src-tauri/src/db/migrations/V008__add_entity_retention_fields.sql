-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

ALTER TABLE kg_entities ADD COLUMN confidence_score REAL;
ALTER TABLE kg_entities ADD COLUMN access_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE kg_entities ADD COLUMN last_accessed TEXT;
