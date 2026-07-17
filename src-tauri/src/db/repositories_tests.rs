//! Repository 单元测试
//!
//! 覆盖 Story / Character / Chapter 的完整 CRUD 流程。

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::db::connection::create_test_pool;

    // ==================== StoryRepository ====================

    #[test]
    fn test_story_create_and_get() {
        let pool = create_test_pool().unwrap();
        let repo = StoryRepository::new(pool);

        let req = CreateStoryRequest {
            title: "测试小说".to_string(),
            description: Some("描述".to_string()),
            genre: Some("科幻".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };

        let story = repo.create(req).unwrap();
        assert_eq!(story.title, "测试小说");
        assert_eq!(story.genre, Some("科幻".to_string()));

        let fetched = repo.get_by_id(&story.id).unwrap().unwrap();
        assert_eq!(fetched.title, story.title);
        assert_eq!(fetched.genre, story.genre);
    }

    #[test]
    fn test_story_get_all() {
        let pool = create_test_pool().unwrap();
        let repo = StoryRepository::new(pool);

        let req1 = CreateStoryRequest {
            title: "小说A".to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let req2 = CreateStoryRequest {
            title: "小说B".to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };

        repo.create(req1).unwrap();
        repo.create(req2).unwrap();

        let all = repo.get_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_story_update() {
        let pool = create_test_pool().unwrap();
        let repo = StoryRepository::new(pool);

        let req = CreateStoryRequest {
            title: "原标题".to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let story = repo.create(req).unwrap();

        let update_req = UpdateStoryRequest {
            title: Some("新标题".to_string()),
            description: Some("新描述".to_string()),
            genre: None,
            tone: Some("轻快".to_string()),
            pacing: Some("快速".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            methodology_step: None,
            reference_book_id: None,
        };

        let count = repo.update(&story.id, &update_req).unwrap();
        assert_eq!(count, 1);

        let updated = repo.get_by_id(&story.id).unwrap().unwrap();
        assert_eq!(updated.title, "新标题");
        assert_eq!(updated.tone, Some("轻快".to_string()));
    }

    #[test]
    fn test_story_delete() {
        let pool = create_test_pool().unwrap();
        let repo = StoryRepository::new(pool);

        let req = CreateStoryRequest {
            title: "待删除".to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let story = repo.create(req).unwrap();

        let count = repo.delete(&story.id).unwrap();
        assert_eq!(count, 1);

        let deleted = repo.get_by_id(&story.id).unwrap();
        assert!(deleted.is_none());
    }

    #[test]
    fn test_story_get_by_id_not_found() {
        let pool = create_test_pool().unwrap();
        let repo = StoryRepository::new(pool);

        let result = repo.get_by_id("non-existent-id").unwrap();
        assert!(result.is_none());
    }

    // ==================== CharacterRepository ====================

    #[test]
    fn test_character_create_and_get_by_story() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let char_repo = CharacterRepository::new(pool);

        let story_req = CreateStoryRequest {
            title: "角色测试".to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let story = story_repo.create(story_req).unwrap();

        let char_req = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "张三".to_string(),
            background: Some("主角".to_string()),
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,

            source: None,
            is_auto_generated: None,
        };
        let character = char_repo.create(char_req).unwrap();
        assert_eq!(character.name, "张三");
        assert_eq!(character.story_id, story.id);

        let chars = char_repo.get_by_story(&story.id).unwrap();
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "张三");
    }

    #[test]
    fn test_character_get_by_id() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let char_repo = CharacterRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let char_req = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "李四".to_string(),
            background: None,
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,

            source: None,
            is_auto_generated: None,
        };
        let character = char_repo.create(char_req).unwrap();

        let fetched = char_repo.get_by_id(&character.id).unwrap().unwrap();
        assert_eq!(fetched.name, "李四");
    }

    #[test]
    fn test_character_update() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let char_repo = CharacterRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let char_req = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "原名".to_string(),
            background: Some("背景".to_string()),
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,

            source: None,
            is_auto_generated: None,
        };
        let character = char_repo.create(char_req).unwrap();

        let count = char_repo
            .update(
                &character.id,
                Some("新名".to_string()),
                Some("新背景".to_string()),
                Some("开朗".to_string()),
                Some("成为英雄".to_string()),
                None,
                None,
                None,
            )
            .unwrap();
        assert_eq!(count, 1);

        let updated = char_repo.get_by_id(&character.id).unwrap().unwrap();
        assert_eq!(updated.name, "新名");
        assert_eq!(updated.background, Some("新背景".to_string()));
        assert_eq!(updated.personality, Some("开朗".to_string()));
        assert_eq!(updated.goals, Some("成为英雄".to_string()));
    }

    #[test]
    fn test_character_delete() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let char_repo = CharacterRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let char_req = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "待删除".to_string(),
            background: None,
            personality: None,
            goals: None,
            appearance: None,
            gender: None,
            age: None,

            source: None,
            is_auto_generated: None,
        };
        let character = char_repo.create(char_req).unwrap();

        let count = char_repo.delete(&character.id).unwrap();
        assert_eq!(count, 1);

        let chars = char_repo.get_by_story(&story.id).unwrap();
        assert_eq!(chars.len(), 0);
    }

    // ==================== ChapterRepository ====================

    #[test]
    fn test_chapter_create_and_get_by_story() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let chapter_repo = ChapterRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "章节测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let chapter_req = CreateChapterRequest {
            story_id: story.id.clone(),
            chapter_number: 1,
            title: Some("第一章".to_string()),
            outline: Some("大纲".to_string()),
            content: Some("正文内容".to_string()),
        };
        let chapter = chapter_repo.create(chapter_req).unwrap();
        assert_eq!(chapter.chapter_number, 1);
        assert_eq!(chapter.title, Some("第一章".to_string()));

        let chapters = chapter_repo.get_by_story(&story.id).unwrap();
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title, Some("第一章".to_string()));
    }

    #[test]
    fn test_chapter_get_by_id() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let chapter_repo = ChapterRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let chapter_req = CreateChapterRequest {
            story_id: story.id.clone(),
            chapter_number: 1,
            title: Some("标题".to_string()),
            outline: None,
            content: None,
        };
        let chapter = chapter_repo.create(chapter_req).unwrap();

        let fetched = chapter_repo.get_by_id(&chapter.id).unwrap().unwrap();
        assert_eq!(fetched.chapter_number, 1);
    }

    #[test]
    fn test_chapter_update() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let chapter_repo = ChapterRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let chapter_req = CreateChapterRequest {
            story_id: story.id.clone(),
            chapter_number: 1,
            title: Some("原标题".to_string()),
            outline: Some("原大纲".to_string()),
            content: Some("原内容".to_string()),
        };
        let chapter = chapter_repo.create(chapter_req).unwrap();

        let count = chapter_repo
            .update(
                &chapter.id,
                Some("新标题".to_string()),
                Some("新大纲".to_string()),
                None, // Phase 1: word_count 手动指定
            )
            .unwrap();
        assert_eq!(count, 1);

        let updated = chapter_repo.get_by_id(&chapter.id).unwrap().unwrap();
        assert_eq!(updated.title, Some("新标题".to_string()));
        // Phase 1: content 走 Scene 表，get_by_id 会自动 fallback 到 scenes
    }

    #[test]
    fn test_chapter_delete() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let chapter_repo = ChapterRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let chapter_req = CreateChapterRequest {
            story_id: story.id.clone(),
            chapter_number: 1,
            title: None,
            outline: None,
            content: None,
        };
        let chapter = chapter_repo.create(chapter_req).unwrap();

        let count = chapter_repo.delete(&chapter.id).unwrap();
        assert_eq!(count, 1);

        let deleted = chapter_repo.get_by_id(&chapter.id).unwrap();
        assert!(deleted.is_none());
    }

    #[test]
    fn test_chapter_order_by_number() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let chapter_repo = ChapterRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "排序测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let req1 = CreateChapterRequest {
            story_id: story.id.clone(),
            chapter_number: 3,
            title: Some("第三章".to_string()),
            outline: None,
            content: None,
        };
        let req2 = CreateChapterRequest {
            story_id: story.id.clone(),
            chapter_number: 1,
            title: Some("第一章".to_string()),
            outline: None,
            content: None,
        };
        chapter_repo.create(req1).unwrap();
        chapter_repo.create(req2).unwrap();

        let chapters = chapter_repo.get_by_story(&story.id).unwrap();
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].chapter_number, 1); // 按 chapter_number 排序
        assert_eq!(chapters[1].chapter_number, 3);
    }

    #[test]
    fn test_chapter_get_by_story_paged_excludes_large_fields() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let chapter_repo = ChapterRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "分页测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        for i in 1..=3 {
            chapter_repo
                .create(CreateChapterRequest {
                    story_id: story.id.clone(),
                    chapter_number: i,
                    title: Some(format!("第{}章", i)),
                    outline: Some(format!("大纲{}", i)),
                    content: Some(format!("正文{}", i)),
                })
                .unwrap();
        }

        let paged = chapter_repo.get_by_story_paged(&story.id, 2, 0).unwrap();
        assert_eq!(paged.len(), 2);
        assert_eq!(paged[0].chapter_number, 1);
        assert_eq!(paged[0].title, Some("第1章".to_string()));
        assert_eq!(paged[0].content, None);
        assert_eq!(paged[0].outline, None);

        let second_page = chapter_repo.get_by_story_paged(&story.id, 2, 2).unwrap();
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].chapter_number, 3);

        assert_eq!(chapter_repo.count_by_story(&story.id).unwrap(), 3);
        // "正文1" + "正文2" + "正文3" = 3 * 3 = 9 chars
        assert_eq!(
            chapter_repo
                .total_content_length_by_story(&story.id)
                .unwrap(),
            9
        );
    }

    // ==================== SceneRepository ====================

    #[test]
    fn test_scene_create_and_get_by_story() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "场景测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let scene = scene_repo.create(&story.id, 1, Some("开场")).unwrap();
        assert_eq!(scene.sequence_number, 1);
        assert_eq!(scene.title, Some("开场".to_string()));
        assert_eq!(scene.story_id, story.id);

        let scenes = scene_repo.get_by_story(&story.id).unwrap();
        assert_eq!(scenes.len(), 1);
        assert_eq!(scenes[0].title, Some("开场".to_string()));
        assert_eq!(scenes[0].sequence_number, 1);
    }

    #[test]
    fn test_scene_get_by_id() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let scene = scene_repo.create(&story.id, 1, Some("场景1")).unwrap();
        let fetched = scene_repo.get_by_id(&scene.id).unwrap().unwrap();
        assert_eq!(fetched.id, scene.id);
        assert_eq!(fetched.title, Some("场景1".to_string()));
    }

    #[test]
    fn test_scene_update() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let scene = scene_repo.create(&story.id, 1, Some("原标题")).unwrap();

        let updates = SceneUpdate {
            title: Some("新标题".to_string()),
            dramatic_goal: Some("制造悬念".to_string()),
            external_pressure: Some("时间紧迫".to_string()),
            conflict_type: Some(ConflictType::ManVsMan),
            characters_present: Some(vec!["角色A".to_string(), "角色B".to_string()]),
            character_conflicts: Some(vec![CharacterConflict {
                character_a_id: "a".to_string(),
                character_b_id: "b".to_string(),
                conflict_nature: "对立".to_string(),
                stakes: "生死攸关".to_string(),
            }]),
            content: Some("新的场景内容".to_string()),
            setting_location: Some("古堡".to_string()),
            setting_time: Some("午夜".to_string()),
            setting_atmosphere: Some("阴森".to_string()),
            previous_scene_id: None,
            next_scene_id: None,
            confidence_score: Some(0.95),
            execution_stage: None,
            outline_content: None,
            draft_content: None,
            style_blend_override: None,
            foreshadowing_ids: None,

            source: None,
            is_auto_generated: None,
        };

        let count = scene_repo.update(&scene.id, &updates).unwrap();
        assert_eq!(count, 1);

        let updated = scene_repo.get_by_id(&scene.id).unwrap().unwrap();
        assert_eq!(updated.title, Some("新标题".to_string()));
        assert_eq!(updated.dramatic_goal, Some("制造悬念".to_string()));
        assert_eq!(updated.external_pressure, Some("时间紧迫".to_string()));
        assert_eq!(updated.conflict_type, Some(ConflictType::ManVsMan));
        assert_eq!(
            updated.characters_present,
            vec!["角色A".to_string(), "角色B".to_string()]
        );
        assert_eq!(updated.content, Some("新的场景内容".to_string()));
        assert_eq!(updated.setting_location, Some("古堡".to_string()));
        assert_eq!(updated.setting_time, Some("午夜".to_string()));
        assert_eq!(updated.setting_atmosphere, Some("阴森".to_string()));
        assert_eq!(updated.confidence_score, Some(0.95));
    }

    #[test]
    fn test_scene_delete() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let scene = scene_repo.create(&story.id, 1, Some("待删除")).unwrap();
        let count = scene_repo.delete(&scene.id).unwrap();
        assert_eq!(count, 1);

        let deleted = scene_repo.get_by_id(&scene.id).unwrap();
        assert!(deleted.is_none());

        let scenes = scene_repo.get_by_story(&story.id).unwrap();
        assert_eq!(scenes.len(), 0);
    }

    #[test]
    fn test_scene_reorder() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool);

        let story = story_repo
            .create(CreateStoryRequest {
                title: "排序测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let scene1 = scene_repo.create(&story.id, 1, Some("场景1")).unwrap();
        let scene2 = scene_repo.create(&story.id, 2, Some("场景2")).unwrap();

        // 交换顺序（借助临时序号避免 UNIQUE 冲突）
        let _ = scene_repo.update_sequence(&scene1.id, 999).unwrap();
        let count1 = scene_repo.update_sequence(&scene2.id, 1).unwrap();
        let count2 = scene_repo.update_sequence(&scene1.id, 2).unwrap();
        assert_eq!(count1, 1);
        assert_eq!(count2, 1);

        let scenes = scene_repo.get_by_story(&story.id).unwrap();
        assert_eq!(scenes.len(), 2);
        assert_eq!(scenes[0].id, scene2.id); // 现在是第1个
        assert_eq!(scenes[0].sequence_number, 1);
        assert_eq!(scenes[1].id, scene1.id); // 现在是第2个
        assert_eq!(scenes[1].sequence_number, 2);
    }

    #[test]
    fn test_scene_create_in_tx() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool.clone());

        let story = story_repo
            .create(CreateStoryRequest {
                title: "事务测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let mut conn = pool.get().unwrap();
        let tx = conn.transaction().unwrap();

        let scene = scene_repo
            .create_in_tx(&tx, &story.id, 1, Some("事务场景"))
            .unwrap();
        assert_eq!(scene.story_id, story.id);
        assert_eq!(scene.sequence_number, 1);
        assert_eq!(scene.title, Some("事务场景".to_string()));
        assert!(scene.characters_present.is_empty());
        assert!(scene.character_conflicts.is_empty());

        tx.commit().unwrap();

        // Verify the scene is persisted after commit
        let fetched = scene_repo.get_by_id(&scene.id).unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.title, Some("事务场景".to_string()));
    }

    #[test]
    fn test_scene_get_by_story_full_field_mapping() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool.clone());

        let story = story_repo
            .create(CreateStoryRequest {
                title: "字段映射测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        // Create scene with all fields populated via direct SQL
        let conn = pool.get().unwrap();
        let scene_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scenes (
                id, story_id, sequence_number, title, dramatic_goal, external_pressure,
                conflict_type, characters_present, character_conflicts, setting_location,
                setting_time, setting_atmosphere, content, previous_scene_id, next_scene_id,
                model_used, cost, created_at, updated_at, confidence_score, execution_stage,
                outline_content, draft_content, style_blend_override, foreshadowing_ids, chapter_id,
                narrative_intensity, narrative_sentiment, narrative_event_types,
                narrative_preceding_scene_id, narrative_following_scene_id, act_number, position_in_act
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33)",
            rusqlite::params![
                &scene_id, &story.id, 1, "完整场景", "目标", "压力", "ManVsSelf",
                "[\"Alice\", \"Bob\"]", "[{\"character_a_id\": \"c1\", \"character_b_id\": \"c2\", \"conflict_nature\": \"internal\", \"stakes\": \"test\"}]",
                "古堡", "午夜", "阴森", "场景内容", rusqlite::types::Null, rusqlite::types::Null,
                "gpt-4", 0.05, &now, &now, 0.95, "drafting", "大纲", "草稿",
                "blend-1", "[\"f1\", \"f2\"]", rusqlite::types::Null,
                0.8, 0.8, "[\"event1\"]", rusqlite::types::Null, rusqlite::types::Null, 2, 0
            ],
        ).unwrap();
        drop(conn);

        let scenes = scene_repo.get_by_story(&story.id).unwrap();
        assert_eq!(scenes.len(), 1);
        let scene = &scenes[0];

        assert_eq!(scene.id, scene_id);
        assert_eq!(scene.story_id, story.id);
        assert_eq!(scene.sequence_number, 1);
        assert_eq!(scene.title, Some("完整场景".to_string()));
        assert_eq!(scene.dramatic_goal, Some("目标".to_string()));
        assert_eq!(scene.external_pressure, Some("压力".to_string()));
        assert_eq!(scene.conflict_type, Some(ConflictType::ManVsSelf));
        assert_eq!(scene.characters_present, vec!["Alice", "Bob"]);
        assert_eq!(scene.character_conflicts.len(), 1);
        assert_eq!(scene.character_conflicts[0].character_a_id, "c1");
        assert_eq!(scene.setting_location, Some("古堡".to_string()));
        assert_eq!(scene.setting_time, Some("午夜".to_string()));
        assert_eq!(scene.setting_atmosphere, Some("阴森".to_string()));
        assert_eq!(scene.content, Some("场景内容".to_string()));
        assert_eq!(scene.previous_scene_id, None);
        assert_eq!(scene.next_scene_id, None);
        assert_eq!(scene.model_used, Some("gpt-4".to_string()));
        assert_eq!(scene.cost, Some(0.05));
        assert_eq!(scene.confidence_score, Some(0.95));
        assert_eq!(scene.execution_stage, Some("drafting".to_string()));
        assert_eq!(scene.outline_content, Some("大纲".to_string()));
        assert_eq!(scene.draft_content, Some("草稿".to_string()));
        assert_eq!(scene.style_blend_override, Some("blend-1".to_string()));
        assert_eq!(
            scene.foreshadowing_ids,
            Some(vec!["f1".to_string(), "f2".to_string()])
        );
        assert_eq!(scene.chapter_id, None);
        assert_eq!(scene.narrative_intensity, Some(0.8));
        assert_eq!(scene.narrative_sentiment, Some(0.8));
        assert_eq!(
            scene.narrative_event_types,
            Some("[\"event1\"]".to_string())
        );
        assert_eq!(scene.narrative_preceding_scene_id, None);
        assert_eq!(scene.narrative_following_scene_id, None);
        assert_eq!(scene.act_number, Some(2));
        assert_eq!(scene.position_in_act, Some(0));
    }

    #[test]
    fn test_scene_get_by_story_paged_excludes_large_fields() {
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool.clone());

        let story = story_repo
            .create(CreateStoryRequest {
                title: "场景分页测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let scene = scene_repo.create(&story.id, 1, Some("场景1")).unwrap();
        scene_repo
            .update(
                &scene.id,
                &SceneUpdate {
                    content: Some("场景正文".to_string()),
                    outline_content: Some("大纲内容".to_string()),
                    draft_content: Some("草稿内容".to_string()),
                    ..Default::default()
                },
            )
            .unwrap();

        let paged = scene_repo.get_by_story_paged(&story.id, 5, 0).unwrap();
        assert_eq!(paged.len(), 1);
        assert_eq!(paged[0].title, Some("场景1".to_string()));
        assert_eq!(paged[0].content, None);
        assert_eq!(paged[0].outline_content, None);
        assert_eq!(paged[0].draft_content, None);

        assert_eq!(scene_repo.count_by_story(&story.id).unwrap(), 1);
        assert_eq!(
            scene_repo.total_content_length_by_story(&story.id).unwrap(),
            4
        );
    }

    #[test]
    fn test_scene_total_content_length_matches_scene_contents() {
        // B5 Stage2: 验证 get_story_word_count 底层聚合与场景 content 长度一致
        let pool = create_test_pool().unwrap();
        let story_repo = StoryRepository::new(pool.clone());
        let scene_repo = SceneRepository::new(pool.clone());

        let story = story_repo
            .create(CreateStoryRequest {
                title: "字数统计测试".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();

        let contents = ["第一段场景内容。", "第二段更长一些的场景内容。", ""];
        for (i, content) in contents.iter().enumerate() {
            let scene = scene_repo
                .create(&story.id, (i + 1) as i32, Some(&format!("场景{}", i + 1)))
                .unwrap();
            if !content.is_empty() {
                scene_repo
                    .update(
                        &scene.id,
                        &SceneUpdate {
                            content: Some(content.to_string()),
                            ..Default::default()
                        },
                    )
                    .unwrap();
            }
        }

        // SQLite LENGTH 返回字符数，因此测试期望也用 chars().count()
        let expected_total: i64 = contents.iter().map(|c| c.chars().count() as i64).sum();
        assert_eq!(
            scene_repo.total_content_length_by_story(&story.id).unwrap(),
            expected_total
        );
        assert_eq!(scene_repo.count_by_story(&story.id).unwrap(), 3);

        // 删除一个场景后聚合结果应同步减少
        let scenes = scene_repo.get_by_story(&story.id).unwrap();
        let to_delete = scenes.iter().find(|s| s.sequence_number == 2).unwrap();
        scene_repo.delete(&to_delete.id).unwrap();

        assert_eq!(
            scene_repo.total_content_length_by_story(&story.id).unwrap(),
            expected_total - contents[1].chars().count() as i64
        );
        assert_eq!(scene_repo.count_by_story(&story.id).unwrap(), 2);
    }

    // ==================== GenesisRunRepository (P0-4) ====================

    // v0.26.19 P0-4 契约：genesis_runs 表必须记录创世运行的完整状态机迁移：
    //   pending → running → quick_done → completed（成功路径）
    //   pending → running → failed（快速阶段失败路径）
    //   仪表盘「Genesis 运行记录」依赖此表，此前旧创世管线不写导致永远为空。
    #[test]
    fn test_genesis_run_lifecycle_success() {
        let pool = create_test_pool().unwrap();
        let repo = GenesisRunRepository::new(pool);

        let run = repo.create("run-1", "ses-1", "末世废土幸存者", 8).unwrap();
        assert_eq!(run.status, "pending");
        assert_eq!(run.total_steps, 8);
        assert_eq!(run.premise, "末世废土幸存者");
        assert!(run.story_id.is_none());

        // 切 running
        repo.update_step("run-1", "构思故事", 1, "running", "{}")
            .unwrap();
        // 概念生成完成，记录 story_id 并切 quick_done
        repo.set_story_id_and_status("run-1", "story-abc", "quick_done")
            .unwrap();
        // 后台阶段完成
        repo.complete("run-1", Some("story-abc")).unwrap();

        let fetched = repo.get_by_id("run-1").unwrap().unwrap();
        assert_eq!(fetched.status, "completed");
        assert_eq!(fetched.story_id.as_deref(), Some("story-abc"));
        assert_eq!(fetched.current_step.as_deref(), Some("构思故事"));
        assert_eq!(fetched.current_step_number, 1);
    }

    #[test]
    fn test_genesis_run_lifecycle_quick_fail() {
        let pool = create_test_pool().unwrap();
        let repo = GenesisRunRepository::new(pool);

        repo.create("run-2", "ses-2", "科幻太空歌剧", 8).unwrap();
        repo.update_step("run-2", "构思故事", 1, "running", "{}")
            .unwrap();
        // 快速阶段失败
        repo.fail("run-2", "LLM gateway timeout").unwrap();

        let fetched = repo.get_by_id("run-2").unwrap().unwrap();
        assert_eq!(fetched.status, "failed");
        assert_eq!(
            fetched.error_message.as_deref(),
            Some("LLM gateway timeout")
        );
        assert!(fetched.story_id.is_none());
    }

    #[test]
    fn test_genesis_run_list_all_orders_by_created_desc() {
        let pool = create_test_pool().unwrap();
        let repo = GenesisRunRepository::new(pool);

        repo.create("run-a", "ses-a", "first", 8).unwrap();
        // 确保 created_at 不同（Local::now 精度到秒，简单 sleep）
        std::thread::sleep(std::time::Duration::from_millis(1100));
        repo.create("run-b", "ses-b", "second", 8).unwrap();
        repo.complete("run-b", Some("story-b")).unwrap();

        let list = repo.list_all(10).unwrap();
        assert_eq!(list.len(), 2);
        // 最近创建的在前
        assert_eq!(list[0].id, "run-b");
        assert_eq!(list[1].id, "run-a");
    }

    #[test]
    fn test_genesis_run_update_steps_json() {
        let pool = create_test_pool().unwrap();
        let repo = GenesisRunRepository::new(pool);

        repo.create("run-3", "ses-3", "推理悬疑", 8).unwrap();
        let steps_json = r#"{"errors":["world_update_failed","kg_relation_failed"]}"#;
        repo.update_steps_json("run-3", steps_json).unwrap();

        let fetched = repo.get_by_id("run-3").unwrap().unwrap();
        assert_eq!(fetched.steps_json, steps_json);
    }
}
