//! WriteTimeBundle - 时间线 1（写作时刻）的最小可行约束包
//!
//! 设计依据：docs/plans/2026-06-14-time-sliced-intervention-design.md 模块 8
//!
//! Phase 0 实证结论（2026-06-14，qwen3.6-35b）：
//! - 最小约束 vs 全量资产平均质量差距仅 7.9%（< 30% 阈值），架构成立。
//! - S1 玄幻：最小约束反而反超全量（A=110 vs B=99），因为全量 prompt 太长导致
//!   模型忽略了世界观红线。教训：红线必须最前最突出。
//! - S3 都市：全量大胜最小约束（B=125 vs A=99，差 26
//!   分），因为都市题材吃风格细节。 教训：风格片段需按题材自适应纳入。
//!
//! 因此本模块实现两条改进：
//! 1. 红线突出注入：to_prompt() 输出时红线在最前、加粗强调。
//! 2. 题材自适应：按 stories.genre 决定是否纳入风格片段。

use crate::{
    book_deconstruction::models::ReferenceScene,
    creative_engine::asset_snapshot::CreativeAssetSnapshot,
    db::{
        Character, CharacterRepository, DbPool, GenreProfileRepository, SceneRepository,
        StoryContractRepository, StyleDnaRepository,
    },
    domain::contracts::RuntimeContract,
    story_system::StorySystemEngine,
};

// ==================== 数据结构 ====================

/// 写作时刻的最小约束包。
///
/// 只含"一次写对基本盘"必需的资产。审计用资产（Inspector/记忆比对）
/// 不在此处——它们在时间线 2（AuditExecutor）异步加载。
///
/// P1-1（审计报告）：在保持速度优先的前提下，接入精选资产子集，
/// 解决 TimeSliced "资产黑洞"问题——伏笔状态、叙事阶段、主导风格摘要、
/// 叙事四元组此前在默认续写路径完全不进入 LLM prompt。
pub struct WriteTimeBundle {
    /// 合同红线：MASTER_SETTING 核心世界观约束
    pub contract_redlines: Option<String>,
    /// 当前章节出场角色核心（姓名 + 当前状态）
    pub core_characters: Vec<CoreCharacter>,
    /// 当前 scene 大纲（dramatic_goal + conflict_type + setting）
    pub scene_outline: Option<SceneOutline>,
    /// GenreProfile 反模式清单
    pub genre_antipatterns: Vec<String>,
    /// 风格 DNA 片段（题材自适应，部分题材为 None）
    pub style_slice: Option<String>,
    /// 故事基础元信息
    pub story_meta: StoryMeta,
    /// 题材分类（决定 style_slice 是否纳入）
    pub genre_category: GenreCategory,
    /// P1-1: 叙事阶段指导（一行，来自 CanonicalStateManager）
    pub narrative_phase_guidance: Option<String>,
    /// P1-1: 待回收伏笔（top 3，每条一行）
    pub pending_foreshadowings: Vec<String>,
    /// P1-1: 逾期伏笔（top 1，每条一行，带警告）
    pub overdue_foreshadowings: Vec<String>,
    /// P1-1: 主导风格一句话摘要（来自 StyleDNA，全题材纳入）
    pub style_dna_summary: Option<String>,
    /// P1-1: 叙事四元组渲染文本（来自 task.parameters，由调用方设置）
    pub narrative_quartet: Option<String>,
    // v0.22.0: 解决 TimeSliced "资产黑洞"（Phase A）
    // 此前默认续写路径不注入 StyleDNA 六维量化指标/方法论规则/题材画像全字段/写作策略
    /// 风格 DNA 完整六维指标
    pub style_dna_extension: Option<String>,
    /// 方法论约束（当前步骤的完整规则）
    pub methodology_extension: Option<String>,
    /// 题材画像策略（core_tone + pacing_strategy + reference_tables +
    /// typical_structure）
    pub genre_profile_strategy: Option<String>,
    /// Phase 4: 次要题材画像策略（复合题材时从 selected_strategy 透传）
    pub secondary_genre_profile_strategy: Option<String>,
    /// 写作策略约束
    pub writing_strategy_constraints: Option<String>,
    /// v0.22.5: Story System 运行时合同（MASTER_SETTING + 当前章节合同）
    pub runtime_contract: Option<RuntimeContract>,
    /// Phase 3.1: 参考场景 few-shots（来自关联 reference_book 的相似场景）
    pub reference_scene_fewshots: Vec<ReferenceSceneFewShot>,
}

pub struct CoreCharacter {
    pub name: String,
    pub identity: Option<String>,
    pub physical_state: Option<String>,
    pub mental_state: Option<String>,
    pub location: Option<String>,
    pub personality: Option<String>,
}

pub struct SceneOutline {
    pub dramatic_goal: Option<String>,
    pub conflict_type: Option<String>,
    pub external_pressure: Option<String>,
    pub setting_location: Option<String>,
}

/// Phase 3.1: 参考场景 few-shot（用于拆书功能关联写作）。
#[derive(Debug, Clone)]
pub struct ReferenceSceneFewShot {
    pub title: String,
    pub summary: String,
    pub content_snippet: String,
    pub similarity: f32,
}

pub struct StoryMeta {
    pub title: String,
    pub genre: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub description: Option<String>,
}

/// 题材分类——决定风格片段是否纳入（Phase 0 实证）。
#[derive(Debug, Clone, PartialEq)]
pub enum GenreCategory {
    /// 都市/情感/现实主义：风格细节是质量关键，纳入轻量风格片段
    RealismEmotional,
    /// 玄幻/仙侠/科幻：红线守严 > 风格约束，不纳入风格片段
    Speculative,
    /// 悬疑/推理：逻辑链是关键
    Mystery,
    /// 未知/默认：保守策略，不纳入
    Unknown,
}

impl GenreCategory {
    /// 是否应纳入轻量风格片段（Phase 0 实证）。
    pub fn include_style_slice(&self) -> bool {
        matches!(
            self,
            GenreCategory::RealismEmotional | GenreCategory::Mystery
        )
    }

    /// 根据 genre 字符串推断题材分类。
    /// 匹配逻辑宽松（包含关键词即命中），未命中归 Unknown。
    pub fn from_genre(genre: Option<&str>) -> Self {
        let g = match genre {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => return GenreCategory::Unknown,
        };
        let g_lower = g.to_lowercase();
        // 现实/情感类
        let realism_keywords = [
            "都市", "现实", "情感", "言情", "青春", "校园", "职场", "家庭", "年代", "生活", "治愈",
            "日常", "urban", "realism", "romance",
        ];
        if realism_keywords.iter().any(|k| g_lower.contains(k)) {
            return GenreCategory::RealismEmotional;
        }
        // 悬疑/推理类
        let mystery_keywords = [
            "悬疑",
            "推理",
            "侦探",
            "犯罪",
            "惊悚",
            "mystery",
            "thriller",
            "detective",
        ];
        if mystery_keywords.iter().any(|k| g_lower.contains(k)) {
            return GenreCategory::Mystery;
        }
        // 架空/幻想类
        let speculative_keywords = [
            "玄幻", "仙侠", "科幻", "奇幻", "修真", "末世", "网游", "灵异", "fantasy", "scifi",
            "sci-fi", "xianxia",
        ];
        if speculative_keywords.iter().any(|k| g_lower.contains(k)) {
            return GenreCategory::Speculative;
        }
        GenreCategory::Unknown
    }
}

// ==================== 加载 ====================

impl WriteTimeBundle {
    /// 从 DB 加载最小约束包。全部走 spawn_blocking（由调用方包裹）。
    ///
    /// `style_slice_override` 允许调用方传入预生成的风格片段（来自 StyleDna）。
    /// 若为 None，则按 genre_category.include_style_slice() 决定是否留空。
    pub fn load_sync(
        pool: &DbPool,
        story_id: &str,
        chapter_number: i32,
        style_slice_override: Option<String>,
        secondary_genre_profile_ids: Option<Vec<String>>,
    ) -> Result<Self, String> {
        // 1. 故事元信息
        let story_repo = crate::db::StoryRepository::new(pool.clone());
        let story = story_repo
            .get_by_id(story_id)
            .map_err(|e| format!("查询故事失败: {}", e))?
            .ok_or_else(|| format!("故事 {} 不存在", story_id))?;

        let genre_category = GenreCategory::from_genre(story.genre.as_deref());

        let story_meta = StoryMeta {
            title: story.title.clone(),
            genre: story.genre.clone(),
            tone: story.tone.clone(),
            pacing: story.pacing.clone(),
            description: story.description.clone(),
        };

        // 2. 合同红线（MASTER_SETTING）
        let contract_repo = StoryContractRepository::new(pool.clone());
        let contract_redlines = match contract_repo.get_by_story(story_id) {
            Ok(contracts) => {
                let master = contracts
                    .iter()
                    .find(|c| c.contract_type == "MASTER_SETTING");
                master.map(|c| c.contract_json.clone())
            }
            Err(e) => {
                log::warn!("[WriteTimeBundle] 查询合同失败: {}", e);
                None
            }
        };

        // v0.22.5: 加载运行时合同（写前真源）
        let runtime_contract = match StorySystemEngine::new(pool.clone())
            .get_runtime_contract(story_id, chapter_number)
        {
            Ok(rc) => Some(rc),
            Err(e) => {
                log::debug!(
                    "[WriteTimeBundle] 运行时合同未加载: story={} chapter={} err={}",
                    story_id,
                    chapter_number,
                    e
                );
                None
            }
        };

        // 3. 角色核心
        let char_repo = CharacterRepository::new(pool.clone());
        let core_characters: Vec<CoreCharacter> = match char_repo.get_by_story(story_id) {
            Ok(chars) => chars
                .iter()
                .map(|c: &Character| CoreCharacter {
                    name: c.name.clone(),
                    identity: c.background.clone(),
                    physical_state: c.cs_physical_state.clone(),
                    mental_state: c.cs_mental_state.clone(),
                    location: c.cs_location.clone(),
                    personality: c.personality.clone(),
                })
                .collect(),
            Err(e) => {
                log::warn!("[WriteTimeBundle] 查询角色失败: {}", e);
                vec![]
            }
        };

        // 4. 场景大纲
        let scene_repo = SceneRepository::new(pool.clone());
        let scene_outline = match scene_repo.get_by_story(story_id) {
            Ok(scenes) => {
                let scene = scenes.iter().find(|s| s.sequence_number == chapter_number);
                scene.map(|s| SceneOutline {
                    dramatic_goal: s.dramatic_goal.clone(),
                    conflict_type: s.conflict_type.as_ref().map(|c| format!("{:?}", c)),
                    external_pressure: s.external_pressure.clone(),
                    setting_location: s.setting_location.clone(),
                })
            }
            Err(e) => {
                log::warn!("[WriteTimeBundle] 查询场景失败: {}", e);
                None
            }
        };

        // Phase 3.1: 加载参考场景 few-shots（若故事关联了参考书籍）。
        // 当前 load_sync 为同步上下文，无法直接调用 LanceVectorStore 的异步向量搜索，
        // 因此退化为基于场景大纲与参考场景文本的关键词重叠排序，取 top 3。
        let reference_scene_fewshots = match story.reference_book_id.as_deref() {
            Some(book_id) if !book_id.is_empty() => {
                Self::load_reference_scene_fewshots_sync(pool, book_id, &scene_outline)
                    .unwrap_or_else(|e| {
                        log::warn!("[WriteTimeBundle] 加载参考场景失败: {}", e);
                        vec![]
                    })
            }
            _ => vec![],
        };

        // 5. GenreProfile 反模式清单
        let genre_repo = GenreProfileRepository::new(pool.clone());
        let genre_antipatterns = match &story.genre {
            Some(genre_name) => match genre_repo.get_by_name(genre_name) {
                Ok(Some(profile)) => parse_antipatterns(&profile.anti_patterns_json),
                _ => vec![],
            },
            None => vec![],
        };

        // 6. 风格片段（题材自适应）
        let style_slice = if genre_category.include_style_slice() {
            style_slice_override
        } else {
            None
        };

        // v0.22.0: 加载完整 StyleDNA 六维指标
        let style_dna_extension = match story.style_dna_id.as_deref() {
            Some(dna_id) if !dna_id.is_empty() => {
                let dna_repo = StyleDnaRepository::new(pool.clone());
                match dna_repo.get_by_id(dna_id) {
                    Ok(Some(dna)) => {
                        match serde_json::from_str::<crate::domain::style::StyleDNA>(&dna.dna_json)
                        {
                            Ok(dna_obj) => Some(dna_obj.to_prompt_extension()),
                            Err(e) => {
                                log::warn!("[WriteTimeBundle] StyleDNA 解析失败: {}", e);
                                None
                            }
                        }
                    }
                    _ => None,
                }
            }
            _ => None,
        };

        // v0.22.0: 加载方法论扩展
        let methodology_extension = match story.methodology_id.as_deref() {
            Some(mid) if !mid.is_empty() => {
                let step = story.methodology_step.unwrap_or(1);
                // v0.22.1: 按 methodology_id 动态选择 prompt ID
                let (prompt_id, label) = match mid {
                    "snowflake" => (
                        format!("methodology_snowflake_step{}", step),
                        format!("雪花法 第{}步", step),
                    ),
                    "hero_journey" => (
                        "methodology_hero_journey".to_string(),
                        "英雄之旅".to_string(),
                    ),
                    "scene_structure" => (
                        "methodology_scene_structure".to_string(),
                        "场景结构".to_string(),
                    ),
                    "character_depth" => (
                        "methodology_character_depth".to_string(),
                        "人物深度".to_string(),
                    ),
                    "high_density_world_building" => {
                        let (prompt_id, label) = match step {
                            2 => ("methodology_hdwb_expansion", "高密度世界构建-状态网扩张"),
                            3 => ("methodology_hdwb_convergence", "高密度世界构建-多线交织"),
                            4 => ("methodology_hdwb_iteration", "高密度世界构建-密度迭代"),
                            _ => ("methodology_hdwb_seed", "高密度世界构建-最小世界种子"),
                        };
                        (prompt_id.to_string(), label.to_string())
                    }
                    _ => (String::new(), String::new()),
                };
                if prompt_id.is_empty() {
                    None
                } else if let Some(content) =
                    crate::prompts::registry::resolve_prompt_default(&prompt_id)
                {
                    Some(format!("【创作方法论（{}）】\n{}", label, content))
                } else {
                    None
                }
            }
            _ => None,
        };

        // v0.22.0: 加载 GenreProfile 完整策略
        let genre_profile_strategy = {
            let genre_name = story.genre.as_deref().unwrap_or("");
            if genre_name.is_empty() {
                None
            } else {
                let genre_repo2 = GenreProfileRepository::new(pool.clone());
                match genre_repo2.get_by_name(genre_name) {
                    Ok(Some(profile)) => {
                        let mut parts = vec![];
                        if let Some(ref tone) = profile.core_tone {
                            parts.push(format!("基调：{}", tone));
                        }
                        if let Some(ref pacing) = profile.pacing_strategy {
                            parts.push(format!("节奏策略：{}", pacing));
                        }
                        if !parts.is_empty() {
                            Some(format!(
                                "【体裁画像策略（{}）】\n{}",
                                genre_name,
                                parts.join("\n")
                            ))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
        };

        // Phase 4: 加载次要题材画像策略（复合题材资产补强）
        let secondary_genre_profile_strategy = {
            let ids = secondary_genre_profile_ids.unwrap_or_default();
            if ids.is_empty() {
                None
            } else {
                let genre_repo3 = GenreProfileRepository::new(pool.clone());
                let mut summaries = vec![];
                for id in ids {
                    if let Ok(Some(profile)) = genre_repo3.get_by_id(&id) {
                        let mut parts = vec![];
                        if let Some(ref tone) = profile.core_tone {
                            parts.push(format!("基调：{}", tone));
                        }
                        if let Some(ref pacing) = profile.pacing_strategy {
                            parts.push(format!("节奏策略：{}", pacing));
                        }
                        if !parts.is_empty() {
                            summaries.push(format!(
                                "- {}（{}）：{}",
                                profile.genre_name,
                                profile.canonical_name,
                                parts.join("，")
                            ));
                        }
                    }
                }
                if !summaries.is_empty() {
                    Some(format!(
                        "【次要题材画像补充（复合题材）】\n{}\n\n续写时需同时满足主、次题材画像的核心基调与节奏策略；若两者冲突，以主题材画像为准，但应保留次题材画像的独特氛围。",
                        summaries.join("\n")
                    ))
                } else {
                    None
                }
            }
        };

        // v0.22.0: 加载写作策略约束
        let writing_strategy_constraints = {
            // 使用默认策略（后续可通过参数传入覆盖）
            Some(
                "【写作策略约束】\n运行模式：标准\n冲突强度：0.5\n叙事节奏：正常\nAI 自由度：0.5"
                    .to_string(),
            )
        };

        // P1-1: 精选资产子集——解决 TimeSliced "资产黑洞"
        // P3-3: 使用统一资产注入网关 CreativeAssetSnapshot，消除重复加载逻辑。
        let snapshot =
            CreativeAssetSnapshot::load_sync(pool, story_id, story.style_dna_id.as_deref());

        let narrative_phase_guidance = snapshot.narrative_phase_guidance();
        let pending_foreshadowings = snapshot.pending_foreshadowings(3);
        let overdue_foreshadowings = snapshot.overdue_foreshadowings(1);
        let style_dna_summary = snapshot.style_dna_summary;

        Ok(WriteTimeBundle {
            contract_redlines,
            core_characters,
            scene_outline,
            genre_antipatterns,
            style_slice,
            story_meta,
            genre_category,
            narrative_phase_guidance,
            pending_foreshadowings,
            overdue_foreshadowings,
            style_dna_summary,
            narrative_quartet: None, // 由调用方（orchestrator）从 task.parameters 设置
            style_dna_extension,
            methodology_extension,
            genre_profile_strategy,
            secondary_genre_profile_strategy,
            writing_strategy_constraints,
            runtime_contract,
            reference_scene_fewshots,
        })
    }

    /// Phase 3.1: 同步加载参考场景 few-shots。
    ///
    /// 当前实现为同步上下文下的降级方案：基于当前场景大纲与参考场景文本的
    /// 关键词重叠进行排序，返回 top 3。若未来需要向量搜索，可在外部先异步
    /// 计算 embedding 再传入，或把 load_sync 改造为 async。
    fn load_reference_scene_fewshots_sync(
        pool: &DbPool,
        book_id: &str,
        scene_outline: &Option<SceneOutline>,
    ) -> Result<Vec<ReferenceSceneFewShot>, Box<dyn std::error::Error>> {
        use crate::book_deconstruction::repository::ReferenceSceneRepository;

        let scene_repo = ReferenceSceneRepository::new(pool.clone());
        let scenes = scene_repo.get_reference_scenes_by_book(book_id)?;
        if scenes.is_empty() {
            return Ok(vec![]);
        }

        let query_text = Self::scene_outline_query_text(scene_outline);
        if query_text.trim().is_empty() {
            // 无场景大纲时按顺序取前 3 个作为兜底
            return Ok(scenes
                .into_iter()
                .take(3)
                .map(Self::reference_scene_to_fewshot)
                .collect());
        }

        let query_tokens = tokenize_text(&query_text);

        let mut scored: Vec<(f32, ReferenceScene)> = scenes
            .into_iter()
            .map(|scene| {
                let scene_text = Self::reference_scene_text(&scene);
                let scene_tokens = tokenize_text(&scene_text);

                let overlap = query_tokens.intersection(&scene_tokens).count() as f32;
                let total = query_tokens.union(&scene_tokens).count() as f32;
                let similarity = if total > 0.0 { overlap / total } else { 0.0 };
                (similarity, scene)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(3);

        Ok(scored
            .into_iter()
            .map(|(similarity, scene)| {
                let mut fewshot = Self::reference_scene_to_fewshot(scene);
                fewshot.similarity = similarity;
                fewshot
            })
            .collect())
    }

    fn scene_outline_query_text(scene_outline: &Option<SceneOutline>) -> String {
        let mut parts = Vec::new();
        if let Some(ref outline) = scene_outline {
            if let Some(ref g) = outline.dramatic_goal {
                parts.push(g.clone());
            }
            if let Some(ref c) = outline.conflict_type {
                parts.push(c.clone());
            }
            if let Some(ref p) = outline.external_pressure {
                parts.push(p.clone());
            }
            if let Some(ref s) = outline.setting_location {
                parts.push(s.clone());
            }
        }
        parts.join(" ")
    }

    fn reference_scene_text(scene: &ReferenceScene) -> String {
        let mut parts = Vec::new();
        if let Some(ref title) = scene.title {
            parts.push(title.clone());
        }
        if let Some(ref summary) = scene.summary {
            parts.push(summary.clone());
        }
        if let Some(ref key_events) = scene.key_events {
            parts.push(key_events.clone());
        }
        if let Some(ref characters) = scene.characters_present {
            parts.push(characters.clone());
        }
        if let Some(ref conflict) = scene.conflict_type {
            parts.push(conflict.clone());
        }
        if let Some(ref tone) = scene.emotional_tone {
            parts.push(tone.clone());
        }
        parts.join(" ")
    }

    fn reference_scene_to_fewshot(scene: ReferenceScene) -> ReferenceSceneFewShot {
        let title = scene
            .title
            .clone()
            .unwrap_or_else(|| format!("场景 {}", scene.sequence_number));
        let summary = scene.summary.clone().unwrap_or_default();
        let content_snippet = {
            let text = Self::reference_scene_text(&scene);
            truncate(&text, 300)
        };
        ReferenceSceneFewShot {
            title,
            summary,
            content_snippet,
            similarity: 0.0,
        }
    }

    /// 渲染参考场景 few-shots 段落。
    pub fn render_reference_scene_fewshots(fewshots: &[ReferenceSceneFewShot]) -> String {
        let items: Vec<String> = fewshots
            .iter()
            .enumerate()
            .map(|(i, fs)| {
                let mut lines = vec![format!(
                    "示例 {}：{}（相似度：{:.2}）",
                    i + 1,
                    fs.title,
                    fs.similarity
                )];
                if !fs.summary.is_empty() {
                    lines.push(format!("  摘要：{}", fs.summary));
                }
                if !fs.content_snippet.is_empty() {
                    lines.push(format!("  片段：{}", fs.content_snippet));
                }
                lines.join("\n")
            })
            .collect();
        format!(
            "【参考场景 few-shots（仅借鉴叙事节奏与冲突处理方式，禁止复制原文）】\n{}\n\n请仅学习上述参考场景的节奏、张力与写作技巧，不得直接复制其文字、人物或专有设定。",
            items.join("\n\n")
        )
    }

    /// 将 bundle 序列化为 prompt 注入字符串。
    ///
    /// 注入顺序（Phase 0 实证：红线最前最突出）：
    /// 1. 世界观红线（加粗强调「绝不可违背」）
    /// 2. 角色当前状态（直接影响行为合理性）
    /// 3. 场景大纲
    /// 4. GenreProfile 反模式
    /// 5. 风格片段（若有）
    pub fn to_prompt(&self) -> String {
        let mut sections: Vec<String> = vec![];

        // ① 世界观红线——最前、最突出（Phase 0 S1 实证：资产多 ≠ 幻觉少，红线必须醒目）
        if let Some(ref redlines) = self.contract_redlines {
            // 尝试从 contract_json 提取核心约束文本；若解析失败，原文兜底
            let redline_text = extract_redline_text(redlines);
            sections.push(format!(
                "【⚠️ 世界观红线（绝不可违背，违反即判定为严重错误）】\n{}",
                redline_text
            ));
        }

        // ② 角色核心 + 当前状态
        if !self.core_characters.is_empty() {
            let char_lines: Vec<String> = self
                .core_characters
                .iter()
                .map(|c| {
                    let mut parts = vec![format!("姓名：{}", c.name)];
                    if let Some(ref id) = c.identity {
                        parts.push(format!("身份：{}", id));
                    }
                    // 当前状态优先（Phase 0 memory 维度是最大波动源）
                    let mut state_parts = vec![];
                    if let Some(ref s) = c.physical_state {
                        state_parts.push(format!("身体：{}", s));
                    }
                    if let Some(ref s) = c.mental_state {
                        state_parts.push(format!("精神：{}", s));
                    }
                    if let Some(ref s) = c.location {
                        state_parts.push(format!("位置：{}", s));
                    }
                    if !state_parts.is_empty() {
                        parts.push(format!("当前状态：{}", state_parts.join("，")));
                    }
                    if let Some(ref p) = c.personality {
                        parts.push(format!("性格：{}", p));
                    }
                    format!("- {}", parts.join(" | "))
                })
                .collect();
            sections.push(format!(
                "【登场角色（必须严格遵循其当前状态）】\n{}",
                char_lines.join("\n")
            ));
        }

        // ③ 场景大纲
        if let Some(ref outline) = self.scene_outline {
            let mut outline_parts = vec![];
            if let Some(ref g) = outline.dramatic_goal {
                outline_parts.push(format!("戏剧目标：{}", g));
            }
            if let Some(ref c) = outline.conflict_type {
                outline_parts.push(format!("冲突类型：{}", c));
            }
            if let Some(ref p) = outline.external_pressure {
                outline_parts.push(format!("外部压迫：{}", p));
            }
            if let Some(ref s) = outline.setting_location {
                outline_parts.push(format!("场景地点：{}", s));
            }
            if !outline_parts.is_empty() {
                sections.push(format!("【本场景任务】\n{}", outline_parts.join("\n")));
            }
        }

        // ④ 反模式清单
        if !self.genre_antipatterns.is_empty() {
            let anti_lines: Vec<String> = self
                .genre_antipatterns
                .iter()
                .map(|a| format!("  - {}", a))
                .collect();
            sections.push(format!("【必须避免的反模式】\n{}", anti_lines.join("\n")));
        }

        // ⑤ 风格片段（题材自适应，仅 RealismEmotional/Mystery 纳入）
        if let Some(ref style) = self.style_slice {
            sections.push(format!("【风格指引】\n{}", style));
        }

        // P1-1 精选资产子集：以下 4 项此前在 TimeSliced 路径完全不进入 prompt，
        // 现在以压缩形式注入（每项 1-3 行），解决"资产黑洞"。

        // ⑥ 叙事阶段指导（一行）
        if let Some(ref phase) = self.narrative_phase_guidance {
            sections.push(format!("【叙事阶段】\n{}", phase));
        }

        // ⑦ 待回收伏笔（top 3）
        if !self.pending_foreshadowings.is_empty() {
            let lines: Vec<String> = self
                .pending_foreshadowings
                .iter()
                .map(|f| format!("  - {}", f))
                .collect();
            sections.push(format!(
                "【待回收伏笔（请在续写中适时推进）】\n{}",
                lines.join("\n")
            ));
        }

        // ⑧ 逾期伏笔（top 1，带警告）
        if !self.overdue_foreshadowings.is_empty() {
            let lines: Vec<String> = self
                .overdue_foreshadowings
                .iter()
                .map(|f| format!("  ⚠️ {}", f))
                .collect();
            sections.push(format!(
                "【⚠️ 逾期伏笔——请在续写中优先回收】\n{}",
                lines.join("\n")
            ));
        }

        // ⑨ 主导风格一句话摘要（全题材，非完整六维 DNA）
        if let Some(ref summary) = self.style_dna_summary {
            sections.push(format!("【主导风格】{}", summary));
        }

        // ⑩ 叙事四元组（来自 task.parameters，由 orchestrator 设置）
        if let Some(ref quartet) = self.narrative_quartet {
            sections.push(quartet.clone());
        }

        // v0.22.0: 解决 TimeSliced "资产黑洞"——注入与 Full 路径对等的完整资产
        // ⑪ 风格 DNA 六维量化指标（完整，替代之前的"一句话摘要"）
        if let Some(ref dna) = self.style_dna_extension {
            sections.push(format!("【风格 DNA 六维指标】\n{}", dna));
        }

        // ⑫ 方法论约束（当前步骤的完整规则）
        if let Some(ref method) = self.methodology_extension {
            sections.push(format!("【创作方法论约束】\n{}", method));
        }

        // ⑬ 题材画像策略（core_tone + pacing + reference + structure）
        if let Some(ref genre) = self.genre_profile_strategy {
            sections.push(genre.clone());
        }

        // ⑬-2 次要题材画像策略（复合题材资产补强）
        if let Some(ref secondary) = self.secondary_genre_profile_strategy {
            sections.push(secondary.clone());
        }

        // ⑭ 写作策略约束
        if let Some(ref ws) = self.writing_strategy_constraints {
            sections.push(ws.clone());
        }

        // v0.22.5: Story System 运行时合同约束
        if let Some(ref rc) = self.runtime_contract {
            let vars = rc.to_constraint_vars();
            if let Some(section) = crate::prompts::registry::resolve_prompt_default_with_vars(
                "write_time_bundle_contract",
                &vars,
            ) {
                if !section.trim().is_empty() {
                    sections.push(section);
                }
            }
        }

        // Phase 3.1: 参考场景 few-shots（来自关联拆书）
        if !self.reference_scene_fewshots.is_empty() {
            sections.push(Self::render_reference_scene_fewshots(
                &self.reference_scene_fewshots,
            ));
        }

        sections.join("\n\n")
    }
}

// ==================== 辅助函数 ====================

/// 解析 anti_patterns_json（可能是 JSON 数组或换行分隔文本）。
fn parse_antipatterns(json_str: &Option<String>) -> Vec<String> {
    let s = match json_str {
        Some(s) if !s.trim().is_empty() => s,
        _ => return vec![],
    };
    // 尝试 JSON 数组
    if let Ok(arr) = serde_json::from_str::<Vec<String>>(s) {
        return arr;
    }
    // 尝试 JSON 数组（元素为对象，取 text 字段）
    if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(s) {
        return arr
            .iter()
            .filter_map(|v| {
                v.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| v.as_str().map(|s| s.to_string()))
            })
            .collect();
    }
    // 兜底：按换行分割
    s.lines()
        .map(|l| l.trim().trim_start_matches('-').trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// 从 contract_json 提取核心红线文本。
/// 若能解析为 JSON 且含 world_rules/redlines
/// 字段，提取之；否则原文兜底（截断）。
pub(crate) fn extract_redline_text(contract_json: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(contract_json) {
        // 尝试常见字段名
        for key in &[
            "redlines",
            "world_rules",
            "core_rules",
            "world_setting",
            "description",
        ] {
            if let Some(val) = v.get(key) {
                if let Some(s) = val.as_str() {
                    return truncate(s, 800);
                }
                if let Ok(s) = serde_json::to_string(val) {
                    return truncate(&s, 800);
                }
            }
        }
        // 兜底：整个 JSON 的文本内容
        if let Some(s) = v.as_str() {
            return truncate(s, 800);
        }
    }
    // 非 JSON：原文截断
    truncate(contract_json, 800)
}

fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!(
            "{}...（已截断）",
            chars.iter().take(max_chars).collect::<String>()
        )
    }
}

/// 简单文本分词：按空白与常见中英文标点切分，过滤单字符与空串。
fn tokenize_text(s: &str) -> std::collections::HashSet<String> {
    let delimiters: &[char] = &[
        ' ', '\t', '\n', '\r', '，', '。', '！', '？', '；', '：', '"', '“', '”', '\'', '‘', '’',
        '（', '）', '(', ')', '[', ']', '、', '《', '》', ',', '.', '!', '?', ';', ':',
    ];
    s.to_lowercase()
        .split(delimiters)
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty() && t.chars().count() > 1)
        .collect()
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genre_category_realism_detection() {
        assert_eq!(
            GenreCategory::from_genre(Some("都市言情")),
            GenreCategory::RealismEmotional
        );
        assert_eq!(
            GenreCategory::from_genre(Some("青春校园")),
            GenreCategory::RealismEmotional
        );
        assert_eq!(
            GenreCategory::from_genre(Some("Urban Romance")),
            GenreCategory::RealismEmotional
        );
    }

    #[test]
    fn genre_category_speculative_detection() {
        assert_eq!(
            GenreCategory::from_genre(Some("东方玄幻")),
            GenreCategory::Speculative
        );
        assert_eq!(
            GenreCategory::from_genre(Some("硬科幻")),
            GenreCategory::Speculative
        );
        assert_eq!(
            GenreCategory::from_genre(Some("Sci-Fi")),
            GenreCategory::Speculative
        );
    }

    #[test]
    fn genre_category_mystery_detection() {
        assert_eq!(
            GenreCategory::from_genre(Some("悬疑推理")),
            GenreCategory::Mystery
        );
        assert_eq!(
            GenreCategory::from_genre(Some("侦探小说")),
            GenreCategory::Mystery
        );
    }

    #[test]
    fn genre_category_unknown_for_empty_or_unmatched() {
        assert_eq!(GenreCategory::from_genre(None), GenreCategory::Unknown);
        assert_eq!(GenreCategory::from_genre(Some("")), GenreCategory::Unknown);
        assert_eq!(
            GenreCategory::from_genre(Some("武侠")),
            GenreCategory::Unknown
        );
    }

    #[test]
    fn style_slice_only_for_realism_and_mystery() {
        assert!(GenreCategory::RealismEmotional.include_style_slice());
        assert!(GenreCategory::Mystery.include_style_slice());
        assert!(!GenreCategory::Speculative.include_style_slice());
        assert!(!GenreCategory::Unknown.include_style_slice());
    }

    #[test]
    fn parse_antipatterns_json_array() {
        let result = parse_antipatterns(&Some(
            r#"["主角突然觉醒血脉", "无铺垫神级法器"]"#.to_string(),
        ));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "主角突然觉醒血脉");
    }

    #[test]
    fn parse_antipatterns_newline_separated() {
        let result = parse_antipatterns(&Some("第一行反模式\n第二行反模式".to_string()));
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn parse_antipatterns_empty() {
        assert!(parse_antipatterns(&None).is_empty());
        assert!(parse_antipatterns(&Some("".to_string())).is_empty());
    }

    #[test]
    fn truncate_respects_char_boundary() {
        let long =
            "一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十一二三四五六七八九十";
        let t = truncate(long, 10);
        assert!(t.ends_with("...（已截断）"));
        // 截断后（不含后缀）应 <= 10 字符
        let body = t.trim_end_matches("...（已截断）");
        assert!(body.chars().count() <= 10);
    }

    #[test]
    fn extract_redline_from_json_field() {
        let json = r#"{"redlines": "修炼者不可凭空变出实物"}"#;
        let text = extract_redline_text(json);
        assert!(text.contains("修炼者不可凭空变出实物"));
    }

    #[test]
    fn extract_redline_fallback_to_raw() {
        let raw = "这不是JSON只是一段纯文本红线描述";
        let text = extract_redline_text(raw);
        assert!(text.contains("纯文本红线"));
    }

    #[test]
    fn to_prompt_secondary_genre_strategy_rendered() {
        let bundle = WriteTimeBundle {
            contract_redlines: None,
            core_characters: vec![],
            scene_outline: None,
            genre_antipatterns: vec![],
            style_slice: None,
            story_meta: StoryMeta {
                title: "测试".to_string(),
                genre: Some("末世流".to_string()),
                tone: None,
                pacing: None,
                description: None,
            },
            genre_category: GenreCategory::Speculative,
            narrative_phase_guidance: None,
            pending_foreshadowings: vec![],
            overdue_foreshadowings: vec![],
            style_dna_summary: None,
            narrative_quartet: None,
            style_dna_extension: None,
            methodology_extension: None,
            genre_profile_strategy: Some("【体裁画像策略（末世流）】\n基调：文明崩溃".to_string()),
            secondary_genre_profile_strategy: Some(
                "【次要题材画像补充（复合题材）】\n- 异星世界（Alien World）：基调：陌生星球"
                    .to_string(),
            ),
            writing_strategy_constraints: None,
            runtime_contract: None,
            reference_scene_fewshots: vec![],
        };
        let prompt = bundle.to_prompt();
        assert!(prompt.contains("次要题材画像补充"));
        assert!(prompt.contains("异星世界"));
    }

    #[test]
    fn to_prompt_redlines_appear_first() {
        let bundle = WriteTimeBundle {
            contract_redlines: Some(r#"{"redlines": "绝对红线内容"}"#.to_string()),
            core_characters: vec![CoreCharacter {
                name: "测试角色".to_string(),
                identity: None,
                physical_state: None,
                mental_state: None,
                location: None,
                personality: None,
            }],
            scene_outline: None,
            genre_antipatterns: vec!["某反模式".to_string()],
            style_slice: None,
            story_meta: StoryMeta {
                title: "测试".to_string(),
                genre: Some("玄幻".to_string()),
                tone: None,
                pacing: None,
                description: None,
            },
            genre_category: GenreCategory::Speculative,
            narrative_phase_guidance: None,
            pending_foreshadowings: vec![],
            overdue_foreshadowings: vec![],
            style_dna_summary: None,
            narrative_quartet: None,
            style_dna_extension: None,
            methodology_extension: None,
            genre_profile_strategy: None,
            secondary_genre_profile_strategy: None,
            writing_strategy_constraints: None,
            runtime_contract: None,
            reference_scene_fewshots: vec![],
        };
        let prompt = bundle.to_prompt();
        let redline_pos = prompt.find("绝对红线内容").unwrap_or(usize::MAX);
        let char_pos = prompt.find("测试角色").unwrap_or(usize::MAX);
        let anti_pos = prompt.find("某反模式").unwrap_or(usize::MAX);
        // 红线必须最前
        assert!(redline_pos < char_pos, "红线应在角色之前");
        assert!(redline_pos < anti_pos, "红线应在反模式之前");
        // Speculative 题材不应有风格片段
        assert!(!prompt.contains("风格指引"));
    }

    #[test]
    fn render_reference_scene_fewshots_includes_title_and_snippet() {
        let fewshots = vec![ReferenceSceneFewShot {
            title: "山谷决战".to_string(),
            summary: "主角与反派在山谷中决战。".to_string(),
            content_snippet: "剑光一闪，两人错身而过。".to_string(),
            similarity: 0.85,
        }];
        let section = WriteTimeBundle::render_reference_scene_fewshots(&fewshots);
        assert!(section.contains("山谷决战"));
        assert!(section.contains("0.85"));
        assert!(section.contains("主角与反派在山谷中决战"));
        assert!(section.contains("剑光一闪"));
        assert!(section.contains("禁止复制原文"));
    }

    #[test]
    fn tokenize_text_splits_on_punctuation() {
        let tokens = tokenize_text("主角，反派；决战：山谷！");
        assert!(tokens.contains("主角"));
        assert!(tokens.contains("反派"));
        assert!(tokens.contains("决战"));
        assert!(tokens.contains("山谷"));
        assert!(!tokens.contains("主"));
    }
}
