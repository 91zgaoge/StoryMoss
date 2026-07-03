//! 生成中自检（In-Generation Self-Check）
//!
//! 在 TriShot Call 3 / Writer 输出最终内容后、返回前端前，按规则检查：
//! - AI 陈词滥调（复用 anti_ai::AntiAiReviewer）
//! - 在世作者名（复用 creative_engine::style::LivingAuthorGuard）
//! - 世界观一致性（基于 world_rules 关键词的简单匹配）
//!
//! 若检测到问题，立即触发 MiniRewrite（本地轻量改写），并记录到诊断存储。
//! 本模块不引入额外线程 LLM 调用，符合"计算验证优先"原则。

use serde::{Deserialize, Serialize};

use crate::{
    anti_ai::{
        rewriter::{AntiAiRewriter, RewriteRequest, RewriteStrategy},
        AntiAiReview, AntiAiReviewer,
    },
    creative_engine::style::living_author_guard::sanitize_style_brief,
    domain::agent_context::AgentContext,
};

/// 生成中自检结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InGenerationCheckResult {
    /// 是否发现需要立即处理的问题
    pub triggered: bool,
    /// 问题概述
    pub issues: Vec<String>,
    /// MiniRewrite 后的内容（若未触发则与原文相同）
    pub rewritten_content: String,
    /// 是否实际改写了内容
    pub mutated: bool,
}

/// 对 Writer 输出内容进行生成中自检与 MiniRewrite。
///
/// 当前实现为轻量规则改写：仅剥离 AI 陈词滥调中的高频短语，
/// 后续可升级为基于 LLM 的段落级改写。
pub async fn check_and_rewrite(ctx: &AgentContext, content: &str) -> InGenerationCheckResult {
    let mut issues = Vec::new();
    let mut mutated = false;
    let mut rewritten = content.to_string();

    // 1. Anti-AI 陈词滥调检查
    let reviewer = AntiAiReviewer::new();
    let review = reviewer.review(content, Some(&ctx.story.genre));
    if !review.issues.is_empty() {
        issues.extend(
            review
                .issues
                .iter()
                .map(|i| format!("[Anti-AI] {}", i.description)),
        );
    }

    // 2. 在世作者保护检查
    let guard_outcome = sanitize_style_brief(content);
    if !guard_outcome.removed_authors.is_empty() {
        issues.push(format!(
            "[在世作者] 检测到在世作者名：{}，已替换为风格描述",
            guard_outcome.removed_authors.join(", ")
        ));
    }
    if guard_outcome.sanitized != content {
        rewritten = guard_outcome.sanitized;
        mutated = true;
    }

    // 3. 世界观一致性检查（简单关键词匹配）
    if let Some(ref rules) = ctx.world.world_rules {
        if !rules.trim().is_empty() {
            let violations = check_world_rule_violations(rules, content);
            if !violations.is_empty() {
                issues.extend(
                    violations
                        .into_iter()
                        .map(|v| format!("[世界观] 可能违反规则：{}", v)),
                );
            }
        }
    }

    // 4. 若触发则调用 MiniRewrite
    let triggered = !issues.is_empty();
    if triggered {
        let rewriter = AntiAiRewriter::new();
        let req = RewriteRequest {
            original_content: rewritten.clone(),
            review,
            strategy: RewriteStrategy::LocalReplace,
            budget_chars: 0,
        };
        match rewriter.rewrite(req).await {
            Ok(outcome) => {
                if outcome.mutated {
                    rewritten = outcome.rewritten_content;
                    mutated = true;
                }
            }
            Err(e) => {
                log::warn!("[InGenerationChecker] MiniRewrite 失败: {}", e);
            }
        }
    }

    InGenerationCheckResult {
        triggered,
        issues,
        rewritten_content: rewritten,
        mutated,
    }
}

/// 简单世界观规则检查：扫描正文中是否出现规则中明确禁止的关键词。
///
/// 规则文本格式约定：每行一条规则，以"禁止"、"不可"、"不能"、"不得"开头。
fn check_world_rule_violations(rules: &str, content: &str) -> Vec<String> {
    let content_lower = content.to_lowercase();
    let mut violations = Vec::new();
    for line in rules.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();

        // 尝试剥离前缀
        let keyword_part = if let Some(rest) = lower.strip_prefix("禁止") {
            rest
        } else if let Some(rest) = lower.strip_prefix("不可") {
            rest
        } else if let Some(rest) = lower.strip_prefix("不能") {
            rest
        } else if let Some(rest) = lower.strip_prefix("不得") {
            rest
        } else {
            ""
        };

        if keyword_part.is_empty() {
            continue;
        }

        // 去除前导标点并取第一个语义段（冒号/逗号前）
        let keyword = keyword_part
            .split(|c: char| c == '，' || c == '：' || c == ':' || c == ' ')
            .next()
            .unwrap_or("")
            .trim();
        if !keyword.is_empty() && keyword.chars().count() >= 2 && content_lower.contains(keyword) {
            violations.push(trimmed.to_string());
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent_context::AgentContext;

    fn ctx_with_rules(rules: &str) -> AgentContext {
        let mut ctx = AgentContext::minimal("story-1".to_string(), "继续写".to_string());
        ctx.world.world_rules = Some(rules.to_string());
        ctx
    }

    #[tokio::test]
    async fn test_detects_ai_cliche_and_author() {
        let ctx = ctx_with_rules("");
        let content = "不言而喻，主角眼中闪过一丝决然。\n\n这是莫言的风格。";
        let result = check_and_rewrite(&ctx, content).await;
        assert!(result.triggered);
        assert!(result.issues.iter().any(|i| i.contains("Anti-AI")));
        assert!(result.issues.iter().any(|i| i.contains("在世作者")));
    }

    #[tokio::test]
    async fn test_world_rule_violation() {
        let ctx = ctx_with_rules("禁止飞行：本世界不存在飞行器");
        let content = "主角驾驶飞行器掠过城市。";
        let result = check_and_rewrite(&ctx, content).await;
        assert!(result.triggered);
        assert!(result.issues.iter().any(|i| i.contains("世界观")));
    }

    #[tokio::test]
    async fn test_clean_content_no_trigger() {
        let ctx = ctx_with_rules("禁止飞行");
        let content = "主角走在山路上，风吹过衣角。";
        let result = check_and_rewrite(&ctx, content).await;
        assert!(!result.triggered);
        assert!(result.issues.is_empty());
    }
}
