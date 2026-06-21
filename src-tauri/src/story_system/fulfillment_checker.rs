//! 章节合同履行度检查
//!
//! 纯同步、启发式：检查正文是否覆盖章节合同的 must_cover_nodes、
//! 是否触及 forbidden_zones，并给出 0.0-1.0 的履行度评分。

use serde::{Deserialize, Serialize};

use crate::domain::contracts::RuntimeContract;

/// 合同履行度结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FulfillmentResult {
    pub score: f64,
    pub covered_nodes: Vec<String>,
    pub violated_rules: Vec<String>,
    pub forbidden_zones_hit: Vec<String>,
}

/// 评估正文对运行时合同的履行情况。
///
/// 规则：
/// - 覆盖每个 must_cover_node 加分；未覆盖扣分。
/// - 触及每个 forbidden_zone 扣分并记录。
/// - 正文未体现 world_rules 时轻微扣分（作为简单一致性代理）。
pub fn evaluate_contract_fulfillment(
    content: &str,
    contract: &RuntimeContract,
) -> FulfillmentResult {
    let mut covered_nodes = Vec::new();
    let mut violated_rules = Vec::new();
    let mut forbidden_zones_hit = Vec::new();
    let mut score = 1.0_f64;

    if let Some(ch) = &contract.chapter_contract {
        for node in &ch.chapter_directive.must_cover_nodes {
            if node.is_empty() {
                continue;
            }
            if content.contains(node) {
                covered_nodes.push(node.clone());
            } else {
                score -= 0.15;
            }
        }

        for zone in &ch.chapter_directive.forbidden_zones {
            if zone.is_empty() {
                continue;
            }
            if content.contains(zone) {
                forbidden_zones_hit.push(zone.clone());
                score -= 0.25;
            }
        }
    }

    // 简单的世界规则代理：未出现则视为可能未遵守
    for rule in &contract.master_setting.world_rules {
        if rule.is_empty() {
            continue;
        }
        if !content.contains(rule) {
            violated_rules.push(format!("可能未体现规则: {}", rule));
            score -= 0.05;
        }
    }

    // 空内容保护
    if content.trim().is_empty() {
        score = 0.0;
        violated_rules.push("内容为空".to_string());
    }

    score = score.clamp(0.0, 1.0);

    FulfillmentResult {
        score,
        covered_nodes,
        violated_rules,
        forbidden_zones_hit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::contracts::{ChapterContract, ChapterDirective, MasterSettingContract};

    fn sample_contract() -> RuntimeContract {
        RuntimeContract {
            master_setting: MasterSettingContract {
                schema_version: "1".to_string(),
                contract_type: "MASTER_SETTING".to_string(),
                generator_version: "0.22.5".to_string(),
                genre: "玄幻".to_string(),
                core_tone: "黑暗压抑".to_string(),
                pacing_strategy: "慢热铺陈".to_string(),
                anti_patterns: vec![],
                world_rules: vec!["灵气不可再生".to_string()],
            },
            chapter_contract: Some(ChapterContract {
                schema_version: "1".to_string(),
                contract_type: "CHAPTER".to_string(),
                generator_version: "0.22.5".to_string(),
                chapter_number: 1,
                chapter_directive: ChapterDirective {
                    goal: "主角发现真相".to_string(),
                    must_cover_nodes: vec!["主角出场".to_string(), "灵气异常".to_string()],
                    forbidden_zones: vec!["提前揭示反派".to_string()],
                    time_anchor: None,
                    chapter_span: None,
                },
            }),
        }
    }

    #[test]
    fn perfect_fulfillment_scores_one() {
        let contract = sample_contract();
        let content = "主角出场，发现灵气异常，这个世界灵气不可再生。";
        let result = evaluate_contract_fulfillment(content, &contract);
        assert!((result.score - 1.0).abs() < f64::EPSILON);
        assert_eq!(result.covered_nodes.len(), 2);
        assert!(result.forbidden_zones_hit.is_empty());
    }

    #[test]
    fn missing_nodes_and_forbidden_zone_reduce_score() {
        let contract = sample_contract();
        let content = "提前揭示反派身份，但主角还没有出场。";
        let result = evaluate_contract_fulfillment(content, &contract);
        assert!(result.score < 1.0);
        assert!(result.score > 0.0);
        assert_eq!(result.forbidden_zones_hit.len(), 1);
        assert!(result.covered_nodes.is_empty());
    }
}
