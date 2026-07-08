use super::*;

// ==================== KnowledgeGraph Repository ====================

pub struct KnowledgeGraphRepository {
    pool: DbPool,
}

impl KnowledgeGraphRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_entity_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        name: &str,
        entity_type: &str,
        attributes: &serde_json::Value,
        embedding: Option<Vec<f32>>,
    ) -> Result<Entity, rusqlite::Error> {
        self.create_entity_in_tx_with_source(
            tx,
            story_id,
            name,
            entity_type,
            attributes,
            embedding,
            None,
            None,
        )
    }

    pub fn create_entity_in_tx_with_source(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        name: &str,
        entity_type: &str,
        attributes: &serde_json::Value,
        embedding: Option<Vec<f32>>,
        source: Option<&str>,
        is_auto_generated: Option<bool>,
    ) -> Result<Entity, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let embedding_blob = embedding.as_ref().map(|vec| {
            vec.iter()
                .flat_map(|&f| f.to_le_bytes().to_vec())
                .collect::<Vec<u8>>()
        });
        let source = source.unwrap_or("user_created");
        let is_auto_generated = is_auto_generated.unwrap_or(false) as i32;

        tx.execute(
            "INSERT INTO kg_entities (id, story_id, name, entity_type, attributes, embedding, \
             first_seen, last_updated, is_archived, source, is_auto_generated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9, ?10)",
            params![
                &id,
                story_id,
                name,
                entity_type,
                attributes.to_string(),
                embedding_blob,
                now.to_rfc3339(),
                now.to_rfc3339(),
                source,
                is_auto_generated,
            ],
        )?;

        Ok(Entity {
            id,
            story_id: story_id.to_string(),
            name: name.to_string(),
            entity_type: entity_type.parse().map_err(|_| {
                rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
            })?,
            attributes: attributes.clone(),
            embedding,
            first_seen: now,
            last_updated: now,
            confidence_score: None,
            access_count: 0,
            last_accessed: None,
            is_archived: false,
            archived_at: None,
            source: Some(source.to_string()),
            is_auto_generated: Some(is_auto_generated != 0),
        })
    }

    pub fn create_entity(
        &self,
        story_id: &str,
        name: &str,
        entity_type: &str,
        attributes: &serde_json::Value,
        embedding: Option<Vec<f32>>,
    ) -> Result<Entity, rusqlite::Error> {
        self.create_entity_with_source(
            story_id,
            name,
            entity_type,
            attributes,
            embedding,
            None,
            None,
        )
    }

    pub fn create_entity_with_source(
        &self,
        story_id: &str,
        name: &str,
        entity_type: &str,
        attributes: &serde_json::Value,
        embedding: Option<Vec<f32>>,
        source: Option<&str>,
        is_auto_generated: Option<bool>,
    ) -> Result<Entity, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let entity = self.create_entity_in_tx_with_source(
            &tx,
            story_id,
            name,
            entity_type,
            attributes,
            embedding,
            source,
            is_auto_generated,
        )?;
        tx.commit()?;
        Ok(entity)
    }

    pub fn get_entities_by_story(&self, story_id: &str) -> Result<Vec<Entity>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, \
             last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at,
                    source, is_auto_generated
             FROM kg_entities WHERE story_id = ?1 AND is_archived = 0",
        )?;

        let entities = stmt
            .query_map([story_id], |row| {
                let type_str: String = row.get(3)?;
                let entity_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
                })?;

                let attrs_json: String = row.get(4)?;
                let attributes: serde_json::Value =
                    serde_json::from_str(&attrs_json).unwrap_or_default();

                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
                        .collect()
                });

                let first_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;
                let last_accessed: Option<String> = row.get(10)?;
                let is_archived: i32 = row.get(11)?;
                let archived_at: Option<String> = row.get(12)?;
                let source: Option<String> = row.get(13).ok();
                let is_auto_generated: Option<i32> = row.get(14).ok();

                Ok(Entity {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    entity_type,
                    attributes,
                    embedding,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    last_updated: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: last_accessed.and_then(|s| s.parse().ok()),
                    is_archived: is_archived != 0,
                    archived_at: archived_at.and_then(|s| s.parse().ok()),
                    source,
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entities)
    }

    pub fn get_archived_entities(&self, story_id: &str) -> Result<Vec<Entity>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, \
             last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at,
                    source, is_auto_generated
             FROM kg_entities WHERE story_id = ?1 AND is_archived = 1",
        )?;

        let entities = stmt
            .query_map([story_id], |row| {
                let type_str: String = row.get(3)?;
                let entity_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
                })?;

                let attrs_json: String = row.get(4)?;
                let attributes: serde_json::Value =
                    serde_json::from_str(&attrs_json).unwrap_or_default();

                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
                        .collect()
                });

                let first_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;
                let last_accessed: Option<String> = row.get(10)?;
                let is_archived: i32 = row.get(11)?;
                let archived_at: Option<String> = row.get(12)?;
                let source: Option<String> = row.get(13).ok();
                let is_auto_generated: Option<i32> = row.get(14).ok();

                Ok(Entity {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    entity_type,
                    attributes,
                    embedding,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    last_updated: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: last_accessed.and_then(|s| s.parse().ok()),
                    is_archived: is_archived != 0,
                    archived_at: archived_at.and_then(|s| s.parse().ok()),
                    source,
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entities)
    }

    pub fn archive_entity(&self, entity_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE kg_entities SET is_archived = 1, archived_at = ?2, last_updated = ?2 WHERE id \
             = ?1",
            params![entity_id, now],
        )
    }

    pub fn restore_entity(&self, entity_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE kg_entities SET is_archived = 0, archived_at = NULL, last_updated = ?2 WHERE \
             id = ?1",
            params![entity_id, now],
        )
    }

    pub fn delete_relation(&self, relation_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM kg_relations WHERE id = ?1",
            params![relation_id],
        )
    }

    pub fn create_relation_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        story_id: &str,
        source_id: &str,
        target_id: &str,
        relation_type: &str,
        strength: f32,
    ) -> Result<Relation, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        tx.execute(
            "INSERT INTO kg_relations (id, story_id, source_id, target_id, relation_type, \
             strength, evidence, first_seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &id,
                story_id,
                source_id,
                target_id,
                relation_type,
                strength,
                "[]",
                now.to_rfc3339()
            ],
        )?;

        Ok(Relation {
            id,
            story_id: story_id.to_string(),
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            relation_type: relation_type.parse().map_err(|_| {
                rusqlite::Error::InvalidParameterName("Invalid relation type".to_string())
            })?,
            strength,
            evidence: vec![],
            first_seen: now,
            confidence_score: None,
        })
    }

    pub fn create_relation(
        &self,
        story_id: &str,
        source_id: &str,
        target_id: &str,
        relation_type: &str,
        strength: f32,
    ) -> Result<Relation, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let relation = self.create_relation_in_tx(
            &tx,
            story_id,
            source_id,
            target_id,
            relation_type,
            strength,
        )?;
        tx.commit()?;
        Ok(relation)
    }

    /// 批量保存 Ingest 生成的实体（已包含完整字段，直接 INSERT）
    pub fn save_entities_batch(&self, entities: &[Entity]) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let mut count = 0;
        for entity in entities {
            let embedding_blob = entity.embedding.as_ref().map(|vec| {
                vec.iter()
                    .flat_map(|&f| f.to_le_bytes().to_vec())
                    .collect::<Vec<u8>>()
            });
            let source = entity.source.as_deref().unwrap_or("user_created");
            let is_auto_generated = entity.is_auto_generated.unwrap_or(false) as i32;
            tx.execute(
                "INSERT INTO kg_entities (id, story_id, name, entity_type, attributes, embedding, \
                 first_seen, last_updated, confidence_score, access_count, last_accessed, \
                 is_archived, archived_at, source, is_auto_generated)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                 ON CONFLICT(id) DO UPDATE SET
                     name=excluded.name,
                     attributes=excluded.attributes,
                     embedding=excluded.embedding,
                     last_updated=excluded.last_updated,
                     confidence_score=excluded.confidence_score,
                     source=excluded.source,
                     is_auto_generated=excluded.is_auto_generated",
                params![
                    &entity.id,
                    &entity.story_id,
                    &entity.name,
                    entity.entity_type.to_string(),
                    entity.attributes.to_string(),
                    embedding_blob,
                    entity.first_seen.to_rfc3339(),
                    entity.last_updated.to_rfc3339(),
                    entity.confidence_score,
                    entity.access_count,
                    entity.last_accessed.map(|d| d.to_rfc3339()),
                    entity.is_archived as i32,
                    entity.archived_at.map(|d| d.to_rfc3339()),
                    source,
                    is_auto_generated,
                ],
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    /// 批量保存 Ingest 生成的关系（已包含完整字段，直接 INSERT）
    pub fn save_relations_batch(&self, relations: &[Relation]) -> Result<usize, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let mut count = 0;
        for relation in relations {
            let evidence_json =
                serde_json::to_string(&relation.evidence).unwrap_or_else(|_| "[]".to_string());
            tx.execute(
                "INSERT INTO kg_relations (id, story_id, source_id, target_id, relation_type, \
                 strength, evidence, first_seen, confidence_score)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                     strength=excluded.strength,
                     evidence=excluded.evidence,
                     confidence_score=excluded.confidence_score",
                params![
                    &relation.id,
                    &relation.story_id,
                    &relation.source_id,
                    &relation.target_id,
                    relation.relation_type.to_string(),
                    relation.strength,
                    evidence_json,
                    relation.first_seen.to_rfc3339(),
                    relation.confidence_score
                ],
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn get_relations_by_entity(
        &self,
        entity_id: &str,
    ) -> Result<Vec<Relation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, source_id, target_id, relation_type, strength, evidence, \
             first_seen, confidence_score
             FROM kg_relations WHERE source_id = ?1 OR target_id = ?1",
        )?;

        let relations = stmt
            .query_map([entity_id], |row| {
                let type_str: String = row.get(4)?;
                let relation_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid relation type".to_string())
                })?;

                let evidence_json: String = row.get(6)?;
                let evidence: Vec<String> =
                    serde_json::from_str(&evidence_json).unwrap_or_default();

                let first_str: String = row.get(7)?;

                Ok(Relation {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    source_id: row.get(2)?,
                    target_id: row.get(3)?,
                    relation_type,
                    strength: row.get(5)?,
                    evidence,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(relations)
    }

    pub fn get_relations_by_story(&self, story_id: &str) -> Result<Vec<Relation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, source_id, target_id, relation_type, strength, evidence, \
             first_seen, confidence_score
             FROM kg_relations WHERE story_id = ?1",
        )?;

        let relations = stmt
            .query_map([story_id], |row| {
                let type_str: String = row.get(4)?;
                let relation_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid relation type".to_string())
                })?;

                let evidence_json: String = row.get(6)?;
                let evidence: Vec<String> =
                    serde_json::from_str(&evidence_json).unwrap_or_default();

                let first_str: String = row.get(7)?;

                Ok(Relation {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    source_id: row.get(2)?,
                    target_id: row.get(3)?,
                    relation_type,
                    strength: row.get(5)?,
                    evidence,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(relations)
    }

    pub fn get_entity_by_id(&self, entity_id: &str) -> Result<Option<Entity>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, \
             last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at,
                    source, is_auto_generated
             FROM kg_entities WHERE id = ?1",
        )?;

        let entity = stmt
            .query_row([entity_id], |row| {
                let type_str: String = row.get(3)?;
                let entity_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
                })?;
                let attrs_json: String = row.get(4)?;
                let attributes: serde_json::Value =
                    serde_json::from_str(&attrs_json).unwrap_or_default();
                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
                        .collect()
                });

                let first_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;
                let last_accessed: Option<String> = row.get(10)?;
                let is_archived: i32 = row.get(11)?;
                let archived_at: Option<String> = row.get(12)?;
                let source: Option<String> = row.get(13).ok();
                let is_auto_generated: Option<i32> = row.get(14).ok();

                Ok(Entity {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    entity_type,
                    attributes,
                    embedding,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    last_updated: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: last_accessed.and_then(|s| s.parse().ok()),
                    is_archived: is_archived != 0,
                    archived_at: archived_at.and_then(|s| s.parse().ok()),
                    source,
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                })
            })
            .optional()?;

        Ok(entity)
    }

    pub fn update_entity(
        &self,
        entity_id: &str,
        name: Option<&str>,
        attributes: Option<&serde_json::Value>,
        embedding: Option<Vec<f32>>,
    ) -> Result<Entity, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let entity = self
            .get_entity_by_id(entity_id)?
            .ok_or_else(|| rusqlite::Error::InvalidParameterName("Entity not found".to_string()))?;

        let new_name = name.unwrap_or(&entity.name);
        let new_attributes = attributes.unwrap_or(&entity.attributes);
        let embedding_blob = embedding.as_ref().map(|vec| {
            vec.iter()
                .flat_map(|&f| f.to_le_bytes().to_vec())
                .collect::<Vec<u8>>()
        });

        conn.execute(
            "UPDATE kg_entities SET name = ?2, attributes = ?3, embedding = ?4, last_updated = ?5 \
             WHERE id = ?1",
            params![
                entity_id,
                new_name,
                new_attributes.to_string(),
                embedding_blob,
                now
            ],
        )?;

        Ok(Entity {
            id: entity.id,
            story_id: entity.story_id,
            name: new_name.to_string(),
            entity_type: entity.entity_type,
            attributes: new_attributes.clone(),
            embedding,
            first_seen: entity.first_seen,
            last_updated: Local::now(),
            confidence_score: entity.confidence_score,
            access_count: entity.access_count,
            last_accessed: entity.last_accessed,
            is_archived: entity.is_archived,
            archived_at: entity.archived_at,
            source: entity.source,
            is_auto_generated: entity.is_auto_generated,
        })
    }

    /// 根据名称查找实体（用于 QueryPipeline 图谱扩展）
    pub fn find_entity_by_name(&self, name: &str) -> Result<Option<Entity>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, entity_type, attributes, embedding, first_seen, \
             last_updated,
                    confidence_score, access_count, last_accessed, is_archived, archived_at,
                    source, is_auto_generated
             FROM kg_entities WHERE name = ?1 AND is_archived = 0 LIMIT 1",
        )?;

        let entity = stmt
            .query_row([name], |row| {
                let type_str: String = row.get(3)?;
                let entity_type = type_str.parse().map_err(|_| {
                    rusqlite::Error::InvalidParameterName("Invalid entity type".to_string())
                })?;
                let attrs_json: String = row.get(4)?;
                let attributes: serde_json::Value =
                    serde_json::from_str(&attrs_json).unwrap_or_default();
                let embedding_blob: Option<Vec<u8>> = row.get(5)?;
                let embedding = embedding_blob.map(|bytes| {
                    bytes
                        .chunks_exact(4)
                        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
                        .collect()
                });
                let first_str: String = row.get(6)?;
                let updated_str: String = row.get(7)?;
                let last_accessed: Option<String> = row.get(10)?;
                let is_archived: i32 = row.get(11)?;
                let archived_at: Option<String> = row.get(12)?;
                let source: Option<String> = row.get(13).ok();
                let is_auto_generated: Option<i32> = row.get(14).ok();

                Ok(Entity {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    entity_type,
                    attributes,
                    embedding,
                    first_seen: first_str.parse().unwrap_or_else(|_| Local::now()),
                    last_updated: updated_str.parse().unwrap_or_else(|_| Local::now()),
                    confidence_score: row.get(8)?,
                    access_count: row.get(9)?,
                    last_accessed: last_accessed.and_then(|s| s.parse().ok()),
                    is_archived: is_archived != 0,
                    archived_at: archived_at.and_then(|s| s.parse().ok()),
                    source,
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                })
            })
            .optional()?;

        Ok(entity)
    }

    /// 获取与指定实体相关的实体及其关系强度
    pub fn get_related_entities(
        &self,
        entity_id: &str,
        min_strength: f32,
    ) -> Result<Vec<(Entity, f32)>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, strength FROM kg_relations 
             WHERE (source_id = ?1 OR target_id = ?1) AND strength >= ?2",
        )?;

        let rows = stmt
            .query_map(params![entity_id, min_strength], |row| {
                let source_id: String = row.get(0)?;
                let target_id: String = row.get(1)?;
                let strength: f32 = row.get(2)?;
                let other_id = if source_id == entity_id {
                    target_id
                } else {
                    source_id
                };
                Ok((other_id, strength))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut results = Vec::new();
        for (other_id, strength) in rows {
            if let Ok(Some(entity)) = self.get_entity_by_id(&other_id) {
                results.push((entity, strength));
            }
        }

        Ok(results)
    }
}

// 为 KnowledgeGraphRepository 实现 memory::query::KnowledgeGraph trait
#[async_trait::async_trait]
impl crate::memory::query::KnowledgeGraph for KnowledgeGraphRepository {
    async fn find_entity_by_name(
        &self,
        name: &str,
    ) -> Result<crate::db::models::Entity, Box<dyn std::error::Error + Send + Sync>> {
        self.find_entity_by_name(name)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
            .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                "Entity not found".into()
            })
    }

    async fn get_related_entities(
        &self,
        entity_id: &str,
        min_strength: f32,
    ) -> Result<Vec<(crate::db::models::Entity, f32)>, Box<dyn std::error::Error + Send + Sync>>
    {
        self.get_related_entities(entity_id, min_strength)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{connection::create_test_pool, CreateStoryRequest, StoryRepository};

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

    #[test]
    fn test_delete_relation_removes_row() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let kg_repo = KnowledgeGraphRepository::new(pool);

        let story = story_repo.create(story_req("关系删除测试")).unwrap();
        let source = kg_repo
            .create_entity(
                &story.id,
                "源实体",
                "Character",
                &serde_json::json!({}),
                None,
            )
            .unwrap();
        let target = kg_repo
            .create_entity(
                &story.id,
                "目标实体",
                "Character",
                &serde_json::json!({}),
                None,
            )
            .unwrap();
        let relation = kg_repo
            .create_relation(&story.id, &source.id, &target.id, "Friend", 0.8)
            .unwrap();

        let before = kg_repo.get_relations_by_story(&story.id).unwrap();
        assert_eq!(before.len(), 1);

        let deleted = kg_repo.delete_relation(&relation.id).unwrap();
        assert_eq!(deleted, 1);

        let after = kg_repo.get_relations_by_story(&story.id).unwrap();
        assert!(after.is_empty());
    }

    #[test]
    fn test_delete_relation_non_existent_returns_zero() {
        let pool = create_test_pool().unwrap();
        let kg_repo = KnowledgeGraphRepository::new(pool);

        let count = kg_repo.delete_relation("non-existent-id").unwrap();
        assert_eq!(count, 0);
    }
}
