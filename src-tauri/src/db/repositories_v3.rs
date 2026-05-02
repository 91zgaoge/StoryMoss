//! V3 架构 Repository 层
#![allow(dead_code)]

use super::{DbPool, Scene, ConflictType, CharacterConflict, WorldBuilding, WorldRule, Culture};
use super::{WritingStyle, StudioConfig};
use super::{LlmStudioConfig, UiStudioConfig, AgentBotConfig, Entity, Relation};
use super::{SceneVersion, CreatorType, SceneAnnotation, TextAnnotation, StorySummary, ChangeTrack, ChangeType, ChangeStatus, CommentThread, CommentMessage, CommentThreadWithMessages, AnchorType, ThreadStatus};
use super::StoryStyleConfig;
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde::{Serialize, Deserialize};
use serde_json;
use uuid::Uuid;

// ==================== Scene Repository ====================

pub struct SceneRepository {
    pool: DbPool,
}

impl SceneRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, story_id: &str, sequence_number: i32, title: Option<&str>) -> Result<Scene, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        
        let mut conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        
        // 1. 先插入 scene（chapter_id 暂时为 NULL，避免外键约束冲突）
        tx.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, characters_present, character_conflicts, execution_stage, chapter_id, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9)",
            params![&id, story_id, sequence_number, title, "[]", "[]", "drafting", now.to_rfc3339(), now.to_rfc3339()],
        )?;
        
        // 2. 查找或创建关联 chapter
        let existing_chapter: Option<String> = tx.query_row(
            "SELECT id FROM chapters WHERE story_id = ?1 AND chapter_number = ?2",
            params![story_id, sequence_number],
            |row| row.get(0)
        ).optional()?;
        
        let chapter_id = if let Some(chapter_id) = existing_chapter {
            // Link to existing chapter
            tx.execute(
                "UPDATE chapters SET scene_id = ?1 WHERE id = ?2",
                params![&id, &chapter_id],
            )?;
            Some(chapter_id)
        } else {
            // Create a new chapter linked to this scene
            let chapter_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO chapters (id, story_id, chapter_number, title, word_count, model_used, cost, scene_id, created_at, updated_at) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![&chapter_id, story_id, sequence_number, title, 0, "", 0.0, &id, now.to_rfc3339(), now.to_rfc3339()],
            )?;
            Some(chapter_id)
        };
        
        // 3. 更新 scene 的 chapter_id（此时 chapter 已存在，外键约束满足）
        if let Some(ref cid) = chapter_id {
            tx.execute(
                "UPDATE scenes SET chapter_id = ?1 WHERE id = ?2",
                params![cid, &id],
            )?;
        }
        
        tx.commit()?;
        
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
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<Scene>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, sequence_number, title, dramatic_goal, external_pressure, conflict_type,
                    characters_present, character_conflicts, setting_location, setting_time, setting_atmosphere,
                    content, previous_scene_id, next_scene_id, model_used, cost, created_at, updated_at, confidence_score,
                    execution_stage, outline_content, draft_content, style_blend_override, foreshadowing_ids, chapter_id
             FROM scenes WHERE story_id = ?1 ORDER BY sequence_number"
        )?;

        let scenes = stmt.query_map([story_id], |row| {
            let conflict_type_str: Option<String> = row.get(6)?;
            let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());
            
            let chars_json: String = row.get(7)?;
            let characters_present: Vec<String> = serde_json::from_str(&chars_json).unwrap_or_default();
            
            let conflicts_json: String = row.get(8)?;
            let character_conflicts: Vec<CharacterConflict> = serde_json::from_str(&conflicts_json).unwrap_or_default();
            
            let created_str: String = row.get(17)?;
            let updated_str: String = row.get(18)?;
            let confidence_score: Option<f32> = row.get(19)?;
            let execution_stage: Option<String> = row.get(20)?;
            let outline_content: Option<String> = row.get(21)?;
            let draft_content: Option<String> = row.get(22)?;
            let foreshadowing_ids: Option<Vec<String>> = row.get::<_, Option<String>>(24)?.and_then(|s: String| serde_json::from_str(&s).ok());
            
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
                chapter_id: row.get(25)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(scenes)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Scene>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, sequence_number, title, dramatic_goal, external_pressure, conflict_type,
                    characters_present, character_conflicts, setting_location, setting_time, setting_atmosphere,
                    content, previous_scene_id, next_scene_id, model_used, cost, created_at, updated_at, confidence_score,
                    execution_stage, outline_content, draft_content, style_blend_override, foreshadowing_ids, chapter_id
             FROM scenes WHERE id = ?1"
        )?;

        let scene = stmt.query_row([id], |row| {
            let conflict_type_str: Option<String> = row.get(6)?;
            let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());
            
            let chars_json: String = row.get(7)?;
            let characters_present: Vec<String> = serde_json::from_str(&chars_json).unwrap_or_default();
            
            let conflicts_json: String = row.get(8)?;
            let character_conflicts: Vec<CharacterConflict> = serde_json::from_str(&conflicts_json).unwrap_or_default();
            
            let created_str: String = row.get(17)?;
            let updated_str: String = row.get(18)?;
            let confidence_score: Option<f32> = row.get(19)?;
            let execution_stage: Option<String> = row.get(20)?;
            let outline_content: Option<String> = row.get(21)?;
            let draft_content: Option<String> = row.get(22)?;
            let foreshadowing_ids: Option<Vec<String>> = row.get::<_, Option<String>>(24)?.and_then(|s: String| serde_json::from_str(&s).ok());
            
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
                chapter_id: row.get(25)?,
            })
        }).optional()?;

        Ok(scene)
    }

    pub fn update(&self, id: &str, updates: &SceneUpdate) -> Result<usize, rusqlite::Error> {
        let mut conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        
        let tx = conn.transaction()?;
        
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
                updates.characters_present.as_ref().map(|c| serde_json::to_string(c).unwrap()),
                updates.character_conflicts.as_ref().map(|c| serde_json::to_string(c).unwrap()),
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
                updates.foreshadowing_ids.as_ref().map(|c| serde_json::to_string(c).unwrap()),
                &now
            ],
        )?;
        
        // Sync associated chapter if title or content changed
        if updates.title.is_some() || updates.content.is_some() {
            let chapter_id: Option<String> = tx.query_row(
                "SELECT chapter_id FROM scenes WHERE id = ?1",
                [id],
                |row| row.get(0)
            ).optional()?;
            if let Some(cid) = chapter_id {
                tx.execute(
                    "UPDATE chapters SET title = COALESCE(?2, title), content = COALESCE(?3, content), updated_at = ?4 WHERE id = ?1",
                    params![cid, &updates.title, &updates.content, &now],
                )?;
            }
        }
        
        tx.commit()?;
        Ok(count)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM scenes WHERE id = ?1", [id])?;
        Ok(count)
    }

    pub fn update_sequence(&self, id: &str, new_sequence: i32) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let count = conn.execute(
            "UPDATE scenes SET sequence_number = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, new_sequence, now],
        )?;
        Ok(count)
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
    pub fn create_version(&self, scene: &Scene, change_summary: &str, created_by: CreatorType,
                          model_used: Option<&str>, confidence_score: Option<f32>) -> Result<SceneVersion, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        
        // 获取当前版本号
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let version_number: i32 = conn.query_row(
            "SELECT COALESCE(MAX(version_number), 0) + 1 FROM scene_versions WHERE scene_id = ?1",
            [&scene.id],
            |row| row.get(0)
        )?;
        
        // 获取上一个版本ID
        let previous_version_id: Option<String> = conn.query_row(
            "SELECT id FROM scene_versions WHERE scene_id = ?1 ORDER BY version_number DESC LIMIT 1",
            [&scene.id],
            |row| row.get(0)
        ).ok();
        
        let word_count = scene.content.as_ref().map(|c| c.len() as i32).unwrap_or(0);
        
        conn.execute(
            "INSERT INTO scene_versions (id, scene_id, version_number, title, content, dramatic_goal, 
             external_pressure, conflict_type, characters_present, character_conflicts,
             setting_location, setting_time, setting_atmosphere, word_count, change_summary,
             created_by, model_used, confidence_score, previous_version_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                &id, &scene.id, version_number, scene.title, scene.content, scene.dramatic_goal,
                scene.external_pressure, scene.conflict_type.as_ref().map(|c| c.to_string()),
                serde_json::to_string(&scene.characters_present).unwrap(),
                serde_json::to_string(&scene.character_conflicts).unwrap(),
                scene.setting_location, scene.setting_time, scene.setting_atmosphere,
                word_count, change_summary, created_by.to_string(), model_used, confidence_score,
                previous_version_id, now.to_rfc3339()
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, version_number, title, content, dramatic_goal, external_pressure,
                    conflict_type, characters_present, character_conflicts, setting_location, setting_time,
                    setting_atmosphere, word_count, change_summary, created_by, model_used, confidence_score,
                    previous_version_id, superseded_by, created_at
             FROM scene_versions WHERE scene_id = ?1 ORDER BY version_number DESC"
        )?;
        
        let versions = stmt.query_map([scene_id], |row| {
            let conflict_type_str: Option<String> = row.get(7)?;
            let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());
            
            let chars_json: String = row.get(8)?;
            let characters_present: Vec<String> = serde_json::from_str(&chars_json).unwrap_or_default();
            
            let conflicts_json: String = row.get(9)?;
            let character_conflicts: Vec<CharacterConflict> = serde_json::from_str(&conflicts_json).unwrap_or_default();
            
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
        })?.collect::<Result<Vec<_>, _>>()?;
        
        Ok(versions)
    }

    /// 获取特定版本
    pub fn get_version(&self, version_id: &str) -> Result<Option<SceneVersion>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, version_number, title, content, dramatic_goal, external_pressure,
                    conflict_type, characters_present, character_conflicts, setting_location, setting_time,
                    setting_atmosphere, word_count, change_summary, created_by, model_used, confidence_score,
                    previous_version_id, superseded_by, created_at
             FROM scene_versions WHERE id = ?1"
        )?;
        
        let version = stmt.query_row([version_id], |row| {
            let conflict_type_str: Option<String> = row.get(7)?;
            let conflict_type = conflict_type_str.and_then(|s| s.parse().ok());
            
            let chars_json: String = row.get(8)?;
            let characters_present: Vec<String> = serde_json::from_str(&chars_json).unwrap_or_default();
            
            let conflicts_json: String = row.get(9)?;
            let character_conflicts: Vec<CharacterConflict> = serde_json::from_str(&conflicts_json).unwrap_or_default();
            
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
        }).optional()?;
        
        Ok(version)
    }

    /// 删除版本
    pub fn delete_version(&self, version_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM scene_versions WHERE id = ?1", [version_id])?;
        Ok(count)
    }

    /// 获取场景版本数量
    pub fn get_version_count(&self, scene_id: &str) -> Result<i32, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM scene_versions WHERE scene_id = ?1",
            [scene_id],
            |row| row.get(0)
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

    pub fn create(&self, story_id: &str, concept: &str) -> Result<WorldBuilding, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![&id, story_id, concept, "[]", "", "[]", now.to_rfc3339(), now.to_rfc3339()],
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

    pub fn get_by_story(&self, story_id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, concept, rules, history, cultures, created_at, updated_at 
             FROM world_buildings WHERE story_id = ?1"
        )?;

        let wb = stmt.query_row([story_id], |row| {
            let rules_json: String = row.get(3)?;
            let rules: Vec<WorldRule> = serde_json::from_str(&rules_json).unwrap_or_default();
            
            let cultures_json: String = row.get(5)?;
            let cultures: Vec<Culture> = serde_json::from_str(&cultures_json).unwrap_or_default();
            
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
        }).optional()?;

        Ok(wb)
    }

    pub fn update(&self, id: &str, concept: Option<&str>, rules: Option<&[WorldRule]>, 
                  history: Option<&str>, cultures: Option<&[Culture]>) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        
        let count = conn.execute(
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
}

// ==================== WritingStyle Repository ====================

pub struct WritingStyleRepository {
    pool: DbPool,
}

impl WritingStyleRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, story_id: &str, name: Option<&str>) -> Result<WritingStyle, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO writing_styles (id, story_id, name, description, tone, pacing, 
             vocabulary_level, sentence_structure, custom_rules, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![&id, story_id, name, "", "", "", "", "", "[]", now.to_rfc3339(), now.to_rfc3339()],
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

    pub fn get_by_story(&self, story_id: &str) -> Result<Option<WritingStyle>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, description, tone, pacing, vocabulary_level, 
                    sentence_structure, custom_rules, created_at, updated_at 
             FROM writing_styles WHERE story_id = ?1"
        )?;

        let style = stmt.query_row([story_id], |row| {
            let rules_json: String = row.get(8)?;
            let custom_rules: Vec<String> = serde_json::from_str(&rules_json).unwrap_or_default();
            
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
        }).optional()?;

        Ok(style)
    }

    pub fn update(&self, id: &str, updates: &WritingStyleUpdate) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        
        let count = conn.execute(
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
                updates.custom_rules.as_ref().map(|r| serde_json::to_string(r).unwrap()),
                now
            ],
        )?;
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

    pub fn create_default(&self, story_id: &str, title: &str) -> Result<StudioConfig, rusqlite::Error> {
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
        
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO studio_configs (id, story_id, pen_name, llm_config, ui_config, 
             agent_bots, frontstage_theme, backstage_theme, created_at, updated_at) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &id, story_id, title,
                serde_json::to_string(&llm_config).unwrap(),
                serde_json::to_string(&ui_config).unwrap(),
                "[]",
                "paper", "dark",
                now.to_rfc3339(), now.to_rfc3339()
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, pen_name, llm_config, ui_config, agent_bots, 
                    frontstage_theme, backstage_theme, created_at, updated_at 
             FROM studio_configs WHERE story_id = ?1"
        )?;

        let config = stmt.query_row([story_id], |row| {
            let llm_json: String = row.get(3)?;
            let llm_config: LlmStudioConfig = serde_json::from_str(&llm_json).unwrap_or_default();
            
            let ui_json: String = row.get(4)?;
            let ui_config: UiStudioConfig = serde_json::from_str(&ui_json).unwrap_or_default();
            
            let bots_json: String = row.get(5)?;
            let agent_bots: Vec<AgentBotConfig> = serde_json::from_str(&bots_json).unwrap_or_default();
            
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
        }).optional()?;

        Ok(config)
    }

    /// 更新配置 (兼容旧接口)
    pub fn update(&self, id: &str, _pen_name: Option<&str>, 
                  llm_config: Option<&LlmStudioConfig>,
                  ui_config: Option<&UiStudioConfig>,
                  agent_bots: Option<&[AgentBotConfig]>) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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
    pub fn update_themes(&self, id: &str, frontstage_theme: Option<&str>, 
                         backstage_theme: Option<&str>) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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

    pub fn create_entity(&self, story_id: &str, name: &str, entity_type: &str, attributes: &serde_json::Value, embedding: Option<Vec<f32>>) 
        -> Result<Entity, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let embedding_blob = embedding.as_ref().map(|vec| {
            vec.iter().flat_map(|&f| f.to_le_bytes().to_vec()).collect::<Vec<u8>>()
        });
        
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO kg_entities (id, story_id, name, entity_type, attributes, embedding, first_seen, last_updated, is_archived) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0)",
            params![&id, story_id, name, entity_type, attributes.to_string(), embedding_blob, now.to_rfc3339(), now.to_rfc3339()],
        )?;
        
        Ok(Entity {
            id,
            story_id: story_id.to_string(),
            name: name.to_string(),
            entity_type: entity_type.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid entity type".to_string()))?,
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

    pub fn get_entities_by_story(&self, story_id: &str) -> Result<Vec<Entity>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at
             FROM kg_entities WHERE story_id = ?1 AND is_archived = 0"
        )?;

        let entities = stmt.query_map([story_id], |row| {
            let type_str: String = row.get(3)?;
            let entity_type = type_str.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid entity type".to_string()))?;
            
            let attrs_json: String = row.get(4)?;
            let attributes: serde_json::Value = serde_json::from_str(&attrs_json).unwrap_or_default();
            
            let embedding_blob: Option<Vec<u8>> = row.get(5)?;
            let embedding = embedding_blob.map(|bytes| {
                bytes.chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0;4])))
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(entities)
    }
    
    pub fn get_archived_entities(&self, story_id: &str) -> Result<Vec<Entity>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at
             FROM kg_entities WHERE story_id = ?1 AND is_archived = 1"
        )?;

        let entities = stmt.query_map([story_id], |row| {
            let type_str: String = row.get(3)?;
            let entity_type = type_str.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid entity type".to_string()))?;
            
            let attrs_json: String = row.get(4)?;
            let attributes: serde_json::Value = serde_json::from_str(&attrs_json).unwrap_or_default();
            
            let embedding_blob: Option<Vec<u8>> = row.get(5)?;
            let embedding = embedding_blob.map(|bytes| {
                bytes.chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0;4])))
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(entities)
    }
    
    pub fn archive_entity(&self, entity_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE kg_entities SET is_archived = 1, archived_at = ?2, last_updated = ?2 WHERE id = ?1",
            params![entity_id, now],
        )
    }
    
    pub fn restore_entity(&self, entity_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE kg_entities SET is_archived = 0, archived_at = NULL, last_updated = ?2 WHERE id = ?1",
            params![entity_id, now],
        )
    }

    pub fn create_relation(&self, story_id: &str, source_id: &str, target_id: &str, 
                           relation_type: &str, strength: f32) -> Result<Relation, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO kg_relations (id, story_id, source_id, target_id, relation_type, strength, evidence, first_seen) 
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![&id, story_id, source_id, target_id, relation_type, strength, "[]", now.to_rfc3339()],
        )?;
        
        Ok(Relation {
            id,
            story_id: story_id.to_string(),
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            relation_type: relation_type.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid relation type".to_string()))?,
            strength,
            evidence: vec![],
            first_seen: now,
            confidence_score: None,
        })
    }

    /// 批量保存 Ingest 生成的实体（已包含完整字段，直接 INSERT）
    pub fn save_entities_batch(&self, entities: &[Entity]) -> Result<usize, rusqlite::Error> {
        let mut conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let mut count = 0;
        for entity in entities {
            let embedding_blob = entity.embedding.as_ref().map(|vec| {
                vec.iter().flat_map(|&f| f.to_le_bytes().to_vec()).collect::<Vec<u8>>()
            });
            tx.execute(
                "INSERT INTO kg_entities (id, story_id, name, entity_type, attributes, embedding, first_seen, last_updated, confidence_score, access_count, last_accessed, is_archived, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(id) DO UPDATE SET
                     name=excluded.name,
                     attributes=excluded.attributes,
                     embedding=excluded.embedding,
                     last_updated=excluded.last_updated,
                     confidence_score=excluded.confidence_score",
                params![
                    &entity.id, &entity.story_id, &entity.name,
                    entity.entity_type.to_string(), entity.attributes.to_string(),
                    embedding_blob, entity.first_seen.to_rfc3339(), entity.last_updated.to_rfc3339(),
                    entity.confidence_score, entity.access_count,
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
        let mut conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let mut count = 0;
        for relation in relations {
            let evidence_json = serde_json::to_string(&relation.evidence).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "INSERT INTO kg_relations (id, story_id, source_id, target_id, relation_type, strength, evidence, first_seen, confidence_score)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     strength=excluded.strength,
                     evidence=excluded.evidence,
                     confidence_score=excluded.confidence_score",
                params![
                    &relation.id, &relation.story_id, &relation.source_id, &relation.target_id,
                    relation.relation_type.to_string(), relation.strength, evidence_json,
                    relation.first_seen.to_rfc3339(), relation.confidence_score
                ],
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn get_relations_by_entity(&self, entity_id: &str) -> Result<Vec<Relation>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, source_id, target_id, relation_type, strength, evidence, first_seen, confidence_score
             FROM kg_relations WHERE source_id = ?1 OR target_id = ?1"
        )?;

        let relations = stmt.query_map([entity_id], |row| {
            let type_str: String = row.get(4)?;
            let relation_type = type_str.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid relation type".to_string()))?;
            
            let evidence_json: String = row.get(6)?;
            let evidence: Vec<String> = serde_json::from_str(&evidence_json).unwrap_or_default();
            
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(relations)
    }

    pub fn get_relations_by_story(&self, story_id: &str) -> Result<Vec<Relation>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, source_id, target_id, relation_type, strength, evidence, first_seen, confidence_score
             FROM kg_relations WHERE story_id = ?1"
        )?;

        let relations = stmt.query_map([story_id], |row| {
            let type_str: String = row.get(4)?;
            let relation_type = type_str.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid relation type".to_string()))?;
            
            let evidence_json: String = row.get(6)?;
            let evidence: Vec<String> = serde_json::from_str(&evidence_json).unwrap_or_default();
            
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(relations)
    }
    
    pub fn get_entity_by_id(&self, entity_id: &str) -> Result<Option<Entity>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at
             FROM kg_entities WHERE id = ?1"
        )?;

        let entity = stmt.query_row([entity_id], |row| {
            let type_str: String = row.get(3)?;
            let entity_type = type_str.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid entity type".to_string()))?;
            let attrs_json: String = row.get(4)?;
            let attributes: serde_json::Value = serde_json::from_str(&attrs_json).unwrap_or_default();
            let embedding_blob: Option<Vec<u8>> = row.get(5)?;
            let embedding = embedding_blob.map(|bytes| {
                bytes.chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0;4])))
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
        }).optional()?;

        Ok(entity)
    }

    pub fn update_entity(&self, entity_id: &str, name: Option<&str>, attributes: Option<&serde_json::Value>, embedding: Option<Vec<f32>>) -> Result<Entity, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let entity = self.get_entity_by_id(entity_id)?
            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Entity not found".to_string()))?;

        let new_name = name.unwrap_or(&entity.name);
        let new_attributes = attributes.unwrap_or(&entity.attributes);
        let embedding_blob = embedding.as_ref().map(|vec| {
            vec.iter().flat_map(|&f| f.to_le_bytes().to_vec()).collect::<Vec<u8>>()
        });

        conn.execute(
            "UPDATE kg_entities SET name = ?2, attributes = ?3, embedding = ?4, last_updated = ?5 WHERE id = ?1",
            params![entity_id, new_name, new_attributes.to_string(), embedding_blob, now],
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at
             FROM kg_entities WHERE name = ?1 AND is_archived = 0 LIMIT 1"
        )?;

        let entity = stmt.query_row([name], |row| {
            let type_str: String = row.get(3)?;
            let entity_type = type_str.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid entity type".to_string()))?;
            let attrs_json: String = row.get(4)?;
            let attributes: serde_json::Value = serde_json::from_str(&attrs_json).unwrap_or_default();
            let embedding_blob: Option<Vec<u8>> = row.get(5)?;
            let embedding = embedding_blob.map(|bytes| {
                bytes.chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0;4])))
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
        }).optional()?;

        Ok(entity)
    }

    /// 获取与指定实体相关的实体及其关系强度
    pub fn get_related_entities(&self, entity_id: &str, min_strength: f32) -> Result<Vec<(Entity, f32)>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, strength FROM kg_relations 
             WHERE (source_id = ?1 OR target_id = ?1) AND strength >= ?2"
        )?;

        let rows = stmt.query_map(params![entity_id, min_strength], |row| {
            let source_id: String = row.get(0)?;
            let target_id: String = row.get(1)?;
            let strength: f32 = row.get(2)?;
            let other_id = if source_id == entity_id { target_id } else { source_id };
            Ok((other_id, strength))
        })?.collect::<Result<Vec<_>, _>>()?;

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
    ) -> Result<crate::db::models_v3::Entity, Box<dyn std::error::Error + Send + Sync>> {
        self.find_entity_by_name(name)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
            .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> { "Entity not found".into() })
    }

    async fn get_related_entities(
        &self,
        entity_id: &str,
        min_strength: f32,
    ) -> Result<Vec<(crate::db::models_v3::Entity, f32)>, Box<dyn std::error::Error + Send + Sync>> {
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

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO scene_annotations (id, scene_id, story_id, content, annotation_type, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![&id, scene_id, story_id, content, annotation_type, now.to_rfc3339(), now.to_rfc3339()],
        )?;

        Ok(SceneAnnotation {
            id,
            scene_id: scene_id.to_string(),
            story_id: story_id.to_string(),
            content: content.to_string(),
            annotation_type: annotation_type.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string()))?,
            created_at: now,
            updated_at: now,
            resolved_at: None,
        })
    }

    pub fn get_annotations_by_scene(&self, scene_id: &str) -> Result<Vec<SceneAnnotation>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, story_id, content, annotation_type, created_at, updated_at, resolved_at
             FROM scene_annotations WHERE scene_id = ?1 ORDER BY created_at DESC"
        )?;

        let annotations = stmt.query_map([scene_id], |row| {
            let type_str: String = row.get(4)?;
            let annotation_type = type_str.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string()))?;
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(annotations)
    }

    pub fn get_unresolved_annotations_by_story(&self, story_id: &str) -> Result<Vec<SceneAnnotation>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, story_id, content, annotation_type, created_at, updated_at, resolved_at
             FROM scene_annotations WHERE story_id = ?1 AND resolved_at IS NULL ORDER BY created_at DESC"
        )?;

        let annotations = stmt.query_map([story_id], |row| {
            let type_str: String = row.get(4)?;
            let annotation_type = type_str.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string()))?;
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(annotations)
    }

    pub fn update_annotation(&self, annotation_id: &str, content: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE scene_annotations SET content = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, content, now],
        )
    }

    pub fn resolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE scene_annotations SET resolved_at = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, now, now],
        )
    }

    pub fn unresolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE scene_annotations SET resolved_at = NULL, updated_at = ?2 WHERE id = ?1",
            params![annotation_id, now],
        )
    }

    pub fn delete_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO text_annotations (id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, to_pos, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![&id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, to_pos, now.to_rfc3339(), now.to_rfc3339()],
        )?;

        Ok(TextAnnotation {
            id,
            story_id: story_id.to_string(),
            scene_id: scene_id.map(|s| s.to_string()),
            chapter_id: chapter_id.map(|s| s.to_string()),
            content: content.to_string(),
            annotation_type: annotation_type.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string()))?,
            from_pos,
            to_pos,
            created_at: now,
            updated_at: now,
            resolved_at: None,
        })
    }

    pub fn get_annotations_by_chapter(&self, chapter_id: &str) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, to_pos, created_at, updated_at, resolved_at
             FROM text_annotations WHERE chapter_id = ?1 AND resolved_at IS NULL ORDER BY from_pos ASC"
        )?;
        let rows = stmt.query([chapter_id])?;
        Self::map_annotations(rows)
    }

    pub fn get_annotations_by_scene(&self, scene_id: &str) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, to_pos, created_at, updated_at, resolved_at
             FROM text_annotations WHERE scene_id = ?1 AND resolved_at IS NULL ORDER BY from_pos ASC"
        )?;
        let rows = stmt.query([scene_id])?;
        Self::map_annotations(rows)
    }

    pub fn get_annotations_by_story(&self, story_id: &str) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, content, annotation_type, from_pos, to_pos, created_at, updated_at, resolved_at
             FROM text_annotations WHERE story_id = ?1 AND resolved_at IS NULL ORDER BY created_at DESC"
        )?;
        let rows = stmt.query([story_id])?;
        Self::map_annotations(rows)
    }

    fn map_annotations(mut rows: rusqlite::Rows<'_>) -> Result<Vec<TextAnnotation>, rusqlite::Error> {
        let mut annotations = Vec::new();
        while let Some(row) = rows.next()? {
            let type_str: String = row.get(5)?;
            let annotation_type = type_str.parse().map_err(|_| rusqlite::Error::InvalidParameterName("Invalid annotation type".to_string()))?;
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

    pub fn update_annotation(&self, annotation_id: &str, content: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE text_annotations SET content = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, content, now],
        )
    }

    pub fn resolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE text_annotations SET resolved_at = ?2, updated_at = ?3 WHERE id = ?1",
            params![annotation_id, now, now],
        )
    }

    pub fn unresolve_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE text_annotations SET resolved_at = NULL, updated_at = ?2 WHERE id = ?1",
            params![annotation_id, now],
        )
    }

    pub fn delete_annotation(&self, annotation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO story_summaries (id, story_id, summary_type, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![&id, story_id, summary_type, content, now.to_rfc3339(), now.to_rfc3339()],
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

    pub fn get_summaries_by_story(&self, story_id: &str) -> Result<Vec<StorySummary>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, summary_type, content, created_at, updated_at
             FROM story_summaries WHERE story_id = ?1 ORDER BY updated_at DESC"
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

    pub fn get_summary_by_type(&self, story_id: &str, summary_type: &str) -> Result<Option<StorySummary>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let result = conn.query_row(
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
        ).optional()?;
        Ok(result)
    }

    pub fn update_summary(&self, id: &str, content: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE story_summaries SET content = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, content, now],
        )
    }

    pub fn delete_summary(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM story_summaries WHERE id = ?1",
            params![id],
        )
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO change_tracks (id, scene_id, chapter_id, version_id, author_id, author_name, change_type, from_pos, to_pos, content, status, created_at, resolved_at)
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE id = ?1"
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE scene_id = ?1 ORDER BY created_at DESC"
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

    pub fn get_pending_by_scene(&self, scene_id: &str) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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

    pub fn get_pending_by_chapter(&self, chapter_id: &str) -> Result<Vec<ChangeTrack>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE chapter_id = ?1 AND status = 'Pending' ORDER BY created_at DESC"
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, author_id, author_name, change_type, from_pos, to_pos, content, status, created_at, resolved_at
             FROM change_tracks WHERE version_id = ?1 ORDER BY created_at DESC"
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Accepted', resolved_at = ?2 WHERE scene_id = ?1 AND status = 'Pending'",
            params![scene_id, now],
        )
    }

    pub fn reject_all_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Rejected', resolved_at = ?2 WHERE scene_id = ?1 AND status = 'Pending'",
            params![scene_id, now],
        )
    }

    pub fn accept_all_by_chapter(&self, chapter_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Accepted', resolved_at = ?2 WHERE chapter_id = ?1 AND status = 'Pending'",
            params![chapter_id, now],
        )
    }

    pub fn reject_all_by_chapter(&self, chapter_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE change_tracks SET status = 'Rejected', resolved_at = ?2 WHERE chapter_id = ?1 AND status = 'Pending'",
            params![chapter_id, now],
        )
    }

    pub fn delete_by_scene(&self, scene_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO comment_threads (id, scene_id, chapter_id, version_id, anchor_type, from_pos, to_pos, selected_text, status, created_at, resolved_at)
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO comment_messages (id, thread_id, author_id, author_name, content, created_at)
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

    pub fn get_threads_by_chapter(&self, chapter_id: &str) -> Result<Vec<CommentThreadWithMessages>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, anchor_type, from_pos, to_pos, selected_text, status, created_at, resolved_at
             FROM comment_threads WHERE chapter_id = ?1 ORDER BY created_at DESC"
        )?;

        let threads: Vec<CommentThread> = stmt.query_map([chapter_id], |row| self.parse_thread(row))?.collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for thread in threads {
            let messages = self.get_messages(&thread.id)?;
            result.push(CommentThreadWithMessages { thread, messages });
        }
        Ok(result)
    }

    pub fn get_threads_by_scene(&self, scene_id: &str) -> Result<Vec<CommentThreadWithMessages>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, chapter_id, version_id, anchor_type, from_pos, to_pos, selected_text, status, created_at, resolved_at
             FROM comment_threads WHERE scene_id = ?1 ORDER BY created_at DESC"
        )?;

        let threads: Vec<CommentThread> = stmt.query_map([scene_id], |row| self.parse_thread(row))?.collect::<Result<Vec<_>, _>>()?;

        let mut result = Vec::new();
        for thread in threads {
            let messages = self.get_messages(&thread.id)?;
            result.push(CommentThreadWithMessages { thread, messages });
        }
        Ok(result)
    }

    pub fn get_messages(&self, thread_id: &str) -> Result<Vec<CommentMessage>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, thread_id, author_id, author_name, content, created_at
             FROM comment_messages WHERE thread_id = ?1 ORDER BY created_at ASC"
        )?;

        let rows = stmt.query_map([thread_id], |row| self.parse_message(row))?;
        rows.collect()
    }

    pub fn resolve_thread(&self, thread_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE comment_threads SET status = 'Resolved', resolved_at = ?2 WHERE id = ?1",
            params![thread_id, now],
        )
    }

    pub fn reopen_thread(&self, thread_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "UPDATE comment_threads SET status = 'Open', resolved_at = NULL WHERE id = ?1",
            params![thread_id],
        )
    }

    pub fn delete_thread(&self, thread_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM comment_threads WHERE id = ?1",
            params![thread_id],
        )
    }
}


// ==================== StoryStyleConfig Repository (v4.4.0 - 风格混合配置) ====================

pub struct StoryStyleConfigRepository {
    pool: DbPool,
}

impl StoryStyleConfigRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, story_id: &str, name: &str, blend_json: &str) -> Result<StoryStyleConfig, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO story_style_configs (id, story_id, name, blend_json, is_active, created_at, updated_at)
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

    pub fn get_active_by_story(&self, story_id: &str) -> Result<Option<StoryStyleConfig>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, blend_json, is_active, created_at, updated_at
             FROM story_style_configs WHERE story_id = ?1 AND is_active = 1 LIMIT 1"
        )?;

        let result = stmt.query_row([story_id], |row| {
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
        }).optional()?;

        Ok(result)
    }

    pub fn get_all_by_story(&self, story_id: &str) -> Result<Vec<StoryStyleConfig>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, blend_json, is_active, created_at, updated_at
             FROM story_style_configs WHERE story_id = ?1 ORDER BY updated_at DESC"
        )?;

        let configs = stmt.query_map([story_id], |row| {
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(configs)
    }

    pub fn update(&self, id: &str, name: Option<&str>, blend_json: Option<&str>) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM story_style_configs WHERE id = ?1",
            params![id],
        )
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

    pub fn create(&self, name: &str, author: Option<&str>, dna_json: &str, is_builtin: bool) -> Result<super::models_v3::StyleDNA, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO style_dnas (id, name, author, dna_json, is_builtin, is_user_created, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![&id, name, author, dna_json, is_builtin as i32, !is_builtin as i32, now],
        )?;

        Ok(super::models_v3::StyleDNA {
            id,
            name: name.to_string(),
            author: author.map(|s| s.to_string()),
            dna_json: dna_json.to_string(),
            is_builtin,
            is_user_created: !is_builtin,
            created_at: Local::now(),
        })
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<super::models_v3::StyleDNA>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas WHERE id = ?1"
        )?;

        let result = stmt.query_row([id], |row| {
            let is_builtin: i32 = row.get(4)?;
            let is_user_created: i32 = row.get(5)?;
            let created_str: String = row.get(6)?;
            Ok(super::models_v3::StyleDNA {
                id: row.get(0)?,
                name: row.get(1)?,
                author: row.get(2)?,
                dna_json: row.get(3)?,
                is_builtin: is_builtin != 0,
                is_user_created: is_user_created != 0,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;

        Ok(result)
    }

    pub fn get_all(&self) -> Result<Vec<super::models_v3::StyleDNA>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas ORDER BY is_builtin DESC, name ASC"
        )?;

        let dnas = stmt.query_map([], |row| {
            let is_builtin: i32 = row.get(4)?;
            let is_user_created: i32 = row.get(5)?;
            let created_str: String = row.get(6)?;
            Ok(super::models_v3::StyleDNA {
                id: row.get(0)?,
                name: row.get(1)?,
                author: row.get(2)?,
                dna_json: row.get(3)?,
                is_builtin: is_builtin != 0,
                is_user_created: is_user_created != 0,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(dnas)
    }

    pub fn get_builtin(&self) -> Result<Vec<super::models_v3::StyleDNA>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, author, dna_json, is_builtin, is_user_created, created_at
             FROM style_dnas WHERE is_builtin = 1 ORDER BY name ASC"
        )?;

        let dnas = stmt.query_map([], |row| {
            let is_builtin: i32 = row.get(4)?;
            let is_user_created: i32 = row.get(5)?;
            let created_str: String = row.get(6)?;
            Ok(super::models_v3::StyleDNA {
                id: row.get(0)?,
                name: row.get(1)?,
                author: row.get(2)?,
                dna_json: row.get(3)?,
                is_builtin: is_builtin != 0,
                is_user_created: is_user_created != 0,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(dnas)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM style_dnas WHERE id = ?1 AND is_builtin = 0",
            params![id],
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
    ) -> Result<super::models_v3::UserFeedbackLog, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO user_feedback_log (id, story_id, scene_id, chapter_id, feedback_type, agent_type, original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &id, story_id, scene_id, chapter_id, feedback_type,
                agent_type, original_ai_text, final_text,
                ai_score, user_satisfaction,
                metadata.map(|m| m.to_string()), now
            ],
        )?;

        Ok(super::models_v3::UserFeedbackLog {
            id,
            story_id: story_id.to_string(),
            scene_id: scene_id.map(|s| s.to_string()),
            chapter_id: chapter_id.map(|s| s.to_string()),
            feedback_type: feedback_type.parse().unwrap_or(super::models_v3::FeedbackType::Accept),
            agent_type: agent_type.map(|s| s.to_string()),
            original_ai_text: original_ai_text.to_string(),
            final_text: final_text.to_string(),
            ai_score,
            user_satisfaction,
            metadata: metadata.cloned(),
            created_at: Local::now(),
        })
    }

    pub fn get_by_story(&self, story_id: &str, limit: Option<i64>) -> Result<Vec<super::models_v3::UserFeedbackLog>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let sql = if let Some(lim) = limit {
            format!(
                "SELECT id, story_id, scene_id, chapter_id, feedback_type, agent_type, original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at
                 FROM user_feedback_log WHERE story_id = ?1 ORDER BY created_at DESC LIMIT {}",
                lim
            )
        } else {
            "SELECT id, story_id, scene_id, chapter_id, feedback_type, agent_type, original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at
             FROM user_feedback_log WHERE story_id = ?1 ORDER BY created_at DESC".to_string()
        };
        let mut stmt = conn.prepare(&sql)?;

        let logs = stmt.query_map([story_id], |row| {
            let meta_str: Option<String> = row.get(10)?;
            let meta = meta_str.and_then(|s| serde_json::from_str(&s).ok());
            let created_str: String = row.get(11)?;
            Ok(super::models_v3::UserFeedbackLog {
                id: row.get(0)?,
                story_id: row.get(1)?,
                scene_id: row.get(2)?,
                chapter_id: row.get(3)?,
                feedback_type: row.get::<_, String>(4)?.parse().unwrap_or(super::models_v3::FeedbackType::Accept),
                agent_type: row.get(5)?,
                original_ai_text: row.get(6)?,
                final_text: row.get(7)?,
                ai_score: row.get(8)?,
                user_satisfaction: row.get(9)?,
                metadata: meta,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    pub fn get_recent(&self, story_id: &str, days: i64) -> Result<Vec<super::models_v3::UserFeedbackLog>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let cutoff = (Local::now() - chrono::Duration::days(days)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, feedback_type, agent_type, original_ai_text, final_text, ai_score, user_satisfaction, metadata, created_at
             FROM user_feedback_log WHERE story_id = ?1 AND created_at >= ?2 ORDER BY created_at DESC"
        )?;

        let logs = stmt.query_map(params![story_id, cutoff], |row| {
            let meta_str: Option<String> = row.get(10)?;
            let meta = meta_str.and_then(|s| serde_json::from_str(&s).ok());
            let created_str: String = row.get(11)?;
            Ok(super::models_v3::UserFeedbackLog {
                id: row.get(0)?,
                story_id: row.get(1)?,
                scene_id: row.get(2)?,
                chapter_id: row.get(3)?,
                feedback_type: row.get::<_, String>(4)?.parse().unwrap_or(super::models_v3::FeedbackType::Accept),
                agent_type: row.get(5)?,
                original_ai_text: row.get(6)?,
                final_text: row.get(7)?,
                ai_score: row.get(8)?,
                user_satisfaction: row.get(9)?,
                metadata: meta,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    pub fn get_stats(&self, story_id: &str) -> Result<FeedbackStats, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT feedback_type, COUNT(*) FROM user_feedback_log WHERE story_id = ?1 GROUP BY feedback_type"
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

        Ok(FeedbackStats { accept, reject, modify })
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
    ) -> Result<super::models_v3::UserPreference, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        // 先检查是否已存在
        let existing: Option<String> = conn.query_row(
            "SELECT id FROM user_preferences WHERE story_id = ?1 AND preference_type = ?2 AND preference_key = ?3",
            params![story_id, preference_type, preference_key],
            |row| row.get(0),
        ).optional()?;

        if let Some(id) = existing {
            // 更新
            conn.execute(
                "UPDATE user_preferences SET preference_value = ?4, confidence = ?5, evidence_count = ?6, updated_at = ?7
                 WHERE id = ?1",
                params![&id, preference_value, confidence, evidence_count, now],
            )?;

            Ok(super::models_v3::UserPreference {
                id,
                story_id: story_id.to_string(),
                preference_type: preference_type.parse().unwrap_or(super::models_v3::PreferenceType::Content),
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
                "INSERT INTO user_preferences (id, story_id, preference_type, preference_key, preference_value, confidence, evidence_count, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![&id, story_id, preference_type, preference_key, preference_value, confidence, evidence_count, now],
            )?;

            Ok(super::models_v3::UserPreference {
                id,
                story_id: story_id.to_string(),
                preference_type: preference_type.parse().unwrap_or(super::models_v3::PreferenceType::Content),
                preference_key: preference_key.to_string(),
                preference_value: preference_value.to_string(),
                confidence,
                evidence_count,
                updated_at: Local::now(),
            })
        }
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<super::models_v3::UserPreference>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, preference_type, preference_key, preference_value, confidence, evidence_count, updated_at
             FROM user_preferences WHERE story_id = ?1 ORDER BY confidence DESC"
        )?;

        let prefs = stmt.query_map([story_id], |row| {
            let updated_str: String = row.get(7)?;
            Ok(super::models_v3::UserPreference {
                id: row.get(0)?,
                story_id: row.get(1)?,
                preference_type: row.get::<_, String>(2)?.parse().unwrap_or(super::models_v3::PreferenceType::Content),
                preference_key: row.get(3)?,
                preference_value: row.get(4)?,
                confidence: row.get(5)?,
                evidence_count: row.get(6)?,
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(prefs)
    }

    pub fn get_by_type(&self, story_id: &str, pref_type: &str) -> Result<Vec<super::models_v3::UserPreference>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, preference_type, preference_key, preference_value, confidence, evidence_count, updated_at
             FROM user_preferences WHERE story_id = ?1 AND preference_type = ?2 ORDER BY confidence DESC"
        )?;

        let prefs = stmt.query_map(params![story_id, pref_type], |row| {
            let updated_str: String = row.get(7)?;
            Ok(super::models_v3::UserPreference {
                id: row.get(0)?,
                story_id: row.get(1)?,
                preference_type: row.get::<_, String>(2)?.parse().unwrap_or(super::models_v3::PreferenceType::Content),
                preference_key: row.get(3)?,
                preference_value: row.get(4)?,
                confidence: row.get(5)?,
                evidence_count: row.get(6)?,
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(prefs)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM user_preferences WHERE id = ?1",
            params![id],
        )
    }
}


// ==================== Story Outline Repository (v5.0.0 - 创世引擎) ====================

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
    ) -> Result<super::models_v3::StoryOutline, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO story_outlines (id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![&id, story_id, content, structure_json, act_count, total_scenes_estimate, now.to_rfc3339(), now.to_rfc3339()],
        )?;

        Ok(super::models_v3::StoryOutline {
            id,
            story_id: story_id.to_string(),
            content: content.to_string(),
            structure_json: structure_json.map(|s| s.to_string()),
            act_count,
            total_scenes_estimate,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Option<super::models_v3::StoryOutline>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, content, structure_json, act_count, total_scenes_estimate, created_at, updated_at
             FROM story_outlines WHERE story_id = ?1"
        )?;

        let outline = stmt.query_row([story_id], |row| {
            let created_str: String = row.get(6)?;
            let updated_str: String = row.get(7)?;

            Ok(super::models_v3::StoryOutline {
                id: row.get(0)?,
                story_id: row.get(1)?,
                content: row.get(2)?,
                structure_json: row.get(3)?,
                act_count: row.get(4)?,
                total_scenes_estimate: row.get(5)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;

        Ok(outline)
    }

    pub fn update(
        &self,
        story_id: &str,
        content: Option<&str>,
        structure_json: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE story_outlines SET content = COALESCE(?2, content), structure_json = COALESCE(?3, structure_json), updated_at = ?4 WHERE story_id = ?1",
            params![story_id, content, structure_json, now],
        )?;
        Ok(count)
    }

    pub fn delete(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM story_outlines WHERE story_id = ?1", [story_id])
    }
}

// ==================== Character Relationship Repository (v5.0.0 - 创世引擎) ====================

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
    ) -> Result<super::models_v3::CharacterRelationship, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO character_relationships (id, story_id, source_character_id, target_character_id, relationship_type, description, dynamic, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![&id, story_id, source_character_id, target_character_id, relationship_type, description, dynamic, now.to_rfc3339()],
        )?;

        Ok(super::models_v3::CharacterRelationship {
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

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<super::models_v3::CharacterRelationship>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT r.id, r.story_id, r.source_character_id, r.target_character_id, c.name as target_name,
                    r.relationship_type, r.description, r.dynamic, r.created_at
             FROM character_relationships r
             LEFT JOIN characters c ON r.target_character_id = c.id
             WHERE r.story_id = ?1
             ORDER BY r.created_at"
        )?;

        let relationships = stmt.query_map([story_id], |row| {
            let created_str: String = row.get(8)?;

            Ok(super::models_v3::CharacterRelationship {
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(relationships)
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM character_relationships WHERE story_id = ?1", [story_id])
    }
}
