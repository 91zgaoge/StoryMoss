#![allow(unused_imports)]
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::{DbPool, StorySummary};

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
