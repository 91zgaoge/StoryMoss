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
            logline: None,
            strategy_json: None,
            story_format: "novel".to_string(),
            production_constraints: None,
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
             methodology_id, methodology_step, reference_book_id, logline, strategy_json, \
             story_format, production_constraints, created_at, updated_at \
             FROM stories ORDER BY updated_at DESC",
        )?;

        let stories = stmt
            .query_map([], map_story_row)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(stories)
    }

    /// 列表查询并附带角色/场景/章节/字数聚合，供仪表盘统计使用。
    pub fn get_all_with_counts(&self) -> Result<Vec<StoryListItem>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT s.id, s.title, s.description, s.genre, s.tone, s.pacing, s.style_dna_id, \
             s.genre_profile_id, s.methodology_id, s.methodology_step, s.reference_book_id, \
             s.logline, s.strategy_json, s.story_format, s.production_constraints, s.created_at, \
             s.updated_at, \
             (SELECT COUNT(*) FROM characters c WHERE c.story_id = s.id) AS character_count, \
             (SELECT COUNT(*) FROM scenes sc WHERE sc.story_id = s.id) AS scene_count, \
             (SELECT COUNT(*) FROM chapters ch WHERE ch.story_id = s.id) AS chapter_count, \
             (SELECT COALESCE(SUM(LENGTH(sc.content)), 0) FROM scenes sc WHERE sc.story_id = s.id) \
             AS word_count \
             FROM stories s ORDER BY s.updated_at DESC",
        )?;

        let stories = stmt
            .query_map([], |row| {
                Ok(StoryListItem {
                    story: map_story_row(row)?,
                    character_count: row.get(17)?,
                    scene_count: row.get(18)?,
                    chapter_count: row.get(19)?,
                    word_count: row.get(20)?,
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
             methodology_id, methodology_step, reference_book_id, logline, strategy_json, \
             story_format, production_constraints, created_at, updated_at \
             FROM stories WHERE id = ?1",
        )?;

        let story = stmt.query_row([id], map_story_row).optional()?;

        Ok(story)
    }

    /// v0.30.22: 更新故事的 PROBLEM logline（genesis 完成后调用）。
    pub fn update_logline(&self, id: &str, logline: &str) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE stories SET logline = ?1, updated_at = ?2 WHERE id = ?3",
            params![logline, now, id],
        )?;
        Ok(())
    }

    /// v0.31: 持久化向导选中的创作策略四元组（apply_wizard_to_story 调用）。
    pub fn update_strategy_json(
        &self,
        id: &str,
        strategy_json: &str,
    ) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE stories SET strategy_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![strategy_json, now, id],
        )?;
        Ok(())
    }

    pub fn update_story_format(
        &self,
        id: &str,
        format: &str,
        constraints: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let format = if format == "short_drama" {
            "short_drama"
        } else {
            "novel"
        };
        conn.execute(
            "UPDATE stories SET story_format = ?1, production_constraints = COALESCE(?2, \
             production_constraints), updated_at = ?3 WHERE id = ?4",
            params![format, constraints, now, id],
        )?;
        Ok(())
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
             reference_book_id = COALESCE(?11, reference_book_id), strategy_json = COALESCE(?12, \
             strategy_json), story_format = COALESCE(?13, story_format), production_constraints = \
             COALESCE(?14, production_constraints), updated_at = ?15 WHERE id = ?1",
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
                req.strategy_json,
                req.story_format.as_deref().map(|f| {
                    if f == "short_drama" {
                        "short_drama"
                    } else {
                        "novel"
                    }
                }),
                req.production_constraints,
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

        // v0.59.0：agency_* / ingest_jobs / llm_calls 没有 story 外键，
        // 此前删故事会留下永久孤儿行（黑板草稿含整章正文，体积可观）。
        // 子查询按 run_id 定位的表必须先于 agency_runs 删除。
        let _ = tx.execute("DELETE FROM agency_board_items WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM agency_checkpoints WHERE story_id = ?1", [id]);
        let _ = tx.execute(
            "DELETE FROM agency_messages WHERE run_id IN (SELECT id FROM agency_runs WHERE story_id = ?1)",
            [id],
        );
        let _ = tx.execute(
            "DELETE FROM agency_activity_log WHERE run_id IN (SELECT id FROM agency_runs WHERE story_id = ?1)",
            [id],
        );
        let _ = tx.execute("DELETE FROM agency_runs WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM ingest_jobs WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM llm_calls WHERE story_id = ?1", [id]);

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
                // v0.59.0：无外键的 story 级工作数据也必须随故事删除
                ("agency_runs", "story_id"),
                ("agency_board_items", "story_id"),
                ("agency_checkpoints", "story_id"),
                ("ingest_jobs", "story_id"),
                ("llm_calls", "story_id"),
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

fn map_story_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Story> {
    let created_str: String = row.get(15)?;
    let updated_str: String = row.get(16)?;
    let format: Option<String> = row.get(13)?;
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
        logline: row.get(11)?,
        strategy_json: row.get(12)?,
        story_format: if format.as_deref() == Some("short_drama") {
            "short_drama".to_string()
        } else {
            "novel".to_string()
        },
        production_constraints: row.get(14)?,
        created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
        updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
    })
}
