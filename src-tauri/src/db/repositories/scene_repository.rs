use super::*;

// ==================== Scene Repository ====================

pub struct SceneRepository {
    pool: DbPool,
}

impl SceneRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        sequence_number: i32,
        title: Option<&str>,
    ) -> Result<Scene, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        tx.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, characters_present, \
             character_conflicts, execution_stage, chapter_id, source, is_auto_generated, \
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?10, ?11)",
            params![
                &id,
                story_id,
                sequence_number,
                title,
                "[]",
                "[]",
                "drafting",
                "user_created",
                0,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        let existing_chapter: Option<String> = tx
            .query_row(
                "SELECT id FROM chapters WHERE story_id = ?1 AND chapter_number = ?2",
                params![story_id, sequence_number],
                |row| row.get(0),
            )
            .optional()?;

        let chapter_id = if let Some(chapter_id) = existing_chapter {
            Some(chapter_id)
        } else {
            let chapter_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO chapters (id, story_id, chapter_number, title, word_count, \
                 model_used, cost, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    &chapter_id,
                    story_id,
                    sequence_number,
                    title,
                    0,
                    "",
                    0.0,
                    now.to_rfc3339(),
                    now.to_rfc3339()
                ],
            )?;
            Some(chapter_id)
        };

        if let Some(ref cid) = chapter_id {
            tx.execute(
                "UPDATE scenes SET chapter_id = ?1 WHERE id = ?2",
                params![cid, &id],
            )?;
        }

        Ok(Scene {
            id,
            story_id: story_id.to_string(),
            sequence_number,
            title: title.map(|s| s.to_string()),
            dramatic_goal: None,
            external_pressure: None,
            conflict_type: None,
            characters_present: vec![],
            character_conflicts: vec![],
            content: None,
            setting_location: None,
            setting_time: None,
            setting_atmosphere: None,
            previous_scene_id: None,
            next_scene_id: None,
            execution_stage: Some("drafting".to_string()),
            outline_content: None,
            draft_content: None,
            model_used: None,
            cost: None,
            source: Some("user_created".to_string()),
            is_auto_generated: Some(false),
            created_at: now,
            updated_at: now,
            confidence_score: None,
            style_blend_override: None,
            foreshadowing_ids: None,
            chapter_id,
            narrative_intensity: None,
            narrative_sentiment: None,
            narrative_event_types: None,
            narrative_preceding_scene_id: None,
            narrative_following_scene_id: None,
            act_number: None,
            position_in_act: None,
        })
    }

    pub fn create(
        &self,
        story_id: &str,
        sequence_number: i32,
        title: Option<&str>,
    ) -> Result<Scene, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let scene = self.create_in_tx(&tx, story_id, sequence_number, title)?;
        tx.commit()?;
        Ok(scene)
    }

    pub fn update(&self, id: &str, updates: &SceneUpdate) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let count = self.update_in_tx(&tx, id, updates)?;
        tx.commit()?;
        Ok(count)
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, sequence_number, title, dramatic_goal, external_pressure, \
             conflict_type,
                    characters_present, character_conflicts, setting_location, setting_time, \
             setting_atmosphere,
                    content, previous_scene_id, next_scene_id, model_used, cost, source, \
             is_auto_generated, created_at, updated_at, confidence_score,
                    execution_stage, outline_content, draft_content, style_blend_override, \
             foreshadowing_ids, chapter_id,
                    narrative_intensity, narrative_sentiment, narrative_event_types, \
             narrative_preceding_scene_id,
                    narrative_following_scene_id, act_number, position_in_act
             FROM scenes WHERE story_id = ?1 ORDER BY sequence_number",
        )?;

        let scenes = stmt
            .query_map([story_id], |row| {
                let conflict_type_str: Option<String> = row.get(6)?;
                let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());

                let chars_json: String = row
                    .get::<_, Option<String>>(7)?
                    .unwrap_or_else(|| "[]".to_string());
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row
                    .get::<_, Option<String>>(8)?
                    .unwrap_or_else(|| "[]".to_string());
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_str: String = row.get(19)?;
                let updated_str: String = row.get(20)?;
                let confidence_score: Option<f32> = row.get(21)?;
                let execution_stage: Option<String> = row.get(22)?;
                let outline_content: Option<String> = row.get(23)?;
                let draft_content: Option<String> = row.get(24)?;
                let foreshadowing_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(26)?
                    .and_then(|s: String| serde_json::from_str(&s).ok());
                let is_auto_generated: Option<i32> = row.get(18).ok();

                Ok(Scene {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    sequence_number: row.get(2)?,
                    title: row.get(3)?,
                    dramatic_goal: row.get(4)?,
                    external_pressure: row.get(5)?,
                    conflict_type,
                    characters_present,
                    character_conflicts,
                    setting_location: row.get(9)?,
                    setting_time: row.get(10)?,
                    setting_atmosphere: row.get(11)?,
                    content: row.get(12)?,
                    previous_scene_id: row.get(13)?,
                    next_scene_id: row.get(14)?,
                    model_used: row.get(15)?,
                    cost: row.get(16)?,
                    source: row.get(17).ok(),
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score,
                    execution_stage,
                    outline_content,
                    draft_content,
                    style_blend_override: row.get(25)?,
                    foreshadowing_ids,
                    chapter_id: row.get::<_, Option<String>>(27)?,
                    narrative_intensity: row.get(28)?,
                    narrative_sentiment: row.get(29)?,
                    narrative_event_types: row.get(30)?,
                    narrative_preceding_scene_id: row.get(31)?,
                    narrative_following_scene_id: row.get(32)?,
                    act_number: row.get(33)?,
                    position_in_act: row.get(34)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(scenes)
    }

    /// 分页查询 story 下的场景列表（不返回 content / outline_content /
    /// draft_content 等大字段）。
    pub fn get_by_story_paged(
        &self,
        story_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<Scene>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, sequence_number, title, dramatic_goal, external_pressure, \
             conflict_type,
                    characters_present, character_conflicts, setting_location, setting_time, \
             setting_atmosphere,
                    previous_scene_id, next_scene_id, model_used, cost, source, is_auto_generated, \
             created_at, updated_at, confidence_score,
                    execution_stage, style_blend_override, foreshadowing_ids, chapter_id,
                    narrative_intensity, narrative_sentiment, narrative_event_types, \
             narrative_preceding_scene_id,
                    narrative_following_scene_id, act_number, position_in_act
             FROM scenes WHERE story_id = ?1 ORDER BY sequence_number LIMIT ?2 OFFSET ?3",
        )?;

        let scenes = stmt
            .query_map(params![story_id, limit, offset], |row| {
                let conflict_type_str: Option<String> = row.get(6)?;
                let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());

                let chars_json: String = row
                    .get::<_, Option<String>>(7)?
                    .unwrap_or_else(|| "[]".to_string());
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row
                    .get::<_, Option<String>>(8)?
                    .unwrap_or_else(|| "[]".to_string());
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_str: String = row.get(18)?;
                let updated_str: String = row.get(19)?;
                let confidence_score: Option<f32> = row.get(20)?;
                let execution_stage: Option<String> = row.get(21)?;
                let foreshadowing_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(23)?
                    .and_then(|s: String| serde_json::from_str(&s).ok());
                let is_auto_generated: Option<i32> = row.get(17).ok();

                Ok(Scene {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    sequence_number: row.get(2)?,
                    title: row.get(3)?,
                    dramatic_goal: row.get(4)?,
                    external_pressure: row.get(5)?,
                    conflict_type,
                    characters_present,
                    character_conflicts,
                    setting_location: row.get(9)?,
                    setting_time: row.get(10)?,
                    setting_atmosphere: row.get(11)?,
                    content: None,
                    previous_scene_id: row.get(12)?,
                    next_scene_id: row.get(13)?,
                    model_used: row.get(14)?,
                    cost: row.get(15)?,
                    source: row.get(16).ok(),
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score,
                    execution_stage,
                    outline_content: None,
                    draft_content: None,
                    style_blend_override: row.get(22)?,
                    foreshadowing_ids,
                    chapter_id: row.get::<_, Option<String>>(24)?,
                    narrative_intensity: row.get(25)?,
                    narrative_sentiment: row.get(26)?,
                    narrative_event_types: row.get(27)?,
                    narrative_preceding_scene_id: row.get(28)?,
                    narrative_following_scene_id: row.get(29)?,
                    act_number: row.get(30)?,
                    position_in_act: row.get(31)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(scenes)
    }

    /// 统计 story 下场景总数。
    pub fn count_by_story(&self, story_id: &str) -> Result<i64, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM scenes WHERE story_id = ?1",
            [story_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// 聚合 story 下所有场景 content 字段的总长度（用于总字数统计，避免全量
    /// IPC）。
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

    pub fn get_by_chapter(&self, chapter_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, sequence_number, title, dramatic_goal, external_pressure, \
             conflict_type,
                    characters_present, character_conflicts, setting_location, setting_time, \
             setting_atmosphere,
                    content, previous_scene_id, next_scene_id, model_used, cost, source, \
             is_auto_generated, created_at, updated_at, confidence_score,
                    execution_stage, outline_content, draft_content, style_blend_override, \
             foreshadowing_ids, chapter_id,
                    narrative_intensity, narrative_sentiment, narrative_event_types, \
             narrative_preceding_scene_id,
                    narrative_following_scene_id, act_number, position_in_act
             FROM scenes WHERE chapter_id = ?1 ORDER BY sequence_number",
        )?;

        let scenes = stmt
            .query_map([chapter_id], |row| {
                let conflict_type_str: Option<String> = row.get(6)?;
                let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());

                let chars_json: String = row
                    .get::<_, Option<String>>(7)?
                    .unwrap_or_else(|| "[]".to_string());
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row
                    .get::<_, Option<String>>(8)?
                    .unwrap_or_else(|| "[]".to_string());
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_str: String = row.get(19)?;
                let updated_str: String = row.get(20)?;
                let confidence_score: Option<f32> = row.get(21)?;
                let execution_stage: Option<String> = row.get(22)?;
                let outline_content: Option<String> = row.get(23)?;
                let draft_content: Option<String> = row.get(24)?;
                let foreshadowing_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(26)?
                    .and_then(|s: String| serde_json::from_str(&s).ok());
                let is_auto_generated: Option<i32> = row.get(18).ok();

                Ok(Scene {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    sequence_number: row.get(2)?,
                    title: row.get(3)?,
                    dramatic_goal: row.get(4)?,
                    external_pressure: row.get(5)?,
                    conflict_type,
                    characters_present,
                    character_conflicts,
                    setting_location: row.get(9)?,
                    setting_time: row.get(10)?,
                    setting_atmosphere: row.get(11)?,
                    content: row.get(12)?,
                    previous_scene_id: row.get(13)?,
                    next_scene_id: row.get(14)?,
                    model_used: row.get(15)?,
                    cost: row.get(16)?,
                    source: row.get(17).ok(),
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score,
                    execution_stage,
                    outline_content,
                    draft_content,
                    style_blend_override: row.get(25)?,
                    foreshadowing_ids,
                    chapter_id: row.get::<_, Option<String>>(27)?,
                    narrative_intensity: row.get(28)?,
                    narrative_sentiment: row.get(29)?,
                    narrative_event_types: row.get(30)?,
                    narrative_preceding_scene_id: row.get(31)?,
                    narrative_following_scene_id: row.get(32)?,
                    act_number: row.get(33)?,
                    position_in_act: row.get(34)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(scenes)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Scene>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, sequence_number, title, dramatic_goal, external_pressure, \
             conflict_type,
                    characters_present, character_conflicts, setting_location, setting_time, \
             setting_atmosphere,
                    content, previous_scene_id, next_scene_id, model_used, cost, source, \
             is_auto_generated, created_at, updated_at, confidence_score,
                    execution_stage, outline_content, draft_content, style_blend_override, \
             foreshadowing_ids, chapter_id,
                    narrative_intensity, narrative_sentiment, narrative_event_types, \
             narrative_preceding_scene_id,
                    narrative_following_scene_id, act_number, position_in_act
             FROM scenes WHERE id = ?1",
        )?;

        let scene = stmt
            .query_row([id], |row| {
                let conflict_type_str: Option<String> = row.get(6)?;
                let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());

                let chars_json: String = row
                    .get::<_, Option<String>>(7)?
                    .unwrap_or_else(|| "[]".to_string());
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row
                    .get::<_, Option<String>>(8)?
                    .unwrap_or_else(|| "[]".to_string());
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_str: String = row.get(19)?;
                let updated_str: String = row.get(20)?;
                let confidence_score: Option<f32> = row.get(21)?;
                let execution_stage: Option<String> = row.get(22)?;
                let outline_content: Option<String> = row.get(23)?;
                let draft_content: Option<String> = row.get(24)?;
                let foreshadowing_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(26)?
                    .and_then(|s: String| serde_json::from_str(&s).ok());
                let is_auto_generated: Option<i32> = row.get(18).ok();

                Ok(Scene {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    sequence_number: row.get(2)?,
                    title: row.get(3)?,
                    dramatic_goal: row.get(4)?,
                    external_pressure: row.get(5)?,
                    conflict_type,
                    characters_present,
                    character_conflicts,
                    setting_location: row.get(9)?,
                    setting_time: row.get(10)?,
                    setting_atmosphere: row.get(11)?,
                    content: row.get(12)?,
                    previous_scene_id: row.get(13)?,
                    next_scene_id: row.get(14)?,
                    model_used: row.get(15)?,
                    cost: row.get(16)?,
                    source: row.get(17).ok(),
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score,
                    execution_stage,
                    outline_content,
                    draft_content,
                    style_blend_override: row.get(25)?,
                    foreshadowing_ids,
                    chapter_id: row.get::<_, Option<String>>(27)?,
                    narrative_intensity: row.get(28)?,
                    narrative_sentiment: row.get(29)?,
                    narrative_event_types: row.get(30)?,
                    narrative_preceding_scene_id: row.get(31)?,
                    narrative_following_scene_id: row.get(32)?,
                    act_number: row.get(33)?,
                    position_in_act: row.get(34)?,
                })
            })
            .optional()?;

        Ok(scene)
    }

    pub fn update_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        id: &str,
        updates: &SceneUpdate,
    ) -> Result<usize, rusqlite::Error> {
        let now = Local::now().to_rfc3339();

        let count = tx.execute(
            "UPDATE scenes SET
                title = COALESCE(?2, title),
                dramatic_goal = COALESCE(?3, dramatic_goal),
                external_pressure = COALESCE(?4, external_pressure),
                conflict_type = COALESCE(?5, conflict_type),
                characters_present = COALESCE(?6, characters_present),
                character_conflicts = COALESCE(?7, character_conflicts),
                content = COALESCE(?8, content),
                setting_location = COALESCE(?9, setting_location),
                setting_time = COALESCE(?10, setting_time),
                setting_atmosphere = COALESCE(?11, setting_atmosphere),
                previous_scene_id = COALESCE(?12, previous_scene_id),
                next_scene_id = COALESCE(?13, next_scene_id),
                confidence_score = COALESCE(?14, confidence_score),
                execution_stage = COALESCE(?15, execution_stage),
                outline_content = COALESCE(?16, outline_content),
                draft_content = COALESCE(?17, draft_content),
                style_blend_override = COALESCE(?18, style_blend_override),
                foreshadowing_ids = COALESCE(?19, foreshadowing_ids),
                source = COALESCE(?20, source),
                is_auto_generated = COALESCE(?21, is_auto_generated),
                updated_at = ?22
             WHERE id = ?1",
            params![
                id,
                updates.title,
                updates.dramatic_goal,
                updates.external_pressure,
                updates.conflict_type.as_ref().map(|c| c.to_string()),
                updates
                    .characters_present
                    .as_ref()
                    .map(|c| serde_json::to_string(c).unwrap()),
                updates
                    .character_conflicts
                    .as_ref()
                    .map(|c| serde_json::to_string(c).unwrap()),
                updates.content,
                updates.setting_location,
                updates.setting_time,
                updates.setting_atmosphere,
                updates.previous_scene_id,
                updates.next_scene_id,
                updates.confidence_score,
                updates.execution_stage,
                updates.outline_content,
                updates.draft_content,
                updates.style_blend_override,
                updates
                    .foreshadowing_ids
                    .as_ref()
                    .map(|c| serde_json::to_string(c).unwrap()),
                updates.source.as_ref(),
                updates.is_auto_generated.map(|v| v as i32),
                &now
            ],
        )?;

        // Phase 1: 仅同步标题到关联 Chapter（标题是共享元数据）。
        // 内容不再反向同步到 chapters.content（Scene 为唯一内容真相源）。
        if updates.title.is_some() {
            let chapter_id: Option<String> = tx
                .query_row("SELECT chapter_id FROM scenes WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()?;
            if let Some(cid) = chapter_id {
                tx.execute(
                    "UPDATE chapters SET title = COALESCE(?2, title), updated_at = ?3 WHERE id = ?1",
                    params![cid, &updates.title, &now],
                )?;
            }
        }

        // W2-F3: 世界-场景自动关联 — 场景 setting 变更同步到 world_building
        if updates.setting_location.is_some()
            || updates.setting_time.is_some()
            || updates.setting_atmosphere.is_some()
        {
            let story_id: String =
                tx.query_row("SELECT story_id FROM scenes WHERE id = ?1", [id], |row| {
                    row.get(0)
                })?;
            self.sync_scene_settings_to_world_building(
                &tx,
                &story_id,
                updates.setting_location.as_deref(),
                updates.setting_time.as_deref(),
                updates.setting_atmosphere.as_deref(),
            )?;
        }

        Ok(count)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        // 删除 scene 时无需清理 chapter 表（chapter 不持有 scene_id 外键）。

        // W2-F3: 获取 setting 信息用于世界构建清理
        let (story_id, old_location, old_atmosphere): (String, Option<String>, Option<String>) = tx
            .query_row(
                "SELECT story_id, setting_location, setting_atmosphere FROM scenes WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;

        let count = tx.execute("DELETE FROM scenes WHERE id = ?1", [id])?;

        // W2-F3: 世界-场景自动关联 — 清理无引用的自动生成规则
        self.cleanup_world_building_after_delete(
            &tx,
            &story_id,
            old_location.as_deref(),
            old_atmosphere.as_deref(),
        )?;

        tx.commit()?;
        Ok(count)
    }

    pub fn update_sequence(&self, id: &str, new_sequence: i32) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let count = conn.execute(
            "UPDATE scenes SET sequence_number = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, new_sequence, now],
        )?;
        Ok(count)
    }

    // ==================== 世界-场景自动关联 (W2-F3) ====================

    /// 将场景的 setting 信息同步到 world_building（"场景增世界增"）
    fn sync_scene_settings_to_world_building(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        setting_location: Option<&str>,
        setting_time: Option<&str>,
        setting_atmosphere: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        if setting_location.is_none() && setting_time.is_none() && setting_atmosphere.is_none() {
            return Ok(());
        }

        // 1. 获取或创建 world_building
        let (wb_id, current_rules_json, current_history): (String, String, Option<String>) =
            match tx
                .query_row(
                    "SELECT id, rules, history FROM world_buildings WHERE story_id = ?1",
                    [story_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    },
                )
                .optional()?
            {
                Some(row) => row,
                None => {
                    let id = Uuid::new_v4().to_string();
                    let now = Local::now().to_rfc3339();
                    tx.execute(
                        "INSERT INTO world_buildings (id, story_id, concept, rules, history, \
                         cultures, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            &id,
                            story_id,
                            "Auto-generated world building",
                            "[]",
                            "",
                            "[]",
                            &now,
                            &now
                        ],
                    )?;
                    (id, "[]".to_string(), None)
                }
            };

        let mut rules: Vec<WorldRule> =
            serde_json::from_str(&current_rules_json).unwrap_or_default();
        let mut rules_changed = false;

        // 2. setting_location -> Physical 规则
        if let Some(loc) = setting_location {
            let loc = loc.trim();
            if !loc.is_empty() {
                let exists = rules
                    .iter()
                    .any(|r| r.name == loc && r.rule_type == RuleType::Physical);
                if !exists {
                    rules.push(WorldRule {
                        id: Uuid::new_v4().to_string(),
                        name: loc.to_string(),
                        description: Some("(auto-generated from scene)".to_string()),
                        rule_type: RuleType::Physical,
                        importance: 5,
                    });
                    rules_changed = true;
                }
            }
        }

        // 3. setting_atmosphere -> Cultural 规则
        if let Some(atm) = setting_atmosphere {
            let atm = atm.trim();
            if !atm.is_empty() {
                let exists = rules
                    .iter()
                    .any(|r| r.name == atm && r.rule_type == RuleType::Cultural);
                if !exists {
                    rules.push(WorldRule {
                        id: Uuid::new_v4().to_string(),
                        name: atm.to_string(),
                        description: Some("(auto-generated from scene)".to_string()),
                        rule_type: RuleType::Cultural,
                        importance: 5,
                    });
                    rules_changed = true;
                }
            }
        }

        // 4. 保存 rules 变更
        if rules_changed {
            let rules_json = serde_json::to_string(&rules).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "UPDATE world_buildings SET rules = ?1, updated_at = ?2 WHERE id = ?3",
                params![rules_json, Local::now().to_rfc3339(), &wb_id],
            )?;
        }

        // 5. setting_time -> 追加到 history（去重）
        if let Some(time) = setting_time {
            let time = time.trim();
            if !time.is_empty() {
                let fragment = format!("[时间设定] {}\n", time);
                let new_history = match current_history {
                    Some(ref h) if h.contains(&fragment) => h.clone(),
                    Some(h) => format!("{}{}", h, fragment),
                    None => fragment,
                };
                tx.execute(
                    "UPDATE world_buildings SET history = ?1, updated_at = ?2 WHERE id = ?3",
                    params![new_history, Local::now().to_rfc3339(), &wb_id],
                )?;
            }
        }

        Ok(())
    }

    /// 场景删除后清理 world_building 中无引用的自动生成规则（"场景减世界减"）
    fn cleanup_world_building_after_delete(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        old_location: Option<&str>,
        old_atmosphere: Option<&str>,
    ) -> Result<(), rusqlite::Error> {
        if old_location.is_none() && old_atmosphere.is_none() {
            return Ok(());
        }

        let (wb_id_opt, rules_json): (Option<String>, String) = match tx
            .query_row(
                "SELECT id, rules FROM world_buildings WHERE story_id = ?1",
                [story_id],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            Some(row) => row,
            None => return Ok(()),
        };

        let wb_id = match wb_id_opt {
            Some(id) => id,
            None => return Ok(()),
        };

        let mut rules: Vec<WorldRule> = serde_json::from_str(&rules_json).unwrap_or_default();
        let original_len = rules.len();

        rules.retain(|r| {
            // 只处理自动生成的规则
            let is_auto = r
                .description
                .as_deref()
                .unwrap_or("")
                .contains("auto-generated");
            if !is_auto {
                return true;
            }

            let should_check = match r.rule_type {
                RuleType::Physical => old_location.map(|loc| r.name == loc).unwrap_or(false),
                RuleType::Cultural => old_atmosphere.map(|atm| r.name == atm).unwrap_or(false),
                _ => false,
            };

            if !should_check {
                return true;
            }

            // 检查是否还有其他场景引用该 setting
            let column = match r.rule_type {
                RuleType::Physical => "setting_location",
                RuleType::Cultural => "setting_atmosphere",
                _ => return true,
            };

            let still_used = tx
                .query_row(
                    &format!(
                        "SELECT 1 FROM scenes WHERE story_id = ?1 AND {} = ?2 LIMIT 1",
                        column
                    ),
                    params![story_id, &r.name],
                    |_| Ok(true),
                )
                .optional()
                .unwrap_or(None)
                .is_some();

            // 如果仍被使用则保留，否则删除（retain 中 false 表示删除）
            still_used
        });

        if rules.len() < original_len {
            let rules_json = serde_json::to_string(&rules).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "UPDATE world_buildings SET rules = ?1, updated_at = ?2 WHERE id = ?3",
                params![rules_json, Local::now().to_rfc3339(), &wb_id],
            )?;
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SceneUpdate {
    pub title: Option<String>,
    pub dramatic_goal: Option<String>,
    pub external_pressure: Option<String>,
    pub conflict_type: Option<ConflictType>,
    pub characters_present: Option<Vec<String>>,
    pub character_conflicts: Option<Vec<CharacterConflict>>,
    pub content: Option<String>,
    pub setting_location: Option<String>,
    pub setting_time: Option<String>,
    pub setting_atmosphere: Option<String>,
    pub previous_scene_id: Option<String>,
    pub next_scene_id: Option<String>,
    pub confidence_score: Option<f32>,
    pub execution_stage: Option<String>,
    pub outline_content: Option<String>,
    pub draft_content: Option<String>,
    pub style_blend_override: Option<String>,
    pub foreshadowing_ids: Option<Vec<String>>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub is_auto_generated: Option<bool>,
}
