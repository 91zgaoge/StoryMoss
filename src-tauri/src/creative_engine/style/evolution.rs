//! Style Evolution Engine - StyleDNA 反馈闭环
//!
//! W3-B8: Anti-AI Review 和 Pipeline Review 的结果作为 feedback 输入，
//! 驱动 StyleDNA 演化。
//!
//! 核心逻辑：将 Review 发现的问题映射到 StyleDNA 六维度的调整建议。

use serde::{Deserialize, Serialize};

use super::dna::StyleDNA;
use crate::{
    anti_ai::{AntiAiReview, ReviewIssue},
    pipeline::types::{ReviewDimensionResult, ReviewResult},
};

/// StyleDNA 维度调整建议
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StyleDnaDelta {
    /// 句长调整（字符数）
    pub sentence_length_delta: i32,
    /// 对话比例调整
    pub dialogue_ratio_delta: f32,
    /// 比喻密度调整（个/千字）
    pub metaphor_density_delta: f32,
    /// 内心独白比例调整
    pub interior_monologue_delta: f32,
    /// 情感词密度调整
    pub emotion_density_delta: f32,
    /// 节奏起伏度调整
    pub rhythm_score_delta: f32,
    /// 词汇密度档位变化（如 Some("medium") 表示建议调整为 medium）
    pub vocabulary_density_shift: Option<String>,
    /// 情感外露档位变化
    pub expressiveness_shift: Option<String>,
    /// 需要新增的避讳模式
    pub avoided_patterns_add: Vec<String>,
    /// 调整原因摘要
    pub reasons: Vec<String>,
}

impl StyleDnaDelta {
    /// 将 Delta 应用到现有 StyleDNA，返回新的 DNA
    pub fn apply(&self, base: &StyleDNA) -> StyleDNA {
        let mut evolved = base.clone();

        // 数值型维度：应用增量并 clamp
        let new_sl = (base.syntax.avg_sentence_length as i32 + self.sentence_length_delta)
            .clamp(5, 200) as u32;
        evolved.syntax.avg_sentence_length = new_sl;

        evolved.dialogue.dialogue_ratio =
            (base.dialogue.dialogue_ratio + self.dialogue_ratio_delta).clamp(0.0, 1.0);

        evolved.rhetoric.metaphor_density =
            (base.rhetoric.metaphor_density + self.metaphor_density_delta).clamp(0.0, 50.0);

        evolved.perspective.interior_monologue_ratio = (base.perspective.interior_monologue_ratio
            + self.interior_monologue_delta)
            .clamp(0.0, 1.0);

        evolved.emotion.emotion_word_density =
            (base.emotion.emotion_word_density + self.emotion_density_delta).clamp(0.0, 1.0);

        // 档位型维度
        if let Some(ref density) = self.vocabulary_density_shift {
            evolved.vocabulary.density = density.clone();
        }
        if let Some(ref expr) = self.expressiveness_shift {
            evolved.emotion.expressiveness = expr.clone();
        }

        // 避讳模式
        for pattern in &self.avoided_patterns_add {
            if !evolved.vocabulary.avoided_patterns.contains(pattern) {
                evolved.vocabulary.avoided_patterns.push(pattern.clone());
            }
        }

        evolved
    }

    /// 判断是否有实质调整
    pub fn is_empty(&self) -> bool {
        self.sentence_length_delta == 0
            && self.dialogue_ratio_delta == 0.0
            && self.metaphor_density_delta == 0.0
            && self.interior_monologue_delta == 0.0
            && self.emotion_density_delta == 0.0
            && self.rhythm_score_delta == 0.0
            && self.vocabulary_density_shift.is_none()
            && self.expressiveness_shift.is_none()
            && self.avoided_patterns_add.is_empty()
    }
}

/// 风格演化引擎
pub struct StyleEvolutionEngine;

impl StyleEvolutionEngine {
    pub fn new() -> Self {
        Self
    }

    /// 综合应用 Anti-AI Review 和 Pipeline Review 的反馈
    pub fn evolve_from_reviews(
        &self,
        _base: &StyleDNA,
        anti_ai: Option<&AntiAiReview>,
        pipeline: Option<&ReviewResult>,
    ) -> StyleDnaDelta {
        let mut delta = StyleDnaDelta::default();

        if let Some(review) = anti_ai {
            Self::accumulate_anti_ai(&mut delta, review);
        }
        if let Some(review) = pipeline {
            Self::accumulate_pipeline(&mut delta, review);
        }

        delta
    }

    // ==================== Anti-AI Review 映射 ====================

    fn accumulate_anti_ai(delta: &mut StyleDnaDelta, review: &AntiAiReview) {
        for issue in &review.issues {
            Self::map_anti_ai_issue(delta, issue);
        }

        // 维度层面：整体得分过低的维度给出粗略调整
        for dim in &review.dimensions {
            if dim.score < 0.5 {
                delta.reasons.push(format!(
                    "Anti-AI {}维度得分 {:.0}%，建议重点改进",
                    dim.name,
                    dim.score * 100.0
                ));
            }
        }
    }

    fn map_anti_ai_issue(delta: &mut StyleDnaDelta, issue: &ReviewIssue) {
        let severity_multiplier = match issue.severity.as_str() {
            "critical" => 3.0_f32,
            "high" => 2.0,
            "medium" => 1.0,
            _ => 0.5,
        };

        match issue.dimension.as_str() {
            "词汇" => {
                if issue.description.contains("cliché") || issue.description.contains("同质化")
                {
                    delta.avoided_patterns_add.push(issue.example.clone());
                    delta.vocabulary_density_shift = Some("high".to_string());
                    delta
                        .reasons
                        .push(format!("词汇 cliché: {}", issue.suggestion));
                }
                if issue.description.contains("重复用词") {
                    delta.vocabulary_density_shift = Some("high".to_string());
                    delta
                        .reasons
                        .push(format!("重复用词: {}", issue.suggestion));
                }
            }
            "语法" => {
                if issue.description.contains("短句过多") {
                    delta.sentence_length_delta += (5.0 * severity_multiplier) as i32;
                    delta.rhythm_score_delta += 0.05 * severity_multiplier;
                    delta
                        .reasons
                        .push(format!("短句碎片化: {}", issue.suggestion));
                }
                if issue.description.contains("长句过多") {
                    delta.sentence_length_delta -= (5.0 * severity_multiplier) as i32;
                    delta.rhythm_score_delta += 0.05 * severity_multiplier;
                    delta
                        .reasons
                        .push(format!("长句负担: {}", issue.suggestion));
                }
                if issue.description.contains("被动") {
                    delta.sentence_length_delta += (2.0 * severity_multiplier) as i32;
                    delta
                        .reasons
                        .push(format!("被动句式: {}", issue.suggestion));
                }
            }
            "叙事" => {
                if issue.description.contains("流水账") || issue.description.contains("均匀") {
                    delta.rhythm_score_delta += 0.1 * severity_multiplier;
                    delta.sentence_length_delta += (3.0 * severity_multiplier) as i32;
                    delta
                        .reasons
                        .push(format!("叙事节奏: {}", issue.suggestion));
                }
                if issue.description.contains("叙事密度过低") {
                    delta.metaphor_density_delta += 1.0 * severity_multiplier;
                    delta.emotion_density_delta += 0.01 * severity_multiplier;
                    delta
                        .reasons
                        .push(format!("叙事密度: {}", issue.suggestion));
                }
            }
            "情感" => {
                if issue.description.contains("标签化") {
                    delta.emotion_density_delta -= 0.02 * severity_multiplier;
                    delta.expressiveness_shift = Some("restrained".to_string());
                    delta.interior_monologue_delta -= 0.05 * severity_multiplier;
                    delta
                        .reasons
                        .push(format!("情感标签化: {}", issue.suggestion));
                }
                if issue.description.contains("内心独白占比偏高") {
                    delta.interior_monologue_delta -= 0.05 * severity_multiplier;
                    delta
                        .reasons
                        .push(format!("内心独白过多: {}", issue.suggestion));
                }
            }
            "对话" => {
                if issue.description.contains("说明性") {
                    delta.dialogue_ratio_delta -= 0.05 * severity_multiplier;
                    delta
                        .reasons
                        .push(format!("对话说明性: {}", issue.suggestion));
                }
                if issue.description.contains("标签单调") {
                    delta.dialogue_ratio_delta += 0.02 * severity_multiplier;
                    delta
                        .reasons
                        .push(format!("对话标签单调: {}", issue.suggestion));
                }
            }
            _ => {}
        }
    }

    // ==================== Pipeline Review 映射 ====================

    fn accumulate_pipeline(delta: &mut StyleDnaDelta, review: &ReviewResult) {
        for dim in &review.dimensions {
            Self::map_pipeline_dimension(delta, dim);
        }

        // 如果总分过低，给出总体调整
        if review.overall_score < 60.0 {
            delta.reasons.push(format!(
                "Pipeline 综合评分 {:.0}，建议全面检查风格一致性",
                review.overall_score
            ));
        }
    }

    fn map_pipeline_dimension(delta: &mut StyleDnaDelta, dim: &ReviewDimensionResult) {
        let score = dim.score;
        let name = dim.name.to_lowercase();

        if score >= 70.0 {
            return;
        }

        let strength = (70.0 - score) / 70.0; // 0-1 之间，越低分力度越大

        match name.as_str() {
            "continuity" | "logic" | "foreshadow" => {
                // 剧情类问题：增加内心独白以解释动机
                delta.interior_monologue_delta += 0.03 * strength;
                delta
                    .reasons
                    .push(format!("{}: 增加内心独白解释动机", dim.name));
            }
            "character" => {
                // 角色一致性：通过对话和动作强化人设
                delta.dialogue_ratio_delta += 0.03 * strength;
                delta
                    .reasons
                    .push(format!("{}: 调整对话比例强化人设", dim.name));
            }
            "pacing" => {
                // 节奏问题：调整句长和节奏起伏
                if dim.comment.contains("拖沓") {
                    delta.sentence_length_delta -= (5.0 * strength) as i32;
                    delta.rhythm_score_delta += 0.08 * strength;
                } else if dim.comment.contains("跳跃") {
                    delta.sentence_length_delta += (3.0 * strength) as i32;
                    delta.rhythm_score_delta -= 0.05 * strength;
                }
                delta.reasons.push(format!("{}: 调整叙事节奏", dim.name));
            }
            "style" => {
                // 风格问题：综合调整
                if dim.comment.contains("描写") || dim.comment.contains("画面") {
                    delta.metaphor_density_delta += 1.0 * strength;
                    delta.emotion_density_delta += 0.01 * strength;
                }
                if dim.comment.contains("对白") || dim.comment.contains("对话") {
                    delta.dialogue_ratio_delta += 0.03 * strength;
                }
                delta.reasons.push(format!("{}: 综合风格调整", dim.name));
            }
            _ => {}
        }
    }
}

impl Default for StyleEvolutionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anti_ai::{AntiAiReviewer, DimensionScore};

    #[test]
    fn test_evolve_from_anti_ai() {
        let reviewer = AntiAiReviewer::new();
        let review = reviewer.review(
            "他很生气。她很高兴。非常简单。总而言之，这是显而易见的。",
            None,
        );

        let base = StyleDNA::new("测试");
        let engine = StyleEvolutionEngine::new();
        let delta = engine.evolve_from_reviews(&base, Some(&review), None);

        // 情感标签化 + 词汇 cliché 应该会触发调整
        assert!(!delta.is_empty(), "应该有调整建议");
        assert!(!delta.avoided_patterns_add.is_empty() || delta.emotion_density_delta != 0.0);
    }

    #[test]
    fn test_delta_apply() {
        let mut base = StyleDNA::new("测试");
        base.syntax.avg_sentence_length = 30;
        base.dialogue.dialogue_ratio = 0.3;
        base.rhetoric.metaphor_density = 3.0;

        let delta = StyleDnaDelta {
            sentence_length_delta: 10,
            dialogue_ratio_delta: 0.1,
            metaphor_density_delta: 2.0,
            ..Default::default()
        };

        let evolved = delta.apply(&base);
        assert_eq!(evolved.syntax.avg_sentence_length, 40);
        assert!((evolved.dialogue.dialogue_ratio - 0.4).abs() < 0.01);
        assert!((evolved.rhetoric.metaphor_density - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_empty_delta() {
        let delta = StyleDnaDelta::default();
        assert!(delta.is_empty());
    }

    #[test]
    fn test_pipeline_review_mapping() {
        let base = StyleDNA::new("测试");
        let engine = StyleEvolutionEngine::new();

        let pipeline_review = ReviewResult {
            review_id: "r1".to_string(),
            overall_score: 55.0,
            dimensions: vec![
                ReviewDimensionResult {
                    name: "pacing".to_string(),
                    score: 50.0,
                    comment: "节奏拖沓，部分段落冗长".to_string(),
                },
                ReviewDimensionResult {
                    name: "style".to_string(),
                    score: 60.0,
                    comment: "描写不够生动".to_string(),
                },
            ],
            issues: vec![],
            summary: "测试".to_string(),
        };

        let delta = engine.evolve_from_reviews(&base, None, Some(&pipeline_review));
        assert!(!delta.is_empty());
        assert!(delta.sentence_length_delta < 0 || delta.rhythm_score_delta > 0.0);
    }
}
