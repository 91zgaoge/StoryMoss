use super::*;

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
                let rules_json: String = row
                    .get::<_, Option<String>>(8)?
                    .unwrap_or_else(|| "[]".to_string());
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
