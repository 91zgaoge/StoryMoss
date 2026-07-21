-- V112: 回填 world_buildings.cultures / rules NULL -> '[]'
-- 这两列在基础 schema 为 nullable TEXT（无 NOT NULL/DEFAULT），旧数据
-- （StoryForge 数据迁移导入的世界观行、或早期无该列的行）可能为 NULL，
-- 导致 get_by_story/get_by_id 读取为 String 时报
-- "Invalid column type Null at index: 5, name: cultures"（或 index: 3 rules），
-- 续写/创世获取世界观失败。回填空数组保证数据一致（读取层已同步加 NULL 兜底）。
UPDATE world_buildings SET cultures = '[]' WHERE cultures IS NULL;
UPDATE world_buildings SET rules = '[]' WHERE rules IS NULL;
