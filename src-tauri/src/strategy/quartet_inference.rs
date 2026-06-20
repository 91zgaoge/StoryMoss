//! 智能后台预访谈：根据输入清晰度 + 题材自动推断中文叙事四元组
//!
//! v0.17.1：完全后台决定，不打扰用户创作流。

use super::SelectedStrategy;
use crate::{
    creative_engine::{
        beat_cards::builtin_beat_cards, pressure_relationships::builtin_pressure_relationships,
        story_engines::builtin_story_engines,
    },
    intent::InputClarity,
};

/// 基于题材规范名 + 输入清晰度透明补全四元组（不调 LLM，纯启发式 + 资产匹配）。
pub fn infer_narrative_quartet(
    strategy: &mut SelectedStrategy,
    canonical_genre: Option<&str>,
    reader_promise: Option<&str>,
    clarity: InputClarity,
) {
    if !clarity.needs_quartet_inference() {
        return;
    }
    if strategy.emotional_payoff.is_none() {
        if let Some(promise) = reader_promise {
            if let Some(first) = promise.split(',').next() {
                let trimmed = first.trim();
                if !trimmed.is_empty() {
                    strategy.emotional_payoff = Some(trimmed.to_string());
                }
            }
        }
    }
    if strategy.conflict_arena.is_none() {
        strategy.conflict_arena = Some(default_arena_for(canonical_genre).to_string());
    }
    if strategy.pressure_relationship_id.is_none() {
        if let Some(rel_id) = recommend_pressure_relationship(canonical_genre) {
            strategy.pressure_relationship_id = Some(rel_id);
        }
    }
    if strategy.story_engine_ids.is_empty() {
        strategy.story_engine_ids = recommend_story_engines(canonical_genre);
    }
    if strategy.beat_card_ids.is_empty() {
        if let Some(card_id) = recommend_beat_card(canonical_genre) {
            strategy.beat_card_ids = vec![card_id];
        }
    }
}

fn default_arena_for(canonical: Option<&str>) -> &'static str {
    match canonical {
        Some("Cultivation") | Some("Xianxia") | Some("Xuanhuan") | Some("Wuxia") => "宗门大比",
        Some("Urban") | Some("Realistic") => "公司复盘会",
        Some("Romance") | Some("Light Novel") => "公开质证",
        Some("Suspense/Mystery") | Some("Tomb Raiding") | Some("Cthulhu/Lovecraftian") => {
            "法庭翻案"
        }
        Some("Historical") | Some("Hegemony/Conquest") | Some("National Destiny") => "朝堂质证",
        Some("Game/Esports") | Some("Sports") => "公开赛事",
        Some("Sci-Fi") | Some("Mecha / Stellar Warfare") | Some("Cyberpunk") => "听证会",
        Some("Post-apocalyptic") | Some("Doomsday Pioneer") => "公开抉择",
        Some("System") | Some("Quick Transmigration") | Some("Transmigration") => "公开舞台",
        _ => "公开舞台",
    }
}

fn recommend_pressure_relationship(canonical: Option<&str>) -> Option<String> {
    let id = match canonical {
        Some("Cultivation") | Some("Xianxia") | Some("Xuanhuan") | Some("Wuxia") => {
            "pressure_relationship.master_disciple_sect"
        }
        Some("Urban") | Some("Realistic") => "pressure_relationship.superior_vs_outsider",
        Some("Romance") | Some("Light Novel") => "pressure_relationship.ex_spouse",
        Some("Behind-the-Scenes") => "pressure_relationship.backstage_vs_frontstage",
        Some("Suspense/Mystery") | Some("Tomb Raiding") => {
            "pressure_relationship.rival_collaborator"
        }
        Some("Historical") | Some("Hegemony/Conquest") | Some("National Destiny") => {
            "pressure_relationship.true_vs_fake_heir"
        }
        Some("Sports") | Some("Game/Esports") => "pressure_relationship.rival_collaborator",
        Some("Post-apocalyptic") | Some("Doomsday Pioneer") => {
            "pressure_relationship.rescuer_vs_protected"
        }
        Some("Rebirth") | Some("Quick Transmigration") => "pressure_relationship.kin_vs_stepkin",
        _ => return None,
    };
    if builtin_pressure_relationships().iter().any(|r| r.id == id) {
        Some(id.to_string())
    } else {
        None
    }
}

fn recommend_story_engines(canonical: Option<&str>) -> Vec<String> {
    let candidates: &[&str] = match canonical {
        Some("Cultivation") | Some("Xianxia") | Some("Xuanhuan") => &[
            "story_engine.progression_ladder",
            "story_engine.public_arena",
        ],
        Some("Wuxia") => &["story_engine.hidden_identity", "story_engine.public_arena"],
        Some("Urban") | Some("Realistic") => &[
            "story_engine.voice_authority_flip",
            "story_engine.stakeholder_collision",
        ],
        Some("Romance") | Some("Light Novel") => &[
            "story_engine.contract_binding",
            "story_engine.mistaken_identity",
        ],
        Some("Suspense/Mystery") | Some("Tomb Raiding") => &[
            "story_engine.conspiracy_clue_chain",
            "story_engine.object_proof",
        ],
        Some("Cthulhu/Lovecraftian") | Some("Supernatural") | Some("Weird/Uncanny") => &[
            "story_engine.forbidden_bargain",
            "story_engine.sealed_memory",
        ],
        Some("Historical") | Some("Hegemony/Conquest") | Some("National Destiny") => &[
            "story_engine.class_displacement",
            "story_engine.public_arena",
        ],
        Some("Game/Esports") | Some("Sports") => {
            &["story_engine.trial_assessment", "story_engine.rule_exploit"]
        }
        Some("Post-apocalyptic") | Some("Doomsday Pioneer") => &[
            "story_engine.procedure_against_clock",
            "story_engine.forced_low_point",
        ],
        Some("System") | Some("Simulator") => &[
            "story_engine.progression_ladder",
            "story_engine.rule_exploit",
        ],
        Some("Rebirth") | Some("Quick Transmigration") | Some("Transmigration") => &[
            "story_engine.rebirth_second_chance",
            "story_engine.long_revenge_via_bait",
        ],
        Some("Behind-the-Scenes") => &[
            "story_engine.backstage_mission_pov",
            "story_engine.stakeholder_collision",
        ],
        Some("Cyberpunk") | Some("Sci-Fi") | Some("Mecha / Stellar Warfare") => &[
            "story_engine.conspiracy_clue_chain",
            "story_engine.rule_exploit",
        ],
        _ => &["story_engine.public_arena", "story_engine.object_proof"],
    };
    let builtin = builtin_story_engines();
    candidates
        .iter()
        .filter(|id| builtin.iter().any(|e| e.id == **id))
        .map(|s| s.to_string())
        .collect()
}

fn recommend_beat_card(canonical: Option<&str>) -> Option<String> {
    let id = match canonical {
        Some("Cultivation") | Some("Xianxia") | Some("Xuanhuan") => "beat_card.rule_exploit_win",
        Some("Wuxia") => "beat_card.hidden_skill_in_forbidden_zone",
        Some("Urban") | Some("Realistic") => "beat_card.voice_authority_flip",
        Some("Romance") | Some("Light Novel") => "beat_card.pride_and_misjudgment",
        Some("Suspense/Mystery") | Some("Tomb Raiding") => "beat_card.small_detail_overrules",
        Some("Cthulhu/Lovecraftian") | Some("Supernatural") => "beat_card.absurd_rule_trap",
        Some("Historical") | Some("Hegemony/Conquest") | Some("National Destiny") => {
            "beat_card.lost_legitimate_heir"
        }
        Some("Game/Esports") | Some("Sports") => "beat_card.rule_exploit_win",
        Some("Post-apocalyptic") | Some("Doomsday Pioneer") => "beat_card.countdown_survival",
        Some("System") => "beat_card.rule_exploit_win",
        Some("Rebirth") => "beat_card.long_revenge_via_bait",
        Some("Behind-the-Scenes") => "beat_card.backstage_pov_reframing",
        Some("Cyberpunk") | Some("Sci-Fi") => "beat_card.system_collapse_seer",
        _ => "beat_card.auction_appraisal",
    };
    if builtin_beat_cards().iter().any(|c| c.id == id) {
        Some(id.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vague_input_triggers_full_inference() {
        let mut s = SelectedStrategy::default();
        infer_narrative_quartet(
            &mut s,
            Some("Cultivation"),
            Some("燃,爽"),
            InputClarity::Vague,
        );
        assert_eq!(s.emotional_payoff, Some("燃".to_string()));
        assert!(s.conflict_arena.is_some());
        assert!(s.pressure_relationship_id.is_some());
        assert_eq!(s.story_engine_ids.len(), 2);
        assert_eq!(s.beat_card_ids.len(), 1);
    }

    #[test]
    fn full_concept_skips_inference() {
        let mut s = SelectedStrategy::default();
        infer_narrative_quartet(
            &mut s,
            Some("Cultivation"),
            Some("燃,爽"),
            InputClarity::WithFullConcept,
        );
        assert!(s.emotional_payoff.is_none());
        assert!(s.story_engine_ids.is_empty());
    }

    #[test]
    fn existing_values_preserved() {
        let mut s = SelectedStrategy::default();
        s.emotional_payoff = Some("虐".to_string());
        s.story_engine_ids = vec!["story_engine.hidden_identity".to_string()];
        infer_narrative_quartet(
            &mut s,
            Some("Cultivation"),
            Some("燃,爽"),
            InputClarity::Vague,
        );
        assert_eq!(s.emotional_payoff, Some("虐".to_string()));
        assert_eq!(s.story_engine_ids.len(), 1);
    }

    #[test]
    fn unknown_genre_uses_default() {
        let mut s = SelectedStrategy::default();
        infer_narrative_quartet(&mut s, Some("UnknownGenre"), None, InputClarity::Vague);
        assert!(s.conflict_arena.is_some());
        assert_eq!(s.story_engine_ids.len(), 2);
        assert!(s.beat_card_ids.len() == 1);
    }

    #[test]
    fn romance_pairs_with_ex_spouse_relationship() {
        let mut s = SelectedStrategy::default();
        infer_narrative_quartet(
            &mut s,
            Some("Romance"),
            Some("甜,虐"),
            InputClarity::WithSeed,
        );
        assert_eq!(
            s.pressure_relationship_id.as_deref(),
            Some("pressure_relationship.ex_spouse")
        );
        assert_eq!(s.emotional_payoff, Some("甜".to_string()));
    }
}

/// 把 SelectedStrategy 中的中文叙事四元组（含桥段卡）渲染为 LLM 可读的 JSON
/// 片段， 供 Writer prompt 在末尾追加。
///
/// 输出格式（含 builtin 资产的标题与功能描述）：
/// ```json
/// {
///   "emotional_payoff": "燃",
///   "conflict_arena": "宗门大比",
///   "pressure_relationship": { "name": "...", "pressure_source": "..." },
///   "story_engines": [{ "name": "...", "payoff": "...", "best_payoff": "..." }, ...],
///   "beat_cards": [{ "name": "...", "function": "...", "remix_hint": "...", "avoid": "..." }]
/// }
/// ```
///
/// 若 strategy 中四元组全部为空，返回 `Value::Null`，供调用方判断跳过注入。
pub fn serialize_quartet_for_prompt(
    strategy: &SelectedStrategy,
) -> Result<serde_json::Value, serde_json::Error> {
    let payoff = strategy.emotional_payoff.as_deref();
    let arena = strategy.conflict_arena.as_deref();
    let rel_id = strategy.pressure_relationship_id.as_deref();
    let engine_ids = &strategy.story_engine_ids;
    let card_ids = &strategy.beat_card_ids;

    if payoff.is_none()
        && arena.is_none()
        && rel_id.is_none()
        && engine_ids.is_empty()
        && card_ids.is_empty()
    {
        return Ok(serde_json::Value::Null);
    }

    let mut out = serde_json::Map::new();
    if let Some(p) = payoff {
        out.insert("emotional_payoff".to_string(), p.into());
    }
    if let Some(a) = arena {
        out.insert("conflict_arena".to_string(), a.into());
    }

    if let Some(rid) = rel_id {
        if let Some(rel) = builtin_pressure_relationships()
            .into_iter()
            .find(|r| r.id == rid)
        {
            // P0-4: 完整展开资产 payload，不再只传 name/pressure_source
            let works_with = if rel.works_with.is_empty() {
                String::new()
            } else {
                rel.works_with.join("、")
            };
            out.insert(
                "pressure_relationship".to_string(),
                serde_json::json!({
                    "name": rel.name,
                    "pressure_source": rel.pressure_source,
                    "works_with": works_with,
                }),
            );
        }
    }

    if !engine_ids.is_empty() {
        let engines = builtin_story_engines();
        let resolved: Vec<serde_json::Value> = engine_ids
            .iter()
            .filter_map(|id| engines.iter().find(|e| &e.id == id))
            .map(|e| {
                // P0-4: 增加 pairs_well_with 帮助作者正交组合引擎
                let pairs = if e.pairs_well_with.is_empty() {
                    String::new()
                } else {
                    e.pairs_well_with.join("、")
                };
                serde_json::json!({
                    "name": e.name,
                    "payoff": e.payoff,
                    "best_payoff": e.best_payoff,
                    "avoid": e.avoid,
                    "pairs_well_with": pairs,
                })
            })
            .collect();
        if !resolved.is_empty() {
            out.insert(
                "story_engines".to_string(),
                serde_json::Value::Array(resolved),
            );
        }
    }

    if !card_ids.is_empty() {
        let cards = builtin_beat_cards();
        let resolved: Vec<serde_json::Value> = card_ids
            .iter()
            .filter_map(|id| cards.iter().find(|c| &c.id == id))
            .map(|c| {
                // P0-4: 增加 category 标签与 when_to_use 适用场景
                serde_json::json!({
                    "name": c.name,
                    "category": c.category.label(),
                    "function": c.function,
                    "when_to_use": c.when_to_use,
                    "remix_hint": c.remix_hint,
                    "avoid": c.avoid,
                })
            })
            .collect();
        if !resolved.is_empty() {
            out.insert("beat_cards".to_string(), serde_json::Value::Array(resolved));
        }
    }

    Ok(serde_json::Value::Object(out))
}

#[cfg(test)]
mod prompt_serialization_tests {
    use super::*;

    #[test]
    fn empty_strategy_returns_null() {
        let s = SelectedStrategy::default();
        let v = serialize_quartet_for_prompt(&s).unwrap();
        assert!(v.is_null());
    }

    #[test]
    fn full_strategy_serializes() {
        let mut s = SelectedStrategy::default();
        s.emotional_payoff = Some("燃".to_string());
        s.conflict_arena = Some("宗门大比".to_string());
        s.pressure_relationship_id = Some("pressure_relationship.master_disciple_sect".to_string());
        s.story_engine_ids = vec![
            "story_engine.progression_ladder".to_string(),
            "story_engine.public_arena".to_string(),
        ];
        s.beat_card_ids = vec!["beat_card.rule_exploit_win".to_string()];
        let v = serialize_quartet_for_prompt(&s).unwrap();
        let o = v.as_object().expect("expected object");
        assert_eq!(
            o.get("emotional_payoff").and_then(|v| v.as_str()),
            Some("燃")
        );
        assert_eq!(
            o.get("conflict_arena").and_then(|v| v.as_str()),
            Some("宗门大比")
        );
        assert!(o.get("pressure_relationship").is_some());
        let engines = o.get("story_engines").and_then(|v| v.as_array()).unwrap();
        assert_eq!(engines.len(), 2);
        let cards = o.get("beat_cards").and_then(|v| v.as_array()).unwrap();
        assert_eq!(cards.len(), 1);
    }

    #[test]
    fn unknown_ids_filtered_out() {
        let mut s = SelectedStrategy::default();
        s.story_engine_ids = vec![
            "story_engine.progression_ladder".to_string(),
            "story_engine.nonexistent_id".to_string(),
        ];
        let v = serialize_quartet_for_prompt(&s).unwrap();
        let engines = v
            .as_object()
            .and_then(|o| o.get("story_engines"))
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(engines.len(), 1);
    }
}
