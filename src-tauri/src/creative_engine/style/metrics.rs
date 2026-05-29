//! Style Metrics - 六维风格向量计算
//!
//! 从文本样例中提取可量化的六维风格特征：
//! - 平均句长 (sentence_length)
//! - 对话比例 (dialogue_ratio)
//! - 比喻密度 (metaphor_density)
//! - 内心独白比例 (inner_monologue_ratio)
//! - 情感词密度 (emotion_density)
//! - 节奏起伏度 (rhythm_score)

use serde::{Deserialize, Serialize};

/// 六维风格度量向量
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct StyleMetrics {
    /// 平均句长（中文字符数）
    pub sentence_length: f32,
    /// 对话占总文本的比例
    pub dialogue_ratio: f32,
    /// 比喻密度（个/千字）
    pub metaphor_density: f32,
    /// 内心独白占句子比例
    pub inner_monologue_ratio: f32,
    /// 情感词密度（情感词数/总字符数）
    pub emotion_density: f32,
    /// 节奏起伏度（0.0=平稳，1.0=剧烈起伏）
    pub rhythm_score: f32,
}

impl StyleMetrics {
    /// 从文本一次性计算全部六维指标
    pub fn from_text(text: &str) -> Self {
        Self {
            sentence_length: compute_sentence_length(text),
            dialogue_ratio: compute_dialogue_ratio(text),
            metaphor_density: compute_metaphor_density(text),
            inner_monologue_ratio: compute_inner_monologue_ratio(text),
            emotion_density: compute_emotion_density(text),
            rhythm_score: compute_rhythm_score(text),
        }
    }

    /// 计算与目标向量的欧几里得距离（归一化后）
    pub fn distance(&self, other: &StyleMetrics) -> f32 {
        let diff_sl = self.sentence_length - other.sentence_length;
        let diff_dr = (self.dialogue_ratio - other.dialogue_ratio) * 100.0;
        let diff_md = self.metaphor_density - other.metaphor_density;
        let diff_im = (self.inner_monologue_ratio - other.inner_monologue_ratio) * 100.0;
        let diff_ed = (self.emotion_density - other.emotion_density) * 100.0;
        let diff_rs = (self.rhythm_score - other.rhythm_score) * 50.0;

        ((diff_sl * diff_sl
            + diff_dr * diff_dr
            + diff_md * diff_md
            + diff_im * diff_im
            + diff_ed * diff_ed
            + diff_rs * diff_rs)
            / 6.0)
            .sqrt()
    }
}

/// 将文本拆分为句子（支持中英文标点）
fn split_sentences(text: &str) -> Vec<&str> {
    text.split(['。', '！', '？', '.', '!', '?'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 1. 计算平均句长（字符数）
pub fn compute_sentence_length(text: &str) -> f32 {
    let sentences = split_sentences(text);
    if sentences.is_empty() {
        return 0.0;
    }
    let total_chars: usize = sentences.iter().map(|s| s.chars().count()).sum();
    total_chars as f32 / sentences.len() as f32
}

/// 2. 计算对话比例
///
/// 基于引号标记字符数估算对话占比。
pub fn compute_dialogue_ratio(text: &str) -> f32 {
    let char_count = text.chars().count();
    if char_count == 0 {
        return 0.0;
    }

    let dialogue_markers = ['"', '「', '『', '\''];
    let dialogue_chars: usize = text
        .chars()
        .collect::<Vec<_>>()
        .windows(2)
        .filter(|w| dialogue_markers.contains(&w[0]))
        .count();

    (dialogue_chars as f32 / char_count as f32).min(1.0)
}

/// 3. 计算比喻密度（个/千字）
pub fn compute_metaphor_density(text: &str) -> f32 {
    let metaphor_markers = ["像", "如", "似", "仿佛", "好比"];
    let metaphor_count: usize = metaphor_markers
        .iter()
        .map(|&m| text.matches(m).count())
        .sum();
    let thousand_chars = text.chars().count() as f32 / 1000.0;
    if thousand_chars > 0.0 {
        metaphor_count as f32 / thousand_chars
    } else {
        0.0
    }
}

/// 4. 计算内心独白比例（内心独白标记句 / 总句数）
pub fn compute_inner_monologue_ratio(text: &str) -> f32 {
    let sentences = split_sentences(text);
    if sentences.is_empty() {
        return 0.0;
    }

    let interior_markers = ["想", "觉得", "感到", "心想", "暗想", "寻思", "琢磨"];
    let interior_count: usize = sentences
        .iter()
        .filter(|&s| interior_markers.iter().any(|&m| s.contains(m)))
        .count();

    (interior_count as f32 / sentences.len() as f32).min(1.0)
}

/// 5. 计算情感词密度（情感词数 / 总字符数）
pub fn compute_emotion_density(text: &str) -> f32 {
    let char_count = text.chars().count();
    if char_count == 0 {
        return 0.0;
    }

    let emotion_words = [
        "爱", "恨", "悲", "喜", "怒", "哀", "乐", "忧", "愁", "欢", "痛", "苦", "甜", "酸", "涩",
        "暖", "冷", "热", "凉", "湿",
    ];
    let emotion_count: usize = emotion_words.iter().map(|&w| text.matches(w).count()).sum();

    emotion_count as f32 / char_count as f32
}

/// 6. 计算节奏起伏度
///
/// 基于句子长度的变异系数（CV = 标准差 / 均值）衡量节奏起伏。
/// CV 越高，长短句交替越剧烈，节奏越起伏。
pub fn compute_rhythm_score(text: &str) -> f32 {
    let sentences = split_sentences(text);
    if sentences.len() < 2 {
        return 0.5;
    }

    let lengths: Vec<f32> = sentences.iter().map(|s| s.chars().count() as f32).collect();
    let avg = lengths.iter().sum::<f32>() / lengths.len() as f32;
    if avg == 0.0 {
        return 0.5;
    }

    let variance = lengths.iter().map(|&l| (l - avg).powi(2)).sum::<f32>() / lengths.len() as f32;
    let std_dev = variance.sqrt();
    let cv = std_dev / avg;

    // 将 CV 映射到 0.0-1.0：CV=0 → 0.0, CV≥1.0 → 1.0
    cv.clamp(0.0, 1.0)
}

/// 根据句长推断句法复杂度标签
pub fn infer_clause_complexity(avg_sentence_length: f32) -> String {
    if avg_sentence_length > 40.0 {
        "complex".to_string()
    } else if avg_sentence_length > 20.0 {
        "moderate".to_string()
    } else {
        "simple".to_string()
    }
}

/// 根据情感词密度推断情感外露程度标签
pub fn infer_expressiveness(emotion_density: f32) -> String {
    if emotion_density > 0.08 {
        "expressive".to_string()
    } else if emotion_density > 0.03 {
        "balanced".to_string()
    } else {
        "restrained".to_string()
    }
}

/// 根据代词频率推断视角类型
pub fn infer_pov_type(text: &str) -> String {
    let first_person = text.matches("我").count() + text.matches("咱").count();
    let third_person = text.matches("他").count() + text.matches("她").count();
    if first_person > third_person * 2 {
        "first_person".to_string()
    } else if third_person > first_person * 2 {
        "close_third".to_string()
    } else {
        "omniscient".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sentence_length() {
        let text = "这是一句。这是第二句，比较长一点。";
        let avg = compute_sentence_length(text);
        assert!(avg > 0.0);
    }

    #[test]
    fn test_compute_dialogue_ratio() {
        let text = "「你好。」他说。「再见。」她答。剩下的都是叙述。";
        let ratio = compute_dialogue_ratio(text);
        assert!(ratio > 0.0);
        assert!(ratio <= 1.0);
    }

    #[test]
    fn test_compute_metaphor_density() {
        let text = "他像山一样稳。她的眼睛如星星般闪烁。时间仿佛凝固了。";
        let density = compute_metaphor_density(text);
        assert!(density > 0.0);
    }

    #[test]
    fn test_compute_inner_monologue_ratio() {
        let text = "他心想，这不对。她感到一阵寒意。天气不错。";
        let ratio = compute_inner_monologue_ratio(text);
        assert!(ratio > 0.0);
        assert!(ratio <= 1.0);
    }

    #[test]
    fn test_compute_emotion_density() {
        let text = "爱恨情仇，悲喜交加。";
        let density = compute_emotion_density(text);
        assert!(density > 0.0);
    }

    #[test]
    fn test_compute_rhythm_score() {
        // 短句和长句交替，节奏起伏大
        let text = "短。很短。这是一个非常非常非常非常非常非常长的句子，包含很多内容。短。";
        let score = compute_rhythm_score(text);
        assert!(score > 0.3, "期望起伏度 > 0.3，实际 {}", score);

        // 句子长度均匀，节奏平稳
        let text2 = "句子长度差不多。每个句子都差不多长。没有什么变化。";
        let score2 = compute_rhythm_score(text2);
        assert!(score2 < 0.5, "期望起伏度 < 0.5，实际 {}", score2);
    }

    #[test]
    fn test_metrics_distance() {
        let m1 = StyleMetrics {
            sentence_length: 30.0,
            dialogue_ratio: 0.3,
            metaphor_density: 5.0,
            inner_monologue_ratio: 0.2,
            emotion_density: 0.05,
            rhythm_score: 0.5,
        };
        let m2 = StyleMetrics {
            sentence_length: 30.0,
            dialogue_ratio: 0.3,
            metaphor_density: 5.0,
            inner_monologue_ratio: 0.2,
            emotion_density: 0.05,
            rhythm_score: 0.5,
        };
        assert_eq!(m1.distance(&m2), 0.0);

        let m3 = StyleMetrics {
            sentence_length: 60.0,
            dialogue_ratio: 0.6,
            metaphor_density: 10.0,
            inner_monologue_ratio: 0.4,
            emotion_density: 0.10,
            rhythm_score: 1.0,
        };
        assert!(m1.distance(&m3) > 0.0);
    }
}
