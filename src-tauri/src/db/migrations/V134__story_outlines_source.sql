-- v0.59.3：给 story_outlines 增加来源标记，防止机器提取静默覆盖作者手写大纲。
--
-- 背景：story_outlines 每故事一行（story_id UNIQUE），机器路径（创世 materialize /
-- 资产回流 ingest）会整体覆盖或追加 content，作者在幕后手写的大纲会被无声改掉。
--
-- 存量行标记为 'unknown'：保持原语义（仍允许机器更新），不回填成 user_created——
-- 否则所有老库的大纲会被永久冻结，机器再也无法精炼。
-- 语义：'user_created' = 作者手写或在弹窗中确认过 → 机器路径不得覆盖/追加；
--       其余（agency / ingest / unknown）视为机器来源，可继续精炼。
ALTER TABLE story_outlines ADD COLUMN source TEXT;
UPDATE story_outlines SET source = 'unknown' WHERE source IS NULL;
