-- v0.59.0：补 6 处热查询索引。
--
-- 均为已在代码中高频执行、但此前只能全表扫描/排序的查询：
--   1. llm_calls 按故事取最近调用（usage 面板 / 成本统计）
--   2. llm_calls 按模型取时间窗（model_gateway 健康探测）
--   3. agency_board_items 跨 run 按故事读取（黑板资产区）
--   4. agency_activity_log 按时间剪枝（保留策略需要 created_at）
--   5. character_relationships 三元组去重（每章每关系一次查找）
--   6. characters 按故事+名字查找（资产桥接/角色 upsert）
CREATE INDEX IF NOT EXISTS idx_llm_calls_story_time ON llm_calls(story_id, created_at);
CREATE INDEX IF NOT EXISTS idx_llm_calls_model_time ON llm_calls(model_id, created_at);
CREATE INDEX IF NOT EXISTS idx_agency_board_story_time ON agency_board_items(story_id, created_at);
CREATE INDEX IF NOT EXISTS idx_agency_activity_log_created ON agency_activity_log(created_at);
CREATE INDEX IF NOT EXISTS idx_char_rel_triple ON character_relationships(story_id, source_character_id, target_character_id);
CREATE INDEX IF NOT EXISTS idx_characters_story_name ON characters(story_id, name);
