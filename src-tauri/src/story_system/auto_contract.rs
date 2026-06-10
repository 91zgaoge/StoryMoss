//! Auto Contract Builder — 自动合同补齐
//!
//! 当预检发现缺少世界观合同、章节合同或场景大纲时，根据已有故事内容自动调用 LLM
//! 生成并保存。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::{
    db::{
        repositories::{SceneRepository, SceneUpdate},
        ChapterRepository, CharacterRepository, DbPool, StoryRepository, WorldBuildingRepository,
    },
    llm::LlmService,
};

/// 自动合同构建进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoContractProgress {
    pub stage: String, /* "analyzing" | "generating_master" | "generating_chapter" |
                        * "generating_outline" | "saving" | "completed" | "error" */
    pub message: String,
    pub progress: f32, // 0.0 - 1.0
}

/// 自动补齐结果
#[derive(Debug, Default)]
pub struct AutoFillResult {
    pub created_master: bool,
    pub created_chapter: bool,
    pub created_character: bool,
    pub created_scene: bool,
    pub created_outline: bool,
}

/// 自动合同构建器
pub struct AutoContractBuilder {
    pool: DbPool,
    app_handle: AppHandle,
}

impl AutoContractBuilder {
    pub fn new(pool: DbPool, app_handle: AppHandle) -> Self {
        Self { pool, app_handle }
    }

    fn emit_progress(&self, stage: &str, message: &str, progress: f32) {
        let _ = self.app_handle.emit(
            "contract-auto-progress",
            AutoContractProgress {
                stage: stage.to_string(),
                message: message.to_string(),
                progress,
            },
        );
    }

    /// 自动补齐指定故事缺失的合同、角色、场景及场景大纲
    ///
    /// 当预检发现阻塞性问题时，auto_fill 尝试自动创建所有缺失的基础数据，
    /// 包括默认角色、默认场景、世界观合同、章节合同和场景大纲。
    pub async fn auto_fill(
        &self,
        story_id: &str,
        chapter_number: i32,
        scene_id: Option<&str>,
    ) -> Result<AutoFillResult, String> {
        let engine = super::StorySystemEngine::new(self.pool.clone());
        let tree = engine.get_contract_tree(story_id)?;

        let mut result = AutoFillResult::default();

        // 1. 检查并补齐角色（角色是后续合同/大纲生成的重要上下文）
        let char_repo = CharacterRepository::new(self.pool.clone());
        let characters = char_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取角色失败: {}", e))?;
        if characters.is_empty() {
            self.emit_progress("analyzing", "正在分析故事内容，准备生成默认角色...", 0.05);
            match self.build_default_character(story_id).await {
                Ok(_character) => {
                    self.emit_progress("saving", "默认角色已生成并保存", 0.15);
                    result.created_character = true;
                    log::info!(
                        "[AutoContract] Created default character for story {}",
                        story_id
                    );
                }
                Err(e) => {
                    self.emit_progress("error", &format!("默认角色生成失败: {}", e), 0.0);
                    log::warn!("[AutoContract] Failed to create default character: {}", e);
                }
            }
        }

        // 2. 检查并补齐场景（场景存在后，大纲才能被填充）
        let scene_repo = SceneRepository::new(self.pool.clone());
        let scenes = scene_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取场景失败: {}", e))?;
        let target_scene_id = if let Some(sid) = scene_id {
            Some(sid.to_string())
        } else {
            scenes
                .into_iter()
                .find(|s| s.sequence_number == chapter_number)
                .map(|s| s.id)
        };
        let target_scene_id = if target_scene_id.is_none() {
            self.emit_progress("analyzing", "正在创建默认场景...", 0.18);
            match self.build_default_scene(story_id, chapter_number).await {
                Ok(scene) => {
                    self.emit_progress("saving", "默认场景已创建", 0.22);
                    result.created_scene = true;
                    log::info!(
                        "[AutoContract] Created default scene for story {} chapter {}",
                        story_id,
                        chapter_number
                    );
                    Some(scene.id)
                }
                Err(e) => {
                    self.emit_progress("error", &format!("默认场景创建失败: {}", e), 0.0);
                    log::warn!("[AutoContract] Failed to create default scene: {}", e);
                    None
                }
            }
        } else {
            target_scene_id
        };

        // 3. 检查并补齐 MASTER_SETTING
        if tree.master_setting.is_none() {
            self.emit_progress("analyzing", "正在分析故事内容，准备生成世界观合同...", 0.3);
            match self.build_master_setting(story_id).await {
                Ok(_contract) => {
                    self.emit_progress("saving", "世界观合同已生成并保存", 0.45);
                    result.created_master = true;
                    log::info!(
                        "[AutoContract] Created MASTER_SETTING for story {}",
                        story_id
                    );
                }
                Err(e) => {
                    self.emit_progress("error", &format!("世界观合同生成失败: {}", e), 0.0);
                    log::warn!("[AutoContract] Failed to create MASTER_SETTING: {}", e);
                }
            }
        }

        // 4. 检查并补齐 CHAPTER 合同
        let has_chapter_contract = tree.chapters.values().any(|c| {
            serde_json::from_str::<super::ChapterContract>(&c.contract_json)
                .map(|cc| cc.chapter_number == chapter_number)
                .unwrap_or(false)
        });

        if !has_chapter_contract {
            self.emit_progress("analyzing", "正在分析章节内容，准备生成章节合同...", 0.5);
            match self.build_chapter_contract(story_id, chapter_number).await {
                Ok(_contract) => {
                    self.emit_progress("saving", "章节合同已生成并保存", 0.65);
                    result.created_chapter = true;
                    log::info!(
                        "[AutoContract] Created CHAPTER contract for story {} chapter {}",
                        story_id,
                        chapter_number
                    );
                }
                Err(e) => {
                    self.emit_progress("error", &format!("章节合同生成失败: {}", e), 0.0);
                    log::warn!("[AutoContract] Failed to create CHAPTER contract: {}", e);
                }
            }
        }

        // 5. 检查并补齐 Scene 大纲
        if let Some(ref sid) = target_scene_id {
            let scene_repo = SceneRepository::new(self.pool.clone());
            if let Ok(Some(scene)) = scene_repo.get_by_id(sid) {
                let has_outline = scene
                    .outline_content
                    .as_ref()
                    .map(|o| !o.trim().is_empty())
                    .unwrap_or(false);
                if !has_outline {
                    self.emit_progress("analyzing", "正在分析场景信息，准备生成场景大纲...", 0.75);
                    match self.build_scene_outline(story_id, sid, &scene).await {
                        Ok(_) => {
                            self.emit_progress("saving", "场景大纲已生成并保存", 0.9);
                            result.created_outline = true;
                            log::info!(
                                "[AutoContract] Created scene outline for story {} scene {}",
                                story_id,
                                sid
                            );
                        }
                        Err(e) => {
                            self.emit_progress("error", &format!("场景大纲生成失败: {}", e), 0.0);
                            log::warn!("[AutoContract] Failed to create scene outline: {}", e);
                        }
                    }
                }
            }
        }

        self.emit_progress("completed", "补齐完成", 1.0);
        Ok(result)
    }

    /// 根据已有故事内容自动生成世界观合同
    async fn build_master_setting(
        &self,
        story_id: &str,
    ) -> Result<crate::db::StoryContract, String> {
        // 收集故事信息
        let story_repo = StoryRepository::new(self.pool.clone());
        let story = story_repo
            .get_by_id(story_id)
            .map_err(|e| format!("读取故事失败: {}", e))?
            .ok_or_else(|| "故事不存在".to_string())?;

        let character_repo = CharacterRepository::new(self.pool.clone());
        let characters = character_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取角色失败: {}", e))?;

        let chapter_repo = ChapterRepository::new(self.pool.clone());
        let chapters = chapter_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取章节失败: {}", e))?;

        let wb_repo = WorldBuildingRepository::new(self.pool.clone());
        let world_building = wb_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取世界构建失败: {}", e))?;

        // 构建 prompt
        let mut prompt = "根据以下故事信息，生成一个 MASTER_SETTING 世界观合同 JSON。\n\n".to_string();
        prompt.push_str(&format!("故事标题: {}\n", story.title));
        if let Some(ref genre) = story.genre {
            prompt.push_str(&format!("体裁: {}\n", genre));
        }
        if let Some(ref desc) = story.description {
            prompt.push_str(&format!("故事简介: {}\n", desc));
        }

        // 角色信息
        if !characters.is_empty() {
            prompt.push_str("\n已有角色:\n");
            for (i, c) in characters.iter().take(10).enumerate() {
                prompt.push_str(&format!(
                    "  {}. {} - 背景: {} 性格: {} 目标: {}\n",
                    i + 1,
                    c.name,
                    c.background.as_deref().unwrap_or("无"),
                    c.personality.as_deref().unwrap_or("无"),
                    c.goals.as_deref().unwrap_or("无")
                ));
            }
        }

        // 世界构建信息
        if let Some(ref wb) = world_building {
            if !wb.concept.is_empty() {
                prompt.push_str(&format!("\n世界观核心概念: {}\n", wb.concept));
            }
            if !wb.rules.is_empty() {
                prompt.push_str("世界规则:\n");
                for rule in wb.rules.iter().take(10) {
                    prompt.push_str(&format!(
                        "  - [{}] {} (重要性: {}/10): {}\n",
                        rule.rule_type,
                        rule.name,
                        rule.importance,
                        rule.description.as_deref().unwrap_or("")
                    ));
                }
            }
            if let Some(ref history) = wb.history {
                if !history.is_empty() {
                    prompt.push_str(&format!(
                        "\n历史背景: {}\n",
                        history.chars().take(500).collect::<String>()
                    ));
                }
            }
        }

        // 已有章节摘要
        if !chapters.is_empty() {
            prompt.push_str("\n已有章节摘要:\n");
            for ch in chapters.iter().take(5) {
                let summary = ch
                    .content
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(300)
                    .collect::<String>();
                prompt.push_str(&format!(
                    "  第{}章 {}: {}\n",
                    ch.chapter_number,
                    ch.title.as_deref().unwrap_or("无标题"),
                    if summary.is_empty() {
                        "（无内容）".to_string()
                    } else {
                        summary
                    }
                ));
            }
        }

        prompt.push_str("\n请严格输出以下格式的 JSON，不要包含任何其他文本:\n");
        prompt.push_str(
            r#"{
  "genre": "体裁名称",
  "core_tone": "核心基调描述（50字以内）",
  "pacing_strategy": "节奏策略描述（50字以内）",
  "anti_patterns": ["反模式1", "反模式2", "反模式3"],
  "world_rules": ["世界规则1", "世界规则2", "世界规则3", "世界规则4"]
}"#,
        );

        // 调用 LLM
        self.emit_progress("generating_master", "正在生成世界观合同...", 0.2);
        let llm_service = LlmService::new(self.app_handle.clone());
        let response = llm_service
            .generate(prompt, Some(2048), Some(0.7))
            .await
            .map_err(|e| format!("LLM 生成失败: {}", e))?;

        // 解析 JSON
        let json_str = crate::narrative::extract_and_sanitize_json(&response.content)
            .unwrap_or_else(|_| response.content.clone());

        let contract_data: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("解析 LLM 返回 JSON 失败: {}\n原始内容: {}", e, json_str))?;

        let genre = contract_data["genre"]
            .as_str()
            .unwrap_or("未知体裁")
            .to_string();
        let core_tone = contract_data["core_tone"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let pacing_strategy = contract_data["pacing_strategy"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let anti_patterns: Vec<String> = contract_data["anti_patterns"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let world_rules: Vec<String> = contract_data["world_rules"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // 保存合同
        let engine = super::StorySystemEngine::new(self.pool.clone());
        engine.create_master_setting(
            story_id,
            &genre,
            &core_tone,
            &pacing_strategy,
            &anti_patterns,
            &world_rules,
        )
    }

    /// 根据已有内容自动生成章节合同
    async fn build_chapter_contract(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<crate::db::StoryContract, String> {
        // 收集信息
        let story_repo = StoryRepository::new(self.pool.clone());
        let story = story_repo
            .get_by_id(story_id)
            .map_err(|e| format!("读取故事失败: {}", e))?
            .ok_or_else(|| "故事不存在".to_string())?;

        let chapter_repo = ChapterRepository::new(self.pool.clone());
        let chapters = chapter_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取章节失败: {}", e))?;

        let current_chapter = chapters
            .iter()
            .find(|c| c.chapter_number == chapter_number)
            .cloned();

        let prev_chapter = chapters
            .iter()
            .find(|c| c.chapter_number == chapter_number - 1)
            .cloned();

        let next_chapter = chapters
            .iter()
            .find(|c| c.chapter_number == chapter_number + 1)
            .cloned();

        // 读取世界观合同
        let engine = super::StorySystemEngine::new(self.pool.clone());
        let tree = engine.get_contract_tree(story_id)?;
        let master_summary = tree
            .master_setting
            .map(|c| {
                serde_json::from_str::<super::MasterSettingContract>(&c.contract_json)
                    .map(|m| {
                        format!(
                            "体裁: {}, 基调: {}, 节奏: {}",
                            m.genre, m.core_tone, m.pacing_strategy
                        )
                    })
                    .unwrap_or_else(|_| c.contract_json.chars().take(500).collect())
            })
            .unwrap_or_default();

        // 构建 prompt
        let mut prompt = "根据以下信息，生成一个 CHAPTER 章节合同 JSON。\n\n".to_string();
        prompt.push_str(&format!("故事标题: {}\n", story.title));
        if let Some(ref genre) = story.genre {
            prompt.push_str(&format!("体裁: {}\n", genre));
        }
        prompt.push_str(&format!("世界观概要: {}\n\n", master_summary));

        // 前一章信息
        if let Some(ref prev) = prev_chapter {
            let content = prev
                .content
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(500)
                .collect::<String>();
            prompt.push_str(&format!(
                "前一章（第{}章 {}）内容摘要: {}\n\n",
                prev.chapter_number,
                prev.title.as_deref().unwrap_or("无标题"),
                content
            ));
        }

        // 当前章信息
        if let Some(ref current) = current_chapter {
            let content = current
                .content
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(1000)
                .collect::<String>();
            prompt.push_str(&format!(
                "当前章节（第{}章 {}）内容: {}\n\n",
                current.chapter_number,
                current.title.as_deref().unwrap_or("无标题"),
                content
            ));
        } else {
            prompt.push_str(&format!(
                "当前章节: 第{}章（尚未创建内容）\n\n",
                chapter_number
            ));
        }

        // 后一章信息
        if let Some(ref next) = next_chapter {
            let content = next
                .content
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(300)
                .collect::<String>();
            prompt.push_str(&format!(
                "后一章（第{}章 {}）内容摘要: {}\n\n",
                next.chapter_number,
                next.title.as_deref().unwrap_or("无标题"),
                content
            ));
        }

        prompt.push_str("请严格输出以下格式的 JSON，不要包含任何其他文本:\n");
        prompt.push_str(
            r#"{
  "goal": "本章叙事目标（100字以内）",
  "must_cover_nodes": ["必须覆盖的节点1", "必须覆盖的节点2"],
  "forbidden_zones": ["禁区1", "禁区2"],
  "time_anchor": "时间锚点（可选，如'三天后'、'黄昏'）",
  "chapter_span": "章节预计跨度（可选，如'单一场景'、'三天'）"
}"#,
        );

        // 调用 LLM
        self.emit_progress("generating_chapter", "正在生成章节合同...", 0.6);
        let llm_service = LlmService::new(self.app_handle.clone());
        let response = llm_service
            .generate(prompt, Some(2048), Some(0.7))
            .await
            .map_err(|e| format!("LLM 生成失败: {}", e))?;

        // 解析 JSON
        let json_str = crate::narrative::extract_and_sanitize_json(&response.content)
            .unwrap_or_else(|_| response.content.clone());

        let contract_data: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("解析 LLM 返回 JSON 失败: {}\n原始内容: {}", e, json_str))?;

        let goal = contract_data["goal"]
            .as_str()
            .unwrap_or("推进剧情")
            .to_string();
        let must_cover_nodes: Vec<String> = contract_data["must_cover_nodes"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let forbidden_zones: Vec<String> = contract_data["forbidden_zones"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let time_anchor = contract_data["time_anchor"].as_str().map(|s| s.to_string());
        let chapter_span = contract_data["chapter_span"]
            .as_str()
            .map(|s| s.to_string());

        // 保存合同
        engine.create_chapter_contract(
            story_id,
            chapter_number,
            &goal,
            &must_cover_nodes,
            &forbidden_zones,
            time_anchor.as_deref(),
            chapter_span.as_deref(),
        )
    }

    /// 根据已有内容自动生成场景大纲
    async fn build_scene_outline(
        &self,
        story_id: &str,
        scene_id: &str,
        scene: &crate::db::models::Scene,
    ) -> Result<(), String> {
        let story_repo = StoryRepository::new(self.pool.clone());
        let story = story_repo
            .get_by_id(story_id)
            .map_err(|e| format!("读取故事失败: {}", e))?
            .ok_or_else(|| "故事不存在".to_string())?;

        let character_repo = CharacterRepository::new(self.pool.clone());
        let characters = character_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取角色失败: {}", e))?;

        let chapter_repo = ChapterRepository::new(self.pool.clone());
        let chapters = chapter_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取章节失败: {}", e))?;

        let wb_repo = WorldBuildingRepository::new(self.pool.clone());
        let world_building = wb_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取世界构建失败: {}", e))?;

        // 读取世界观合同
        let engine = super::StorySystemEngine::new(self.pool.clone());
        let tree = engine.get_contract_tree(story_id)?;
        let master_summary = tree
            .master_setting
            .map(|c| {
                serde_json::from_str::<super::MasterSettingContract>(&c.contract_json)
                    .map(|m| {
                        format!(
                            "体裁: {}, 基调: {}, 节奏: {}",
                            m.genre, m.core_tone, m.pacing_strategy
                        )
                    })
                    .unwrap_or_else(|_| c.contract_json.chars().take(500).collect())
            })
            .unwrap_or_default();

        // 读取前后场景内容
        let scene_repo = SceneRepository::new(self.pool.clone());
        let all_scenes = scene_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取场景失败: {}", e))?;

        let prev_scene = scene
            .previous_scene_id
            .as_ref()
            .and_then(|pid| all_scenes.iter().find(|s| s.id == *pid));
        let next_scene = scene
            .next_scene_id
            .as_ref()
            .and_then(|nid| all_scenes.iter().find(|s| s.id == *nid));

        // 当前章节内容
        let current_chapter = chapters
            .iter()
            .find(|c| c.chapter_number == scene.sequence_number)
            .cloned();

        // 构建 prompt
        let mut prompt =
            "根据以下故事和场景信息，生成一段详细的场景大纲（200-400字）。\n\n".to_string();
        prompt.push_str(&format!("故事标题: {}\n", story.title));
        if let Some(ref genre) = story.genre {
            prompt.push_str(&format!("体裁: {}\n", genre));
        }
        prompt.push_str(&format!("世界观概要: {}\n\n", master_summary));

        // 场景信息
        prompt.push_str("当前场景信息:\n");
        prompt.push_str(&format!(
            "  标题: {}\n",
            scene.title.as_deref().unwrap_or("未命名")
        ));
        if let Some(ref goal) = scene.dramatic_goal {
            prompt.push_str(&format!("  戏剧目标: {}\n", goal));
        }
        if let Some(ref pressure) = scene.external_pressure {
            prompt.push_str(&format!("  外部压迫: {}\n", pressure));
        }
        if let Some(ref conflict) = scene.conflict_type {
            prompt.push_str(&format!("  冲突类型: {:?}\n", conflict));
        }
        if let Some(ref location) = scene.setting_location {
            prompt.push_str(&format!("  地点: {}\n", location));
        }
        if let Some(ref time) = scene.setting_time {
            prompt.push_str(&format!("  时间: {}\n", time));
        }
        if let Some(ref atmosphere) = scene.setting_atmosphere {
            prompt.push_str(&format!("  氛围: {}\n", atmosphere));
        }
        if !scene.characters_present.is_empty() {
            prompt.push_str(&format!(
                "  出场角色: {}\n",
                scene.characters_present.join(", ")
            ));
        }

        // 角色详细信息
        if !scene.characters_present.is_empty() {
            prompt.push_str("\n出场角色详情:\n");
            for name in scene.characters_present.iter() {
                if let Some(c) = characters.iter().find(|c| c.name == *name) {
                    prompt.push_str(&format!(
                        "  {} - 背景: {} 性格: {} 目标: {}\n",
                        c.name,
                        c.background.as_deref().unwrap_or("无"),
                        c.personality.as_deref().unwrap_or("无"),
                        c.goals.as_deref().unwrap_or("无")
                    ));
                }
            }
        }

        // 世界构建信息
        if let Some(ref wb) = world_building {
            if !wb.concept.is_empty() {
                prompt.push_str(&format!("\n世界观核心概念: {}\n", wb.concept));
            }
            if !wb.rules.is_empty() {
                let relevant_rules: Vec<_> = wb
                    .rules
                    .iter()
                    .filter(|r| {
                        let desc = r.description.as_deref().unwrap_or("");
                        scene
                            .setting_location
                            .as_ref()
                            .map(|loc| desc.contains(loc))
                            .unwrap_or(false)
                            || scene
                                .characters_present
                                .iter()
                                .any(|name| r.name.contains(name))
                    })
                    .take(5)
                    .collect();
                if !relevant_rules.is_empty() {
                    prompt.push_str("相关世界规则:\n");
                    for rule in relevant_rules {
                        prompt.push_str(&format!(
                            "  - [{}] {}: {}\n",
                            rule.rule_type,
                            rule.name,
                            rule.description.as_deref().unwrap_or("")
                        ));
                    }
                }
            }
        }

        // 前一场景摘要
        if let Some(prev) = prev_scene {
            let content = prev
                .content
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(300)
                .collect::<String>();
            if !content.is_empty() {
                prompt.push_str(&format!("\n前一场景内容摘要: {}\n", content));
            }
        }

        // 当前章节内容
        if let Some(ref ch) = current_chapter {
            let content = ch
                .content
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(500)
                .collect::<String>();
            if !content.is_empty() {
                prompt.push_str(&format!("\n当前章节已有内容: {}\n", content));
            }
        }

        // 后一场景摘要
        if let Some(next) = next_scene {
            let content = next
                .content
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect::<String>();
            if !content.is_empty() {
                prompt.push_str(&format!("\n后一场景内容摘要: {}\n", content));
            }
        }

        prompt.push_str("\n请生成一段连贯、具体的场景大纲，包含：\n");
        prompt.push_str("1. 场景开场：角色在做什么，处于什么状态\n");
        prompt.push_str("2. 冲突触发：什么事件打破了平衡\n");
        prompt.push_str("3. 冲突升级：角色如何应对，局势如何变化\n");
        prompt.push_str("4. 场景收束：以什么状态结束，留下什么悬念\n");
        prompt.push_str("\n直接输出大纲文本，不要包含 JSON 格式或其他标记。200-400字。");

        // 调用 LLM
        self.emit_progress("generating_outline", "正在生成场景大纲...", 0.7);
        let llm_service = LlmService::new(self.app_handle.clone());
        let response = llm_service
            .generate(prompt, Some(1024), Some(0.7))
            .await
            .map_err(|e| format!("LLM 生成失败: {}", e))?;

        let outline = response.content.trim().to_string();
        if outline.is_empty() {
            return Err("LLM 返回了大纲为空".to_string());
        }

        // 保存大纲
        let scene_repo = SceneRepository::new(self.pool.clone());
        scene_repo
            .update(
                scene_id,
                &SceneUpdate {
                    outline_content: Some(outline),
                    execution_stage: Some("outline".to_string()),
                    ..Default::default()
                },
            )
            .map_err(|e| format!("保存大纲失败: {}", e))?;

        Ok(())
    }

    /// 基于故事概念生成一个默认主角
    async fn build_default_character(
        &self,
        story_id: &str,
    ) -> Result<crate::db::models::Character, String> {
        let story_repo = StoryRepository::new(self.pool.clone());
        let story = story_repo
            .get_by_id(story_id)
            .map_err(|e| format!("读取故事失败: {}", e))?
            .ok_or_else(|| "故事不存在".to_string())?;

        let prompt = format!(
            "根据以下故事信息，生成一个主角的基本设定，以 JSON 格式输出。\n\n\
             故事标题: {}\n\
             体裁: {}\n\
             简介: {}\n\n\
             请输出以下格式的 JSON：\n\
             {{\n  \"name\": \"角色姓名\",\n  \"background\": \"背景故事（50字以内）\",\n  \"personality\": \"性格特点（50字以内）\",\n  \"goals\": \"目标动机（50字以内）\"\n}}",
            story.title,
            story.genre.as_deref().unwrap_or("未知"),
            story.description.as_deref().unwrap_or("暂无简介")
        );

        self.emit_progress("generating_character", "正在生成默认角色...", 0.08);
        let llm_service = LlmService::new(self.app_handle.clone());
        let response = llm_service
            .generate(prompt, Some(1024), Some(0.7))
            .await
            .map_err(|e| format!("LLM 生成失败: {}", e))?;

        let json_str = crate::narrative::extract_and_sanitize_json(&response.content)
            .unwrap_or_else(|_| response.content.clone());

        let char_data: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("解析角色 JSON 失败: {}\n原始内容: {}", e, json_str))?;

        let name = char_data["name"].as_str().unwrap_or("主角").to_string();
        let background = char_data["background"].as_str().map(|s| s.to_string());
        let personality = char_data["personality"].as_str().map(|s| s.to_string());
        let goals = char_data["goals"].as_str().map(|s| s.to_string());

        let char_repo = CharacterRepository::new(self.pool.clone());
        let character = char_repo
            .create(crate::db::CreateCharacterRequest {
                story_id: story_id.to_string(),
                name,
                background,
                personality,
                goals,
                appearance: None,
                gender: None,
                age: None,
            })
            .map_err(|e| format!("保存角色失败: {}", e))?;

        Ok(character)
    }

    /// 创建一个默认场景（占位符，供后续大纲生成使用）
    async fn build_default_scene(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Result<crate::db::models::Scene, String> {
        let scene_repo = SceneRepository::new(self.pool.clone());
        let scene = scene_repo
            .create(
                story_id,
                chapter_number,
                Some(&format!("第{}章", chapter_number)),
            )
            .map_err(|e| format!("创建场景失败: {}", e))?;

        // 将已有角色加入场景出场角色列表，增强后续大纲生成的上下文
        let char_repo = CharacterRepository::new(self.pool.clone());
        if let Ok(characters) = char_repo.get_by_story(story_id) {
            if !characters.is_empty() {
                let character_names: Vec<String> =
                    characters.iter().map(|c| c.name.clone()).collect();
                let _ = scene_repo.update(
                    &scene.id,
                    &SceneUpdate {
                        characters_present: Some(character_names),
                        ..Default::default()
                    },
                );
            }
        }

        Ok(scene)
    }
}
