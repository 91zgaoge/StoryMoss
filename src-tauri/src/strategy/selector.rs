//! Strategy Selector
//!
//! 调用 LLM 从资产目录中选择最适合当前场景的创作策略。

use std::collections::HashMap;

use super::models::{SelectableAsset, SelectionContext};
use crate::{
    book_deconstruction::repository::ReferenceBookRepository,
    db::{DbPool, StoryRepository},
    domain::strategy::{SelectedStrategy, StrategyOverrides},
    error::AppError,
    llm::LlmService,
    router::TaskType,
    strategy::GenreResolver,
};

/// 策略选择器
#[derive(Clone)]
pub struct StrategySelector {
    llm_service: LlmService,
    pool: DbPool,
}

impl StrategySelector {
    pub fn new(llm_service: LlmService, pool: DbPool) -> Self {
        Self { llm_service, pool }
    }

    /// 为给定的创作场景选择策略
    ///
    /// v0.22.x: 新增 `genre_repo` 参数，用于精确匹配失败时通过 `GenreResolver`
    /// 做模糊/复合题材匹配。传 `None` 时保持旧行为。
    pub async fn select_strategy(
        &self,
        context: &SelectionContext,
        assets: &[SelectableAsset],
        genre_repo: Option<&crate::db::GenreProfileRepository>,
        overrides: Option<&StrategyOverrides>,
    ) -> Result<SelectedStrategy, AppError> {
        // 1. 先尝试精确匹配 genre profile
        let mut strategy = exact_genre_match(context, assets);

        // 2. 精确失败时，使用 GenreResolver 做模糊/复合题材匹配
        if strategy.genre_profile_id.is_none() {
            if let Some(repo) = genre_repo {
                if let Some(hint) = context.genre_hint.as_deref() {
                    let resolver = GenreResolver::new();
                    if let Ok(matches) = resolver.resolve_from_hint(hint, repo) {
                        if let Some(first) = matches.first() {
                            strategy.genre_profile_id = Some(first.profile_id.clone());
                            strategy.rationale = format!(
                                "GenreResolver 模糊匹配到 '{}'/{} (score={:.1})",
                                first.genre_name, first.canonical_name, first.score
                            );
                            if matches.len() > 1 {
                                let secondary: Vec<String> = matches
                                    .iter()
                                    .skip(1)
                                    .map(|m| m.profile_id.clone())
                                    .collect();
                                strategy.parameters.insert(
                                    "secondary_genre_profile_ids".to_string(),
                                    serde_json::json!(secondary),
                                );
                            }
                        }
                    }
                }
            }
        }

        // 3. 调用 LLM 做最终选择
        let prompt = build_selection_prompt(context, assets, &strategy, &self.pool);
        let response = self
            .llm_service
            .generate_for_task(
                TaskType::Analysis,
                prompt,
                Some(1024),
                Some(0.3),
                Some("strategy_select"),
            )
            .await?;

        let llm_strategy = parse_strategy_response(&response.content)?;

        // 4. 合并 LLM 结果与精确匹配兜底
        strategy = merge_strategies(strategy, llm_strategy);

        // 5. 应用用户覆盖
        if let Some(ov) = overrides {
            strategy.merge_user_overrides(ov);
        }

        Ok(strategy)
    }
}

/// 根据 genre hint 从资产中精确匹配 genre profile
pub fn exact_genre_match(
    context: &SelectionContext,
    assets: &[SelectableAsset],
) -> SelectedStrategy {
    let mut strategy = SelectedStrategy::default();

    // Phase 1.5: 若 LLM 已输出标准化 genre_profile_ids，优先直接匹配
    if !context.preferred_genre_profile_ids.is_empty() {
        for preferred_id in &context.preferred_genre_profile_ids {
            for asset in assets {
                if !matches!(asset.kind, super::models::AssetKind::GenreProfile) {
                    continue;
                }
                let id = asset.id.strip_prefix("genre_profile.").unwrap_or(&asset.id);
                if id == preferred_id {
                    strategy.genre_profile_id = Some(preferred_id.clone());
                    strategy.rationale = format!("LLM 标准化题材画像 '{}'", asset.name);
                    return strategy;
                }
            }
        }
    }

    let hint = match &context.genre_hint {
        Some(h) if !h.trim().is_empty() => h.trim().to_lowercase(),
        _ => return strategy,
    };

    for asset in assets {
        if !matches!(asset.kind, super::models::AssetKind::GenreProfile) {
            continue;
        }
        let genre_name = asset
            .payload
            .get("genre_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        let canonical = asset
            .payload
            .get("canonical_name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        let aliases: Vec<String> = asset
            .payload
            .get("aliases")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                    .collect()
            })
            .unwrap_or_default();

        if genre_name == hint || canonical == hint || aliases.iter().any(|a| a == &hint) {
            let id = asset.id.strip_prefix("genre_profile.").unwrap_or(&asset.id);
            strategy.genre_profile_id = Some(id.to_string());
            strategy.rationale = format!("精确匹配到体裁画像 '{}'", asset.name);
            break;
        }
    }

    strategy
}

fn merge_strategies(a: SelectedStrategy, b: SelectedStrategy) -> SelectedStrategy {
    SelectedStrategy {
        rationale: if b.rationale.is_empty() {
            a.rationale
        } else {
            format!("{}；LLM: {}", a.rationale, b.rationale)
        },
        genre_profile_id: b.genre_profile_id.or(a.genre_profile_id),
        methodology_id: b.methodology_id.or(a.methodology_id),
        style_dna_ids: if b.style_dna_ids.is_empty() {
            a.style_dna_ids
        } else {
            b.style_dna_ids
        },
        skill_ids: if b.skill_ids.is_empty() {
            a.skill_ids
        } else {
            b.skill_ids
        },
        workflow_id: b.workflow_id.or(a.workflow_id),
        parameters: if b.parameters.is_empty() {
            a.parameters
        } else {
            b.parameters
        },
        // v0.17.0 中文叙事增强字段：LLM 输出（b）优先，fallback 到种子（a）
        emotional_payoff: b.emotional_payoff.or(a.emotional_payoff),
        pressure_relationship_id: b.pressure_relationship_id.or(a.pressure_relationship_id),
        conflict_arena: b.conflict_arena.or(a.conflict_arena),
        story_engine_ids: if b.story_engine_ids.is_empty() {
            a.story_engine_ids
        } else {
            b.story_engine_ids
        },
        beat_card_ids: if b.beat_card_ids.is_empty() {
            a.beat_card_ids
        } else {
            b.beat_card_ids
        },
    }
}

/// 根据 story_id 加载关联参考书籍的分析摘要。
///
/// 任何失败（无故事、无关联书籍、数据库错误）都返回 `None`，
/// 保证策略选择不被阻塞。
fn load_reference_book_summary(
    story_id: Option<&str>,
    pool: &DbPool,
) -> Option<crate::book_deconstruction::models::ReferenceBookSummary> {
    let story_id = story_id?;
    let story = StoryRepository::new(pool.clone())
        .get_by_id(story_id)
        .ok()
        .flatten()?;
    let book_id = story.reference_book_id?;
    ReferenceBookRepository::new(pool.clone())
        .get_book_analysis_summary(&book_id)
        .ok()
        .flatten()
}

/// 将参考书籍摘要注入模板变量表。
fn inject_reference_book_vars(
    vars: &mut std::collections::HashMap<String, String>,
    summary: Option<&crate::book_deconstruction::models::ReferenceBookSummary>,
) {
    let empty = String::new();
    vars.insert("reference_book_title".to_string(), empty.clone());
    vars.insert("reference_book_genre".to_string(), empty.clone());
    vars.insert("reference_book_world_keywords".to_string(), empty.clone());
    vars.insert("reference_book_arc_type".to_string(), empty.clone());
    vars.insert("reference_book_tone".to_string(), empty);

    if let Some(s) = summary {
        vars.insert("reference_book_title".to_string(), s.title.clone());
        if let Some(ref genre) = s.genre {
            vars.insert("reference_book_genre".to_string(), genre.clone());
        }
        if !s.world_keywords.is_empty() {
            vars.insert(
                "reference_book_world_keywords".to_string(),
                s.world_keywords.join(", "),
            );
        }
        if let Some(ref arc) = s.arc_type {
            vars.insert("reference_book_arc_type".to_string(), arc.clone());
        }
        if let Some(ref tone) = s.tone {
            vars.insert("reference_book_tone".to_string(), tone.clone());
        }
    }
}

fn build_selection_prompt(
    context: &SelectionContext,
    assets: &[SelectableAsset],
    current_strategy: &SelectedStrategy,
    pool: &DbPool,
) -> String {
    // Phase 3.2: 加载关联参考书籍分析摘要（非阻塞）
    let ref_summary = load_reference_book_summary(context.story_id.as_deref(), pool);

    // v0.21.0: 优先从 PromptRegistry 读取覆盖
    if let Some(tpl) = crate::prompts::registry::resolve_prompt(pool, "strategy_selector")
        .ok()
        .or_else(|| crate::prompts::registry::resolve_prompt_default("strategy_selector"))
    {
        let context_str = format!(
            "user_input: {}\nstory_progress: {}\nhas_story: {}",
            context.user_input, context.story_progress, context.has_story
        );
        let assets_str = assets
            .iter()
            .map(|a| format!("- {}: {}", a.id, a.name))
            .collect::<Vec<_>>()
            .join("\n");
        let mut vars = std::collections::HashMap::new();
        vars.insert("context".to_string(), context_str);
        vars.insert("available_assets".to_string(), assets_str);
        inject_reference_book_vars(&mut vars, ref_summary.as_ref());
        return crate::prompts::engine::TemplateEngine::render_with_conditions(&tpl, &vars);
    }

    let mut sections: Vec<String> = vec![
        "You are a creative strategy selector for a Chinese web-novel writing assistant."
            .to_string(),
        "Your task: choose the best combination of creative assets for the current task."
            .to_string(),
        "".to_string(),
        "Current scene:".to_string(),
        format!("- user input: {}", context.user_input),
        format!("- story progress: {}", context.story_progress),
        format!("- has story: {}", context.has_story),
        format!(
            "- genre hint: {}",
            context.genre_hint.as_deref().unwrap_or("none")
        ),
        format!(
            "- methodology hint: {}",
            context.methodology_hint.as_deref().unwrap_or("none")
        ),
        format!(
            "- word count target: {}",
            context
                .word_count_target
                .map(|n| n.to_string())
                .unwrap_or_else(|| "default".to_string())
        ),
    ];

    if !current_strategy.rationale.is_empty() {
        sections.push("".to_string());
        sections.push("Pre-matched hint:".to_string());
        sections.push(format!(
            "- genre_profile_id: {} ({})",
            current_strategy
                .genre_profile_id
                .as_deref()
                .unwrap_or("none"),
            current_strategy.rationale
        ));
    }

    sections.push("".to_string());
    sections.push("Available assets:".to_string());

    // v0.22.1: 注入题材→风格推荐映射（意见1）
    // 让 StrategySelector 知道各题材的推荐风格/方法论，
    // 而非仅凭风格名称凭空推断
    if let Some(ref genre_hint) = context.genre_hint {
        let recommendations = get_genre_recommendations(genre_hint);
        if !recommendations.is_empty() {
            sections.push("".to_string());
            sections.push(
                "Genre-based recommendations (prefer these for the given genre):".to_string(),
            );
            sections.push(recommendations);
        }
    }

    // Phase 3.2: 注入关联参考书籍上下文（fallback prompt）
    if let Some(ref summary) = ref_summary {
        sections.push("".to_string());
        sections.push("Reference book context:".to_string());
        sections.push(format!("- title: {}", summary.title));
        if let Some(ref genre) = summary.genre {
            sections.push(format!("- genre: {}", genre));
        }
        if !summary.world_keywords.is_empty() {
            sections.push(format!(
                "- world keywords: {}",
                summary.world_keywords.join(", ")
            ));
        }
        if let Some(ref arc) = summary.arc_type {
            sections.push(format!("- story arc type: {}", arc));
        }
        if let Some(ref tone) = summary.tone {
            sections.push(format!("- tone: {}", tone));
        }
    }

    // 按 kind 分组，控制总长度
    let mut by_kind: HashMap<String, Vec<&SelectableAsset>> = HashMap::new();
    for asset in assets {
        by_kind
            .entry(asset.kind.to_string())
            .or_default()
            .push(asset);
    }

    let mut total_chars = sections.iter().map(|s| s.chars().count()).sum::<usize>();
    let max_total = 8000usize;

    for (kind, items) in by_kind {
        sections.push(format!("\n## {}", kind));
        for (idx, asset) in items.iter().enumerate() {
            let entry = asset.to_prompt_entry();
            let entry_len = entry.chars().count();
            if total_chars + entry_len > max_total {
                let remaining = items.len().saturating_sub(idx);
                sections.push(format!(
                    "- ... ({} more {} assets omitted)",
                    remaining, kind
                ));
                break;
            }
            sections.push(entry);
            total_chars += entry_len;
        }
    }

    sections.push("".to_string());
    sections.push(format!(
        "Respond with JSON:\n{}\n\nRules:\n1. Choose exactly one genre_profile_id if relevant.\n2. Choose exactly one methodology_id.\n3. style_dna_ids and skill_ids can be empty or multiple.\n4. rationale must explain why these assets fit the user input and genre.\n5. Only use IDs that appear above.",
        serde_json::json!({
            "rationale": "...",
            "genre_profile_id": "optional id without prefix",
            "methodology_id": "optional id without prefix",
            "style_dna_ids": ["..."],
            "skill_ids": ["..."],
            "workflow_id": "optional id without prefix",
            "parameters": {}
        }).to_string()
    ));

    sections.join("\n")
}

fn parse_strategy_response(content: &str) -> Result<SelectedStrategy, AppError> {
    let trimmed = content.trim();
    let json_str = if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        &trimmed[start..=end]
    } else {
        trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim()
    };

    // v0.26.28 hotfix: 优先按标准 SelectedStrategy schema 解析；
    // 失败时回退到旧版 schema（selected_strategy/reasoning/asset_combination），
    // 避免 prompts 外部化过程中旧格式残留导致 Genesis 策略选择步骤失败。
    if let Ok(strategy) = serde_json::from_str::<SelectedStrategy>(json_str) {
        return Ok(strategy);
    }

    if let Ok(legacy) = serde_json::from_str::<LegacyStrategyResponse>(json_str) {
        return Ok(legacy.into_selected_strategy());
    }

    Err(AppError::validation_failed(
        format!(
            "Failed to parse strategy JSON: content does not match SelectedStrategy or legacy schema. Content: {}",
            content
        ),
        None::<String>,
    ))
}

/// v0.26.28 hotfix: 旧版策略选择响应 schema，用于兼容 prompts 外部化前
/// 可能残留的 prompt 模板（字段名为
/// selected_strategy/reasoning/asset_combination）。
#[derive(Debug, Clone, serde::Deserialize)]
struct LegacyStrategyResponse {
    #[serde(default)]
    selected_strategy: Option<String>,
    #[serde(default)]
    reasoning: String,
    #[serde(default)]
    asset_combination: Vec<String>,
    #[serde(default)]
    parameters: HashMap<String, serde_json::Value>,
}

impl LegacyStrategyResponse {
    fn into_selected_strategy(self) -> SelectedStrategy {
        let mut strategy = SelectedStrategy::default();
        strategy.rationale = if self.reasoning.is_empty() {
            "Legacy strategy response".to_string()
        } else {
            self.reasoning
        };
        strategy.parameters = self.parameters;

        for asset in self.asset_combination {
            if let Some((prefix, id)) = asset.split_once('.') {
                match prefix {
                    "genre_profile" => strategy.genre_profile_id = Some(id.to_string()),
                    "methodology" => strategy.methodology_id = Some(id.to_string()),
                    "style_dna" => strategy.style_dna_ids.push(id.to_string()),
                    "skill" => strategy.skill_ids.push(id.to_string()),
                    "workflow" => strategy.workflow_id = Some(id.to_string()),
                    _ => {}
                }
            }
        }

        // selected_strategy 通常指向 methodology，作为兜底
        if let Some(selected) = self.selected_strategy {
            if strategy.methodology_id.is_none() {
                if let Some((prefix, id)) = selected.split_once('.') {
                    if prefix == "methodology" {
                        strategy.methodology_id = Some(id.to_string());
                    }
                }
            }
        }

        strategy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::asset_catalog::{genre_profile_assets, methodology_assets};

    fn sample_assets() -> Vec<SelectableAsset> {
        // 使用与 templates/genres.json 一致的真实 aliases，避免测试数据与真实数据不一致
        let profile = crate::db::GenreProfile {
            id: "apocalyptic".to_string(),
            genre_name: "末世流".to_string(),
            canonical_name: "Post-apocalyptic".to_string(),
            aliases_json: Some("[\"post-apocalyptic\", \"apocalyptic\", \"末世\", \"末日\", \"废土\", \"末世生存\"]".to_string()),
            core_tone: Some("文明崩溃后的世界".to_string()),
            pacing_strategy: Some("快节奏".to_string()),
            anti_patterns_json: Some("[]".to_string()),
            reference_tables_json: None,
            typical_structure_json: None,
            reader_promise: Some("怕,燃,生存压迫".to_string()),
            recommended_style_dna_ids: Some("[\"余华\",\"海明威\"]".to_string()),
            recommended_methodology_id: Some("hero_journey".to_string()),
            recommended_skill_ids: Some("[\"emotion_pacing\"]".to_string()),
            min_quality_tier: Some("high".to_string()),
            is_builtin: true,
            created_at: chrono::Local::now(),
        };

        let mut assets = methodology_assets();
        assets.extend(genre_profile_assets(&[profile]));
        assets
    }

    #[test]
    fn test_exact_genre_match_by_name() {
        let mut ctx = SelectionContext::default();
        ctx.genre_hint = Some("末世流".to_string());

        let strategy = exact_genre_match(&ctx, &sample_assets());
        assert_eq!(strategy.genre_profile_id, Some("apocalyptic".to_string()));
    }

    #[test]
    fn test_exact_genre_match_by_alias() {
        let mut ctx = SelectionContext::default();
        ctx.genre_hint = Some("末世".to_string());

        let strategy = exact_genre_match(&ctx, &sample_assets());
        assert_eq!(strategy.genre_profile_id, Some("apocalyptic".to_string()));
    }

    #[test]
    fn test_exact_genre_match_prefers_profile_ids() {
        let mut ctx = SelectionContext::default();
        ctx.genre_hint = Some("完全不相关".to_string());
        ctx.preferred_genre_profile_ids = vec!["apocalyptic".to_string()];

        let strategy = exact_genre_match(&ctx, &sample_assets());
        assert_eq!(strategy.genre_profile_id, Some("apocalyptic".to_string()));
    }

    #[test]
    fn test_parse_strategy_response_valid() {
        let json = r#"{"rationale": "适合末世", "genre_profile_id": "apocalyptic", "methodology_id": "high_density_world_building", "style_dna_ids": [], "skill_ids": ["builtin.style_enhancer"], "workflow_id": null, "parameters": {}}"#;
        let strategy = parse_strategy_response(json).unwrap();
        assert_eq!(strategy.genre_profile_id, Some("apocalyptic".to_string()));
        assert_eq!(
            strategy.methodology_id,
            Some("high_density_world_building".to_string())
        );
        assert_eq!(strategy.skill_ids, vec!["builtin.style_enhancer"]);
    }

    // v0.26.28 hotfix: prompts 外部化后残留的 legacy schema 仍能解析，
    // 避免 Genesis 策略选择步骤因字段名不一致而失败。
    #[test]
    fn test_parse_strategy_response_legacy_schema() {
        let json = r#"{"selected_strategy": "methodology.scene_structure", "reasoning": "选择场景结构模板以帮助构建一个有结构的故事，同时利用其他策略和资源提升剧情深度和文风。", "asset_combination": ["methodology.snowflake", "genre_profile.418c516d-66aa-42ec-bfe5-09003249446b"], "parameters": {"temperature": 0.8, "max_tokens": 2500}}"#;
        let strategy = parse_strategy_response(json).unwrap();
        assert_eq!(strategy.methodology_id, Some("snowflake".to_string()));
        assert_eq!(
            strategy.genre_profile_id,
            Some("418c516d-66aa-42ec-bfe5-09003249446b".to_string())
        );
        assert!(!strategy.rationale.is_empty());
    }

    #[test]
    fn test_merge_strategies_prefers_llm() {
        let a = SelectedStrategy {
            genre_profile_id: Some("a".to_string()),
            methodology_id: Some("m_a".to_string()),
            ..Default::default()
        };
        let b = SelectedStrategy {
            genre_profile_id: Some("b".to_string()),
            methodology_id: Some("m_b".to_string()),
            style_dna_ids: vec!["style_1".to_string()],
            rationale: "LLM choice".to_string(),
            ..Default::default()
        };
        let merged = merge_strategies(a, b);
        assert_eq!(merged.genre_profile_id, Some("b".to_string()));
        assert_eq!(merged.methodology_id, Some("m_b".to_string()));
        assert_eq!(merged.style_dna_ids, vec!["style_1".to_string()]);
    }

    /// 审计测试：复合题材 "异星球末世生存" 的精确匹配能力
    ///
    /// 该测试模拟 LLM 从用户输入 "写一部异星球末世生存题材的小说" 中可能提取的
    /// genre 字符串，并验证当前 `exact_genre_match` 的匹配结果。
    /// 目的：暴露复合题材/别名覆盖不足导致的后台资产断链风险。
    #[test]
    fn test_compound_genre_exact_match_audit() {
        // 使用与 templates/genres.json 一致的真实资产字段
        let profiles = vec![
            crate::db::GenreProfile {
                id: "apocalyptic".to_string(),
                genre_name: "末世流".to_string(),
                canonical_name: "Post-apocalyptic".to_string(),
                aliases_json: Some("[\"post-apocalyptic\", \"apocalyptic\"]".to_string()),
                core_tone: Some("文明崩溃后的世界...".to_string()),
                pacing_strategy: None,
                anti_patterns_json: None,
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: Some("怕,燃,生存压迫".to_string()),
                recommended_style_dna_ids: Some("[\"余华\",\"海明威\",\"鲁迅\"]".to_string()),
                recommended_methodology_id: Some("hero_journey".to_string()),
                recommended_skill_ids: Some("[\"emotion_pacing\",\"character_voice\"]".to_string()),
                min_quality_tier: Some("high".to_string()),
                is_builtin: true,
                created_at: chrono::Local::now(),
            },
            crate::db::GenreProfile {
                id: "doomsday-pioneer".to_string(),
                genre_name: "末日拓荒".to_string(),
                canonical_name: "Doomsday Pioneer".to_string(),
                aliases_json: Some(
                    "[\"doomsday pioneer\", \"post-apocalyptic pioneer\"]".to_string(),
                ),
                core_tone: Some("末日后的开拓史...".to_string()),
                pacing_strategy: None,
                anti_patterns_json: None,
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: Some("燃,爽".to_string()),
                recommended_style_dna_ids: None,
                recommended_methodology_id: None,
                recommended_skill_ids: None,
                min_quality_tier: Some("high".to_string()),
                is_builtin: true,
                created_at: chrono::Local::now(),
            },
            crate::db::GenreProfile {
                id: "scifi".to_string(),
                genre_name: "科幻".to_string(),
                canonical_name: "Sci-Fi".to_string(),
                aliases_json: Some("[\"sci-fi\", \"science fiction\"]".to_string()),
                core_tone: Some("科学技术驱动...".to_string()),
                pacing_strategy: None,
                anti_patterns_json: None,
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: Some("燃,惊".to_string()),
                recommended_style_dna_ids: Some("[\"海明威\",\"余华\",\"王小波\"]".to_string()),
                recommended_methodology_id: Some("hero_journey".to_string()),
                recommended_skill_ids: Some("[\"style_enhancer\",\"emotion_pacing\"]".to_string()),
                min_quality_tier: Some("high".to_string()),
                is_builtin: true,
                created_at: chrono::Local::now(),
            },
            crate::db::GenreProfile {
                id: "mecha-stellar".to_string(),
                genre_name: "星际机甲".to_string(),
                canonical_name: "Mecha / Stellar Warfare".to_string(),
                aliases_json: Some("[\"mecha/stellar warfare\", \"mecha\"]".to_string()),
                core_tone: Some("星际战争与机甲...".to_string()),
                pacing_strategy: None,
                anti_patterns_json: None,
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: Some("燃,爽".to_string()),
                recommended_style_dna_ids: None,
                recommended_methodology_id: None,
                recommended_skill_ids: None,
                min_quality_tier: Some("high".to_string()),
                is_builtin: true,
                created_at: chrono::Local::now(),
            },
        ];

        let assets = genre_profile_assets(&profiles);

        // LLM 从 "写一部异星球末世生存题材的小说" 可能提取的 genre 字符串
        let cases: Vec<(&str, Option<&str>)> = vec![
            ("异星球末世生存", None), // 复合新词，无精确匹配
            ("末世科幻", None),       // 复合词，无精确匹配
            ("科幻末世", None),       // 复合词，无精确匹配
            ("异星球", None),         // 非标准别名
            ("末世", None),           // 真实 aliases 中未包含 "末世"
            ("生存", None),           // 主题词，非体裁名
            ("末世流", Some("apocalyptic")),
            ("Post-apocalyptic", Some("apocalyptic")),
            ("Sci-Fi", Some("scifi")),
            ("Mecha / Stellar Warfare", Some("mecha-stellar")),
            ("星际机甲", Some("mecha-stellar")),
        ];

        for (hint, expected_id) in cases {
            let mut ctx = SelectionContext::default();
            ctx.genre_hint = Some(hint.to_string());
            let strategy = exact_genre_match(&ctx, &assets);
            assert_eq!(
                strategy.genre_profile_id.as_deref(),
                expected_id,
                "genre_hint='{}' 应匹配 {:?}, 实际匹配 {:?}",
                hint,
                expected_id,
                strategy.genre_profile_id
            );
        }
    }
}

/// v0.22.1: 题材→风格推荐映射表（意见1）
///
/// 让 StrategySelector 在 LLM 选择前获得预置推荐，
/// 而非仅凭风格名称凭空推断。推荐基于网文创作领域共识。
fn get_genre_recommendations(genre_hint: &str) -> String {
    let g = genre_hint.to_lowercase();
    let (styles, methodology, skills) =
        if g.contains("末世") || g.contains("apocalypse") || g.contains("废土") {
            (
                "余华（冷酷白描,苦难叙事）> 海明威（极简白描,短句为主）> 鲁迅（冷峻讽刺）",
                "hero_journey（英雄之旅,12阶段生存叙事）",
                "emotion_pacing（情感节奏优化）, character_voice（角色声音一致性）",
            )
        } else if g.contains("玄幻") || g.contains("xianxia") || g.contains("仙侠") {
            (
                "金庸（武侠诗意）> 曹雪芹（华丽古典）> 莫言（魔幻现实）",
                "snowflake（雪花写作法,多轮展开）",
                "style_enhancer（风格增强）, character_voice",
            )
        } else if g.contains("都市") || g.contains("urban") || g.contains("现实") {
            (
                "张爱玲（细腻心理）> 老舍（京味白描）> 余华（冷酷白描）",
                "scene_structure（场景结构）",
                "character_voice, emotion_pacing",
            )
        } else if g.contains("科幻") || g.contains("sci-fi") {
            (
                "海明威（极简白描）> 余华（冷酷白描）> 王小波（黑色幽默）",
                "hero_journey",
                "style_enhancer, emotion_pacing",
            )
        } else if g.contains("悬疑") || g.contains("推理") || g.contains("mystery") {
            (
                "鲁迅（冷峻讽刺）> 海明威（极简白描）> 黑色侦探",
                "scene_structure",
                "emotion_pacing",
            )
        } else if g.contains("古言") || g.contains("历史") {
            (
                "曹雪芹（华丽古典）> 金庸（武侠诗意）> 张爱玲（细腻心理）",
                "snowflake（雪花写作法）",
                "style_enhancer, character_voice",
            )
        } else {
            return String::new();
        };
    format!(
        "- Style DNA: {}\n- Methodology: {}\n- Skills: {}",
        styles, methodology, skills
    )
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    /// 旧版 schema（selected_strategy/reasoning/asset_combination）的兼容解析。
    /// 该 JSON 缺少 SelectedStrategy 的 style_dna_ids/skill_ids 等必填数组，
    /// 因此会走 LegacyStrategyResponse 分支；reasoning 应映射到 rationale。
    #[test]
    fn test_parse_strategy_response_accepts_legacy_reasoning() {
        let json = r#"{
            "selected_strategy": "methodology.scene_structure",
            "reasoning": "选择场景结构模板以帮助构建一个有结构的故事",
            "asset_combination": ["methodology.snowflake", "genre_profile.418c516d-66aa-42ec-bfe5-09003249446b"],
            "parameters": { "temperature": 0.8, "max_tokens": 2500 }
        }"#;

        let strategy = parse_strategy_response(json).unwrap();
        assert_eq!(
            strategy.rationale,
            "选择场景结构模板以帮助构建一个有结构的故事"
        );
        assert_eq!(strategy.methodology_id, Some("snowflake".to_string()));
        assert_eq!(
            strategy.genre_profile_id,
            Some("418c516d-66aa-42ec-bfe5-09003249446b".to_string())
        );
    }

    /// 标准 schema 中 rationale 缺失时，应使用默认值空字符串，而不是解析失败。
    #[test]
    fn test_parse_strategy_response_defaults_missing_rationale() {
        let json = r#"{
            "rationale": "",
            "genre_profile_id": "apocalyptic",
            "methodology_id": "hero_journey",
            "style_dna_ids": [],
            "skill_ids": [],
            "story_engine_ids": [],
            "beat_card_ids": [],
            "parameters": {}
        }"#;

        let strategy = parse_strategy_response(json).unwrap();
        assert!(strategy.rationale.is_empty());
        assert_eq!(strategy.genre_profile_id, Some("apocalyptic".to_string()));
        assert_eq!(strategy.methodology_id, Some("hero_journey".to_string()));
    }

    /// 标准 schema 中 rationale 字段使用旧名 reasoning 时，应通过 alias
    /// 正确解析。
    #[test]
    fn test_parse_strategy_response_accepts_reasoning_alias() {
        let json = r#"{
            "reasoning": "使用 reasoning 别名",
            "genre_profile_id": "apocalyptic",
            "methodology_id": "hero_journey",
            "style_dna_ids": [],
            "skill_ids": [],
            "story_engine_ids": [],
            "beat_card_ids": []
        }"#;

        let strategy = parse_strategy_response(json).unwrap();
        assert_eq!(strategy.rationale, "使用 reasoning 别名");
    }
}
