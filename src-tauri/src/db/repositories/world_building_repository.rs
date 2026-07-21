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
                let rules_json: String = row
                    .get::<_, Option<String>>(3)?
                    .unwrap_or_else(|| "[]".to_string());
                let rules: Vec<WorldRule> = serde_json::from_str(&rules_json).unwrap_or_default();

                let cultures_json: String = row
                    .get::<_, Option<String>>(5)?
                    .unwrap_or_else(|| "[]".to_string());
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
                let rules_json: String = row
                    .get::<_, Option<String>>(3)?
                    .unwrap_or_else(|| "[]".to_string());
                let rules: Vec<WorldRule> = serde_json::from_str(&rules_json).unwrap_or_default();

                let cultures_json: String = row
                    .get::<_, Option<String>>(5)?
                    .unwrap_or_else(|| "[]".to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

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

    /// 回归：cultures / rules 列为 NULL（旧数据 / StoryForge 迁移导入的行）
    /// 时，get_by_story / get_by_id 不得报 "Invalid column type Null at
    /// index: 5, name: cultures"（或 index: 3 rules），应兜底为空数组。
    #[test]
    fn test_get_tolerates_null_cultures_and_rules() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let story = story_repo.create(story_req("世界观测试")).unwrap();
        let repo = WorldBuildingRepository::new(pool.clone());
        let wb = repo.create(&story.id, "测试世界").unwrap();

        // 模拟旧数据：手动置 NULL（覆盖 V112 回填未触及的路径）
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE world_buildings SET cultures = NULL, rules = NULL WHERE id = ?1",
            [&wb.id],
        )
        .unwrap();
        drop(conn);

        // get_by_story 不得 panic/Err
        let fetched = repo.get_by_story(&story.id).unwrap().unwrap();
        assert_eq!(fetched.concept, "测试世界");
        assert!(fetched.cultures.is_empty(), "NULL cultures 应兜底为空数组");
        assert!(fetched.rules.is_empty(), "NULL rules 应兜底为空数组");

        // get_by_id 同理
        let by_id = repo.get_by_id(&wb.id).unwrap().unwrap();
        assert!(by_id.cultures.is_empty());
        assert!(by_id.rules.is_empty());
    }

    /// 正常路径：cultures / rules 为合法 JSON 数组时正确解析。
    #[test]
    fn test_get_parses_valid_cultures_and_rules() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let story = story_repo.create(story_req("世界观测试2")).unwrap();
        let repo = WorldBuildingRepository::new(pool.clone());
        let wb = repo.create(&story.id, "魔法世界").unwrap();

        // 写入合法 JSON 数组
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE world_buildings SET cultures = ?1, rules = ?2 WHERE id = ?3",
            params![
                r#"[{"name":"北方部落","description":"游牧文化","customs":["祭祀"],"values":["勇气"]}]"#,
                r#"[{"id":"r1","name":"魔法限制","description":"test","rule_type":"Magic","importance":5}]"#,
                &wb.id
            ],
        )
        .unwrap();
        drop(conn);

        let fetched = repo.get_by_story(&story.id).unwrap().unwrap();
        assert_eq!(fetched.cultures.len(), 1);
        assert_eq!(fetched.cultures[0].name, "北方部落");
        assert_eq!(fetched.rules.len(), 1);
        assert_eq!(fetched.rules[0].name, "魔法限制");
        assert_eq!(fetched.rules[0].importance, 5);
    }
}
