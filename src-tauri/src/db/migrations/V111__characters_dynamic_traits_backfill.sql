-- V111: 回填 characters.dynamic_traits NULL -> '[]'
-- 该列在基础 schema 为 nullable TEXT（无 NOT NULL/DEFAULT），旧数据
-- （StoryForge 数据迁移导入的角色行、或早期无该列的行）可能为 NULL，
-- 导致 get_by_story/get_by_id 读取为 String 时报
-- "Invalid column type Null at index: 9, name: dynamic_traits"，
-- 续写/创世获取角色失败。回填空数组保证数据一致（读取层已同步加 NULL 兜底）。
UPDATE characters SET dynamic_traits = '[]' WHERE dynamic_traits IS NULL;
