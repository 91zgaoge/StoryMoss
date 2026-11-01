-- V151：多 write 并行。
--
-- 1) V149 的 (story_id, lane) 唯一索引把 write lane 也锁成单活跃。现在 write
--    lane 允许多个活跃 run（上限由命令层按 MAX_ACTIVE_WRITE_RUNS_PER_STORY
--    控制）；review / inspect / chat 仍保持单活跃。
-- 2) 章号分配改为 story_chapter_sequences 原子递增，避免多个 write run 同时
--    用 MAX(scenes.sequence_number)+1 抢到同一章号。

DROP INDEX IF EXISTS idx_agency_runs_one_active_per_lane;
CREATE UNIQUE INDEX IF NOT EXISTS idx_agency_runs_one_active_per_lane
  ON agency_runs(story_id, lane)
  WHERE status IN ('pending', 'running') AND lane <> 'write';

CREATE TABLE IF NOT EXISTS story_chapter_sequences (
  story_id TEXT PRIMARY KEY,
  next_chapter INTEGER NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE
);

INSERT OR IGNORE INTO story_chapter_sequences (story_id, next_chapter, updated_at)
SELECT s.id,
       COALESCE((SELECT MAX(sequence_number) + 1 FROM scenes WHERE story_id = s.id), 1),
       datetime('now')
FROM stories s;
