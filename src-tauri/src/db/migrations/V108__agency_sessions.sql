-- V108: Agency 会话快照（记忆持久性）
CREATE TABLE IF NOT EXISTS agency_sessions (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    story_id TEXT,
    phase TEXT NOT NULL,
    snapshot_json TEXT NOT NULL DEFAULT '{}',
    summary TEXT,
    kind TEXT NOT NULL DEFAULT 'auto',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agency_sessions_run ON agency_sessions(run_id, created_at);
CREATE INDEX IF NOT EXISTS idx_agency_sessions_story ON agency_sessions(story_id, created_at);
