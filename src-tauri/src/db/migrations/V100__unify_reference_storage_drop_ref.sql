-- v0.23: 拆书存储统一
-- 1) 为 narrative_scenes 补齐拆书专用字段
--    新数据库已通过 CREATE TABLE 包含这些列，重复 ALTER 会被 MigrationRunner 跳过
ALTER TABLE narrative_scenes ADD COLUMN key_events TEXT;
ALTER TABLE narrative_scenes ADD COLUMN emotional_tone TEXT;
ALTER TABLE narrative_scenes ADD COLUMN narrative_intensity REAL DEFAULT 0.0;
ALTER TABLE narrative_scenes ADD COLUMN narrative_sentiment REAL DEFAULT 0.0;
ALTER TABLE narrative_scenes ADD COLUMN narrative_event_types TEXT DEFAULT '[]';
ALTER TABLE narrative_scenes ADD COLUMN act_number INTEGER DEFAULT 1;
ALTER TABLE narrative_scenes ADD COLUMN position_in_act REAL DEFAULT 0.0;

-- 2) 删除已弃用的拆书参考表；拆书数据统一保存在 narrative_* 统一表
DROP TABLE IF EXISTS reference_characters;
DROP TABLE IF EXISTS reference_scenes;
