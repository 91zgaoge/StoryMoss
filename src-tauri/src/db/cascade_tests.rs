//! 级联删除功能测试
//! 验证数据库外键约束和级联删除是否正确工作

use super::*;
use crate::db::repositories::*;
use crate::db::models::*;
use rusqlite::Connection;
use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试数据库连接
    fn create_test_db() -> Result<Arc<Connection>, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        initialize_database(&conn)?;
        Ok(Arc::new(conn))
    }

    /// 测试删除故事时级联删除所有相关数据
    #[test]
    fn test_story_cascade_delete() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_test_db()?;

        // 创建测试故事
        let story_req = CreateStoryRequest {
            title: "测试故事".to_string(),
            description: Some("测试描述".to_string()),
            genre: Some("科幻".to_string()),
            target_audience: Some("成人".to_string()),
            estimated_length: Some("中篇".to_string()),
            language: Some("zh-CN".to_string()),
            tags: Some(vec!["测试".to_string()]),
        };

        let story_repo = StoryRepository::new(conn.clone());
        let story = story_repo.create(story_req)?;

        // 创建角色
        let character_req = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "测试角色".to_string(),
            description: Some("测试角色描述".to_string()),
            role: Some("主角".to_string()),
            personality: None,
            background: None,
            goals: None,
            avatar_url: None,
        };

        let character_repo = CharacterRepository::new(conn.clone());
        let character = character_repo.create(character_req)?;

        // 创建章节
        let chapter_req = CreateChapterRequest {
            story_id: story.id.clone(),
            title: "测试章节".to_string(),
            description: Some("测试章节描述".to_string()),
            order_index: 1,
            word_count_target: Some(2000),
        };

        let chapter_repo = ChapterRepository::new(conn.clone());
        let chapter = chapter_repo.create(chapter_req)?;

        // 创建场景
        let scene_req = CreateSceneRequest {
            story_id: story.id.clone(),
            chapter_id: Some(chapter.id.clone()),
            title: "测试场景".to_string(),
            description: Some("测试场景描述".to_string()),
            content: Some("测试场景内容".to_string()),
            order_index: 1,
            location: None,
            time_period: None,
            weather: None,
            mood: None,
            pov_character_id: Some(character.id.clone()),
        };

        let scene_repo = SceneRepository::new(conn.clone());
        let scene = scene_repo.create(scene_req)?;

        // 创建情节点
        let plot_point_req = CreatePlotPointRequest {
            story_id: story.id.clone(),
            scene_id: Some(scene.id.clone()),
            title: "测试情节点".to_string(),
            description: "测试情节点描述".to_string(),
            plot_point_type: "转折点".to_string(),
            order_index: 1,
            emotional_impact: Some(5),
            character_ids: Some(vec![character.id.clone()]),
        };

        let plot_point_repo = PlotPointRepository::new(conn.clone());
        let plot_point = plot_point_repo.create(plot_point_req)?;

        // 验证数据已创建
        assert!(story_repo.get_by_id(&story.id)?.is_some());
        assert!(character_repo.get_by_id(&character.id)?.is_some());
        assert!(chapter_repo.get_by_id(&chapter.id)?.is_some());
        assert!(scene_repo.get_by_id(&scene.id)?.is_some());
        assert!(plot_point_repo.get_by_id(&plot_point.id)?.is_some());

        // 删除故事 - 应该级联删除所有相关数据
        story_repo.delete(&story.id)?;

        // 验证所有相关数据都被删除
        assert!(story_repo.get_by_id(&story.id)?.is_none());
        assert!(character_repo.get_by_id(&character.id)?.is_none());
        assert!(chapter_repo.get_by_id(&chapter.id)?.is_none());
        assert!(scene_repo.get_by_id(&scene.id)?.is_none());
        assert!(plot_point_repo.get_by_id(&plot_point.id)?.is_none());

        println!("✅ 故事级联删除测试通过");
        Ok(())
    }

    /// 测试删除章节时级联删除场景和情节点
    #[test]
    fn test_chapter_cascade_delete() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_test_db()?;

        // 创建测试故事
        let story_req = CreateStoryRequest {
            title: "测试故事2".to_string(),
            description: Some("测试描述2".to_string()),
            genre: Some("奇幻".to_string()),
            target_audience: Some("青少年".to_string()),
            estimated_length: Some("长篇".to_string()),
            language: Some("zh-CN".to_string()),
            tags: Some(vec!["测试".to_string()]),
        };

        let story_repo = StoryRepository::new(conn.clone());
        let story = story_repo.create(story_req)?;

        // 创建章节
        let chapter_req = CreateChapterRequest {
            story_id: story.id.clone(),
            title: "测试章节2".to_string(),
            description: Some("测试章节描述2".to_string()),
            order_index: 1,
            word_count_target: Some(3000),
        };

        let chapter_repo = ChapterRepository::new(conn.clone());
        let chapter = chapter_repo.create(chapter_req)?;

        // 创建场景
        let scene_req = CreateSceneRequest {
            story_id: story.id.clone(),
            chapter_id: Some(chapter.id.clone()),
            title: "测试场景2".to_string(),
            description: Some("测试场景描述2".to_string()),
            content: Some("测试场景内容2".to_string()),
            order_index: 1,
            location: None,
            time_period: None,
            weather: None,
            mood: None,
            pov_character_id: None,
        };

        let scene_repo = SceneRepository::new(conn.clone());
        let scene = scene_repo.create(scene_req)?;

        // 创建情节点
        let plot_point_req = CreatePlotPointRequest {
            story_id: story.id.clone(),
            scene_id: Some(scene.id.clone()),
            title: "测试情节点2".to_string(),
            description: "测试情节点描述2".to_string(),
            plot_point_type: "高潮".to_string(),
            order_index: 1,
            emotional_impact: Some(8),
            character_ids: None,
        };

        let plot_point_repo = PlotPointRepository::new(conn.clone());
        let plot_point = plot_point_repo.create(plot_point_req)?;

        // 验证数据已创建
        assert!(chapter_repo.get_by_id(&chapter.id)?.is_some());
        assert!(scene_repo.get_by_id(&scene.id)?.is_some());
        assert!(plot_point_repo.get_by_id(&plot_point.id)?.is_some());

        // 删除章节 - 应该级联删除相关场景和情节点
        chapter_repo.delete(&chapter.id)?;

        // 验证章节、场景和情节点都被删除，但故事保留
        assert!(story_repo.get_by_id(&story.id)?.is_some()); // 故事应该保留
        assert!(chapter_repo.get_by_id(&chapter.id)?.is_none());
        assert!(scene_repo.get_by_id(&scene.id)?.is_none());
        assert!(plot_point_repo.get_by_id(&plot_point.id)?.is_none());

        println!("✅ 章节级联删除测试通过");
        Ok(())
    }

    /// 测试删除角色时相关引用被正确处理
    #[test]
    fn test_character_delete_with_references() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_test_db()?;

        // 创建测试故事
        let story_req = CreateStoryRequest {
            title: "测试故事3".to_string(),
            description: Some("测试描述3".to_string()),
            genre: Some("悬疑".to_string()),
            target_audience: Some("成人".to_string()),
            estimated_length: Some("短篇".to_string()),
            language: Some("zh-CN".to_string()),
            tags: Some(vec!["测试".to_string()]),
        };

        let story_repo = StoryRepository::new(conn.clone());
        let story = story_repo.create(story_req)?;

        // 创建角色
        let character_req = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "测试角色3".to_string(),
            description: Some("测试角色描述3".to_string()),
            role: Some("配角".to_string()),
            personality: None,
            background: None,
            goals: None,
            avatar_url: None,
        };

        let character_repo = CharacterRepository::new(conn.clone());
        let character = character_repo.create(character_req)?;

        // 创建场景，使用该角色作为POV角色
        let scene_req = CreateSceneRequest {
            story_id: story.id.clone(),
            chapter_id: None,
            title: "测试场景3".to_string(),
            description: Some("测试场景描述3".to_string()),
            content: Some("测试场景内容3".to_string()),
            order_index: 1,
            location: None,
            time_period: None,
            weather: None,
            mood: None,
            pov_character_id: Some(character.id.clone()),
        };

        let scene_repo = SceneRepository::new(conn.clone());
        let scene = scene_repo.create(scene_req)?;

        // 验证场景的POV角色ID已设置
        let created_scene = scene_repo.get_by_id(&scene.id)?.unwrap();
        assert_eq!(created_scene.pov_character_id, Some(character.id.clone()));

        // 删除角色 - 场景的pov_character_id应该被设置为NULL
        character_repo.delete(&character.id)?;

        // 验证角色被删除，但场景保留且pov_character_id为NULL
        assert!(character_repo.get_by_id(&character.id)?.is_none());
        let updated_scene = scene_repo.get_by_id(&scene.id)?.unwrap();
        assert!(updated_scene.pov_character_id.is_none());

        println!("✅ 角色删除引用处理测试通过");
        Ok(())
    }

    /// 测试场景版本的级联删除
    #[test]
    fn test_scene_version_cascade_delete() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_test_db()?;

        // 创建测试故事
        let story_req = CreateStoryRequest {
            title: "测试故事4".to_string(),
            description: Some("测试描述4".to_string()),
            genre: Some("历史".to_string()),
            target_audience: Some("成人".to_string()),
            estimated_length: Some("中篇".to_string()),
            language: Some("zh-CN".to_string()),
            tags: Some(vec!["测试".to_string()]),
        };

        let story_repo = StoryRepository::new(conn.clone());
        let story = story_repo.create(story_req)?;

        // 创建场景
        let scene_req = CreateSceneRequest {
            story_id: story.id.clone(),
            chapter_id: None,
            title: "测试场景4".to_string(),
            description: Some("测试场景描述4".to_string()),
            content: Some("测试场景内容4".to_string()),
            order_index: 1,
            location: None,
            time_period: None,
            weather: None,
            mood: None,
            pov_character_id: None,
        };

        let scene_repo = SceneRepository::new(conn.clone());
        let scene = scene_repo.create(scene_req)?;

        // 直接在数据库中创建场景版本记录来测试级联删除
        conn.execute(
            "INSERT INTO scene_versions (id, scene_id, version_number, content, created_at) VALUES (?, ?, ?, ?, datetime('now'))",
            [&format!("version_{}", scene.id), &scene.id, "1", "版本1内容"],
        )?;

        conn.execute(
            "INSERT INTO scene_versions (id, scene_id, version_number, content, created_at) VALUES (?, ?, ?, ?, datetime('now'))",
            [&format!("version2_{}", scene.id), &scene.id, "2", "版本2内容"],
        )?;

        // 验证场景版本已创建
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM scene_versions WHERE scene_id = ?")?;
        let count: i64 = stmt.query_row([&scene.id], |row| row.get(0))?;
        assert_eq!(count, 2);

        // 删除场景 - 应该级联删除所有版本
        scene_repo.delete(&scene.id)?;

        // 验证场景和所有版本都被删除
        assert!(scene_repo.get_by_id(&scene.id)?.is_none());
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM scene_versions WHERE scene_id = ?")?;
        let count: i64 = stmt.query_row([&scene.id], |row| row.get(0))?;
        assert_eq!(count, 0);

        println!("✅ 场景版本级联删除测试通过");
        Ok(())
    }

    /// 测试角色关系的级联删除
    #[test]
    fn test_character_relationship_cascade_delete() -> Result<(), Box<dyn std::error::Error>> {
        let conn = create_test_db()?;

        // 创建测试故事
        let story_req = CreateStoryRequest {
            title: "测试故事5".to_string(),
            description: Some("测试描述5".to_string()),
            genre: Some("爱情".to_string()),
            target_audience: Some("成人".to_string()),
            estimated_length: Some("长篇".to_string()),
            language: Some("zh-CN".to_string()),
            tags: Some(vec!["测试".to_string()]),
        };

        let story_repo = StoryRepository::new(conn.clone());
        let story = story_repo.create(story_req)?;

        // 创建两个角色
        let character1_req = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "角色1".to_string(),
            description: Some("角色1描述".to_string()),
            role: Some("主角".to_string()),
            personality: None,
            background: None,
            goals: None,
            avatar_url: None,
        };

        let character2_req = CreateCharacterRequest {
            story_id: story.id.clone(),
            name: "角色2".to_string(),
            description: Some("角色2描述".to_string()),
            role: Some("主角".to_string()),
            personality: None,
            background: None,
            goals: None,
            avatar_url: None,
        };

        let character_repo = CharacterRepository::new(conn.clone());
        let character1 = character_repo.create(character1_req)?;
        let character2 = character_repo.create(character2_req)?;

        // 创建角色关系
        conn.execute(
            "INSERT INTO character_relationships (id, story_id, character_a_id, character_b_id, relationship_type, description, created_at) VALUES (?, ?, ?, ?, ?, ?, datetime('now'))",
            [
                &format!("rel_{}_{}", character1.id, character2.id),
                &story.id,
                &character1.id,
                &character2.id,
                "朋友",
                "好朋友关系"
            ],
        )?;

        // 验证关系已创建
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM character_relationships WHERE character_a_id = ? OR character_b_id = ?")?;
        let count: i64 = stmt.query_row([&character1.id, &character1.id], |row| row.get(0))?;
        assert!(count > 0);

        // 删除角色1 - 应该级联删除相关的关系
        character_repo.delete(&character1.id)?;

        // 验证角色1被删除，相关关系也被删除
        assert!(character_repo.get_by_id(&character1.id)?.is_none());
        assert!(character_repo.get_by_id(&character2.id)?.is_some()); // 角色2应该保留

        let mut stmt = conn.prepare("SELECT COUNT(*) FROM character_relationships WHERE character_a_id = ? OR character_b_id = ?")?;
        let count: i64 = stmt.query_row([&character1.id, &character1.id], |row| row.get(0))?;
        assert_eq!(count, 0); // 相关关系应该被删除

        println!("✅ 角色关系级联删除测试通过");
        Ok(())
    }

    /// 运行所有级联删除测试
    #[test]
    fn test_all_cascade_deletes() -> Result<(), Box<dyn std::error::Error>> {
        println!("🧪 开始运行级联删除测试套件...");

        test_story_cascade_delete()?;
        test_chapter_cascade_delete()?;
        test_character_delete_with_references()?;
        test_scene_version_cascade_delete()?;
        test_character_relationship_cascade_delete()?;

        println!("🎉 所有级联删除测试通过！");
        Ok(())
    }
}