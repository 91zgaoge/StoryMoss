-- v0.59.0：为已应用迁移记录内容校验和。
--
-- 背景：schema_migrations 原先只有 (version, applied_at)，两份同版本但内容
-- 不同的迁移（历史上 target/ 下的陈旧副本曾遮蔽源码目录）不会被任何机制发现；
-- 迁移文件在发布后被静默改动同样无从察觉。运行器现在按版本记录 SHA-256，
-- 启动时比对已应用版本与新文件的内容。
--
-- 旧行保持 NULL；运行器只在库内已有校验和时才比对，不会误报。
ALTER TABLE schema_migrations ADD COLUMN checksum TEXT;
