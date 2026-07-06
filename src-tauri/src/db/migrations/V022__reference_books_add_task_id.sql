-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

ALTER TABLE reference_books ADD COLUMN task_id TEXT;
CREATE INDEX idx_ref_books_task ON reference_books(task_id);
