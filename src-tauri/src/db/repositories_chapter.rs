#![allow(unused_imports)]
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::{Chapter, CreateChapterRequest, DbPool};

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
        let word_count = req.content.as_ref().map(|c| c.len() as i32);

        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;

        // 1. 插入 Chapter
        tx.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title, outline, content, \
             word_count, model_used, cost, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, \
             ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &id,
                &req.story_id,
                req.chapter_number,
                req.title,
                req.outline,
                req.content,
                word_count,
                "",
                0.0,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        // 2. 查找或创建关联的 Scene
        let _scene_id = match tx
            .query_row(
                "SELECT id FROM scenes WHERE story_id = ?1 AND sequence_number = ?2",
                params![&req.story_id, req.chapter_number],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            Some(sid) => {
                // 关联已有 Scene
                tx.execute(
                    "UPDATE scenes SET chapter_id = ?1 WHERE id = ?2",
                    params![&id, &sid],
                )?;
                Some(sid)
            }
            None => {
                // 创建新 Scene
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

        Ok(Chapter {
            id,
            story_id: req.story_id,
            chapter_number: req.chapter_number,
            title: req.title,
            outline: req.outline,
            content: req.content,
            word_count,
            model_used: None,
            cost: None,
            created_at: now,
            updated_at: now,
        })
    }

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

        let chapters = stmt
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

        Ok(chapters)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Chapter>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, title, outline, content, word_count, \
             model_used, cost, created_at, updated_at FROM chapters WHERE id = ?1",
        )?;

        let chapter = stmt
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

        Ok(chapter)
    }

    pub fn update(
        &self,
        id: &str,
        title: Option<String>,
        outline: Option<String>,
        content: Option<String>,
        word_count: Option<i32>,
    ) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let word_count = word_count.or_else(|| content.as_ref().map(|c| c.len() as i32));

        let tx = conn.transaction()?;

        let count = tx.execute(
            "UPDATE chapters SET title = COALESCE(?2, title), outline = COALESCE(?3, outline),
             content = COALESCE(?4, content), word_count = COALESCE(?5, word_count), updated_at = \
             ?6 WHERE id = ?1",
            params![id, title, outline, content, word_count, now],
        )?;

        // 同步更新关联的 Scene(s)
        if title.is_some() || content.is_some() {
            let scene_ids: Vec<String> = tx
                .prepare("SELECT id FROM scenes WHERE chapter_id = ?1")?
                .query_map([id], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for sid in scene_ids {
                tx.execute(
                    "UPDATE scenes SET title = COALESCE(?2, title), content = COALESCE(?3, \
                     content), updated_at = ?4 WHERE id = ?1",
                    params![sid, title, content, now],
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

// ==================== UserRepository ====================
