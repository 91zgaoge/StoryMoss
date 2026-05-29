//! 工作室配置管理器
//!
//! 负责每部小说的独立配置管理，包括导入/导出功能
#![allow(dead_code)]

use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

use chrono::Local;
use serde::{Deserialize, Serialize};
use zip::{write::FileOptions, ZipWriter};

use crate::db::*;

/// 工作室配置管理器
pub struct StudioManager {
    pool: DbPool,
    studios_dir: std::path::PathBuf,
}

impl StudioManager {
    pub fn new(pool: DbPool, app_dir: &Path) -> Self {
        let studios_dir = app_dir.join("studios");
        fs::create_dir_all(&studios_dir).ok();

        Self { pool, studios_dir }
    }

    /// 为小说创建默认工作室配置
    pub fn create_default_studio(
        &self,
        story_id: &str,
        _title: &str,
    ) -> Result<StudioConfig, Box<dyn std::error::Error>> {
        let studio_repo = StudioConfigRepository::new(self.pool.clone());

        // 检查是否已存在
        if let Some(existing) = studio_repo.get_by_story(story_id)? {
            return Ok(existing);
        }

        // 创建默认配置
        let mut studio = studio_repo.create(story_id)?;

        // 设置默认值
        studio.llm_config = Self::default_llm_config();
        studio.ui_config = Self::default_ui_config();
        studio.agent_bots = Self::default_agent_bots();

        // 保存到数据库
        studio_repo.update(
            &studio.id,
            None,
            Some(&studio.llm_config),
            Some(&studio.ui_config),
            Some(&studio.agent_bots),
        )?;

        // 创建配置文件目录
        let studio_dir = self.studios_dir.join(story_id);
        fs::create_dir_all(&studio_dir)?;

        // 写入默认CSS主题
        let frontstage_css = Self::default_frontstage_theme();
        let backstage_css = Self::default_backstage_theme();

        fs::write(studio_dir.join("frontstage_theme.css"), &frontstage_css)?;
        fs::write(studio_dir.join("backstage_theme.css"), &backstage_css)?;

        studio_repo.update_themes(&studio.id, Some(&frontstage_css), Some(&backstage_css))?;

        Ok(studio)
    }

    /// 导出工作室配置
    pub fn export_studio(
        &self,
        req: &StudioExportRequest,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let story_repo = StoryRepository::new(self.pool.clone());
        let scene_repo = SceneRepository::new(self.pool.clone());
        let world_repo = WorldBuildingRepository::new(self.pool.clone());
        let style_repo = WritingStyleRepository::new(self.pool.clone());
        let char_repo = CharacterRepository::new(self.pool.clone());
        let studio_repo = StudioConfigRepository::new(self.pool.clone());

        // 获取故事信息
        let story = story_repo
            .get_by_id(&req.story_id)?
            .ok_or("Story not found")?;

        // 构建导出数据
        let mut export_data = StudioExportData {
            manifest: ExportManifest {
                version: "3.0.0".to_string(),
                exported_at: Local::now(),
                story_id: story.id.clone(),
                story_title: story.title.clone(),
            },
            story: story.clone(),
            world_building: None,
            characters: vec![],
            writing_style: None,
            scenes: vec![],
            studio_config: None,
        };

        // 根据请求包含数据
        if req.include_world_building {
            export_data.world_building = world_repo.get_by_story(&req.story_id)?;
        }

        if req.include_characters {
            export_data.characters = char_repo.get_by_story(&req.story_id)?;
        }

        if req.include_writing_style {
            export_data.writing_style = style_repo.get_by_story(&req.story_id)?;
        }

        if req.include_scenes {
            export_data.scenes = scene_repo.get_by_story(&req.story_id)?;
        }

        if req.include_llm_config || req.include_ui_config || req.include_agent_bots {
            export_data.studio_config = studio_repo.get_by_story(&req.story_id)?;
        }

        // 打包为ZIP
        let mut zip_buffer = Vec::new();
        {
            let mut zip = ZipWriter::new(std::io::Cursor::new(&mut zip_buffer));
            let options =
                FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            // 写入manifest
            zip.start_file("manifest.json", options)?;
            zip.write_all(serde_json::to_string_pretty(&export_data.manifest)?.as_bytes())?;

            // 写入story
            zip.start_file("story.json", options)?;
            zip.write_all(serde_json::to_string_pretty(&export_data.story)?.as_bytes())?;

            // 写入world_building
            if let Some(wb) = &export_data.world_building {
                zip.start_file("world_building.json", options)?;
                zip.write_all(serde_json::to_string_pretty(wb)?.as_bytes())?;
            }

            // 写入characters
            if !export_data.characters.is_empty() {
                zip.start_file("characters.json", options)?;
                zip.write_all(serde_json::to_string_pretty(&export_data.characters)?.as_bytes())?;
            }

            // 写入writing_style
            if let Some(ws) = &export_data.writing_style {
                zip.start_file("writing_style.json", options)?;
                zip.write_all(serde_json::to_string_pretty(ws)?.as_bytes())?;
            }

            // 写入scenes
            if !export_data.scenes.is_empty() {
                zip.start_file("scenes.json", options)?;
                zip.write_all(serde_json::to_string_pretty(&export_data.scenes)?.as_bytes())?;
            }

            // 写入studio_config
            if let Some(sc) = &export_data.studio_config {
                zip.start_file("studio_config.json", options)?;
                zip.write_all(serde_json::to_string_pretty(sc)?.as_bytes())?;
            }

            zip.finish()?;
        }

        Ok(zip_buffer)
    }

    /// 导入工作室配置
    pub fn import_studio(
        &self,
        data: &[u8],
        options: &ImportOptions,
    ) -> Result<Story, Box<dyn std::error::Error>> {
        let story_repo = StoryRepository::new(self.pool.clone());
        let _scene_repo = SceneRepository::new(self.pool.clone());
        let _world_repo = WorldBuildingRepository::new(self.pool.clone());
        let _style_repo = WritingStyleRepository::new(self.pool.clone());
        let _char_repo = CharacterRepository::new(self.pool.clone());
        let _studio_repo = StudioConfigRepository::new(self.pool.clone());

        // 解压ZIP
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))?;

        // 读取manifest
        let manifest: ExportManifest = {
            let mut file = archive.by_name("manifest.json")?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            serde_json::from_str(&contents)?
        };

        // 检查故事是否已存在
        let existing_story = story_repo
            .get_all()?
            .into_iter()
            .find(|s| s.title == manifest.story_title);

        let story_id = if let Some(existing) = existing_story {
            if options.skip_existing {
                return Ok(existing);
            } else if options.merge_existing {
                existing.id
            } else {
                // 生成新ID（重命名）
                uuid::Uuid::new_v4().to_string()
            }
        } else {
            uuid::Uuid::new_v4().to_string()
        };

        // 读取并导入story
        let mut story: Story = {
            let mut file = archive.by_name("story.json")?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            serde_json::from_str(&contents)?
        };
        story.id = story_id.clone();

        // 创建故事记录（使用低级别SQL因为我们可能需要自定义ID）
        {
            let conn = self
                .pool
                .get()
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
            conn.execute(
                "INSERT OR REPLACE INTO stories (id, title, description, genre, tone, pacing, \
                 created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    &story.id,
                    &story.title,
                    story.description,
                    story.genre,
                    story.tone,
                    story.pacing,
                    story.created_at.to_rfc3339(),
                    story.updated_at.to_rfc3339()
                ],
            )?;
        }

        // 导入world_building
        if options.include_world_building {
            if let Ok(mut file) = archive.by_name("world_building.json") {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let mut wb: WorldBuilding = serde_json::from_str(&contents)?;
                wb.story_id = story_id.clone();
                wb.id = uuid::Uuid::new_v4().to_string();

                let conn = self
                    .pool
                    .get()
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                conn.execute(
                    "INSERT INTO world_buildings (id, story_id, concept, rules, history, \
                     cultures, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    rusqlite::params![
                        &wb.id,
                        &wb.story_id,
                        &wb.concept,
                        serde_json::to_string(&wb.rules)?,
                        wb.history,
                        serde_json::to_string(&wb.cultures)?,
                        wb.created_at.to_rfc3339(),
                        wb.updated_at.to_rfc3339()
                    ],
                )?;
            }
        }

        // 导入characters
        if options.include_characters {
            if let Ok(mut file) = archive.by_name("characters.json") {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let characters: Vec<Character> = serde_json::from_str(&contents)?;

                for mut char in characters {
                    char.story_id = story_id.clone();
                    char.id = uuid::Uuid::new_v4().to_string();

                    let conn = self
                        .pool
                        .get()
                        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                    conn.execute(
                        "INSERT INTO characters (id, story_id, name, background, personality, \
                         goals, dynamic_traits, created_at, updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                        rusqlite::params![
                            &char.id,
                            &char.story_id,
                            &char.name,
                            char.background,
                            char.personality,
                            char.goals,
                            serde_json::to_string(&char.dynamic_traits)?,
                            char.created_at.to_rfc3339(),
                            char.updated_at.to_rfc3339()
                        ],
                    )?;
                }
            }
        }

        // 导入writing_style
        if options.include_writing_style {
            if let Ok(mut file) = archive.by_name("writing_style.json") {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let mut ws: WritingStyle = serde_json::from_str(&contents)?;
                ws.story_id = story_id.clone();
                ws.id = uuid::Uuid::new_v4().to_string();

                let conn = self
                    .pool
                    .get()
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                conn.execute(
                    "INSERT INTO writing_styles (id, story_id, name, description, tone, pacing, \
                     vocabulary_level, sentence_structure, custom_rules, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                    rusqlite::params![
                        &ws.id,
                        &ws.story_id,
                        ws.name,
                        ws.description,
                        ws.tone,
                        ws.pacing,
                        ws.vocabulary_level,
                        ws.sentence_structure,
                        serde_json::to_string(&ws.custom_rules)?,
                        ws.created_at.to_rfc3339(),
                        ws.updated_at.to_rfc3339()
                    ],
                )?;
            }
        }

        // 导入scenes
        if options.include_scenes {
            if let Ok(mut file) = archive.by_name("scenes.json") {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let scenes: Vec<Scene> = serde_json::from_str(&contents)?;

                for mut scene in scenes {
                    scene.story_id = story_id.clone();
                    scene.id = uuid::Uuid::new_v4().to_string();
                    scene.previous_scene_id = None;
                    scene.next_scene_id = None;

                    let conn = self
                        .pool
                        .get()
                        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                    conn.execute(
                        "INSERT INTO scenes (id, story_id, sequence_number, title, dramatic_goal, \
                         external_pressure, conflict_type, characters_present, \
                         character_conflicts, setting_location, setting_time, setting_atmosphere, \
                         content, previous_scene_id, next_scene_id, model_used, cost, created_at, \
                         updated_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, \
                         ?16, ?17, ?18, ?19)",
                        rusqlite::params![
                            &scene.id,
                            &scene.story_id,
                            scene.sequence_number,
                            scene.title,
                            scene.dramatic_goal,
                            scene.external_pressure,
                            scene.conflict_type.as_ref().map(|c| c.to_string()),
                            serde_json::to_string(&scene.characters_present)?,
                            serde_json::to_string(&scene.character_conflicts)?,
                            scene.setting_location,
                            scene.setting_time,
                            scene.setting_atmosphere,
                            scene.content,
                            scene.previous_scene_id,
                            scene.next_scene_id,
                            scene.model_used,
                            scene.cost,
                            scene.created_at.to_rfc3339(),
                            scene.updated_at.to_rfc3339()
                        ],
                    )?;
                }
            }
        }

        // 导入studio_config
        if options.include_llm_config || options.include_ui_config || options.include_agent_bots {
            if let Ok(mut file) = archive.by_name("studio_config.json") {
                let mut contents = String::new();
                file.read_to_string(&mut contents)?;
                let mut sc: StudioConfig = serde_json::from_str(&contents)?;
                sc.story_id = story_id.clone();
                sc.id = uuid::Uuid::new_v4().to_string();

                let conn = self
                    .pool
                    .get()
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                conn.execute(
                    "INSERT INTO studio_configs (id, story_id, pen_name, llm_config, ui_config, \
                     agent_bots, frontstage_theme, backstage_theme, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                    rusqlite::params![
                        &sc.id,
                        &sc.story_id,
                        sc.pen_name,
                        serde_json::to_string(&sc.llm_config)?,
                        serde_json::to_string(&sc.ui_config)?,
                        serde_json::to_string(&sc.agent_bots)?,
                        sc.frontstage_theme,
                        sc.backstage_theme,
                        sc.created_at.to_rfc3339(),
                        sc.updated_at.to_rfc3339()
                    ],
                )?;
            }
        }

        Ok(story)
    }

    // ==================== 默认值 ====================

    fn default_llm_config() -> LlmStudioConfig {
        LlmStudioConfig {
            default_provider: "openai".to_string(),
            default_model: "gpt-4".to_string(),
            generation_temperature: 0.7,
            max_tokens: 4096,
            profiles: vec![LlmProfile {
                id: uuid::Uuid::new_v4().to_string(),
                name: "默认配置".to_string(),
                provider: "openai".to_string(),
                model: "gpt-4".to_string(),
                api_key: None,
                base_url: None,
                temperature: 0.7,
                max_tokens: 4096,
            }],
        }
    }

    fn default_ui_config() -> UiStudioConfig {
        UiStudioConfig {
            frontstage_font_size: 18,
            frontstage_font_family: "Noto Serif SC, Crimson Pro, serif".to_string(),
            frontstage_line_height: 1.8,
            frontstage_paper_color: "#f5f4ed".to_string(),
            frontstage_text_color: "#4d4c48".to_string(),
            backstage_theme: "dark".to_string(),
            backstage_accent_color: "#c9a227".to_string(),
        }
    }

    fn default_agent_bots() -> Vec<AgentBotConfig> {
        vec![
            AgentBotConfig {
                id: uuid::Uuid::new_v4().to_string(),
                name: "世界观助手".to_string(),
                agent_type: AgentBotType::WorldBuilding,
                enabled: true,
                llm_profile_id: "default".to_string(),
                system_prompt: "你是世界观助手，专门帮助构建和完善小说的世界观设定。".to_string(),
                custom_settings: serde_json::json!({}),
            },
            AgentBotConfig {
                id: uuid::Uuid::new_v4().to_string(),
                name: "人物助手".to_string(),
                agent_type: AgentBotType::Character,
                enabled: true,
                llm_profile_id: "default".to_string(),
                system_prompt: "你是人物助手，专门帮助塑造角色形象和性格发展。".to_string(),
                custom_settings: serde_json::json!({}),
            },
            AgentBotConfig {
                id: uuid::Uuid::new_v4().to_string(),
                name: "文风助手".to_string(),
                agent_type: AgentBotType::WritingStyle,
                enabled: true,
                llm_profile_id: "default".to_string(),
                system_prompt: "你是文风助手，专门帮助优化写作风格和语言表达。".to_string(),
                custom_settings: serde_json::json!({}),
            },
            AgentBotConfig {
                id: uuid::Uuid::new_v4().to_string(),
                name: "场景助手".to_string(),
                agent_type: AgentBotType::Scene,
                enabled: true,
                llm_profile_id: "default".to_string(),
                system_prompt: "你是场景助手，专门帮助设计戏剧性的场景和情节发展。".to_string(),
                custom_settings: serde_json::json!({}),
            },
        ]
    }

    fn default_frontstage_theme() -> String {
        r#"/* 草苔 - 幕前默认主题 */
:root {
    --parchment: #f5f4ed;
    --warm-sand: #e8e6dc;
    --terracotta: #c96442;
    --charcoal: #4d4c48;
    --stone-gray: #87867f;
    --soft-white: #faf9f4;
}

body {
    background-color: var(--parchment);
    color: var(--charcoal);
    font-family: "Noto Serif SC", "Crimson Pro", serif;
    line-height: 1.8;
}

.editor {
    background-color: var(--soft-white);
    border-radius: 8px;
    box-shadow: 0 1px 3px rgba(0,0,0,0.05);
}
"#
        .to_string()
    }

    fn default_backstage_theme() -> String {
        r#"/* 草苔 - 幕后默认主题 */
:root {
    --cinema-black: #0a0a0f;
    --cinema-900: #12121a;
    --cinema-800: #1e1e2e;
    --cinema-700: #2a2a3e;
    --cinema-600: #363650;
    --cinema-gold: #c9a227;
    --cinema-gold-light: #e8c547;
}

body {
    background-color: var(--cinema-black);
    color: #e0e0e0;
}

.sidebar {
    background-color: var(--cinema-900);
    border-right: 1px solid var(--cinema-700);
}
"#
        .to_string()
    }
}

/// 导入选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportOptions {
    pub include_world_building: bool,
    pub include_characters: bool,
    pub include_writing_style: bool,
    pub include_scenes: bool,
    pub include_llm_config: bool,
    pub include_ui_config: bool,
    pub include_agent_bots: bool,
    pub skip_existing: bool,
    pub merge_existing: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            include_world_building: true,
            include_characters: true,
            include_writing_style: true,
            include_scenes: true,
            include_llm_config: true,
            include_ui_config: true,
            include_agent_bots: true,
            skip_existing: false,
            merge_existing: false,
        }
    }
}
