use super::*;

pub struct ChapterRepository {
    pool: DbPool,
}

impl ChapterRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, req: CreateChapterRequest) -> Result<Chapter, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;

        // 1. 插入 Chapter（Phase 1: content 不再写入 chapters 表，Scene 为真相源）
        tx.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title, outline, content, \
             word_count, model_used, cost, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, \
             '', ?6, ?7, ?8, ?9, ?10)",
            params![
                &id,
                &req.story_id,
                req.chapter_number,
                req.title,
                req.outline,
                0, // word_count 从 Scene 聚合计算
                "",
                0.0,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        // 2. 查找或创建关联的 Scene（内容写入 Scene 表）
        let _scene_id = match tx
            .query_row(
                "SELECT id FROM scenes WHERE story_id = ?1 AND sequence_number = ?2",
                params![&req.story_id, req.chapter_number],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            Some(sid) => {
                // 关联已有 Scene，同时写入内容
                tx.execute(
                    "UPDATE scenes SET chapter_id = ?1, content = COALESCE(?2, content), title = COALESCE(?3, title) WHERE id = ?4",
                    params![&id, req.content, req.title, &sid],
                )?;
                Some(sid)
            }
            None => {
                // 创建新 Scene 并写入内容
                let sid = Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO scenes (id, story_id, sequence_number, title, content, \
                     characters_present, character_conflicts, execution_stage, chapter_id, \
                     created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    params![
                        &sid,
                        &req.story_id,
                        req.chapter_number,
                        req.title,
                        req.content,
                        "[]",
                        "[]",
                        "drafting",
                        &id,
                        now.to_rfc3339(),
                        now.to_rfc3339()
                    ],
                )?;
                Some(sid)
            }
        };

        tx.commit()?;

        let word_count = req.content.as_ref().map(|c| c.len() as i32);
        Ok(Chapter {
            id,
            story_id: req.story_id,
            chapter_number: req.chapter_number,
            title: req.title,
            outline: req.outline,
            content: req.content, // 从请求参数传回（Scene 为真相源，此处为便利字段）
            word_count,
            model_used: None,
            cost: None,
            created_at: now,
            updated_at: now,
        })
    }

    /// Phase 1: content 字段优先读 chapters.content（兼容旧数据），
    /// 为空时从 scenes 表聚合（新数据路径）。
    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<Chapter>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, title, outline, content, word_count, \
             model_used, cost, created_at, updated_at FROM chapters WHERE story_id = ?1 ORDER BY \
             chapter_number",
        )?;

        let mut chapters: Vec<Chapter> = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(9)?;
                let updated_str: String = row.get(10)?;
                Ok(Chapter {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    title: row.get(3)?,
                    outline: row.get(4)?,
                    content: row.get(5)?,
                    word_count: row.get(6)?,
                    model_used: row.get(7)?,
                    cost: row.get(8)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Phase 1: 当 chapters.content 为空时，从 scenes 表聚合
        for chapter in &mut chapters {
            if chapter.content.as_ref().map_or(true, |c| c.is_empty()) {
                chapter.content = Some(self.get_content(&chapter.id)?);
                // 同步更新 word_count
                if chapter.word_count.unwrap_or(0) == 0 {
                    chapter.word_count =
                        Some(chapter.content.as_ref().map_or(0, |c| c.len() as i32));
                }
            }
        }

        Ok(chapters)
    }

    /// 分页查询 story 下的章节列表（不返回 content / outline 等大字段）。
    pub fn get_by_story_paged(
        &self,
        story_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Chapter>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, title, word_count, model_used, cost, created_at, \
             updated_at
             FROM chapters WHERE story_id = ?1 ORDER BY chapter_number LIMIT ?2 OFFSET ?3",
        )?;

        let chapters = stmt
            .query_map(params![story_id, limit, offset], |row| {
                let created_str: String = row.get(7)?;
                let updated_str: String = row.get(8)?;
                Ok(Chapter {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    title: row.get(3)?,
                    outline: None,
                    content: None,
                    word_count: row.get(4)?,
                    model_used: row.get(5)?,
                    cost: row.get(6)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(chapters)
    }

    /// 统计 story 下章节总数。
    pub fn count_by_story(&self, story_id: &str) -> Result<i64, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM chapters WHERE story_id = ?1",
            [story_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// 聚合 story 下所有场景 content 字段的总长度（用于总字数统计，避免全量
    /// IPC）。
    /// Phase 1: 改为从 scenes 表聚合（Scene 为内容真相源）。
    pub fn total_content_length_by_story(&self, story_id: &str) -> Result<i64, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(content)), 0) FROM scenes WHERE story_id = ?1",
            [story_id],
            |row| row.get(0),
        )?;
        Ok(total)
    }

    /// Phase 1: 从 scenes 表聚合章节内容（Scene 为唯一内容真相源）。
    /// 按 sequence_number 排序，多个 Scene 内容直接拼接，无分隔符。
    pub fn get_content(&self, chapter_id: &str) -> Result<String, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT COALESCE(content, '') FROM scenes WHERE chapter_id = ?1 ORDER BY sequence_number",
        )?;
        let parts: Vec<String> = stmt
            .query_map([chapter_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(parts.concat())
    }

    /// Phase 1: content 字段优先读 chapters.content（兼容旧数据），
    /// 为空时从 scenes 表聚合（新数据路径）。
    pub fn get_by_id(&self, id: &str) -> Result<Option<Chapter>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, title, outline, content, word_count, \
             model_used, cost, created_at, updated_at FROM chapters WHERE id = ?1",
        )?;

        let mut chapter = stmt
            .query_row([id], |row| {
                let created_str: String = row.get(9)?;
                let updated_str: String = row.get(10)?;
                Ok(Chapter {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    title: row.get(3)?,
                    outline: row.get(4)?,
                    content: row.get(5)?,
                    word_count: row.get(6)?,
                    model_used: row.get(7)?,
                    cost: row.get(8)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        // Phase 1: 当 chapters.content 为空时，从 scenes 表聚合
        if let Some(ref mut ch) = chapter {
            if ch.content.as_ref().map_or(true, |c| c.is_empty()) {
                ch.content = Some(self.get_content(&ch.id)?);
                if ch.word_count.unwrap_or(0) == 0 {
                    ch.word_count = Some(ch.content.as_ref().map_or(0, |c| c.len() as i32));
                }
            }
        }

        Ok(chapter)
    }

    /// Phase 1: content 参数已移除。章内容以 Scene 为唯一真相源，
    /// 请使用 SceneRepository
    /// 写入内容。本方法仅更新章级元数据（title/outline/word_count）。 title
    /// 变更会同步到关联的 Scene(s)。
    pub fn update(
        &self,
        id: &str,
        title: Option<String>,
        outline: Option<String>,
        word_count: Option<i32>,
    ) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let tx = conn.transaction()?;

        let count = tx.execute(
            "UPDATE chapters SET title = COALESCE(?2, title), outline = COALESCE(?3, outline),
             word_count = COALESCE(?4, word_count), updated_at = ?5 WHERE id = ?1",
            params![id, title, outline, word_count, now],
        )?;

        // 同步标题到关联的 Scene(s)（标题是共享元数据）
        if title.is_some() {
            let scene_ids: Vec<String> = tx
                .prepare("SELECT id FROM scenes WHERE chapter_id = ?1")?
                .query_map([id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for sid in scene_ids {
                tx.execute(
                    "UPDATE scenes SET title = COALESCE(?2, title), updated_at = ?3 WHERE id = ?1",
                    params![sid, title, now],
                )?;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;

        // 验证章节是否存在
        let exists: bool = tx
            .query_row("SELECT 1 FROM chapters WHERE id = ?1", [id], |_| Ok(true))
            .unwrap_or(false);

        if !exists {
            tx.rollback()?;
            return Ok(0);
        }

        // 解除与 scenes 的关联关系
        tx.execute(
            "UPDATE scenes SET chapter_id = NULL WHERE chapter_id = ?1",
            [id],
        )?;

        // 删除章节
        let count = tx.execute("DELETE FROM chapters WHERE id = ?1", [id])?;

        tx.commit()?;
        Ok(count)
    }
}
