use super::*;

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
    ) -> Result<CharacterRelationship, rusqlite::Error> {
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

        Ok(CharacterRelationship {
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

    pub fn get_by_id(&self, id: &str) -> Result<Option<CharacterRelationship>, rusqlite::Error> {
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

            Ok(CharacterRelationship {
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
    ) -> Result<Vec<CharacterRelationship>, rusqlite::Error> {
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

                Ok(CharacterRelationship {
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
