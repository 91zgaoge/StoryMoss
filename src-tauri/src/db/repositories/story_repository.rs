use super::*;

pub struct StoryRepository {
    pool: DbPool,
}

impl StoryRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        req: CreateStoryRequest,
    ) -> Result<Story, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        tx.execute(
            "INSERT INTO stories (id, title, description, genre, tone, pacing, style_dna_id, \
             genre_profile_id, methodology_id, methodology_step, reference_book_id, created_at, \
             updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &id,
                &req.title,
                req.description,
                req.genre,
                "dark",
                "medium",
                req.style_dna_id,
                req.genre_profile_id,
                req.methodology_id,
                None::<i32>,
                req.reference_book_id,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(Story {
            id,
            title: req.title,
            description: req.description,
            genre: req.genre,
            tone: Some("dark".to_string()),
            pacing: Some("medium".to_string()),
            style_dna_id: req.style_dna_id,
            genre_profile_id: req.genre_profile_id,
            methodology_id: req.methodology_id,
            methodology_step: None,
            reference_book_id: req.reference_book_id,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn create(&self, req: CreateStoryRequest) -> Result<Story, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let story = self.create_in_tx(&tx, req)?;
        tx.commit()?;
        Ok(story)
    }

    pub fn get_all(&self) -> Result<Vec<Story>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, title, description, genre, tone, pacing, style_dna_id, genre_profile_id, \
             methodology_id, methodology_step, reference_book_id, created_at, updated_at FROM \
             stories ORDER BY updated_at DESC",
        )?;

        let stories = stmt
            .query_map([], |row| {
                let created_str: String = row.get(11)?;
                let updated_str: String = row.get(12)?;
                Ok(Story {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    genre: row.get(3)?,
                    tone: row.get(4)?,
                    pacing: row.get(5)?,
                    style_dna_id: row.get(6)?,
                    genre_profile_id: row.get(7)?,
                    methodology_id: row.get(8)?,
                    methodology_step: row.get(9)?,
                    reference_book_id: row.get(10)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(stories)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Story>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, title, description, genre, tone, pacing, style_dna_id, genre_profile_id, \
             methodology_id, methodology_step, reference_book_id, created_at, updated_at FROM \
             stories WHERE id = ?1",
        )?;

        let story = stmt
            .query_row([id], |row| {
                let created_str: String = row.get(11)?;
                let updated_str: String = row.get(12)?;
                Ok(Story {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    description: row.get(2)?,
                    genre: row.get(3)?,
                    tone: row.get(4)?,
                    pacing: row.get(5)?,
                    style_dna_id: row.get(6)?,
                    genre_profile_id: row.get(7)?,
                    methodology_id: row.get(8)?,
                    methodology_step: row.get(9)?,
                    reference_book_id: row.get(10)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(story)
    }

    pub fn update(&self, id: &str, req: &UpdateStoryRequest) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE stories SET title = COALESCE(?2, title), description = COALESCE(?3, \
             description),
             genre = COALESCE(?4, genre), tone = COALESCE(?5, tone), pacing = COALESCE(?6, pacing),
             style_dna_id = COALESCE(?7, style_dna_id), genre_profile_id = COALESCE(?8, \
             genre_profile_id),
             methodology_id = COALESCE(?9, methodology_id), methodology_step = COALESCE(?10, \
             methodology_step),
             reference_book_id = COALESCE(?11, reference_book_id), updated_at = ?12 WHERE id = ?1",
            params![
                id,
                req.title,
                req.description,
                req.genre,
                req.tone,
                req.pacing,
                req.style_dna_id,
                req.genre_profile_id,
                req.methodology_id,
                req.methodology_step,
                req.reference_book_id,
                now
            ],
        )?;
        Ok(count)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        // 在事务中执行删除操作，确保级联删除正确执行
        let tx = conn.unchecked_transaction()?;

        // 验证故事是否存在
        let exists: bool = tx
            .query_row("SELECT 1 FROM stories WHERE id = ?1", [id], |_| Ok(true))
            .unwrap_or(false);

        if !exists {
            tx.rollback()?;
            return Ok(0);
        }
        // 即使外键约束已启用，也作为防御性编程添加显式 DELETE
        let _ = tx.execute("DELETE FROM story_metadata WHERE story_id = ?1", [id]);
        let _ = tx.execute(
            "DELETE FROM foreshadowing_tracker WHERE story_id = ?1",
            [id],
        );
        let _ = tx.execute("DELETE FROM user_preferences WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM story_runtime_states WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM story_style_configs WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM story_outlines WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM studio_configs WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM story_summaries WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM narrative_characters WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM narrative_scenes WHERE story_id = ?1", [id]);
        let _ = tx.execute(
            "DELETE FROM narrative_world_buildings WHERE story_id = ?1",
            [id],
        );
        let _ = tx.execute("DELETE FROM chat_sessions WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM text_annotations WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM ai_operations WHERE story_id = ?1", [id]);

        // 执行删除操作 - 由于外键约束已启用，大部分相关数据会自动级联删除
        let count = tx.execute("DELETE FROM stories WHERE id = ?1", [id])?;

        tx.commit()?;

        // 不变量断言: 删除 story 后，所有关联表不应存在孤儿数据
        // 仅在 debug 构建时检查，用于在开发和测试阶段快速发现级联删除遗漏
        #[cfg(debug_assertions)]
        {
            let check_conn = self
                .pool
                .get()
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
            let orphan_tables = [
                ("chapters", "story_id"),
                ("characters", "story_id"),
                ("scenes", "story_id"),
                ("kg_entities", "story_id"),
                ("kg_relations", "story_id"),
                ("character_relationships", "story_id"),
                ("scene_annotations", "story_id"),
            ];
            for (table, col) in orphan_tables {
                let orphan_count: i64 = check_conn
                    .query_row(
                        &format!("SELECT COUNT(*) FROM {} WHERE {} = ?1", table, col),
                        [id],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                debug_assert_eq!(
                    orphan_count, 0,
                    "StoryRepository::delete orphan invariant violated: {} rows remain in {} \
                     after story {} deletion",
                    orphan_count, table, id
                );
            }
        }

        Ok(count)
    }
}
