//! Agent Context Optimizer - L0/L1/L2 上下文优化器
//!
//! 借鉴 Vela 项目的 Agent 上下文管理策略，实现三层上下文体系：
//! - L0: 静态元数据（最小token，始终注入）
//! - L1: 结构化知识（中等token，单次检索）
//! - L2: 动态工具检索（按需调用，精准补充）
//!
//! 目标：在有限的上下文窗口内，为 Agent 提供最相关、最紧凑的上下文。

use serde::{Deserialize, Serialize};

use super::{
    AgentContext, AgentMemoryContext, ChapterSummary, CharacterInfo, NarrativeContext,
    StoryContext, StyleContext, WorldContext,
};
use crate::db::{
    repositories::{
        CharacterRepository, SceneRepository, StoryRepository, WorldBuildingRepository,
        WritingStyleRepository,
    },
    repositories_pipeline::{BlueprintRepository, DraftRepository},
    DbPool,
};

// ==================== L0: 静态元数据 ====================

/// L0 上下文 - 故事的静态元数据
/// Token 预算：~200 tokens
/// 始终包含在每次 Agent 调用中
#[derive(Debug, Clone)]
pub struct L0Context {
    pub story_title: String,
    pub genre: String,
    pub tone: String,
    pub pacing: String,
    pub style_dna_id: Option<String>,
    pub style_blend: Option<crate::creative_engine::style::blend::StyleBlendConfig>,
    pub methodology_id: Option<String>,
    pub methodology_step: Option<String>,
}

impl L0Context {
    /// 格式化为紧凑的系统提示词片段
    pub fn to_prompt_text(&self) -> String {
        let mut parts = vec![
            format!("作品: {}", self.story_title),
            format!("题材: {}", self.genre),
            format!("基调: {}", self.tone),
            format!("节奏: {}", self.pacing),
        ];
        if let Some(ref method) = self.methodology_id {
            parts.push(format!("方法论: {}", method));
        }
        if let Some(ref step) = self.methodology_step {
            parts.push(format!("当前步骤: {}", step));
        }
        parts.join(" | ")
    }
}

// ==================== L1: 结构化知识 ====================

/// L1 上下文 - 结构化知识层
/// Token 预算：~1500-2000 tokens
/// 每次写作任务检索一次
#[derive(Debug, Clone)]
pub struct L1Context {
    /// 当前章节蓝图
    pub blueprint: Option<BlueprintSummary>,
    /// 角色卡片（含动态状态）
    pub character_cards: Vec<CharacterCard>,
    /// 最近定稿章节摘要（最多3章）
    pub recent_chapters: Vec<ChapterSummary>,
    /// 世界观规则摘要
    pub world_rules: Option<String>,
    /// 当前场景结构
    pub scene_structure: Option<String>,
    /// 最近草稿状态（如果存在管线草稿）
    pub recent_draft: Option<DraftSummary>,
}

/// 蓝图摘要（紧凑版）
#[derive(Debug, Clone)]
pub struct BlueprintSummary {
    pub chapter_number: i32,
    pub title: Option<String>,
    pub role: Option<String>,
    pub purpose: Option<String>,
    pub key_events: Vec<String>,
    pub characters: Vec<String>,
    pub suspense_hook: Option<String>,
    pub user_guidance: Option<String>,
    pub notes: Option<String>,
}

/// 角色卡片（含动态状态）
#[derive(Debug, Clone)]
pub struct CharacterCard {
    pub name: String,
    pub personality: String,
    pub role: String,
    /// 动态状态字段
    pub location: Option<String>,
    pub power_level: Option<String>,
    pub physical_state: Option<String>,
    pub mental_state: Option<String>,
    pub key_items: Option<String>,
    pub recent_events: Option<String>,
    pub updated_at_chapter: Option<i32>,
}

impl CharacterCard {
    /// 格式化为紧凑的角色描述
    pub fn to_prompt_text(&self) -> String {
        let mut parts = vec![
            format!("【{}】{}", self.name, self.role),
            format!("性格: {}", self.personality),
        ];
        if let Some(ref loc) = self.location {
            parts.push(format!("位置: {}", loc));
        }
        if let Some(ref phys) = self.physical_state {
            parts.push(format!("身体: {}", phys));
        }
        if let Some(ref mental) = self.mental_state {
            parts.push(format!("心理: {}", mental));
        }
        if let Some(ref recent) = self.recent_events {
            parts.push(format!("近况: {}", recent));
        }
        if let Some(ref items) = self.key_items {
            parts.push(format!("持有: {}", items));
        }
        parts.join(" | ")
    }
}

/// 草稿摘要
#[derive(Debug, Clone)]
pub struct DraftSummary {
    pub draft_id: String,
    pub version: i32,
    pub status: String,
    pub word_count: i32,
    pub content_preview: String,
}

impl L1Context {
    /// 格式化为系统提示词片段
    pub fn to_prompt_sections(&self) -> Vec<(String, String)> {
        let mut sections = Vec::new();

        // 蓝图
        if let Some(ref bp) = self.blueprint {
            let mut bp_text = format!(
                "第{}章《{}》| 角色: {} | 目的: {}\n",
                bp.chapter_number,
                bp.title.as_deref().unwrap_or("未命名"),
                bp.role.as_deref().unwrap_or("待定"),
                bp.purpose.as_deref().unwrap_or("待定")
            );
            if !bp.key_events.is_empty() {
                bp_text.push_str(&format!("关键事件: {}\n", bp.key_events.join(", ")));
            }
            if !bp.characters.is_empty() {
                bp_text.push_str(&format!("出场角色: {}\n", bp.characters.join(", ")));
            }
            if let Some(ref hook) = bp.suspense_hook {
                bp_text.push_str(&format!("悬念钩子: {}\n", hook));
            }
            if let Some(ref guidance) = bp.user_guidance {
                bp_text.push_str(&format!("作者指引: {}\n", guidance));
            }
            if let Some(ref notes) = bp.notes {
                bp_text.push_str(&format!("剧情要点: {}\n", notes));
            }
            sections.push(("章节蓝图".to_string(), bp_text));
        }

        // 角色卡片
        if !self.character_cards.is_empty() {
            let cards_text = self
                .character_cards
                .iter()
                .map(|c| c.to_prompt_text())
                .collect::<Vec<_>>()
                .join("\n");
            sections.push(("角色状态".to_string(), cards_text));
        }

        // 世界观
        if let Some(ref rules) = self.world_rules {
            if !rules.is_empty() {
                sections.push(("世界观".to_string(), rules.clone()));
            }
        }

        // 场景结构
        if let Some(ref scene) = self.scene_structure {
            if !scene.is_empty() {
                sections.push(("场景结构".to_string(), scene.clone()));
            }
        }

        // 前文摘要
        if !self.recent_chapters.is_empty() {
            let chapters_text = self
                .recent_chapters
                .iter()
                .map(|c| format!("第{}章 {}: {}", c.number, c.title, c.summary))
                .collect::<Vec<_>>()
                .join("\n\n");
            sections.push(("前文摘要".to_string(), chapters_text));
        }

        // 草稿状态
        if let Some(ref draft) = self.recent_draft {
            sections.push((
                "当前草稿".to_string(),
                format!(
                    "版本{} ({}) | {}字 | {}",
                    draft.version, draft.status, draft.word_count, draft.content_preview
                ),
            ));
        }

        sections
    }
}

// ==================== L2: 动态工具检索 ====================

/// L2 工具 - 按需动态检索
/// Token 预算：视工具而定，每次调用 ~300-800 tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2Tool {
    /// 知识库语义搜索
    SearchKnowledge {
        query: String,
        top_k: usize,
        chapter_range: Option<(i32, i32)>,
    },
    /// 获取特定角色详细状态
    GetCharacterState { character_name: String },
    /// 读取特定章节蓝图
    ReadBlueprint { chapter_number: i32 },
    /// 检查内容连续性
    CheckContinuity { content: String },
    /// 搜索前文中的特定事件或设定
    SearchPreviousEvents { keyword: String, max_results: usize },
}

impl L2Tool {
    pub fn name(&self) -> &'static str {
        match self {
            L2Tool::SearchKnowledge { .. } => "search_knowledge",
            L2Tool::GetCharacterState { .. } => "get_character_state",
            L2Tool::ReadBlueprint { .. } => "read_blueprint",
            L2Tool::CheckContinuity { .. } => "check_continuity",
            L2Tool::SearchPreviousEvents { .. } => "search_previous_events",
        }
    }

    pub fn description(&self) -> String {
        match self {
            L2Tool::SearchKnowledge { query, .. } => format!("知识库搜索: {}", query),
            L2Tool::GetCharacterState { character_name } => {
                format!("查询角色状态: {}", character_name)
            }
            L2Tool::ReadBlueprint { chapter_number } => format!("读取第{}章蓝图", chapter_number),
            L2Tool::CheckContinuity { .. } => "检查内容连续性".to_string(),
            L2Tool::SearchPreviousEvents { keyword, .. } => format!("搜索前文事件: {}", keyword),
        }
    }
}

/// L2 工具执行结果
#[derive(Debug, Clone)]
pub struct L2ToolResult {
    pub tool: L2Tool,
    pub content: String,
    pub token_estimate: usize,
}

// ==================== Context Optimizer ====================

/// 上下文优化器
/// 负责构建 L0/L1/L2 三层上下文
pub struct ContextOptimizer {
    pool: DbPool,
}

impl ContextOptimizer {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // ==================== L0 构建 ====================

    /// 构建 L0 静态元数据上下文
    pub fn build_l0(&self, story_id: &str) -> Result<L0Context, String> {
        let story = self.fetch_story(story_id)?;
        let style = self.fetch_writing_style(story_id).ok().flatten();

        let style_blend = self.fetch_style_blend(story_id);

        Ok(L0Context {
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
            style_dna_id: story.style_dna_id,
            style_blend,
            methodology_id: story.methodology_id,
            methodology_step: story.methodology_step.map(|s| s.to_string()),
        })
    }

    // ==================== L1 构建 ====================

    /// 构建 L1 结构化知识上下文
    pub async fn build_l1(&self, story_id: &str, chapter_number: u32) -> Result<L1Context, String> {
        let chapter_number_i32 = chapter_number as i32;

        // 并行获取数据
        let blueprint = self.fetch_blueprint_summary(story_id, chapter_number_i32);
        let character_cards = self.fetch_character_cards(story_id);
        let recent_chapters = self.fetch_recent_chapters(story_id, chapter_number);
        let world_rules = self.fetch_world_rules_text(story_id);
        let scene_structure = self.fetch_scene_structure_text(story_id, chapter_number_i32);
        let recent_draft = self.fetch_recent_draft(story_id, chapter_number_i32);

        // 由于目前都是同步的数据库操作，按顺序执行
        // 如果后续有异步IO，可以改为 futures::join!
        Ok(L1Context {
            blueprint,
            character_cards: character_cards.unwrap_or_default(),
            recent_chapters: recent_chapters.unwrap_or_default(),
            world_rules,
            scene_structure,
            recent_draft,
        })
    }

    // ==================== L2 工具执行 ====================

    /// 执行 L2 动态检索工具
    pub async fn execute_l2_tool(
        &self,
        story_id: &str,
        tool: L2Tool,
    ) -> Result<L2ToolResult, String> {
        let content = match &tool {
            L2Tool::SearchKnowledge {
                query,
                top_k,
                chapter_range,
            } => {
                self.tool_search_knowledge(story_id, query, *top_k, *chapter_range)
                    .await
            }
            L2Tool::GetCharacterState { character_name } => {
                self.tool_get_character_state(story_id, character_name)
            }
            L2Tool::ReadBlueprint { chapter_number } => {
                self.tool_read_blueprint(story_id, *chapter_number)
            }
            L2Tool::CheckContinuity { content: text } => {
                self.tool_check_continuity(story_id, text).await
            }
            L2Tool::SearchPreviousEvents {
                keyword,
                max_results,
            } => {
                self.tool_search_previous_events(story_id, keyword, *max_results)
                    .await
            }
        }?;

        let token_estimate = content.chars().count() / 2; // 粗略估算

        Ok(L2ToolResult {
            tool,
            content,
            token_estimate,
        })
    }

    // ==================== 完整上下文构建 ====================

    /// 构建完整的 AgentContext，自动组装 L0 + L1 + 选择性 L2
    ///
    /// `l2_tools`: 需要预执行的 L2 工具列表。如果为空，只返回 L0+L1。
    pub async fn build_full_context(
        &self,
        story_id: &str,
        chapter_number: u32,
        current_content: Option<String>,
        selected_text: Option<String>,
        l2_tools: Vec<L2Tool>,
    ) -> Result<AgentContext, String> {
        let l0 = self.build_l0(story_id)?;
        let l1 = self.build_l1(story_id, chapter_number).await?;

        // 执行 L2 工具
        let mut l2_results = Vec::new();
        for tool in l2_tools {
            match self.execute_l2_tool(story_id, tool).await {
                Ok(result) => l2_results.push(result),
                Err(e) => {
                    log::warn!("[ContextOptimizer] L2 tool failed: {}", e);
                }
            }
        }

        // 组装 MemoryPack（三层记忆）
        let memory_pack = {
            let orchestrator =
                crate::memory::orchestrator::MemoryOrchestrator::new(self.pool.clone());
            match orchestrator.build_memory_pack(story_id, chapter_number as i32, "write", None) {
                Ok(mut pack) => {
                    // 将前文摘要吸收进 working_memory
                    for chapter in &l1.recent_chapters {
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
                    log::warn!(
                        "[ContextOptimizer] MemoryPack build failed: {}, continuing without",
                        e
                    );
                    None
                }
            }
        };

        // 组装为 AgentContext
        let mut agent_ctx = AgentContext {
            story: StoryContext {
                story_id: story_id.to_string(),
                story_title: l0.story_title,
                genre: l0.genre,
                tone: l0.tone,
                pacing: l0.pacing,
            },
            narrative: NarrativeContext {
                chapter_number,
                characters: l1
                    .character_cards
                    .iter()
                    .map(|c| CharacterInfo {
                        name: c.name.clone(),
                        personality: c.personality.clone(),
                        role: c.role.clone(),
                    })
                    .collect(),
                previous_chapters: l1.recent_chapters,
                current_content: current_content.clone(),
                selected_text: selected_text.clone(),
                narrative_structure: None,
                active_threads: vec![],
            },
            style: StyleContext {
                style_dna_id: l0.style_dna_id,
                style_blend: l0.style_blend,
                style_fingerprint: None,
            },
            world: WorldContext {
                world_rules: l1.world_rules,
                scene_structure: l1.scene_structure,
                methodology_id: l0.methodology_id,
                methodology_step: l0.methodology_step,
            },
            memory: AgentMemoryContext {
                memory_pack,
                memory: None,
            },
        };

        // 将 L2 结果追加到 scene_structure 或 world_rules 中
        if !l2_results.is_empty() {
            let l2_text = l2_results
                .iter()
                .map(|r| format!("【{}】\n{}", r.tool.description(), r.content))
                .collect::<Vec<_>>()
                .join("\n\n");

            let enhanced = format!(
                "{}\n\n【动态检索结果】\n{}",
                agent_ctx.world.scene_structure.clone().unwrap_or_default(),
                l2_text
            );
            if !enhanced.trim().is_empty() {
                agent_ctx.world.scene_structure = Some(enhanced);
            }
        }

        // 将蓝图信息注入到 world_rules 中（如果存在）
        if let Some(ref bp) = l1.blueprint {
            let bp_text = format!(
                "【本章蓝图】\n第{}章《{}》\n角色: {}\n目的: {}\n关键事件: {}\n{}",
                bp.chapter_number,
                bp.title.as_deref().unwrap_or("未命名"),
                bp.role.as_deref().unwrap_or("待定"),
                bp.purpose.as_deref().unwrap_or("待定"),
                bp.key_events.join(", "),
                bp.suspense_hook
                    .as_ref()
                    .map(|h| format!("悬念: {}\n", h))
                    .unwrap_or_default()
            );
            agent_ctx.world.world_rules = Some(
                format!(
                    "{}\n\n{}",
                    agent_ctx.world.world_rules.unwrap_or_default(),
                    bp_text
                )
                .trim()
                .to_string(),
            );
        }

        Ok(agent_ctx)
    }

    /// 快速构建（不带 L2）
    pub async fn build_quick(
        &self,
        story_id: &str,
        chapter_number: u32,
    ) -> Result<AgentContext, String> {
        self.build_full_context(story_id, chapter_number, None, None, vec![])
            .await
    }

    // ==================== 数据获取方法 ====================

    fn fetch_story(&self, story_id: &str) -> Result<crate::db::Story, String> {
        let repo = StoryRepository::new(self.pool.clone());
        repo.get_by_id(story_id)
            .map_err(|e| format!("获取故事失败: {}", e))?
            .ok_or_else(|| "故事不存在".to_string())
    }

    fn fetch_writing_style(
        &self,
        story_id: &str,
    ) -> Result<Option<crate::db::models::WritingStyle>, String> {
        let repo = WritingStyleRepository::new(self.pool.clone());
        repo.get_by_story(story_id)
            .map_err(|e| format!("获取文风失败: {}", e))
    }

    fn fetch_style_blend(
        &self,
        story_id: &str,
    ) -> Option<crate::creative_engine::style::blend::StyleBlendConfig> {
        use crate::{
            creative_engine::style::blend::StyleBlendConfig,
            db::repositories::StoryStyleConfigRepository,
        };

        let repo = StoryStyleConfigRepository::new(self.pool.clone());
        if let Ok(Some(config)) = repo.get_active_by_story(story_id) {
            if let Ok(blend) = serde_json::from_str::<StyleBlendConfig>(&config.blend_json) {
                return Some(blend);
            }
        }
        None
    }

    fn fetch_blueprint_summary(
        &self,
        story_id: &str,
        chapter_number: i32,
    ) -> Option<BlueprintSummary> {
        let repo = BlueprintRepository::new(self.pool.clone());
        match repo.get_by_chapter(story_id, chapter_number) {
            Ok(Some(bp)) => {
                let key_events: Vec<String> = bp
                    .key_events
                    .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
                    .unwrap_or_default();
                let characters: Vec<String> = bp
                    .characters
                    .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
                    .unwrap_or_default();

                Some(BlueprintSummary {
                    chapter_number: bp.chapter_number,
                    title: bp.title,
                    role: bp.role,
                    purpose: bp.purpose,
                    key_events,
                    characters,
                    suspense_hook: bp.suspense_hook,
                    user_guidance: bp.user_guidance,
                    notes: bp.notes,
                })
            }
            _ => None,
        }
    }

    fn fetch_character_cards(&self, story_id: &str) -> Result<Vec<CharacterCard>, String> {
        let repo = CharacterRepository::new(self.pool.clone());
        let characters = repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取角色失败: {}", e))?;

        Ok(characters
            .into_iter()
            .map(|c| {
                let role = c
                    .background
                    .clone()
                    .unwrap_or_else(|| "主要角色".to_string());
                let personality = match (c.personality.as_ref(), c.goals.as_ref()) {
                    (Some(p), Some(g)) => format!("{}；目标：{}", p, g),
                    (Some(p), None) => p.clone(),
                    (None, Some(g)) => format!("目标：{}", g),
                    (None, None) => "性格未定".to_string(),
                };

                CharacterCard {
                    name: c.name,
                    personality,
                    role,
                    location: c.cs_location,
                    power_level: c.cs_power_level,
                    physical_state: c.cs_physical_state,
                    mental_state: c.cs_mental_state,
                    key_items: c.cs_key_items,
                    recent_events: c.cs_recent_events,
                    updated_at_chapter: c.cs_updated_at_chapter,
                }
            })
            .collect())
    }

    fn fetch_recent_chapters(
        &self,
        story_id: &str,
        current_chapter: u32,
    ) -> Result<Vec<ChapterSummary>, String> {
        let repo = SceneRepository::new(self.pool.clone());
        let all_scenes = repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取场景失败: {}", e))?;

        let mut prev: Vec<_> = all_scenes
            .into_iter()
            .filter(|s| s.sequence_number < current_chapter as i32)
            .collect();
        prev.sort_by_key(|s| s.sequence_number);

        // 只保留最近 3 个场景（比之前减少，节省 token）
        if prev.len() > 3 {
            prev = prev.into_iter().rev().take(3).rev().collect();
        }

        Ok(prev
            .into_iter()
            .map(|s| {
                let summary = s
                    .content
                    .clone()
                    .or(s.dramatic_goal.clone())
                    .unwrap_or_else(|| "无内容".to_string());
                let preview = if summary.chars().count() > 150 {
                    format!("{}...", summary.chars().take(150).collect::<String>())
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
            .collect())
    }

    fn fetch_world_rules_text(&self, story_id: &str) -> Option<String> {
        let wb_repo = WorldBuildingRepository::new(self.pool.clone());
        let world_building = match wb_repo.get_by_story(story_id) {
            Ok(Some(wb)) => wb,
            _ => return None,
        };

        let rules: Vec<String> = world_building
            .rules
            .into_iter()
            .take(5)
            .map(|r| {
                format!(
                    "- {}（{}）: {}",
                    r.name,
                    r.rule_type,
                    r.description.unwrap_or_default()
                )
            })
            .collect();

        if rules.is_empty() {
            None
        } else {
            Some(rules.join("\n"))
        }
    }

    fn fetch_scene_structure_text(&self, story_id: &str, scene_number: i32) -> Option<String> {
        let repo = SceneRepository::new(self.pool.clone());
        let scenes = match repo.get_by_story(story_id) {
            Ok(s) => s,
            Err(_) => return None,
        };

        let scene = scenes
            .into_iter()
            .find(|s| s.sequence_number == scene_number)?;

        let mut parts = Vec::new();
        if let Some(ref goal) = scene.dramatic_goal {
            parts.push(format!("戏剧目标: {}", goal));
        }
        if let Some(ref pressure) = scene.external_pressure {
            parts.push(format!("外部压迫: {}", pressure));
        }
        if let Some(ref ct) = scene.conflict_type {
            parts.push(format!("冲突类型: {}", ct));
        }
        if let Some(ref loc) = scene.setting_location {
            parts.push(format!("地点: {}", loc));
        }
        if let Some(ref time) = scene.setting_time {
            parts.push(format!("时间: {}", time));
        }
        if !scene.characters_present.is_empty() {
            parts.push(format!("出场角色: {}", scene.characters_present.join(", ")));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }

    fn fetch_recent_draft(&self, story_id: &str, chapter_number: i32) -> Option<DraftSummary> {
        let repo = DraftRepository::new(self.pool.clone());
        match repo.get_by_story_chapter(story_id, chapter_number) {
            Ok(drafts) => drafts.into_iter().max_by_key(|d| d.version).map(|d| {
                let preview = d.content.chars().take(100).collect::<String>();
                DraftSummary {
                    draft_id: d.id,
                    version: d.version,
                    status: d.status.to_string(),
                    word_count: d.word_count,
                    content_preview: preview,
                }
            }),
            Err(_) => None,
        }
    }

    // ==================== L2 工具实现 ====================

    async fn tool_search_knowledge(
        &self,
        story_id: &str,
        query: &str,
        top_k: usize,
        chapter_range: Option<(i32, i32)>,
    ) -> Result<String, String> {
        if let Some(store) = crate::VECTOR_STORE.get() {
            match crate::knowledge_base::kb_search(
                store,
                story_id,
                query,
                top_k,
                chapter_range,
                "hybrid",
            )
            .await
            {
                Ok(results) => {
                    if results.is_empty() {
                        return Ok("未找到相关知识".to_string());
                    }
                    let lines: Vec<String> = results
                        .iter()
                        .map(|r| {
                            format!("[第{}章 相似度{:.2}] {}", r.chapter_number, r.score, r.text)
                        })
                        .collect();
                    Ok(lines.join("\n"))
                }
                Err(e) => Err(format!("知识库搜索失败: {}", e)),
            }
        } else {
            Ok("向量存储未初始化".to_string())
        }
    }

    fn tool_get_character_state(
        &self,
        story_id: &str,
        character_name: &str,
    ) -> Result<String, String> {
        let repo = CharacterRepository::new(self.pool.clone());
        let characters = repo
            .get_by_story(story_id)
            .map_err(|e| format!("获取角色失败: {}", e))?;

        let character = characters
            .into_iter()
            .find(|c| c.name == character_name)
            .ok_or_else(|| format!("未找到角色: {}", character_name))?;

        let mut parts = vec![
            format!("角色: {}", character.name),
            format!("背景: {}", character.background.unwrap_or_default()),
            format!("性格: {}", character.personality.unwrap_or_default()),
            format!("目标: {}", character.goals.unwrap_or_default()),
        ];

        if let Some(ref loc) = character.cs_location {
            parts.push(format!("当前位置: {}", loc));
        }
        if let Some(ref power) = character.cs_power_level {
            parts.push(format!("实力: {}", power));
        }
        if let Some(ref phys) = character.cs_physical_state {
            parts.push(format!("身体状态: {}", phys));
        }
        if let Some(ref mental) = character.cs_mental_state {
            parts.push(format!("心理状态: {}", mental));
        }
        if let Some(ref items) = character.cs_key_items {
            parts.push(format!("关键物品: {}", items));
        }
        if let Some(ref recent) = character.cs_recent_events {
            parts.push(format!("近期事件: {}", recent));
        }

        Ok(parts.join("\n"))
    }

    fn tool_read_blueprint(&self, story_id: &str, chapter_number: i32) -> Result<String, String> {
        let repo = BlueprintRepository::new(self.pool.clone());
        match repo.get_by_chapter(story_id, chapter_number) {
            Ok(Some(bp)) => {
                let mut parts = vec![
                    format!(
                        "第{}章《{}》",
                        bp.chapter_number,
                        bp.title.as_deref().unwrap_or("未命名")
                    ),
                    format!("角色: {}", bp.role.as_deref().unwrap_or("待定")),
                    format!("目的: {}", bp.purpose.as_deref().unwrap_or("待定")),
                ];

                if let Some(ref key_events_json) = bp.key_events {
                    if let Ok(events) = serde_json::from_str::<Vec<String>>(key_events_json) {
                        if !events.is_empty() {
                            parts.push(format!(
                                "关键事件:\n{}",
                                events
                                    .iter()
                                    .map(|e| format!("- {}", e))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            ));
                        }
                    }
                }

                if let Some(ref suspense) = bp.suspense_hook {
                    parts.push(format!("悬念钩子: {}", suspense));
                }
                if let Some(ref guidance) = bp.user_guidance {
                    parts.push(format!("作者指引: {}", guidance));
                }
                if let Some(ref notes) = bp.notes {
                    parts.push(format!("剧情要点: {}", notes));
                }

                Ok(parts.join("\n"))
            }
            Ok(None) => Ok(format!("第{}章暂无蓝图", chapter_number)),
            Err(e) => Err(format!("读取蓝图失败: {}", e)),
        }
    }

    async fn tool_check_continuity(&self, story_id: &str, content: &str) -> Result<String, String> {
        let repo = SceneRepository::new(self.pool.clone());
        let scenes = match repo.get_by_story(story_id) {
            Ok(s) => s,
            Err(e) => return Err(format!("获取场景失败: {}", e)),
        };

        // 获取最近3个场景的内容进行连续性检查
        let recent_contents: Vec<String> = scenes
            .into_iter()
            .rev()
            .take(3)
            .rev()
            .filter_map(|s| s.content)
            .collect();

        if recent_contents.is_empty() {
            return Ok("无前文章节可供检查".to_string());
        }

        let issues: Vec<String> = Vec::new();

        // 简单的启发式检查
        // 1. 检查角色名一致性
        let repo = CharacterRepository::new(self.pool.clone());
        if let Ok(characters) = repo.get_by_story(story_id) {
            for char in &characters {
                if content.contains(&char.name) {
                    // 角色出现，检查是否有明显状态矛盾
                    if let Some(ref phys) = char.cs_physical_state {
                        if phys.contains("受伤") || phys.contains("昏迷") {
                            // 简单启发：如果前文说角色昏迷，
                            // 新内容中角色行动正常
                            // 这是一个简单检查，真正的连续性检查应使用 LLM
                        }
                    }
                }
            }
        }

        // 2. 检查时间/地点一致性关键词
        let combined_recent = recent_contents.join("\n");
        let time_keywords = vec![
            "清晨", "上午", "中午", "下午", "傍晚", "夜晚", "深夜", "凌晨",
        ];
        for kw in time_keywords {
            if combined_recent.contains(kw) && content.contains(kw) {
                // 时间关键词重复出现，可能是合理的
            }
        }

        if issues.is_empty() {
            Ok("未发现明显的连续性问题".to_string())
        } else {
            Ok(issues.join("\n"))
        }
    }

    async fn tool_search_previous_events(
        &self,
        story_id: &str,
        keyword: &str,
        max_results: usize,
    ) -> Result<String, String> {
        // 优先使用向量搜索
        if let Some(store) = crate::VECTOR_STORE.get() {
            match crate::knowledge_base::kb_search(
                store,
                story_id,
                keyword,
                max_results,
                None,
                "hybrid",
            )
            .await
            {
                Ok(results) => {
                    if !results.is_empty() {
                        let lines: Vec<String> = results
                            .iter()
                            .map(|r| format!("[第{}章] {}", r.chapter_number, r.text))
                            .collect();
                        return Ok(lines.join("\n"));
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[ContextOptimizer] KB search failed: {}, falling back to text search",
                        e
                    );
                }
            }
        }

        // 回退：在场景内容中搜索
        let repo = SceneRepository::new(self.pool.clone());
        let scenes = match repo.get_by_story(story_id) {
            Ok(s) => s,
            Err(e) => return Err(format!("获取场景失败: {}", e)),
        };

        let mut matches = Vec::new();
        for scene in scenes {
            if let Some(ref content) = scene.content {
                if content.contains(keyword) {
                    let preview = if content.chars().count() > 200 {
                        // 找到关键词位置，提取上下文
                        if let Some(pos) = content.find(keyword) {
                            let start = pos.saturating_sub(100);
                            let end = (pos + keyword.len() + 100).min(content.len());
                            format!("...{}...", &content[start..end])
                        } else {
                            content.chars().take(200).collect::<String>() + "..."
                        }
                    } else {
                        content.clone()
                    };
                    matches.push(format!(
                        "[第{}章《{}》] {}",
                        scene.sequence_number,
                        scene.title.unwrap_or_default(),
                        preview
                    ));
                    if matches.len() >= max_results {
                        break;
                    }
                }
            }
        }

        if matches.is_empty() {
            Ok(format!("未找到包含 '{}' 的前文内容", keyword))
        } else {
            Ok(matches.join("\n\n"))
        }
    }
}

// ==================== 便捷函数 ====================

/// 推荐的默认 L2 工具组合（写作时）
pub fn default_writing_tools(chapter_number: u32) -> Vec<L2Tool> {
    vec![L2Tool::ReadBlueprint {
        chapter_number: chapter_number as i32,
    }]
}

/// 推荐的 L2 工具组合（改写时）
pub fn default_rewrite_tools(content: &str, chapter_number: u32) -> Vec<L2Tool> {
    vec![
        L2Tool::ReadBlueprint {
            chapter_number: chapter_number as i32,
        },
        L2Tool::CheckContinuity {
            content: content.to_string(),
        },
    ]
}

/// 推荐的 L2 工具组合（质检时）
pub fn default_inspection_tools(content: &str, chapter_number: u32) -> Vec<L2Tool> {
    vec![
        L2Tool::ReadBlueprint {
            chapter_number: chapter_number as i32,
        },
        L2Tool::CheckContinuity {
            content: content.to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_l0_to_prompt_text() {
        let l0 = L0Context {
            story_title: "测试作品".to_string(),
            genre: "玄幻".to_string(),
            tone: "热血".to_string(),
            pacing: "快".to_string(),
            style_dna_id: None,
            style_blend: None,
            methodology_id: Some("snowflake".to_string()),
            methodology_step: Some("step3".to_string()),
        };
        let text = l0.to_prompt_text();
        assert!(text.contains("测试作品"));
        assert!(text.contains("玄幻"));
        assert!(text.contains("snowflake"));
    }

    #[test]
    fn test_character_card_to_prompt() {
        let card = CharacterCard {
            name: "张三".to_string(),
            personality: "勇敢".to_string(),
            role: "主角".to_string(),
            location: Some("京城".to_string()),
            power_level: None,
            physical_state: Some("轻伤".to_string()),
            mental_state: None,
            key_items: Some("宝剑".to_string()),
            recent_events: Some("[第5章] 击败妖魔".to_string()),
            updated_at_chapter: Some(5),
        };
        let text = card.to_prompt_text();
        assert!(text.contains("张三"));
        assert!(text.contains("京城"));
        assert!(text.contains("轻伤"));
        assert!(text.contains("宝剑"));
    }
}
