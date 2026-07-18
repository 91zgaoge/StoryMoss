//! 确定性 grader 层（code/rule，ECC 四级 grader 的前两级——零 LLM 成本）。

use crate::db::DbPool;
use crate::domain::contracts::RuntimeContract;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CodeGraderReport {
    pub word_count: usize,
    pub repetition_ratio: f64,
    pub forbidden_hits: Vec<String>,
    pub score: f64,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleGraderReport {
    pub contract_score: f64,
    pub reading_power_score: f64,
    pub subagent_issues: Vec<String>,
    pub score: f64,
    pub issues: Vec<String>,
}

/// 从 draft.key（"第N章"）解析章号；中文数字不解析（生产 key 为阿拉伯数字）。
pub fn parse_chapter_number(key: &str) -> Option<i32> {
    let digits: String = key.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || !key.starts_with('第') || !key.ends_with('章') {
        return None;
    }
    digits.parse().ok()
}

/// 同步 code grader：字数 / 自重复率 / 合同禁则区（纯规则，无 DB、无 LLM）。
pub fn run_code_grader(content: &str, contract: Option<&RuntimeContract>) -> CodeGraderReport {
    let word_count = content.chars().count();
    let mut issues = Vec::new();
    // 自重复率（TextUtils::trim_self_repetition 去重后 vs 原文的裁剪比，
    // 与 orchestrator 续写路径 8% 重试闸门同一实现）
    let cleaned = crate::utils::text::TextUtils::trim_self_repetition(content);
    let repetition_ratio =
        crate::agents::trim_utils::compute_trim_ratio(word_count, cleaned.chars().count()) as f64;
    let mut score = 1.0f64;
    if repetition_ratio > 0.08 {
        // 每超 0.01 扣 0.05（ceil 取档），上限 0.4
        let penalty = ((repetition_ratio - 0.08) * 100.0).ceil() * 0.05;
        score -= penalty.min(0.4);
        issues.push(format!("自重复率 {:.1}%（阈值 8%）", repetition_ratio * 100.0));
    }
    // 字数（length_penalty 取大者）
    if word_count < 200 {
        score -= 0.5;
        issues.push(format!("字数过少（{}）", word_count));
    } else if word_count < 800 {
        score -= 0.2;
        issues.push(format!("字数偏少（{}）", word_count));
    }
    // 合同禁则区（每个命中扣 0.25）
    let forbidden_hits = match contract {
        Some(c) => {
            let result =
                crate::story_system::fulfillment_checker::evaluate_contract_fulfillment(content, c);
            let hits = result.forbidden_zones_hit;
            score -= 0.25 * hits.len() as f64;
            for h in &hits {
                issues.push(format!("禁则区命中: {}", h));
            }
            hits
        }
        None => Vec::new(),
    };
    CodeGraderReport {
        word_count,
        repetition_ratio,
        forbidden_hits,
        score: score.clamp(0.0, 1.0),
        issues,
    }
}

/// Rule grader（async：DB 读取合同/复检上下文 + 规则子代理复检；
/// Gate v2 在 async 上下文调用，故不做 block_in_place）。
pub async fn run_rule_grader(
    pool: &DbPool,
    story_id: &str,
    chapter_number: i32,
    content: &str,
    foreshadowing_hints: &[String],
) -> RuleGraderReport {
    // 合同兑现（无合同则合同分回退为追读力分）
    let contract = crate::story_system::contract_service::StorySystemEngine::new(pool.clone())
        .get_runtime_contract(story_id, chapter_number)
        .ok();
    // 追读力（纯规则特征：hook*0.4 + coolpoint*0.3 + micropayoff*0.3，无 debt 项）
    let reading_power_score = reading_power_score_of(content);
    let (contract_score, has_contract) = match &contract {
        Some(c) => (
            crate::story_system::fulfillment_checker::evaluate_contract_fulfillment(content, c)
                .score,
            true,
        ),
        None => (reading_power_score, false),
    };
    // 规则子代理复检（High+ 不扣分但全进 issues，拦截决策留给 Gate v2）
    let ctx = crate::agency::gate::build_review_context(pool, story_id, foreshadowing_hints);
    let notes = crate::agents::subagents::run_subagent_review(&ctx, content).await;
    let subagent_issues = crate::agency::gate::merge_rule_issues(&notes);
    let score = contract_score * 0.5 + reading_power_score * 0.5;
    let mut issues = Vec::new();
    if has_contract && contract_score < 0.7 {
        issues.push(format!("合同兑现偏低（{:.2}）", contract_score));
    }
    issues.extend(subagent_issues.iter().cloned());
    RuleGraderReport {
        contract_score,
        reading_power_score,
        subagent_issues,
        score: score.clamp(0.0, 1.0),
        issues,
    }
}

fn reading_power_score_of(content: &str) -> f64 {
    let features = crate::reading_power::evaluator::ContentFeatureExtractor::extract(content);
    // hook 映射沿用 reading_power/mod.rs 既有约定（evaluator 只产出 hook_type 枚举串）：
    // 过渡章 0；cliffhanger/mystery 0.9；emotional/action 0.6；其余（weak/None）0.3
    let hook = if features.is_transition {
        0.0
    } else {
        match features.hook_type.as_deref() {
            Some("cliffhanger") | Some("mystery") => 0.9,
            Some("emotional") | Some("action") => 0.6,
            _ => 0.3,
        }
    };
    // coolpoint/micropayoff 归一化：min(count,3)/3.0
    let coolpoint = features.coolpoint_patterns.len().min(3) as f64 / 3.0;
    let micropayoff = features.micropayoffs.len().min(3) as f64 / 3.0;
    (hook * 0.4 + coolpoint * 0.3 + micropayoff * 0.3).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    #[test]
    fn test_code_grader_clean_content() {
        // 300 句互不相同（散布块重复检测不会命中），~4200 字，无重复禁则
        let content: String = (1..=300)
            .map(|i| format!("第{}句，场景与情绪各不相同。", i))
            .collect();
        let report = run_code_grader(&content, None);
        assert!(report.score > 0.9, "干净内容应高分: {}", report.score);
        assert!(report.word_count >= 2000);
        assert!(report.forbidden_hits.is_empty());
    }

    #[test]
    fn test_code_grader_penalizes_repetition_and_short() {
        // 48 字（越过 trim_self_repetition 的 40 字短文本旁路）且 7/8 为重复句
        let content = "同一句开头。".repeat(8);
        let report = run_code_grader(&content, None);
        assert!(report.score < 0.5, "短且重复应低分: {}", report.score);
        assert!(report
            .issues
            .iter()
            .any(|i| i.contains("字数") || i.contains("重复")));
    }

    #[tokio::test]
    async fn test_rule_grader_without_contract() {
        let pool = create_test_pool().unwrap();
        // 无故事资产 → 无合同；追读力特征取自内容本身
        let content = "他推开那扇门，门外竟是失踪十年的师父。「你怎么会在这里？」".to_string()
            + &"情节推进。".repeat(400);
        let report = run_rule_grader(&pool, "story-x", 1, &content, &[]).await;
        assert!(report.score >= 0.0 && report.score <= 1.0);
        assert_eq!(report.contract_score, report.reading_power_score); // 无合同时回退
    }

    #[test]
    fn test_parse_chapter_number_from_key() {
        assert_eq!(parse_chapter_number("第3章"), Some(3));
        assert_eq!(parse_chapter_number("第12章"), Some(12));
        assert_eq!(parse_chapter_number("序章"), None);
        assert_eq!(parse_chapter_number("第一章"), None); // 中文数字不解析（生产 key 为阿拉伯）
    }
}
