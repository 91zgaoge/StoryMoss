//! Style DNA 系统 - 深度风格引擎
//!
//! 从"排版皮肤"升级为"创作基因"，让 AI 真正模仿风格。
//!
//! 核心组件：
//! - StyleDNA: 风格量化描述（词汇/句法/修辞/视角/情感/对话六维）
//! - StyleBlend: 多风格混合引擎（3风格三角框架）
//! - StyleDriftChecker: 防漂移自检清单（5项检查）
//! - StyleAnalyzer: 从文本样例自动解析生成 StyleDNA
//! - StyleChecker: 验证生成内容是否符合目标 StyleDNA
//! - ClassicStyles: 内置经典作家风格库

pub mod blend;
pub mod classic_styles;
pub mod classic_styles_extended;
pub mod dna;
pub mod drift_checker;
pub mod evolution;
pub mod fingerprint;
pub mod metrics;

pub use blend::StyleBlendConfig;
pub use dna::StyleDNA;
pub use drift_checker::StyleDriftChecker;
use serde::{Deserialize, Serialize};

use crate::llm::service::LlmService;

/// 风格分析器 - 从文本样例解析风格特征
///
/// 支持基于规则的快速分析和基于 LLM 的精确分析。
pub struct StyleAnalyzer;

impl StyleAnalyzer {
    /// 分析文本样例，生成 StyleDNA
    ///
    /// 基于启发式规则进行快速分析（无需 LLM）。
    /// 若需要更精确的分析，可调用 LLM 进行深度解析。
    pub fn analyze_sample(text: &str, name: &str) -> StyleDNA {
        let mut dna = StyleDNA::new(name);

        // 1. 词汇密度分析
        let char_count = text.chars().count();
        let word_count = text.split_whitespace().count();
        let density_ratio = if char_count > 0 {
            word_count as f32 / char_count as f32
        } else {
            0.0
        };
        dna.vocabulary.density = if density_ratio > 0.15 {
            "high".to_string()
        } else if density_ratio > 0.08 {
            "medium".to_string()
        } else {
            "low".to_string()
        };

        // 2. 平均句长分析
        let sentences: Vec<&str> = text
            .split(['。', '！', '？', '.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .collect();
        let avg_len = if !sentences.is_empty() {
            sentences.iter().map(|s| s.chars().count()).sum::<usize>() / sentences.len()
        } else {
            0
        };
        dna.syntax.avg_sentence_length = avg_len as u32;

        // 3. 句法复杂度分析
        dna.syntax.clause_complexity = if avg_len > 40 {
            "complex".to_string()
        } else if avg_len > 20 {
            "moderate".to_string()
        } else {
            "simple".to_string()
        };

        // 4. 对话比例分析
        let dialogue_markers = ['"', '「', '『', '\''];
        let dialogue_chars: usize = text
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .filter(|w| dialogue_markers.contains(&w[0]))
            .count();
        dna.dialogue.dialogue_ratio = if char_count > 0 {
            (dialogue_chars as f32 / char_count as f32).min(1.0)
        } else {
            0.0
        };

        // 5. 比喻密度分析（简化：搜索"像""如""似"等比喻词）
        let metaphor_markers = ["像", "如", "似", "仿佛", "好比"];
        let metaphor_count = metaphor_markers
            .iter()
            .map(|&m| text.matches(m).count())
            .sum::<usize>();
        let thousand_chars = char_count as f32 / 1000.0;
        dna.rhetoric.metaphor_density = if thousand_chars > 0.0 {
            metaphor_count as f32 / thousand_chars
        } else {
            0.0
        };

        // 6. 情感外露程度分析（情感词汇计数）
        let emotion_words = [
            "爱", "恨", "悲", "喜", "怒", "哀", "乐", "忧", "愁", "欢", "痛", "苦", "甜", "酸",
            "涩", "暖", "冷", "热", "凉", "湿",
        ];
        let emotion_count = emotion_words
            .iter()
            .map(|&w| text.matches(w).count())
            .sum::<usize>();
        let emotion_density = if char_count > 0 {
            emotion_count as f32 / char_count as f32
        } else {
            0.0
        };
        dna.emotion.emotion_word_density = emotion_density;
        dna.emotion.expressiveness = if emotion_density > 0.08 {
            "expressive".to_string()
        } else if emotion_density > 0.03 {
            "balanced".to_string()
        } else {
            "restrained".to_string()
        };

        // 7. 视角检测（第一人称代词频率）
        let first_person = text.matches("我").count() + text.matches("咱").count();
        let third_person = text.matches("他").count() + text.matches("她").count();
        dna.perspective.pov_type = if first_person > third_person * 2 {
            "first_person".to_string()
        } else if third_person > first_person * 2 {
            "close_third".to_string()
        } else {
            "omniscient".to_string()
        };

        // 8. 内心独白比例（"想""觉得""感到"等）
        let interior_markers = ["想", "觉得", "感到", "心想", "暗想"];
        let interior_count = interior_markers
            .iter()
            .map(|&m| text.matches(m).count())
            .sum::<usize>();
        dna.perspective.interior_monologue_ratio = if !sentences.is_empty() {
            (interior_count as f32 / sentences.len() as f32).min(1.0)
        } else {
            0.0
        };

        dna
    }

    /// 使用 LLM 深度分析文本样例，生成 StyleDNA
    ///
    /// 调用 LLM 进行专业文学风格分析，精度远高于规则分析。
    pub async fn analyze_with_llm(
        text: &str,
        name: &str,
        llm: &LlmService,
    ) -> Result<StyleDNA, String> {
        let prompt = Self::build_llm_analysis_prompt(text);
        let response = llm
            .generate(prompt, Some(2000), Some(0.3))
            .await
            .map_err(|e| format!("LLM 生成失败: {}", e))?;

        let json_str = Self::extract_json(&response.content);
        let mut dna: StyleDNA = serde_json::from_str(&json_str)
            .map_err(|e| format!("JSON 解析失败: {}\n原始内容: {}", e, &response.content))?;

        // 确保名称使用用户指定的名称
        dna.meta.name = name.to_string();

        Ok(dna)
    }

    /// 从 LLM 响应中提取 JSON（支持 markdown 代码块包裹）
    fn extract_json(content: &str) -> String {
        let trimmed = content.trim();
        // 尝试提取 ```json ... ``` 或 ``` ... ``` 中的内容
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
        //  fallback: 尝试找第一个 { 到最后一个 }
        if let Some(start) = trimmed.find('{') {
            if let Some(end) = trimmed.rfind('}') {
                if end > start {
                    return trimmed[start..=end].to_string();
                }
            }
        }
        trimmed.to_string()
    }

    /// 生成 LLM 用的分析提示词
    ///
    /// 当基于规则的分析不够精确时，可调用 LLM 使用此提示词生成更准确的
    /// StyleDNA。
    pub fn build_llm_analysis_prompt(text: &str) -> String {
        format!(
            r#"你是一位专业的文学风格分析师。请分析以下文本的风格特征，并以 JSON 格式输出 StyleDNA。

【待分析文本】
{}

【输出格式】
请输出以下结构的 JSON：
{{
  "meta": {{
    "name": "风格名称",
    "description": "风格描述"
  }},
  "vocabulary": {{
    "density": "low/medium/high",
    "abstraction": "concrete/balanced/abstract",
    "preferred_categories": ["词汇类别1", "类别2"],
    "signature_words": ["标志性词汇1", "词汇2"]
  }},
  "syntax": {{
    "avg_sentence_length": 平均句长数字,
    "clause_complexity": "simple/moderate/complex",
    "rhythm_pattern": "节奏描述"
  }},
  "rhetoric": {{
    "metaphor_density": 比喻密度浮点数,
    "preferred_devices": ["修辞手法1"]
  }},
  "perspective": {{
    "pov_type": "first_person/close_third/omniscient/multiple",
    "narrative_distance": "intimate/close/moderate/distant",
    "interior_monologue_ratio": 内心独白比例0到1
  }},
  "emotion": {{
    "expressiveness": "restrained/balanced/expressive/melodramatic",
    "dominant_mood": "主要情感基调"
  }},
  "dialogue": {{
    "dialogue_ratio": 对话比例0到1,
    "dialogue_length": "terse/moderate/verbose",
    "subtext_ratio": 潜台词比例0到1
  }}
}}

请确保分析准确，每个字段都有具体值。"#,
            text.chars().take(3000).collect::<String>()
        )
    }
}

/// 风格验证器 - 检查生成内容是否符合目标 StyleDNA
pub struct StyleChecker;

impl StyleChecker {
    /// 检查文本与目标 StyleDNA 的匹配度
    ///
    /// 返回匹配分数（0.0-1.0）和具体不符项列表。
    pub fn check(text: &str, target: &StyleDNA) -> StyleCheckResult {
        let mut score = 0.0;
        let mut issues = Vec::new();
        let mut checks = 0;

        // 1. 句长检查
        let sentences: Vec<&str> = text
            .split(['。', '！', '？', '.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .collect();
        if !sentences.is_empty() && target.syntax.avg_sentence_length > 0 {
            let avg_len =
                sentences.iter().map(|s| s.chars().count()).sum::<usize>() / sentences.len();
            let target_len = target.syntax.avg_sentence_length as usize;
            let diff = (avg_len as i32 - target_len as i32).abs();
            let tolerance = (target_len as f32 * 0.3) as i32; // 30% 容差

            if diff <= tolerance {
                score += 1.0;
            } else {
                let pct = if target_len > 0 {
                    diff as f32 / target_len as f32 * 100.0
                } else {
                    0.0
                };
                issues.push(format!(
                    "句长不符：实际平均 {} 字，目标 {} 字（偏差 {:.0}%）",
                    avg_len, target_len, pct
                ));
            }
            checks += 1;
        }

        // 2. 对话比例检查
        let char_count = text.chars().count();
        let dialogue_markers = ['"', '「', '『'];
        let dialogue_count = text
            .chars()
            .filter(|&c| dialogue_markers.contains(&c))
            .count()
            / 2; // 粗略估算
        let dialogue_ratio = if char_count > 0 {
            dialogue_count as f32 / char_count as f32
        } else {
            0.0
        };
        let target_ratio = target.dialogue.dialogue_ratio;
        let ratio_diff = (dialogue_ratio - target_ratio).abs();

        if ratio_diff <= 0.15 {
            score += 1.0;
        } else {
            issues.push(format!(
                "对话比例不符：实际 {:.0}%，目标 {:.0}%",
                dialogue_ratio * 100.0,
                target_ratio * 100.0
            ));
        }
        checks += 1;

        // 3. 比喻密度检查
        let metaphor_markers = ["像", "如", "似", "仿佛", "好比"];
        let metaphor_count = metaphor_markers
            .iter()
            .map(|&m| text.matches(m).count())
            .sum::<usize>();
        let thousand_chars = char_count as f32 / 1000.0;
        let actual_density = if thousand_chars > 0.0 {
            metaphor_count as f32 / thousand_chars
        } else {
            0.0
        };
        let target_density = target.rhetoric.metaphor_density;
        let density_diff = (actual_density - target_density).abs();

        if density_diff <= 0.05 || (target_density > 0.0 && density_diff / target_density <= 0.5) {
            score += 1.0;
        } else {
            issues.push(format!(
                "比喻密度不符：实际 {:.1} 个/千字，目标 {:.1} 个/千字",
                actual_density, target_density
            ));
        }
        checks += 1;

        // 4. 情感词汇密度检查
        let emotion_words = [
            "爱", "恨", "悲", "喜", "怒", "哀", "乐", "忧", "愁", "欢", "痛", "苦", "甜", "酸",
            "涩", "暖", "冷", "热", "凉", "湿",
        ];
        let emotion_count = emotion_words
            .iter()
            .map(|&w| text.matches(w).count())
            .sum::<usize>();
        let actual_emotion_density = if char_count > 0 {
            emotion_count as f32 / char_count as f32
        } else {
            0.0
        };
        let target_emotion_density = target.emotion.emotion_word_density;
        let emotion_diff = (actual_emotion_density - target_emotion_density).abs();

        if emotion_diff <= 0.03 {
            score += 1.0;
        } else {
            issues.push(format!(
                "情感密度不符：实际 {:.2}%，目标 {:.2}%",
                actual_emotion_density * 100.0,
                target_emotion_density * 100.0
            ));
        }
        checks += 1;

        let final_score = if checks > 0 {
            score / checks as f32
        } else {
            1.0
        };

        StyleCheckResult {
            score: final_score,
            passed: final_score >= 0.7,
            issues,
        }
    }

    /// 检查文本与混合风格的匹配度
    pub fn check_blend(
        text: &str,
        blend: &StyleBlendConfig,
        dnas: &[StyleDNA],
    ) -> StyleCheckResult {
        let drift_result = StyleDriftChecker::check(text, blend, dnas);
        StyleCheckResult {
            score: drift_result.overall_score,
            passed: drift_result.passed,
            issues: drift_result
                .checks
                .iter()
                .filter(|c| !c.passed)
                .map(|c| {
                    format!(
                        "{}: {} (实际 {:.2}, 目标 {:.2}-{:.2})",
                        c.dimension, c.suggestion, c.actual_value, c.target_min, c.target_max
                    )
                })
                .collect(),
        }
    }

    /// 生成 LLM 用的风格检查提示词
    ///
    /// 当规则检查不够精确时，可调用 LLM 使用此提示词进行深度验证。
    pub fn build_llm_check_prompt(text: &str, target: &StyleDNA) -> String {
        format!(
            r#"你是一位专业的文学风格验证师。请检查以下文本是否符合目标风格DNA的要求。

【目标风格】
{}

【待检查文本】
{}

【检查要求】
1. 逐维度对比文本实际特征与目标风格DNA的差异
2. 对每个维度给出匹配度评分（0-100）
3. 列出具体的不符项和改进建议
4. 给出总体匹配度评分（0-100）

请输出 JSON 格式：
{{
  "overall_score": 总体分数,
  "passed": true/false（是否达到70分以上）,
  "dimensions": [
    {{"name": "词汇", "score": 分数, "issues": ["问题1"]}},
    {{"name": "句法", "score": 分数, "issues": []}},
    ...
  ],
  "suggestions": ["改进建议1", "建议2"]
}}"#,
            target.to_prompt_extension(),
            text.chars().take(2000).collect::<String>()
        )
    }
}

/// 风格检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleCheckResult {
    pub score: f32,
    pub passed: bool,
    pub issues: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{classic_styles::jin_yong, *};

    #[test]
    fn test_analyze_sample() {
        let sample = "江湖风云变幻。他拔剑出鞘，剑光如虹，一招天外飞仙直取对手咽喉。那人身形一闪，\
                      掌风呼啸而至。两人你来我往，斗了三百回合不分胜负。";
        let dna = StyleAnalyzer::analyze_sample(sample, "测试风格");
        assert_eq!(dna.meta.name, "测试风格");
        assert!(dna.syntax.avg_sentence_length > 0);
        assert!(dna.rhetoric.metaphor_density >= 0.0);
    }

    #[test]
    fn test_style_checker() {
        let target = jin_yong();
        let text = "江湖风云变幻。他拔剑出鞘，剑光如虹。那人身形一闪，掌风呼啸而至。";
        let result = StyleChecker::check(text, &target);
        assert!(result.score >= 0.0 && result.score <= 1.0);
    }

    #[test]
    fn test_style_checker_with_high_dialogue_ratio() {
        let mut target = StyleDNA::new("测试");
        target.dialogue.dialogue_ratio = 0.5;
        let text = "「你好。」他说。「再见。」她答。";
        let result = StyleChecker::check(text, &target);
        assert!(result.score >= 0.0);
    }
}
