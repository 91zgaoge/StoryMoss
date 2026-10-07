-- v0.64.6：人物别称表（「称呼 → 人物」）
--
-- 背景：`characters` 此前只有 name，且 ingest 按名字精确匹配建行。中文小说里同一人物
-- 会用「姓+称号」（苏世子）、「称号+名」（景亲王曹元寿）、字、号、官职、小名等多种
-- 称呼，于是每个新称呼都会长出一个幻影人物行（真机《帝国的烟火》：`苏世子` 与
-- `景亲王` / `景亲王曹元寿` 各占一行，关系表与提示词随之出现两个"同一个人"）。
--
-- 本表登记「别称 → 本人」的映射：
--   * ingest 抽到的 aliases 落在这里；
--   * 建行前先按 name / alias / 称号形态解析，命中则挂到既有角色并补登记别称；
--   * 别称与某个既有角色行同名时，把那一行合并进来（见 character_identity 合并工具）。
--
-- UNIQUE(story_id, alias)：同一故事里一个称呼只能指向一个人物；不同故事互不影响。
CREATE TABLE IF NOT EXISTS character_aliases (
    id TEXT PRIMARY KEY,
    story_id TEXT NOT NULL,
    character_id TEXT NOT NULL,
    alias TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'ingest',
    created_at TEXT NOT NULL,
    FOREIGN KEY (story_id) REFERENCES stories(id) ON DELETE CASCADE,
    FOREIGN KEY (character_id) REFERENCES characters(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_character_aliases_unique
    ON character_aliases(story_id, alias);

CREATE INDEX IF NOT EXISTS idx_character_aliases_character
    ON character_aliases(character_id);
