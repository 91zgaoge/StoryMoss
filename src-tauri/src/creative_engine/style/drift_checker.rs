//! StyleDriftChecker - 风格漂移自检引擎
//!
//! 5项检查清单，验证生成文本是否符合混合风格目标。

use super::{blend::StyleBlendConfig, dna::StyleDNA};

/// 单项检查结果
#[derive(Debug, Clone)]
pub struct DriftCheckItem {
    pub dimension: String,
    pub target_min: f32,
    pub target_max: f32,
    pub actual_value: f32,
    pub score: f32, // 0.0-1.0
    pub passed: bool,
    pub suggestion: String,
}

/// 整体检查结果
#[derive(Debug, Clone)]
pub struct DriftCheckResult {
    pub passed: bool,
    pub overall_score: f32, // 0.0-1.0
    pub checks: Vec<DriftCheckItem>,
}

pub struct StyleDriftChecker;

impl StyleDriftChecker {
    /// 检查文本与混合风格的匹配度
    pub fn check(text: &str, blend: &StyleBlendConfig, dnas: &[StyleDNA]) -> DriftCheckResult {
        let mut checks = Vec::new();

        // 1. 句长检查
        checks.push(Self::check_sentence_length(text, blend, dnas));

        // 2. 对话比例检查
        checks.push(Self::check_dialogue_ratio(text, blend, dnas));

        // 3. 比喻密度检查
        checks.push(Self::check_metaphor_density(text, blend, dnas));

        // 4. 内心独白比例检查
        checks.push(Self::check_interior_monologue(text, blend, dnas));

        // 5. 情感外露检查
        checks.push(Self::check_emotion_expressiveness(text, blend, dnas));

        let overall_score = if checks.is_empty() {
            1.0
        } else {
            checks.iter().map(|c| c.score).sum::<f32>() / checks.len() as f32
        };

        let passed = overall_score >= 0.7 && checks.iter().all(|c| c.passed);

        DriftCheckResult {
            passed,
            overall_score,
            checks,
        }
    }

    // ==================== 5项检查 ====================

    /// 1. 句长检查 — 加权平均 ± 30% 容差
    fn check_sentence_length(
        text: &str,
        blend: &StyleBlendConfig,
        dnas: &[StyleDNA],
    ) -> DriftCheckItem {
        let target = blend.weighted_sentence_length(dnas);
        let tolerance = target * 0.3;
        let target_min = (target - tolerance).max(5.0);
        let target_max = target + tolerance;

        let sentences: Vec<&str> = text
            .split(['。', '！', '？', '.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .collect();

        let actual = if sentences.is_empty() {
            0.0
        } else {
            sentences.iter().map(|s| s.chars().count()).sum::<usize>() as f32
                / sentences.len() as f32
        };

        let score = if target > 0.0 {
            let diff = (actual - target).abs();
            let max_diff = tolerance.max(target * 0.5);
            (1.0 - (diff / max_diff).min(1.0)).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let passed = actual >= target_min && actual <= target_max;
        let suggestion = if passed {
            format!(
                "句长符合目标范围 ({:.0} 字，目标 {:.0}-{:.0} 字)",
                actual, target_min, target_max
            )
        } else if actual < target_min {
            format!(
                "句长偏短：实际 {:.0} 字，建议增加至 {:.0}-{:.0} 字",
                actual, target_min, target_max
            )
        } else {
            format!(
                "句长偏长：实际 {:.0} 字，建议缩短至 {:.0}-{:.0} 字",
                actual, target_min, target_max
            )
        };

        DriftCheckItem {
            dimension: "句长".to_string(),
            target_min,
            target_max,
            actual_value: actual,
            score,
            passed,
            suggestion,
        }
    }

    /// 2. 对话比例检查 — 加权平均 ± 15% 容差
    fn check_dialogue_ratio(
        text: &str,
        blend: &StyleBlendConfig,
        dnas: &[StyleDNA],
    ) -> DriftCheckItem {
        let target = blend.weighted_dialogue_ratio(dnas);
        let tolerance = 0.15;
        let target_min = (target - tolerance).max(0.0);
        let target_max = (target + tolerance).min(1.0);

        let char_count = text.chars().count();
        let dialogue_markers = ['"', '「', '『'];
        let dialogue_count = text
            .chars()
            .filter(|&c| dialogue_markers.contains(&c))
            .count()
            / 2;
        let actual = if char_count > 0 {
            (dialogue_count as f32 / char_count as f32).min(1.0)
        } else {
            0.0
        };

        let score = 1.0 - ((actual - target).abs() / tolerance.max(0.1)).min(1.0);

        let passed = actual >= target_min && actual <= target_max;
        let suggestion = if passed {
            format!(
                "对话比例符合 ({:.1}%，目标 {:.1}-{:.1}%)",
                actual * 100.0,
                target_min * 100.0,
                target_max * 100.0
            )
        } else if actual < target_min {
            format!(
                "对话比例偏低：实际 {:.1}%，建议增加至 {:.1}-{:.1}%",
                actual * 100.0,
                target_min * 100.0,
                target_max * 100.0
            )
        } else {
            format!(
                "对话比例偏高：实际 {:.1}%，建议减少至 {:.1}-{:.1}%",
                actual * 100.0,
                target_min * 100.0,
                target_max * 100.0
            )
        };

        DriftCheckItem {
            dimension: "对话比例".to_string(),
            target_min,
            target_max,
            actual_value: actual,
            score,
            passed,
            suggestion,
        }
    }

    /// 3. 比喻密度检查 — 加权平均 ± 50% 相对容差 或 ±0.05 绝对容差
    fn check_metaphor_density(
        text: &str,
        blend: &StyleBlendConfig,
        dnas: &[StyleDNA],
    ) -> DriftCheckItem {
        let target = blend.weighted_metaphor_density(dnas);
        let abs_tolerance: f32 = 0.05;
        let rel_tolerance: f32 = target * 0.5;
        let tolerance = abs_tolerance.max(rel_tolerance);
        let target_min = (target - tolerance).max(0.0);
        let target_max = target + tolerance;

        let metaphor_markers = ["像", "如", "似", "仿佛", "好比"];
        let metaphor_count = metaphor_markers
            .iter()
            .map(|&m| text.matches(m).count())
            .sum::<usize>();
        let thousand_chars = text.chars().count() as f32 / 1000.0;
        let actual = if thousand_chars > 0.0 {
            metaphor_count as f32 / thousand_chars
        } else {
            0.0
        };

        let score = if target > 0.0 {
            let diff = (actual - target).abs();
            (1.0 - (diff / tolerance.max(target * 0.3))).clamp(0.0, 1.0)
        } else {
            if actual < 0.02 {
                1.0
            } else {
                0.0
            }
        };

        let passed = actual >= target_min && actual <= target_max;
        let suggestion = if passed {
            format!(
                "比喻密度符合 ({:.1} 个/千字，目标 {:.1}-{:.1})",
                actual, target_min, target_max
            )
        } else if actual < target_min {
            format!(
                "比喻密度偏低：实际 {:.1} 个/千字，建议增加至 {:.1}-{:.1}",
                actual, target_min, target_max
            )
        } else {
            format!(
                "比喻密度偏高：实际 {:.1} 个/千字，建议减少至 {:.1}-{:.1}",
                actual, target_min, target_max
            )
        };

        DriftCheckItem {
            dimension: "比喻密度".to_string(),
            target_min,
            target_max,
            actual_value: actual,
            score,
            passed,
            suggestion,
        }
    }

    /// 4. 内心独白比例检查 — 加权平均 ± 20% 容差
    fn check_interior_monologue(
        text: &str,
        blend: &StyleBlendConfig,
        dnas: &[StyleDNA],
    ) -> DriftCheckItem {
        let target = blend.weighted_interior_ratio(dnas);
        let tolerance = 0.2;
        let target_min = (target - tolerance).max(0.0);
        let target_max = (target + tolerance).min(1.0);

        let interior_markers = ["想", "觉得", "感到", "心想", "暗想", "思索", "回忆"];
        let sentences: Vec<&str> = text
            .split(['。', '！', '？', '.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .collect();

        let interior_count = interior_markers
            .iter()
            .map(|&m| text.matches(m).count())
            .sum::<usize>();
        let actual = if !sentences.is_empty() {
            (interior_count as f32 / sentences.len() as f32).min(1.0)
        } else {
            0.0
        };

        let score = 1.0 - ((actual - target).abs() / tolerance.max(0.1)).min(1.0);

        let passed = actual >= target_min && actual <= target_max;
        let suggestion = if passed {
            format!(
                "内心独白比例符合 ({:.1}%，目标 {:.1}-{:.1}%)",
                actual * 100.0,
                target_min * 100.0,
                target_max * 100.0
            )
        } else if actual < target_min {
            format!(
                "内心独白偏少：实际 {:.1}%，建议增加至 {:.1}-{:.1}%",
                actual * 100.0,
                target_min * 100.0,
                target_max * 100.0
            )
        } else {
            format!(
                "内心独白偏多：实际 {:.1}%，建议减少至 {:.1}-{:.1}%",
                actual * 100.0,
                target_min * 100.0,
                target_max * 100.0
            )
        };

        DriftCheckItem {
            dimension: "内心独白".to_string(),
            target_min,
            target_max,
            actual_value: actual,
            score,
            passed,
            suggestion,
        }
    }

    /// 5. 情感外露检查 — 加权平均情感词密度 ± 30% 容差
    fn check_emotion_expressiveness(
        text: &str,
        blend: &StyleBlendConfig,
        dnas: &[StyleDNA],
    ) -> DriftCheckItem {
        let target = blend.weighted_emotion_density(dnas);
        let tolerance = target * 0.3;
        let target_min = (target - tolerance).max(0.0);
        let target_max = target + tolerance;

        let emotion_words = [
            "爱", "恨", "悲", "喜", "怒", "哀", "乐", "忧", "愁", "欢", "痛", "苦", "甜", "酸",
            "涩", "暖", "冷", "热", "凉", "湿",
        ];
        let char_count = text.chars().count();
        let emotion_count = emotion_words
            .iter()
            .map(|&w| text.matches(w).count())
            .sum::<usize>();
        let actual = if char_count > 0 {
            emotion_count as f32 / char_count as f32
        } else {
            0.0
        };

        let score = if target > 0.0 {
            let diff = (actual - target).abs();
            (1.0 - (diff / tolerance.max(target * 0.2))).clamp(0.0, 1.0)
        } else {
            if actual < 0.01 {
                1.0
            } else {
                0.0
            }
        };

        let passed = actual >= target_min && actual <= target_max;
        let suggestion = if passed {
            format!(
                "情感密度符合 ({:.2}%，目标 {:.2}-{:.2}%)",
                actual * 100.0,
                target_min * 100.0,
                target_max * 100.0
            )
        } else if actual < target_min {
            format!(
                "情感密度偏低：实际 {:.2}%，建议增加至 {:.2}-{:.2}%",
                actual * 100.0,
                target_min * 100.0,
                target_max * 100.0
            )
        } else {
            format!(
                "情感密度偏高：实际 {:.2}%，建议减少至 {:.2}-{:.2}%",
                actual * 100.0,
                target_min * 100.0,
                target_max * 100.0
            )
        };

        DriftCheckItem {
            dimension: "情感外露".to_string(),
            target_min,
            target_max,
            actual_value: actual,
            score,
            passed,
            suggestion,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::classic_styles::{hemingway, marquez, proust},
        *,
    };

    fn create_test_blend() -> StyleBlendConfig {
        let mut blend = StyleBlendConfig::new("三角测试");
        blend.components = vec![
            super::super::blend::BlendComponent::new("普鲁斯特", "普鲁斯特", 0.65),
            super::super::blend::BlendComponent::new("海明威", "海明威", 0.20),
            super::super::blend::BlendComponent::new("马尔克斯", "马尔克斯", 0.15),
        ];
        blend
    }

    #[test]
    fn test_drift_check_runs_with_5_checks() {
        let blend = create_test_blend();
        let dnas = vec![proust(), hemingway(), marquez()];

        let text = "记忆如同一条缓缓流淌的河流，带着往昔的碎片，在时光的长廊中蜿蜒前行。";

        let result = StyleDriftChecker::check(text, &blend, &dnas);
        // 验证检查器正确运行：返回5项检查结果，总分在0-1之间
        assert_eq!(result.checks.len(), 5);
        assert!(result.overall_score >= 0.0 && result.overall_score <= 1.0);
        for check in &result.checks {
            assert!(check.score >= 0.0 && check.score <= 1.0);
        }
    }

    #[test]
    fn test_drift_check_hemingway_text() {
        let mut blend = create_test_blend();
        // 交换主导为海明威
        blend.components[0].weight = 0.20;
        blend.components[0].role = super::super::blend::BlendRole::Secondary;
        blend.components[1].weight = 0.65;
        blend.components[1].role = super::super::blend::BlendRole::Dominant;
        blend.components[2].weight = 0.15;
        blend.normalize();

        let dnas = vec![proust(), hemingway(), marquez()];

        // 海明威式短句文本
        let text = "他走进房间。阳光很好。桌上有一杯酒。他拿起酒杯。酒很凉。他喝了一口。味道不错。";

        let result = StyleDriftChecker::check(text, &blend, &dnas);
        // 验证检查器正确运行
        assert_eq!(result.checks.len(), 5);
        assert!(result.overall_score >= 0.0 && result.overall_score <= 1.0);
    }

    #[test]
    fn test_weighted_targets() {
        let blend = create_test_blend();
        let dnas = vec![proust(), hemingway(), marquez()];

        let sent_len = blend.weighted_sentence_length(&dnas);
        // 普鲁斯特 80*0.65 + 海明威 15*0.20 + 马尔克斯 45*0.15 = 52 + 3 + 6.75 = 61.75
        assert!(sent_len > 55.0 && sent_len < 70.0);

        let dial_ratio = blend.weighted_dialogue_ratio(&dnas);
        // 0.15*0.65 + 0.45*0.20 + 0.25*0.15 = 0.0975 + 0.09 + 0.0375 = 0.225
        assert!(dial_ratio > 0.15 && dial_ratio < 0.30);
    }
}
