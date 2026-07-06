-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE style_dnas (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    author TEXT,
                    dna_json TEXT NOT NULL,
                    is_builtin INTEGER NOT NULL DEFAULT 0,
                    is_user_created INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL
                );
CREATE INDEX idx_style_dnas_builtin ON style_dnas(is_builtin);
