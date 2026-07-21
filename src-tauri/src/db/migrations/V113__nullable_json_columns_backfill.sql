-- V113: 全面回填 nullable JSON/TEXT 列 NULL -> 空数组/对象/默认值
-- 与 V111 (characters.dynamic_traits) / V112 (world_buildings.cultures/rules) 同类，
-- 覆盖其余表的 nullable JSON TEXT 列与 user_preferences 全部 nullable 列。
-- 读取层已同步加 NULL 兜底（Option<String> + unwrap_or_default/unwrap_or_else），
-- 本迁移保证数据一致，避免旧数据（StoryForge 迁移导入 / ALTER TABLE ADD COLUMN
-- 无 DEFAULT 的行）在各 repository 读为 String 时报
-- "Invalid column type Null at index: N, name: COLUMN"。

-- scenes
UPDATE scenes SET characters_present = '[]' WHERE characters_present IS NULL;
UPDATE scenes SET character_conflicts = '[]' WHERE character_conflicts IS NULL;

-- scene_versions
UPDATE scene_versions SET characters_present = '[]' WHERE characters_present IS NULL;
UPDATE scene_versions SET character_conflicts = '[]' WHERE character_conflicts IS NULL;

-- studio_configs
UPDATE studio_configs SET llm_config = '{}' WHERE llm_config IS NULL;
UPDATE studio_configs SET ui_config = '{}' WHERE ui_config IS NULL;
UPDATE studio_configs SET agent_bots = '[]' WHERE agent_bots IS NULL;

-- writing_styles
UPDATE writing_styles SET custom_rules = '[]' WHERE custom_rules IS NULL;

-- kg_entities
UPDATE kg_entities SET attributes = '{}' WHERE attributes IS NULL;

-- kg_relations
UPDATE kg_relations SET evidence = '[]' WHERE evidence IS NULL;

-- user_preferences（全表 nullable，回填合理默认值）
UPDATE user_preferences SET preference_type = 'content' WHERE preference_type IS NULL;
UPDATE user_preferences SET preference_key = '' WHERE preference_key IS NULL;
UPDATE user_preferences SET preference_value = '' WHERE preference_value IS NULL;
UPDATE user_preferences SET confidence = 0.0 WHERE confidence IS NULL;
UPDATE user_preferences SET evidence_count = 0 WHERE evidence_count IS NULL;
UPDATE user_preferences SET updated_at = '' WHERE updated_at IS NULL;
