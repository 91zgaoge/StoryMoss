-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

ALTER TABLE scenes ADD COLUMN confidence_score REAL;
ALTER TABLE kg_relations ADD COLUMN confidence_score REAL;
