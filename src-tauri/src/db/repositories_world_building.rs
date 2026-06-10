#![allow(unused_imports)]
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde_json;
use uuid::Uuid;

use super::{Culture, DbPool, WorldBuilding, WorldRule};

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
