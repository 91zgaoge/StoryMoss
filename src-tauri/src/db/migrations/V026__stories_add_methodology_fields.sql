-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

ALTER TABLE stories ADD COLUMN methodology_id TEXT;
ALTER TABLE stories ADD COLUMN methodology_step INTEGER;
