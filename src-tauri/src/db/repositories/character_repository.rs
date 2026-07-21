use super::*;

pub struct CharacterRepository {
    pool: DbPool,
}

impl CharacterRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_in_tx(
        &self,
        tx: &rusqlite::Transaction,
        req: CreateCharacterRequest,
    ) -> Result<Character, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let traits_json = "[]";

        let source = req.source.as_deref().unwrap_or("user_created");
        let is_auto_generated = req.is_auto_generated.unwrap_or(false) as i32;

        tx.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, \
             appearance, gender, age, dynamic_traits, cs_location, cs_power_level, \
             cs_physical_state, cs_mental_state, cs_key_items, cs_recent_events, \
             cs_updated_at_chapter, cs_json, source, is_auto_generated, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, \
             ?18, ?19, ?20, ?21, ?22)",
            params![
                &id,
                &req.story_id,
                &req.name,
                req.background,
                req.personality,
                req.goals,
                req.appearance,
                req.gender,
                req.age,
                traits_json,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                rusqlite::types::Null,
                source,
                is_auto_generated,
                now.to_rfc3339(),
                now.to_rfc3339()
            ],
        )?;

        Ok(Character {
            id,
            story_id: req.story_id,
            name: req.name,
            background: req.background,
            personality: req.personality,
            goals: req.goals,
            appearance: req.appearance,
            gender: req.gender,
            age: req.age,
            dynamic_traits: vec![],
            cs_location: None,
            cs_power_level: None,
            cs_physical_state: None,
            cs_mental_state: None,
            cs_key_items: None,
            cs_recent_events: None,
            cs_updated_at_chapter: None,
            cs_json: None,
            source: Some(source.to_string()),
            is_auto_generated: Some(is_auto_generated != 0),
            created_at: now,
            updated_at: now,
        })
    }

    pub fn create(&self, req: CreateCharacterRequest) -> Result<Character, rusqlite::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;
        let character = self.create_in_tx(&tx, req)?;
        tx.commit()?;
        Ok(character)
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<Character>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, background, personality, goals, appearance, gender, age, \
             dynamic_traits, cs_location, cs_power_level, cs_physical_state, cs_mental_state, \
             cs_key_items, cs_recent_events, cs_updated_at_chapter, cs_json, source, \
             is_auto_generated, created_at, updated_at FROM characters WHERE story_id = ?1",
        )?;

        let characters = stmt
            .query_map([story_id], |row| {
                // dynamic_traits 列无 NOT NULL/DEFAULT 约束，旧数据（列迁移
                // 前的行）可能为 NULL -> 读为 Option 兜底 "[]"，避免
                // "Invalid column type Null at index: 9" 致获取角色失败。
                let traits_json: String = row
                    .get::<_, Option<String>>(9)?
                    .unwrap_or_else(|| "[]".to_string());
                let dynamic_traits: Vec<DynamicTrait> =
                    serde_json::from_str(&traits_json).unwrap_or_default();
                let created_str: String = row.get(20)?;
                let updated_str: String = row.get(21)?;
                let is_auto_generated: Option<i32> = row.get(19).ok();

                Ok(Character {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    background: row.get(3)?,
                    personality: row.get(4)?,
                    goals: row.get(5)?,
                    appearance: row.get(6)?,
                    gender: row.get(7)?,
                    age: row.get(8)?,
                    dynamic_traits,
                    cs_location: row.get(10).ok(),
                    cs_power_level: row.get(11).ok(),
                    cs_physical_state: row.get(12).ok(),
                    cs_mental_state: row.get(13).ok(),
                    cs_key_items: row.get(14).ok(),
                    cs_recent_events: row.get(15).ok(),
                    cs_updated_at_chapter: row.get(16).ok(),
                    cs_json: row.get(17).ok(),
                    source: row.get(18).ok(),
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(characters)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Character>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, background, personality, goals, appearance, gender, age, \
             dynamic_traits, cs_location, cs_power_level, cs_physical_state, cs_mental_state, \
             cs_key_items, cs_recent_events, cs_updated_at_chapter, cs_json, source, \
             is_auto_generated, created_at, updated_at FROM characters WHERE id = ?1",
        )?;

        let character = stmt
            .query_row([id], |row| {
                // dynamic_traits 列无 NOT NULL/DEFAULT 约束，旧数据（列迁移
                // 前的行）可能为 NULL -> 读为 Option 兜底 "[]"，避免
                // "Invalid column type Null at index: 9" 致获取角色失败。
                let traits_json: String = row
                    .get::<_, Option<String>>(9)?
                    .unwrap_or_else(|| "[]".to_string());
                let dynamic_traits: Vec<DynamicTrait> =
                    serde_json::from_str(&traits_json).unwrap_or_default();
                let created_str: String = row.get(20)?;
                let updated_str: String = row.get(21)?;
                let is_auto_generated: Option<i32> = row.get(19).ok();

                Ok(Character {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    name: row.get(2)?,
                    background: row.get(3)?,
                    personality: row.get(4)?,
                    goals: row.get(5)?,
                    appearance: row.get(6)?,
                    gender: row.get(7)?,
                    age: row.get(8)?,
                    dynamic_traits,
                    cs_location: row.get(10).ok(),
                    cs_power_level: row.get(11).ok(),
                    cs_physical_state: row.get(12).ok(),
                    cs_mental_state: row.get(13).ok(),
                    cs_key_items: row.get(14).ok(),
                    cs_recent_events: row.get(15).ok(),
                    cs_updated_at_chapter: row.get(16).ok(),
                    cs_json: row.get(17).ok(),
                    source: row.get(18).ok(),
                    is_auto_generated: is_auto_generated.map(|v| v != 0),
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                    updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(character)
    }

    pub fn update(
        &self,
        id: &str,
        name: Option<String>,
        background: Option<String>,
        personality: Option<String>,
        goals: Option<String>,
        appearance: Option<String>,
        gender: Option<String>,
        age: Option<i32>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE characters SET name = COALESCE(?2, name), background = COALESCE(?3, \
             background),
             personality = COALESCE(?4, personality), goals = COALESCE(?5, goals), appearance = \
             COALESCE(?6, appearance),
             gender = COALESCE(?7, gender), age = COALESCE(?8, age), updated_at = ?9 WHERE id = ?1",
            params![
                id,
                name,
                background,
                personality,
                goals,
                appearance,
                gender,
                age,
                now
            ],
        )?;
        Ok(count)
    }

    pub fn update_character_state(
        &self,
        character_id: &str,
        state: &CharacterState,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE characters SET
                cs_location = COALESCE(?2, cs_location),
                cs_power_level = COALESCE(?3, cs_power_level),
                cs_physical_state = COALESCE(?4, cs_physical_state),
                cs_mental_state = COALESCE(?5, cs_mental_state),
                cs_key_items = COALESCE(?6, cs_key_items),
                cs_recent_events = COALESCE(?7, cs_recent_events),
                cs_updated_at_chapter = COALESCE(?8, cs_updated_at_chapter),
                updated_at = ?9
            WHERE id = ?1",
            params![
                character_id,
                state.location,
                state.power_level,
                state.physical_state,
                state.mental_state,
                state.key_items,
                state.recent_events,
                state.updated_at_chapter,
                now,
            ],
        )?;
        Ok(count)
    }

    pub fn get_character_state(
        &self,
        character_id: &str,
    ) -> Result<Option<CharacterState>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT cs_location, cs_power_level, cs_physical_state, cs_mental_state, \
             cs_key_items, cs_recent_events, cs_updated_at_chapter FROM characters WHERE id = ?1",
        )?;

        let state = stmt
            .query_row([character_id], |row| {
                Ok(CharacterState {
                    location: row.get(0).ok(),
                    power_level: row.get(1).ok(),
                    physical_state: row.get(2).ok(),
                    mental_state: row.get(3).ok(),
                    key_items: row.get(4).ok(),
                    recent_events: row.get(5).ok(),
                    updated_at_chapter: row.get(6).ok(),
                    arc_type: None,
                    state_transitions_json: None,
                })
            })
            .optional()?;

        Ok(state)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        // 在事务中执行删除操作
        let tx = conn.unchecked_transaction()?;

        // 验证角色是否存在
        let exists: bool = tx
            .query_row("SELECT 1 FROM characters WHERE id = ?1", [id], |_| Ok(true))
            .unwrap_or(false);

        if !exists {
            tx.rollback()?;
            return Ok(0);
        }
        let _ = tx.execute("DELETE FROM scene_characters WHERE character_id = ?1", [id]);
        let _ = tx.execute(
            "DELETE FROM scene_character_actions WHERE character_id = ?1",
            [id],
        );
        let _ = tx.execute(
            "DELETE FROM character_relationships WHERE source_character_id = ?1 OR \
             target_character_id = ?1",
            [id],
        );
        let _ = tx.execute("DELETE FROM character_states WHERE character_id = ?1", [id]);

        // 执行删除操作 - 外键约束会自动级联剩余关联数据
        let count = tx.execute("DELETE FROM characters WHERE id = ?1", [id])?;

        tx.commit()?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn req(story_id: &str, name: &str) -> CreateCharacterRequest {
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
        }
    }

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

    /// 回归：dynamic_traits 列为 NULL（旧数据 / StoryForge 迁移导入的行）
    /// 时，get_by_story / get_by_id 不得报 "Invalid column type Null at
    /// index: 9"，应兜底为空数组。
    #[test]
    fn test_get_by_story_tolerates_null_dynamic_traits() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let story = story_repo.create(story_req("角色测试")).unwrap();
        let repo = CharacterRepository::new(pool.clone());
        let ch = repo.create(req(&story.id, "李明")).unwrap();

        // 模拟旧数据：手动置 NULL（覆盖 V111 回填未触及的路径）
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE characters SET dynamic_traits = NULL WHERE id = ?1",
            [&ch.id],
        )
        .unwrap();
        drop(conn);

        // get_by_story 不得 panic/Err
        let fetched = repo.get_by_story(&story.id).unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].name, "李明");
        assert!(
            fetched[0].dynamic_traits.is_empty(),
            "NULL dynamic_traits 应兜底为空数组"
        );

        // get_by_id 同理
        let by_id = repo.get_by_id(&ch.id).unwrap().unwrap();
        assert!(by_id.dynamic_traits.is_empty());
    }

    /// 正常路径：dynamic_traits 为合法 JSON 数组时正确解析。
    #[test]
    fn test_get_by_story_parses_dynamic_traits_json() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let story = story_repo.create(story_req("角色测试2")).unwrap();
        let repo = CharacterRepository::new(pool.clone());
        let ch = repo.create(req(&story.id, "韩雪")).unwrap();

        // 写入合法 JSON 数组（模拟有动态特征的角色）
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE characters SET dynamic_traits = ?1 WHERE id = ?2",
            params![r#"[{"trait":"坚定","confidence":0.9}]"#, &ch.id],
        )
        .unwrap();
        drop(conn);

        let fetched = repo.get_by_story(&story.id).unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].dynamic_traits.len(), 1);
        assert_eq!(fetched[0].dynamic_traits[0].trait_name, "坚定");
    }
}
