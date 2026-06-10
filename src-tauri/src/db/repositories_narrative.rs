#![allow(unused_imports)]
//! 统一叙事元素 Repository
//!
//! 操作 narrative_characters / narrative_scenes / narrative_world_buildings
//! 表。 生产表（characters/scenes/
//! world_buildings）和参考表（reference_characters/reference_scenes）
//! 的数据最终都汇聚到这些统一表中。

use chrono::Local;
use rusqlite::params;

use super::DbPool;
use crate::narrative::elements::*;

// ==================== Character Repository ====================

pub struct NarrativeCharacterRepository {
    pool: DbPool,
}

impl NarrativeCharacterRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, character: &CharacterElement) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let _relationships_json =
            serde_json::to_string(&character.relationships).unwrap_or_default();

        conn.execute(
            "INSERT INTO narrative_characters (
                id, story_id, name, role_type, personality, background, goals, appearance,
                gender, age, importance_score, source, source_ref_id, status, created_at, \
             updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
            params![
                character.id,
                character.story_id,
                character.name,
                character.role_type,
                character.personality,
                character.background,
                character.goals,
                character.appearance,
                character.gender,
                character.age,
                character.importance_score,
                format!("{}", character.source),
                character.source_ref_id,
                format!("{}", character.status),
                now
            ],
        )?;

        Ok(())
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<CharacterElement>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, role_type, personality, background, goals, appearance,
                    gender, age, importance_score, source, source_ref_id, status
             FROM narrative_characters WHERE story_id = ?1 ORDER BY importance_score DESC",
        )?;

        let mut characters: Vec<CharacterElement> = stmt
            .query_map([story_id], |row| {
                Ok(CharacterElement {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    role_type: row.get(3)?,
                    personality: row.get(4)?,
                    background: row.get(5)?,
                    goals: row.get(6)?,
                    // narrative_characters 表缺少 fears 列，暂时返回空字符串
                    fears: String::new(),
                    appearance: row.get(7)?,
                    gender: row.get(8)?,
                    age: row.get(9)?,
                    importance_score: row.get(10)?,
                    source: parse_source(&row.get::<_, String>(11).unwrap_or_default()),
                    source_ref_id: row.get(12)?,
                    status: parse_status(&row.get::<_, String>(13).unwrap_or_default()),
                    relationships: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // P0-3 修复: 从 character_relationships 表二次查询填充 relationships
        for character in &mut characters {
            let mut rel_stmt = conn.prepare(
                "SELECT cr.relationship_type, cr.description, c.name as target_name
                 FROM character_relationships cr
                 LEFT JOIN characters c ON cr.target_character_id = c.id
                 WHERE cr.source_character_id = ?1",
            )?;
            let rel_rows = rel_stmt.query_map([&character.id], |row| {
                Ok(CharacterRelationship {
                    relation_type: row.get(0)?,
                    description: row.get(1)?,
                    target_name: row.get(2)?,
                })
            })?;
            character.relationships = rel_rows.collect::<Result<Vec<_>, _>>()?;
        }

        Ok(characters)
    }

    pub fn create_batch(&self, characters: &[CharacterElement]) -> Result<(), rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let now = Local::now().to_rfc3339();
        for character in characters {
            let _relationships_json =
                serde_json::to_string(&character.relationships).unwrap_or_default();
            tx.execute(
                "INSERT INTO narrative_characters (
                    id, story_id, name, role_type, personality, background, goals, appearance,
                    gender, age, importance_score, source, source_ref_id, status, created_at, \
                 updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
                params![
                    character.id,
                    character.story_id,
                    character.name,
                    character.role_type,
                    character.personality,
                    character.background,
                    character.goals,
                    character.appearance,
                    character.gender,
                    character.age,
                    character.importance_score,
                    format!("{}", character.source),
                    character.source_ref_id,
                    format!("{}", character.status),
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM narrative_characters WHERE story_id = ?1",
            [story_id],
        )
    }
}

// ==================== Scene Repository ====================

pub struct NarrativeSceneRepository {
    pool: DbPool,
}

impl NarrativeSceneRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, scene: &SceneElement) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let chars_present_json =
            serde_json::to_string(&scene.characters_present).unwrap_or_default();

        conn.execute(
            "INSERT INTO narrative_scenes (
                id, story_id, sequence_number, title, summary, dramatic_goal, external_pressure,
                conflict_type, characters_present, setting_location, setting_time, content,
                source, source_ref_id, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16)",
            params![
                scene.id,
                scene.story_id,
                scene.sequence_number,
                scene.title,
                scene.summary,
                scene.dramatic_goal,
                scene.external_pressure,
                scene.conflict_type,
                chars_present_json,
                scene.setting_location,
                scene.setting_time,
                scene.content,
                format!("{}", scene.source),
                scene.source_ref_id,
                format!("{}", scene.status),
                now
            ],
        )?;

        Ok(())
    }

    pub fn create_batch(&self, scenes: &[SceneElement]) -> Result<(), rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let now = Local::now().to_rfc3339();
        for scene in scenes {
            let chars_present_json =
                serde_json::to_string(&scene.characters_present).unwrap_or_default();
            tx.execute(
                "INSERT INTO narrative_scenes (
                    id, story_id, sequence_number, title, summary, dramatic_goal, \
                 external_pressure,
                    conflict_type, characters_present, setting_location, setting_time, content,
                    source, source_ref_id, status, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, \
                 ?16)",
                params![
                    scene.id,
                    scene.story_id,
                    scene.sequence_number,
                    scene.title,
                    scene.summary,
                    scene.dramatic_goal,
                    scene.external_pressure,
                    scene.conflict_type,
                    chars_present_json,
                    scene.setting_location,
                    scene.setting_time,
                    scene.content,
                    format!("{}", scene.source),
                    scene.source_ref_id,
                    format!("{}", scene.status),
                    now
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<SceneElement>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, sequence_number, title, summary, dramatic_goal, \
             external_pressure,
                    conflict_type, characters_present, setting_location, setting_time, content,
                    source, source_ref_id, status
             FROM narrative_scenes WHERE story_id = ?1 ORDER BY sequence_number",
        )?;

        let rows = stmt.query_map([story_id], |row| {
            let chars_json: String = row.get(8).unwrap_or_default();
            let characters_present: Vec<String> =
                serde_json::from_str(&chars_json).unwrap_or_default();

            Ok(SceneElement {
                id: row.get(0)?,
                story_id: row.get(1)?,
                sequence_number: row.get(2)?,
                title: row.get(3)?,
                summary: row.get(4)?,
                dramatic_goal: row.get(5)?,
                external_pressure: row.get(6)?,
                conflict_type: row.get(7)?,
                characters_present,
                setting_location: row.get(9)?,
                setting_time: row.get(10)?,
                content: row.get(11)?,
                narrative_intensity: 0.0,
                narrative_sentiment: 0.0,
                narrative_event_types: Vec::new(),
                act_number: 1,
                position_in_act: 0.0,
                source: parse_source(&row.get::<_, String>(12).unwrap_or_default()),
                source_ref_id: row.get(13)?,
                status: parse_status(&row.get::<_, String>(14).unwrap_or_default()),
            })
        })?;

        rows.collect()
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM narrative_scenes WHERE story_id = ?1",
            [story_id],
        )
    }
}

// ==================== WorldBuilding Repository ====================

pub struct NarrativeWorldBuildingRepository {
    pool: DbPool,
}

impl NarrativeWorldBuildingRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, wb: &WorldBuildingElement) -> Result<(), rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let rules_json = serde_json::to_string(&wb.rules).unwrap_or_default();
        let locations_json = serde_json::to_string(&wb.key_locations).unwrap_or_default();

        conn.execute(
            "INSERT INTO narrative_world_buildings (
                id, story_id, concept, rules, history, key_locations, power_system,
                source, source_ref_id, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
            params![
                wb.id,
                wb.story_id,
                wb.concept,
                rules_json,
                wb.history,
                locations_json,
                wb.power_system,
                format!("{}", wb.source),
                wb.source_ref_id,
                format!("{}", wb.status),
                now
            ],
        )?;

        Ok(())
    }

    pub fn get_by_story(
        &self,
        story_id: &str,
    ) -> Result<Option<WorldBuildingElement>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, concept, rules, history, key_locations, power_system,
                    source, source_ref_id, status
             FROM narrative_world_buildings WHERE story_id = ?1",
        )?;

        let mut rows = stmt.query_map([story_id], |row| {
            let rules_json: String = row.get(3).unwrap_or_default();
            let rules: Vec<WorldRule> = serde_json::from_str(&rules_json).unwrap_or_default();
            let locations_json: String = row.get(5).unwrap_or_default();
            let key_locations: Vec<String> =
                serde_json::from_str(&locations_json).unwrap_or_default();

            Ok(WorldBuildingElement {
                id: row.get(0)?,
                story_id: row.get(1)?,
                concept: row.get(2)?,
                rules,
                history: row.get(4)?,
                key_locations,
                power_system: row.get(6)?,
                source: parse_source(&row.get::<_, String>(7).unwrap_or_default()),
                source_ref_id: row.get(8)?,
                status: parse_status(&row.get::<_, String>(9).unwrap_or_default()),
            })
        })?;

        rows.next().transpose()
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM narrative_world_buildings WHERE story_id = ?1",
            [story_id],
        )
    }
}

// ==================== 辅助函数 ====================

fn parse_source(s: &str) -> ElementSource {
    match s {
        "generated" => ElementSource::Generated,
        "extracted" => ElementSource::Extracted,
        "user_created" => ElementSource::UserCreated,
        "imported" => ElementSource::Imported,
        _ => ElementSource::UserCreated,
    }
}

fn parse_status(s: &str) -> crate::narrative::elements::ElementStatus {
    use crate::narrative::elements::ElementStatus;
    match s {
        "active" => ElementStatus::Active,
        "reference" => ElementStatus::Reference,
        "archived" => ElementStatus::Archived,
        _ => ElementStatus::Active,
    }
}
