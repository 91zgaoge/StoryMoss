-- V107: Agency 多代理框架核心表（创世 2.0）
CREATE TABLE IF NOT EXISTS agency_runs (
    id TEXT PRIMARY KEY,
    story_id TEXT,
    premise TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    phase TEXT NOT NULL DEFAULT 'concept',
    result_json TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agency_runs_story ON agency_runs(story_id);
CREATE INDEX IF NOT EXISTS idx_agency_runs_status ON agency_runs(status);

CREATE TABLE IF NOT EXISTS agency_board_items (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    story_id TEXT NOT NULL,
    zone TEXT NOT NULL,
    item_type TEXT NOT NULL,
    key TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    summary TEXT NOT NULL DEFAULT '',
    version INTEGER NOT NULL DEFAULT 1,
    producer TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agency_board_run_zone ON agency_board_items(run_id, zone);
CREATE INDEX IF NOT EXISTS idx_agency_board_run_key ON agency_board_items(run_id, zone, key);

CREATE TABLE IF NOT EXISTS agency_messages (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    from_role TEXT NOT NULL,
    to_role TEXT NOT NULL,
    msg_type TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agency_messages_run ON agency_messages(run_id, to_role);
