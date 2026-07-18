-- V109: agency_runs 同 story 仅一个进行中 run（护栏原子化）
-- 先清理（与启动收割同语义，幂等），再建部分唯一索引
UPDATE agency_runs SET status = 'failed', error_message = COALESCE(error_message, 'reaped by V109'),
    updated_at = datetime('now')
WHERE status IN ('pending', 'running');

CREATE UNIQUE INDEX IF NOT EXISTS idx_agency_runs_one_active_per_story
ON agency_runs(story_id) WHERE status IN ('pending', 'running');
