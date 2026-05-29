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
        };
        let req2 = CreateStoryRequest {
            title: "小说B".to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
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
        };
        let story = repo.create(req).unwrap();

        let update_req = UpdateStoryRequest {
            title: Some("新标题".to_string()),
            description: Some("新描述".to_string()),
            genre: None,
            tone: Some("轻快".to_string()),
            pacing: Some("快速".to_string()),
            style_dna_id: None,
            methodology_id: None,
            methodology_step: None,
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
                Some("新内容，更长一些".to_string()),
                None, // word_count 应该从 content 自动计算
            )
            .unwrap();
        assert_eq!(count, 1);

        let updated = chapter_repo.get_by_id(&chapter.id).unwrap().unwrap();
        assert_eq!(updated.title, Some("新标题".to_string()));
        assert_eq!(updated.content, Some("新内容，更长一些".to_string()));
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
}
