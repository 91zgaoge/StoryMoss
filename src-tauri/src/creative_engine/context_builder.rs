#![allow(dead_code)]
//! StoryContextBuilder - 创作上下文构建器
//!
//! 从数据库中读取真实故事数据，为 Agent 提供完整的创作上下文。
//! 解决 intent.rs 中硬编码 "未命名作品"/"小说"/"中性" 的问题。

use crate::{
    agents::{
        AgentContext, AgentMemoryContext, ChapterSummary, CharacterInfo, NarrativeContext,
        NarrativeStructureContext, StoryContext, StyleContext, WorldContext,
    },
    db::{
        repositories::{
            CharacterRepository, SceneRepository, StoryRepository, WorldBuildingRepository,
            WritingStyleRepository,
        },
        Character, DbPool, Story,
    },
    error::AppError,
};

/// 知识图谱实体摘要（用于注入提示词）
#[derive(Debug, Clone)]
pub struct RelevantEntity {
    pub name: String,
    pub entity_type: String,
    pub description: String,
    pub relation_hint: Option<String>,
}

/// 世界观规则摘要
#[derive(Debug, Clone)]
pub struct WorldRuleSummary {
    pub name: String,
    pub description: String,
    pub rule_type: String,
    pub importance: i32,
}

/// 场景结构信息
#[derive(Debug, Clone)]
pub struct SceneStructure {
    pub scene_id: String,
    pub sequence_number: i32,
    pub title: Option<String>,
    pub dramatic_goal: Option<String>,
    pub external_pressure: Option<String>,
    pub conflict_type: Option<String>,
    pub setting_location: Option<String>,
    pub setting_time: Option<String>,
    pub characters_present: Vec<String>,
}

/// 上下文构建器
pub struct StoryContextBuilder {
    pool: DbPool,
}

impl StoryContextBuilder {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 构建完整的 Agent 上下文
    ///
    /// # Arguments
    /// * `story_id` - 故事 ID
    /// * `scene_number` - 当前场景序号（用于获取前文和当前场景结构）
    /// * `current_content` - 当前已写内容（可选）
    /// * `selected_text` - 用户选中的文本（可选）
    pub fn build(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
        current_content: Option<String>,
        selected_text: Option<String>,
    ) -> Result<AgentContext, AppError> {
        let story = self.fetch_story(story_id)?;

        // Phase 3.1: fetch 失败即 fatal — 所有 DB 查询错误通过 ? 传播
        let characters = self.fetch_characters(story_id)?;
        let previous_scenes = self.fetch_previous_scenes(story_id, scene_number)?;
        let world_rules = self.fetch_world_rules(story_id)?;
        let style = self.fetch_writing_style(story_id)?;
        let current_scene = match scene_number {
            Some(n) => self.fetch_current_scene(story_id, n)?,
            None => None,
        };
        let relevant_entities = self.fetch_relevant_entities(story_id, 10)?;

        // Phase 3.2: 空数据致命性判断 — 需要角色的场景类型不能为空角色
        if characters.is_empty() {
            if let Some(ref scene) = current_scene {
                if scene.conflict_type.is_some() {
                    return Err(AppError::context_unavailable(
                        "characters",
                        "当前场景存在冲突类型，但故事中无角色，无法生成场景内容",
                    ));
                }
            }
        }

        // 构建角色信息（增强版：包含目标、背景）
        let character_infos: Vec<CharacterInfo> = characters
            .into_iter()
            .map(|c| {
                let role = if let Some(first_trait) = c.dynamic_traits.first() {
                    first_trait.trait_name.clone()
                } else {
                    c.background
                        .clone()
                        .unwrap_or_else(|| "主要角色".to_string())
                };
                // 合并 personality + goals 作为更丰富的描述
                let personality = match (c.personality.as_ref(), c.goals.as_ref()) {
                    (Some(p), Some(g)) => format!("{}；目标：{}", p, g),
                    (Some(p), None) => p.clone(),
                    (None, Some(g)) => format!("目标：{}", g),
                    (None, None) => "性格未定".to_string(),
                };
                CharacterInfo {
                    name: c.name,
                    personality,
                    role,
                }
            })
            .collect();

        // 构建前文摘要
        let previous_chapters: Vec<ChapterSummary> = previous_scenes
            .into_iter()
            .map(|s| {
                let summary = s
                    .content
                    .clone()
                    .or(s.dramatic_goal.clone())
                    .unwrap_or_else(|| "无内容".to_string());
                let preview = if summary.chars().count() > 200 {
                    format!("{}...", summary.chars().take(200).collect::<String>())
                } else {
                    summary
                };
                ChapterSummary {
                    title: s
                        .title
                        .unwrap_or_else(|| format!("场景 {}", s.sequence_number)),
                    number: s.sequence_number.max(0) as u32,
                    summary: preview,
                }
            })
            .collect();

        // 构建独立的上下文组件（分别注入系统提示词的不同部分）
        let world_rules_text = Self::format_world_rules(&world_rules);
        let scene_structure_text =
            Self::format_scene_structure(current_scene.as_ref(), &relevant_entities);
        let style_blend = self.fetch_style_blend(story_id, scene_number, current_scene.as_ref());

        // W3-B1: 构建 MemoryPack，将 previous_chapters 吸收进 working_memory
        let memory_pack = {
            let orchestrator =
                crate::memory::orchestrator::MemoryOrchestrator::new(self.pool.clone());
            match orchestrator.build_memory_pack(
                story_id,
                scene_number.map(|n| n.max(0) as i32).unwrap_or(1),
                "write",
                current_scene
                    .as_ref()
                    .and_then(|s| s.outline_content.as_ref().map(|o| o.as_str())),
            ) {
                Ok(mut pack) => {
                    // 将 previous_chapters 吸收进 working_memory
                    for chapter in &previous_chapters {
                        pack.working_memory
                            .push(crate::memory::orchestrator::MemoryEntry {
                                layer: "working".to_string(),
                                source: "previous_chapter".to_string(),
                                chapter: chapter.number as i32,
                                content: serde_json::json!({
                                    "title": chapter.title,
                                    "summary": chapter.summary
                                }),
                            });
                    }
                    Some(pack)
                }
                Err(e) => {
                    log::warn!("[StoryContextBuilder] 记忆包构建失败: {}, 继续无记忆包", e);
                    None
                }
            }
        };

        // LitSeg E1: 构建叙事结构感知上下文
        let narrative_structure = self.build_narrative_structure_context(story_id, scene_number);
        let active_threads = self.fetch_active_threads(story_id);

        Ok(AgentContext {
            story: StoryContext {
                story_id: story_id.to_string(),
                story_title: story.title,
                genre: story.genre.unwrap_or_else(|| "小说".to_string()),
                tone: style
                    .as_ref()
                    .and_then(|s| s.tone.clone())
                    .or(story.tone)
                    .unwrap_or_else(|| "中性".to_string()),
                pacing: style
                    .as_ref()
                    .and_then(|s| s.pacing.clone())
                    .or(story.pacing)
                    .unwrap_or_else(|| "正常".to_string()),
            },
            narrative: NarrativeContext {
                chapter_number: scene_number.map(|n| n.max(0) as u32).unwrap_or(1),
                characters: character_infos,
                previous_chapters,
                current_content,
                selected_text,
                narrative_structure: Some(narrative_structure),
                active_threads,
            },
            style: StyleContext {
                style_dna_id: story.style_dna_id,
                style_blend,
                style_fingerprint: None,
            },
            world: WorldContext {
                world_rules: world_rules_text,
                scene_structure: scene_structure_text,
                methodology_id: None,
                methodology_step: None,
            },
            memory: AgentMemoryContext {
                memory_pack,
                memory: None,
            },
        })
    }

    /// 快速构建（用于 intent 执行等场景）
    pub fn build_quick(&self, story_id: &str) -> Result<AgentContext, AppError> {
        self.build(story_id, None, None, None)
    }

    /// 带当前场景号的构建
    pub fn build_for_scene(
        &self,
        story_id: &str,
        scene_number: i32,
        current_content: Option<String>,
    ) -> Result<AgentContext, AppError> {
        self.build(story_id, Some(scene_number), current_content, None)
    }

    // ==================== 数据获取 ====================

    fn fetch_story(&self, story_id: &str) -> Result<Story, String> {
        let repo = StoryRepository::new(self.pool.clone());
        repo.get_by_id(story_id)
            .map_err(|e| format!("获取故事失败: {}", e))?
            .ok_or_else(|| "故事不存在".to_string())
    }

    fn fetch_characters(&self, story_id: &str) -> Result<Vec<Character>, String> {
        let repo = CharacterRepository::new(self.pool.clone());
        repo.get_by_story(story_id)
            .map_err(|e| format!("获取角色失败: {}", e))
    }

    fn fetch_previous_scenes(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
    ) -> Result<Vec<crate::db::models::Scene>, String> {
        let repo = SceneRepository::new(self.pool.clone());
        let all_scenes = repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取场景失败: {}", e))?;

        let cutoff = scene_number.unwrap_or(i32::MAX);
        let mut prev: Vec<_> = all_scenes
            .into_iter()
            .filter(|s| s.sequence_number < cutoff)
            .collect();
        prev.sort_by_key(|s| s.sequence_number);

        // 只保留最近 5 个场景（避免提示词过长）
        if prev.len() > 5 {
            prev = prev.into_iter().rev().take(5).rev().collect();
        }

        Ok(prev)
    }

    fn fetch_current_scene(
        &self,
        story_id: &str,
        scene_number: i32,
    ) -> Result<Option<crate::db::models::Scene>, String> {
        let repo = SceneRepository::new(self.pool.clone());
        let scenes = repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取场景失败: {}", e))?;

        Ok(scenes
            .into_iter()
            .find(|s| s.sequence_number == scene_number))
    }

    fn fetch_world_rules(&self, story_id: &str) -> Result<Vec<WorldRuleSummary>, String> {
        let wb_repo = WorldBuildingRepository::new(self.pool.clone());
        let world_building = match wb_repo.get_by_story(story_id) {
            Ok(Some(wb)) => wb,
            Ok(None) => return Ok(vec![]),
            Err(e) => return Err(format!("获取世界观失败: {}", e)),
        };

        Ok(world_building
            .rules
            .into_iter()
            .map(|r| WorldRuleSummary {
                name: r.name,
                description: r.description.unwrap_or_default(),
                rule_type: r.rule_type.to_string(),
                importance: r.importance,
            })
            .collect())
    }

    fn fetch_writing_style(
        &self,
        story_id: &str,
    ) -> Result<Option<crate::db::models::WritingStyle>, String> {
        let repo = WritingStyleRepository::new(self.pool.clone());
        repo.get_by_story(story_id)
            .map_err(|e| format!("获取文风失败: {}", e))
    }

    fn fetch_relevant_entities(
        &self,
        story_id: &str,
        limit: usize,
    ) -> Result<Vec<RelevantEntity>, String> {
        use crate::db::repositories::KnowledgeGraphRepository;

        let kg_repo = KnowledgeGraphRepository::new(self.pool.clone());
        let entities = kg_repo
            .get_entities_by_story(story_id)
            .map_err(|e| format!("获取知识图谱实体失败: {}", e))?;

        let mut results: Vec<RelevantEntity> = entities
            .into_iter()
            .filter(|e| !e.is_archived)
            .map(|e| {
                let description = e
                    .attributes
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("无描述")
                    .to_string();
                RelevantEntity {
                    name: e.name,
                    entity_type: e.entity_type.to_string(),
                    description,
                    relation_hint: None,
                }
            })
            .collect();

        // 按访问次数排序（优先返回重要实体）
        results.sort_by(|a, b| b.entity_type.cmp(&a.entity_type));
        results.truncate(limit);

        Ok(results)
    }

    /// 获取风格混合配置
    ///
    /// 优先检查 scene 级别的 override，否则回退到 story 级别的 active 配置
    fn fetch_style_blend(
        &self,
        story_id: &str,
        _scene_number: Option<i32>,
        current_scene: Option<&crate::db::models::Scene>,
    ) -> Option<crate::creative_engine::style::blend::StyleBlendConfig> {
        use crate::{
            creative_engine::style::blend::StyleBlendConfig,
            db::repositories::StoryStyleConfigRepository,
        };

        // 1. 检查 scene 级别的 override
        if let Some(scene) = current_scene {
            if let Some(ref override_json) = scene.style_blend_override {
                if let Ok(blend) = serde_json::from_str::<StyleBlendConfig>(override_json) {
                    return Some(blend);
                }
            }
        }

        // 2. 回退到 story 级别的 active 配置
        let repo = StoryStyleConfigRepository::new(self.pool.clone());
        if let Ok(Some(config)) = repo.get_active_by_story(story_id) {
            if let Ok(blend) = serde_json::from_str::<StyleBlendConfig>(&config.blend_json) {
                return Some(blend);
            }
        }

        None
    }

    // ==================== 上下文格式化 ====================

    /// 格式化世界观规则为系统提示词可用文本
    fn format_world_rules(world_rules: &[WorldRuleSummary]) -> Option<String> {
        if world_rules.is_empty() {
            return None;
        }
        let mut parts = Vec::new();
        for rule in world_rules.iter().take(5) {
            parts.push(format!(
                "- {}（{}）: {}",
                rule.name, rule.rule_type, rule.description
            ));
        }
        Some(parts.join("\n"))
    }

    /// 格式化场景结构为系统提示词可用文本
    fn format_scene_structure(
        scene: Option<&crate::db::models::Scene>,
        relevant_entities: &[RelevantEntity],
    ) -> Option<String> {
        let mut parts = Vec::new();

        // 当前场景结构
        if let Some(s) = scene {
            if let Some(ref goal) = s.dramatic_goal {
                parts.push(format!("戏剧目标: {}", goal));
            }
            if let Some(ref pressure) = s.external_pressure {
                parts.push(format!("外部压迫: {}", pressure));
            }
            if let Some(ref ct) = s.conflict_type {
                parts.push(format!("冲突类型: {}", ct));
            }
            if let Some(ref loc) = s.setting_location {
                parts.push(format!("地点: {}", loc));
            }
            if let Some(ref time) = s.setting_time {
                parts.push(format!("时间: {}", time));
            }
            if !s.characters_present.is_empty() {
                parts.push(format!("出场角色: {}", s.characters_present.join(", ")));
            }
        }

        // 知识图谱实体（关键设定）
        if !relevant_entities.is_empty() {
            if !parts.is_empty() {
                parts.push(String::new());
            }
            parts.push("【相关设定】".to_string());
            for entity in relevant_entities.iter().take(10) {
                parts.push(format!(
                    "- {}（{}）: {}",
                    entity.name, entity.entity_type, entity.description
                ));
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }

    // ==================== LitSeg E1: 叙事结构感知 ====================

    /// 构建叙事结构感知上下文
    fn build_narrative_structure_context(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
    ) -> NarrativeStructureContext {
        let current_chapter = scene_number.unwrap_or(1) as i32;

        // 优先从 story_outlines.analyzed_structure_json 读取 LitSeg 分析结果
        if let Some(structure) = self.fetch_analyzed_structure(story_id) {
            if let Some(ctx) = self.locate_in_structure(&structure, current_chapter) {
                return ctx;
            }
        }

        // 其次从 scenes.act_number 推断
        if let Some(ctx) = self.infer_from_scene_acts(story_id, current_chapter) {
            return ctx;
        }

        // 最终回退：基于场景数量做实时推断
        self.infer_narrative_structure_from_scenes(story_id, current_chapter)
    }

    /// 从 story_outlines 读取 LitSeg 分析后的幕结构
    fn fetch_analyzed_structure(
        &self,
        story_id: &str,
    ) -> Option<Vec<crate::narrative::structure::Act>> {
        use crate::db::repositories::StoryOutlineRepository;

        let repo = StoryOutlineRepository::new(self.pool.clone());
        let outline = repo.get_by_story(story_id).ok()??;
        let json_str = outline.analyzed_structure_json?;
        serde_json::from_str(&json_str).ok()
    }

    /// 根据当前章节在分析结构中定位
    fn locate_in_structure(
        &self,
        acts: &[crate::narrative::structure::Act],
        current_chapter: i32,
    ) -> Option<NarrativeStructureContext> {
        let current_act = acts
            .iter()
            .find(|a| current_chapter >= a.start_chapter && current_chapter <= a.end_chapter)?;

        let position_in_act = if current_act.end_chapter > current_act.start_chapter {
            (current_chapter - current_act.start_chapter) as f32
                / (current_act.end_chapter - current_act.start_chapter) as f32
        } else {
            0.5
        };

        let is_near_boundary = position_in_act < 0.15 || position_in_act > 0.85;
        let act_type_str = format!("{:?}", current_act.act_type).to_lowercase();

        Some(NarrativeStructureContext {
            current_act: act_type_str.clone(),
            act_number: current_act.act_number,
            position_in_act,
            dramatic_function: Self::map_act_to_dramatic_function(&act_type_str, position_in_act),
            is_near_boundary,
        })
    }

    /// 从 scenes.act_number 推断叙事位置
    fn infer_from_scene_acts(
        &self,
        story_id: &str,
        current_chapter: i32,
    ) -> Option<NarrativeStructureContext> {
        use crate::db::repositories::SceneRepository;

        let repo = SceneRepository::new(self.pool.clone());
        let scenes = repo.get_by_story(story_id).ok()?;
        let current_scene = scenes
            .iter()
            .find(|s| s.sequence_number == current_chapter)?;
        let act_number = current_scene.act_number.unwrap_or(1);
        let position_in_act = current_scene.position_in_act.unwrap_or(1) as f32 / 3.0;

        let act_type = match act_number {
            1 => "introduction",
            2 => "development",
            3 => "turn",
            4 => "resolution",
            _ => "development",
        };

        Some(NarrativeStructureContext {
            current_act: act_type.to_string(),
            act_number,
            position_in_act,
            dramatic_function: Self::map_act_to_dramatic_function(act_type, position_in_act),
            is_near_boundary: position_in_act < 0.15 || position_in_act > 0.85,
        })
    }

    /// 获取当前活跃的叙事线索
    fn fetch_active_threads(&self, story_id: &str) -> Vec<String> {
        let mut threads = Vec::new();

        // 1. 未回收的伏笔
        use crate::creative_engine::foreshadowing::ForeshadowingTracker;
        let fs_tracker = ForeshadowingTracker::new(self.pool.clone());
        if let Ok(unresolved) = fs_tracker.get_unresolved(story_id) {
            for fs in unresolved.iter().take(5) {
                threads.push(format!(
                    "伏笔:{}(风险:{:.1})",
                    fs.content,
                    fs.risk_signals_score.unwrap_or(0.0)
                ));
            }
        }

        // 2. 有弧光的角色
        use crate::db::repositories::CharacterRepository;
        let char_repo = CharacterRepository::new(self.pool.clone());
        if let Ok(chars) = char_repo.get_by_story(story_id) {
            for ch in chars.iter().take(5) {
                threads.push(format!("角色:{}(弧光)", ch.name));
            }
        }

        threads
    }

    /// 基于场景数量实时推断叙事结构（pipeline 运行前的 fallback）
    fn infer_narrative_structure_from_scenes(
        &self,
        story_id: &str,
        current_chapter: i32,
    ) -> NarrativeStructureContext {
        use crate::db::repositories::SceneRepository;

        let repo = SceneRepository::new(self.pool.clone());
        let scenes = match repo.get_by_story(story_id) {
            Ok(s) => s,
            Err(_) => return NarrativeStructureContext::default(),
        };

        if scenes.is_empty() {
            return NarrativeStructureContext::default();
        }

        let total = scenes.len() as i32;
        let current = current_chapter.max(1).min(total);
        let ratio = current as f32 / total as f32;

        // 简单四分法推断幕结构
        let (act_type, act_number, position_in_act) = if ratio <= 0.25 {
            ("introduction", 1, ratio / 0.25)
        } else if ratio <= 0.5 {
            ("development", 2, (ratio - 0.25) / 0.25)
        } else if ratio <= 0.75 {
            ("turn", 3, (ratio - 0.5) / 0.25)
        } else {
            ("resolution", 4, (ratio - 0.75) / 0.25)
        };

        let is_near_boundary = position_in_act < 0.15 || position_in_act > 0.85;

        NarrativeStructureContext {
            current_act: act_type.to_string(),
            act_number,
            position_in_act,
            dramatic_function: Self::map_act_to_dramatic_function(act_type, position_in_act),
            is_near_boundary,
        }
    }

    /// 将幕类型和位置映射到戏剧功能
    fn map_act_to_dramatic_function(act_type: &str, position: f32) -> String {
        match act_type {
            "introduction" => {
                if position < 0.3 {
                    "铺垫".to_string()
                } else {
                    "触发事件".to_string()
                }
            }
            "development" => {
                if position < 0.5 {
                    "上升动作".to_string()
                } else {
                    " complication".to_string()
                }
            }
            "turn" => {
                if position < 0.5 {
                    "发现".to_string()
                } else {
                    "逆转".to_string()
                }
            }
            "resolution" => {
                if position < 0.3 {
                    "高潮".to_string()
                } else if position < 0.7 {
                    "回落".to_string()
                } else {
                    "结局".to_string()
                }
            }
            _ => "发展".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_world_rules() {
        let rules = vec![WorldRuleSummary {
            name: "灵力体系".to_string(),
            description: "炼气→筑基→金丹".to_string(),
            rule_type: "Magic".to_string(),
            importance: 10,
        }];

        let text = StoryContextBuilder::format_world_rules(&rules).unwrap();
        assert!(text.contains("灵力体系"));
        assert!(text.contains("炼气→筑基→金丹"));
    }

    #[test]
    fn test_format_scene_structure() {
        let scene = crate::db::models::Scene {
            id: "s1".to_string(),
            story_id: "story1".to_string(),
            sequence_number: 3,
            title: Some("决战".to_string()),
            dramatic_goal: Some("主角必须击败反派".to_string()),
            external_pressure: Some("时间限制：日出前".to_string()),
            conflict_type: Some(crate::db::models::ConflictType::ManVsMan),
            characters_present: vec!["张三".to_string(), "李四".to_string()],
            character_conflicts: vec![],
            content: None,
            setting_location: Some("古城钟楼".to_string()),
            setting_time: Some("深夜".to_string()),
            setting_atmosphere: Some("紧张".to_string()),
            previous_scene_id: None,
            next_scene_id: None,
            model_used: None,
            cost: None,
            created_at: chrono::Local::now(),
            updated_at: chrono::Local::now(),
            confidence_score: None,
            execution_stage: None,
            outline_content: None,
            draft_content: None,
            style_blend_override: None,
            foreshadowing_ids: None,
            chapter_id: None,
            narrative_intensity: None,
            narrative_sentiment: None,
            narrative_event_types: None,
            narrative_preceding_scene_id: None,
            narrative_following_scene_id: None,
            act_number: None,
            position_in_act: None,
        };

        let text = StoryContextBuilder::format_scene_structure(Some(&scene), &[]).unwrap();
        assert!(text.contains("戏剧目标"));
        assert!(text.contains("主角必须击败反派"));
        assert!(text.contains("外部压迫"));
        assert!(text.contains("时间限制：日出前"));
        assert!(text.contains("出场角色"));
        assert!(text.contains("张三"));
    }

    // W4-B4: 验证 DB 错误时返回 Err 而非空默认值
    #[test]
    fn test_build_returns_err_when_story_query_fails() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        // 制造致命错误：删除 stories 表使 fetch_story 触发 DB 错误
        {
            let conn = pool.get().unwrap();
            conn.execute("DROP TABLE stories", []).unwrap();
        }

        let builder = StoryContextBuilder::new(pool);
        let result = builder.build("any-id", None, None, None);

        assert!(
            result.is_err(),
            "当 stories 表不存在时，build 应返回致命错误"
        );
        let err_msg = result.unwrap_err().message();
        assert!(
            err_msg.contains("获取故事失败") || err_msg.contains("no such table"),
            "错误信息应指示 DB 查询失败: {}",
            err_msg
        );
    }

    #[test]
    fn test_build_returns_err_when_story_not_found() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let builder = StoryContextBuilder::new(pool);
        let result = builder.build("non-existent-story", None, None, None);

        assert!(result.is_err(), "当故事不存在时，build 应返回 Err");
        assert!(result.unwrap_err().message().contains("故事不存在"));
    }

    #[test]
    fn test_build_fatal_when_characters_empty_with_conflict() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        // 先插入一个合法的故事和一个场景（带冲突类型）
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO stories (id, title, genre, created_at, updated_at) VALUES (?1, ?2, \
                 ?3, ?4, ?5)",
                rusqlite::params![
                    "story-1",
                    "测试故事",
                    "奇幻",
                    chrono::Local::now().to_rfc3339(),
                    chrono::Local::now().to_rfc3339()
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO scenes (id, story_id, sequence_number, title, conflict_type, \
                 characters_present, character_conflicts, created_at, updated_at) VALUES (?1, ?2, \
                 ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    "scene-1",
                    "story-1",
                    1,
                    "测试场景",
                    "ManVsMan",
                    "[]",
                    "[]",
                    chrono::Local::now().to_rfc3339(),
                    chrono::Local::now().to_rfc3339()
                ],
            )
            .unwrap();
        }

        let builder = StoryContextBuilder::new(pool);
        // 没有插入角色，且场景有冲突类型 — 应为 fatal
        let result = builder.build("story-1", Some(1), None, None);

        assert!(
            result.is_err(),
            "有冲突类型的场景但无角色时，build 应返回 fatal 错误"
        );
        assert_eq!(result.unwrap_err().code(), "CONTEXT_UNAVAILABLE");
    }

    #[test]
    fn test_build_ok_when_characters_empty_no_conflict() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        // 先插入一个合法的故事和一个场景（无冲突类型）
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO stories (id, title, genre, created_at, updated_at) VALUES (?1, ?2, \
                 ?3, ?4, ?5)",
                rusqlite::params![
                    "story-1",
                    "测试故事",
                    "奇幻",
                    chrono::Local::now().to_rfc3339(),
                    chrono::Local::now().to_rfc3339()
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO scenes (id, story_id, sequence_number, title, conflict_type, \
                 characters_present, character_conflicts, created_at, updated_at) VALUES (?1, ?2, \
                 ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    "scene-1",
                    "story-1",
                    1,
                    "测试场景",
                    rusqlite::types::Null,
                    "[]",
                    "[]",
                    chrono::Local::now().to_rfc3339(),
                    chrono::Local::now().to_rfc3339()
                ],
            )
            .unwrap();
        }

        let builder = StoryContextBuilder::new(pool);
        // 没有插入角色，但场景无冲突类型 — 应为 ok
        let result = builder.build("story-1", Some(1), None, None);

        assert!(
            result.is_ok(),
            "无冲突类型的场景且无角色时，build 应返回 Ok"
        );
    }
}
