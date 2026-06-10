#![allow(unused_imports)]
use chrono::Local;
use rusqlite::params;
use uuid::Uuid;

use super::{DbPool, SceneAnnotation};

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
