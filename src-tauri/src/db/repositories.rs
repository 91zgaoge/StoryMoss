#![allow(dead_code)]
use super::{DbPool, Story, Character, Chapter, CreateStoryRequest, CreateCharacterRequest, CreateChapterRequest, DynamicTrait, CharacterState};
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

pub struct StoryRepository {
    pool: DbPool,
}

impl StoryRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, req: CreateStoryRequest) -> Result<Story, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO stories (id, title, description, genre, tone, pacing, style_dna_id, methodology_id, methodology_step, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![&id, &req.title, req.description, req.genre, "dark", "medium", req.style_dna_id, None::<String>, None::<i32>, now.to_rfc3339(), now.to_rfc3339()],
        )?;
        
        Ok(Story {
            id,
            title: req.title,
            description: req.description,
            genre: req.genre,
            tone: Some("dark".to_string()),
            pacing: Some("medium".to_string()),
            style_dna_id: req.style_dna_id,
            methodology_id: None,
            methodology_step: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_all(&self) -> Result<Vec<Story>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, title, description, genre, tone, pacing, style_dna_id, methodology_id, methodology_step, created_at, updated_at FROM stories ORDER BY updated_at DESC"
        )?;
        
        let stories = stmt.query_map([], |row| {
            let created_str: String = row.get(9)?;
            let updated_str: String = row.get(10)?;
            Ok(Story {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                genre: row.get(3)?,
                tone: row.get(4)?,
                pacing: row.get(5)?,
                style_dna_id: row.get(6)?,
                methodology_id: row.get(7)?,
                methodology_step: row.get(8)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        
        Ok(stories)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Story>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, title, description, genre, tone, pacing, style_dna_id, methodology_id, methodology_step, created_at, updated_at FROM stories WHERE id = ?1"
        )?;
        
        let story = stmt.query_row([id], |row| {
            let created_str: String = row.get(9)?;
            let updated_str: String = row.get(10)?;
            Ok(Story {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                genre: row.get(3)?,
                tone: row.get(4)?,
                pacing: row.get(5)?,
                style_dna_id: row.get(6)?,
                methodology_id: row.get(7)?,
                methodology_step: row.get(8)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;
        
        Ok(story)
    }

    pub fn update(&self, id: &str, req: &super::UpdateStoryRequest) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE stories SET title = COALESCE(?2, title), description = COALESCE(?3, description),
             genre = COALESCE(?4, genre), tone = COALESCE(?5, tone), pacing = COALESCE(?6, pacing),
             style_dna_id = COALESCE(?7, style_dna_id), methodology_id = COALESCE(?8, methodology_id),
             methodology_step = COALESCE(?9, methodology_step), updated_at = ?10 WHERE id = ?1",
            params![id, req.title, req.description, req.genre, req.tone, req.pacing, req.style_dna_id, req.methodology_id, req.methodology_step, now],
        )?;
        Ok(count)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        // 在事务中执行删除操作，确保级联删除正确执行
        let tx = conn.unchecked_transaction()?;

        // 验证故事是否存在
        let exists: bool = tx.query_row(
            "SELECT 1 FROM stories WHERE id = ?1",
            [id],
            |_| Ok(true)
        ).unwrap_or(false);

        if !exists {
            tx.rollback()?;
            return Ok(0);
        }

        // v5.6.4 修复: 显式清理无外键约束或可能存在遗留的关联表数据
        // 即使外键约束已启用，也作为防御性编程添加显式 DELETE
        let _ = tx.execute("DELETE FROM story_metadata WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM foreshadowing_tracker WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM user_preferences WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM story_runtime_states WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM story_style_configs WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM story_outlines WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM studio_configs WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM story_summaries WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM narrative_characters WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM narrative_scenes WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM narrative_world_buildings WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM chat_sessions WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM text_annotations WHERE story_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM ai_operations WHERE story_id = ?1", [id]);

        // 执行删除操作 - 由于外键约束已启用，大部分相关数据会自动级联删除
        let count = tx.execute("DELETE FROM stories WHERE id = ?1", [id])?;

        tx.commit()?;

        // v5.7 不变量断言: 删除 story 后，所有关联表不应存在孤儿数据
        // 仅在 debug 构建时检查，用于在开发和测试阶段快速发现级联删除遗漏
        #[cfg(debug_assertions)]
        {
            let check_conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
            let orphan_tables = [
                ("chapters", "story_id"),
                ("characters", "story_id"),
                ("scenes", "story_id"),
                ("kg_entities", "story_id"),
                ("kg_relations", "story_id"),
                ("character_relationships", "story_id"),
                ("scene_annotations", "story_id"),
            ];
            for (table, col) in orphan_tables {
                let orphan_count: i64 = check_conn
                    .query_row(
                        &format!("SELECT COUNT(*) FROM {} WHERE {} = ?1", table, col),
                        [id],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                debug_assert_eq!(
                    orphan_count, 0,
                    "StoryRepository::delete orphan invariant violated: {} rows remain in {} after story {} deletion",
                    orphan_count, table, id
                );
            }
        }

        Ok(count)
    }
}

pub struct CharacterRepository {
    pool: DbPool,
}

impl CharacterRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, req: CreateCharacterRequest) -> Result<Character, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let traits_json = "[]";
        
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO characters (id, story_id, name, background, personality, goals, appearance, gender, age, dynamic_traits, cs_location, cs_power_level, cs_physical_state, cs_mental_state, cs_key_items, cs_recent_events, cs_updated_at_chapter, cs_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![&id, &req.story_id, &req.name, req.background, req.personality, req.goals, req.appearance, req.gender, req.age, traits_json, rusqlite::types::Null, rusqlite::types::Null, rusqlite::types::Null, rusqlite::types::Null, rusqlite::types::Null, rusqlite::types::Null, rusqlite::types::Null, rusqlite::types::Null, now.to_rfc3339(), now.to_rfc3339()],
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
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<Character>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, background, personality, goals, appearance, gender, age, dynamic_traits, cs_location, cs_power_level, cs_physical_state, cs_mental_state, cs_key_items, cs_recent_events, cs_updated_at_chapter, cs_json, created_at, updated_at FROM characters WHERE story_id = ?1"
        )?;

        let characters = stmt.query_map([story_id], |row| {
            let traits_json: String = row.get(9)?;
            let dynamic_traits: Vec<DynamicTrait> = serde_json::from_str(&traits_json).unwrap_or_default();
            let created_str: String = row.get(18)?;
            let updated_str: String = row.get(19)?;

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
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(characters)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Character>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, name, background, personality, goals, appearance, gender, age, dynamic_traits, cs_location, cs_power_level, cs_physical_state, cs_mental_state, cs_key_items, cs_recent_events, cs_updated_at_chapter, cs_json, created_at, updated_at FROM characters WHERE id = ?1"
        )?;

        let character = stmt.query_row([id], |row| {
            let traits_json: String = row.get(9)?;
            let dynamic_traits: Vec<DynamicTrait> = serde_json::from_str(&traits_json).unwrap_or_default();
            let created_str: String = row.get(18)?;
            let updated_str: String = row.get(19)?;

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
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;

        Ok(character)
    }

    pub fn update(&self, id: &str, name: Option<String>, background: Option<String>, personality: Option<String>, goals: Option<String>, appearance: Option<String>, gender: Option<String>, age: Option<i32>) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE characters SET name = COALESCE(?2, name), background = COALESCE(?3, background),
             personality = COALESCE(?4, personality), goals = COALESCE(?5, goals), appearance = COALESCE(?6, appearance),
             gender = COALESCE(?7, gender), age = COALESCE(?8, age), updated_at = ?9 WHERE id = ?1",
            params![id, name, background, personality, goals, appearance, gender, age, now],
        )?;
        Ok(count)
    }

    pub fn update_character_state(
        &self,
        character_id: &str,
        state: &CharacterState,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
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

    pub fn batch_update_states(
        &self,
        updates: &[(String, CharacterState)],
    ) -> Result<usize, rusqlite::Error> {
        let mut total = 0;
        for (character_id, state) in updates {
            total += self.update_character_state(character_id, state)?;
        }
        Ok(total)
    }

    pub fn get_character_state(
        &self,
        character_id: &str,
    ) -> Result<Option<CharacterState>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT cs_location, cs_power_level, cs_physical_state, cs_mental_state, cs_key_items, cs_recent_events, cs_updated_at_chapter FROM characters WHERE id = ?1"
        )?;

        let state = stmt.query_row([character_id], |row| {
            Ok(CharacterState {
                location: row.get(0).ok(),
                power_level: row.get(1).ok(),
                physical_state: row.get(2).ok(),
                mental_state: row.get(3).ok(),
                key_items: row.get(4).ok(),
                recent_events: row.get(5).ok(),
                updated_at_chapter: row.get(6).ok(),
            })
        }).optional()?;

        Ok(state)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        // 在事务中执行删除操作
        let tx = conn.unchecked_transaction()?;

        // 验证角色是否存在
        let exists: bool = tx.query_row(
            "SELECT 1 FROM characters WHERE id = ?1",
            [id],
            |_| Ok(true)
        ).unwrap_or(false);

        if !exists {
            tx.rollback()?;
            return Ok(0);
        }

        // v5.6.4 修复: 显式清理角色关联数据，消除幽灵数据
        let _ = tx.execute("DELETE FROM scene_characters WHERE character_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM scene_character_actions WHERE character_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM character_relationships WHERE source_character_id = ?1 OR target_character_id = ?1", [id]);
        let _ = tx.execute("DELETE FROM character_states WHERE character_id = ?1", [id]);

        // 执行删除操作 - 外键约束会自动级联剩余关联数据
        let count = tx.execute("DELETE FROM characters WHERE id = ?1", [id])?;

        tx.commit()?;
        Ok(count)
    }
}

pub struct ChapterRepository {
    pool: DbPool,
}

impl ChapterRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, req: CreateChapterRequest) -> Result<Chapter, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();
        let word_count = req.content.as_ref().map(|c| c.len() as i32);

        let mut conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;

        // 1. 插入 Chapter
        tx.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title, outline, content, word_count, model_used, cost, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &id, &req.story_id, req.chapter_number, req.title, req.outline, req.content,
                word_count, "", 0.0, now.to_rfc3339(), now.to_rfc3339()
            ],
        )?;

        // 2. 查找或创建关联的 Scene
        let scene_id = match tx.query_row(
            "SELECT id FROM scenes WHERE story_id = ?1 AND sequence_number = ?2",
            params![&req.story_id, req.chapter_number],
            |row| row.get::<_, String>(0)
        ).optional()? {
            Some(sid) => {
                // 关联已有 Scene
                tx.execute(
                    "UPDATE scenes SET chapter_id = ?1 WHERE id = ?2",
                    params![&id, &sid],
                )?;
                Some(sid)
            }
            None => {
                // 创建新 Scene
                let sid = Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO scenes (id, story_id, sequence_number, title, content, characters_present, character_conflicts, execution_stage, created_at, updated_at) 
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    params![
                        &sid, &req.story_id, req.chapter_number, req.title, req.content,
                        "[]", "[]", "drafting", now.to_rfc3339(), now.to_rfc3339()
                    ],
                )?;
                Some(sid)
            }
        };

        // 3. 更新 chapter 的 scene_id
        if let Some(ref sid) = scene_id {
            tx.execute(
                "UPDATE chapters SET scene_id = ?1 WHERE id = ?2",
                params![sid, &id],
            )?;
        }

        tx.commit()?;

        Ok(Chapter {
            id,
            story_id: req.story_id,
            chapter_number: req.chapter_number,
            title: req.title,
            outline: req.outline,
            content: req.content,
            word_count,
            model_used: None,
            cost: None,
            scene_id,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<Chapter>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, title, outline, content, word_count, model_used, cost, scene_id, created_at, updated_at FROM chapters WHERE story_id = ?1 ORDER BY chapter_number"
        )?;

        let chapters = stmt.query_map([story_id], |row| {
            let created_str: String = row.get(10)?;
            let updated_str: String = row.get(11)?;
            Ok(Chapter {
                id: row.get(0)?,
                story_id: row.get(1)?,
                chapter_number: row.get(2)?,
                title: row.get(3)?,
                outline: row.get(4)?,
                content: row.get(5)?,
                word_count: row.get(6)?,
                model_used: row.get(7)?,
                cost: row.get(8)?,
                scene_id: row.get::<_, Option<String>>(9)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(chapters)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<Chapter>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, chapter_number, title, outline, content, word_count, model_used, cost, scene_id, created_at, updated_at FROM chapters WHERE id = ?1"
        )?;

        let chapter = stmt.query_row([id], |row| {
            let created_str: String = row.get(10)?;
            let updated_str: String = row.get(11)?;
            Ok(Chapter {
                id: row.get(0)?,
                story_id: row.get(1)?,
                chapter_number: row.get(2)?,
                title: row.get(3)?,
                outline: row.get(4)?,
                content: row.get(5)?,
                word_count: row.get(6)?,
                model_used: row.get(7)?,
                cost: row.get(8)?,
                scene_id: row.get::<_, Option<String>>(9)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;

        Ok(chapter)
    }

    pub fn update(&self, id: &str, title: Option<String>, outline: Option<String>, content: Option<String>, word_count: Option<i32>) -> Result<usize, rusqlite::Error> {
        let mut conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let word_count = word_count.or_else(|| content.as_ref().map(|c| c.len() as i32));

        let tx = conn.transaction()?;

        let count = tx.execute(
            "UPDATE chapters SET title = COALESCE(?2, title), outline = COALESCE(?3, outline),
             content = COALESCE(?4, content), word_count = COALESCE(?5, word_count), updated_at = ?6 WHERE id = ?1",
            params![id, title, outline, content, word_count, now],
        )?;

        // 同步更新关联的 Scene
        if title.is_some() || content.is_some() {
            let scene_id: Option<String> = tx.query_row(
                "SELECT scene_id FROM chapters WHERE id = ?1",
                [id],
                |row| row.get(0)
            ).optional()?;
            if let Some(sid) = scene_id {
                tx.execute(
                    "UPDATE scenes SET title = COALESCE(?2, title), content = COALESCE(?3, content), updated_at = ?4 WHERE id = ?1",
                    params![sid, title, content, now],
                )?;
            }
        }

        tx.commit()?;
        Ok(count)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let mut conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let tx = conn.transaction()?;

        // 验证章节是否存在
        let exists: bool = tx.query_row(
            "SELECT 1 FROM chapters WHERE id = ?1",
            [id],
            |_| Ok(true)
        ).unwrap_or(false);

        if !exists {
            tx.rollback()?;
            return Ok(0);
        }

        // 解除与 scenes 的关联关系
        tx.execute("UPDATE scenes SET chapter_id = NULL WHERE chapter_id = ?1", [id])?;

        // 删除章节
        let count = tx.execute("DELETE FROM chapters WHERE id = ?1", [id])?;

        tx.commit()?;
        Ok(count)
    }
}


// ==================== UserRepository (v4.5.0) ====================

use super::{User, OAuthAccount, Session, UserInfo};

pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create_user(&self, email: Option<String>, display_name: Option<String>, avatar_url: Option<String>) -> Result<User, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO users (id, email, display_name, avatar_url, is_local_user, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            params![&id, email, display_name, avatar_url, now.to_rfc3339()],
        )?;

        Ok(User {
            id,
            email,
            display_name,
            avatar_url,
            is_local_user: false,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn find_by_id(&self, id: &str) -> Result<Option<User>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, email, display_name, avatar_url, is_local_user, created_at, updated_at FROM users WHERE id = ?1"
        )?;

        let user = stmt.query_row([id], |row| {
            let created_str: String = row.get(5)?;
            let updated_str: String = row.get(6)?;
            Ok(User {
                id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get(2)?,
                avatar_url: row.get(3)?,
                is_local_user: row.get::<_, i32>(4)? != 0,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;

        Ok(user)
    }

    pub fn find_by_email(&self, email: &str) -> Result<Option<User>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, email, display_name, avatar_url, is_local_user, created_at, updated_at FROM users WHERE email = ?1"
        )?;

        let user = stmt.query_row([email], |row| {
            let created_str: String = row.get(5)?;
            let updated_str: String = row.get(6)?;
            Ok(User {
                id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get(2)?,
                avatar_url: row.get(3)?,
                is_local_user: row.get::<_, i32>(4)? != 0,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;

        Ok(user)
    }

    pub fn find_by_oauth(&self, provider: &str, provider_account_id: &str) -> Result<Option<User>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT u.id, u.email, u.display_name, u.avatar_url, u.is_local_user, u.created_at, u.updated_at
             FROM users u
             JOIN oauth_accounts oa ON u.id = oa.user_id
             WHERE oa.provider = ?1 AND oa.provider_account_id = ?2"
        )?;

        let user = stmt.query_row([provider, provider_account_id], |row| {
            let created_str: String = row.get(5)?;
            let updated_str: String = row.get(6)?;
            Ok(User {
                id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get(2)?,
                avatar_url: row.get(3)?,
                is_local_user: row.get::<_, i32>(4)? != 0,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;

        Ok(user)
    }

    pub fn create_oauth_account(&self, user_id: &str, provider: &str, provider_account_id: &str, access_token: Option<String>, refresh_token: Option<String>, expires_at: Option<chrono::DateTime<Local>>) -> Result<OAuthAccount, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO oauth_accounts (id, user_id, provider, provider_account_id, access_token, refresh_token, expires_at, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![&id, user_id, provider, provider_account_id, access_token, refresh_token, expires_at.map(|d| d.to_rfc3339()), now.to_rfc3339()],
        )?;

        Ok(OAuthAccount {
            id,
            user_id: user_id.to_string(),
            provider: provider.to_string(),
            provider_account_id: provider_account_id.to_string(),
            access_token,
            refresh_token,
            expires_at,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn create_session(&self, user_id: &str, token: &str, expires_at: chrono::DateTime<Local>) -> Result<Session, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now();

        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO sessions (id, user_id, token, expires_at, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&id, user_id, token, expires_at.to_rfc3339(), now.to_rfc3339()],
        )?;

        Ok(Session {
            id,
            user_id: user_id.to_string(),
            token: token.to_string(),
            expires_at,
            created_at: now,
        })
    }

    pub fn find_session_by_token(&self, token: &str) -> Result<Option<Session>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, user_id, token, expires_at, created_at FROM sessions WHERE token = ?1"
        )?;

        let session = stmt.query_row([token], |row| {
            let expires_str: String = row.get(3)?;
            let created_str: String = row.get(4)?;
            Ok(Session {
                id: row.get(0)?,
                user_id: row.get(1)?,
                token: row.get(2)?,
                expires_at: expires_str.parse().unwrap_or_else(|_| Local::now()),
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;

        Ok(session)
    }

    pub fn delete_session(&self, token: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM sessions WHERE token = ?1", [token])?;
        Ok(count)
    }

    pub fn delete_user_sessions(&self, user_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let count = conn.execute("DELETE FROM sessions WHERE user_id = ?1", [user_id])?;
        Ok(count)
    }

    pub fn cleanup_expired_sessions(&self) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        let count = conn.execute("DELETE FROM sessions WHERE expires_at < ?1", [now])?;
        Ok(count)
    }

    pub fn to_user_info(&self, user: &User) -> UserInfo {
        UserInfo {
            id: user.id.clone(),
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            avatar_url: user.avatar_url.clone(),
        }
    }
}

// ==================== GenesisRun Repository (W2-B9) ====================

pub struct GenesisRunRepository {
    pool: DbPool,
}

impl GenesisRunRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        id: &str,
        session_id: &str,
        premise: &str,
        total_steps: i32,
    ) -> Result<super::GenesisRun, rusqlite::Error> {
        let now = Local::now();
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO genesis_runs (id, session_id, premise, status, total_steps, steps_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, session_id, premise, "pending", total_steps, "{}", now.to_rfc3339(), now.to_rfc3339()],
        )?;
        Ok(super::GenesisRun {
            id: id.to_string(),
            story_id: None,
            session_id: session_id.to_string(),
            premise: premise.to_string(),
            status: "pending".to_string(),
            current_step: None,
            current_step_number: 0,
            total_steps,
            steps_json: "{}".to_string(),
            error_message: None,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_step(
        &self,
        id: &str,
        step_name: &str,
        step_number: i32,
        status: &str,
        steps_json: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET current_step = ?2, current_step_number = ?3, status = ?4, steps_json = ?5, updated_at = ?6 WHERE id = ?1",
            params![id, step_name, step_number, status, steps_json, now],
        )
    }

    pub fn complete(
        &self,
        id: &str,
        story_id: Option<&str>,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET status = 'completed', story_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, story_id, now],
        )
    }

    pub fn fail(
        &self,
        id: &str,
        error_message: &str,
    ) -> Result<usize, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let now = Local::now().to_rfc3339();
        conn.execute(
            "UPDATE genesis_runs SET status = 'failed', error_message = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, error_message, now],
        )
    }

    pub fn get_by_id(
        &self,
        id: &str,
    ) -> Result<Option<super::GenesisRun>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, session_id, premise, status, current_step, current_step_number, total_steps, steps_json, error_message, created_at, updated_at FROM genesis_runs WHERE id = ?1"
        )?;
        let run = stmt.query_row([id], |row| {
            let created_str: String = row.get(10)?;
            let updated_str: String = row.get(11)?;
            Ok(super::GenesisRun {
                id: row.get(0)?,
                story_id: row.get(1)?,
                session_id: row.get(2)?,
                premise: row.get(3)?,
                status: row.get(4)?,
                current_step: row.get(5)?,
                current_step_number: row.get(6)?,
                total_steps: row.get(7)?,
                steps_json: row.get(8)?,
                error_message: row.get(9)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;
        Ok(run)
    }

    pub fn get_by_session(
        &self,
        session_id: &str,
    ) -> Result<Option<super::GenesisRun>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, session_id, premise, status, current_step, current_step_number, total_steps, steps_json, error_message, created_at, updated_at FROM genesis_runs WHERE session_id = ?1 ORDER BY created_at DESC LIMIT 1"
        )?;
        let run = stmt.query_row([session_id], |row| {
            let created_str: String = row.get(10)?;
            let updated_str: String = row.get(11)?;
            Ok(super::GenesisRun {
                id: row.get(0)?,
                story_id: row.get(1)?,
                session_id: row.get(2)?,
                premise: row.get(3)?,
                status: row.get(4)?,
                current_step: row.get(5)?,
                current_step_number: row.get(6)?,
                total_steps: row.get(7)?,
                steps_json: row.get(8)?,
                error_message: row.get(9)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        }).optional()?;
        Ok(run)
    }

    pub fn list_all(
        &self,
        limit: i64,
    ) -> Result<Vec<super::GenesisRun>, rusqlite::Error> {
        let conn = self.pool.get().map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, session_id, premise, status, current_step, current_step_number, total_steps, steps_json, error_message, created_at, updated_at FROM genesis_runs ORDER BY created_at DESC LIMIT ?1"
        )?;
        let runs = stmt.query_map([limit], |row| {
            let created_str: String = row.get(10)?;
            let updated_str: String = row.get(11)?;
            Ok(super::GenesisRun {
                id: row.get(0)?,
                story_id: row.get(1)?,
                session_id: row.get(2)?,
                premise: row.get(3)?,
                status: row.get(4)?,
                current_step: row.get(5)?,
                current_step_number: row.get(6)?,
                total_steps: row.get(7)?,
                steps_json: row.get(8)?,
                error_message: row.get(9)?,
                created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                updated_at: updated_str.parse().unwrap_or_else(|_| Local::now()),
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(runs)
    }
}
