//! Style DNA - 风格基因模型
//!
//! 从"排版皮肤"升级为"创作基因"，量化描述任意写作风格。
//!
//! StyleDNA 包含六个维度：
//! - 词汇特征 (VocabularyProfile)
//! - 句法特征 (SyntaxProfile)
//! - 修辞偏好 (RhetoricProfile)
//! - 视角规范 (PerspectiveProfile)
//! - 情感表达 (EmotionProfile)
//! - 对话风格 (DialogueProfile)

use serde::{Deserialize, Serialize};

/// 风格 DNA（完整的风格量化描述）
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct StyleDNA {
    pub meta: StyleMeta,
    pub vocabulary: VocabularyProfile,
    pub syntax: SyntaxProfile,
    pub rhetoric: RhetoricProfile,
    pub perspective: PerspectiveProfile,
    pub emotion: EmotionProfile,
    pub dialogue: DialogueProfile,
}

/// 风格元信息
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct StyleMeta {
    pub name: String,
    pub author: Option<String>, // 来源作家（如"金庸"）
    pub description: String,
    pub genre_association: Option<String>, // 关联题材
}

/// 词汇特征
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct VocabularyProfile {
    /// 词汇密度: low / medium / high
    pub density: String,
    /// 抽象度: concrete / balanced / abstract
    pub abstraction: String,
    /// 时代感: archaic / modern / mixed / futuristic
    pub temporal_quality: String,
    /// 偏好词类（如["武侠术语","古典诗词","色彩词汇"]）
    pub preferred_categories: Vec<String>,
    /// 高频标志性词汇
    pub signature_words: Vec<String>,
    /// 避讳词汇类型
    pub avoided_patterns: Vec<String>,
}

/// 句法特征
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SyntaxProfile {
    /// 平均句长（中文字符数）
    pub avg_sentence_length: u32,
    /// 从句复杂度: simple / moderate / complex
    pub clause_complexity: String,
    /// 节奏模式描述
    pub rhythm_pattern: String,
    /// 偏好句式（如["四字格","长短交替","排比"]）
    pub preferred_structures: Vec<String>,
    /// 句子开头多样性: repetitive / moderate / varied
    pub opening_variety: String,
    /// 标点运用特征
    pub punctuation_style: String,
}

/// 修辞偏好
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RhetoricProfile {
    /// 比喻密度（每千字）
    pub metaphor_density: f32,
    /// 偏好修辞手法
    pub preferred_devices: Vec<String>,
    /// 意象偏好（如["自然意象","色彩意象","战争意象"]）
    pub imagery_preference: Vec<String>,
    /// 排比使用频率: rare / moderate / frequent
    pub parallelism_frequency: String,
    /// 反讽/双关使用: none / subtle / overt
    pub irony_usage: String,
}

/// 视角规范
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PerspectiveProfile {
    /// POV 类型: first_person / close_third / omniscient / multiple
    pub pov_type: String,
    /// 叙事距离: intimate / close / moderate / distant
    pub narrative_distance: String,
    /// 内心独白比例（0.0-1.0）
    pub interior_monologue_ratio: f32,
    /// 全知程度（0.0=严格限制，1.0=全知）
    pub omniscience_level: f32,
    /// 时间处理方式: linear / flashback / nonlinear / stream
    pub temporal_handling: String,
}

/// 情感表达
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct EmotionProfile {
    /// 外露程度: restrained / balanced / expressive / melodramatic
    pub expressiveness: String,
    /// 情感词汇密度（相对于总词汇的比例）
    pub emotion_word_density: f32,
    /// 主要情感基调
    pub dominant_mood: String,
    /// 情感变化节奏: gradual / sudden / cyclical / static
    pub emotional_arc_pattern: String,
    /// 幽默感: none / dry / witty / slapstick / dark
    pub humor_style: String,
}

/// 对话风格
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct DialogueProfile {
    /// 对话比例（对话占总文本的比例）
    pub dialogue_ratio: f32,
    /// 对话长度: terse / moderate / verbose
    pub dialogue_length: String,
    /// 潜台词比例（0.0=直说，1.0=全靠暗示）
    pub subtext_ratio: f32,
    /// 对话特征（如["说话前先动作","方言特征","古典白话"]）
    pub signature_patterns: Vec<String>,
    /// 对话标签偏好: said_only / varied_tags / action_beats / minimal
    pub tag_style: String,
}

impl StyleDNA {
    /// 创建新的空 StyleDNA
    pub fn new(name: &str) -> Self {
        Self {
            meta: StyleMeta {
                name: name.to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// 将 StyleDNA 转换为可用于注入 Writer prompt 的文本
    pub fn to_prompt_extension(&self) -> String {
        let mut parts = vec![format!("【风格DNA: {}】", self.meta.name), String::new()];

        if !self.meta.description.is_empty() {
            parts.push(format!("概述: {}", self.meta.description));
        }

        // 词汇
        parts.push("\n【词汇特征】".to_string());
        parts.push(format!("- 密度: {}", self.vocabulary.density));
        parts.push(format!("- 抽象度: {}", self.vocabulary.abstraction));
        if !self.vocabulary.preferred_categories.is_empty() {
            parts.push(format!(
                "- 偏好词类: {}",
                self.vocabulary.preferred_categories.join("、")
            ));
        }
        if !self.vocabulary.signature_words.is_empty() {
            parts.push(format!(
                "- 标志性词汇: {}",
                self.vocabulary.signature_words.join("、")
            ));
        }

        // 句法
        parts.push("\n【句法特征】".to_string());
        parts.push(format!(
            "- 平均句长: {} 字",
            self.syntax.avg_sentence_length
        ));
        parts.push(format!("- 从句复杂度: {}", self.syntax.clause_complexity));
        if !self.syntax.rhythm_pattern.is_empty() {
            parts.push(format!("- 节奏模式: {}", self.syntax.rhythm_pattern));
        }
        if !self.syntax.preferred_structures.is_empty() {
            parts.push(format!(
                "- 偏好句式: {}",
                self.syntax.preferred_structures.join("、")
            ));
        }

        // 修辞
        parts.push("\n【修辞偏好】".to_string());
        parts.push(format!(
            "- 比喻密度: {:.1} 个/千字",
            self.rhetoric.metaphor_density
        ));
        if !self.rhetoric.preferred_devices.is_empty() {
            parts.push(format!(
                "- 偏好手法: {}",
                self.rhetoric.preferred_devices.join("、")
            ));
        }
        if !self.rhetoric.imagery_preference.is_empty() {
            parts.push(format!(
                "- 意象偏好: {}",
                self.rhetoric.imagery_preference.join("、")
            ));
        }

        // 视角
        parts.push("\n【视角规范】".to_string());
        parts.push(format!("- POV: {}", self.perspective.pov_type));
        parts.push(format!(
            "- 叙事距离: {}",
            self.perspective.narrative_distance
        ));
        parts.push(format!(
            "- 内心独白比例: {:.0}%",
            self.perspective.interior_monologue_ratio * 100.0
        ));

        // 情感
        parts.push("\n【情感表达】".to_string());
        parts.push(format!("- 外露程度: {}", self.emotion.expressiveness));
        parts.push(format!("- 主要基调: {}", self.emotion.dominant_mood));
        if !self.emotion.humor_style.is_empty() && self.emotion.humor_style != "none" {
            parts.push(format!("- 幽默感: {}", self.emotion.humor_style));
        }

        // 对话
        parts.push("\n【对话风格】".to_string());
        parts.push(format!(
            "- 对话占比: {:.0}%",
            self.dialogue.dialogue_ratio * 100.0
        ));
        parts.push(format!("- 对话长度: {}", self.dialogue.dialogue_length));
        parts.push(format!(
            "- 潜台词比例: {:.0}%",
            self.dialogue.subtext_ratio * 100.0
        ));
        if !self.dialogue.signature_patterns.is_empty() {
            parts.push(format!(
                "- 对话特征: {}",
                self.dialogue.signature_patterns.join("、")
            ));
        }

        parts.push("\n【写作指令】".to_string());
        parts.push("你必须严格遵循以上风格DNA的所有维度进行写作。".to_string());
        parts.push("每一句话、每一个词的选择都要符合该风格的量化特征。".to_string());

        parts.join("\n")
    }

    /// 计算两个 StyleDNA 的相似度（0.0-1.0）
    pub fn similarity(&self, other: &StyleDNA) -> f32 {
        let mut score = 0.0;
        let mut count = 0.0;

        // 词汇密度匹配
        if !self.vocabulary.density.is_empty() && !other.vocabulary.density.is_empty() {
            score += if self.vocabulary.density == other.vocabulary.density {
                1.0
            } else {
                0.0
            };
            count += 1.0;
        }

        // 平均句长差异（越小越相似，归一化到 0-1）
        if self.syntax.avg_sentence_length > 0 && other.syntax.avg_sentence_length > 0 {
            let diff = (self.syntax.avg_sentence_length as f32
                - other.syntax.avg_sentence_length as f32)
                .abs();
            let max_len = self
                .syntax
                .avg_sentence_length
                .max(other.syntax.avg_sentence_length) as f32;
            score += if max_len > 0.0 {
                1.0 - (diff / max_len).min(1.0)
            } else {
                1.0
            };
            count += 1.0;
        }

        // 比喻密度差异
        if self.rhetoric.metaphor_density > 0.0 || other.rhetoric.metaphor_density > 0.0 {
            let diff = (self.rhetoric.metaphor_density - other.rhetoric.metaphor_density).abs();
            score += 1.0 - diff.min(1.0);
            count += 1.0;
        }

        // 对话比例差异
        if self.dialogue.dialogue_ratio > 0.0 || other.dialogue.dialogue_ratio > 0.0 {
            let diff = (self.dialogue.dialogue_ratio - other.dialogue.dialogue_ratio).abs();
            score += 1.0 - diff.min(1.0);
            count += 1.0;
        }

        // POV 类型匹配
        if !self.perspective.pov_type.is_empty() && !other.perspective.pov_type.is_empty() {
            score += if self.perspective.pov_type == other.perspective.pov_type {
                1.0
            } else {
                0.0
            };
            count += 1.0;
        }

        // 情感外露程度匹配
        if !self.emotion.expressiveness.is_empty() && !other.emotion.expressiveness.is_empty() {
            score += if self.emotion.expressiveness == other.emotion.expressiveness {
                1.0
            } else {
                0.0
            };
            count += 1.0;
        }

        if count > 0.0 {
            score / count
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_dna_new() {
        let dna = StyleDNA::new("测试风格");
        assert_eq!(dna.meta.name, "测试风格");
    }

    #[test]
    fn test_to_prompt_extension() {
        let mut dna = StyleDNA::new("古典深沉");
        dna.vocabulary.density = "high".to_string();
        dna.syntax.avg_sentence_length = 45;
        dna.emotion.expressiveness = "restrained".to_string();

        let prompt = dna.to_prompt_extension();
        assert!(prompt.contains("古典深沉"));
        assert!(prompt.contains("high"));
        assert!(prompt.contains("45"));
        assert!(prompt.contains("restrained"));
    }

    #[test]
    fn test_similarity_identical() {
        let dna1 = StyleDNA::new("风格A");
        let dna2 = StyleDNA::new("风格B");
        let sim = dna1.similarity(&dna2);
        assert_eq!(sim, 0.0); // 空的 DNA 相似度为0
    }

    #[test]
    fn test_similarity_with_data() {
        let mut dna1 = StyleDNA::new("风格1");
        dna1.vocabulary.density = "high".to_string();
        dna1.syntax.avg_sentence_length = 40;
        dna1.rhetoric.metaphor_density = 0.08;
        dna1.perspective.pov_type = "omniscient".to_string();
        dna1.emotion.expressiveness = "restrained".to_string();
        dna1.dialogue.dialogue_ratio = 0.3;

        let mut dna2 = StyleDNA::new("风格2");
        dna2.vocabulary.density = "high".to_string();
        dna2.syntax.avg_sentence_length = 40;
        dna2.rhetoric.metaphor_density = 0.08;
        dna2.perspective.pov_type = "omniscient".to_string();
        dna2.emotion.expressiveness = "restrained".to_string();
        dna2.dialogue.dialogue_ratio = 0.3;

        assert_eq!(dna1.similarity(&dna2), 1.0);
    }
}
