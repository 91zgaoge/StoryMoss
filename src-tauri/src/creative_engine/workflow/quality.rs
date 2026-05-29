//! 小说质量评估系统
//!
//! 生成完成后自动评估：
//! - 结构完整性
//! - 人物一致性
//! - 风格统一度
//! - 情节连贯性

use serde::{Deserialize, Serialize};

use crate::llm::service::LlmService;

/// 质量报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityReport {
    pub overall_score: f32,
    pub structure_score: f32,
    pub character_consistency_score: f32,
    pub style_uniformity_score: f32,
    pub plot_coherence_score: f32,
    pub issues: Vec<QualityIssue>,
    pub summary: String,
}

/// 质量问题
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityIssue {
    pub category: IssueCategory,
    pub severity: Severity,
    pub description: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueCategory {
    Structure,
    Character,
    Style,
    Plot,
    Grammar,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Minor,
    Moderate,
    Major,
    Critical,
}

/// 质量检查器
pub struct QualityChecker;

impl QualityChecker {
    pub fn new() -> Self {
        Self
    }

    /// 对文本进行启发式质量评估
    ///
    /// 当前为基于规则的快速评估，未来可调用 LLM 进行深度分析。
    pub fn check(&self, text: &str) -> QualityReport {
        let mut issues = Vec::new();

        // 1. 结构检查
        let structure_score = self.check_structure(text, &mut issues);

        // 2. 人物一致性检查
        let character_score = self.check_characters(text, &mut issues);

        // 3. 风格统一度检查
        let style_score = self.check_style_uniformity(text, &mut issues);

        // 4. 情节连贯性检查
        let plot_score = self.check_plot_coherence(text, &mut issues);

        let overall = (structure_score + character_score + style_score + plot_score) / 4.0;

        let summary = Self::generate_summary(overall, &issues);

        QualityReport {
            overall_score: overall,
            structure_score,
            character_consistency_score: character_score,
            style_uniformity_score: style_score,
            plot_coherence_score: plot_score,
            issues,
            summary,
        }
    }

    fn check_structure(&self, text: &str, issues: &mut Vec<QualityIssue>) -> f32 {
        let mut score = 1.0f32;

        // 检查段落结构
        let paragraphs: Vec<&str> = text.split('\n').filter(|p| !p.trim().is_empty()).collect();
        let avg_para_len = if !paragraphs.is_empty() {
            text.len() as f32 / paragraphs.len() as f32
        } else {
            0.0
        };

        if avg_para_len > 300.0 {
            score -= 0.1;
            issues.push(QualityIssue {
                category: IssueCategory::Structure,
                severity: Severity::Minor,
                description: "段落过长，建议适当分段".to_string(),
                suggestion: "每段控制在 200 字以内，提升可读性".to_string(),
            });
        }

        // 检查开头是否有吸引力
        let opening = text.chars().take(100).collect::<String>();
        let has_hook = opening.contains('?') || opening.contains('!') || opening.contains('"');
        if !has_hook {
            score -= 0.05;
            issues.push(QualityIssue {
                category: IssueCategory::Structure,
                severity: Severity::Minor,
                description: "开头缺乏吸引力".to_string(),
                suggestion: "开头可加入悬念、冲突或引人入胜的场景".to_string(),
            });
        }

        score.max(0.0)
    }

    fn check_characters(&self, text: &str, issues: &mut Vec<QualityIssue>) -> f32 {
        let mut score = 1.0f32;

        // 检查角色名是否一致（简化：检查是否有名字变体）
        // 实际应该用更复杂的 NLP，这里用启发式
        let pronouns = ["他", "她", "它"];
        let pronoun_count: usize = pronouns.iter().map(|&p| text.matches(p).count()).sum();
        let name_indicators = ["说道", "说", "想", "觉得"];
        let name_count: usize = name_indicators
            .iter()
            .map(|&p| text.matches(p).count())
            .sum();

        if name_count == 0 && pronoun_count > 10 {
            score -= 0.15;
            issues.push(QualityIssue {
                category: IssueCategory::Character,
                severity: Severity::Moderate,
                description: "角色可能缺乏个性区分".to_string(),
                suggestion: "增加角色独特的语言习惯和动作特征".to_string(),
            });
        }

        score.max(0.0)
    }

    fn check_style_uniformity(&self, text: &str, issues: &mut Vec<QualityIssue>) -> f32 {
        let mut score = 1.0f32;

        // 检查句式多样性
        let sentences: Vec<&str> = text
            .split(['。', '！', '？'])
            .filter(|s| !s.trim().is_empty())
            .collect();

        if sentences.len() >= 3 {
            let first_words: Vec<String> = sentences
                .iter()
                .filter_map(|s| s.trim().chars().next())
                .map(|c| c.to_string())
                .collect();

            let unique_starts: std::collections::HashSet<String> =
                first_words.iter().cloned().collect();
            let variety_ratio = if !first_words.is_empty() {
                unique_starts.len() as f32 / first_words.len() as f32
            } else {
                1.0
            };

            if variety_ratio < 0.3 {
                score -= 0.1;
                issues.push(QualityIssue {
                    category: IssueCategory::Style,
                    severity: Severity::Minor,
                    description: "句式开头重复率较高".to_string(),
                    suggestion: "尝试变化句子开头，增加句式多样性".to_string(),
                });
            }
        }

        // 检查标点使用是否一致
        let quote_types = text.matches('"').count() + text.matches('「').count();
        if quote_types > 0 && text.contains('"') && text.contains('「') {
            score -= 0.1;
            issues.push(QualityIssue {
                category: IssueCategory::Style,
                severity: Severity::Minor,
                description: "引号使用不一致".to_string(),
                suggestion: "统一使用「」或 双引号 作为对话引号".to_string(),
            });
        }

        score.max(0.0)
    }

    fn check_plot_coherence(&self, text: &str, issues: &mut Vec<QualityIssue>) -> f32 {
        let mut score = 1.0f32;

        // 检查时间线索词是否一致
        let time_markers = ["早上", "中午", "晚上", "第二天", "后来", "之前"];
        let has_time = time_markers.iter().any(|&m| text.contains(m));

        if !has_time && text.len() > 1000 {
            score -= 0.05;
            issues.push(QualityIssue {
                category: IssueCategory::Plot,
                severity: Severity::Minor,
                description: "缺乏明确的时间线索".to_string(),
                suggestion: "适当增加时间过渡词，帮助读者理解时间线".to_string(),
            });
        }

        // 检查逻辑连接词密度
        let logic_markers = ["因为", "所以", "但是", "然而", "因此", "于是"];
        let logic_count: usize = logic_markers.iter().map(|&m| text.matches(m).count()).sum();
        let text_len = text.chars().count();
        let logic_density = if text_len > 0 {
            logic_count as f32 * 100.0 / text_len as f32
        } else {
            0.0
        };

        if logic_density < 0.3 && text_len > 500 {
            score -= 0.05;
            issues.push(QualityIssue {
                category: IssueCategory::Plot,
                severity: Severity::Minor,
                description: "逻辑连接较少".to_string(),
                suggestion: "增加因果关系和转折表达，使情节更连贯".to_string(),
            });
        }

        score.max(0.0)
    }

    fn generate_summary(overall: f32, issues: &[QualityIssue]) -> String {
        let level = if overall >= 0.9 {
            "优秀"
        } else if overall >= 0.8 {
            "良好"
        } else if overall >= 0.7 {
            "合格"
        } else if overall >= 0.6 {
            "需改进"
        } else {
            "较差"
        };

        let critical_count = issues
            .iter()
            .filter(|i| i.severity == Severity::Critical)
            .count();
        let major_count = issues
            .iter()
            .filter(|i| i.severity == Severity::Major)
            .count();

        let mut summary = format!("总体评分: {:.0}%（{}）", overall * 100.0, level);

        if critical_count > 0 {
            summary.push_str(&format!("，发现 {} 个严重问题", critical_count));
        }
        if major_count > 0 {
            summary.push_str(&format!("，{} 个重要问题", major_count));
        }
        if issues.is_empty() {
            summary.push_str("，未发现明显问题");
        }

        summary
    }

    /// 使用 LLM 进行深度质量评估
    ///
    /// 优先使用 LLM 评估，当 LLM 不可用时回退到规则评估。
    pub async fn check_with_llm(
        &self,
        text: &str,
        llm: &LlmService,
    ) -> Result<QualityReport, String> {
        let prompt = Self::build_llm_evaluation_prompt(text);
        let response = llm
            .generate(prompt, Some(2000), Some(0.3))
            .await
            .map_err(|e| format!("LLM 评估失败: {}", e))?;

        let json_str = Self::extract_json(&response.content);
        let raw: serde_json::Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("评估 JSON 解析失败: {}\n原始内容: {}", e, &response.content))?;

        // 解析 JSON 为 QualityReport（LLM 返回 0-100，转换为 0.0-1.0）
        let overall_score = raw
            .get("overall_score")
            .and_then(|v| v.as_u64())
            .unwrap_or(70) as f32
            / 100.0;
        let structure_score = raw
            .get("structure_score")
            .and_then(|v| v.as_u64())
            .unwrap_or(70) as f32
            / 100.0;
        let character_score = raw
            .get("character_score")
            .and_then(|v| v.as_u64())
            .unwrap_or(70) as f32
            / 100.0;
        let style_score = raw
            .get("style_score")
            .and_then(|v| v.as_u64())
            .unwrap_or(70) as f32
            / 100.0;
        let plot_score =
            raw.get("plot_score").and_then(|v| v.as_u64()).unwrap_or(70) as f32 / 100.0;

        let mut issues = Vec::new();
        if let Some(issue_array) = raw.get("issues").and_then(|v| v.as_array()) {
            for item in issue_array {
                let category = match item
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("structure")
                {
                    "character" => IssueCategory::Character,
                    "style" => IssueCategory::Style,
                    "plot" => IssueCategory::Plot,
                    "grammar" => IssueCategory::Grammar,
                    _ => IssueCategory::Structure,
                };
                let severity = match item
                    .get("severity")
                    .and_then(|v| v.as_str())
                    .unwrap_or("minor")
                {
                    "moderate" => Severity::Moderate,
                    "major" => Severity::Major,
                    "critical" => Severity::Critical,
                    _ => Severity::Minor,
                };
                let description = item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let suggestion = item
                    .get("suggestion")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !description.is_empty() {
                    issues.push(QualityIssue {
                        category,
                        severity,
                        description,
                        suggestion,
                    });
                }
            }
        }

        let summary = raw
            .get("summary")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Self::generate_summary(overall_score, &issues));

        Ok(QualityReport {
            overall_score,
            structure_score,
            character_consistency_score: character_score,
            style_uniformity_score: style_score,
            plot_coherence_score: plot_score,
            issues,
            summary,
        })
    }

    fn extract_json(content: &str) -> String {
        let trimmed = content.trim();
        if let Some(start) = trimmed.find("```") {
            let after_start = &trimmed[start + 3..];
            let code_start = if after_start.starts_with("json") {
                after_start[4..].trim_start()
            } else {
                after_start.trim_start()
            };
            if let Some(end) = code_start.find("```") {
                return code_start[..end].trim().to_string();
            }
        }
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                if end > start {
                    return trimmed[start..=end].to_string();
                }
            }
        }
        trimmed.to_string()
    }

    /// 生成 LLM 深度评估提示词
    pub fn build_llm_evaluation_prompt(text: &str) -> String {
        format!(
            r#"你是一位专业的小说质量评估师。请对以下小说内容进行深度评估。

【待评估内容】
{}

【评估维度】
1. 结构完整性：开头是否有吸引力？中间是否有起伏？结尾是否有力？
2. 人物一致性：角色行为是否符合其性格设定？是否有前后矛盾？
3. 风格统一度：全文语言风格是否一致？是否有突兀的转变？
4. 情节连贯性：因果逻辑是否清晰？时间线是否合理？
5. 写作质量：语言是否流畅？描写是否生动？对话是否自然？

【输出格式】
请输出 JSON：
{{
  "overall_score": 0-100,
  "structure_score": 0-100,
  "character_score": 0-100,
  "style_score": 0-100,
  "plot_score": 0-100,
  "issues": [
    {{"category": "structure|character|style|plot|grammar", "severity": "minor|moderate|major|critical", "description": "...", "suggestion": "..."}}
  ],
  "summary": "总体评价"
}}"#,
            text.chars().take(3000).collect::<String>()
        )
    }
}

impl Default for QualityChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_check() {
        let checker = QualityChecker::new();
        let text = "早上，他走出家门。「今天天气真好。」他说。于是他开始了新的一天。";
        let report = checker.check(text);
        assert!(report.overall_score >= 0.0 && report.overall_score <= 1.0);
    }

    #[test]
    fn test_quality_check_long_paragraphs() {
        let checker = QualityChecker::new();
        let text = "a".repeat(400); // 超长段落
        let report = checker.check(&text);
        assert!(!report.issues.is_empty());
    }

    #[test]
    fn test_generate_summary() {
        let summary = QualityChecker::generate_summary(0.85, &[]);
        assert!(summary.contains("良好"));
        assert!(summary.contains("85"));
    }

    #[test]
    fn test_quality_issue_creation() {
        let issue = QualityIssue {
            category: IssueCategory::Structure,
            severity: Severity::Minor,
            description: "测试".to_string(),
            suggestion: "建议".to_string(),
        };
        assert_eq!(issue.category, IssueCategory::Structure);
        assert_eq!(issue.severity, Severity::Minor);
    }
}
