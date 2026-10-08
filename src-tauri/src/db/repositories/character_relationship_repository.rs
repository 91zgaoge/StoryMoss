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
        emotional_bond: Option<&str>,
        emotional_intensity: Option<f32>,
        reverse_emotional_bond: Option<&str>,
        reverse_emotional_intensity: Option<f32>,
    ) -> Result<CharacterRelationship, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        // v0.64.11：写入归一列（受控 kind + 标志位），自由标签原样保留
        let class =
            crate::db::relation_kind::RelationClass::classify(relationship_type, emotional_bond);
        conn.execute(
            "INSERT INTO character_relationships (id, story_id, source_character_id, \
             target_character_id, relationship_type, description, dynamic, \
             emotional_bond, emotional_intensity, reverse_emotional_bond, \
             reverse_emotional_intensity, created_at, relation_kind, relation_flags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                &id,
                story_id,
                source_character_id,
                target_character_id,
                relationship_type,
                description,
                dynamic,
                emotional_bond,
                emotional_intensity,
                reverse_emotional_bond,
                reverse_emotional_intensity,
                now.to_rfc3339(),
                class.kind,
                class.flags()
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
            emotional_bond: emotional_bond.map(|s| s.to_string()),
            emotional_intensity,
            reverse_emotional_bond: reverse_emotional_bond.map(|s| s.to_string()),
            reverse_emotional_intensity,
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
                    r.relationship_type, r.description, r.dynamic, r.created_at,
                    r.emotional_bond, r.emotional_intensity, r.reverse_emotional_bond,
                    r.reverse_emotional_intensity
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
                emotional_bond: row.get(9).ok(),
                emotional_intensity: row.get(10).ok(),
                reverse_emotional_bond: row.get(11).ok(),
                reverse_emotional_intensity: row.get(12).ok(),
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
                    r.relationship_type, r.description, r.dynamic, r.created_at,
                    r.emotional_bond, r.emotional_intensity, r.reverse_emotional_bond,
                    r.reverse_emotional_intensity
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
                    emotional_bond: row.get(9).ok(),
                    emotional_intensity: row.get(10).ok(),
                    reverse_emotional_bond: row.get(11).ok(),
                    reverse_emotional_intensity: row.get(12).ok(),
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
        emotional_bond: Option<&str>,
        emotional_intensity: Option<f32>,
        reverse_emotional_bond: Option<&str>,
        reverse_emotional_intensity: Option<f32>,
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
            // v0.64.11：改了自由标签就同步重算归一列（供确定性消费者使用）
            let class = crate::db::relation_kind::RelationClass::classify(rt, emotional_bond);
            let flags = class.flags();
            updates.push("relation_kind = ?");
            params.push(Box::new(class.kind));
            updates.push("relation_flags = ?");
            params.push(Box::new(flags));
        }
        if let Some(desc) = description {
            updates.push("description = ?");
            params.push(Box::new(desc.to_string()));
        }
        if let Some(dyn_val) = dynamic {
            updates.push("dynamic = ?");
            params.push(Box::new(dyn_val.to_string()));
        }
        if let Some(bond) = emotional_bond {
            updates.push("emotional_bond = ?");
            params.push(Box::new(bond.to_string()));
        }
        if let Some(intensity) = emotional_intensity {
            updates.push("emotional_intensity = ?");
            params.push(Box::new(intensity));
        }
        if let Some(rev_bond) = reverse_emotional_bond {
            updates.push("reverse_emotional_bond = ?");
            params.push(Box::new(rev_bond.to_string()));
        }
        if let Some(rev_intensity) = reverse_emotional_intensity {
            updates.push("reverse_emotional_intensity = ?");
            params.push(Box::new(rev_intensity));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        connection::create_test_pool,
        dto::{CreateCharacterRequest, CreateStoryRequest},
    };

    fn story_req(title: &str) -> CreateStoryRequest {
        CreateStoryRequest {
            title: title.to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        }
    }

    fn char_req(story_id: &str, name: &str) -> CreateCharacterRequest {
        CreateCharacterRequest {
            story_id: story_id.to_string(),
            name: name.to_string(),
            background: None,
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,
            source: None,
            is_auto_generated: None,
            emotional_core: None,
            emotional_trigger: None,
            emotional_wound: None,
            emotional_need: None,
        }
    }

    #[test]
    fn test_create_relationship_with_emotional_bond() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let story = story_repo.create(story_req("关系情感测试")).unwrap();

        let char_repo = CharacterRepository::new(pool.clone());
        let ch_a = char_repo.create(char_req(&story.id, "甲")).unwrap();
        let ch_b = char_repo.create(char_req(&story.id, "乙")).unwrap();

        let repo = CharacterRelationshipRepository::new(pool.clone());
        let rel = repo
            .create(
                &story.id,
                &ch_a.id,
                &ch_b.id,
                "师徒",
                Some("甲是乙的师父"),
                Some("表面师徒，实则利用"),
                Some("恨"),   // emotional_bond
                Some(0.9),    // emotional_intensity
                Some("恐惧"), // reverse_emotional_bond
                Some(0.7),    // reverse_emotional_intensity
            )
            .unwrap();

        // get_by_id 回读
        let by_id = repo.get_by_id(&rel.id).unwrap().unwrap();
        assert_eq!(by_id.emotional_bond.as_deref(), Some("恨"));
        assert_eq!(by_id.emotional_intensity, Some(0.9));
        assert_eq!(by_id.reverse_emotional_bond.as_deref(), Some("恐惧"));
        assert_eq!(by_id.reverse_emotional_intensity, Some(0.7));

        // get_by_story 回读
        let by_story = repo.get_by_story(&story.id).unwrap();
        assert_eq!(by_story.len(), 1);
        assert_eq!(by_story[0].emotional_bond.as_deref(), Some("恨"));
        assert_eq!(by_story[0].reverse_emotional_bond.as_deref(), Some("恐惧"));
    }

    #[test]
    fn test_update_relationship_emotional_bond() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let story = story_repo.create(story_req("关系更新测试")).unwrap();

        let char_repo = CharacterRepository::new(pool.clone());
        let ch_a = char_repo.create(char_req(&story.id, "丙")).unwrap();
        let ch_b = char_repo.create(char_req(&story.id, "丁")).unwrap();

        let repo = CharacterRelationshipRepository::new(pool.clone());

        // 创建时不带情感维度
        let rel = repo
            .create(
                &story.id,
                &ch_a.id,
                &ch_b.id,
                "恋人",
                Some("青梅竹马"),
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        assert!(rel.emotional_bond.is_none());

        // 更新情感维度
        repo.update(
            &rel.id,
            None,
            None,
            None,
            Some("爱"),
            Some(0.95),
            Some("执念"),
            Some(0.6),
        )
        .unwrap();

        let by_id = repo.get_by_id(&rel.id).unwrap().unwrap();
        assert_eq!(by_id.emotional_bond.as_deref(), Some("爱"));
        assert_eq!(by_id.emotional_intensity, Some(0.95));
        assert_eq!(by_id.reverse_emotional_bond.as_deref(), Some("执念"));
        assert_eq!(by_id.reverse_emotional_intensity, Some(0.6));
        // 原有关系类型不变
        assert_eq!(by_id.relationship_type, "恋人");
    }
}
