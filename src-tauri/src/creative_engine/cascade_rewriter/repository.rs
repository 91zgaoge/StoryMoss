use super::models::EntityMention;
use crate::{db::DbPool, error::AppError};

pub struct EntityMentionRepository {
    pool: DbPool,
}

impl EntityMentionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, mention: &EntityMention) -> Result<EntityMention, AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        conn.execute(
            "INSERT INTO entity_mentions (id, story_id, scene_id, entity_id, entity_type, \
             start_pos, end_pos, mention_text, confidence, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            [
                &mention.id,
                &mention.story_id,
                &mention.scene_id,
                &mention.entity_id,
                &mention.entity_type,
                &mention.start_pos.to_string(),
                &mention.end_pos.to_string(),
                &mention.mention_text,
                &mention.confidence.to_string(),
                &mention.created_at,
                &mention.updated_at,
            ],
        )
        .map_err(AppError::from)?;
        Ok(mention.clone())
    }

    pub fn get_by_entity(&self, entity_id: &str) -> Result<Vec<EntityMention>, AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, scene_id, entity_id, entity_type, start_pos, end_pos, \
                 mention_text, confidence, created_at, updated_at
             FROM entity_mentions WHERE entity_id = ?1 ORDER BY confidence DESC",
            )
            .map_err(AppError::from)?;

        let mentions = stmt
            .query_map([entity_id], |row| {
                Ok(EntityMention {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    entity_id: row.get(3)?,
                    entity_type: row.get(4)?,
                    start_pos: row.get(5)?,
                    end_pos: row.get(6)?,
                    mention_text: row.get(7)?,
                    confidence: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .map_err(AppError::from)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;

        Ok(mentions)
    }

    pub fn get_by_scene(&self, scene_id: &str) -> Result<Vec<EntityMention>, AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, story_id, scene_id, entity_id, entity_type, start_pos, end_pos, \
                 mention_text, confidence, created_at, updated_at
             FROM entity_mentions WHERE scene_id = ?1 ORDER BY start_pos ASC",
            )
            .map_err(AppError::from)?;

        let mentions = stmt
            .query_map([scene_id], |row| {
                Ok(EntityMention {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    entity_id: row.get(3)?,
                    entity_type: row.get(4)?,
                    start_pos: row.get(5)?,
                    end_pos: row.get(6)?,
                    mention_text: row.get(7)?,
                    confidence: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .map_err(AppError::from)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;

        Ok(mentions)
    }

    pub fn delete_by_scene(&self, scene_id: &str) -> Result<usize, AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let count = conn
            .execute(
                "DELETE FROM entity_mentions WHERE scene_id = ?1",
                [scene_id],
            )
            .map_err(AppError::from)?;
        Ok(count)
    }

    pub fn delete_by_entity(&self, entity_id: &str) -> Result<usize, AppError> {
        let conn = self.pool.get().map_err(AppError::from)?;
        let count = conn
            .execute(
                "DELETE FROM entity_mentions WHERE entity_id = ?1",
                [entity_id],
            )
            .map_err(AppError::from)?;
        Ok(count)
    }
}
