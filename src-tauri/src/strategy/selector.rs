//! Strategy Selector
//!
//! 调用 LLM 从资产目录中选择最适合当前场景的创作策略。

use std::collections::HashMap;

use super::models::{SelectableAsset, SelectedStrategy, SelectionContext, StrategyOverrides};
use crate::{error::AppError, llm::LlmService, router::TaskType};

/// 策略选择器
#[derive(Clone)]
pub struct StrategySelector {
    llm_service: LlmService,
}

impl StrategySelector {
    pub fn new(llm_service: LlmService) -> Self {
        Self { llm_service }
    }

    /// 为给定的创作场景选择策略
    pub async fn select_strategy(
        &self,
        context: &SelectionContext,
        assets: &[SelectableAsset],
        overrides: Option<&StrategyOverrides>,
    ) -> Result<SelectedStrategy, AppError> {
        // 1. 先尝试精确匹配 genre profile
        let mut strategy = exact_genre_match(context, assets);

        // 2. 调用 LLM 做最终选择
        let prompt = build_selection_prompt(context, assets, &strategy);
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

        // 3. 合并 LLM 结果与精确匹配兜底
        strategy = merge_strategies(strategy, llm_strategy);

        // 4. 应用用户覆盖
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

fn build_selection_prompt(
    context: &SelectionContext,
    assets: &[SelectableAsset],
    current_strategy: &SelectedStrategy,
) -> String {
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

    serde_json::from_str::<SelectedStrategy>(json_str).map_err(|e| {
        AppError::validation_failed(
            format!("Failed to parse strategy JSON: {}. Content: {}", e, content),
            None::<String>,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::asset_catalog::{genre_profile_assets, methodology_assets};

    fn sample_assets() -> Vec<SelectableAsset> {
        let profile = crate::db::GenreProfile {
            id: "apocalyptic".to_string(),
            genre_name: "末世流".to_string(),
            canonical_name: "Post-apocalyptic".to_string(),
            aliases_json: Some("[\"post-apocalyptic\", \"apocalyptic\", \"末世\"]".to_string()),
            core_tone: Some("文明崩溃后的世界".to_string()),
            pacing_strategy: Some("快节奏".to_string()),
            anti_patterns_json: Some("[]".to_string()),
            reference_tables_json: None,
            typical_structure_json: None,
            reader_promise: Some("怕,燃,生存压迫".to_string()),
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
}
