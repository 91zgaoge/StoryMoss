//! 生成前约束门（Pre-Generation Gate）
//!
//! 在调用 Writer 之前做轻量规则检查，避免生成后才发现不可用：
//! 1. 前文内容是否足够，避免重复开头；
//! 2. 是否存在活跃的未闭合线索（伏笔）需要回收；
//! 3. 风格 DNA 是否完整。
//!
//! 所有检查均为计算规则，不引入 LLM 调用，符合“计算验证优先”原则。

use serde::{Deserialize, Serialize};

use crate::{domain::agent_context::AgentContext, error::ErrorSeverity};

/// 生成前约束门检查结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateResult {
    /// 是否允许继续生成
    pub can_proceed: bool,
    /// 错误严重级别（can_proceed=false 时有效）
    pub severity: Option<ErrorSeverity>,
    /// 警告信息（可继续，但需追加到提示词）
    pub warnings: Vec<String>,
    /// 需要追加到生成提示词的硬性约束
    pub constraints: Vec<String>,
}

impl GateResult {
    pub fn ok() -> Self {
        Self {
            can_proceed: true,
            ..Default::default()
        }
    }

    pub fn blocked(reason: impl Into<String>, severity: ErrorSeverity) -> Self {
        Self {
            can_proceed: false,
            severity: Some(severity),
            warnings: vec![reason.into()],
            constraints: vec![],
        }
    }

    pub fn with_constraint(mut self, constraint: impl Into<String>) -> Self {
        self.constraints.push(constraint.into());
        self
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// 将约束渲染为可追加到提示词的文本
    pub fn render_constraints(&self) -> String {
        if self.constraints.is_empty() {
            return String::new();
        }
        format!(
            "\n\n【生成前约束】\n{}\n",
            self.constraints
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{}. {}", i + 1, c))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// 在调用 Writer 前执行生成前约束门。
///
/// 检查基于已有的 `AgentContext`，不访问数据库，不引入 LLM。
pub fn check(ctx: &AgentContext) -> GateResult {
    let mut result = GateResult::ok();

    // 1. 前文内容长度检查：避免重复开头
    result = check_prior_content(ctx, result);

    // 2. 活跃线索/伏笔检查：提示优先回收
    result = check_active_threads(ctx, result);

    // 3. 风格 DNA 完整性检查
    result = check_style_dna(ctx, result);

    result
}

fn check_prior_content(ctx: &AgentContext, mut result: GateResult) -> GateResult {
    let current_len = ctx
        .narrative
        .current_content
        .as_ref()
        .map(|s| s.trim().chars().count())
        .unwrap_or(0);
    let previous_len: usize = ctx
        .narrative
        .previous_chapters
        .iter()
        .map(|c| c.summary.chars().count())
        .sum();
    let total = current_len + previous_len;

    // 仅在第 2 章及以后检查，且总前文内容不足 200 字时提示
    if ctx.narrative.chapter_number > 1 && total < 200 {
        result = result.with_warning(format!(
            "前文内容较短（约 {} 字），请避免在续写中重复开头或重新介绍背景。",
            total
        ));
    }
    result
}

fn check_active_threads(ctx: &AgentContext, mut result: GateResult) -> GateResult {
    if !ctx.narrative.active_threads.is_empty() {
        let threads = ctx.narrative.active_threads.join("、");
        result = result.with_constraint(format!(
            "当前存在以下活跃线索/伏笔，请在本章中优先推进或回收：{}",
            threads
        ));
    }
    result
}

fn check_style_dna(ctx: &AgentContext, mut result: GateResult) -> GateResult {
    let has_style = ctx.style.style_dna_id.is_some()
        || ctx.style.style_blend.is_some()
        || ctx.style.style_dna_extension.is_some()
        || ctx.style.style_fingerprint.is_some()
        || ctx.style.writing_style_name.is_some();

    if !has_style {
        result = result
            .with_warning("风格 DNA 尚未配置，本次生成将使用默认风格。建议前往后台完善写作风格。");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_with_chapter_and_content(
        chapter_number: u32,
        current_content: Option<&str>,
    ) -> AgentContext {
        let mut ctx = AgentContext::minimal("story-1".to_string(), "继续写".to_string());
        ctx.narrative.chapter_number = chapter_number;
        ctx.narrative.current_content = current_content.map(|s| s.to_string());
        ctx
    }

    #[test]
    fn test_short_prior_content_warns() {
        let ctx = ctx_with_chapter_and_content(2, Some("很短。"));
        let result = check(&ctx);
        assert!(result.can_proceed);
        assert!(
            result.warnings.iter().any(|w| w.contains("前文内容较短")),
            "{:?}",
            result.warnings
        );
    }

    #[test]
    fn test_long_prior_content_no_warning() {
        let mut ctx = ctx_with_chapter_and_content(2, Some(&"a".repeat(500)));
        ctx.style.style_dna_id = Some("style-1".to_string());
        let result = check(&ctx);
        assert!(result.can_proceed);
        assert!(
            !result.warnings.iter().any(|w| w.contains("前文内容较短")),
            "{:?}",
            result.warnings
        );
    }

    #[test]
    fn test_active_threads_constraint() {
        let mut ctx = ctx_with_chapter_and_content(1, None);
        ctx.narrative.active_threads = vec!["神秘符号".to_string(), "失踪的导师".to_string()];
        let result = check(&ctx);
        assert!(result.can_proceed);
        assert_eq!(result.constraints.len(), 1);
        assert!(result.constraints[0].contains("神秘符号"));
        assert!(result.constraints[0].contains("失踪的导师"));
    }

    #[test]
    fn test_missing_style_dna_warns() {
        let ctx = ctx_with_chapter_and_content(1, None);
        let result = check(&ctx);
        assert!(result.can_proceed);
        assert!(result.warnings.iter().any(|w| w.contains("风格 DNA")));
    }

    #[test]
    fn test_render_constraints() {
        let result = GateResult::ok()
            .with_constraint("不要重复开头")
            .with_constraint("回收伏笔");
        let rendered = result.render_constraints();
        assert!(rendered.contains("不要重复开头"));
        assert!(rendered.contains("回收伏笔"));
    }
}
