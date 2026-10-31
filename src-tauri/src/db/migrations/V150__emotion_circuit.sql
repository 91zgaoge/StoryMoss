-- V150：情感回路（Story Emotion Circuit）持久化。
-- 设计：docs/plans/2026-10-31-story-emotion-circuit-design.md
-- 全部 additive；旧故事不写入也能按原路径运行。

CREATE TABLE IF NOT EXISTS story_emotion_contracts (
  story_id TEXT PRIMARY KEY,
  core_question TEXT,
  reader_promise TEXT,
  palette_json TEXT NOT NULL DEFAULT '{}',
  ending_residue TEXT,
  forbidden_json TEXT NOT NULL DEFAULT '[]',
  peak_budget INTEGER NOT NULL DEFAULT 1,
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS character_emotion_states (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  character_id TEXT NOT NULL,
  scene_id TEXT,
  chapter_number INTEGER,
  beat_index INTEGER,
  vector_json TEXT NOT NULL,
  intensity REAL NOT NULL DEFAULT 0.5,
  active_goal TEXT,
  defense TEXT,
  cause_event TEXT,
  source TEXT NOT NULL DEFAULT 'ingest',
  created_at TEXT NOT NULL,
  FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_character_emotion_states_story
  ON character_emotion_states(story_id, chapter_number);
CREATE INDEX IF NOT EXISTS idx_character_emotion_states_character
  ON character_emotion_states(story_id, character_id, chapter_number);

CREATE TABLE IF NOT EXISTS scene_emotion_beats (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  scene_id TEXT NOT NULL,
  beat_index INTEGER NOT NULL,
  from_json TEXT NOT NULL,
  to_json TEXT NOT NULL,
  trigger_event TEXT,
  appraisal_json TEXT,
  action_tendency TEXT,
  dialogue_policy_json TEXT,
  cost TEXT,
  carrier_weights_json TEXT NOT NULL,
  reader_target_json TEXT,
  scale REAL NOT NULL DEFAULT 0.8,
  event_kind TEXT,
  source_name TEXT,
  target_name TEXT,
  tension_id TEXT,
  debt_id TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
  FOREIGN KEY (scene_id) REFERENCES scenes(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_scene_emotion_beats_scene
  ON scene_emotion_beats(scene_id, beat_index);
CREATE INDEX IF NOT EXISTS idx_scene_emotion_beats_story
  ON scene_emotion_beats(story_id, created_at);

CREATE TABLE IF NOT EXISTS emotion_tensions (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  source_character_id TEXT NOT NULL,
  target_character_id TEXT,
  tension_type TEXT NOT NULL,
  bond TEXT,
  reverse_bond TEXT,
  pressure REAL NOT NULL DEFAULT 0.5,
  accumulated_beats INTEGER NOT NULL DEFAULT 0,
  due_chapter INTEGER,
  status TEXT NOT NULL DEFAULT 'open',
  last_escalated_chapter INTEGER,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_emotion_tensions_story
  ON emotion_tensions(story_id, status, due_chapter);

CREATE TABLE IF NOT EXISTS emotion_debts (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  source_ref TEXT,
  description TEXT,
  pressure REAL NOT NULL DEFAULT 0.5,
  due_chapter INTEGER,
  status TEXT NOT NULL DEFAULT 'open',
  created_at TEXT NOT NULL,
  FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_emotion_debts_story
  ON emotion_debts(story_id, status, due_chapter);

CREATE TABLE IF NOT EXISTS reader_resonance_reviews (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  scene_id TEXT,
  chapter_number INTEGER,
  persona TEXT NOT NULL,
  scores_json TEXT NOT NULL,
  verdict TEXT NOT NULL,
  evidence_json TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_reader_resonance_story
  ON reader_resonance_reviews(story_id, chapter_number);

CREATE TABLE IF NOT EXISTS emotion_probe_log (
  id TEXT PRIMARY KEY,
  story_id TEXT NOT NULL,
  scene_id TEXT,
  beat_index INTEGER,
  probe TEXT NOT NULL,
  severity TEXT NOT NULL,
  passed INTEGER NOT NULL,
  detail_json TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_emotion_probe_story
  ON emotion_probe_log(story_id, scene_id, probe);
