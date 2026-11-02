-- v0.80.1: structured scene outcomes for progress feedback and next-beat input.
CREATE TABLE IF NOT EXISTS scene_outcomes (
    scene_id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    chapter_number INTEGER NOT NULL,
    outcome TEXT NOT NULL,
    state_delta_json TEXT NOT NULL,
    emotion_json TEXT,
    open_question TEXT NOT NULL,
    contract_misses_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_scene_outcomes_story
    ON scene_outcomes (story_id, chapter_number DESC);
