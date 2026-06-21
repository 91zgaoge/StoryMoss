//! 轻量级章节 mini review
//!
//! 在 SceneCommitService::auto_commit 中异步执行，生成 review_result_json。
//! 若 LLM 不可用或调用失败，自动回退到基于关键词的启发式评分，
//! 确保 commit 流程不会被阻塞。

use serde::{Deserialize, Serialize};

use crate::{
    domain::contracts::RuntimeContract, error::AppError, llm::service::LlmService,
    prompts::registry::resolve_prompt_default_with_vars, router::TaskType,
};

/// Mini review 结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub score: f64,
    pub dimensions: Vec<ReviewDimension>,
    pub summary: String,
    pub issues: Vec<String>,
}

/// 单一维度评分
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewDimension {
    pub name: String,
    pub score: f64,
    pub comment: String,
}

/// 运行 mini review。
///
/// 优先使用 LLM（通过 `mini_review_system` PromptRegistry 提示词）返回结构化
/// JSON； 若 LLM 不可用、超时或返回无法解析，则回退到启发式评分。
pub async fn run_mini_review(
    content: &str,
    contract: &RuntimeContract,
    llm_service: Option<&LlmService>,
) -> Result<ReviewResult, AppError> {
    if let Some(service) = llm_service {
        let vars = build_prompt_vars(content, contract);
        match resolve_prompt_default_with_vars("mini_review_system", &vars) {
            Some(prompt) => match call_llm_review(service, &prompt).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    log::warn!("[mini_review] LLM review 失败，使用启发式回退: {}", e);
                }
            },
            None => {
                log::warn!("[mini_review] 无法解析提示词，使用启发式回退");
            }
        }
    }

    Ok(heuristic_review(content, contract))
}

fn build_prompt_vars(
    content: &str,
    contract: &RuntimeContract,
) -> std::collections::HashMap<String, String> {
    let mut vars = std::collections::HashMap::new();
    vars.insert("content".to_string(), content.to_string());
    vars.insert("genre".to_string(), contract.master_setting.genre.clone());
    vars.insert(
        "core_tone".to_string(),
        contract.master_setting.core_tone.clone(),
    );
    vars.insert(
        "pacing_strategy".to_string(),
        contract.master_setting.pacing_strategy.clone(),
    );
    vars.insert(
        "world_rules".to_string(),
        contract
            .master_setting
            .world_rules
            .iter()
            .enumerate()
            .map(|(i, r)| format!("{}. {}", i + 1, r))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    if let Some(ch) = &contract.chapter_contract {
        vars.insert(
            "chapter_goal".to_string(),
            ch.chapter_directive.goal.clone(),
        );
        vars.insert(
            "must_cover_nodes".to_string(),
            ch.chapter_directive
                .must_cover_nodes
                .iter()
                .enumerate()
                .map(|(i, n)| format!("{}. {}", i + 1, n))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        vars.insert(
            "forbidden_zones".to_string(),
            ch.chapter_directive
                .forbidden_zones
                .iter()
                .enumerate()
                .map(|(i, n)| format!("{}. {}", i + 1, n))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    } else {
        vars.insert("chapter_goal".to_string(), "（未指定）".to_string());
        vars.insert("must_cover_nodes".to_string(), "无".to_string());
        vars.insert("forbidden_zones".to_string(), "无".to_string());
    }

    vars
}

async fn call_llm_review(service: &LlmService, prompt: &str) -> Result<ReviewResult, AppError> {
    let response = service
        .generate_for_task(
            TaskType::Analysis,
            prompt.to_string(),
            Some(800),
            Some(0.3),
            Some("mini_review"),
        )
        .await?;

    parse_review_json(&response.content)
}

fn parse_review_json(text: &str) -> Result<ReviewResult, AppError> {
    // 兼容 markdown 代码块：提取第一个 { ... } 之间的 JSON
    let json_text = if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            &text[start..=end]
        } else {
            text
        }
    } else {
        text
    };

    let mut result: ReviewResult = serde_json::from_str(json_text)
        .map_err(|e| AppError::internal(format!("mini_review JSON 解析失败: {}", e)))?;

    // 校验并归一化分数
    result.score = result.score.clamp(0.0, 1.0);
    for d in &mut result.dimensions {
        d.score = d.score.clamp(0.0, 1.0);
    }

    Ok(result)
}

/// 启发式 review：基于关键词匹配合同节点、禁忌区、世界规则和内容长度。
pub fn heuristic_review(content: &str, contract: &RuntimeContract) -> ReviewResult {
    let mut issues: Vec<String> = Vec::new();
    let mut score = 0.75_f64;

    // 章节合同覆盖检查
    if let Some(ch) = &contract.chapter_contract {
        for node in &ch.chapter_directive.must_cover_nodes {
            if content.contains(node) {
                score += 0.03;
            } else {
                issues.push(format!("未覆盖必须节点: {}", node));
                score -= 0.08;
            }
        }

        for zone in &ch.chapter_directive.forbidden_zones {
            if content.contains(zone) {
                issues.push(format!("触及禁忌区: {}", zone));
                score -= 0.15;
            }
        }
    }

    // 世界规则检查（出现即认为遵守，未出现则轻微扣分）
    for rule in &contract.master_setting.world_rules {
        if !rule.is_empty() && !content.contains(rule) {
            score -= 0.02;
            issues.push(format!("可能未体现世界规则: {}", rule));
        }
    }

    // 内容长度检查
    let char_count = content.chars().count();
    if char_count < 50 {
        issues.push("内容过短，难以评估".to_string());
        score -= 0.15;
    } else if char_count > 10_000 {
        score += 0.02;
    }

    score = score.clamp(0.0, 1.0);

    let dimensions = vec![
        ReviewDimension {
            name: "合同目标达成".to_string(),
            score: goal_coverage_score(content, contract),
            comment: "基于必须节点覆盖率的启发式评估".to_string(),
        },
        ReviewDimension {
            name: "世界规则一致".to_string(),
            score: world_rule_score(content, contract),
            comment: "基于世界规则关键词出现情况的启发式评估".to_string(),
        },
        ReviewDimension {
            name: "叙事连贯性".to_string(),
            score: 0.8,
            comment: "默认连贯性评分".to_string(),
        },
        ReviewDimension {
            name: "基调一致".to_string(),
            score: 0.78,
            comment: format!(
                "与 '{}' 基调的启发式匹配",
                contract.master_setting.core_tone
            ),
        },
    ];

    ReviewResult {
        score,
        dimensions,
        summary: format!("启发式 mini review: 综合评分 {:.2}", score),
        issues,
    }
}

fn goal_coverage_score(content: &str, contract: &RuntimeContract) -> f64 {
    if let Some(ch) = &contract.chapter_contract {
        let nodes = &ch.chapter_directive.must_cover_nodes;
        if nodes.is_empty() {
            return 1.0;
        }
        let covered = nodes.iter().filter(|n| content.contains(*n)).count();
        (covered as f64 / nodes.len() as f64).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn world_rule_score(content: &str, contract: &RuntimeContract) -> f64 {
    let rules = &contract.master_setting.world_rules;
    if rules.is_empty() {
        return 1.0;
    }
    let matched = rules
        .iter()
        .filter(|r| !r.is_empty() && content.contains(*r))
        .count();
    (matched as f64 / rules.len() as f64).clamp(0.0, 1.0)
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
    fn heuristic_review_detects_missing_nodes() {
        let contract = sample_contract();
        let content = "这是一个普通的场景。";
        let result = heuristic_review(content, &contract);
        assert!(result.score < 0.75);
        assert!(result.issues.iter().any(|i| i.contains("未覆盖必须节点")));
    }

    #[test]
    fn heuristic_review_rewards_coverage() {
        let contract = sample_contract();
        let content = "主角出场后，在青云镇外发现灵气异常。他很快意识到，这正符合灵气不可再生的世界规则，心中顿时沉了下去。";
        let result = heuristic_review(content, &contract);
        assert!(result.score >= 0.75);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn parse_review_json_handles_markdown() {
        let text = r#"这里是分析结果：
```json
{
  "score": 0.85,
  "dimensions": [
    {"name": "连贯性", "score": 0.9, "comment": "很好"}
  ],
  "summary": "总体良好",
  "issues": []
}
```"#;
        let result = parse_review_json(text).unwrap();
        assert!((result.score - 0.85).abs() < f64::EPSILON);
        assert_eq!(result.dimensions.len(), 1);
    }
}
