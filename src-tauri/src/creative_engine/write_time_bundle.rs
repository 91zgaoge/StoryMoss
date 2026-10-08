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

// ==================== 数据结构 ====================
//
// 数据类型已迁移到 `crate::domain::write_time_bundle` 以保持中性；
// 本模块仅保留 I/O 加载与 prompt 渲染行为实现。
use std::sync::Arc;

pub use crate::domain::write_time_bundle::*;
use crate::{
    creative_engine::asset_snapshot::CreativeAssetSnapshot,
    db::{
        repositories_narrative::NarrativeSceneRepository, Character,
        CharacterRelationshipRepository, CharacterRepository, DbPool, GenreProfileRepository,
        SceneRepository, StoryContractRepository, StoryOutlineRepository, StyleDnaRepository,
    },
    domain::narrative_elements::SceneElement,
};

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
        contract_provider: Option<Arc<dyn crate::domain::creative_engine::RuntimeContractProvider>>,
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
        let runtime_contract = match contract_provider {
            Some(provider) => match provider.get_runtime_contract(story_id, chapter_number) {
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
            },
            None => None,
        };

        // 3. 角色核心
        let char_repo = CharacterRepository::new(pool.clone());
        let chars = match char_repo.get_by_story(story_id) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("[WriteTimeBundle] 查询角色失败: {}", e);
                vec![]
            }
        };
        let states = char_repo
            .get_character_states_by_story(story_id)
            .map_err(|e| {
                log::warn!("[WriteTimeBundle] 加载角色状态失败: {}", e);
                e
            })
            .unwrap_or_default();
        let mut core_characters: Vec<CoreCharacter> = chars
            .iter()
            .map(|c: &Character| {
                let state = states.get(&c.id);
                CoreCharacter {
                    name: c.name.clone(),
                    identity: c.background.clone(),
                    physical_state: state.and_then(|s| s.physical_state.clone()),
                    mental_state: state.and_then(|s| s.mental_state.clone()),
                    location: state.and_then(|s| s.location.clone()),
                    personality: c.personality.clone(),
                    emotional_core: c.emotional_core.as_ref().filter(|s| !s.is_empty()).cloned(),
                    emotional_trigger: c
                        .emotional_trigger
                        .as_ref()
                        .filter(|s| !s.is_empty())
                        .cloned(),
                    emotional_wound: c
                        .emotional_wound
                        .as_ref()
                        .filter(|s| !s.is_empty())
                        .cloned(),
                    emotional_need: c.emotional_need.as_ref().filter(|s| !s.is_empty()).cloned(),
                }
            })
            .collect();

        // v0.64.7：已死角色（持久判定）在角色卡「身体：」一行带标记——
        // 角色卡渲染只认 physical_state，不改结构体即可覆盖全部注入路径。
        let dead_cards = crate::story_system::life_status::dead_marker_map(pool, story_id);
        if !dead_cards.is_empty() {
            for c in core_characters.iter_mut() {
                if let Some(chapter) = dead_cards.get(&c.name) {
                    c.physical_state = Some(crate::db::character_life::annotate_physical_state(
                        c.physical_state.as_deref(),
                        *chapter,
                    ));
                }
            }
        }

        let relationship_lines: Vec<String> =
            match CharacterRelationshipRepository::new(pool.clone()).get_by_story(story_id) {
                Ok(rels) => rels
                    .iter()
                    .map(|r| {
                        let src_name = chars
                            .iter()
                            .find(|c| c.id == r.source_character_id)
                            .map(|c| c.name.as_str())
                            .unwrap_or("?");
                        let tgt_name = r.target_character_name.as_deref().unwrap_or("?");
                        let bond = r.emotional_bond.as_deref().unwrap_or("未明");
                        let intensity = r.emotional_intensity.unwrap_or(0.5);
                        let rev_bond = r.reverse_emotional_bond.as_deref().unwrap_or("未明");
                        let rev_intensity = r.reverse_emotional_intensity.unwrap_or(0.5);
                        format!(
                            "■ {} -> {}：社会关系={} ｜ 情感={}[{:.1}]（{} -> {}：{}[{:.1}]）",
                            src_name,
                            tgt_name,
                            r.relationship_type,
                            bond,
                            intensity,
                            tgt_name,
                            src_name,
                            rev_bond,
                            rev_intensity,
                        )
                    })
                    .collect(),
                Err(e) => {
                    log::warn!("[WriteTimeBundle] 查询角色关系失败: {}", e);
                    vec![]
                }
            };

        // 4. 场景大纲
        let scene_repo = SceneRepository::new(pool.clone());
        // Phase 4: 使用 scene_id 直接查找（1:N 兼容）
        let scene_outline = match scene_repo.get_by_story(story_id) {
            Ok(scenes) => {
                let scene = scenes.iter().find(|s| s.sequence_number == chapter_number);
                scene.map(|s| SceneOutline {
                    dramatic_goal: s.dramatic_goal.clone(),
                    conflict_type: s.conflict_type.as_ref().map(|c| format!("{:?}", c)),
                    external_pressure: s.external_pressure.clone(),
                    setting_location: s.setting_location.clone(),
                    // Phase 4: 补全场景级字段
                    characters_present: s.characters_present.clone(),
                    setting_time: s.setting_time.clone(),
                    setting_atmosphere: s.setting_atmosphere.clone(),
                    outline_content: s.outline_content.clone(),
                })
            }
            Err(e) => {
                log::warn!("[WriteTimeBundle] 查询场景失败: {}", e);
                None
            }
        };

        // Phase 3.1: 加载参考场景 few-shots（若故事关联了参考书籍）。
        // 当前 load_sync 为同步上下文，无法直接调用 LanceVectorStore
        // 的异步向量搜索，
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

        // v0.31.0: 加载方法论扩展——动态解析（Task 6）。
        // 先试 methodology_{id}_step{N}，再试 methodology_{id}，hdwb 旧命名
        // 走兼容映射；未知 ID 在 resolve_methodology_extension 内 log::warn!
        // 并返回 None。自定义（指导书提炼）方法论从 DB 渲染当前步骤。
        let methodology_extension = match story.methodology_id.as_deref() {
            Some(mid) if !mid.is_empty() => {
                let step = story.methodology_step.unwrap_or(1);
                if crate::domain::methodology::is_custom_methodology_id(mid) {
                    // 自定义（指导书提炼）方法论：从 DB 渲染当前步骤
                    crate::guidebook_distillation::render_custom_methodology_extension(
                        &pool, mid, step,
                    )
                } else {
                    resolve_methodology_extension(mid, step)
                }
            }
            _ => None,
        };

        // v0.22.0: 加载 GenreProfile 完整策略（profile 取出后同时供
        // genre_reference 复用）
        let primary_genre_profile = {
            let genre_name = story.genre.as_deref().unwrap_or("");
            if genre_name.is_empty() {
                None
            } else {
                let genre_repo2 = GenreProfileRepository::new(pool.clone());
                genre_repo2.get_by_name(genre_name).ok().flatten()
            }
        };
        let genre_profile_strategy = {
            let genre_name = story.genre.as_deref().unwrap_or("");
            primary_genre_profile.as_ref().and_then(|profile| {
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
            })
        };
        // 设计第一节：体裁元素参考表 + 典型结构（复用 Task 1 共享函数，预算
        // ~800 字）
        let genre_reference = primary_genre_profile
            .as_ref()
            .and_then(|p| crate::agents::writer_assets::format_genre_reference_tables(p, 800));

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

        // v0.22.0: 加载写作策略约束（默认值；execute_time_sliced 会用 AppConfig
        // 覆盖）
        let writing_strategy_constraints = Some(format_writing_strategy_constraints(
            &crate::config::settings::WritingStrategy::default(),
        ));

        // P1-1: 精选资产子集——解决 TimeSliced "资产黑洞"
        // P3-3: 使用统一资产注入网关 CreativeAssetSnapshot，消除重复加载逻辑。
        // 通过中性 port 注入伏笔/账本能力，避免 creative_engine 直接依赖
        // canonical_state。
        let foreshadowing_port: Arc<dyn crate::domain::creative_engine::ForeshadowingPort> =
            Arc::new(
                crate::creative_engine::foreshadowing::ForeshadowingTracker::new(pool.clone()),
            );
        let payoff_ledger_port: Arc<dyn crate::domain::creative_engine::PayoffLedgerPort> =
            Arc::new(crate::creative_engine::payoff_ledger::PayoffLedger::new(
                pool.clone(),
            ));
        let snapshot = CreativeAssetSnapshot::load_sync(
            pool,
            story_id,
            story.style_dna_id.as_deref(),
            foreshadowing_port,
            payoff_ledger_port,
        );

        let narrative_phase_guidance = snapshot.narrative_phase_guidance();
        let pending_foreshadowings = snapshot.pending_foreshadowings(3);
        let overdue_foreshadowings = snapshot.overdue_foreshadowings(1);
        let style_dna_summary = snapshot.style_dna_summary;

        // P1b: KG 相关设定摘要（与 StoryContextBuilder 共用 MemoryFacade）
        let related_entity_summaries = crate::memory::MemoryFacade::related_entity_summaries(
            pool,
            story_id,
            crate::memory::DEFAULT_RELATED_ENTITY_LIMIT,
        );

        // 设计第一节：续写链路资产贯通——活跃冲突与角色目标复用 Task 1
        // 共享函数， 补齐 TimeSliced 死注入（预算 ~600 字 / 每角色 ~200
        // 字）。 规范状态快照在此一次性加载并传入，
        // 避免两段各自重复聚合；加载失败时
        // 两段一并跳过（与原每段各自返回 None 的行为一致）。
        let cs_snapshot = crate::canonical_state::CanonicalStateManager::new(pool.clone())
            .get_snapshot_sync(story_id)
            .map_err(|e| {
                log::warn!(
                    "[write_time_bundle] 规范状态快照加载失败({}): {}",
                    story_id,
                    e
                )
            })
            .ok();
        let active_conflicts = cs_snapshot
            .as_ref()
            .and_then(|s| crate::agents::writer_assets::format_active_conflicts(s, 600));
        let character_goals = cs_snapshot
            .as_ref()
            .and_then(|s| crate::agents::writer_assets::format_character_goals(s, 200));

        // v0.30.15: 加载完整故事大纲，让 writer
        // 围绕大纲展开（TimeSliced/TriShot 此前
        // 看不到故事大纲，导致续写偏离大纲自创情节/角色）。
        let story_outline = StoryOutlineRepository::new(pool.clone())
            .get_by_story(story_id)
            .ok()
            .flatten()
            .map(|o| {
                let c = o.content;
                if c.chars().count() > 4000 {
                    let truncated: String = c.chars().take(4000).collect();
                    truncated + "\n…（已截断）"
                } else {
                    c
                }
            });

        // v0.30.31: 加载 world_buildings
        // 表渲染世界观设定（concept/rules/history/ cultures）。Legacy
        // bundle 此前只读 MASTER_SETTING 合同红线，用户在世界观
        // 面板填的设定从不到达 writer，导致"世界观没体现在续写中"。
        let world_setting = {
            use crate::db::repositories::WorldBuildingRepository;
            match WorldBuildingRepository::new(pool.clone()).get_by_story(story_id) {
                Ok(Some(w)) => {
                    let mut parts: Vec<String> = Vec::new();
                    if !w.concept.trim().is_empty() {
                        parts.push(format!("世界概念：{}", w.concept));
                    }
                    if !w.rules.is_empty() {
                        let rules_text = w
                            .rules
                            .iter()
                            .take(5)
                            .map(|r| {
                                format!("- {}：{}", r.name, r.description.as_deref().unwrap_or(""))
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        parts.push(format!("核心规则：\n{}", rules_text));
                    }
                    if let Some(ref h) = w.history {
                        if !h.trim().is_empty() {
                            parts.push(format!("历史背景：{}", h));
                        }
                    }
                    if !w.cultures.is_empty() {
                        let cultures_text = w
                            .cultures
                            .iter()
                            .take(3)
                            .map(|c| format!("- {}：{}", c.name, c.description))
                            .collect::<Vec<_>>()
                            .join("\n");
                        parts.push(format!("文化与势力：\n{}", cultures_text));
                    }
                    if parts.is_empty() {
                        None
                    } else {
                        let joined = parts.join("\n\n");
                        // 2000 字符预算截断
                        if joined.chars().count() > 2000 {
                            let truncated: String = joined.chars().take(2000).collect();
                            Some(format!("{}…（已截断）", truncated))
                        } else {
                            Some(joined)
                        }
                    }
                }
                _ => None,
            }
        };

        // v0.34.0 弹性扩张：轮换账本段（数据缺失/空书 → None，整段省略；
        // 加载失败降级为 None 并 log::warn
        // 留痕——非关键增强段，不得阻断创作主流程）
        let rotation_ledger_text =
            crate::creative_engine::expansion::RotationLedger::load_sync(pool, story_id)
                .map_err(|e| log::warn!("[WriteTimeBundle] 轮换账本加载失败: {}", e))
                .ok()
                .and_then(|l| l.render_for_prompt());

        Ok(WriteTimeBundle {
            contract_redlines,
            core_characters,
            relationship_lines,
            scene_outline,
            story_outline,
            world_setting,
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
            related_entity_summaries,
            active_conflicts,
            character_goals,
            chase_debt_text: None, // 由调用方（orchestrator）从 task.parameters 设置
            genre_reference,
            style_blend_text: None, // 由调用方（orchestrator）从 task.parameters 设置
            rotation_ledger_text,
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
        let scene_repo = NarrativeSceneRepository::new(pool.clone());
        let scenes = scene_repo.get_by_story(book_id)?;
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

        let mut scored: Vec<(f32, SceneElement)> = scenes
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

    fn reference_scene_text(scene: &SceneElement) -> String {
        let mut parts = Vec::new();
        if !scene.title.is_empty() {
            parts.push(scene.title.clone());
        }
        if !scene.summary.is_empty() {
            parts.push(scene.summary.clone());
        }
        if !scene.key_events.is_empty() {
            parts.push(scene.key_events.join(", "));
        }
        if !scene.characters_present.is_empty() {
            parts.push(scene.characters_present.join(", "));
        }
        if !scene.conflict_type.is_empty() {
            parts.push(scene.conflict_type.clone());
        }
        if !scene.emotional_tone.is_empty() {
            parts.push(scene.emotional_tone.clone());
        }
        parts.join(" ")
    }

    fn reference_scene_to_fewshot(scene: SceneElement) -> ReferenceSceneFewShot {
        let title = if scene.title.is_empty() {
            format!("场景 {}", scene.sequence_number)
        } else {
            scene.title.clone()
        };
        let summary = scene.summary.clone();
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

        // ① 世界观红线——最前、最突出（Phase 0 S1 实证：资产多 ≠
        // 幻觉少，红线必须醒目）
        if let Some(ref redlines) = self.contract_redlines {
            // 尝试从 contract_json 提取核心约束文本；若解析失败，原文兜底
            let redline_text = extract_redline_text(redlines);
            sections.push(format!(
                "【⚠️ 世界观红线（绝不可违背，违反即判定为严重错误）】\n{}",
                redline_text
            ));
        }

        // ①b 故事大纲--writer 必须围绕展开（v0.30.15：TimeSliced/TriShot
        // 此前看不到 故事大纲，导致续写偏离大纲自创情节/角色）。
        // 置于红线之后、角色之前， 醒目且不破坏 红线第一的不变量。
        if let Some(ref outline) = self.story_outline {
            sections.push(format!(
                "【故事大纲（本场景必须围绕此大纲展开，禁止偏离）】\n{}\n（若下方「本场景任务」与此大纲冲突，以本故事大纲为准。{}）",
                outline,
                new_character_policy_text(self.narrative_phase_guidance.as_deref())
            ));
        }

        // ①c 世界观设定--v0.30.31：world_buildings 表的
        // concept/rules/history/cultures。 与①的红线（MASTER_SETTING
        // 合同）互补：红线是硬性禁令，本段是世界观土壤，
        // writer 须在其规则与约束内推进情节。置于故事大纲之后、角色之前。
        if let Some(ref world) = self.world_setting {
            sections.push(format!(
                "【世界观设定（须遵循其规则与约束，违反即判定为严重错误）】\n{}",
                world
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
                    if let Some(ref v) = c.emotional_core {
                        parts.push(format!("情感内核：{}", v));
                    }
                    if let Some(ref v) = c.emotional_trigger {
                        parts.push(format!("情感触发：{}", v));
                    }
                    if let Some(ref v) = c.emotional_wound {
                        parts.push(format!("情感创伤：{}", v));
                    }
                    if let Some(ref v) = c.emotional_need {
                        parts.push(format!("情感需求：{}", v));
                    }
                    format!("- {}", parts.join(" | "))
                })
                .collect();
            sections.push(format!(
                "【登场角色（必须严格遵循其当前状态）】\n{}",
                char_lines.join("\n")
            ));
        }

        if !self.relationship_lines.is_empty() {
            sections.push(format!(
                "【角色情感关系（真实情感，可与表面关系不一致）】\n{}\n要求：言行须与情感关系一致。",
                self.relationship_lines.join("\n")
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
            // Phase 4: 补全场景级字段
            if let Some(ref t) = outline.setting_time {
                outline_parts.push(format!("时间：{}", t));
            }
            if let Some(ref a) = outline.setting_atmosphere {
                outline_parts.push(format!("氛围：{}", a));
            }
            if !outline.characters_present.is_empty() {
                outline_parts.push(format!(
                    "出场人物：{}",
                    outline.characters_present.join("、")
                ));
            }
            if let Some(ref o) = outline.outline_content {
                outline_parts.push(format!("场景大纲：{}", o));
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

        // ⑧b 活跃冲突清单（设计第一节：TimeSliced 补齐死注入）
        if let Some(ref conflicts) = self.active_conflicts {
            sections.push(conflicts.clone());
        }

        // ⑧c 角色目标/弧光/秘密
        if let Some(ref goals) = self.character_goals {
            sections.push(goals.clone());
        }

        // ⑧d 追读力债务 + 本章追读力目标（由 orchestrator 渲染后设置）
        if let Some(ref chase) = self.chase_debt_text {
            sections.push(chase.clone());
        }

        // ⑧e 轮换账本（v0.34.0 弹性扩张：场景/角色使用数据驱动调度）
        if let Some(ref ledger) = self.rotation_ledger_text {
            sections.push(ledger.clone());
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
        // ⑪ 风格：优先风格混合 blend（多 DNA 融合），缺省回退单 DNA 六维指标
        if let Some(ref blend) = self.style_blend_text {
            sections.push(format!(
                "【风格混合（多风格融合，须兼顾各成分风格）】\n{}",
                blend
            ));
        } else if let Some(ref dna) = self.style_dna_extension {
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

        // ⑬b 体裁元素参考表 + 典型结构（~800 字预算）
        if let Some(ref reference) = self.genre_reference {
            sections.push(format!("【体裁元素参考】\n{}", reference));
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

        // P1b: 知识图谱相关设定（轻量 top-N，零 LLM）
        if !self.related_entity_summaries.is_empty() {
            let lines: Vec<String> = self
                .related_entity_summaries
                .iter()
                .map(|s| format!("  - {}", s))
                .collect();
            sections.push(format!("【相关设定】\n{}", lines.join("\n")));
        }

        sections.join("\n\n")
    }

    /// v0.26.40: 资产→prompt 覆盖率（各槽是否非空），供 Tracing 面板展示。
    pub fn prompt_coverage(&self) -> serde_json::Value {
        let slots = [
            self.contract_redlines.is_some(),
            self.runtime_contract.is_some(),
            !self.core_characters.is_empty(),
            self.scene_outline.is_some(),
            !self.pending_foreshadowings.is_empty() || !self.overdue_foreshadowings.is_empty(),
            !self.genre_antipatterns.is_empty(),
            self.style_slice.is_some() || self.style_dna_summary.is_some(),
            self.methodology_extension.is_some(),
            !self.related_entity_summaries.is_empty(),
            !self.reference_scene_fewshots.is_empty(),
        ];
        let filled_slots = slots.iter().filter(|&&x| x).count();
        serde_json::json!({
            "contract_redlines": self.contract_redlines.is_some(),
            "runtime_contract": self.runtime_contract.is_some(),
            "core_characters": !self.core_characters.is_empty(),
            "scene_outline": self.scene_outline.is_some(),
            "pending_foreshadowings": !self.pending_foreshadowings.is_empty(),
            "overdue_foreshadowings": !self.overdue_foreshadowings.is_empty(),
            "genre_antipatterns": !self.genre_antipatterns.is_empty(),
            "style_slice": self.style_slice.is_some(),
            "style_dna_summary": self.style_dna_summary.is_some(),
            "methodology_extension": self.methodology_extension.is_some(),
            "related_entity_summaries": !self.related_entity_summaries.is_empty(),
            "reference_scene_fewshots": !self.reference_scene_fewshots.is_empty(),
            "filled_slots": filled_slots,
            "total_slots": 10,
        })
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

// ==================== 工具函数 ====================

/// v0.31.0: 方法论扩展动态解析（推荐资产贯通 + 方法论动态化）。
///
/// 解析顺序：
/// 1. `methodology_{id}_step{N}`（step 变体，如雪花法 10 步）
/// 2. `methodology_{id}`（无 step 后缀的单文件，如英雄之旅）
/// 3. 兼容旧命名：hdwb 系列 4
///    个阶段文件（seed/expansion/convergence/iteration） 未按 step
///    规范命名，按步数映射到既有文件
///
/// 均未命中：记 `log::warn!`（不再静默丢弃）并返回 None。
/// 新增方法论 = 向 `resources/prompts/methodology/` 丢一个
/// `methodology_{id}.md`（可选 `methodology_{id}_step{N}.md`），无需改代码。
pub fn resolve_methodology_extension(methodology_id: &str, step: i32) -> Option<String> {
    let mid = crate::domain::methodology::normalize_methodology_id(methodology_id);
    let step = step.max(1);

    let step_id = format!("methodology_{}_step{}", mid, step);
    if let Some(content) = crate::prompts::registry::resolve_prompt_default(&step_id) {
        let label = crate::prompts::registry::prompt_display_name(&step_id);
        return Some(format!("【创作方法论（{}）】\n{}", label, content));
    }

    let base_id = format!("methodology_{}", mid);
    if let Some(content) = crate::prompts::registry::resolve_prompt_default(&base_id) {
        let label = crate::prompts::registry::prompt_display_name(&base_id);
        return Some(format!("【创作方法论（{}）】\n{}", label, content));
    }

    // 兼容旧命名：hdwb 的 4 个阶段文件（step 1=seed / 2=expansion /
    // 3=convergence / 4=iteration）
    if mid == "high_density_world_building" {
        let legacy_id = match step {
            2 => "methodology_hdwb_expansion",
            3 => "methodology_hdwb_convergence",
            4 => "methodology_hdwb_iteration",
            _ => "methodology_hdwb_seed",
        };
        if let Some(content) = crate::prompts::registry::resolve_prompt_default(legacy_id) {
            let label = crate::prompts::registry::prompt_display_name(legacy_id);
            return Some(format!("【创作方法论（{}）】\n{}", label, content));
        }
    }

    log::warn!(
        "[WriteTimeBundle] 未知方法论 ID '{}'（step {}），跳过方法论注入",
        mid,
        step
    );
    None
}

/// v0.31.0: 阶段感知的新角色策略（替代旧「禁止自创新角色」一刀切文案）。
///
/// 依据【叙事阶段】指导文本判定扩张/收敛取向；底线恒定：新角色必须有
/// 明确叙事功能、不得违反世界观红线（MASTER_SETTING）。
pub(crate) fn new_character_policy_text(narrative_phase_guidance: Option<&str>) -> &'static str {
    const EXPANSION: &str = "若当前处于开篇/发展期，允许引入具有明确叙事功能的新角色（推动冲突、揭示世界观或制造转折），允许合理切换场景、推动冲突升级；新角色必须服务于本场景戏剧目标，不得违反上述世界观红线与故事大纲。";
    const CONVERGENCE: &str = "当前处于高潮/收尾期：聚焦既有角色与既有冲突的爆发与收束，不引入新的重要角色，优先回收伏笔、推进故事大纲的下一节点。";
    match narrative_phase_guidance {
        Some(g) if g.contains("高潮期") || g.contains("收尾期") || g.contains("冲突激化期") => {
            CONVERGENCE
        }
        _ => EXPANSION,
    }
}

/// v0.23.59: 将 `WritingStrategy` 格式化为写作策略约束提示文本。
///
/// 冲突强度与叙事节奏复用 Full 路径的分档语义文案（writer_assets 共享函数，
/// 原 service.rs:1888-1908），替代此前的裸数字（"冲突强度：0.5"）。
pub fn format_writing_strategy_constraints(
    strategy: &crate::config::settings::WritingStrategy,
) -> String {
    format!(
        "【写作策略约束】\n运行模式：{}\n{}\nAI 自由度：{}",
        strategy.run_mode,
        crate::agents::writer_assets::writing_constraints_semantic_text(
            strategy.conflict_level as f64,
            crate::agents::writer_assets::pace_to_factor(&strategy.pace),
        ),
        strategy.ai_freedom,
    )
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
            relationship_lines: vec![],
            scene_outline: None,
            story_outline: None,
            world_setting: None,
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
            related_entity_summaries: vec![],
            active_conflicts: None,
            character_goals: None,
            chase_debt_text: None,
            genre_reference: None,
            style_blend_text: None,
            rotation_ledger_text: None,
        };
        let prompt = bundle.to_prompt();
        assert!(prompt.contains("次要题材画像补充"));
        assert!(prompt.contains("异星世界"));
    }

    #[test]
    fn to_prompt_includes_related_entity_summaries() {
        let bundle = WriteTimeBundle {
            contract_redlines: None,
            core_characters: vec![],
            relationship_lines: vec![],
            scene_outline: None,
            story_outline: None,
            world_setting: None,
            genre_antipatterns: vec![],
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
            related_entity_summaries: vec![
                "玄铁剑（Item）: 传说中的神兵".into(),
                "北境（Location）: 苦寒之地".into(),
            ],
            active_conflicts: None,
            character_goals: None,
            chase_debt_text: None,
            genre_reference: None,
            style_blend_text: None,
            rotation_ledger_text: None,
        };
        let prompt = bundle.to_prompt();
        assert!(prompt.contains("【相关设定】"));
        assert!(prompt.contains("玄铁剑（Item）"));
        assert!(prompt.contains("北境（Location）"));
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
                emotional_core: None,
                emotional_trigger: None,
                emotional_wound: None,
                emotional_need: None,
            }],
            relationship_lines: vec![],
            scene_outline: None,
            story_outline: None,
            world_setting: None,
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
            related_entity_summaries: vec![],
            active_conflicts: None,
            character_goals: None,
            chase_debt_text: None,
            genre_reference: None,
            style_blend_text: None,
            rotation_ledger_text: None,
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

    // ---- v0.30.15: story_outline 权威段渲染 ----

    fn bundle_with_outline(
        story_outline: Option<String>,
        redlines: Option<String>,
    ) -> WriteTimeBundle {
        WriteTimeBundle {
            contract_redlines: redlines,
            core_characters: vec![],
            relationship_lines: vec![],
            scene_outline: None,
            story_outline,
            world_setting: None,
            genre_antipatterns: vec![],
            style_slice: None,
            story_meta: StoryMeta {
                title: "t".to_string(),
                genre: None,
                tone: None,
                pacing: None,
                description: None,
            },
            genre_category: GenreCategory::Unknown,
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
            related_entity_summaries: vec![],
            active_conflicts: None,
            character_goals: None,
            chase_debt_text: None,
            genre_reference: None,
            style_blend_text: None,
            rotation_ledger_text: None,
        }
    }

    #[test]
    fn to_prompt_includes_story_outline_when_present() {
        let bundle = bundle_with_outline(
            Some("1. 首尔暗战：韩雪遭伏击\n2. 东京阴影：佐藤健拦截元件".to_string()),
            None,
        );
        let prompt = bundle.to_prompt();
        assert!(prompt.contains("故事大纲（本场景必须围绕此大纲展开"));
        assert!(prompt.contains("韩雪遭伏击"));
        assert!(prompt.contains("以本故事大纲为准"));
    }

    #[test]
    fn to_prompt_story_outline_after_redlines_invariant() {
        // 红线必须仍在故事大纲之前（红线第一不变量不被破坏）。
        let bundle = bundle_with_outline(
            Some("1. 首尔暗战".to_string()),
            Some("核心世界观不可违背".to_string()),
        );
        let prompt = bundle.to_prompt();
        let redlines_pos = prompt.find("世界观红线").expect("redlines section present");
        let outline_pos = prompt
            .find("故事大纲（本场景必须围绕")
            .expect("story outline section present");
        assert!(
            redlines_pos < outline_pos,
            "redlines must precede story outline"
        );
    }

    #[test]
    fn to_prompt_no_story_outline_section_when_absent() {
        let bundle = bundle_with_outline(None, None);
        let prompt = bundle.to_prompt();
        assert!(!prompt.contains("本场景必须围绕此大纲展开"));
    }

    fn empty_bundle() -> WriteTimeBundle {
        bundle_with_outline(None, None)
    }

    #[test]
    fn to_prompt_includes_emotional_fields_and_relationships() {
        let mut bundle = empty_bundle();
        bundle.core_characters = vec![CoreCharacter {
            name: "沈炼".into(),
            identity: None,
            physical_state: None,
            mental_state: None,
            location: None,
            personality: Some("隐忍".into()),
            emotional_core: Some("压抑的悲愤".into()),
            emotional_trigger: Some("被背叛".into()),
            emotional_wound: Some("师父之死".into()),
            emotional_need: Some("讨回公道".into()),
        }];
        bundle.relationship_lines = vec!["沈炼 -> 顾长夜：社会关系=同僚 ｜ 情感=仇恨[0.9]".into()];
        let prompt = bundle.to_prompt();
        assert!(prompt.contains("情感内核：压抑的悲愤"));
        assert!(prompt.contains("情感创伤：师父之死"));
        assert!(prompt.contains("角色情感关系"));
        assert!(prompt.contains("顾长夜"));
    }

    // ---- 设计第一节：续写链路资产贯通，新段落注入 ----

    #[test]
    fn to_prompt_new_asset_sections_rendered() {
        let mut bundle = bundle_with_outline(None, None);
        bundle.active_conflicts =
            Some("【当前活跃冲突】\n- 角色冲突: 涉及 张三, 李四, 赌注: 家族存亡".to_string());
        bundle.character_goals = Some("【角色当前状态】\n- 张三: 目标: 复仇".to_string());
        bundle.chase_debt_text =
            Some("【追读力债务】\n当前有 1 条待偿还的追读力债务，需在后续章节中兑现：".to_string());
        bundle.genre_reference = Some("元素参考表：\n境界体系表".to_string());
        let prompt = bundle.to_prompt();
        assert!(prompt.contains("【当前活跃冲突】"));
        assert!(prompt.contains("家族存亡"));
        assert!(prompt.contains("【角色当前状态】"));
        assert!(prompt.contains("目标: 复仇"));
        assert!(prompt.contains("【追读力债务】"));
        assert!(prompt.contains("【体裁元素参考】"));
        assert!(prompt.contains("境界体系表"));
    }

    #[test]
    fn to_prompt_new_asset_sections_skipped_when_none() {
        let bundle = bundle_with_outline(None, None);
        let prompt = bundle.to_prompt();
        assert!(!prompt.contains("【当前活跃冲突】"));
        assert!(!prompt.contains("【角色当前状态】"));
        assert!(!prompt.contains("【追读力债务】"));
        assert!(!prompt.contains("【体裁元素参考】"));
        assert!(!prompt.contains("【风格混合"));
    }

    // ---- v0.34.0 弹性扩张：⑧e 轮换账本段 ----

    #[test]
    fn to_prompt_includes_rotation_ledger_when_present() {
        let mut bundle = bundle_with_outline(None, None);
        bundle.rotation_ledger_text =
            Some("【场景与角色轮换账本（调度参考）】\n近 10 章场景使用：练功房×4".to_string());
        let prompt = bundle.to_prompt();
        assert!(prompt.contains("轮换账本"));
        assert!(prompt.contains("练功房×4"));
    }

    #[test]
    fn to_prompt_omits_rotation_ledger_when_absent() {
        let bundle = bundle_with_outline(None, None);
        assert!(!bundle.to_prompt().contains("轮换账本"));
    }

    #[test]
    fn to_prompt_style_blend_takes_precedence_over_single_dna() {
        let mut bundle = bundle_with_outline(None, None);
        bundle.style_blend_text = Some("风格混合 [燃爽融合]: 热血:70%, 冷峻:30%".to_string());
        bundle.style_dna_extension = Some("单DNA六维内容".to_string());
        let prompt = bundle.to_prompt();
        assert!(prompt.contains("【风格混合"));
        assert!(prompt.contains("热血:70%"));
        assert!(
            !prompt.contains("【风格 DNA 六维指标】"),
            "blend 存在时不再渲染单 DNA 段"
        );
    }

    #[test]
    fn to_prompt_single_dna_rendered_when_no_blend() {
        let mut bundle = bundle_with_outline(None, None);
        bundle.style_dna_extension = Some("单DNA六维内容".to_string());
        let prompt = bundle.to_prompt();
        assert!(prompt.contains("【风格 DNA 六维指标】"));
        assert!(!prompt.contains("【风格混合"));
    }

    #[test]
    fn format_writing_strategy_constraints_semantic() {
        let strategy = crate::config::settings::WritingStrategy {
            run_mode: "standard".to_string(),
            conflict_level: 85,
            pace: "fast".to_string(),
            ai_freedom: "medium".to_string(),
        };
        let text = format_writing_strategy_constraints(&strategy);
        assert!(text.contains("【写作策略约束】"));
        assert!(text.contains("运行模式：standard"));
        assert!(text.contains("冲突强度：极高"));
        assert!(text.contains("叙事节奏：快"));
        assert!(text.contains("AI 自由度：medium"));
        assert!(!text.contains("冲突强度：85"), "不再输出裸数字冲突强度");
    }

    #[test]
    fn test_resolve_methodology_extension_step_variant_hit() {
        // snowflake step3 有独立 md 文件，应命中 step 变体
        let ext = resolve_methodology_extension("snowflake", 3).expect("snowflake step3 应命中");
        assert!(
            ext.contains("雪花法第3步：角色概要"),
            "应使用 md frontmatter 的 name 作为标签: {}",
            ext
        );
        assert!(ext.contains("为每个主要角色写一页概要"));
    }

    #[test]
    fn test_resolve_methodology_extension_hdwb_legacy_alias() {
        // hdwb 文件未按 step 规范命名，走兼容映射；id 别名 hdwb 同样归一化
        let ext = resolve_methodology_extension("high_density_world_building", 2)
            .expect("hdwb step2 应命中旧命名 alias");
        assert!(ext.contains("状态网扩张"));
        let ext_alias = resolve_methodology_extension("hdwb", 1).expect("hdwb 别名应命中");
        assert!(ext_alias.contains("最小世界种子"));
    }

    #[test]
    fn test_resolve_methodology_extension_unknown_returns_none() {
        assert!(resolve_methodology_extension("nonexistent_methodology_xyz", 1).is_none());
    }

    #[test]
    fn test_resolve_methodology_base_file_fallback() {
        // hero_journey 无 step 变体文件，应回退 methodology_hero_journey 单文件
        let ext = resolve_methodology_extension("hero_journey", 5)
            .expect("hero_journey 应回退到无 step 后缀的单文件");
        assert!(ext.contains("英雄之旅"));
    }

    #[test]
    fn test_load_sync_dynamic_methodology_resolution() {
        // 集成断言：load_sync 走动态解析后，snowflake step2 的扩展来自 md 文件
        let pool = crate::db::create_test_pool().expect("test pool");
        let story = crate::db::StoryRepository::new(pool.clone())
            .create(crate::db::CreateStoryRequest {
                title: "动态解析测试".to_string(),
                description: None,
                genre: Some("玄幻".to_string()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: Some("snowflake".to_string()),
                reference_book_id: None,
            })
            .expect("create story");
        crate::db::StoryRepository::new(pool.clone())
            .update(
                &story.id,
                &crate::db::UpdateStoryRequest {
                    title: None,
                    description: None,
                    genre: None,
                    tone: None,
                    pacing: None,
                    style_dna_id: None,
                    genre_profile_id: None,
                    methodology_id: None,
                    methodology_step: Some(2),
                    reference_book_id: None,
                    strategy_json: None,
                    ..Default::default()
                },
            )
            .expect("set step 2");

        let bundle = WriteTimeBundle::load_sync(&pool, &story.id, 1, None, None, None)
            .expect("load_sync 应成功（各资产缺失均为软降级）");
        let ext = bundle
            .methodology_extension
            .expect("snowflake step2 应解析出方法论扩展");
        assert!(
            ext.contains("雪花法第2步"),
            "标签应来自 md frontmatter name: {}",
            ext
        );
    }

    #[test]
    fn test_new_character_policy_phase_aware() {
        // 发展期：允许有叙事功能的新角色
        let dev = new_character_policy_text(Some("当前叙事阶段：上升期。请逐步升级冲突……"));
        assert!(dev.contains("允许引入具有明确叙事功能的新角色"));
        // 高潮期：收敛
        let climax = new_character_policy_text(Some("当前叙事阶段：高潮期。请保持紧张节奏……"));
        assert!(climax.contains("不引入新的重要角色"));
        // 冲突激化期同样收敛
        assert!(
            new_character_policy_text(Some("当前叙事阶段：冲突激化期。"))
                .contains("不引入新的重要角色")
        );
        // 无阶段信息：默认发展期（扩张取向，替代旧「禁止自创新角色」一刀切）
        assert!(new_character_policy_text(None).contains("允许引入"));
    }

    #[test]
    fn test_load_sync_unknown_methodology_warns_and_skips() {
        // 未知 ID：log::warn!
        // 记录（测试无法直接断言日志），行为断言为跳过注入返回 None
        let pool = crate::db::create_test_pool().expect("test pool");
        let story = crate::db::StoryRepository::new(pool.clone())
            .create(crate::db::CreateStoryRequest {
                title: "未知方法论测试".to_string(),
                description: None,
                genre: Some("玄幻".to_string()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: Some("totally_unknown_methodology".to_string()),
                reference_book_id: None,
            })
            .expect("create story");

        let bundle = WriteTimeBundle::load_sync(&pool, &story.id, 1, None, None, None)
            .expect("未知方法论不应导致加载失败");
        assert!(
            bundle.methodology_extension.is_none(),
            "未知方法论 ID 应跳过注入"
        );
    }

    /// v0.64.7 真机契约：第 2 章写死的角色，第 10 章再加载写作包时
    /// 角色卡必须带「已死」标记（此前卡片只有位置，模型当活人写）。
    #[test]
    fn load_sync_marks_persisted_dead_character_in_card() {
        let pool = crate::db::create_test_pool().expect("test pool");
        let story = crate::db::StoryRepository::new(pool.clone())
            .create(crate::db::CreateStoryRequest {
                title: "已死角色卡".to_string(),
                description: None,
                genre: Some("历史".to_string()),
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .expect("create story");
        crate::db::CharacterRepository::new(pool.clone())
            .create(crate::db::CreateCharacterRequest {
                story_id: story.id.clone(),
                name: "明成公主".to_string(),
                ..Default::default()
            })
            .expect("create character");
        assert!(
            crate::story_system::life_status::mark_dead(&pool, &story.id, "明成公主", Some(2))
                .unwrap()
        );

        let bundle = WriteTimeBundle::load_sync(&pool, &story.id, 10, None, None, None)
            .expect("load_sync 应成功");
        let card = bundle
            .core_characters
            .iter()
            .find(|c| c.name == "明成公主")
            .expect("角色卡应在包里");
        let phys = card.physical_state.clone().unwrap_or_default();
        assert!(phys.contains("已死"), "phys={phys}");
        assert!(phys.contains("第2章"), "phys={phys}");
        assert!(
            bundle.to_prompt().contains("已死"),
            "提示词必须带上死亡标记"
        );
    }
}
