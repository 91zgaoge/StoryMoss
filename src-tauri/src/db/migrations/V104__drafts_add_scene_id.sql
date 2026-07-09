-- V104: drafts 表增加 scene_id，定稿按场景落盘（不再 chapter→first scene）
ALTER TABLE drafts ADD COLUMN scene_id TEXT;
CREATE INDEX IF NOT EXISTS idx_drafts_story_scene ON drafts(story_id, scene_id);
