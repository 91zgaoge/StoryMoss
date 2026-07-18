-- V110: Agency 检查点（里程碑指标快照）
CREATE TABLE IF NOT EXISTS agency_checkpoints (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    story_id TEXT NOT NULL,
    milestone TEXT NOT NULL,
    chapter_number INTEGER,
    metrics_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agency_checkpoints_story ON agency_checkpoints(story_id, created_at);
CREATE INDEX IF NOT EXISTS idx_agency_checkpoints_run ON agency_checkpoints(run_id, created_at);
