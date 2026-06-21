#[cfg(test)]
mod cascade_delete_tests {
    use crate::db::{
        connection::create_test_pool,
        repositories::{ChapterRepository, CharacterRepository, StoryRepository},
        CreateChapterRequest, CreateCharacterRequest, CreateStoryRequest,
    };

    #[test]
    fn test_story_cascade_delete() -> Result<(), Box<dyn std::error::Error>> {
        // 创建测试数据库
        let pool = create_test_pool()?;

        // 创建仓库实例
        let story_repo = StoryRepository::new(pool.clone());
        let character_repo = CharacterRepository::new(pool.clone());
        let chapter_repo = ChapterRepository::new(pool.clone());

        // 1. 创建故事
        let story_request = CreateStoryRequest {
            title: "Test Story".to_string(),
            description: Some("A test story for cascade delete".to_string()),
            genre: Some("Fantasy".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let story = story_repo.create(story_request)?;

        // 2. 创建角色
        let character_request = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "Test Character".to_string(),
            background: Some("A brave hero".to_string()),
            personality: Some("Courageous and kind".to_string()),
            goals: Some("Save the world".to_string()),
            appearance: Some("Tall and strong".to_string()),
            gender: Some("Male".to_string()),
            age: Some(25),
        };
        let character = character_repo.create(character_request)?;

        // 3. 创建章节
        let chapter_request = CreateChapterRequest {
            story_id: story.id.clone(),
            chapter_number: 1,
            title: Some("Chapter 1".to_string()),
            outline: Some("The beginning".to_string()),
            content: Some("Once upon a time...".to_string()),
        };
        let chapter = chapter_repo.create(chapter_request)?;

        // 4. 验证数据已创建
        assert!(story_repo.get_by_id(&story.id)?.is_some());
        assert!(!character_repo.get_by_story(&story.id)?.is_empty());
        assert!(!chapter_repo.get_by_story(&story.id)?.is_empty());

        // 5. 删除故事（应该级联删除角色和章节）
        let deleted_count = story_repo.delete(&story.id)?;
        assert_eq!(deleted_count, 1);

        // 6. 验证级联删除生效
        assert!(story_repo.get_by_id(&story.id)?.is_none());
        assert!(character_repo.get_by_story(&story.id)?.is_empty());
        assert!(chapter_repo.get_by_story(&story.id)?.is_empty());

        // 7. 验证角色和章节确实被删除
        assert!(character_repo.get_by_id(&character.id)?.is_none());
        assert!(chapter_repo.get_by_id(&chapter.id)?.is_none());

        Ok(())
    }

    #[test]
    fn test_character_delete_does_not_affect_story() -> Result<(), Box<dyn std::error::Error>> {
        // 创建测试数据库
        let pool = create_test_pool()?;

        // 创建仓库实例
        let story_repo = StoryRepository::new(pool.clone());
        let character_repo = CharacterRepository::new(pool.clone());

        // 1. 创建故事
        let story_request = CreateStoryRequest {
            title: "Test Story".to_string(),
            description: Some("A test story".to_string()),
            genre: Some("Fantasy".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let story = story_repo.create(story_request)?;

        // 2. 创建角色
        let character_request = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "Test Character".to_string(),
            background: Some("A brave hero".to_string()),
            personality: Some("Courageous".to_string()),
            goals: Some("Save the world".to_string()),
            appearance: Some("Tall".to_string()),
            gender: Some("Male".to_string()),
            age: Some(25),
        };
        let character = character_repo.create(character_request)?;

        // 3. 删除角色
        let deleted_count = character_repo.delete(&character.id)?;
        assert_eq!(deleted_count, 1);

        // 4. 验证故事仍然存在
        assert!(story_repo.get_by_id(&story.id)?.is_some());
        assert!(character_repo.get_by_id(&character.id)?.is_none());

        Ok(())
    }

    #[test]
    fn test_chapter_delete_does_not_affect_story() -> Result<(), Box<dyn std::error::Error>> {
        // 创建测试数据库
        let pool = create_test_pool()?;

        // 创建仓库实例
        let story_repo = StoryRepository::new(pool.clone());
        let chapter_repo = ChapterRepository::new(pool.clone());

        // 1. 创建故事
        let story_request = CreateStoryRequest {
            title: "Test Story".to_string(),
            description: Some("A test story".to_string()),
            genre: Some("Fantasy".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let story = story_repo.create(story_request)?;

        // 2. 创建章节
        let chapter_request = CreateChapterRequest {
            story_id: story.id.clone(),
            chapter_number: 1,
            title: Some("Chapter 1".to_string()),
            outline: Some("The beginning".to_string()),
            content: Some("Once upon a time...".to_string()),
        };
        let chapter = chapter_repo.create(chapter_request)?;

        // 3. 删除章节
        let deleted_count = chapter_repo.delete(&chapter.id)?;
        assert_eq!(deleted_count, 1);

        // 4. 验证故事仍然存在
        assert!(story_repo.get_by_id(&story.id)?.is_some());
        assert!(chapter_repo.get_by_id(&chapter.id)?.is_none());

        Ok(())
    }

    #[test]
    fn test_multiple_stories_cascade_delete() -> Result<(), Box<dyn std::error::Error>> {
        // 创建测试数据库
        let pool = create_test_pool()?;

        // 创建仓库实例
        let story_repo = StoryRepository::new(pool.clone());
        let character_repo = CharacterRepository::new(pool.clone());
        let chapter_repo = ChapterRepository::new(pool.clone());

        // 1. 创建两个故事
        let story1_request = CreateStoryRequest {
            title: "Story 1".to_string(),
            description: Some("First story".to_string()),
            genre: Some("Fantasy".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let story1 = story_repo.create(story1_request)?;

        let story2_request = CreateStoryRequest {
            title: "Story 2".to_string(),
            description: Some("Second story".to_string()),
            genre: Some("Sci-Fi".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        };
        let story2 = story_repo.create(story2_request)?;

        // 2. 为每个故事创建角色和章节
        let char1_request = CreateCharacterRequest {
            story_id: story1.id.clone(),
            name: "Hero 1".to_string(),
            background: Some("First hero".to_string()),
            personality: Some("Brave".to_string()),
            goals: Some("Quest".to_string()),
            appearance: Some("Strong".to_string()),
            gender: Some("Male".to_string()),
            age: Some(30),
        };
        let character1 = character_repo.create(char1_request)?;

        let char2_request = CreateCharacterRequest {
            story_id: story2.id.clone(),
            name: "Hero 2".to_string(),
            background: Some("Second hero".to_string()),
            personality: Some("Smart".to_string()),
            goals: Some("Discovery".to_string()),
            appearance: Some("Tall".to_string()),
            gender: Some("Female".to_string()),
            age: Some(25),
        };
        let character2 = character_repo.create(char2_request)?;

        let chapter1_request = CreateChapterRequest {
            story_id: story1.id.clone(),
            chapter_number: 1,
            title: Some("Chapter 1-1".to_string()),
            outline: Some("Beginning of story 1".to_string()),
            content: Some("Story 1 content".to_string()),
        };
        let chapter1 = chapter_repo.create(chapter1_request)?;

        let chapter2_request = CreateChapterRequest {
            story_id: story2.id.clone(),
            chapter_number: 1,
            title: Some("Chapter 2-1".to_string()),
            outline: Some("Beginning of story 2".to_string()),
            content: Some("Story 2 content".to_string()),
        };
        let chapter2 = chapter_repo.create(chapter2_request)?;

        // 3. 删除第一个故事
        story_repo.delete(&story1.id)?;

        // 4. 验证只有第一个故事及其相关数据被删除
        assert!(story_repo.get_by_id(&story1.id)?.is_none());
        assert!(character_repo.get_by_id(&character1.id)?.is_none());
        assert!(chapter_repo.get_by_id(&chapter1.id)?.is_none());

        // 5. 验证第二个故事及其数据仍然存在
        assert!(story_repo.get_by_id(&story2.id)?.is_some());
        assert!(character_repo.get_by_id(&character2.id)?.is_some());
        assert!(chapter_repo.get_by_id(&chapter2.id)?.is_some());

        Ok(())
    }
}
