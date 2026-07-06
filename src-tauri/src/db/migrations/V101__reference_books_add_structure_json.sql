-- V101: 为 reference_books 表补齐 analyzed_structure_json 列
-- 旧版升级路径缺少该列，导致拆书结果的叙事结构无法保存
ALTER TABLE reference_books ADD COLUMN analyzed_structure_json TEXT;
