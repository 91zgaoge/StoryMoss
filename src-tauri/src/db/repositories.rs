#![allow(unused_imports)]
//! Repository 层

use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json;
use uuid::Uuid;

#[allow(unused_imports)]
use super::{
    AnchorType, Chapter, Character, CreateChapterRequest, CreateCharacterRequest,
    CreateStoryRequest, Culture, DbPool, OAuthAccount, Scene, Session, Story, StoryStyleConfig,
    UpdateStoryRequest, User, UserInfo, WorldBuilding, WorldRule, WritingStyle,
};
// Re-export 已拆分的 Repository（保持向后兼容）
pub use crate::db::repositories_change_track::ChangeTrackRepository;
pub use crate::db::{
    repositories_chapter::ChapterRepository,
    repositories_character::CharacterRepository,
    repositories_comment_thread::CommentThreadRepository,
    repositories_knowledge_graph::KnowledgeGraphRepository,
    repositories_scene::{SceneRepository, SceneUpdate},
    repositories_scene_annotation::SceneAnnotationRepository,
    repositories_scene_version::SceneVersionRepository,
    repositories_story::StoryRepository,
    repositories_story_summary::StorySummaryRepository,
    repositories_studio_config::StudioConfigRepository,
    repositories_text_annotation::TextAnnotationRepository,
    repositories_world_building::WorldBuildingRepository,
    repositories_writing_style::{WritingStyleRepository, WritingStyleUpdate},
};
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
