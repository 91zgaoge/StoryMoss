//! 风格指纹引擎 — 从任意参考文本提取可量化的风格特征
//!
//! 核心能力：
//! - 句长分布、词汇偏好、N-gram 频率等量化指标提取
//! - 锚点片段采样（用于少样本注入）
//! - 格式化为 LLM prompt 可直接使用的约束文本
//!
//! 复用现有基础设施：
//! - StyleAnalyzer 的文本统计逻辑
//! - AntiAiReviewer 的词频/句法分析

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 风格指纹 — 统一描述任意文本的语言风格
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StyleFingerprint {
    /// 词汇层指纹
    pub vocabulary: VocabularyFingerprint,
    /// 句法层指纹
    pub syntax: SyntaxFingerprint,
    /// 对话层指纹
    pub dialogue: DialogueFingerprint,
    /// 锚点片段 — 最具代表性的原文段落（用于少样本注入）
    pub anchor_samples: Vec<String>,
    /// N-gram 白名单
    pub ngrams: NgramFingerprint,
}

/// 词汇指纹
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VocabularyFingerprint {
    /// 四字格密度（每百字）
    pub four_char_density: f32,
    /// 虚词 TOP10（频率排序）
    pub function_words: Vec<(String, u32)>,
    /// 标志性实词 TOP10
    pub signature_words: Vec<(String, u32)>,
    /// 平均词长（中文字符数）
    pub avg_word_length: f32,
    /// 时代感：classical / modern / mixed
    pub temporal_quality: String,
}

/// 句法指纹
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyntaxFingerprint {
    /// 平均句长（字）
    pub avg_sentence_length: f32,
    /// 句长标准差
    pub sentence_length_std: f32,
    /// 短句占比（<10字）
    pub short_ratio: f32,
    /// 中句占比（10-25字）
    pub medium_ratio: f32,
    /// 长句占比（>25字）
    pub long_ratio: f32,
    /// 逗号密度（每百字）
    pub comma_density: f32,
}

/// 对话指纹
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DialogueFingerprint {
    /// 对话标签分布：("道", 0.8), ("说", 0.1) ...
    pub tag_distribution: Vec<(String, f32)>,
    /// 对话占全文比例
    pub dialogue_ratio: f32,
    /// 是否有对话
    pub has_dialogue: bool,
}

/// N-gram 指纹
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NgramFingerprint {
    /// 高频双字搭配 TOP30
    pub bigrams: Vec<(String, u32)>,
    /// 高频四字词 TOP20
    pub four_char_phrases: Vec<(String, u32)>,
    /// 高频衔接模式 TOP15
    pub transitions: Vec<(String, u32)>,
}

impl StyleFingerprint {
    /// 从任意参考文本提取风格指纹
    pub fn from_text(text: &str) -> Self {
        let text = text.trim();
        if text.is_empty() {
            return Self::default();
        }

        let sentences = split_sentences(text);
        let char_count = text.chars().count() as f32;

        Self {
            vocabulary: extract_vocabulary_fingerprint(text, &sentences, char_count),
            syntax: extract_syntax_fingerprint(&sentences, text, char_count),
            dialogue: extract_dialogue_fingerprint(text, char_count),
            anchor_samples: sample_anchors(text, &sentences, 5),
            ngrams: extract_ngrams(text),
        }
    }

    /// 将指纹格式化为 Writer prompt 可用的约束文本
    pub fn to_prompt_section(&self) -> String {
        let mut lines = Vec::new();

        lines.push("【风格指纹 — 基于参考文本的量化分析】".to_string());
        lines.push("以下数据精确描述了参考文本的语言风格特征，续写时必须严格遵循：".to_string());
        lines.push(String::new());

        // 句法特征
        lines.push("【句法特征】".to_string());
        lines.push(format!(
            "- 平均句长: {:.1}±{:.1} 字（你的续写必须保持此分布，±30% 以内）",
            self.syntax.avg_sentence_length, self.syntax.sentence_length_std
        ));
        lines.push(format!(
            "- 短句占比(<10字): {:.0}% | 中句(10-25字): {:.0}% | 长句(>25字): {:.0}%",
            self.syntax.short_ratio * 100.0,
            self.syntax.medium_ratio * 100.0,
            self.syntax.long_ratio * 100.0
        ));
        lines.push(format!(
            "- 逗号密度: 每百字 {:.1} 个",
            self.syntax.comma_density
        ));
        lines.push(String::new());

        // 词汇偏好
        lines.push("【词汇偏好】".to_string());
        if !self.vocabulary.function_words.is_empty() {
            let fw: Vec<String> = self
                .vocabulary
                .function_words
                .iter()
                .take(8)
                .map(|(w, c)| format!("{}({}次)", w, c))
                .collect();
            lines.push(format!("- 高频虚词（优先使用）: {}", fw.join("、")));
        }
        lines.push(format!(
            "- 四字格密度: {:.1}%",
            self.vocabulary.four_char_density
        ));
        if !self.vocabulary.temporal_quality.is_empty() {
            let era_hint = match self.vocabulary.temporal_quality.as_str() {
                "classical" => "古典白话（禁用'但是''所以''然后'等现代虚词，改用'只是''故''随后'）",
                "modern" => "现代白话",
                "mixed" => "半文半白",
                _ => "",
            };
            if !era_hint.is_empty() {
                lines.push(format!("- 时代感: {}", era_hint));
            }
        }
        lines.push(String::new());

        // 对话标签
        if self.dialogue.has_dialogue && !self.dialogue.tag_distribution.is_empty() {
            lines.push("【对话标签模式】".to_string());
            let tags: Vec<String> = self
                .dialogue
                .tag_distribution
                .iter()
                .take(5)
                .map(|(t, r)| format!("'{}'({:.0}%)", t, r * 100.0))
                .collect();
            lines.push(format!("- 主要标签: {}", tags.join("、")));
            lines.push(String::new());
        }

        // N-gram 白名单
        if !self.ngrams.bigrams.is_empty() || !self.ngrams.four_char_phrases.is_empty() {
            lines.push("【高频搭配白名单】（生成时优先使用）".to_string());
            if !self.ngrams.bigrams.is_empty() {
                let bg: Vec<String> = self
                    .ngrams
                    .bigrams
                    .iter()
                    .take(10)
                    .map(|(s, _)| s.clone())
                    .collect();
                lines.push(format!("- 双字: {}", bg.join("、")));
            }
            if !self.ngrams.four_char_phrases.is_empty() {
                let fc: Vec<String> = self
                    .ngrams
                    .four_char_phrases
                    .iter()
                    .take(8)
                    .map(|(s, _)| s.clone())
                    .collect();
                lines.push(format!("- 四字: {}", fc.join("、")));
            }
            if !self.ngrams.transitions.is_empty() {
                let tr: Vec<String> = self
                    .ngrams
                    .transitions
                    .iter()
                    .take(6)
                    .map(|(s, _)| s.clone())
                    .collect();
                lines.push(format!("- 衔接: {}", tr.join("、")));
            }
            lines.push(String::new());
        }

        // 锚点片段
        if !self.anchor_samples.is_empty() {
            lines.push("【锚点片段示例】（参考以下段落的语感、节奏和用词习惯）".to_string());
            for (i, sample) in self.anchor_samples.iter().enumerate() {
                lines.push(format!("[片段{}] {}", i + 1, sample));
            }
            lines.push(String::new());
        }

        lines.push(
            "重要：以上风格约束优先于叙事约束。如果情节推进需要使用现代词汇或长句，\
             宁可放慢叙事节奏，也要保持语言风格一致。"
                .to_string(),
        );

        lines.join("\n")
    }
}

// ==================== 内部提取函数 ====================

/// 分句（支持中文标点）
fn split_sentences(text: &str) -> Vec<String> {
    text.split(|c: char| c == '。' || c == '！' || c == '？' || c == '.' || c == '!' || c == '?')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// 提取词汇指纹
fn extract_vocabulary_fingerprint(
    text: &str,
    sentences: &[String],
    char_count: f32,
) -> VocabularyFingerprint {
    // 虚词统计
    let function_word_list = vec![
        "的", "了", "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一个", "上", "也",
        "很", "到", "说", "要", "去", "你", "会", "着", "没有", "看", "好", "自己", "这", "那",
        "原来", "且说", "正是", "不想", "因问", "只得", "偏又", "况又", "一面", "连忙", "但是",
        "所以", "然后", "接着", "于是", "不过", "只是", "然而", "因此", "因为", "便", "且", "罢了",
        "而已", "呢", "罢", "么", "罢咧",
    ];
    let mut fw_freq: HashMap<String, u32> = HashMap::new();
    for fw in &function_word_list {
        let count = text.matches(fw).count() as u32;
        if count > 0 {
            *fw_freq.entry(fw.to_string()).or_insert(0) += count;
        }
    }
    let mut function_words: Vec<(String, u32)> = fw_freq.into_iter().collect();
    function_words.sort_by(|a, b| b.1.cmp(&a.1));
    function_words.truncate(10);

    // 标志性实词（简单的字频统计，排除虚词和常见字）
    let common_chars_str = concat!(
        "的一是在不了有和人这中大为上个国我以要他时来用们生到作地于出就分对成会可主发年动同工也能下过子说产种面而方后多定行学法所民得经十三之进着等部度家电力里如水化高自二理起小物现实加量都两体制机当使点从业本去把性好应开它合还因由其些然前外天政四日那社义事平形相全表间样与关各重新线内数正心反你明看原又么利比或但质气第向道命此变条只没结解问意建月公无系军很情者最立代想已通并提直题党程展五果料象员革位入常文总次品式活设及管特件长求老头基资边流路级少图山统接知较将组见计别她手角期根论运农指几九区强放决西被干做必战先回则任取完举色或",
        "他她它们把被让给跟同从向往朝沿叫让请劝逼催求托派使得"
    );
    let common_chars: std::collections::HashSet<char> = common_chars_str.chars().collect();

    let mut char_freq: HashMap<char, u32> = HashMap::new();
    for ch in text.chars() {
        if ch.is_alphabetic() || (ch as u32) > 0x4e00 && (ch as u32) < 0x9fff {
            if !common_chars.contains(&ch) {
                *char_freq.entry(ch).or_insert(0) += 1;
            }
        }
    }
    let mut signature_words: Vec<(String, u32)> = char_freq
        .into_iter()
        .map(|(c, n)| (c.to_string(), n))
        .collect();
    signature_words.sort_by(|a, b| b.1.cmp(&a.1));
    signature_words.truncate(10);

    // 四字格密度
    let four_char_count = count_four_char_phrases(text);
    let four_char_density = if char_count > 0.0 {
        (four_char_count as f32 / char_count * 100.0).min(50.0)
    } else {
        0.0
    };

    // 时代感检测
    let modern_markers = [
        "但是", "所以", "然后", "接着", "不过", "因为", "因此", "虽然",
    ];
    let classical_markers = [
        "原来", "且说", "正是", "不想", "因问", "只得", "偏又", "况又", "一面", "连忙", "便", "且",
        "罢了", "而已",
    ];
    let modern_count: usize = modern_markers.iter().map(|m| text.matches(m).count()).sum();
    let classical_count: usize = classical_markers
        .iter()
        .map(|m| text.matches(m).count())
        .sum();
    let temporal_quality = if classical_count > modern_count * 2 {
        "classical"
    } else if modern_count > classical_count * 2 {
        "modern"
    } else {
        "mixed"
    }
    .to_string();

    VocabularyFingerprint {
        four_char_density,
        function_words,
        signature_words,
        avg_word_length: 0.0, // TODO: 精确计算需分词
        temporal_quality,
    }
}

/// 提取句法指纹
fn extract_syntax_fingerprint(
    sentences: &[String],
    text: &str,
    char_count: f32,
) -> SyntaxFingerprint {
    let sentence_lengths: Vec<usize> = sentences.iter().map(|s| s.chars().count()).collect();

    let avg_len = if !sentence_lengths.is_empty() {
        sentence_lengths.iter().sum::<usize>() as f32 / sentence_lengths.len() as f32
    } else {
        0.0
    };

    let variance = if !sentence_lengths.is_empty() {
        let mean = avg_len;
        sentence_lengths
            .iter()
            .map(|&l| (l as f32 - mean).powi(2))
            .sum::<f32>()
            / sentence_lengths.len() as f32
    } else {
        0.0
    };
    let std = variance.sqrt();

    let short_count = sentence_lengths.iter().filter(|&&l| l < 10).count();
    let medium_count = sentence_lengths
        .iter()
        .filter(|&&l| l >= 10 && l <= 25)
        .count();
    let long_count = sentence_lengths.iter().filter(|&&l| l > 25).count();
    let total = sentences.len().max(1) as f32;

    let comma_count = text.matches('，').count() as f32;
    let comma_density = if char_count > 0.0 {
        comma_count / char_count * 100.0
    } else {
        0.0
    };

    SyntaxFingerprint {
        avg_sentence_length: avg_len,
        sentence_length_std: std,
        short_ratio: short_count as f32 / total,
        medium_ratio: medium_count as f32 / total,
        long_ratio: long_count as f32 / total,
        comma_density,
    }
}

/// 提取对话指纹
fn extract_dialogue_fingerprint(text: &str, char_count: f32) -> DialogueFingerprint {
    let dialogue_markers = ['"', '「', '『', '\'', '＂'];
    let mut dialogue_char_count = 0;
    let mut in_dialogue = false;
    for ch in text.chars() {
        if dialogue_markers.contains(&ch) {
            in_dialogue = !in_dialogue;
        }
        if in_dialogue {
            dialogue_char_count += 1;
        }
    }

    let dialogue_ratio = if char_count > 0.0 {
        (dialogue_char_count as f32 / char_count).min(1.0)
    } else {
        0.0
    };
    let has_dialogue = dialogue_char_count > 0;

    // 对话标签统计
    let tag_patterns = [
        (
            "道",
            vec!["道", "说道", "问道", "答道", "笑道", "喝道", "叫道"],
        ),
        ("说", vec!["说", "说道", "说说", "说过"]),
        ("问", vec!["问", "问道", "询问", "反问", "追问"]),
        ("答", vec!["答", "答道", "回答", "答应"]),
        ("叫", vec!["叫", "叫道", "叫嚷", "叫喊"]),
    ];

    let mut tag_freq: HashMap<String, u32> = HashMap::new();
    for (canonical, variants) in &tag_patterns {
        let count: usize = variants.iter().map(|v| text.matches(v).count()).sum();
        if count > 0 {
            *tag_freq.entry(canonical.to_string()).or_insert(0) += count as u32;
        }
    }

    let total_tags: u32 = tag_freq.values().sum();
    let mut tag_distribution: Vec<(String, f32)> = tag_freq
        .into_iter()
        .map(|(k, v)| {
            (
                k,
                if total_tags > 0 {
                    v as f32 / total_tags as f32
                } else {
                    0.0
                },
            )
        })
        .collect();
    tag_distribution.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    DialogueFingerprint {
        tag_distribution,
        dialogue_ratio,
        has_dialogue,
    }
}

/// 采样锚点片段
fn sample_anchors(text: &str, sentences: &[String], count: usize) -> Vec<String> {
    if sentences.len() <= 3 {
        return sentences.iter().cloned().collect();
    }

    // 按"风格强度"排序：综合句长多样性 + 四字格密度 + 对话标签密度
    let mut scored: Vec<(f32, String)> = sentences
        .iter()
        .filter(|s| s.chars().count() >= 20 && s.chars().count() <= 120)
        .map(|s| {
            let four_char = count_four_char_phrases(s) as f32;
            let dialogue_tags = ["道", "说", "问", "答"]
                .iter()
                .map(|t| s.matches(t).count() as f32)
                .sum::<f32>();
            let len = s.chars().count() as f32;
            let score = four_char * 2.0 + dialogue_tags * 1.5 + (len / 30.0).min(2.0);
            (score, s.clone())
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(count);

    // 保证片段不截断句子，且长度适中
    scored
        .into_iter()
        .map(|(_, s)| {
            if s.chars().count() > 100 {
                s.chars().take(100).collect::<String>() + "..."
            } else {
                s
            }
        })
        .collect()
}

/// 提取 N-gram
fn extract_ngrams(text: &str) -> NgramFingerprint {
    let chars: Vec<char> = text.chars().collect();

    // 双字搭配
    let mut bigram_freq: HashMap<String, u32> = HashMap::new();
    for window in chars.windows(2) {
        let bg: String = window.iter().collect();
        // 过滤掉包含标点的
        if window
            .iter()
            .all(|c| c.is_alphanumeric() || (*c as u32) > 0x4e00)
        {
            *bigram_freq.entry(bg).or_insert(0) += 1;
        }
    }
    let mut bigrams: Vec<(String, u32)> = bigram_freq.into_iter().collect();
    bigrams.sort_by(|a, b| b.1.cmp(&a.1));
    bigrams.truncate(30);

    // 四字词（简单滑动窗口）
    let mut four_char_freq: HashMap<String, u32> = HashMap::new();
    for window in chars.windows(4) {
        let fc: String = window.iter().collect();
        if window
            .iter()
            .all(|c| (*c as u32) >= 0x4e00 && (*c as u32) <= 0x9fff)
        {
            *four_char_freq.entry(fc).or_insert(0) += 1;
        }
    }
    let mut four_char_phrases: Vec<(String, u32)> = four_char_freq.into_iter().collect();
    four_char_phrases.sort_by(|a, b| b.1.cmp(&a.1));
    four_char_phrases.truncate(20);

    // 衔接模式
    let transition_patterns = [
        "原来", "且说", "正是", "不想", "因问", "只得", "偏又", "况又", "一面", "连忙", "于是",
        "接着", "随后", "然后", "忽然", "只见", "当下", "此时", "那", "这", "可", "却", "便", "又",
    ];
    let mut transitions: Vec<(String, u32)> = transition_patterns
        .iter()
        .map(|&p| (p.to_string(), text.matches(p).count() as u32))
        .filter(|(_, c)| *c > 0)
        .collect();
    transitions.sort_by(|a, b| b.1.cmp(&a.1));
    transitions.truncate(15);

    NgramFingerprint {
        bigrams,
        four_char_phrases,
        transitions,
    }
}

/// 统计四字格数量（连续四个汉字）
fn count_four_char_phrases(text: &str) -> usize {
    let chars: Vec<char> = text.chars().collect();
    chars
        .windows(4)
        .filter(|w| {
            w.iter()
                .all(|c| (*c as u32) >= 0x4e00 && (*c as u32) <= 0x9fff)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_from_honglou() {
        let text = "黛玉道：\"也要来一群，岂不热闹？\"宝玉道：\"什么大家，不过几个人罢了。\"\n\\
                    n原来这黛玉秉绝代姿容，具希世俊美，不期这一哭，\
                    那附近柳枝花朵上宿鸟栖鸦一闻此声，俱忒楞楞飞起远避，不忍再听。\n\\
                    n且说宝玉因见黛玉如此，心中十分不忍，只得连忙劝道：\"好妹妹，快别哭了。\"";

        let fp = StyleFingerprint::from_text(text);

        // 句长应该在合理范围
        assert!(fp.syntax.avg_sentence_length > 10.0);
        assert!(fp.syntax.avg_sentence_length < 60.0);

        // 应该有对话标签
        assert!(fp.dialogue.has_dialogue);
        assert!(!fp.dialogue.tag_distribution.is_empty());

        // 应该有锚点片段
        assert!(!fp.anchor_samples.is_empty());

        // prompt 应该能生成
        let prompt = fp.to_prompt_section();
        assert!(prompt.contains("平均句长"));
        assert!(prompt.contains("四字格密度"));
    }

    #[test]
    fn test_fingerprint_empty() {
        let fp = StyleFingerprint::from_text("");
        assert!(fp.anchor_samples.is_empty());
    }
}
