-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

ALTER TABLE foreshadowing_tracker ADD COLUMN target_start_scene INTEGER;
ALTER TABLE foreshadowing_tracker ADD COLUMN target_end_scene INTEGER;
ALTER TABLE foreshadowing_tracker ADD COLUMN risk_signals TEXT;
ALTER TABLE foreshadowing_tracker ADD COLUMN scope_type TEXT DEFAULT 'story';
ALTER TABLE foreshadowing_tracker ADD COLUMN ledger_key TEXT;
CREATE UNIQUE INDEX IF NOT EXISTS idx_foreshadowing_ledger_key ON foreshadowing_tracker(ledger_key);
