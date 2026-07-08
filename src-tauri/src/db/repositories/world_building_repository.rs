use super::*;

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
        self.create_in_tx_with_source(tx, story_id, concept, None, None)
    }

    pub fn create_in_tx_with_source(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        concept: &str,
        source: Option<&str>,
        is_auto_generated: Option<bool>,
    ) -> Result<WorldBuilding, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let source = source.unwrap_or("user_created");
        let is_auto_generated = is_auto_generated.unwrap_or(false) as i32;

        tx.execute(
            "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, source, \
             is_auto_generated, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &id,
                story_id,
                concept,
                "[]",
                "",
                "[]",
                source,
                is_auto_generated,
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
            source: Some(source.to_string()),
            is_auto_generated: Some(is_auto_generated != 0),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn create(&self, story_id: &str, concept: &str) -> Result<WorldBuilding, rusqlite::Error> {
        self.create_with_source(story_id, concept, None, None)
    }

    pub fn create_with_source(
        &self,
        story_id: &str,
        concept: &str,
        source: Option<&str>,
        is_auto_generated: Option<bool>,
    ) -> Result<WorldBuilding, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let wb =
            self.create_in_tx_with_source(&tx, story_id, concept, source, is_auto_generated)?;
        tx.commit()?;
        Ok(wb)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<WorldBuilding>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, concept, rules, history, cultures, source, is_auto_generated, \
             created_at, updated_at
             FROM world_buildings WHERE id = ?1",
        )?;

        let wb = stmt
            .query_row([id], |row| {
                let rules_json: String = row.get(3)?;
                let rules: Vec<WorldRule> = serde_json::from_str(&rules_json).unwrap_or_default();

                let cultures_json: String = row.get(5)?;
                let cultures: Vec<Culture> =
                    serde_json::from_str(&cultures_json).unwrap_or_default();

                let created_str: String = row.get(8)?;
                let updated_str: String = row.get(9)?;
                let is_auto_generated: Option<i32> = row.get(7).ok();

                Ok(WorldBuilding {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    concept: row.get(2)?,
                    rules,
                    history: row.get(4)?,
                    cultures,
                    source: row.get(6).ok(),
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
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
            "SELECT id, story_id, concept, rules, history, cultures, source, is_auto_generated, \
             created_at, updated_at
             FROM world_buildings WHERE story_id = ?1",
        )?;

        let wb = stmt
            .query_row([story_id], |row| {
                let rules_json: String = row.get(3)?;
                let rules: Vec<WorldRule> = serde_json::from_str(&rules_json).unwrap_or_default();

                let cultures_json: String = row.get(5)?;
                let cultures: Vec<Culture> =
                    serde_json::from_str(&cultures_json).unwrap_or_default();

                let created_str: String = row.get(8)?;
                let updated_str: String = row.get(9)?;
                let is_auto_generated: Option<i32> = row.get(7).ok();

                Ok(WorldBuilding {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    concept: row.get(2)?,
                    rules,
                    history: row.get(4)?,
                    cultures,
                    source: row.get(6).ok(),
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
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
