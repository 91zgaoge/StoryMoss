//! Auto Contract Builder — 自动合同补齐
//!
//! 当预检发现缺少世界观合同、章节合同或场景大纲时，根据已有故事内容自动调用 LLM 生成并保存。

use crate::db::{DbPool, StoryRepository, CharacterRepository, ChapterRepository, WorldBuildingRepository};
use crate::db::repositories_v3::{SceneRepository, SceneUpdate};
use crate::llm::LlmService;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// 自动合同构建进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoContractProgress {
    pub stage: String, // "analyzing" | "generating_master" | "generating_chapter" | "generating_outline" | "saving" | "completed" | "error"
    pub message: String,
    pub progress: f32, // 0.0 - 1.0
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
        let _ = self.app_handle.emit("contract-auto-progress", AutoContractProgress {
            stage: stage.to_string(),
            message: message.to_string(),
            progress,
        });
    }

    /// 自动补齐指定故事缺失的合同及场景大纲
    ///
    /// 返回 (是否创建了 master_setting, 是否创建了 chapter_contract, 是否创建了 scene_outline)
    pub async fn auto_fill(
        &self,
        story_id: &str,
        chapter_number: i32,
        scene_id: Option<&str>,
    ) -> Result<(bool, bool, bool), String> {
        let engine = super::StorySystemEngine::new(self.pool.clone());
        let tree = engine.get_contract_tree(story_id)?;

        let mut created_master = false;
        let mut created_chapter = false;
        let mut created_outline = false;

        // 1. 检查并补齐 MASTER_SETTING
        if tree.master_setting.is_none() {
            self.emit_progress("analyzing", "正在分析故事内容，准备生成世界观合同...", 0.1);
            match self.build_master_setting(story_id).await {
                Ok(_contract) => {
                    self.emit_progress("saving", "世界观合同已生成并保存", 0.25);
                    created_master = true;
                    log::info!("[AutoContract] Created MASTER_SETTING for story {}", story_id);
                }
                Err(e) => {
                    self.emit_progress("error", &format!("世界观合同生成失败: {}", e), 0.0);
                    log::warn!("[AutoContract] Failed to create MASTER_SETTING: {}", e);
                }
            }
        }

        // 2. 检查并补齐 CHAPTER 合同
        let has_chapter_contract = tree.chapters.values().any(|c| {
            serde_json::from_str::<super::ChapterContract>(&c.contract_json)
                .map(|cc| cc.chapter_number == chapter_number)
                .unwrap_or(false)
        });

        if !has_chapter_contract {
            self.emit_progress("analyzing", "正在分析章节内容，准备生成章节合同...", 0.3);
            match self.build_chapter_contract(story_id, chapter_number).await {
                Ok(_contract) => {
                    self.emit_progress("saving", "章节合同已生成并保存", 0.5);
                    created_chapter = true;
                    log::info!("[AutoContract] Created CHAPTER contract for story {} chapter {}", story_id, chapter_number);
                }
                Err(e) => {
                    self.emit_progress("error", &format!("章节合同生成失败: {}", e), 0.0);
                    log::warn!("[AutoContract] Failed to create CHAPTER contract: {}", e);
                }
            }
        }

        // 3. 检查并补齐 Scene 大纲
        if let Some(sid) = scene_id {
            let scene_repo = SceneRepository::new(self.pool.clone());
            if let Ok(Some(scene)) = scene_repo.get_by_id(sid) {
                let has_outline = scene.outline_content.as_ref().map(|o| !o.trim().is_empty()).unwrap_or(false);
                if !has_outline {
                    self.emit_progress("analyzing", "正在分析场景信息，准备生成场景大纲...", 0.6);
                    match self.build_scene_outline(story_id, sid, &scene).await {
                        Ok(_) => {
                            self.emit_progress("saving", "场景大纲已生成并保存", 0.85);
                            created_outline = true;
                            log::info!("[AutoContract] Created scene outline for story {} scene {}", story_id, sid);
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
        Ok((created_master, created_chapter, created_outline))
    }

    /// 根据已有故事内容自动生成世界观合同
    async fn build_master_setting(&self, story_id: &str) -> Result<crate::db::StoryContract, String> {
        // 收集故事信息
        let story_repo = StoryRepository::new(self.pool.clone());
        let story = story_repo.get_by_id(story_id)
            .map_err(|e| format!("读取故事失败: {}", e))?
            .ok_or_else(|| "故事不存在".to_string())?;

        let character_repo = CharacterRepository::new(self.pool.clone());
        let characters = character_repo.get_by_story(story_id)
            .map_err(|e| format!("读取角色失败: {}", e))?;

        let chapter_repo = ChapterRepository::new(self.pool.clone());
        let chapters = chapter_repo.get_by_story(story_id)
            .map_err(|e| format!("读取章节失败: {}", e))?;

        let wb_repo = WorldBuildingRepository::new(self.pool.clone());
        let world_building = wb_repo.get_by_story(story_id)
            .map_err(|e| format!("读取世界构建失败: {}", e))?;

        // 构建 prompt
        let mut prompt = format!(
            "根据以下故事信息，生成一个 MASTER_SETTING 世界观合同 JSON。\n\n"
        );
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
                    prompt.push_str(&format!("\n历史背景: {}\n", history.chars().take(500).collect::<String>()));
                }
            }
        }

        // 已有章节摘要
        if !chapters.is_empty() {
            prompt.push_str("\n已有章节摘要:\n");
            for ch in chapters.iter().take(5) {
                let summary = ch.content.as_deref().unwrap_or("").chars().take(300).collect::<String>();
                prompt.push_str(&format!(
                    "  第{}章 {}: {}\n",
                    ch.chapter_number,
                    ch.title.as_deref().unwrap_or("无标题"),
                    if summary.is_empty() { "（无内容）".to_string() } else { summary }
                ));
            }
        }

        prompt.push_str("\n请严格输出以下格式的 JSON，不要包含任何其他文本:\n");
        prompt.push_str(r#"{
  "genre": "体裁名称",
  "core_tone": "核心基调描述（50字以内）",
  "pacing_strategy": "节奏策略描述（50字以内）",
  "anti_patterns": ["反模式1", "反模式2", "反模式3"],
  "world_rules": ["世界规则1", "世界规则2", "世界规则3", "世界规则4"]
}"#);

        // 调用 LLM
        self.emit_progress("generating_master", "正在生成世界观合同...", 0.2);
        let llm_service = LlmService::new(self.app_handle.clone());
        let response = llm_service.generate(prompt, Some(2048), Some(0.7)).await
            .map_err(|e| format!("LLM 生成失败: {}", e))?;

        // 解析 JSON
        let json_str = crate::narrative::extract_and_sanitize_json(&response.content)
            .unwrap_or_else(|_| response.content.clone());

        let contract_data: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("解析 LLM 返回 JSON 失败: {}\n原始内容: {}", e, json_str))?;

        let genre = contract_data["genre"].as_str().unwrap_or("未知体裁").to_string();
        let core_tone = contract_data["core_tone"].as_str().unwrap_or("").to_string();
        let pacing_strategy = contract_data["pacing_strategy"].as_str().unwrap_or("").to_string();
        let anti_patterns: Vec<String> = contract_data["anti_patterns"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let world_rules: Vec<String> = contract_data["world_rules"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
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
    async fn build_chapter_contract(&self, story_id: &str, chapter_number: i32) -> Result<crate::db::StoryContract, String> {
        // 收集信息
        let story_repo = StoryRepository::new(self.pool.clone());
        let story = story_repo.get_by_id(story_id)
            .map_err(|e| format!("读取故事失败: {}", e))?
            .ok_or_else(|| "故事不存在".to_string())?;

        let chapter_repo = ChapterRepository::new(self.pool.clone());
        let chapters = chapter_repo.get_by_story(story_id)
            .map_err(|e| format!("读取章节失败: {}", e))?;

        let current_chapter = chapters.iter()
            .find(|c| c.chapter_number == chapter_number)
            .cloned();

        let prev_chapter = chapters.iter()
            .find(|c| c.chapter_number == chapter_number - 1)
            .cloned();

        let next_chapter = chapters.iter()
            .find(|c| c.chapter_number == chapter_number + 1)
            .cloned();

        // 读取世界观合同
        let engine = super::StorySystemEngine::new(self.pool.clone());
        let tree = engine.get_contract_tree(story_id)?;
        let master_summary = tree.master_setting
            .map(|c| {
                serde_json::from_str::<super::MasterSettingContract>(&c.contract_json)
                    .map(|m| format!("体裁: {}, 基调: {}, 节奏: {}", m.genre, m.core_tone, m.pacing_strategy))
                    .unwrap_or_else(|_| c.contract_json.chars().take(500).collect())
            })
            .unwrap_or_default();

        // 构建 prompt
        let mut prompt = format!(
            "根据以下信息，生成一个 CHAPTER 章节合同 JSON。\n\n"
        );
        prompt.push_str(&format!("故事标题: {}\n", story.title));
        if let Some(ref genre) = story.genre {
            prompt.push_str(&format!("体裁: {}\n", genre));
        }
        prompt.push_str(&format!("世界观概要: {}\n\n", master_summary));

        // 前一章信息
        if let Some(ref prev) = prev_chapter {
            let content = prev.content.as_deref().unwrap_or("").chars().take(500).collect::<String>();
            prompt.push_str(&format!(
                "前一章（第{}章 {}）内容摘要: {}\n\n",
                prev.chapter_number,
                prev.title.as_deref().unwrap_or("无标题"),
                content
            ));
        }

        // 当前章信息
        if let Some(ref current) = current_chapter {
            let content = current.content.as_deref().unwrap_or("").chars().take(1000).collect::<String>();
            prompt.push_str(&format!(
                "当前章节（第{}章 {}）内容: {}\n\n",
                current.chapter_number,
                current.title.as_deref().unwrap_or("无标题"),
                content
            ));
        } else {
            prompt.push_str(&format!("当前章节: 第{}章（尚未创建内容）\n\n", chapter_number));
        }

        // 后一章信息
        if let Some(ref next) = next_chapter {
            let content = next.content.as_deref().unwrap_or("").chars().take(300).collect::<String>();
            prompt.push_str(&format!(
                "后一章（第{}章 {}）内容摘要: {}\n\n",
                next.chapter_number,
                next.title.as_deref().unwrap_or("无标题"),
                content
            ));
        }

        prompt.push_str("请严格输出以下格式的 JSON，不要包含任何其他文本:\n");
        prompt.push_str(r#"{
  "goal": "本章叙事目标（100字以内）",
  "must_cover_nodes": ["必须覆盖的节点1", "必须覆盖的节点2"],
  "forbidden_zones": ["禁区1", "禁区2"],
  "time_anchor": "时间锚点（可选，如'三天后'、'黄昏'）",
  "chapter_span": "章节预计跨度（可选，如'单一场景'、'三天'）"
}"#);

        // 调用 LLM
        self.emit_progress("generating_chapter", "正在生成章节合同...", 0.6);
        let llm_service = LlmService::new(self.app_handle.clone());
        let response = llm_service.generate(prompt, Some(2048), Some(0.7)).await
            .map_err(|e| format!("LLM 生成失败: {}", e))?;

        // 解析 JSON
        let json_str = crate::narrative::extract_and_sanitize_json(&response.content)
            .unwrap_or_else(|_| response.content.clone());

        let contract_data: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("解析 LLM 返回 JSON 失败: {}\n原始内容: {}", e, json_str))?;

        let goal = contract_data["goal"].as_str().unwrap_or("推进剧情").to_string();
        let must_cover_nodes: Vec<String> = contract_data["must_cover_nodes"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let forbidden_zones: Vec<String> = contract_data["forbidden_zones"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let time_anchor = contract_data["time_anchor"].as_str().map(|s| s.to_string());
        let chapter_span = contract_data["chapter_span"].as_str().map(|s| s.to_string());

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
        scene: &crate::db::models_v3::Scene,
    ) -> Result<(), String> {
        let story_repo = StoryRepository::new(self.pool.clone());
        let story = story_repo.get_by_id(story_id)
            .map_err(|e| format!("读取故事失败: {}", e))?
            .ok_or_else(|| "故事不存在".to_string())?;

        let character_repo = CharacterRepository::new(self.pool.clone());
        let characters = character_repo.get_by_story(story_id)
            .map_err(|e| format!("读取角色失败: {}", e))?;

        let chapter_repo = ChapterRepository::new(self.pool.clone());
        let chapters = chapter_repo.get_by_story(story_id)
            .map_err(|e| format!("读取章节失败: {}", e))?;

        let wb_repo = WorldBuildingRepository::new(self.pool.clone());
        let world_building = wb_repo.get_by_story(story_id)
            .map_err(|e| format!("读取世界构建失败: {}", e))?;

        // 读取世界观合同
        let engine = super::StorySystemEngine::new(self.pool.clone());
        let tree = engine.get_contract_tree(story_id)?;
        let master_summary = tree.master_setting
            .map(|c| {
                serde_json::from_str::<super::MasterSettingContract>(&c.contract_json)
                    .map(|m| format!("体裁: {}, 基调: {}, 节奏: {}", m.genre, m.core_tone, m.pacing_strategy))
                    .unwrap_or_else(|_| c.contract_json.chars().take(500).collect())
            })
            .unwrap_or_default();

        // 读取前后场景内容
        let scene_repo = SceneRepository::new(self.pool.clone());
        let all_scenes = scene_repo.get_by_story(story_id)
            .map_err(|e| format!("读取场景失败: {}", e))?;

        let prev_scene = scene.previous_scene_id.as_ref()
            .and_then(|pid| all_scenes.iter().find(|s| s.id == *pid));
        let next_scene = scene.next_scene_id.as_ref()
            .and_then(|nid| all_scenes.iter().find(|s| s.id == *nid));

        // 当前章节内容
        let current_chapter = chapters.iter()
            .find(|c| c.chapter_number == scene.sequence_number)
            .cloned();

        // 构建 prompt
        let mut prompt = format!(
            "根据以下故事和场景信息，生成一段详细的场景大纲（200-400字）。\n\n"
        );
        prompt.push_str(&format!("故事标题: {}\n", story.title));
        if let Some(ref genre) = story.genre {
            prompt.push_str(&format!("体裁: {}\n", genre));
        }
        prompt.push_str(&format!("世界观概要: {}\n\n", master_summary));

        // 场景信息
        prompt.push_str("当前场景信息:\n");
        prompt.push_str(&format!("  标题: {}\n", scene.title.as_deref().unwrap_or("未命名")));
        if let Some(ref goal) = scene.dramatic_goal {
            prompt.push_str(&format!("  戏剧目标: {}\n", goal));
        }
        if let Some(ref pressure) = scene.external_pressure {
            prompt.push_str(&format!("  外部压迫: {}\n", pressure));
        }
        if let Some(ref conflict) = scene.conflict_type {
            prompt.push_str(&format!("  冲突类型: {}\n", format!("{:?}", conflict)));
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
            prompt.push_str(&format!("  出场角色: {}\n", scene.characters_present.join(", ")));
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
                let relevant_rules: Vec<_> = wb.rules.iter()
                    .filter(|r| {
                        let desc = r.description.as_deref().unwrap_or("");
                        scene.setting_location.as_ref().map(|loc| desc.contains(loc)).unwrap_or(false)
                            || scene.characters_present.iter().any(|name| r.name.contains(name))
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
        if let Some(ref prev) = prev_scene {
            let content = prev.content.as_deref().unwrap_or("").chars().take(300).collect::<String>();
            if !content.is_empty() {
                prompt.push_str(&format!("\n前一场景内容摘要: {}\n", content));
            }
        }

        // 当前章节内容
        if let Some(ref ch) = current_chapter {
            let content = ch.content.as_deref().unwrap_or("").chars().take(500).collect::<String>();
            if !content.is_empty() {
                prompt.push_str(&format!("\n当前章节已有内容: {}\n", content));
            }
        }

        // 后一场景摘要
        if let Some(ref next) = next_scene {
            let content = next.content.as_deref().unwrap_or("").chars().take(200).collect::<String>();
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
        let response = llm_service.generate(prompt, Some(1024), Some(0.7)).await
            .map_err(|e| format!("LLM 生成失败: {}", e))?;

        let outline = response.content.trim().to_string();
        if outline.is_empty() {
            return Err("LLM 返回了大纲为空".to_string());
        }

        // 保存大纲
        let scene_repo = SceneRepository::new(self.pool.clone());
        scene_repo.update(scene_id, &SceneUpdate {
            outline_content: Some(outline),
            execution_stage: Some("outline".to_string()),
            ..Default::default()
        }).map_err(|e| format!("保存大纲失败: {}", e))?;

        Ok(())
    }
}
