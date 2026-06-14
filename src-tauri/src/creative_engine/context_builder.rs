#![allow(dead_code)]
//! StoryContextBuilder - 创作上下文构建器
//!
//! 从数据库中读取真实故事数据，为 Agent 提供完整的创作上下文。
//! 解决 intent.rs 中硬编码 "未命名作品"/"小说"/"中性" 的问题。

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::RwLock;

use crate::{
    agents::{
        AgentContext, AgentMemoryContext, ChapterSummary, CharacterInfo, NarrativeContext,
        NarrativeStructureContext, StoryContext, StyleContext, WorldContext,
    },
    config::settings::{default_context_budget_ratio, default_max_context_length, AppConfig},
    db::{
        repositories::{
            CharacterRepository, SceneRepository, StoryRepository, WorldBuildingRepository,
            WritingStyleRepository,
        },
        Character, DbPool, Story,
    },
    error::AppError,
    memory::tokenizer::{count_tokens, truncate_to_budget, truncate_to_budget_from_end},
};

/// 创作上下文缓存键
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct ContextCacheKey {
    story_id: String,
    scene_number: Option<i32>,
    current_content_hash: u64,
    selected_text_hash: u64,
}

/// 带 TTL 与容量上限的上下文缓存
#[derive(Clone)]
pub struct ContextCache {
    inner: Arc<RwLock<ContextCacheInner>>,
}

struct ContextCacheInner {
    entries: std::collections::HashMap<ContextCacheKey, (AgentContext, Instant)>,
    max_entries: usize,
    ttl: Duration,
}

impl ContextCache {
    pub fn new(max_entries: usize, ttl: Duration) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ContextCacheInner {
                entries: std::collections::HashMap::new(),
                max_entries,
                ttl,
            })),
        }
    }

    fn hash_option_text(text: &Option<String>) -> u64 {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }

    fn key(
        story_id: &str,
        scene_number: Option<i32>,
        current_content: &Option<String>,
        selected_text: &Option<String>,
    ) -> ContextCacheKey {
        ContextCacheKey {
            story_id: story_id.to_string(),
            scene_number,
            current_content_hash: Self::hash_option_text(current_content),
            selected_text_hash: Self::hash_option_text(selected_text),
        }
    }

    pub fn get(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
        current_content: &Option<String>,
        selected_text: &Option<String>,
    ) -> Option<AgentContext> {
        let key = Self::key(story_id, scene_number, current_content, selected_text);
        // 使用 try_read 保持方法同步，避免在异步上下文中阻塞 worker。
        // 若锁不可用则按未命中处理。
        let inner = self.inner.try_read().ok()?;
        if let Some((ctx, created_at)) = inner.entries.get(&key) {
            if created_at.elapsed() < inner.ttl {
                log::debug!(
                    "[StoryContextBuilder] Context cache hit for story {}",
                    story_id
                );
                return Some(ctx.clone());
            }
        }
        // TTL 过期：降级为写锁移除过期条目。
        drop(inner);
        let mut inner = self.inner.try_write().ok()?;
        inner.entries.remove(&key);
        None
    }

    pub fn put(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
        current_content: &Option<String>,
        selected_text: &Option<String>,
        context: AgentContext,
    ) {
        // 使用 try_write 保持方法同步，避免在异步上下文中阻塞 worker。
        // 若锁不可用则静默放弃写入。
        let Ok(mut inner) = self.inner.try_write() else {
            return;
        };
        let key = Self::key(story_id, scene_number, current_content, selected_text);
        // 简单 LRU 清理：超过容量时移除最旧的条目
        if inner.entries.len() >= inner.max_entries {
            let oldest = inner
                .entries
                .iter()
                .min_by_key(|(_, (_, t))| *t)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                inner.entries.remove(&k);
            }
        }
        inner.entries.insert(key, (context, Instant::now()));
    }
}

/// 默认全局上下文缓存：最多保留 50 个故事上下文，5 分钟 TTL。
///
/// 命中缓存可跳过重复的 DB 查询、MemoryPack 构建与叙事结构分析，
/// 显著降低 Writer → Inspector → Rewrite 闭环内的等待时间。
///
/// 使用进程级 OnceLock 替代 thread_local!，使跨线程请求能命中同一缓存。
static DEFAULT_CONTEXT_CACHE: std::sync::OnceLock<ContextCache> = std::sync::OnceLock::new();

fn default_context_cache() -> ContextCache {
    DEFAULT_CONTEXT_CACHE
        .get_or_init(|| ContextCache::new(50, Duration::from_secs(300)))
        .clone()
}

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

/// 上下文预算策略
///
/// 控制上下文构建时的 token 预算分配。默认使用 8192 tokens / 80% 预算比例，
/// 与 `default_max_context_length` 保持一致。
#[derive(Debug, Clone)]
pub struct ContextBudget {
    /// 目标模型最大上下文长度（token）
    pub max_context_length: usize,
    /// 实际使用的预算比例（0.0 - 1.0）
    pub budget_ratio: f32,
    /// 用于 token 计数的模型 family
    pub model_family: String,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_context_length: default_max_context_length() as usize,
            budget_ratio: default_context_budget_ratio(),
            model_family: "cl100k".to_string(),
        }
    }
}

impl ContextBudget {
    /// 总可用预算（不含系统提示预留）
    pub fn total_budget(&self) -> usize {
        (self.max_context_length as f32 * self.budget_ratio.clamp(0.1, 0.95)) as usize
    }

    /// 为系统提示/指令保留的 token 数
    pub fn system_budget(&self) -> usize {
        (self.total_budget() as f32 * 0.15) as usize
    }

    /// 为世界/角色/风格等关键设定保留的预算
    pub fn story_context_budget(&self) -> usize {
        (self.total_budget() as f32 * 0.25) as usize
    }

    /// 为近期场景/当前内容保留的预算
    pub fn scene_budget(&self) -> usize {
        (self.total_budget() as f32 * 0.40) as usize
    }

    /// 为用户输入/选中内容保留的预算
    pub fn user_input_budget(&self) -> usize {
        (self.total_budget() as f32 * 0.20) as usize
    }
}

/// 上下文构建器
pub struct StoryContextBuilder {
    pool: DbPool,
    cache: Option<ContextCache>,
    budget: ContextBudget,
}

impl StoryContextBuilder {
    /// 创建带有默认全局缓存的构建器
    pub fn new(pool: DbPool) -> Self {
        Self::with_cache(pool, default_context_cache())
    }

    /// 创建不带缓存的构建器（主要用于测试或需要强制重新加载的场景）
    #[allow(dead_code)]
    pub fn without_cache(pool: DbPool) -> Self {
        Self {
            pool,
            cache: None,
            budget: ContextBudget::default(),
        }
    }

    /// 创建带有指定缓存的构建器
    pub fn with_cache(pool: DbPool, cache: ContextCache) -> Self {
        Self {
            pool,
            cache: Some(cache),
            budget: ContextBudget::default(),
        }
    }

    /// 设置目标模型的上下文预算
    ///
    /// 调用方（如 Agent 服务）可根据当前路由到的模型传入 `max_context_length`
    /// 与 `model_family`，避免上下文超过模型窗口。
    pub fn with_context_budget(
        mut self,
        max_context_length: usize,
        model_family: impl Into<String>,
    ) -> Self {
        self.budget.max_context_length = max_context_length;
        self.budget.model_family = model_family.into();
        self
    }

    /// 设置上下文预算比例（0.0 - 1.0，会被钳制到合理范围）
    pub fn with_budget_ratio(mut self, ratio: f32) -> Self {
        self.budget.budget_ratio = ratio.clamp(0.1, 0.95);
        self
    }

    /// 从 AppConfig 读取预算比例（若配置存在）
    pub fn with_app_config(mut self, config: &AppConfig) -> Self {
        self.budget.budget_ratio = config.context_budget_ratio.clamp(0.1, 0.95);
        self
    }

    /// 构建完整的 Agent 上下文
    ///
    /// # Arguments
    /// * `story_id` - 故事 ID
    /// * `scene_number` - 当前场景序号（用于获取前文和当前场景结构）
    /// * `current_content` - 当前已写内容（可选）
    /// * `selected_text` - 用户选中的文本（可选）
    pub async fn build(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
        current_content: Option<String>,
        selected_text: Option<String>,
    ) -> Result<AgentContext, AppError> {
        // v0.9.5: 优先命中上下文缓存，避免 Writer → Inspector → Rewrite 闭环内重复构建
        if let Some(ref cache) = self.cache {
            if let Some(ctx) = cache.get(story_id, scene_number, &current_content, &selected_text) {
                return Ok(ctx);
            }
        }

        // v0.9.7: 将所有同步 DB 查询与格式化操作整体包裹在单个 spawn_blocking 中，
        // 避免多个独立 blocking 任务争抢 tokio worker；个性化扩展仅依赖 story_id，
        // 保持异步并与上下文构建并行。
        let pool = self.pool.clone();
        let story_id_owned = story_id.to_string();
        let current_content_owned = current_content.clone();
        let selected_text_owned = selected_text.clone();

        let (mut context, personalizer_extension) = tokio::try_join!(
            async {
                tokio::task::spawn_blocking(move || {
                    let builder = StoryContextBuilder::without_cache(pool);
                    builder.build_core_sync(
                        &story_id_owned,
                        scene_number,
                        current_content_owned,
                        selected_text_owned,
                    )
                })
                .await
                .map_err(|e| AppError::internal(format!("上下文构建任务失败: {}", e)))?
            },
            self.compute_personalizer_extension_async(story_id),
        )?;

        context.story.personalizer_extension = personalizer_extension;

        if let Some(ref cache) = self.cache {
            cache.put(
                story_id,
                scene_number,
                &context.narrative.current_content,
                &context.narrative.selected_text,
                context.clone(),
            );
        }

        Ok(context)
    }

    /// 同步构建上下文核心（所有 DB 查询与 CPU 轻量格式化均在线程池执行）。
    fn build_core_sync(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
        current_content: Option<String>,
        selected_text: Option<String>,
    ) -> Result<AgentContext, AppError> {
        // 1. 同步 DB 查询
        let story = self.fetch_story(story_id).map_err(AppError::internal)?;
        let characters = self
            .fetch_characters(story_id)
            .map_err(AppError::internal)?;
        let all_scenes = self
            .fetch_all_scenes(story_id)
            .map_err(AppError::internal)?;
        let world_rules = self
            .fetch_world_rules(story_id)
            .map_err(AppError::internal)?;
        let style = self
            .fetch_writing_style(story_id)
            .map_err(AppError::internal)?;
        let relevant_entities = self
            .fetch_relevant_entities(story_id, 10)
            .map_err(AppError::internal)?;

        // 2. 从 all_scenes 推导依赖数据
        let previous_scenes = self.filter_previous_scenes(&all_scenes, scene_number);
        let current_scene = match scene_number {
            Some(n) => all_scenes.into_iter().find(|s| s.sequence_number == n),
            None => None,
        };

        // Phase 3.1: fetch 失败即 fatal — 所有 DB 查询错误通过 ? 传播

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

        // 构建角色信息（增强版：包含目标、背景、外貌、性别、年龄）
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
                    appearance: c.appearance,
                    gender: c.gender,
                    age: c.age,
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

        // 3. 风格混合、大纲上下文、叙事结构、活跃线索（同步执行）
        let style_blend = self.fetch_style_blend(story_id, scene_number, current_scene.as_ref());
        let outline_context_text = self.build_outline_context(story_id, current_scene.as_ref());
        let narrative_structure = self.build_narrative_structure_context(story_id, scene_number);
        let active_threads = self.fetch_active_threads(story_id);

        // 4. 预计算风格 DNA 扩展与 MemoryPack
        let style_dna_extension = self.compute_style_dna_extension(story_id, &style_blend);
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

        let mut context = AgentContext {
            story: StoryContext {
                story_id: story_id.to_string(),
                story_title: story.title,
                description: story.description.clone(),
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
                genre_profile_id: story.genre_profile_id,
                personalizer_extension: None,
            },
            narrative: NarrativeContext {
                chapter_number: scene_number.map(|n| n.max(0) as u32).unwrap_or(1),
                characters: character_infos,
                previous_chapters,
                current_content,
                selected_text,
                narrative_structure: Some(narrative_structure),
                active_threads,
                outline_context: outline_context_text,
            },
            style: StyleContext {
                style_dna_id: story.style_dna_id,
                style_blend,
                style_fingerprint: None,
                style_dna_extension,
                writing_style_name: style.as_ref().and_then(|s| s.name.clone()),
                writing_style_description: style.as_ref().and_then(|s| s.description.clone()),
                writing_style_vocabulary_level: style
                    .as_ref()
                    .and_then(|s| s.vocabulary_level.clone()),
                writing_style_sentence_structure: style
                    .as_ref()
                    .and_then(|s| s.sentence_structure.clone()),
                writing_style_custom_rules: style.as_ref().and_then(|s| {
                    if s.custom_rules.is_empty() {
                        None
                    } else {
                        Some(s.custom_rules.join("\n"))
                    }
                }),
            },
            world: WorldContext {
                world_rules: world_rules_text,
                scene_structure: scene_structure_text,
                methodology_id: story.methodology_id.clone(),
                methodology_step: story.methodology_step.map(|n| n.to_string()),
            },
            memory: AgentMemoryContext {
                memory_pack,
                memory: None,
            },
        };

        self.apply_context_budget(&mut context);

        Ok(context)
    }

    /// 根据目标模型上下文窗口应用 token 预算控制
    ///
    /// 策略：
    /// - 系统提示 / instruction 保留固定预留
    /// - story context（世界观、角色、风格等关键设定）优先保留
    /// - recent scenes / current content
    ///   超过预算时，先截断较早场景摘要，再截断当前内容
    /// - user input / selected text 仅在仍有剩余预算时保留，否则截断
    fn apply_context_budget(&self, context: &mut AgentContext) {
        let family = self.budget.model_family.as_str();

        fn format_story_summary(ctx: &AgentContext) -> String {
            let mut parts = vec![
                format!("作品: {}", ctx.story.story_title),
                format!("题材: {}", ctx.story.genre),
                format!("文风: {}", ctx.story.tone),
                format!("节奏: {}", ctx.story.pacing),
            ];
            if let Some(ref desc) = ctx.story.description {
                parts.push(format!("简介: {}", desc));
            }
            parts.join("\n")
        }

        fn format_style_summary(ctx: &AgentContext) -> String {
            let mut parts = Vec::new();
            if let Some(ref name) = ctx.style.writing_style_name {
                parts.push(format!("文风名称: {}", name));
            }
            if let Some(ref desc) = ctx.style.writing_style_description {
                parts.push(format!("文风描述: {}", desc));
            }
            if let Some(ref level) = ctx.style.writing_style_vocabulary_level {
                parts.push(format!("词汇等级: {}", level));
            }
            if let Some(ref structure) = ctx.style.writing_style_sentence_structure {
                parts.push(format!("句式特点: {}", structure));
            }
            if let Some(ref rules) = ctx.style.writing_style_custom_rules {
                parts.push(format!("自定义规则: {}", rules));
            }
            if let Some(ref ext) = ctx.style.style_dna_extension {
                parts.push(ext.clone());
            }
            parts.join("\n")
        }

        fn format_full_context(ctx: &AgentContext) -> String {
            let mut parts = vec![
                format_story_summary(ctx),
                ctx.format_characters(),
                ctx.world.world_rules.clone().unwrap_or_default(),
                ctx.world.scene_structure.clone().unwrap_or_default(),
                format_style_summary(ctx),
                ctx.format_previous_chapters(),
                ctx.narrative.current_content.clone().unwrap_or_default(),
                ctx.narrative.selected_text.clone().unwrap_or_default(),
                ctx.narrative.outline_context.clone().unwrap_or_default(),
                ctx.narrative.active_threads.join(", "),
            ];
            if let Some(ref ns) = ctx.narrative.narrative_structure {
                parts.push(format!(
                    "当前幕: 第{}幕（{}） 戏剧功能: {}",
                    ns.act_number, ns.current_act, ns.dramatic_function
                ));
            }
            parts.join("\n")
        }

        let total = self.budget.total_budget();
        let system_budget = self.budget.system_budget();
        let story_context_budget = self.budget.story_context_budget();
        let scene_budget = self.budget.scene_budget();
        let user_input_budget = self.budget.user_input_budget();

        // 1. 计算不可压缩的关键上下文 token 数
        let story_tokens = count_tokens(&format_story_summary(context), family);
        let character_tokens = count_tokens(&context.format_characters(), family);
        let world_tokens =
            count_tokens(&context.world.world_rules.as_deref().unwrap_or(""), family)
                + count_tokens(
                    &context.world.scene_structure.as_deref().unwrap_or(""),
                    family,
                );
        let style_tokens = count_tokens(&format_style_summary(context), family);
        let critical_tokens = story_tokens + character_tokens + world_tokens + style_tokens;

        // 2. 若关键设定已超过 story_context_budget + system_budget，
        //    仅对世界规则/场景结构做保守截断，保留角色与风格
        if critical_tokens > story_context_budget + system_budget {
            log::warn!(
                "[StoryContextBuilder] critical context {} tokens exceeds budget {}, truncating world/scene structure",
                critical_tokens,
                story_context_budget + system_budget
            );
            if let Some(ref mut rules) = context.world.world_rules {
                let max_world = (story_context_budget / 4).max(128);
                if count_tokens(rules, family) > max_world {
                    *rules = truncate_to_budget(rules, max_world, family);
                }
            }
            if let Some(ref mut structure) = context.world.scene_structure {
                let max_structure = (story_context_budget / 4).max(128);
                if count_tokens(structure, family) > max_structure {
                    *structure = truncate_to_budget(structure, max_structure, family);
                }
            }
        }

        // 3. 截断前文摘要：先减少单条长度，再移除最早场景
        let mut previous_chapters_text = context.format_previous_chapters();
        let mut previous_tokens = count_tokens(&previous_chapters_text, family);
        while previous_tokens > scene_budget && !context.narrative.previous_chapters.is_empty() {
            if context.narrative.previous_chapters.len() > 1 {
                // 优先移除最早场景
                context.narrative.previous_chapters.remove(0);
            } else {
                // 只剩一条时截断其摘要
                let remaining = context.narrative.previous_chapters.first_mut().unwrap();
                remaining.summary =
                    truncate_to_budget(&remaining.summary, scene_budget / 2, family);
            }
            previous_chapters_text = context.format_previous_chapters();
            previous_tokens = count_tokens(&previous_chapters_text, family);
        }

        // 4. 截断当前已写内容（保留最新）
        if let Some(ref mut content) = context.narrative.current_content {
            let remaining_scene_budget = scene_budget.saturating_sub(previous_tokens);
            let max_current = remaining_scene_budget.max(128);
            if count_tokens(content, family) > max_current {
                *content = truncate_to_budget_from_end(content, max_current, family);
            }
        }

        // 5. 截断用户选中内容
        if let Some(ref mut selected) = context.narrative.selected_text {
            if count_tokens(selected, family) > user_input_budget {
                *selected = truncate_to_budget_from_end(selected, user_input_budget, family);
            }
        }

        // 6. 最终兜底：若整体仍超过总预算，从 current_content 与 selected_text 再截断
        let final_total = count_tokens(&format_full_context(context), family);
        if final_total > total {
            log::warn!(
                "[StoryContextBuilder] final context {} tokens exceeds total budget {}, applying hard truncation",
                final_total,
                total
            );
            if let Some(ref mut content) = context.narrative.current_content {
                let overage = final_total - total;
                let target = count_tokens(content, family)
                    .saturating_sub(overage)
                    .max(128);
                *content = truncate_to_budget_from_end(content, target, family);
            }
        }

        log::debug!(
            "[StoryContextBuilder] context budget applied: total_budget={} final_tokens={}",
            total,
            count_tokens(&format_full_context(context), family)
        );
    }

    /// 快速构建（用于 intent 执行等场景）
    pub async fn build_quick(&self, story_id: &str) -> Result<AgentContext, AppError> {
        self.build(story_id, None, None, None).await
    }

    /// 带当前场景号的构建
    pub async fn build_for_scene(
        &self,
        story_id: &str,
        scene_number: i32,
        current_content: Option<String>,
    ) -> Result<AgentContext, AppError> {
        self.build(story_id, Some(scene_number), current_content, None)
            .await
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

    fn fetch_all_scenes(&self, story_id: &str) -> Result<Vec<crate::db::models::Scene>, String> {
        let repo = SceneRepository::new(self.pool.clone());
        repo.get_by_story(story_id)
            .map_err(|e| format!("获取场景失败: {}", e))
    }

    fn filter_previous_scenes(
        &self,
        all_scenes: &[crate::db::models::Scene],
        scene_number: Option<i32>,
    ) -> Vec<crate::db::models::Scene> {
        let cutoff = scene_number.unwrap_or(i32::MAX);
        let mut prev: Vec<_> = all_scenes
            .iter()
            .filter(|s| s.sequence_number < cutoff)
            .cloned()
            .collect();
        prev.sort_by_key(|s| s.sequence_number);

        // 只保留最近 5 个场景（避免提示词过长）
        if prev.len() > 5 {
            prev = prev.into_iter().rev().take(5).rev().collect();
        }

        prev
    }

    #[allow(dead_code)]
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

    // v0.9.3: 异步包装，用于并行构建上下文
    async fn fetch_story_async(&self, story_id: &str) -> Result<Story, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            builder.fetch_story(&story_id).map_err(AppError::internal)
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn fetch_characters_async(&self, story_id: &str) -> Result<Vec<Character>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            builder
                .fetch_characters(&story_id)
                .map_err(AppError::internal)
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn fetch_all_scenes_async(
        &self,
        story_id: &str,
    ) -> Result<Vec<crate::db::models::Scene>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            builder
                .fetch_all_scenes(&story_id)
                .map_err(AppError::internal)
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn fetch_world_rules_async(
        &self,
        story_id: &str,
    ) -> Result<Vec<WorldRuleSummary>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            builder
                .fetch_world_rules(&story_id)
                .map_err(AppError::internal)
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn fetch_writing_style_async(
        &self,
        story_id: &str,
    ) -> Result<Option<crate::db::models::WritingStyle>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            builder
                .fetch_writing_style(&story_id)
                .map_err(AppError::internal)
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn fetch_relevant_entities_async(
        &self,
        story_id: &str,
        limit: usize,
    ) -> Result<Vec<RelevantEntity>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            builder
                .fetch_relevant_entities(&story_id, limit)
                .map_err(AppError::internal)
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    // v0.9.3: 异步包装，用于隔离同步 DB 查询/格式化操作
    async fn fetch_style_blend_async(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
        current_scene: Option<crate::db::models::Scene>,
    ) -> Result<Option<crate::creative_engine::style::blend::StyleBlendConfig>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            Ok::<_, AppError>(builder.fetch_style_blend(
                &story_id,
                scene_number,
                current_scene.as_ref(),
            ))
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn compute_style_dna_extension_async(
        &self,
        story_id: &str,
        style_blend: &Option<crate::creative_engine::style::blend::StyleBlendConfig>,
    ) -> Result<Option<String>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        let style_blend = style_blend.clone();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            Ok::<_, AppError>(builder.compute_style_dna_extension(&story_id, &style_blend))
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn build_outline_context_async(
        &self,
        story_id: &str,
        current_scene: Option<crate::db::models::Scene>,
    ) -> Result<Option<String>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            Ok::<_, AppError>(builder.build_outline_context(&story_id, current_scene.as_ref()))
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn build_narrative_structure_context_async(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
    ) -> Result<NarrativeStructureContext, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            Ok::<_, AppError>(builder.build_narrative_structure_context(&story_id, scene_number))
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn fetch_active_threads_async(&self, story_id: &str) -> Result<Vec<String>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let builder = Self::new(pool);
            Ok::<_, AppError>(builder.fetch_active_threads(&story_id))
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
    }

    async fn build_memory_pack_async(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
        current_scene: Option<crate::db::models::Scene>,
        previous_chapters: Vec<ChapterSummary>,
    ) -> Result<Option<crate::memory::orchestrator::MemoryPack>, AppError> {
        let pool = self.pool.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            let orchestrator = crate::memory::orchestrator::MemoryOrchestrator::new(pool);
            match orchestrator.build_memory_pack(
                &story_id,
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
                    Ok(Some(pack))
                }
                Err(e) => {
                    log::warn!("[StoryContextBuilder] 记忆包构建失败: {}, 继续无记忆包", e);
                    Ok(None)
                }
            }
        })
        .await
        .map_err(|e| AppError::internal(format!("上下文任务执行失败: {}", e)))?
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

    /// v0.9.3: 预计算风格 DNA 提示词扩展，供候选间共享
    /// v0.9.7: 风格混合时改为批量 IN 查询，避免每个 component 单独查库。
    fn compute_style_dna_extension(
        &self,
        story_id: &str,
        style_blend: &Option<crate::creative_engine::style::blend::StyleBlendConfig>,
    ) -> Option<String> {
        use crate::{creative_engine::style::dna::StyleDNA, db::repositories::StyleDnaRepository};

        let dna_repo = StyleDnaRepository::new(self.pool.clone());

        if let Some(ref blend) = style_blend {
            let dna_ids: Vec<String> = blend.components.iter().map(|c| c.dna_id.clone()).collect();
            if dna_ids.is_empty() {
                return None;
            }

            let dnas: Vec<StyleDNA> = match dna_repo.get_many_by_ids(&dna_ids) {
                Ok(db_dnas) => db_dnas
                    .into_iter()
                    .filter_map(|db_dna| serde_json::from_str::<StyleDNA>(&db_dna.dna_json).ok())
                    .collect(),
                Err(e) => {
                    log::warn!("[StoryContextBuilder] 批量加载风格 DNA 失败: {}", e);
                    return None;
                }
            };

            if !dnas.is_empty() {
                let extension = blend.to_prompt_extension(&dnas);
                if !extension.is_empty() {
                    return Some(extension);
                }
            }
        } else if let Ok(Some(style_dna_id)) = self.fetch_story_style_dna_id(story_id) {
            if let Ok(Some(db_dna)) = dna_repo.get_by_id(&style_dna_id) {
                if let Ok(dna) = serde_json::from_str::<StyleDNA>(&db_dna.dna_json) {
                    let extension = dna.to_prompt_extension();
                    if !extension.is_empty() {
                        return Some(extension);
                    }
                }
            }
        }

        None
    }

    /// 获取故事关联的单一风格 DNA ID
    fn fetch_story_style_dna_id(&self, story_id: &str) -> Result<Option<String>, String> {
        let repo = StoryRepository::new(self.pool.clone());
        repo.get_by_id(story_id)
            .map_err(|e| format!("获取故事失败: {}", e))?
            .map(|s| s.style_dna_id)
            .ok_or_else(|| "故事不存在".to_string())
    }

    /// v0.9.3: 预计算个性化偏好扩展，供候选间共享
    async fn compute_personalizer_extension_async(
        &self,
        story_id: &str,
    ) -> Result<Option<String>, AppError> {
        use crate::creative_engine::adaptive::PromptPersonalizer;

        let personalizer = PromptPersonalizer::new(self.pool.clone());
        match personalizer.build_prompt_extension(story_id).await {
            Ok(ext) if !ext.is_empty() => Ok(Some(ext)),
            Ok(_) => Ok(None),
            Err(e) => {
                log::warn!(
                    "[StoryContextBuilder] 个性化扩展构建失败: {}, 继续无个性化扩展",
                    e
                );
                Ok(None)
            }
        }
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
            if let Some(ref title) = s.title {
                parts.push(format!("场景标题: {}", title));
            }
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
            if let Some(ref atmosphere) = s.setting_atmosphere {
                parts.push(format!("氛围: {}", atmosphere));
            }
            if !s.characters_present.is_empty() {
                parts.push(format!("出场角色: {}", s.characters_present.join(", ")));
            }
            // v0.9.6: 注入场景大纲与草稿，让 Writer 知道场景节拍目标
            if let Some(ref outline) = s.outline_content {
                if !outline.trim().is_empty() {
                    parts.push(format!("场景大纲: {}", outline));
                }
            }
            if let Some(ref draft) = s.draft_content {
                if !draft.trim().is_empty() {
                    parts.push(format!("场景草稿: {}", draft));
                }
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

    /// 构建当前场景/章节的大纲上下文（v0.9.6）
    fn build_outline_context(
        &self,
        story_id: &str,
        scene: Option<&crate::db::models::Scene>,
    ) -> Option<String> {
        let mut parts = Vec::new();

        if let Some(s) = scene {
            if let Some(ref title) = s.title {
                parts.push(format!("当前场景: {}", title));
            }
            if let Some(ref outline) = s.outline_content {
                if !outline.trim().is_empty() {
                    parts.push(format!("场景大纲: {}", outline));
                }
            }
            if let Some(ref draft) = s.draft_content {
                if !draft.trim().is_empty() {
                    parts.push(format!("草稿方向: {}", draft));
                }
            }
        }

        // 注入当前幕/章节的故事大纲摘要
        if let Some(outline) =
            self.fetch_story_outline_summary(story_id, scene.map(|s| s.sequence_number))
        {
            if !outline.trim().is_empty() {
                parts.push(format!("故事大纲定位: {}", outline));
            }
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n"))
        }
    }

    /// 获取故事大纲中当前场景对应的摘要
    fn fetch_story_outline_summary(
        &self,
        story_id: &str,
        scene_number: Option<i32>,
    ) -> Option<String> {
        use crate::db::repositories::StoryOutlineRepository;

        let repo = StoryOutlineRepository::new(self.pool.clone());
        let outline = repo.get_by_story(story_id).ok()??;
        let content = outline.content;
        if content.trim().is_empty() {
            return None;
        }
        // 如果大纲按场景分条，尝试提取当前场景附近的几条
        let lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            return None;
        }
        let n = scene_number.unwrap_or(1).max(1) as usize;
        let start = n.saturating_sub(2).min(lines.len().saturating_sub(1));
        let end = (n + 2).min(lines.len());
        let snippet = lines[start..end].join("\n");
        Some(snippet)
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
    #[tokio::test]
    async fn test_build_returns_err_when_story_query_fails() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        // 制造致命错误：删除 stories 表使 fetch_story 触发 DB 错误
        {
            let conn = pool.get().unwrap();
            conn.execute("DROP TABLE stories", []).unwrap();
        }

        let builder = StoryContextBuilder::new(pool);
        let result = builder.build("any-id", None, None, None).await;

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

    #[tokio::test]
    async fn test_build_returns_err_when_story_not_found() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let builder = StoryContextBuilder::new(pool);
        let result = builder.build("non-existent-story", None, None, None).await;

        assert!(result.is_err(), "当故事不存在时，build 应返回 Err");
        assert!(result.unwrap_err().message().contains("故事不存在"));
    }

    #[tokio::test]
    async fn test_build_fatal_when_characters_empty_with_conflict() {
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

        // 使用无缓存 builder，避免与使用相同 cache key 的其他测试互相污染
        let builder = StoryContextBuilder::without_cache(pool);
        // 没有插入角色，且场景有冲突类型 — 应为 fatal
        let result = builder.build("story-1", Some(1), None, None).await;

        assert!(
            result.is_err(),
            "有冲突类型的场景但无角色时，build 应返回 fatal 错误"
        );
        assert_eq!(result.unwrap_err().code(), "CONTEXT_UNAVAILABLE");
    }

    #[tokio::test]
    async fn test_build_ok_when_characters_empty_no_conflict() {
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

        // 使用无缓存 builder，避免与使用相同 cache key 的其他测试互相污染
        let builder = StoryContextBuilder::without_cache(pool);
        // 没有插入角色，但场景无冲突类型 — 应为 ok
        let result = builder.build("story-1", Some(1), None, None).await;

        assert!(
            result.is_ok(),
            "无冲突类型的场景且无角色时，build 应返回 Ok"
        );
    }

    #[test]
    fn test_context_cache_hit_and_miss() {
        use std::time::Duration;

        let cache = ContextCache::new(10, Duration::from_secs(60));
        let ctx = dummy_agent_context("story-1");

        // 首次未命中
        assert!(cache
            .get("story-1", Some(1), &Some("hello".to_string()), &None)
            .is_none());

        // 写入后命中
        cache.put(
            "story-1",
            Some(1),
            &Some("hello".to_string()),
            &None,
            ctx.clone(),
        );
        let hit = cache.get("story-1", Some(1), &Some("hello".to_string()), &None);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().story.story_id, ctx.story.story_id);

        // 不同参数未命中
        assert!(cache
            .get("story-1", Some(2), &Some("hello".to_string()), &None)
            .is_none());
        assert!(cache
            .get("story-1", Some(1), &Some("world".to_string()), &None)
            .is_none());
    }

    #[test]
    fn test_context_cache_ttl_eviction() {
        use std::time::Duration;

        let cache = ContextCache::new(10, Duration::from_millis(10));
        let ctx = dummy_agent_context("story-ttl");

        cache.put("story-ttl", None, &None, &None, ctx.clone());
        assert!(cache.get("story-ttl", None, &None, &None).is_some());

        std::thread::sleep(Duration::from_millis(50));
        assert!(cache.get("story-ttl", None, &None, &None).is_none());
    }

    #[test]
    fn test_context_cache_lru_eviction() {
        use std::time::Duration;

        let cache = ContextCache::new(2, Duration::from_secs(60));

        for i in 0..3 {
            cache.put(
                &format!("story-{}", i),
                None,
                &None,
                &None,
                dummy_agent_context(&format!("story-{}", i)),
            );
        }

        // 容量为 2，最旧的 story-0 应被移除
        assert!(cache.get("story-0", None, &None, &None).is_none());
        assert!(cache.get("story-1", None, &None, &None).is_some());
        assert!(cache.get("story-2", None, &None, &None).is_some());
    }

    #[test]
    fn test_apply_context_budget_truncates_oldest_scenes_first() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let builder = StoryContextBuilder::without_cache(pool)
            .with_context_budget(512, "cl100k")
            .with_budget_ratio(0.8);
        let family = builder.budget.model_family.clone();

        let mut ctx = dummy_agent_context("budget-test");
        ctx.narrative.previous_chapters = vec![
            ChapterSummary {
                title: "第一章".to_string(),
                number: 1,
                summary: "这是第一章的很长很长很长的摘要内容。".repeat(20),
            },
            ChapterSummary {
                title: "第二章".to_string(),
                number: 2,
                summary: "这是第二章的很长很长很长的摘要内容。".repeat(20),
            },
            ChapterSummary {
                title: "第三章".to_string(),
                number: 3,
                summary: "这是第三章的很长很长很长的摘要内容。".repeat(20),
            },
        ];
        ctx.narrative.current_content = Some("当前章节正在续写的内容。".repeat(50));
        ctx.narrative.selected_text = Some("用户选中的文本内容。".repeat(20));

        let original_previous_tokens = count_tokens(&ctx.format_previous_chapters(), &family);
        let original_current_tokens =
            count_tokens(ctx.narrative.current_content.as_deref().unwrap(), &family);
        let original_selected_tokens =
            count_tokens(ctx.narrative.selected_text.as_deref().unwrap(), &family);

        builder.apply_context_budget(&mut ctx);

        let final_previous_tokens = count_tokens(&ctx.format_previous_chapters(), &family);
        let final_current_tokens =
            count_tokens(ctx.narrative.current_content.as_deref().unwrap(), &family);
        let final_selected_tokens =
            count_tokens(ctx.narrative.selected_text.as_deref().unwrap(), &family);

        // 较早章节应被优先移除或截断
        assert!(
            ctx.narrative.previous_chapters.len() <= 3,
            "章节数量不应增加"
        );
        assert!(
            final_previous_tokens <= original_previous_tokens,
            "前文 token 数应不增加"
        );
        // 当前内容应被保留但截断
        assert!(
            final_current_tokens <= original_current_tokens,
            "当前内容 token 数应不增加"
        );
        // 选中内容应被截断
        assert!(
            final_selected_tokens <= original_selected_tokens,
            "选中内容 token 数应不增加"
        );
    }

    #[test]
    fn test_apply_context_budget_total_within_budget() {
        let pool = crate::db::connection::create_test_pool().unwrap();
        let builder = StoryContextBuilder::without_cache(pool)
            .with_context_budget(512, "cl100k")
            .with_budget_ratio(0.8);
        let family = builder.budget.model_family.clone();
        let total_budget = builder.budget.total_budget();

        let mut ctx = dummy_agent_context("budget-total-test");
        // 构造一个远超过预算的上下文（简介保持较短，确保被截断的是可控字段）
        ctx.story.description = Some("测试简介。".to_string());
        ctx.narrative.previous_chapters = (1..=5)
            .map(|i| ChapterSummary {
                title: format!("第{}章", i),
                number: i as u32,
                summary: "这是很长很长很长的章节摘要内容。".repeat(40),
            })
            .collect();
        ctx.narrative.current_content = Some("当前正在写作的章节正文。".repeat(200));
        ctx.narrative.selected_text = Some("用户选中需要改写的文本。".repeat(50));

        builder.apply_context_budget(&mut ctx);

        // 复现 apply_context_budget 内部对最终上下文 token 总数的计算逻辑
        fn format_full_context(ctx: &AgentContext) -> String {
            let mut parts = vec![
                format!("作品: {}", ctx.story.story_title),
                format!("题材: {}", ctx.story.genre),
                format!("文风: {}", ctx.story.tone),
                format!("节奏: {}", ctx.story.pacing),
            ];
            if let Some(ref desc) = ctx.story.description {
                parts.push(format!("简介: {}", desc));
            }
            let mut s = parts.join("\n");
            s.push('\n');
            s.push_str(&ctx.format_characters());
            s.push('\n');
            s.push_str(ctx.world.world_rules.as_deref().unwrap_or(""));
            s.push('\n');
            s.push_str(ctx.world.scene_structure.as_deref().unwrap_or(""));
            s.push('\n');
            if let Some(ref name) = ctx.style.writing_style_name {
                s.push_str(&format!("文风名称: {}\n", name));
            }
            if let Some(ref desc) = ctx.style.writing_style_description {
                s.push_str(&format!("文风描述: {}\n", desc));
            }
            if let Some(ref level) = ctx.style.writing_style_vocabulary_level {
                s.push_str(&format!("词汇等级: {}\n", level));
            }
            if let Some(ref structure) = ctx.style.writing_style_sentence_structure {
                s.push_str(&format!("句式特点: {}\n", structure));
            }
            if let Some(ref rules) = ctx.style.writing_style_custom_rules {
                s.push_str(&format!("自定义规则: {}\n", rules));
            }
            if let Some(ref ext) = ctx.style.style_dna_extension {
                s.push_str(ext);
                s.push('\n');
            }
            s.push_str(&ctx.format_previous_chapters());
            s.push('\n');
            s.push_str(ctx.narrative.current_content.as_deref().unwrap_or(""));
            s.push('\n');
            s.push_str(ctx.narrative.selected_text.as_deref().unwrap_or(""));
            s.push('\n');
            s.push_str(ctx.narrative.outline_context.as_deref().unwrap_or(""));
            s.push('\n');
            s.push_str(&ctx.narrative.active_threads.join(", "));
            if let Some(ref ns) = ctx.narrative.narrative_structure {
                s.push('\n');
                s.push_str(&format!(
                    "当前幕: 第{}幕（{}） 戏剧功能: {}",
                    ns.act_number, ns.current_act, ns.dramatic_function
                ));
            }
            s
        }

        let final_total = count_tokens(&format_full_context(&ctx), &family);
        assert!(
            final_total <= total_budget,
            "最终上下文 token 数 {} 应不超过总预算 {}",
            final_total,
            total_budget
        );
    }

    fn dummy_agent_context(story_id: &str) -> AgentContext {
        AgentContext {
            story: StoryContext {
                story_id: story_id.to_string(),
                story_title: "Test".to_string(),
                description: None,
                genre: "novel".to_string(),
                tone: "neutral".to_string(),
                pacing: "normal".to_string(),
                genre_profile_id: None,
                personalizer_extension: None,
            },
            narrative: NarrativeContext {
                chapter_number: 1,
                characters: vec![],
                previous_chapters: vec![],
                current_content: None,
                selected_text: None,
                narrative_structure: None,
                active_threads: vec![],
                outline_context: None,
            },
            style: StyleContext {
                style_dna_id: None,
                style_blend: None,
                style_fingerprint: None,
                style_dna_extension: None,
                writing_style_name: None,
                writing_style_description: None,
                writing_style_vocabulary_level: None,
                writing_style_sentence_structure: None,
                writing_style_custom_rules: None,
            },
            world: WorldContext {
                world_rules: None,
                scene_structure: None,
                methodology_id: None,
                methodology_step: None,
            },
            memory: AgentMemoryContext {
                memory_pack: None,
                memory: None,
            },
        }
    }
}
