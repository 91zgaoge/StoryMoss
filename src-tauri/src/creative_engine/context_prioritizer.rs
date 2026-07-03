//! Context Prioritizer - 对抗 "Lost in the Middle" 的上下文排序与双重锚定
//!
//! 参考 Akshay Pachaar《The Anatomy of an Agent Harness》中关于 Context
//! Management 的原则： "Context is a scarce resource. The goal is to find the
//! smallest possible set of high-signal tokens that maximize likelihood of the
//! desired outcome."
//!
//! 本模块将系统提示词拆分为带优先级的 ContextChunk，按 Critical / High / Normal
//! / Background 排序，并把 Critical
//! 信息同时前置和后置（轻量摘要），让模型不会遗忘中间内容。

use serde::{Deserialize, Serialize};

use crate::memory::tokenizer::count_tokens;

/// 上下文块优先级
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContextPriority {
    /// 必须始终可见：世界观红线、核心角色目标、未回收伏笔、反 AI 规则等
    Critical,
    /// 强烈影响输出质量：风格 DNA、体裁画像、运行时合同
    High,
    /// 有用但可压缩：近章摘要、场景结构、叙事事件历史
    Normal,
    /// 锦上添花：辅助关系、参考 few-shots、个性化扩展
    Background,
}

impl ContextPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContextPriority::Critical => "critical",
            ContextPriority::High => "high",
            ContextPriority::Normal => "normal",
            ContextPriority::Background => "background",
        }
    }
}

/// 一个带优先级的上下文块
#[derive(Debug, Clone)]
pub struct ContextChunk {
    pub text: String,
    pub priority: ContextPriority,
    /// 来源标识，用于调试和 metrics
    pub source: &'static str,
}

impl ContextChunk {
    pub fn new(text: impl Into<String>, priority: ContextPriority, source: &'static str) -> Self {
        Self {
            text: text.into(),
            priority,
            source,
        }
    }
}

/// 上下文健康度指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextHealthMetrics {
    pub total_tokens: usize,
    pub critical_tokens: usize,
    pub high_tokens: usize,
    pub normal_tokens: usize,
    pub background_tokens: usize,
    pub final_chunk_count: usize,
    /// Critical 信息是否因预算被截断
    pub was_critical_truncated: bool,
}

/// 经过优先级排序后的系统提示词
#[derive(Debug, Clone)]
pub struct PrioritizedSystemPrompt {
    pub prompt: String,
    pub metrics: ContextHealthMetrics,
}

/// 对系统提示词组件进行优先级排序并双重锚定关键信息。
///
/// 算法：
/// 1. base_prompt 放在最开头（通常是 writer_system 模板）。
/// 2. 所有 chunk 按 Critical > High > Normal > Background 排序。
/// 3. Critical chunk
///    同时生成一个后置的轻量提醒，确保关键约束在上下文末尾再次出现。
/// 4. 计算每个优先级 token 占比和总 token 数。
///
/// 注意：当前不主动截断，因为 `ContextBudget::apply_context_budget` 已在
/// AgentContext 字段层面完成截断。后续可在此函数中增加最终兜底截断。
pub fn prioritize_system_prompt(
    base_prompt: String,
    chunks: Vec<ContextChunk>,
    model_family: &str,
) -> PrioritizedSystemPrompt {
    let mut sorted = chunks;
    sorted.sort_by(|a, b| a.priority.cmp(&b.priority));

    let mut front_parts: Vec<String> = vec![base_prompt.clone()];
    let mut back_reminders: Vec<String> = Vec::new();
    let mut metrics = ContextHealthMetrics::default();
    metrics.total_tokens = count_tokens(&base_prompt, model_family);

    for chunk in sorted {
        let tokens = count_tokens(&chunk.text, model_family);
        match chunk.priority {
            ContextPriority::Critical => {
                front_parts.push(chunk.text.clone());
                metrics.critical_tokens += tokens;
                // 后置只保留关键提醒，避免完整重复占用 token
                let reminder = extract_first_meaningful_line(&chunk.text, 120);
                if !reminder.is_empty() {
                    back_reminders.push(format!("[{}] {}", chunk.source, reminder));
                }
            }
            ContextPriority::High => {
                front_parts.push(chunk.text.clone());
                metrics.high_tokens += tokens;
            }
            ContextPriority::Normal => {
                front_parts.push(chunk.text.clone());
                metrics.normal_tokens += tokens;
            }
            ContextPriority::Background => {
                front_parts.push(chunk.text.clone());
                metrics.background_tokens += tokens;
            }
        }
    }

    let mut prompt = front_parts.join("\n\n");
    if !back_reminders.is_empty() {
        prompt.push_str("\n\n【关键约束提醒】\n以下内容必须在本次生成中始终遵守：\n- ");
        prompt.push_str(&back_reminders.join("\n- "));
        prompt.push('\n');
    }

    metrics.total_tokens = count_tokens(&prompt, model_family);
    metrics.final_chunk_count = front_parts.len();
    metrics.was_critical_truncated = false;

    PrioritizedSystemPrompt { prompt, metrics }
}

/// 从文本中提取第一行有意义的内容，作为后置提醒。
fn extract_first_meaningful_line(text: &str, max_chars: usize) -> String {
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('【') && trimmed.ends_with('】') {
            continue;
        }
        let normalized = trimmed
            .trim_start_matches("-")
            .trim_start_matches("*")
            .trim()
            .to_string();
        if normalized.is_empty() {
            continue;
        }
        if normalized.chars().count() > max_chars {
            return normalized.chars().take(max_chars).collect::<String>() + "...";
        }
        return normalized;
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prioritize_system_prompt_orders_by_priority() {
        let base = "Base system prompt.".to_string();
        let chunks = vec![
            ContextChunk::new(
                "Background info.".to_string(),
                ContextPriority::Background,
                "bg",
            ),
            ContextChunk::new(
                "Critical rule.".to_string(),
                ContextPriority::Critical,
                "critical",
            ),
            ContextChunk::new(
                "Normal context.".to_string(),
                ContextPriority::Normal,
                "normal",
            ),
            ContextChunk::new("High priority.".to_string(), ContextPriority::High, "high"),
        ];
        let result = prioritize_system_prompt(base, chunks, "cl100k");
        // Critical 应该在最前，Background 在最后
        assert!(
            result.prompt.find("Critical rule").unwrap()
                < result.prompt.find("High priority").unwrap()
        );
        assert!(
            result.prompt.find("High priority").unwrap()
                < result.prompt.find("Normal context").unwrap()
        );
        assert!(
            result.prompt.find("Normal context").unwrap()
                < result.prompt.find("Background info").unwrap()
        );
    }

    #[test]
    fn test_critical_dual_anchored() {
        let base = "Base.".to_string();
        let chunks = vec![ContextChunk::new(
            "Critical: never use AI clichés.".to_string(),
            ContextPriority::Critical,
            "anti_ai",
        )];
        let result = prioritize_system_prompt(base, chunks, "cl100k");
        // Critical 完整内容前置
        assert!(result.prompt.contains("Critical: never use AI clichés"));
        // 后置提醒也包含
        assert!(result.prompt.contains("【关键约束提醒】"));
        assert!(result.prompt.contains("[anti_ai]"));
    }

    #[test]
    fn test_metrics_tracks_tokens_by_priority() {
        let base = "Base prompt.".to_string();
        let chunks = vec![
            ContextChunk::new("Critical.".to_string(), ContextPriority::Critical, "c"),
            ContextChunk::new("High.".to_string(), ContextPriority::High, "h"),
            ContextChunk::new("Normal.".to_string(), ContextPriority::Normal, "n"),
        ];
        let result = prioritize_system_prompt(base, chunks, "cl100k");
        assert!(result.metrics.critical_tokens > 0);
        assert!(result.metrics.high_tokens > 0);
        assert!(result.metrics.normal_tokens > 0);
        assert!(result.metrics.total_tokens > 0);
        assert_eq!(result.metrics.final_chunk_count, 4);
    }

    #[test]
    fn test_extract_first_meaningful_line_skips_headers() {
        let text = "【世界观】\nNever violate gravity.\nMore details.";
        assert_eq!(
            extract_first_meaningful_line(text, 120),
            "Never violate gravity."
        );
    }

    #[test]
    fn test_extract_first_meaningful_line_trims_long() {
        let text = "A very long meaningful line that should be truncated.";
        let max = 10;
        let out = extract_first_meaningful_line(text, max);
        assert!(out.ends_with("..."));
        assert_eq!(out.chars().count(), max + 3);
    }

    #[test]
    fn test_no_chunks_only_base() {
        let result = prioritize_system_prompt("Only base.".to_string(), vec![], "cl100k");
        assert_eq!(result.prompt, "Only base.");
        assert_eq!(result.metrics.final_chunk_count, 1);
    }

    #[test]
    fn test_multiple_critical_back_reminders() {
        let base = "Base.".to_string();
        let chunks = vec![
            ContextChunk::new("Critical A.".to_string(), ContextPriority::Critical, "a"),
            ContextChunk::new("Critical B.".to_string(), ContextPriority::Critical, "b"),
        ];
        let result = prioritize_system_prompt(base, chunks, "cl100k");
        let reminders_count = result.prompt.matches("[").count();
        // 每个 Critical 都会生成一个后置提醒，格式为 [source] ...
        assert!(reminders_count >= 2);
    }
}
