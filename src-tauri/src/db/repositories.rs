#![allow(dead_code)]
//! Repository 层

use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json;
use uuid::Uuid;

use super::{
    AgentBotConfig, AnchorType, ChangeStatus, ChangeTrack, ChangeType, Chapter, Character,
    CharacterConflict, CharacterState, CommentMessage, CommentThread, CommentThreadWithMessages,
    ConflictType, CreateChapterRequest, CreateCharacterRequest, CreateStoryRequest, CreatorType,
    Culture, DbPool, DynamicTrait, Entity, LlmStudioConfig, OAuthAccount, Relation, RuleType,
    Scene, SceneAnnotation, SceneVersion, Session, Story, StoryStyleConfig, StorySummary,
    StudioConfig, TextAnnotation, ThreadStatus, UiStudioConfig, UpdateStoryRequest, User, UserInfo,
    WorldBuilding, WorldRule, WritingStyle,
};

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
             character_conflicts, execution_stage, chapter_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9)",
            params![
                &id,
                story_id,
                sequence_number,
                title,
                "[]",
                "[]",
                "drafting",
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
                    content, previous_scene_id, next_scene_id, model_used, cost, created_at, \
             updated_at, confidence_score,
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

                let chars_json: String = row.get(7)?;
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row.get(8)?;
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_str: String = row.get(17)?;
                let updated_str: String = row.get(18)?;
                let confidence_score: Option<f32> = row.get(19)?;
                let execution_stage: Option<String> = row.get(20)?;
                let outline_content: Option<String> = row.get(21)?;
                let draft_content: Option<String> = row.get(22)?;
                let foreshadowing_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(24)?
                    .and_then(|s: String| serde_json::from_str(&s).ok());

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
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score,
                    execution_stage,
                    outline_content,
                    draft_content,
                    style_blend_override: row.get(23)?,
                    foreshadowing_ids,
                    chapter_id: row.get::<_, Option<String>>(25)?,
                    narrative_intensity: row.get(26)?,
                    narrative_sentiment: row.get(27)?,
                    narrative_event_types: row.get(28)?,
                    narrative_preceding_scene_id: row.get(29)?,
                    narrative_following_scene_id: row.get(30)?,
                    act_number: row.get(31)?,
                    position_in_act: row.get(32)?,
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
                    previous_scene_id, next_scene_id, model_used, cost, created_at, updated_at, \
             confidence_score,
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

                let chars_json: String = row.get(7)?;
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row.get(8)?;
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_str: String = row.get(16)?;
                let updated_str: String = row.get(17)?;
                let confidence_score: Option<f32> = row.get(18)?;
                let execution_stage: Option<String> = row.get(19)?;
                let foreshadowing_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(21)?
                    .and_then(|s: String| serde_json::from_str(&s).ok());

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
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score,
                    execution_stage,
                    outline_content: None,
                    draft_content: None,
                    style_blend_override: row.get(20)?,
                    foreshadowing_ids,
                    chapter_id: row.get::<_, Option<String>>(22)?,
                    narrative_intensity: row.get(23)?,
                    narrative_sentiment: row.get(24)?,
                    narrative_event_types: row.get(25)?,
                    narrative_preceding_scene_id: row.get(26)?,
                    narrative_following_scene_id: row.get(27)?,
                    act_number: row.get(28)?,
                    position_in_act: row.get(29)?,
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
                    content, previous_scene_id, next_scene_id, model_used, cost, created_at, \
             updated_at, confidence_score,
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

                let chars_json: String = row.get(7)?;
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row.get(8)?;
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_str: String = row.get(17)?;
                let updated_str: String = row.get(18)?;
                let confidence_score: Option<f32> = row.get(19)?;
                let execution_stage: Option<String> = row.get(20)?;
                let outline_content: Option<String> = row.get(21)?;
                let draft_content: Option<String> = row.get(22)?;
                let foreshadowing_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(24)?
                    .and_then(|s: String| serde_json::from_str(&s).ok());

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
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score,
                    execution_stage,
                    outline_content,
                    draft_content,
                    style_blend_override: row.get(23)?,
                    foreshadowing_ids,
                    chapter_id: row.get::<_, Option<String>>(25)?,
                    narrative_intensity: row.get(26)?,
                    narrative_sentiment: row.get(27)?,
                    narrative_event_types: row.get(28)?,
                    narrative_preceding_scene_id: row.get(29)?,
                    narrative_following_scene_id: row.get(30)?,
                    act_number: row.get(31)?,
                    position_in_act: row.get(32)?,
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
                    content, previous_scene_id, next_scene_id, model_used, cost, created_at, \
             updated_at, confidence_score,
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

                let chars_json: String = row.get(7)?;
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row.get(8)?;
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_str: String = row.get(17)?;
                let updated_str: String = row.get(18)?;
                let confidence_score: Option<f32> = row.get(19)?;
                let execution_stage: Option<String> = row.get(20)?;
                let outline_content: Option<String> = row.get(21)?;
                let draft_content: Option<String> = row.get(22)?;
                let foreshadowing_ids: Option<Vec<String>> = row
                    .get::<_, Option<String>>(24)?
                    .and_then(|s: String| serde_json::from_str(&s).ok());

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
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score,
                    execution_stage,
                    outline_content,
                    draft_content,
                    style_blend_override: row.get(23)?,
                    foreshadowing_ids,
                    chapter_id: row.get::<_, Option<String>>(25)?,
                    narrative_intensity: row.get(26)?,
                    narrative_sentiment: row.get(27)?,
                    narrative_event_types: row.get(28)?,
                    narrative_preceding_scene_id: row.get(29)?,
                    narrative_following_scene_id: row.get(30)?,
                    act_number: row.get(31)?,
                    position_in_act: row.get(32)?,
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
                updated_at = ?20
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
                &now
            ],
        )?;

        // Sync associated chapter if title or content changed
        if updates.title.is_some() || updates.content.is_some() {
            let chapter_id: Option<String> = tx
                .query_row("SELECT chapter_id FROM scenes WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()?;
            if let Some(cid) = chapter_id {
                tx.execute(
                    "UPDATE chapters SET title = COALESCE(?2, title), content = COALESCE(?3, \
                     content), updated_at = ?4 WHERE id = ?1",
                    params![cid, &updates.title, &updates.content, &now],
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
}

// ==================== Scene Version Repository (新增) ====================

pub struct SceneVersionRepository {
    pool: DbPool,
}

impl SceneVersionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 创建场景版本快照
    pub fn create_version(
        &self,
        scene: &Scene,
        change_summary: &str,
        created_by: CreatorType,
        model_used: Option<&str>,
        confidence_score: Option<f32>,
    ) -> Result<SceneVersion, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        // 获取当前版本号
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let version_number: i32 = conn.query_row(
            "SELECT COALESCE(MAX(version_number), 0) + 1 FROM scene_versions WHERE scene_id = ?1",
            [&scene.id],
            |row| row.get(0),
        )?;

        // 获取上一个版本ID
        let previous_version_id: Option<String> = conn
            .query_row(
                "SELECT id FROM scene_versions WHERE scene_id = ?1 ORDER BY version_number DESC \
                 LIMIT 1",
                [&scene.id],
                |row| row.get(0),
            )
            .ok();

        let word_count = scene.content.as_ref().map(|c| c.len() as i32).unwrap_or(0);

        conn.execute(
            "INSERT INTO scene_versions (id, scene_id, version_number, title, content, \
             dramatic_goal, 
             external_pressure, conflict_type, characters_present, character_conflicts,
             setting_location, setting_time, setting_atmosphere, word_count, change_summary,
             created_by, model_used, confidence_score, previous_version_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
             ?18, ?19, ?20)",
            params![
                &id,
                &scene.id,
                version_number,
                scene.title,
                scene.content,
                scene.dramatic_goal,
                scene.external_pressure,
                scene.conflict_type.as_ref().map(|c| c.to_string()),
                serde_json::to_string(&scene.characters_present).unwrap(),
                serde_json::to_string(&scene.character_conflicts).unwrap(),
                scene.setting_location,
                scene.setting_time,
                scene.setting_atmosphere,
                word_count,
                change_summary,
                created_by.to_string(),
                model_used,
                confidence_score,
                previous_version_id,
                now.to_rfc3339()
            ],
        )?;

        // 标记上一个版本为被取代
        if let Some(prev_id) = &previous_version_id {
            conn.execute(
                "UPDATE scene_versions SET superseded_by = ?1 WHERE id = ?2",
                params![&id, prev_id],
            )?;
        }

        let version = SceneVersion {
            id,
            scene_id: scene.id.clone(),
            version_number,
            title: scene.title.clone(),
            content: scene.content.clone(),
            dramatic_goal: scene.dramatic_goal.clone(),
            external_pressure: scene.external_pressure.clone(),
            conflict_type: scene.conflict_type.clone(),
            characters_present: scene.characters_present.clone(),
            character_conflicts: scene.character_conflicts.clone(),
            setting_location: scene.setting_location.clone(),
            setting_time: scene.setting_time.clone(),
            setting_atmosphere: scene.setting_atmosphere.clone(),
            word_count,
            change_summary: change_summary.to_string(),
            created_by,
            model_used: model_used.map(|s| s.to_string()),
            confidence_score,
            previous_version_id,
            superseded_by: None,
            created_at: now,
        };

        Ok(version)
    }

    /// 获取场景的所有版本
    pub fn get_versions(&self, scene_id: &str) -> Result<Vec<SceneVersion>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, version_number, title, content, dramatic_goal, \
             external_pressure,
                    conflict_type, characters_present, character_conflicts, setting_location, \
             setting_time,
                    setting_atmosphere, word_count, change_summary, created_by, model_used, \
             confidence_score,
                    previous_version_id, superseded_by, created_at
             FROM scene_versions WHERE scene_id = ?1 ORDER BY version_number DESC",
        )?;

        let versions = stmt
            .query_map([scene_id], |row| {
                let conflict_type_str: Option<String> = row.get(7)?;
                let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());

                let chars_json: String = row.get(8)?;
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row.get(9)?;
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_by_str: String = row.get(15)?;
                let created_by = created_by_str.parse().unwrap_or(CreatorType::System);

                let created_str: String = row.get(20)?;

                Ok(SceneVersion {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    version_number: row.get(2)?,
                    title: row.get(3)?,
                    content: row.get(4)?,
                    dramatic_goal: row.get(5)?,
                    external_pressure: row.get(6)?,
                    conflict_type,
                    characters_present,
                    character_conflicts,
                    setting_location: row.get(10)?,
                    setting_time: row.get(11)?,
                    setting_atmosphere: row.get(12)?,
                    word_count: row.get(13)?,
                    change_summary: row.get(14)?,
                    created_by,
                    model_used: row.get(16)?,
                    confidence_score: row.get(17)?,
                    previous_version_id: row.get(18)?,
                    superseded_by: row.get(19)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(versions)
    }

    /// 获取特定版本
    pub fn get_version(&self, version_id: &str) -> Result<Option<SceneVersion>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, version_number, title, content, dramatic_goal, \
             external_pressure,
                    conflict_type, characters_present, character_conflicts, setting_location, \
             setting_time,
                    setting_atmosphere, word_count, change_summary, created_by, model_used, \
             confidence_score,
                    previous_version_id, superseded_by, created_at
             FROM scene_versions WHERE id = ?1",
        )?;

        let version = stmt
            .query_row([version_id], |row| {
                let conflict_type_str: Option<String> = row.get(7)?;
                let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());

                let chars_json: String = row.get(8)?;
                let characters_present: Vec<String> =
                    serde_json::from_str(&chars_json).unwrap_or_default();

                let conflicts_json: String = row.get(9)?;
                let character_conflicts: Vec<CharacterConflict> =
                    serde_json::from_str(&conflicts_json).unwrap_or_default();

                let created_by_str: String = row.get(15)?;
                let created_by = created_by_str.parse().unwrap_or(CreatorType::System);

                let created_str: String = row.get(20)?;

                Ok(SceneVersion {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    version_number: row.get(2)?,
                    title: row.get(3)?,
                    content: row.get(4)?,
                    dramatic_goal: row.get(5)?,
                    external_pressure: row.get(6)?,
                    conflict_type,
                    characters_present,
                    character_conflicts,
                    setting_location: row.get(10)?,
                    setting_time: row.get(11)?,
                    setting_atmosphere: row.get(12)?,
                    word_count: row.get(13)?,
                    change_summary: row.get(14)?,
                    created_by,
                    model_used: row.get(16)?,
                    confidence_score: row.get(17)?,
                    previous_version_id: row.get(18)?,
                    superseded_by: row.get(19)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(version)
    }

    /// 删除版本
    pub fn delete_version(&self, version_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM scene_versions WHERE id = ?1", [version_id])?;
        Ok(count)
    }

    /// 获取场景版本数量
    pub fn get_version_count(&self, scene_id: &str) -> Result<i32, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM scene_versions WHERE scene_id = ?1",
            [scene_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

// ==================== WorldBuilding Repository ====================

pub struct WorldBuildingRepository {
    pool: DbPool,
}

impl WorldBuildingRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        concept: &str,
    ) -> Result<WorldBuilding, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        tx.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, \
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &id,
                story_id,
                concept,
                "[]",
                "",
                "[]",
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(WorldBuilding {
            id,
            story_id: story_id.to_string(),
            concept: concept.to_string(),
            rules: vec![],
            history: None,
            cultures: vec![],
            created_at: now,
            updated_at: now,
        })
    }

    pub fn create(&self, story_id: &str, concept: &str) -> Result<WorldBuilding, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let wb = self.create_in_tx(&tx, story_id, concept)?;
        tx.commit()?;
        Ok(wb)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, concept, rules, history, cultures, created_at, updated_at
             FROM world_buildings WHERE id = ?1",
        )?;

        let wb = stmt
            .query_row([id], |row| {
                let rules_json: String = row.get(3)?;
                let rules: Vec<WorldRule> = serde_json::from_str(&rules_json).unwrap_or_default();

                let cultures_json: String = row.get(5)?;
                let cultures: Vec<Culture> =
                    serde_json::from_str(&cultures_json).unwrap_or_default();

                let created_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;

                Ok(WorldBuilding {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    concept: row.get(2)?,
                    rules,
                    history: row.get(4)?,
                    cultures,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(wb)
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, concept, rules, history, cultures, created_at, updated_at
             FROM world_buildings WHERE story_id = ?1",
        )?;

        let wb = stmt
            .query_row([story_id], |row| {
                let rules_json: String = row.get(3)?;
                let rules: Vec<WorldRule> = serde_json::from_str(&rules_json).unwrap_or_default();

                let cultures_json: String = row.get(5)?;
                let cultures: Vec<Culture> =
                    serde_json::from_str(&cultures_json).unwrap_or_default();

                let created_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;

                Ok(WorldBuilding {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    concept: row.get(2)?,
                    rules,
                    history: row.get(4)?,
                    cultures,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(wb)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM world_buildings WHERE id = ?1", params![id])
    }

    pub fn update_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        id: &str,
        concept: Option<&str>,
        rules: Option<&[WorldRule]>,
        history: Option<&str>,
        cultures: Option<&[Culture]>,
    ) -> Result<usize, rusqlite::Error> {
        let now = Local::now().to_rfc3339();

        let count = tx.execute(
            "UPDATE world_buildings SET
                concept = COALESCE(?2, concept),
                rules = COALESCE(?3, rules),
                history = COALESCE(?4, history),
                cultures = COALESCE(?5, cultures),
                updated_at = ?6
             WHERE id = ?1",
            params![
                id,
                concept,
                rules.map(|r| serde_json::to_string(r).unwrap()),
                history,
                cultures.map(|c| serde_json::to_string(c).unwrap()),
                now
            ],
        )?;
        Ok(count)
    }

    pub fn update(
        &self,
        id: &str,
        concept: Option<&str>,
        rules: Option<&[WorldRule]>,
        history: Option<&str>,
        cultures: Option<&[Culture]>,
    ) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let count = self.update_in_tx(&tx, id, concept, rules, history, cultures)?;
        tx.commit()?;
        Ok(count)
    }
}

// ==================== WritingStyle Repository ====================

pub struct WritingStyleRepository {
    pool: DbPool,
}

impl WritingStyleRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        name: Option<&str>,
    ) -> Result<WritingStyle, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        tx.execute(
            "INSERT INTO writing_styles (id, story_id, name, description, tone, pacing,
             vocabulary_level, sentence_structure, custom_rules, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &id,
                story_id,
                name,
                "",
                "",
                "",
                "",
                "",
                "[]",
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(WritingStyle {
            id,
            story_id: story_id.to_string(),
            name: name.map(|s| s.to_string()),
            description: None,
            tone: None,
            pacing: None,
            vocabulary_level: None,
            sentence_structure: None,
            custom_rules: vec![],
            created_at: now,
            updated_at: now,
        })
    }

    pub fn create(
        &self,
        story_id: &str,
        name: Option<&str>,
    ) -> Result<WritingStyle, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let ws = self.create_in_tx(&tx, story_id, name)?;
        tx.commit()?;
        Ok(ws)
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Option<WritingStyle>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, description, tone, pacing, vocabulary_level, 
                    sentence_structure, custom_rules, created_at, updated_at 
             FROM writing_styles WHERE story_id = ?1",
        )?;

        let style = stmt
            .query_row([story_id], |row| {
                let rules_json: String = row.get(8)?;
                let custom_rules: Vec<String> =
                    serde_json::from_str(&rules_json).unwrap_or_default();

                let created_str: String = row.get(9)?;
                let updated_str: String = row.get(10)?;

                Ok(WritingStyle {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    tone: row.get(4)?,
                    pacing: row.get(5)?,
                    vocabulary_level: row.get(6)?,
                    sentence_structure: row.get(7)?,
                    custom_rules,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(style)
    }

    pub fn update_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        id: &str,
        updates: &WritingStyleUpdate,
    ) -> Result<usize, rusqlite::Error> {
        let now = Local::now().to_rfc3339();

        let count = tx.execute(
            "UPDATE writing_styles SET
                name = COALESCE(?2, name),
                description = COALESCE(?3, description),
                tone = COALESCE(?4, tone),
                pacing = COALESCE(?5, pacing),
                vocabulary_level = COALESCE(?6, vocabulary_level),
                sentence_structure = COALESCE(?7, sentence_structure),
                custom_rules = COALESCE(?8, custom_rules),
                updated_at = ?9
             WHERE id = ?1",
            params![
                id,
                updates.name,
                updates.description,
                updates.tone,
                updates.pacing,
                updates.vocabulary_level,
                updates.sentence_structure,
                updates
                    .custom_rules
                    .as_ref()
                    .map(|r| serde_json::to_string(r).unwrap()),
                now
            ],
        )?;
        Ok(count)
    }

    pub fn update(&self, id: &str, updates: &WritingStyleUpdate) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let count = self.update_in_tx(&tx, id, updates)?;
        tx.commit()?;
        Ok(count)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WritingStyleUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub vocabulary_level: Option<String>,
    pub sentence_structure: Option<String>,
    pub custom_rules: Option<Vec<String>>,
}

// ==================== StudioConfig Repository ====================

pub struct StudioConfigRepository {
    pool: DbPool,
}

impl StudioConfigRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 创建默认配置 (兼容旧接口)
    pub fn create(&self, story_id: &str) -> Result<StudioConfig, rusqlite::Error> {
        self.create_default(story_id, "新建工作室")
    }

    pub fn create_default(
        &self,
        story_id: &str,
        title: &str,
    ) -> Result<StudioConfig, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let llm_config = LlmStudioConfig {
            default_provider: "openai".to_string(),
            default_model: "gpt-4".to_string(),
            generation_temperature: 0.7,
            max_tokens: 4096,
            profiles: vec![],
        };

        let ui_config = UiStudioConfig {
            frontstage_font_size: 18,
            frontstage_font_family: "Noto Serif SC".to_string(),
            frontstage_line_height: 1.8,
            frontstage_paper_color: "#f5f4ed".to_string(),
            frontstage_text_color: "#2c2c2c".to_string(),
            backstage_theme: "dark".to_string(),
            backstage_accent_color: "#6366f1".to_string(),
        };

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO studio_configs (id, story_id, pen_name, llm_config, ui_config, 
             agent_bots, frontstage_theme, backstage_theme, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &id,
                story_id,
                title,
                serde_json::to_string(&llm_config).unwrap(),
                serde_json::to_string(&ui_config).unwrap(),
                "[]",
                "paper",
                "dark",
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(StudioConfig {
            id,
            story_id: story_id.to_string(),
            pen_name: Some(title.to_string()),
            llm_config,
            ui_config,
            agent_bots: vec![],
            frontstage_theme: Some("paper".to_string()),
            backstage_theme: Some("dark".to_string()),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Option<StudioConfig>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, pen_name, llm_config, ui_config, agent_bots, 
                    frontstage_theme, backstage_theme, created_at, updated_at 
             FROM studio_configs WHERE story_id = ?1",
        )?;

        let config = stmt
            .query_row([story_id], |row| {
                let llm_json: String = row.get(3)?;
                let llm_config: LlmStudioConfig =
                    serde_json::from_str(&llm_json).unwrap_or_default();

                let ui_json: String = row.get(4)?;
                let ui_config: UiStudioConfig = serde_json::from_str(&ui_json).unwrap_or_default();

                let bots_json: String = row.get(5)?;
                let agent_bots: Vec<AgentBotConfig> =
                    serde_json::from_str(&bots_json).unwrap_or_default();

                let created_str: String = row.get(8)?;
                let updated_str: String = row.get(9)?;

                Ok(StudioConfig {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    pen_name: row.get(2)?,
                    llm_config,
                    ui_config,
                    agent_bots,
                    frontstage_theme: row.get(6)?,
                    backstage_theme: row.get(7)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(config)
    }

    /// 更新配置 (兼容旧接口)
    pub fn update(
        &self,
        id: &str,
        _pen_name: Option<&str>,
        llm_config: Option<&LlmStudioConfig>,
        ui_config: Option<&UiStudioConfig>,
        agent_bots: Option<&[AgentBotConfig]>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE studio_configs SET 
                llm_config = COALESCE(?2, llm_config),
                ui_config = COALESCE(?3, ui_config),
                agent_bots = COALESCE(?4, agent_bots),
                updated_at = ?5
             WHERE id = ?1",
            params![
                id,
                llm_config.map(|c| serde_json::to_string(c).unwrap()),
                ui_config.map(|c| serde_json::to_string(c).unwrap()),
                agent_bots.map(|b| serde_json::to_string(&b.to_vec()).unwrap()),
                now
            ],
        )?;
        Ok(count)
    }

    /// 更新主题
    pub fn update_themes(
        &self,
        id: &str,
        frontstage_theme: Option<&str>,
        backstage_theme: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE studio_configs SET 
                frontstage_theme = COALESCE(?2, frontstage_theme),
                backstage_theme = COALESCE(?3, backstage_theme),
                updated_at = ?4
             WHERE id = ?1",
            params![id, frontstage_theme, backstage_theme, now],
        )?;
        Ok(count)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct StudioConfigUpdate {
    pub pen_name: Option<String>,
    pub llm_config: Option<LlmStudioConfig>,
    pub ui_config: Option<UiStudioConfig>,
    pub agent_bots: Option<Vec<AgentBotConfig>>,
    pub frontstage_theme: Option<String>,
    pub backstage_theme: Option<String>,
}

// ==================== KnowledgeGraph Repository ====================

pub struct KnowledgeGraphRepository {
    pool: DbPool,
}

impl KnowledgeGraphRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_entity_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        name: &str,
        entity_type: &str,
        attributes: &serde_json::Value,
        embedding: Option<Vec<f32>>,
    ) -> Result<Entity, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let embedding_blob = embedding.as_ref().map(|vec| {
            vec.iter()
                .flat_map(|&f| f.to_le_bytes().to_vec())
                .collect::<Vec<u8>>()
        });

        tx.execute(
            "INSERT INTO kg_entities (id, story_id, name, entity_type, attributes, embedding, \
             first_seen, last_updated, is_archived)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            params![
                &id,
                story_id,
                name,
                entity_type,
                attributes.to_string(),
                embedding_blob,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(Entity {
            id,
            story_id: story_id.to_string(),
            name: name.to_string(),
            entity_type: entity_type.parse().map_err(|_| {
                rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
            })?,
            attributes: attributes.clone(),
            embedding,
            first_seen: now,
            last_updated: now,
            confidence_score: None,
            access_count: 0,
            last_accessed: None,
            is_archived: false,
            archived_at: None,
        })
    }

    pub fn create_entity(
        &self,
        story_id: &str,
        name: &str,
        entity_type: &str,
        attributes: &serde_json::Value,
        embedding: Option<Vec<f32>>,
    ) -> Result<Entity, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let entity =
            self.create_entity_in_tx(&tx, story_id, name, entity_type, attributes, embedding)?;
        tx.commit()?;
        Ok(entity)
    }

    pub fn get_entities_by_story(&self, story_id: &str) -> Result<Vec<Entity>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, \
             last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at
             FROM kg_entities WHERE story_id = ?1 AND is_archived = 0",
        )?;

        let entities = stmt
            .query_map([story_id], |row| {
                let type_str: String = row.get(3)?;
                let entity_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
                })?;

                let attrs_json: String = row.get(4)?;
                let attributes: serde_json::Value =
                    serde_json::from_str(&attrs_json).unwrap_or_default();

                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
                        .collect()
                });

                let first_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;
                let last_accessed: Option<String> = row.get(10)?;
                let is_archived: i32 = row.get(11)?;
                let archived_at: Option<String> = row.get(12)?;

                Ok(Entity {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    entity_type,
                    attributes,
                    embedding,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    last_updated: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: last_accessed.and_then(|s| s.parse().ok()),
                    is_archived: is_archived != 0,
                    archived_at: archived_at.and_then(|s| s.parse().ok()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entities)
    }

    pub fn get_archived_entities(&self, story_id: &str) -> Result<Vec<Entity>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, \
             last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at
             FROM kg_entities WHERE story_id = ?1 AND is_archived = 1",
        )?;

        let entities = stmt
            .query_map([story_id], |row| {
                let type_str: String = row.get(3)?;
                let entity_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
                })?;

                let attrs_json: String = row.get(4)?;
                let attributes: serde_json::Value =
                    serde_json::from_str(&attrs_json).unwrap_or_default();

                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
                        .collect()
                });

                let first_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;
                let last_accessed: Option<String> = row.get(10)?;
                let is_archived: i32 = row.get(11)?;
                let archived_at: Option<String> = row.get(12)?;

                Ok(Entity {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    entity_type,
                    attributes,
                    embedding,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    last_updated: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: last_accessed.and_then(|s| s.parse().ok()),
                    is_archived: is_archived != 0,
                    archived_at: archived_at.and_then(|s| s.parse().ok()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entities)
    }

    pub fn archive_entity(&self, entity_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE kg_entities SET is_archived = 1, archived_at = ?2, last_updated = ?2 WHERE id \
             = ?1",
            params![entity_id, now],
        )
    }

    pub fn restore_entity(&self, entity_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE kg_entities SET is_archived = 0, archived_at = NULL, last_updated = ?2 WHERE \
             id = ?1",
            params![entity_id, now],
        )
    }

    pub fn create_relation_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        source_id: &str,
        target_id: &str,
        relation_type: &str,
        strength: f32,
    ) -> Result<Relation, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        tx.execute(
            "INSERT INTO kg_relations (id, story_id, source_id, target_id, relation_type, \
             strength, evidence, first_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &id,
                story_id,
                source_id,
                target_id,
                relation_type,
                strength,
                "[]",
                now.to_rfc3339()
            ],
        )?;

        Ok(Relation {
            id,
            story_id: story_id.to_string(),
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            relation_type: relation_type.parse().map_err(|_| {
                rusqlite::Error::InvalidParameterName("Invalid relation type".to_string())
            })?,
            strength,
            evidence: vec![],
            first_seen: now,
            confidence_score: None,
        })
    }

    pub fn create_relation(
        &self,
        story_id: &str,
        source_id: &str,
        target_id: &str,
        relation_type: &str,
        strength: f32,
    ) -> Result<Relation, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let relation = self.create_relation_in_tx(
            &tx,
            story_id,
            source_id,
            target_id,
            relation_type,
            strength,
        )?;
        tx.commit()?;
        Ok(relation)
    }

    /// 批量保存 Ingest 生成的实体（已包含完整字段，直接 INSERT）
    pub fn save_entities_batch(&self, entities: &[Entity]) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let mut count = 0;
        for entity in entities {
            let embedding_blob = entity.embedding.as_ref().map(|vec| {
                vec.iter()
                    .flat_map(|&f| f.to_le_bytes().to_vec())
                    .collect::<Vec<u8>>()
            });
            tx.execute(
                "INSERT INTO kg_entities (id, story_id, name, entity_type, attributes, embedding, \
                 first_seen, last_updated, confidence_score, access_count, last_accessed, \
                 is_archived, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(id) DO UPDATE SET
                     name=excluded.name,
                     attributes=excluded.attributes,
                     embedding=excluded.embedding,
                     last_updated=excluded.last_updated,
                     confidence_score=excluded.confidence_score",
                params![
                    &entity.id,
                    &entity.story_id,
                    &entity.name,
                    entity.entity_type.to_string(),
                    entity.attributes.to_string(),
                    embedding_blob,
                    entity.first_seen.to_rfc3339(),
                    entity.last_updated.to_rfc3339(),
                    entity.confidence_score,
                    entity.access_count,
                    entity.last_accessed.map(|d| d.to_rfc3339()),
                    entity.is_archived as i32,
                    entity.archived_at.map(|d| d.to_rfc3339())
                ],
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    /// 批量保存 Ingest 生成的关系（已包含完整字段，直接 INSERT）
    pub fn save_relations_batch(&self, relations: &[Relation]) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let mut count = 0;
        for relation in relations {
            let evidence_json =
                serde_json::to_string(&relation.evidence).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "INSERT INTO kg_relations (id, story_id, source_id, target_id, relation_type, \
                 strength, evidence, first_seen, confidence_score)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     strength=excluded.strength,
                     evidence=excluded.evidence,
                     confidence_score=excluded.confidence_score",
                params![
                    &relation.id,
                    &relation.story_id,
                    &relation.source_id,
                    &relation.target_id,
                    relation.relation_type.to_string(),
                    relation.strength,
                    evidence_json,
                    relation.first_seen.to_rfc3339(),
                    relation.confidence_score
                ],
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn get_relations_by_entity(
        &self,
        entity_id: &str,
    ) -> Result<Vec<Relation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, source_id, target_id, relation_type, strength, evidence, \
             first_seen, confidence_score
             FROM kg_relations WHERE source_id = ?1 OR target_id = ?1",
        )?;

        let relations = stmt
            .query_map([entity_id], |row| {
                let type_str: String = row.get(4)?;
                let relation_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid relation type".to_string())
                })?;

                let evidence_json: String = row.get(6)?;
                let evidence: Vec<String> =
                    serde_json::from_str(&evidence_json).unwrap_or_default();

                let first_str: String = row.get(7)?;

                Ok(Relation {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    source_id: row.get(2)?,
                    target_id: row.get(3)?,
                    relation_type,
                    strength: row.get(5)?,
                    evidence,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(relations)
    }

    pub fn get_relations_by_story(&self, story_id: &str) -> Result<Vec<Relation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, source_id, target_id, relation_type, strength, evidence, \
             first_seen, confidence_score
             FROM kg_relations WHERE story_id = ?1",
        )?;

        let relations = stmt
            .query_map([story_id], |row| {
                let type_str: String = row.get(4)?;
                let relation_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid relation type".to_string())
                })?;

                let evidence_json: String = row.get(6)?;
                let evidence: Vec<String> =
                    serde_json::from_str(&evidence_json).unwrap_or_default();

                let first_str: String = row.get(7)?;

                Ok(Relation {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    source_id: row.get(2)?,
                    target_id: row.get(3)?,
                    relation_type,
                    strength: row.get(5)?,
                    evidence,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(relations)
    }

    pub fn get_entity_by_id(&self, entity_id: &str) -> Result<Option<Entity>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, \
             last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at
             FROM kg_entities WHERE id = ?1",
        )?;

        let entity = stmt
            .query_row([entity_id], |row| {
                let type_str: String = row.get(3)?;
                let entity_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
                })?;
                let attrs_json: String = row.get(4)?;
                let attributes: serde_json::Value =
                    serde_json::from_str(&attrs_json).unwrap_or_default();
                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
                        .collect()
                });

                let first_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;
                let last_accessed: Option<String> = row.get(10)?;
                let is_archived: i32 = row.get(11)?;
                let archived_at: Option<String> = row.get(12)?;

                Ok(Entity {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    entity_type,
                    attributes,
                    embedding,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    last_updated: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: last_accessed.and_then(|s| s.parse().ok()),
                    is_archived: is_archived != 0,
                    archived_at: archived_at.and_then(|s| s.parse().ok()),
                })
            })
            .optional()?;

        Ok(entity)
    }

    pub fn update_entity(
        &self,
        entity_id: &str,
        name: Option<&str>,
        attributes: Option<&serde_json::Value>,
        embedding: Option<Vec<f32>>,
    ) -> Result<Entity, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let entity = self
            .get_entity_by_id(entity_id)?
            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Entity not found".to_string()))?;

        let new_name = name.unwrap_or(&entity.name);
        let new_attributes = attributes.unwrap_or(&entity.attributes);
        let embedding_blob = embedding.as_ref().map(|vec| {
            vec.iter()
                .flat_map(|&f| f.to_le_bytes().to_vec())
                .collect::<Vec<u8>>()
        });

        conn.execute(
            "UPDATE kg_entities SET name = ?2, attributes = ?3, embedding = ?4, last_updated = ?5 \
             WHERE id = ?1",
            params![
                entity_id,
                new_name,
                new_attributes.to_string(),
                embedding_blob,
                now
            ],
        )?;

        Ok(Entity {
            id: entity.id,
            story_id: entity.story_id,
            name: new_name.to_string(),
            entity_type: entity.entity_type,
            attributes: new_attributes.clone(),
            embedding,
            first_seen: entity.first_seen,
            last_updated: Local::now(),
            confidence_score: entity.confidence_score,
            access_count: entity.access_count,
            last_accessed: entity.last_accessed,
            is_archived: entity.is_archived,
            archived_at: entity.archived_at,
        })
    }

    /// 根据名称查找实体（用于 QueryPipeline 图谱扩展）
    pub fn find_entity_by_name(&self, name: &str) -> Result<Option<Entity>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, \
             last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at
             FROM kg_entities WHERE name = ?1 AND is_archived = 0 LIMIT 1",
        )?;

        let entity = stmt
            .query_row([name], |row| {
                let type_str: String = row.get(3)?;
                let entity_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
                })?;
                let attrs_json: String = row.get(4)?;
                let attributes: serde_json::Value =
                    serde_json::from_str(&attrs_json).unwrap_or_default();
                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
                        .collect()
                });
                let first_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;
                let last_accessed: Option<String> = row.get(10)?;
                let is_archived: i32 = row.get(11)?;
                let archived_at: Option<String> = row.get(12)?;

                Ok(Entity {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    entity_type,
                    attributes,
                    embedding,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    last_updated: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: last_accessed.and_then(|s| s.parse().ok()),
                    is_archived: is_archived != 0,
                    archived_at: archived_at.and_then(|s| s.parse().ok()),
                })
            })
            .optional()?;

        Ok(entity)
    }

    /// 获取与指定实体相关的实体及其关系强度
    pub fn get_related_entities(
        &self,
        entity_id: &str,
        min_strength: f32,
    ) -> Result<Vec<(Entity, f32)>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, strength FROM kg_relations 
             WHERE (source_id = ?1 OR target_id = ?1) AND strength >= ?2",
        )?;

        let rows = stmt
            .query_map(params![entity_id, min_strength], |row| {
                let source_id: String = row.get(0)?;
                let target_id: String = row.get(1)?;
                let strength: f32 = row.get(2)?;
                let other_id = if source_id == entity_id {
                    target_id
                } else {
                    source_id
                };
                Ok((other_id, strength))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut results = Vec::new();
        for (other_id, strength) in rows {
            if let Ok(Some(entity)) = self.get_entity_by_id(&other_id) {
                results.push((entity, strength));
            }
        }

        Ok(results)
    }
}

// 为 KnowledgeGraphRepository 实现 memory::query::KnowledgeGraph trait
#[async_trait::async_trait]
impl crate::memory::query::KnowledgeGraph for KnowledgeGraphRepository {
    async fn find_entity_by_name(
        &self,
        name: &str,
    ) -> Result<crate::db::models::Entity, Box<dyn std::error::Error + Send + Sync>> {
        self.find_entity_by_name(name)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
            .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                "Entity not found".into()
            })
    }

    async fn get_related_entities(
        &self,
        entity_id: &str,
        min_strength: f32,
    ) -> Result<Vec<(crate::db::models::Entity, f32)>, Box<dyn std::error::Error + Send + Sync>>
    {
        self.get_related_entities(entity_id, min_strength)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

// ==================== 场景批注 Repository ====================

pub struct SceneAnnotationRepository {
    pool: DbPool,
}

impl SceneAnnotationRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_annotation(
        &self,
        scene_id: &str,
        story_id: &str,
        content: &str,
        annotation_type: &str,
    ) -> Result<SceneAnnotation, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO scene_annotations (id, scene_id, story_id, content, annotation_type, \
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &id,
                scene_id,
                story_id,
                content,
                annotation_type,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(SceneAnnotation {
            id,
            scene_id: scene_id.to_string(),
            story_id: story_id.to_string(),
            content: content.to_string(),
            annotation_type: annotation_type.parse().map_err(|_| {
                rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string())
            })?,
            created_at: now,
            updated_at: now,
            resolved_at: None,
        })
    }

    pub fn get_annotations_by_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<SceneAnnotation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, story_id, content, annotation_type, created_at, updated_at, \
             resolved_at
             FROM scene_annotations WHERE scene_id = ?1 ORDER BY created_at DESC",
        )?;

        let annotations = stmt
            .query_map([scene_id], |row| {
                let type_str: String = row.get(4)?;
                let annotation_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string())
                })?;
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                let resolved_str: Option<String> = row.get(7)?;

                Ok(SceneAnnotation {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    story_id: row.get(2)?,
                    content: row.get(3)?,
                    annotation_type,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    resolved_at: resolved_str.and_then(|s| s.parse().ok()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(annotations)
    }

    pub fn get_unresolved_annotations_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<SceneAnnotation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, story_id, content, annotation_type, created_at, updated_at, \
             resolved_at
             FROM scene_annotations WHERE story_id = ?1 AND resolved_at IS NULL ORDER BY \
             created_at DESC",
        )?;

        let annotations = stmt
            .query_map([story_id], |row| {
                let type_str: String = row.get(4)?;
                let annotation_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string())
                })?;
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                let resolved_str: Option<String> = row.get(7)?;

                Ok(SceneAnnotation {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    story_id: row.get(2)?,
                    content: row.get(3)?,
                    annotation_type,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    resolved_at: resolved_str.and_then(|s| s.parse().ok()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(annotations)
    }

    pub fn update_annotation(
        &self,
        annotation_id: &str,
        content: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE scene_annotations SET content = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, content, now],
        )
    }

    pub fn resolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE scene_annotations SET resolved_at = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, now, now],
        )
    }

    pub fn unresolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE scene_annotations SET resolved_at = NULL, updated_at = ?2 WHERE id = ?1",
            params![annotation_id, now],
        )
    }

    pub fn delete_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM scene_annotations WHERE id = ?1",
            params![annotation_id],
        )
    }
}

// ==================== 文本内联批注 Repository ====================

pub struct TextAnnotationRepository {
    pool: DbPool,
}

impl TextAnnotationRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_annotation(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_id: Option<&str>,
        content: &str,
        annotation_type: &str,
        from_pos: i32,
        to_pos: i32,
    ) -> Result<TextAnnotation, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO text_annotations (id, story_id, scene_id, chapter_id, content, \
             annotation_type, from_pos, to_pos, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &id,
                story_id,
                scene_id,
                chapter_id,
                content,
                annotation_type,
                from_pos,
                to_pos,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(TextAnnotation {
            id,
            story_id: story_id.to_string(),
            scene_id: scene_id.map(|s| s.to_string()),
            chapter_id: chapter_id.map(|s| s.to_string()),
            content: content.to_string(),
            annotation_type: annotation_type.parse().map_err(|_| {
                rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string())
            })?,
            from_pos,
            to_pos,
            created_at: now,
            updated_at: now,
            resolved_at: None,
        })
    }

    pub fn get_annotations_by_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, \
             to_pos, created_at, updated_at, resolved_at
             FROM text_annotations WHERE chapter_id = ?1 AND resolved_at IS NULL ORDER BY from_pos \
             ASC",
        )?;
        let rows = stmt.query([chapter_id])?;
        Self::map_annotations(rows)
    }

    pub fn get_annotations_by_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, \
             to_pos, created_at, updated_at, resolved_at
             FROM text_annotations WHERE scene_id = ?1 AND resolved_at IS NULL ORDER BY from_pos \
             ASC",
        )?;
        let rows = stmt.query([scene_id])?;
        Self::map_annotations(rows)
    }

    pub fn get_annotations_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, \
             to_pos, created_at, updated_at, resolved_at
             FROM text_annotations WHERE story_id = ?1 AND resolved_at IS NULL ORDER BY created_at \
             DESC",
        )?;
        let rows = stmt.query([story_id])?;
        Self::map_annotations(rows)
    }

    fn map_annotations(
        mut rows: rusqlite::Rows<'_>,
    ) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let mut annotations = Vec::new();
        while let Some(row) = rows.next()? {
            let type_str: String = row.get(5)?;
            let annotation_type = type_str.parse().map_err(|_| {
                rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string())
            })?;
            let created_str: String = row.get(8)?;
            let updated_str: String = row.get(9)?;
            let resolved_str: Option<String> = row.get(10)?;

            annotations.push(TextAnnotation {
                id: row.get(0)?,
                story_id: row.get(1)?,
                scene_id: row.get(2)?,
                chapter_id: row.get(3)?,
                content: row.get(4)?,
                annotation_type,
                from_pos: row.get(6)?,
                to_pos: row.get(7)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            });
        }
        Ok(annotations)
    }

    pub fn update_annotation(
        &self,
        annotation_id: &str,
        content: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE text_annotations SET content = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, content, now],
        )
    }

    pub fn resolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE text_annotations SET resolved_at = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, now, now],
        )
    }

    pub fn unresolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE text_annotations SET resolved_at = NULL, updated_at = ?2 WHERE id = ?1",
            params![annotation_id, now],
        )
    }

    pub fn delete_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM text_annotations WHERE id = ?1",
            params![annotation_id],
        )
    }
}

pub struct StorySummaryRepository {
    pool: DbPool,
}

impl StorySummaryRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_summary(
        &self,
        story_id: &str,
        summary_type: &str,
        content: &str,
    ) -> Result<StorySummary, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO story_summaries (id, story_id, summary_type, content, created_at, \
             updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &id,
                story_id,
                summary_type,
                content,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(StorySummary {
            id,
            story_id: story_id.to_string(),
            summary_type: summary_type.to_string(),
            content: content.to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_summaries_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<StorySummary>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, summary_type, content, created_at, updated_at
             FROM story_summaries WHERE story_id = ?1 ORDER BY updated_at DESC",
        )?;

        let rows = stmt.query_map([story_id], |row| {
            let created_str: String = row.get(4)?;
            let updated_str: String = row.get(5)?;
            Ok(StorySummary {
                id: row.get(0)?,
                story_id: row.get(1)?,
                summary_type: row.get(2)?,
                content: row.get(3)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?;

        rows.collect()
    }

    pub fn get_summary_by_type(
        &self,
        story_id: &str,
        summary_type: &str,
    ) -> Result<Option<StorySummary>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let result = conn
            .query_row(
                "SELECT id, story_id, summary_type, content, created_at, updated_at
             FROM story_summaries WHERE story_id = ?1 AND summary_type = ?2
             ORDER BY updated_at DESC LIMIT 1",
                params![story_id, summary_type],
                |row| {
                    let created_str: String = row.get(4)?;
                    let updated_str: String = row.get(5)?;
                    Ok(StorySummary {
                        id: row.get(0)?,
                        story_id: row.get(1)?,
                        summary_type: row.get(2)?,
                        content: row.get(3)?,
                        created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                        updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    })
                },
            )
            .optional()?;
        Ok(result)
    }

    pub fn update_summary(&self, id: &str, content: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE story_summaries SET content = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, content, now],
        )
    }

    pub fn delete_summary(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM story_summaries WHERE id = ?1", params![id])
    }
}

// ==================== ChangeTrack Repository (修订模式) ====================

pub struct ChangeTrackRepository {
    pool: DbPool,
}

impl ChangeTrackRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, track: &ChangeTrack) -> Result<ChangeTrack, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO change_tracks (id, scene_id, chapter_id, version_id, author_id, \
             author_name, change_type, from_pos, to_pos, content, status, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &track.id,
                &track.scene_id,
                &track.chapter_id,
                &track.version_id,
                &track.author_id,
                &track.author_name,
                format!("{:?}", track.change_type),
                track.from_pos,
                track.to_pos,
                &track.content,
                format!("{:?}", track.status),
                track.created_at.to_rfc3339(),
                track.resolved_at.map(|d| d.to_rfc3339()),
            ],
        )?;
        Ok(track.clone())
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, \
             from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE id = ?1",
        )?;

        let result = stmt.query_row([id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        });

        match result {
            Ok(track) => Ok(Some(track)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_by_scene(&self, scene_id: &str) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, \
             from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE scene_id = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([scene_id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        })?;

        rows.collect()
    }

    pub fn get_pending_by_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE scene_id = ?1 AND status = 'Pending' ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map([scene_id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        })?;

        rows.collect()
    }

    pub fn get_pending_by_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, \
             from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE chapter_id = ?1 AND status = 'Pending' ORDER BY created_at \
             DESC",
        )?;

        let rows = stmt.query_map([chapter_id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        })?;

        rows.collect()
    }

    pub fn get_by_version(&self, version_id: &str) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, \
             from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE version_id = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([version_id], |row| {
            let created_str: String = row.get(11)?;
            let resolved_str: Option<String> = row.get(12)?;
            Ok(ChangeTrack {
                id: row.get(0)?,
                scene_id: row.get(1)?,
                chapter_id: row.get(2)?,
                version_id: row.get(3)?,
                author_id: row.get(4)?,
                author_name: row.get(5)?,
                change_type: match row.get::<_, String>(6)?.as_str() {
                    "Delete" => ChangeType::Delete,
                    "Format" => ChangeType::Format,
                    _ => ChangeType::Insert,
                },
                from_pos: row.get(7)?,
                to_pos: row.get(8)?,
                content: row.get(9)?,
                status: match row.get::<_, String>(10)?.as_str() {
                    "Accepted" => ChangeStatus::Accepted,
                    "Rejected" => ChangeStatus::Rejected,
                    _ => ChangeStatus::Pending,
                },
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                resolved_at: resolved_str.and_then(|s| s.parse().ok()),
            })
        })?;

        rows.collect()
    }

    pub fn update_status(&self, id: &str, status: ChangeStatus) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let resolved = match status {
            ChangeStatus::Pending => None,
            _ => Some(Local::now().to_rfc3339()),
        };
        conn.execute(
            "UPDATE change_tracks SET status = ?2, resolved_at = ?3 WHERE id = ?1",
            params![id, format!("{:?}", status), resolved],
        )
    }

    pub fn accept_all_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Accepted', resolved_at = ?2 WHERE scene_id = ?1 \
             AND status = 'Pending'",
            params![scene_id, now],
        )
    }

    pub fn reject_all_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Rejected', resolved_at = ?2 WHERE scene_id = ?1 \
             AND status = 'Pending'",
            params![scene_id, now],
        )
    }

    pub fn accept_all_by_chapter(&self, chapter_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Accepted', resolved_at = ?2 WHERE chapter_id = ?1 \
             AND status = 'Pending'",
            params![chapter_id, now],
        )
    }

    pub fn reject_all_by_chapter(&self, chapter_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Rejected', resolved_at = ?2 WHERE chapter_id = ?1 \
             AND status = 'Pending'",
            params![chapter_id, now],
        )
    }

    pub fn delete_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM change_tracks WHERE scene_id = ?1",
            params![scene_id],
        )
    }
}

// ==================== CommentThread Repository (评论线程) ====================

pub struct CommentThreadRepository {
    pool: DbPool,
}

impl CommentThreadRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_thread(&self, thread: &CommentThread) -> Result<CommentThread, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO comment_threads (id, scene_id, chapter_id, version_id, anchor_type, \
             from_pos, to_pos, selected_text, status, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &thread.id,
                &thread.scene_id,
                &thread.chapter_id,
                &thread.version_id,
                format!("{:?}", thread.anchor_type),
                thread.from_pos,
                thread.to_pos,
                &thread.selected_text,
                format!("{:?}", thread.status),
                thread.created_at.to_rfc3339(),
                thread.resolved_at.map(|d| d.to_rfc3339()),
            ],
        )?;
        Ok(thread.clone())
    }

    pub fn add_message(&self, message: &CommentMessage) -> Result<CommentMessage, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO comment_messages (id, thread_id, author_id, author_name, content, \
             created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &message.id,
                &message.thread_id,
                &message.author_id,
                &message.author_name,
                &message.content,
                message.created_at.to_rfc3339(),
            ],
        )?;
        Ok(message.clone())
    }

    fn parse_thread(&self, row: &rusqlite::Row) -> Result<CommentThread, rusqlite::Error> {
        let created_str: String = row.get(9)?;
        let resolved_str: Option<String> = row.get(10)?;
        Ok(CommentThread {
            id: row.get(0)?,
            scene_id: row.get(1)?,
            chapter_id: row.get(2)?,
            version_id: row.get(3)?,
            anchor_type: match row.get::<_, String>(4)?.as_str() {
                "SceneLevel" => AnchorType::SceneLevel,
                _ => AnchorType::TextRange,
            },
            from_pos: row.get(5)?,
            to_pos: row.get(6)?,
            selected_text: row.get(7)?,
            status: match row.get::<_, String>(8)?.as_str() {
                "Resolved" => ThreadStatus::Resolved,
                _ => ThreadStatus::Open,
            },
            created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
            resolved_at: resolved_str.and_then(|s| s.parse().ok()),
        })
    }

    fn parse_message(&self, row: &rusqlite::Row) -> Result<CommentMessage, rusqlite::Error> {
        let created_str: String = row.get(5)?;
        Ok(CommentMessage {
            id: row.get(0)?,
            thread_id: row.get(1)?,
            author_id: row.get(2)?,
            author_name: row.get(3)?,
            content: row.get(4)?,
            created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
        })
    }

    pub fn get_threads_by_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<Vec<CommentThreadWithMessages>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, anchor_type, from_pos, to_pos, \
             selected_text, status, created_at, resolved_at
             FROM comment_threads WHERE chapter_id = ?1 ORDER BY created_at DESC",
        )?;

        let threads: Vec<CommentThread> = stmt
            .query_map([chapter_id], |row| self.parse_thread(row))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for thread in threads {
            let messages = self.get_messages(&thread.id)?;
            result.push(CommentThreadWithMessages { thread, messages });
        }
        Ok(result)
    }

    pub fn get_threads_by_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<CommentThreadWithMessages>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, anchor_type, from_pos, to_pos, \
             selected_text, status, created_at, resolved_at
             FROM comment_threads WHERE scene_id = ?1 ORDER BY created_at DESC",
        )?;

        let threads: Vec<CommentThread> = stmt
            .query_map([scene_id], |row| self.parse_thread(row))?
            .collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for thread in threads {
            let messages = self.get_messages(&thread.id)?;
            result.push(CommentThreadWithMessages { thread, messages });
        }
        Ok(result)
    }

    pub fn get_messages(&self, thread_id: &str) -> Result<Vec<CommentMessage>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, author_id, author_name, content, created_at
             FROM comment_messages WHERE thread_id = ?1 ORDER BY created_at ASC",
        )?;

        let rows = stmt.query_map([thread_id], |row| self.parse_message(row))?;
        rows.collect()
    }

    pub fn resolve_thread(&self, thread_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE comment_threads SET status = 'Resolved', resolved_at = ?2 WHERE id = ?1",
            params![thread_id, now],
        )
    }

    pub fn reopen_thread(&self, thread_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "UPDATE comment_threads SET status = 'Open', resolved_at = NULL WHERE id = ?1",
            params![thread_id],
        )
    }

    pub fn delete_thread(&self, thread_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM comment_threads WHERE id = ?1",
            params![thread_id],
        )
    }
}

// ==================== StoryStyleConfig Repository ====================

pub struct StoryStyleConfigRepository {
    pool: DbPool,
}

impl StoryStyleConfigRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        name: &str,
        blend_json: &str,
    ) -> Result<StoryStyleConfig, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO story_style_configs (id, story_id, name, blend_json, is_active, \
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![&id, story_id, name, blend_json, 1, &now, &now],
        )?;

        Ok(StoryStyleConfig {
            id,
            story_id: story_id.to_string(),
            name: name.to_string(),
            blend_json: blend_json.to_string(),
            is_active: true,
            created_at: Local::now(),
            updated_at: Local::now(),
        })
    }

    pub fn get_active_by_story(
        &self,
        story_id: &str,
    ) -> Result<Option<StoryStyleConfig>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, blend_json, is_active, created_at, updated_at
             FROM story_style_configs WHERE story_id = ?1 AND is_active = 1 LIMIT 1",
        )?;

        let result = stmt
            .query_row([story_id], |row| {
                let is_active: i32 = row.get(4)?;
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(StoryStyleConfig {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    blend_json: row.get(3)?,
                    is_active: is_active != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(result)
    }

    pub fn get_all_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<StoryStyleConfig>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, blend_json, is_active, created_at, updated_at
             FROM story_style_configs WHERE story_id = ?1 ORDER BY updated_at DESC",
        )?;

        let configs = stmt
            .query_map([story_id], |row| {
                let is_active: i32 = row.get(4)?;
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(StoryStyleConfig {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    blend_json: row.get(3)?,
                    is_active: is_active != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(configs)
    }

    pub fn update(
        &self,
        id: &str,
        name: Option<&str>,
        blend_json: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE story_style_configs SET
                name = COALESCE(?2, name),
                blend_json = COALESCE(?3, blend_json),
                updated_at = ?4
             WHERE id = ?1",
            params![id, name, blend_json, now],
        )
    }

    pub fn set_active(&self, story_id: &str, config_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        // 先取消该 story 下所有配置的 active 状态
        conn.execute(
            "UPDATE story_style_configs SET is_active = 0 WHERE story_id = ?1",
            params![story_id],
        )?;
        // 再设置指定配置为 active
        conn.execute(
            "UPDATE story_style_configs SET is_active = 1 WHERE id = ?1 AND story_id = ?2",
            params![config_id, story_id],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM story_style_configs WHERE id = ?1", params![id])
    }
}

// ==================== StyleDNA Repository ====================

pub struct StyleDnaRepository {
    pool: DbPool,
}

impl StyleDnaRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        name: &str,
        author: Option<&str>,
        dna_json: &str,
        is_builtin: bool,
    ) -> Result<super::models::StyleDNA, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO style_dnas (id, name, author, dna_json, is_builtin, is_user_created, \
             created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &id,
                name,
                author,
                dna_json,
                is_builtin as i32,
                !is_builtin as i32,
                now
            ],
        )?;

        Ok(super::models::StyleDNA {
            id,
            name: name.to_string(),
            author: author.map(|s| s.to_string()),
            dna_json: dna_json.to_string(),
            is_builtin,
            is_user_created: !is_builtin,
            created_at: Local::now(),
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<super::models::StyleDNA>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas WHERE id = ?1",
        )?;

        let result = stmt
            .query_row([id], |row| {
                let is_builtin: i32 = row.get(4)?;
                let is_user_created: i32 = row.get(5)?;
                let created_str: String = row.get(6)?;
                Ok(super::models::StyleDNA {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    author: row.get(2)?,
                    dna_json: row.get(3)?,
                    is_builtin: is_builtin != 0,
                    is_user_created: is_user_created != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(result)
    }

    /// 批量按 ID 查询 StyleDNA，将多次单条查询合并为一次 SQL IN 查询。
    pub fn get_many_by_ids(
        &self,
        ids: &[String],
    ) -> Result<Vec<super::models::StyleDNA>, rusqlite::Error> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas WHERE id IN ({}) ORDER BY name ASC",
            placeholders
        );

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(&sql)?;

        let dnas = stmt
            .query_map(rusqlite::params_from_iter(ids.iter()), |row| {
                let is_builtin: i32 = row.get(4)?;
                let is_user_created: i32 = row.get(5)?;
                let created_str: String = row.get(6)?;
                Ok(super::models::StyleDNA {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    author: row.get(2)?,
                    dna_json: row.get(3)?,
                    is_builtin: is_builtin != 0,
                    is_user_created: is_user_created != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(dnas)
    }

    pub fn get_all(&self) -> Result<Vec<super::models::StyleDNA>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas ORDER BY is_builtin DESC, name ASC",
        )?;

        let dnas = stmt
            .query_map([], |row| {
                let is_builtin: i32 = row.get(4)?;
                let is_user_created: i32 = row.get(5)?;
                let created_str: String = row.get(6)?;
                Ok(super::models::StyleDNA {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    author: row.get(2)?,
                    dna_json: row.get(3)?,
                    is_builtin: is_builtin != 0,
                    is_user_created: is_user_created != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(dnas)
    }

    pub fn get_builtin(&self) -> Result<Vec<super::models::StyleDNA>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas WHERE is_builtin = 1 ORDER BY name ASC",
        )?;

        let dnas = stmt
            .query_map([], |row| {
                let is_builtin: i32 = row.get(4)?;
                let is_user_created: i32 = row.get(5)?;
                let created_str: String = row.get(6)?;
                Ok(super::models::StyleDNA {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    author: row.get(2)?,
                    dna_json: row.get(3)?,
                    is_builtin: is_builtin != 0,
                    is_user_created: is_user_created != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(dnas)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM style_dnas WHERE id = ?1 AND is_builtin = 0",
            params![id],
        )
    }

    pub fn update_dna_json(&self, id: &str, dna_json: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "UPDATE style_dnas SET dna_json = ?2 WHERE id = ?1",
            params![id, dna_json],
        )
    }
}

// ==================== StyleSnapshot Repository (W3-B7) ====================

pub struct StyleSnapshotRepository {
    pool: DbPool,
}

impl StyleSnapshotRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        chapter_number: Option<i32>,
        scene_number: Option<i32>,
        metrics: &crate::creative_engine::style::metrics::StyleMetrics,
    ) -> Result<super::models::StyleSnapshot, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO style_snapshots
             (id, story_id, chapter_number, scene_number, sentence_length, dialogue_ratio,
              metaphor_density, inner_monologue_ratio, emotion_density, rhythm_score, computed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &id,
                story_id,
                chapter_number,
                scene_number,
                metrics.sentence_length as f64,
                metrics.dialogue_ratio as f64,
                metrics.metaphor_density as f64,
                metrics.inner_monologue_ratio as f64,
                metrics.emotion_density as f64,
                metrics.rhythm_score as f64,
                now,
            ],
        )?;

        Ok(super::models::StyleSnapshot {
            id,
            story_id: story_id.to_string(),
            chapter_number,
            scene_number,
            sentence_length: metrics.sentence_length as f64,
            dialogue_ratio: metrics.dialogue_ratio as f64,
            metaphor_density: metrics.metaphor_density as f64,
            inner_monologue_ratio: metrics.inner_monologue_ratio as f64,
            emotion_density: metrics.emotion_density as f64,
            rhythm_score: metrics.rhythm_score as f64,
            computed_at: Local::now(),
        })
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<super::models::StyleSnapshot>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, scene_number,
                    sentence_length, dialogue_ratio, metaphor_density,
                    inner_monologue_ratio, emotion_density, rhythm_score, computed_at
             FROM style_snapshots WHERE story_id = ?1 ORDER BY computed_at DESC",
        )?;

        let snapshots = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(10)?;
                Ok(super::models::StyleSnapshot {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    scene_number: row.get(3)?,
                    sentence_length: row.get(4)?,
                    dialogue_ratio: row.get(5)?,
                    metaphor_density: row.get(6)?,
                    inner_monologue_ratio: row.get(7)?,
                    emotion_density: row.get(8)?,
                    rhythm_score: row.get(9)?,
                    computed_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(snapshots)
    }

    pub fn get_latest_by_story(
        &self,
        story_id: &str,
    ) -> Result<Option<super::models::StyleSnapshot>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, scene_number,
                    sentence_length, dialogue_ratio, metaphor_density,
                    inner_monologue_ratio, emotion_density, rhythm_score, computed_at
             FROM style_snapshots WHERE story_id = ?1 ORDER BY computed_at DESC LIMIT 1",
        )?;

        let result = stmt
            .query_row([story_id], |row| {
                let created_str: String = row.get(10)?;
                Ok(super::models::StyleSnapshot {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    chapter_number: row.get(2)?,
                    scene_number: row.get(3)?,
                    sentence_length: row.get(4)?,
                    dialogue_ratio: row.get(5)?,
                    metaphor_density: row.get(6)?,
                    inner_monologue_ratio: row.get(7)?,
                    emotion_density: row.get(8)?,
                    rhythm_score: row.get(9)?,
                    computed_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(result)
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM style_snapshots WHERE story_id = ?1",
            params![story_id],
        )
    }
}

// ==================== UserFeedback Repository ====================

pub struct UserFeedbackRepository {
    pool: DbPool,
}

impl UserFeedbackRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        scene_id: Option<&str>,
        chapter_id: Option<&str>,
        feedback_type: &str,
        agent_type: Option<&str>,
        original_ai_text: &str,
        final_text: &str,
        ai_score: Option<f32>,
        user_satisfaction: Option<i32>,
        metadata: Option<&serde_json::Value>,
    ) -> Result<super::models::UserFeedbackLog, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO user_feedback_log (id, story_id, scene_id, chapter_id, feedback_type, \
             agent_type, original_ai_text, final_text, ai_score, user_satisfaction, metadata, \
             created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &id,
                story_id,
                scene_id,
                chapter_id,
                feedback_type,
                agent_type,
                original_ai_text,
                final_text,
                ai_score,
                user_satisfaction,
                metadata.map(|m| m.to_string()),
                now
            ],
        )?;

        Ok(super::models::UserFeedbackLog {
            id,
            story_id: story_id.to_string(),
            scene_id: scene_id.map(|s| s.to_string()),
            chapter_id: chapter_id.map(|s| s.to_string()),
            feedback_type: feedback_type
                .parse()
                .unwrap_or(super::models::FeedbackType::Accept),
            agent_type: agent_type.map(|s| s.to_string()),
            original_ai_text: original_ai_text.to_string(),
            final_text: final_text.to_string(),
            ai_score,
            user_satisfaction,
            metadata: metadata.cloned(),
            created_at: Local::now(),
        })
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
        limit: Option<i64>,
    ) -> Result<Vec<super::models::UserFeedbackLog>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let sql = if let Some(lim) = limit {
            format!(
                "SELECT id, story_id, scene_id, chapter_id, feedback_type, agent_type, \
                 original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at
                 FROM user_feedback_log WHERE story_id = ?1 ORDER BY created_at DESC LIMIT {}",
                lim
            )
        } else {
            "SELECT id, story_id, scene_id, chapter_id, feedback_type, agent_type, \
             original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at
             FROM user_feedback_log WHERE story_id = ?1 ORDER BY created_at DESC"
                .to_string()
        };
        let mut stmt = conn.prepare(&sql)?;

        let logs = stmt
            .query_map([story_id], |row| {
                let meta_str: Option<String> = row.get(10)?;
                let meta = meta_str.and_then(|s| serde_json::from_str(&s).ok());
                let created_str: String = row.get(11)?;
                Ok(super::models::UserFeedbackLog {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    feedback_type: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or(super::models::FeedbackType::Accept),
                    agent_type: row.get(5)?,
                    original_ai_text: row.get(6)?,
                    final_text: row.get(7)?,
                    ai_score: row.get(8)?,
                    user_satisfaction: row.get(9)?,
                    metadata: meta,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    pub fn get_recent(
        &self,
        story_id: &str,
        days: i64,
    ) -> Result<Vec<super::models::UserFeedbackLog>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let cutoff = (Local::now() - chrono::Duration::days(days)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, feedback_type, agent_type, \
             original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at
             FROM user_feedback_log WHERE story_id = ?1 AND created_at >= ?2 ORDER BY created_at \
             DESC",
        )?;

        let logs = stmt
            .query_map(params![story_id, cutoff], |row| {
                let meta_str: Option<String> = row.get(10)?;
                let meta = meta_str.and_then(|s| serde_json::from_str(&s).ok());
                let created_str: String = row.get(11)?;
                Ok(super::models::UserFeedbackLog {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    feedback_type: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or(super::models::FeedbackType::Accept),
                    agent_type: row.get(5)?,
                    original_ai_text: row.get(6)?,
                    final_text: row.get(7)?,
                    ai_score: row.get(8)?,
                    user_satisfaction: row.get(9)?,
                    metadata: meta,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    pub fn get_stats(&self, story_id: &str) -> Result<FeedbackStats, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT feedback_type, COUNT(*) FROM user_feedback_log WHERE story_id = ?1 GROUP BY \
             feedback_type",
        )?;

        let mut accept = 0;
        let mut reject = 0;
        let mut modify = 0;

        let rows = stmt.query_map([story_id], |row| {
            let ft: String = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((ft, count))
        })?;

        for row in rows {
            let (ft, count) = row?;
            match ft.as_str() {
                "accept" => accept = count,
                "reject" => reject = count,
                "modify" => modify = count,
                _ => {}
            }
        }

        Ok(FeedbackStats {
            accept,
            reject,
            modify,
        })
    }
}

#[derive(Debug, Clone)]
pub struct FeedbackStats {
    pub accept: i64,
    pub reject: i64,
    pub modify: i64,
}

// ==================== UserPreference Repository ====================

pub struct UserPreferenceRepository {
    pool: DbPool,
}

impl UserPreferenceRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn upsert(
        &self,
        story_id: &str,
        preference_type: &str,
        preference_key: &str,
        preference_value: &str,
        confidence: f32,
        evidence_count: i32,
    ) -> Result<super::models::UserPreference, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        // 先检查是否已存在
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM user_preferences WHERE story_id = ?1 AND preference_type = ?2 AND \
                 preference_key = ?3",
                params![story_id, preference_type, preference_key],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            // 更新
            conn.execute(
                "UPDATE user_preferences SET preference_value = ?4, confidence = ?5, \
                 evidence_count = ?6, updated_at = ?7
                 WHERE id = ?1",
                params![&id, preference_value, confidence, evidence_count, now],
            )?;

            Ok(super::models::UserPreference {
                id,
                story_id: story_id.to_string(),
                preference_type: preference_type
                    .parse()
                    .unwrap_or(super::models::PreferenceType::Content),
                preference_key: preference_key.to_string(),
                preference_value: preference_value.to_string(),
                confidence,
                evidence_count,
                updated_at: Local::now(),
            })
        } else {
            // 创建
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO user_preferences (id, story_id, preference_type, preference_key, \
                 preference_value, confidence, evidence_count, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    &id,
                    story_id,
                    preference_type,
                    preference_key,
                    preference_value,
                    confidence,
                    evidence_count,
                    now
                ],
            )?;

            Ok(super::models::UserPreference {
                id,
                story_id: story_id.to_string(),
                preference_type: preference_type
                    .parse()
                    .unwrap_or(super::models::PreferenceType::Content),
                preference_key: preference_key.to_string(),
                preference_value: preference_value.to_string(),
                confidence,
                evidence_count,
                updated_at: Local::now(),
            })
        }
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<super::models::UserPreference>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, preference_type, preference_key, preference_value, confidence, \
             evidence_count, updated_at
             FROM user_preferences WHERE story_id = ?1 ORDER BY confidence DESC",
        )?;

        let prefs = stmt
            .query_map([story_id], |row| {
                let updated_str: String = row.get(7)?;
                Ok(super::models::UserPreference {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    preference_type: row
                        .get::<_, String>(2)?
                        .parse()
                        .unwrap_or(super::models::PreferenceType::Content),
                    preference_key: row.get(3)?,
                    preference_value: row.get(4)?,
                    confidence: row.get(5)?,
                    evidence_count: row.get(6)?,
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(prefs)
    }

    pub fn get_by_type(
        &self,
        story_id: &str,
        pref_type: &str,
    ) -> Result<Vec<super::models::UserPreference>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, preference_type, preference_key, preference_value, confidence, \
             evidence_count, updated_at
             FROM user_preferences WHERE story_id = ?1 AND preference_type = ?2 ORDER BY \
             confidence DESC",
        )?;

        let prefs = stmt
            .query_map(params![story_id, pref_type], |row| {
                let updated_str: String = row.get(7)?;
                Ok(super::models::UserPreference {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    preference_type: row
                        .get::<_, String>(2)?
                        .parse()
                        .unwrap_or(super::models::PreferenceType::Content),
                    preference_key: row.get(3)?,
                    preference_value: row.get(4)?,
                    confidence: row.get(5)?,
                    evidence_count: row.get(6)?,
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(prefs)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM user_preferences WHERE id = ?1", params![id])
    }
}

// ==================== Story Outline Repository ====================

pub struct StoryOutlineRepository {
    pool: DbPool,
}

impl StoryOutlineRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        content: &str,
        structure_json: Option<&str>,
        act_count: i32,
        total_scenes_estimate: Option<i32>,
    ) -> Result<super::models::StoryOutline, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, \
             total_scenes_estimate, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &id,
                story_id,
                content,
                structure_json,
                act_count,
                total_scenes_estimate,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(super::models::StoryOutline {
            id,
            story_id: story_id.to_string(),
            content: content.to_string(),
            structure_json: structure_json.map(|s| s.to_string()),
            act_count,
            total_scenes_estimate,
            created_at: now,
            updated_at: now,
            analyzed_structure_json: None,
        })
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
    ) -> Result<Option<super::models::StoryOutline>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, content, structure_json, act_count, total_scenes_estimate, \
             analyzed_structure_json, created_at, updated_at
             FROM story_outlines WHERE story_id = ?1",
        )?;

        let outline = stmt
            .query_row([story_id], |row| {
                let created_str: String = row.get(7)?;
                let updated_str: String = row.get(8)?;

                Ok(super::models::StoryOutline {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    content: row.get(2)?,
                    structure_json: row.get(3)?,
                    act_count: row.get(4)?,
                    total_scenes_estimate: row.get(5)?,
                    analyzed_structure_json: row.get(6)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(outline)
    }

    pub fn update(
        &self,
        story_id: &str,
        content: Option<&str>,
        structure_json: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE story_outlines SET content = COALESCE(?2, content), structure_json = \
             COALESCE(?3, structure_json), updated_at = ?4 WHERE story_id = ?1",
            params![story_id, content, structure_json, now],
        )?;
        Ok(count)
    }

    pub fn delete(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM story_outlines WHERE story_id = ?1", [story_id])
    }
}

// ==================== Character Relationship Repository ====================

pub struct CharacterRelationshipRepository {
    pool: DbPool,
}

impl CharacterRelationshipRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        story_id: &str,
        source_character_id: &str,
        target_character_id: &str,
        relationship_type: &str,
        description: Option<&str>,
        dynamic: Option<&str>,
    ) -> Result<super::models::CharacterRelationship, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO character_relationships (id, story_id, source_character_id, \
             target_character_id, relationship_type, description, dynamic, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &id,
                story_id,
                source_character_id,
                target_character_id,
                relationship_type,
                description,
                dynamic,
                now.to_rfc3339()
            ],
        )?;

        Ok(super::models::CharacterRelationship {
            id,
            story_id: story_id.to_string(),
            source_character_id: source_character_id.to_string(),
            target_character_id: target_character_id.to_string(),
            target_character_name: None,
            relationship_type: relationship_type.to_string(),
            description: description.map(|s| s.to_string()),
            dynamic: dynamic.map(|s| s.to_string()),
            created_at: now,
        })
    }

    pub fn get_by_id(
        &self,
        id: &str,
    ) -> Result<Option<super::models::CharacterRelationship>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.story_id, r.source_character_id, r.target_character_id, c.name as \
             target_name,
                    r.relationship_type, r.description, r.dynamic, r.created_at
             FROM character_relationships r
             LEFT JOIN characters c ON r.target_character_id = c.id
             WHERE r.id = ?1",
        )?;

        let result = stmt.query_row([id], |row| {
            let created_str: String = row.get(8)?;

            Ok(super::models::CharacterRelationship {
                id: row.get(0)?,
                story_id: row.get(1)?,
                source_character_id: row.get(2)?,
                target_character_id: row.get(3)?,
                target_character_name: row.get(4)?,
                relationship_type: row.get(5)?,
                description: row.get(6)?,
                dynamic: row.get(7)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
            })
        });

        match result {
            Ok(r) => Ok(Some(r)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
    ) -> Result<Vec<super::models::CharacterRelationship>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.story_id, r.source_character_id, r.target_character_id, c.name as \
             target_name,
                    r.relationship_type, r.description, r.dynamic, r.created_at
             FROM character_relationships r
             LEFT JOIN characters c ON r.target_character_id = c.id
             WHERE r.story_id = ?1
             ORDER BY r.created_at",
        )?;

        let relationships = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(8)?;

                Ok(super::models::CharacterRelationship {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    source_character_id: row.get(2)?,
                    target_character_id: row.get(3)?,
                    target_character_name: row.get(4)?,
                    relationship_type: row.get(5)?,
                    description: row.get(6)?,
                    dynamic: row.get(7)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(relationships)
    }

    pub fn update(
        &self,
        relationship_id: &str,
        relationship_type: Option<&str>,
        description: Option<&str>,
        dynamic: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let mut updates = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(rt) = relationship_type {
            updates.push("relationship_type = ?");
            params.push(Box::new(rt.to_string()));
        }
        if let Some(desc) = description {
            updates.push("description = ?");
            params.push(Box::new(desc.to_string()));
        }
        if let Some(dyn_val) = dynamic {
            updates.push("dynamic = ?");
            params.push(Box::new(dyn_val.to_string()));
        }

        if updates.is_empty() {
            return Ok(0);
        }

        params.push(Box::new(relationship_id.to_string()));
        let sql = format!(
            "UPDATE character_relationships SET {} WHERE id = ?",
            updates.join(", ")
        );

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, param_refs.as_slice())
    }

    pub fn delete(&self, relationship_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM character_relationships WHERE id = ?1",
            [relationship_id],
        )
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM character_relationships WHERE story_id = ?1",
            [story_id],
        )
    }
}

// ==================== 场景-角色关联 Repository ====================

pub struct SceneCharacterRepository {
    pool: DbPool,
}

impl SceneCharacterRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 添加角色到场景
    pub fn add_character_to_scene(
        &self,
        scene_id: &str,
        character_id: &str,
    ) -> Result<super::models::SceneCharacter, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        // 检查是否已存在
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM scene_characters WHERE scene_id = ?1 AND character_id = ?2",
                [scene_id, character_id],
                |_| Ok(true),
            )
            .unwrap_or(false);

        if exists {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
                Some("Character already in scene".to_string()),
            ));
        }

        conn.execute(
            "INSERT INTO scene_characters (id, scene_id, character_id, created_at) VALUES (?1, \
             ?2, ?3, ?4)",
            params![&id, scene_id, character_id, now.to_rfc3339()],
        )?;

        // 获取角色名称
        let character_name: Option<String> = conn
            .query_row(
                "SELECT name FROM characters WHERE id = ?1",
                [character_id],
                |row| row.get(0),
            )
            .ok();

        Ok(super::models::SceneCharacter {
            id,
            scene_id: scene_id.to_string(),
            character_id: character_id.to_string(),
            character_name,
            created_at: now,
        })
    }

    /// 从场景移除角色
    pub fn remove_character_from_scene(
        &self,
        scene_id: &str,
        character_id: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM scene_characters WHERE scene_id = ?1 AND character_id = ?2",
            [scene_id, character_id],
        )
    }

    /// 获取场景中的所有角色
    pub fn get_characters_in_scene(
        &self,
        scene_id: &str,
    ) -> Result<Vec<super::models::SceneCharacter>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT sc.id, sc.scene_id, sc.character_id, c.name, sc.created_at
             FROM scene_characters sc
             LEFT JOIN characters c ON sc.character_id = c.id
             WHERE sc.scene_id = ?1
             ORDER BY sc.created_at",
        )?;

        let scene_characters = stmt
            .query_map([scene_id], |row| {
                let created_str: String = row.get(4)?;
                Ok(super::models::SceneCharacter {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    character_id: row.get(2)?,
                    character_name: row.get(3)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(scene_characters)
    }

    /// 获取角色参与的所有场景
    pub fn get_scenes_for_character(
        &self,
        character_id: &str,
    ) -> Result<Vec<super::models::SceneCharacter>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT sc.id, sc.scene_id, sc.character_id, c.name, sc.created_at
             FROM scene_characters sc
             LEFT JOIN characters c ON sc.character_id = c.id
             WHERE sc.character_id = ?1
             ORDER BY sc.created_at",
        )?;

        let scene_characters = stmt
            .query_map([character_id], |row| {
                let created_str: String = row.get(4)?;
                Ok(super::models::SceneCharacter {
                    id: row.get(0)?,
                    scene_id: row.get(1)?,
                    character_id: row.get(2)?,
                    character_name: row.get(3)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(scene_characters)
    }

    /// 批量设置场景中的角色
    pub fn set_scene_characters(
        &self,
        scene_id: &str,
        character_ids: &[String],
    ) -> Result<Vec<super::models::SceneCharacter>, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;

        // 先清除现有关联
        tx.execute(
            "DELETE FROM scene_characters WHERE scene_id = ?1",
            [scene_id],
        )?;

        let mut result = Vec::new();
        let now = Local::now();

        // 添加新关联
        for character_id in character_ids {
            let id = Uuid::new_v4().to_string();

            tx.execute(
                "INSERT INTO scene_characters (id, scene_id, character_id, created_at) VALUES \
                 (?1, ?2, ?3, ?4)",
                params![&id, scene_id, character_id, now.to_rfc3339()],
            )?;

            // 获取角色名称
            let character_name: Option<String> = tx
                .query_row(
                    "SELECT name FROM characters WHERE id = ?1",
                    [character_id],
                    |row| row.get(0),
                )
                .ok();

            result.push(super::models::SceneCharacter {
                id,
                scene_id: scene_id.to_string(),
                character_id: character_id.clone(),
                character_name,
                created_at: now,
            });
        }

        tx.commit()?;
        Ok(result)
    }

    /// 删除场景的所有角色关联
    pub fn delete_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM scene_characters WHERE scene_id = ?1",
            [scene_id],
        )
    }

    /// 删除角色的所有场景关联
    pub fn delete_by_character(&self, character_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM scene_characters WHERE character_id = ?1",
            [character_id],
        )
    }
}

// ==================== SceneDividerNode Repository ====================

pub struct SceneDividerRepository {
    pool: DbPool,
}

impl SceneDividerRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 为指定章节创建 divider
    pub fn create(
        &self,
        chapter_id: &str,
        position: i32,
        scene_id: &str,
        label: Option<&str>,
    ) -> Result<super::models::SceneDividerNode, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO scene_divider_nodes (id, chapter_id, position, scene_id, label, \
             created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &id,
                chapter_id,
                position,
                scene_id,
                label,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;
        Ok(super::models::SceneDividerNode {
            id,
            chapter_id: chapter_id.to_string(),
            position,
            scene_id: scene_id.to_string(),
            label: label.map(|s| s.to_string()),
            created_at: now,
            updated_at: now,
        })
    }

    /// 获取章节下的所有 divider，按 position 排序
    pub fn get_by_chapter(
        &self,
        chapter_id: &str,
    ) -> Result<Vec<super::models::SceneDividerNode>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, chapter_id, position, scene_id, label, created_at, updated_at
             FROM scene_divider_nodes WHERE chapter_id = ?1 ORDER BY position ASC",
        )?;
        let nodes = stmt
            .query_map([chapter_id], |row| {
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(super::models::SceneDividerNode {
                    id: row.get(0)?,
                    chapter_id: row.get(1)?,
                    position: row.get(2)?,
                    scene_id: row.get(3)?,
                    label: row.get(4)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(nodes)
    }

    /// 批量设置章节的 divider（用于重排/重建 divider）
    pub fn set_dividers(
        &self,
        chapter_id: &str,
        dividers: &[(String, i32, Option<String>)], // (scene_id, position, label)
    ) -> Result<Vec<super::models::SceneDividerNode>, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM scene_divider_nodes WHERE chapter_id = ?1",
            [chapter_id],
        )?;
        let now = Local::now();
        let mut nodes = Vec::new();
        for (scene_id, position, label) in dividers {
            let id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO scene_divider_nodes (id, chapter_id, position, scene_id, label, \
                 created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    &id,
                    chapter_id,
                    position,
                    scene_id,
                    label,
                    now.to_rfc3339(),
                    now.to_rfc3339()
                ],
            )?;
            nodes.push(super::models::SceneDividerNode {
                id,
                chapter_id: chapter_id.to_string(),
                position: *position,
                scene_id: scene_id.clone(),
                label: label.clone(),
                created_at: now,
                updated_at: now,
            });
        }
        tx.commit()?;
        Ok(nodes)
    }

    /// 删除单个 divider
    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM scene_divider_nodes WHERE id = ?1", [id])
    }

    /// 删除章节的所有 divider
    pub fn delete_by_chapter(&self, chapter_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM scene_divider_nodes WHERE chapter_id = ?1",
            [chapter_id],
        )
    }
}

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
             genre_profile_id, methodology_id, methodology_step, created_at, updated_at) VALUES \
             (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
             methodology_id, methodology_step, created_at, updated_at FROM stories ORDER BY \
             updated_at DESC",
        )?;

        let stories = stmt
            .query_map([], |row| {
                let created_str: String = row.get(10)?;
                let updated_str: String = row.get(11)?;
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
             methodology_id, methodology_step, created_at, updated_at FROM stories WHERE id = ?1",
        )?;

        let story = stmt
            .query_row([id], |row| {
                let created_str: String = row.get(10)?;
                let updated_str: String = row.get(11)?;
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
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(story)
    }

    pub fn update(
        &self,
        id: &str,
        req: &super::UpdateStoryRequest,
    ) -> Result<usize, rusqlite::Error> {
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
             methodology_step), updated_at = ?11 WHERE id = ?1",
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

pub struct CharacterRepository {
    pool: DbPool,
}

impl CharacterRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        req: CreateCharacterRequest,
    ) -> Result<Character, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let traits_json = "[]";

        tx.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, \
             appearance, gender, age, dynamic_traits, cs_location, cs_power_level, \
             cs_physical_state, cs_mental_state, cs_key_items, cs_recent_events, \
             cs_updated_at_chapter, cs_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, \
             ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                &id,
                &req.story_id,
                &req.name,
                req.background,
                req.personality,
                req.goals,
                req.appearance,
                req.gender,
                req.age,
                traits_json,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(Character {
            id,
            story_id: req.story_id,
            name: req.name,
            background: req.background,
            personality: req.personality,
            goals: req.goals,
            appearance: req.appearance,
            gender: req.gender,
            age: req.age,
            dynamic_traits: vec![],
            cs_location: None,
            cs_power_level: None,
            cs_physical_state: None,
            cs_mental_state: None,
            cs_key_items: None,
            cs_recent_events: None,
            cs_updated_at_chapter: None,
            cs_json: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn create(&self, req: CreateCharacterRequest) -> Result<Character, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let character = self.create_in_tx(&tx, req)?;
        tx.commit()?;
        Ok(character)
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<Character>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, background, personality, goals, appearance, gender, age, \
             dynamic_traits, cs_location, cs_power_level, cs_physical_state, cs_mental_state, \
             cs_key_items, cs_recent_events, cs_updated_at_chapter, cs_json, created_at, \
             updated_at FROM characters WHERE story_id = ?1",
        )?;

        let characters = stmt
            .query_map([story_id], |row| {
                let traits_json: String = row.get(9)?;
                let dynamic_traits: Vec<DynamicTrait> =
                    serde_json::from_str(&traits_json).unwrap_or_default();
                let created_str: String = row.get(18)?;
                let updated_str: String = row.get(19)?;

                Ok(Character {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    background: row.get(3)?,
                    personality: row.get(4)?,
                    goals: row.get(5)?,
                    appearance: row.get(6)?,
                    gender: row.get(7)?,
                    age: row.get(8)?,
                    dynamic_traits,
                    cs_location: row.get(10).ok(),
                    cs_power_level: row.get(11).ok(),
                    cs_physical_state: row.get(12).ok(),
                    cs_mental_state: row.get(13).ok(),
                    cs_key_items: row.get(14).ok(),
                    cs_recent_events: row.get(15).ok(),
                    cs_updated_at_chapter: row.get(16).ok(),
                    cs_json: row.get(17).ok(),
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(characters)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Character>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, background, personality, goals, appearance, gender, age, \
             dynamic_traits, cs_location, cs_power_level, cs_physical_state, cs_mental_state, \
             cs_key_items, cs_recent_events, cs_updated_at_chapter, cs_json, created_at, \
             updated_at FROM characters WHERE id = ?1",
        )?;

        let character = stmt
            .query_row([id], |row| {
                let traits_json: String = row.get(9)?;
                let dynamic_traits: Vec<DynamicTrait> =
                    serde_json::from_str(&traits_json).unwrap_or_default();
                let created_str: String = row.get(18)?;
                let updated_str: String = row.get(19)?;

                Ok(Character {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    background: row.get(3)?,
                    personality: row.get(4)?,
                    goals: row.get(5)?,
                    appearance: row.get(6)?,
                    gender: row.get(7)?,
                    age: row.get(8)?,
                    dynamic_traits,
                    cs_location: row.get(10).ok(),
                    cs_power_level: row.get(11).ok(),
                    cs_physical_state: row.get(12).ok(),
                    cs_mental_state: row.get(13).ok(),
                    cs_key_items: row.get(14).ok(),
                    cs_recent_events: row.get(15).ok(),
                    cs_updated_at_chapter: row.get(16).ok(),
                    cs_json: row.get(17).ok(),
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(character)
    }

    pub fn update(
        &self,
        id: &str,
        name: Option<String>,
        background: Option<String>,
        personality: Option<String>,
        goals: Option<String>,
        appearance: Option<String>,
        gender: Option<String>,
        age: Option<i32>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE characters SET name = COALESCE(?2, name), background = COALESCE(?3, \
             background),
             personality = COALESCE(?4, personality), goals = COALESCE(?5, goals), appearance = \
             COALESCE(?6, appearance),
             gender = COALESCE(?7, gender), age = COALESCE(?8, age), updated_at = ?9 WHERE id = ?1",
            params![
                id,
                name,
                background,
                personality,
                goals,
                appearance,
                gender,
                age,
                now
            ],
        )?;
        Ok(count)
    }

    pub fn update_character_state(
        &self,
        character_id: &str,
        state: &CharacterState,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE characters SET
                cs_location = COALESCE(?2, cs_location),
                cs_power_level = COALESCE(?3, cs_power_level),
                cs_physical_state = COALESCE(?4, cs_physical_state),
                cs_mental_state = COALESCE(?5, cs_mental_state),
                cs_key_items = COALESCE(?6, cs_key_items),
                cs_recent_events = COALESCE(?7, cs_recent_events),
                cs_updated_at_chapter = COALESCE(?8, cs_updated_at_chapter),
                updated_at = ?9
            WHERE id = ?1",
            params![
                character_id,
                state.location,
                state.power_level,
                state.physical_state,
                state.mental_state,
                state.key_items,
                state.recent_events,
                state.updated_at_chapter,
                now,
            ],
        )?;
        Ok(count)
    }

    pub fn get_character_state(
        &self,
        character_id: &str,
    ) -> Result<Option<CharacterState>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT cs_location, cs_power_level, cs_physical_state, cs_mental_state, \
             cs_key_items, cs_recent_events, cs_updated_at_chapter FROM characters WHERE id = ?1",
        )?;

        let state = stmt
            .query_row([character_id], |row| {
                Ok(CharacterState {
                    location: row.get(0).ok(),
                    power_level: row.get(1).ok(),
                    physical_state: row.get(2).ok(),
                    mental_state: row.get(3).ok(),
                    key_items: row.get(4).ok(),
                    recent_events: row.get(5).ok(),
                    updated_at_chapter: row.get(6).ok(),
                    arc_type: None,
                    state_transitions_json: None,
                })
            })
            .optional()?;

        Ok(state)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        // 在事务中执行删除操作
        let tx = conn.unchecked_transaction()?;

        // 验证角色是否存在
        let exists: bool = tx
            .query_row("SELECT 1 FROM characters WHERE id = ?1", [id], |_| Ok(true))
            .unwrap_or(false);

        if !exists {
            tx.rollback()?;
            return Ok(0);
        }
        let _ = tx.execute("DELETE FROM scene_characters WHERE character_id = ?1", [id]);
        let _ = tx.execute(
            "DELETE FROM scene_character_actions WHERE character_id = ?1",
            [id],
        );
        let _ = tx.execute(
            "DELETE FROM character_relationships WHERE source_character_id = ?1 OR \
             target_character_id = ?1",
            [id],
        );
        let _ = tx.execute("DELETE FROM character_states WHERE character_id = ?1", [id]);

        // 执行删除操作 - 外键约束会自动级联剩余关联数据
        let count = tx.execute("DELETE FROM characters WHERE id = ?1", [id])?;

        tx.commit()?;
        Ok(count)
    }
}

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

    /// 聚合 story 下所有章节 content 字段的总长度（用于总字数统计，避免全量
    /// IPC）。
    pub fn total_content_length_by_story(&self, story_id: &str) -> Result<i64, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(content)), 0) FROM chapters WHERE story_id = ?1",
            [story_id],
            |row| row.get(0),
        )?;
        Ok(total)
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

pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_user(
        &self,
        email: Option<String>,
        display_name: Option<String>,
        avatar_url: Option<String>,
    ) -> Result<User, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO users (id, email, display_name, avatar_url, is_local_user, created_at, \
             updated_at) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            params![&id, email, display_name, avatar_url, now.to_rfc3339()],
        )?;

        Ok(User {
            id,
            email,
            display_name,
            avatar_url,
            is_local_user: false,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn find_by_oauth(
        &self,
        provider: &str,
        provider_account_id: &str,
    ) -> Result<Option<User>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT u.id, u.email, u.display_name, u.avatar_url, u.is_local_user, u.created_at, \
             u.updated_at
             FROM users u
             JOIN oauth_accounts oa ON u.id = oa.user_id
             WHERE oa.provider = ?1 AND oa.provider_account_id = ?2",
        )?;

        let user = stmt
            .query_row([provider, provider_account_id], |row| {
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;
                Ok(User {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    display_name: row.get(2)?,
                    avatar_url: row.get(3)?,
                    is_local_user: row.get::<_, i32>(4)? != 0,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(user)
    }

    pub fn create_oauth_account(
        &self,
        user_id: &str,
        provider: &str,
        provider_account_id: &str,
        access_token: Option<String>,
        refresh_token: Option<String>,
        expires_at: Option<chrono::DateTime<Local>>,
    ) -> Result<OAuthAccount, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO oauth_accounts (id, user_id, provider, provider_account_id, \
             access_token, refresh_token, expires_at, created_at, updated_at) VALUES (?1, ?2, ?3, \
             ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                &id,
                user_id,
                provider,
                provider_account_id,
                access_token,
                refresh_token,
                expires_at.map(|d| d.to_rfc3339()),
                now.to_rfc3339()
            ],
        )?;

        Ok(OAuthAccount {
            id,
            user_id: user_id.to_string(),
            provider: provider.to_string(),
            provider_account_id: provider_account_id.to_string(),
            access_token,
            refresh_token,
            expires_at,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn create_session(
        &self,
        user_id: &str,
        token: &str,
        expires_at: chrono::DateTime<Local>,
    ) -> Result<Session, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO sessions (id, user_id, token, expires_at, created_at) VALUES (?1, ?2, \
             ?3, ?4, ?5)",
            params![
                &id,
                user_id,
                token,
                expires_at.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(Session {
            id,
            user_id: user_id.to_string(),
            token: token.to_string(),
            expires_at,
            created_at: now,
        })
    }

    pub fn delete_session(&self, token: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM sessions WHERE token = ?1", [token])?;
        Ok(count)
    }

    pub fn to_user_info(&self, user: &User) -> UserInfo {
        UserInfo {
            id: user.id.clone(),
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            avatar_url: user.avatar_url.clone(),
        }
    }
}

// ==================== GenesisRun Repository (W2-B9) ====================

pub struct GenesisRunRepository {
    pool: DbPool,
}

impl GenesisRunRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        id: &str,
        session_id: &str,
        premise: &str,
        total_steps: i32,
    ) -> Result<super::GenesisRun, rusqlite::Error> {
        let now = Local::now();
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO genesis_runs (id, session_id, premise, status, total_steps, steps_json, \
             created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                session_id,
                premise,
                "pending",
                total_steps,
                "{}",
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;
        Ok(super::GenesisRun {
            id: id.to_string(),
            story_id: None,
            session_id: session_id.to_string(),
            premise: premise.to_string(),
            status: "pending".to_string(),
            current_step: None,
            current_step_number: 0,
            total_steps,
            steps_json: "{}".to_string(),
            error_message: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_step(
        &self,
        id: &str,
        step_name: &str,
        step_number: i32,
        status: &str,
        steps_json: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET current_step = ?2, current_step_number = ?3, status = ?4, \
             steps_json = ?5, updated_at = ?6 WHERE id = ?1",
            params![id, step_name, step_number, status, steps_json, now],
        )
    }

    pub fn complete(&self, id: &str, story_id: Option<&str>) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET status = 'completed', story_id = ?2, updated_at = ?3 WHERE \
             id = ?1",
            params![id, story_id, now],
        )
    }

    pub fn fail(&self, id: &str, error_message: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET status = 'failed', error_message = ?2, updated_at = ?3 WHERE \
             id = ?1",
            params![id, error_message, now],
        )
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<super::GenesisRun>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, session_id, premise, status, current_step, current_step_number, \
             total_steps, steps_json, error_message, created_at, updated_at FROM genesis_runs \
             WHERE id = ?1",
        )?;
        let run = stmt
            .query_row([id], |row| {
                let created_str: String = row.get(10)?;
                let updated_str: String = row.get(11)?;
                Ok(super::GenesisRun {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    session_id: row.get(2)?,
                    premise: row.get(3)?,
                    status: row.get(4)?,
                    current_step: row.get(5)?,
                    current_step_number: row.get(6)?,
                    total_steps: row.get(7)?,
                    steps_json: row.get(8)?,
                    error_message: row.get(9)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;
        Ok(run)
    }

    pub fn list_all(&self, limit: i64) -> Result<Vec<super::GenesisRun>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, session_id, premise, status, current_step, current_step_number, \
             total_steps, steps_json, error_message, created_at, updated_at FROM genesis_runs \
             ORDER BY created_at DESC LIMIT ?1",
        )?;
        let runs = stmt
            .query_map([limit], |row| {
                let created_str: String = row.get(10)?;
                let updated_str: String = row.get(11)?;
                Ok(super::GenesisRun {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    session_id: row.get(2)?,
                    premise: row.get(3)?,
                    status: row.get(4)?,
                    current_step: row.get(5)?,
                    current_step_number: row.get(6)?,
                    total_steps: row.get(7)?,
                    steps_json: row.get(8)?,
                    error_message: row.get(9)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(runs)
    }
}

// ==================== Trait Implementations ====================

use crate::db::traits::{
    ChapterRepo, CharacterRepo, SceneRepo, StoryRepo, WorldBuildingRepo, WritingStyleRepo,
};

impl SceneRepo for SceneRepository {
    fn create(
        &self,
        story_id: &str,
        sequence_number: i32,
        title: Option<&str>,
    ) -> Result<Scene, rusqlite::Error> {
        self.create(story_id, sequence_number, title)
    }
    fn get_by_id(&self, id: &str) -> Result<Option<Scene>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn get_by_chapter(&self, chapter_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
        self.get_by_chapter(chapter_id)
    }
    fn update(&self, id: &str, updates: &SceneUpdate) -> Result<usize, rusqlite::Error> {
        self.update(id, updates)
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
    fn update_sequence(&self, id: &str, new_sequence: i32) -> Result<usize, rusqlite::Error> {
        self.update_sequence(id, new_sequence)
    }
}

impl StoryRepo for StoryRepository {
    fn create(&self, req: CreateStoryRequest) -> Result<Story, rusqlite::Error> {
        self.create(req)
    }
    fn get_all(&self) -> Result<Vec<Story>, rusqlite::Error> {
        self.get_all()
    }
    fn get_by_id(&self, id: &str) -> Result<Option<Story>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn update(&self, id: &str, req: &UpdateStoryRequest) -> Result<usize, rusqlite::Error> {
        self.update(id, req)
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
}

impl CharacterRepo for CharacterRepository {
    fn create(&self, req: CreateCharacterRequest) -> Result<Character, rusqlite::Error> {
        self.create(req)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Vec<Character>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn get_by_id(&self, id: &str) -> Result<Option<Character>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn update(
        &self,
        id: &str,
        name: Option<String>,
        background: Option<String>,
        personality: Option<String>,
        goals: Option<String>,
        appearance: Option<String>,
        gender: Option<String>,
        age: Option<i32>,
    ) -> Result<usize, rusqlite::Error> {
        self.update(
            id,
            name,
            background,
            personality,
            goals,
            appearance,
            gender,
            age,
        )
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
}

impl ChapterRepo for ChapterRepository {
    fn create(&self, req: CreateChapterRequest) -> Result<Chapter, rusqlite::Error> {
        self.create(req)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Vec<Chapter>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn get_by_id(&self, id: &str) -> Result<Option<Chapter>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn update(
        &self,
        id: &str,
        title: Option<String>,
        outline: Option<String>,
        content: Option<String>,
        word_count: Option<i32>,
    ) -> Result<usize, rusqlite::Error> {
        self.update(id, title, outline, content, word_count)
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
}

impl WorldBuildingRepo for WorldBuildingRepository {
    fn create(&self, story_id: &str, concept: &str) -> Result<WorldBuilding, rusqlite::Error> {
        self.create(story_id, concept)
    }
    fn get_by_id(&self, id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error> {
        self.get_by_id(id)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn update(
        &self,
        id: &str,
        concept: Option<&str>,
        rules: Option<&[WorldRule]>,
        history: Option<&str>,
        cultures: Option<&[Culture]>,
    ) -> Result<usize, rusqlite::Error> {
        self.update(id, concept, rules, history, cultures)
    }
    fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        self.delete(id)
    }
}

impl WritingStyleRepo for WritingStyleRepository {
    fn create(&self, story_id: &str, name: Option<&str>) -> Result<WritingStyle, rusqlite::Error> {
        self.create(story_id, name)
    }
    fn get_by_story(&self, story_id: &str) -> Result<Option<WritingStyle>, rusqlite::Error> {
        self.get_by_story(story_id)
    }
    fn update(&self, id: &str, updates: &WritingStyleUpdate) -> Result<usize, rusqlite::Error> {
        self.update(id, updates)
    }
}
