-- RUST_LOGIC: This migration contained conditional Rust logic for idempotency checks.
-- The SQL statements below are the unconditional equivalents.

CREATE TABLE subscriptions (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    tier TEXT NOT NULL DEFAULT 'free',
                    status TEXT NOT NULL DEFAULT 'active',
                    started_at TEXT NOT NULL,
                    expires_at TEXT,
                    payment_provider TEXT,
                    payment_id TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
CREATE INDEX idx_subscriptions_user ON subscriptions(user_id);
CREATE INDEX idx_subscriptions_tier ON subscriptions(tier);
