//! 子代理协作模型
//!
//! 在主 Writer 生成后异步运行三类轻量规则审查：
//! - ContinuityAgent：跨章节/场景一致性、伏笔线索、角色状态
//! - StyleAgent：句长、对话比例、比喻密度与风格 DNA 偏差
//! - WorldAgent：世界观规则、设定冲突

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::domain::agent_context::AgentContext;

/// 审查严重度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl ReviewSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReviewSeverity::Low => "low",
            ReviewSeverity::Medium => "medium",
            ReviewSeverity::High => "high",
            ReviewSeverity::Critical => "critical",
        }
    }

    pub fn score(&self) -> u8 {
        match self {
            ReviewSeverity::Low => 1,
            ReviewSeverity::Medium => 2,
            ReviewSeverity::High => 3,
            ReviewSeverity::Critical => 4,
        }
    }
}

impl std::cmp::Ord for ReviewSeverity {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.score().cmp(&other.score())
    }
}

impl std::cmp::PartialOrd for ReviewSeverity {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Default for ReviewSeverity {
    fn default() -> Self {
        ReviewSeverity::Low
    }
}

/// 单条审查问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    pub severity: ReviewSeverity,
    pub category: String,
    pub description: String,
    pub suggestion: String,
}

impl ReviewIssue {
    pub fn new(
        severity: ReviewSeverity,
        category: impl Into<String>,
        description: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            category: category.into(),
            description: description.into(),
            suggestion: suggestion.into(),
        }
    }
}

/// 子代理审查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewNotes {
    pub agent: String,
    pub summary: String,
    pub issues: Vec<ReviewIssue>,
    pub severity: ReviewSeverity,
}

impl ReviewNotes {
    pub fn new(agent: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            agent: agent.into(),
            summary: summary.into(),
            issues: Vec::new(),
            severity: ReviewSeverity::Low,
        }
    }

    pub fn add_issue(&mut self, issue: ReviewIssue) {
        self.severity = self.severity.max(issue.severity);
        self.issues.push(issue);
    }

    pub fn has_high_or_above(&self) -> bool {
        self.severity >= ReviewSeverity::High
    }
}

/// 子代理 trait
#[async_trait]
pub trait Subagent: Send + Sync {
    fn name(&self) -> &'static str;

    async fn review(&self, context: AgentContext, content: String) -> ReviewNotes;
}

// ==================== ContinuityAgent ====================

pub struct ContinuityAgent;

#[async_trait]
impl Subagent for ContinuityAgent {
    fn name(&self) -> &'static str {
        "ContinuityAgent"
    }

    async fn review(&self, context: AgentContext, content: String) -> ReviewNotes {
        let mut notes = ReviewNotes::new("ContinuityAgent", "检查跨章节一致性、伏笔回收与角色状态");
        let content_lower = content.to_lowercase();

        // 1. 活跃线索是否被提及
        if !context.narrative.active_threads.is_empty() {
            let mut missing = Vec::new();
            for thread in &context.narrative.active_threads {
                let key = thread.to_lowercase();
                if !content_lower.contains(&key) {
                    missing.push(thread.clone());
                }
            }
            if !missing.is_empty() {
                notes.add_issue(ReviewIssue::new(
                    ReviewSeverity::Medium,
                    "continuity",
                    format!("本段未回收以下活跃线索：{}", missing.join(", ")),
                    "在后续段落或当前段末尾回应这些线索。",
                ));
            }
        }

        // 2. 是否重复开头（与前面章节摘要前 20 字比对）
        for prev in &context.narrative.previous_chapters {
            let prev_start = prev
                .summary
                .chars()
                .take(20)
                .collect::<String>()
                .to_lowercase();
            if !prev_start.is_empty() && content_lower.starts_with(&prev_start) {
                notes.add_issue(ReviewIssue::new(
                    ReviewSeverity::High,
                    "repetition",
                    format!("本段开头与第{}章摘要高度重复", prev.number),
                    "调整开头，避免读者感觉内容循环。",
                ));
            }
        }

        // 3. 主要角色是否出现（仅第二章以后）
        if context.narrative.chapter_number > 1 {
            for character in &context.narrative.characters {
                if character.name.len() > 1 && !content.contains(&character.name) {
                    notes.add_issue(ReviewIssue::new(
                        ReviewSeverity::Low,
                        "character_presence",
                        format!("角色 '{}' 在本段未出现", character.name),
                        "如该角色应参与当前情节，请补充其动作或反应。",
                    ));
                }
            }
        }

        notes
    }
}

// ==================== StyleAgent ====================

pub struct StyleAgent;

#[async_trait]
impl Subagent for StyleAgent {
    fn name(&self) -> &'static str {
        "StyleAgent"
    }

    async fn review(&self, context: AgentContext, content: String) -> ReviewNotes {
        let mut notes =
            ReviewNotes::new("StyleAgent", "检查句长、对话比例、比喻密度与风格 DNA 偏差");

        let sentences: Vec<&str> = content
            .split(|c| c == '。' || c == '？' || c == '！')
            .filter(|s| !s.trim().is_empty())
            .collect();
        let total_chars = content.chars().count();
        if !sentences.is_empty() && total_chars > 0 {
            let avg_sentence_len = total_chars / sentences.len();

            // 短句/长句风格校验
            if let Some(ref structure) = context.style.writing_style_sentence_structure {
                let structure_lower = structure.to_lowercase();
                if structure_lower.contains("短") && avg_sentence_len > 45 {
                    notes.add_issue(ReviewIssue::new(
                        ReviewSeverity::Medium,
                        "sentence_length",
                        format!("设定为短句为主，但平均句长 {} 字", avg_sentence_len),
                        "拆分为更短的句子，增强节奏感。",
                    ));
                } else if structure_lower.contains("长") && avg_sentence_len < 20 {
                    notes.add_issue(ReviewIssue::new(
                        ReviewSeverity::Low,
                        "sentence_length",
                        format!("设定为长句为主，但平均句长 {} 字", avg_sentence_len),
                        "适当延长句子，增加层次与韵味。",
                    ));
                }
            }

            // 对话比例
            let dialogue_chars = content
                .chars()
                .filter(|c| matches!(c, '"' | '“' | '”' | '‘' | '’' | '「' | '」' | '『' | '』'))
                .count();
            let dialogue_ratio = dialogue_chars as f32 / total_chars as f32;
            if dialogue_ratio > 0.6 {
                notes.add_issue(ReviewIssue::new(
                    ReviewSeverity::Medium,
                    "dialogue_ratio",
                    format!("对话占比 {:.0}% 过高", dialogue_ratio * 100.0),
                    "增加叙述、动作与内心描写，平衡对白密度。",
                ));
            }

            // 比喻密度
            let metaphor_markers = ["像", "如同", "仿佛", "好似", "宛如", "犹如", "似"];
            let metaphor_count = metaphor_markers
                .iter()
                .map(|m| content.match_indices(m).count())
                .sum::<usize>();
            let metaphor_density = (metaphor_count as f32 * 1000.0) / total_chars as f32;
            if metaphor_density > 8.0 {
                notes.add_issue(ReviewIssue::new(
                    ReviewSeverity::Low,
                    "metaphor_density",
                    format!("比喻密度 {:.1} / 千字", metaphor_density),
                    "减少连续比喻，避免修辞疲劳。",
                ));
            }
        }

        // AI 陈词滥调（与 Anti-AI 模块重叠，作为兜底）
        let cliches = [
            "嘴角微微上扬",
            "关键",
            "值得注意的是",
            "综上所述",
            "让我们",
            "在某种程度上",
            "与此同时",
            "这一切的背后",
        ];
        for cliche in &cliches {
            if content.contains(cliche) {
                notes.add_issue(ReviewIssue::new(
                    ReviewSeverity::Medium,
                    "ai_cliche",
                    format!("出现 AI 陈词：{}", cliche),
                    "替换为更具体的动作或描写。",
                ));
            }
        }

        notes
    }
}

// ==================== WorldAgent ====================

pub struct WorldAgent;

#[async_trait]
impl Subagent for WorldAgent {
    fn name(&self) -> &'static str {
        "WorldAgent"
    }

    async fn review(&self, context: AgentContext, content: String) -> ReviewNotes {
        let mut notes = ReviewNotes::new("WorldAgent", "检查世界观规则、设定冲突与地理/时间一致性");

        if let Some(ref rules_text) = context.world.world_rules {
            if !rules_text.trim().is_empty() {
                // 提取规则中的中文关键词（长度 ≥ 2）
                let keywords: Vec<String> = rules_text
                    .split(|c: char| c.is_ascii_punctuation() || c == '，' || c == '。')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && s.chars().count() >= 2)
                    .take(8)
                    .map(|s| s.to_string())
                    .collect();

                let mut matched_any = false;
                for keyword in &keywords {
                    if content.contains(keyword) {
                        matched_any = true;
                        break;
                    }
                }
                if !matched_any && !keywords.is_empty() {
                    notes.add_issue(ReviewIssue::new(
                        ReviewSeverity::Low,
                        "world_presence",
                        "本段未体现核心世界观规则关键词".to_string(),
                        format!("在叙事中自然融入至少一项规则：{}", keywords.join(", ")),
                    ));
                }
            }
        }

        // 运行时合同中的 world_rules
        if let Some(ref contract) = context.runtime_contract {
            let rules: Vec<String> = contract
                .master_setting
                .world_rules
                .iter()
                .map(|r| r.to_lowercase())
                .collect();
            let content_lower = content.to_lowercase();
            for rule in &rules {
                if rule.len() >= 2 && !content_lower.contains(rule) {
                    notes.add_issue(ReviewIssue::new(
                        ReviewSeverity::Low,
                        "world_contract",
                        format!("未体现合同规则：{}", rule),
                        "检查是否违反该规则或补充相关描写。",
                    ));
                }
            }
        }

        notes
    }
}

/// 并发执行三个子代理审查
pub async fn run_subagent_review(context: &AgentContext, content: &str) -> Vec<ReviewNotes> {
    let ctx1 = context.clone();
    let ctx2 = context.clone();
    let ctx3 = context.clone();
    let c1 = content.to_string();
    let c2 = content.to_string();
    let c3 = content.to_string();
    let (continuity, style, world) = tokio::join!(
        ContinuityAgent.review(ctx1, c1),
        StyleAgent.review(ctx2, c2),
        WorldAgent.review(ctx3, c3),
    );
    vec![continuity, style, world]
}

/// 将 ReviewNotes 渲染为 Markdown 段落
pub fn render_notes_to_markdown(notes: &[ReviewNotes]) -> String {
    let mut md = String::new();
    md.push_str("# 当前任务循环\n\n");
    md.push_str("> 由 StoryMoss 子代理在生成后自动审查。\n\n");

    for n in notes {
        md.push_str(&format!("## {}（{}）\n\n", n.agent, n.severity.as_str()));
        md.push_str(&format!("{}\n\n", n.summary));
        if n.issues.is_empty() {
            md.push_str("- 未发现明显问题。\n\n");
        } else {
            for issue in &n.issues {
                md.push_str(&format!(
                    "- **[{}]** {}：{}\n  - 建议：{}\n",
                    issue.severity.as_str(),
                    issue.category,
                    issue.description,
                    issue.suggestion
                ));
            }
            md.push('\n');
        }
    }

    md
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> AgentContext {
        AgentContext {
            story: crate::domain::agent_context::StoryContext {
                story_id: "s1".to_string(),
                story_title: "测试".to_string(),
                genre: "玄幻".to_string(),
                tone: "dark".to_string(),
                pacing: "medium".to_string(),
                ..Default::default()
            },
            narrative: crate::domain::agent_context::NarrativeContext {
                chapter_number: 2,
                active_threads: vec!["失落的剑".to_string()],
                characters: vec![crate::domain::agent_context::CharacterInfo {
                    name: "李青".to_string(),
                    personality: "沉稳".to_string(),
                    role: "主角".to_string(),
                    appearance: None,
                    gender: None,
                    age: None,
                }],
                previous_chapters: vec![crate::domain::agent_context::ChapterSummary {
                    title: "第一章".to_string(),
                    number: 1,
                    summary: "李青在山中醒来".to_string(),
                }],
                ..Default::default()
            },
            style: crate::domain::agent_context::StyleContext {
                writing_style_sentence_structure: Some("短句为主".to_string()),
                ..Default::default()
            },
            world: crate::domain::agent_context::WorldContext {
                world_rules: Some("灵气是这个世界唯一的能量来源。".to_string()),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_continuity_agent_detects_missing_thread() {
        let content = "李青走在路上，没有任何线索。".to_string();
        let notes = ContinuityAgent.review(sample_context(), content).await;
        assert!(notes.issues.iter().any(|i| i.category == "continuity"));
    }

    #[tokio::test]
    async fn test_style_agent_detects_long_sentences() {
        let content = "这是一个非常非常非常非常长的句子，里面有无数修饰和铺垫，只为了说明一件事情，并且完全不符合短句为主的设定。".to_string();
        let notes = StyleAgent.review(sample_context(), content).await;
        assert!(notes.issues.iter().any(|i| i.category == "sentence_length"));
    }

    #[tokio::test]
    async fn test_world_agent_detects_missing_rule() {
        let content = "李青走在路上。".to_string();
        let notes = WorldAgent.review(sample_context(), content).await;
        assert!(notes.issues.iter().any(|i| i.category == "world_presence"));
    }

    #[test]
    fn test_render_notes_to_markdown() {
        let mut n = ReviewNotes::new("TestAgent", "测试摘要");
        n.add_issue(ReviewIssue::new(
            ReviewSeverity::High,
            "test",
            "描述",
            "建议",
        ));
        let md = render_notes_to_markdown(&[n]);
        assert!(md.contains("TestAgent"));
        assert!(md.contains("high"));
        assert!(md.contains("建议"));
    }
}
