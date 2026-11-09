//! Style domain types.
//!
//! Pure data definitions shared across the creative engine, agents, and
//! commands. All analysis / behavior impls remain in
//! `crate::creative_engine::style`.

use serde::{Deserialize, Serialize};

// ==================== Style Blend ====================

/// 混合角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BlendRole {
    Dominant,  // 主导 50-80%
    Secondary, // 辅助 10-30%
    Tertiary,  // 辅助 5-20%
}

/// 混合组件
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlendComponent {
    pub dna_id: String,
    pub dna_name: String,
    pub weight: f32,
    pub role: BlendRole,
}

/// 风格混合配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StyleBlendConfig {
    pub name: String,
    pub components: Vec<BlendComponent>,
    pub drift_check_enabled: bool,
}

impl Default for StyleBlendConfig {
    fn default() -> Self {
        Self {
            name: "默认混合".to_string(),
            components: vec![],
            drift_check_enabled: true,
        }
    }
}

// ==================== Style Fingerprint ====================

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

// ==================== Style DNA ====================

/// 风格 DNA（完整的风格量化描述）
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
// v0.93.7: 容器级 serde default——LLM 返回的 JSON 缺任何字段/子块时回退默认值，不再整份 DNA
// 解析报废（真机：缺 temporal_quality 即失败）
#[serde(default)]
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
// v0.93.7: 容器级 serde default——LLM 返回的 JSON 缺任何字段/子块时回退默认值，不再整份 DNA
// 解析报废（真机：缺 temporal_quality 即失败）
#[serde(default)]
pub struct StyleMeta {
    pub name: String,
    pub author: Option<String>, // 来源作家（如"金庸"）
    pub description: String,
    pub genre_association: Option<String>, // 关联题材
}

/// 词汇特征
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
// v0.93.7: 容器级 serde default——LLM 返回的 JSON 缺任何字段/子块时回退默认值，不再整份 DNA
// 解析报废（真机：缺 temporal_quality 即失败）
#[serde(default)]
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

/// v0.93.12: 平均句长宽容解析——LLM 常返回小数（`22.6`）或数字字符串，
/// 严格 u32 会让整份 StyleDNA 解析失败（真机《阴脉先生》训练：`invalid type:
/// floating point 22.6, expected u32`）。小数四舍五入，负数/非数字回退 0。
fn deserialize_lenient_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct LenientU32;
    impl<'de> Visitor<'de> for LenientU32 {
        type Value = u32;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("非负整数、小数或数字字符串")
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> Result<u32, E> {
            Ok(v.min(u32::MAX as u64) as u32)
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> Result<u32, E> {
            Ok(v.clamp(0, u32::MAX as i64) as u32)
        }

        fn visit_f64<E: de::Error>(self, v: f64) -> Result<u32, E> {
            Ok(Self::from_f64(v))
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<u32, E> {
            let trimmed = v.trim();
            if let Ok(n) = trimmed.parse::<f64>() {
                return Ok(Self::from_f64(n));
            }
            trimmed.parse::<u32>().map_err(|_| {
                de::Error::invalid_value(de::Unexpected::Str(v), &"非负整数、小数或数字字符串")
            })
        }

        fn visit_unit<E: de::Error>(self) -> Result<u32, E> {
            Ok(0)
        }

        fn visit_none<E: de::Error>(self) -> Result<u32, E> {
            Ok(0)
        }
    }

    impl LenientU32 {
        fn from_f64(v: f64) -> u32 {
            if !v.is_finite() || v <= 0.0 {
                return 0;
            }
            v.round().min(u32::MAX as f64) as u32
        }
    }

    deserializer.deserialize_any(LenientU32)
}

/// 句法特征
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
// v0.93.7: 容器级 serde default——LLM 返回的 JSON 缺任何字段/子块时回退默认值，不再整份 DNA
// 解析报废（真机：缺 temporal_quality 即失败）
// v0.93.12: avg_sentence_length 再放宽数字形态（小数/字符串）
#[serde(default)]
pub struct SyntaxProfile {
    /// 平均句长（中文字符数；小数输入四舍五入）
    #[serde(default, deserialize_with = "deserialize_lenient_u32")]
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
// v0.93.7: 容器级 serde default——LLM 返回的 JSON 缺任何字段/子块时回退默认值，不再整份 DNA
// 解析报废（真机：缺 temporal_quality 即失败）
#[serde(default)]
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
// v0.93.7: 容器级 serde default——LLM 返回的 JSON 缺任何字段/子块时回退默认值，不再整份 DNA
// 解析报废（真机：缺 temporal_quality 即失败）
#[serde(default)]
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
// v0.93.7: 容器级 serde default——LLM 返回的 JSON 缺任何字段/子块时回退默认值，不再整份 DNA
// 解析报废（真机：缺 temporal_quality 即失败）
#[serde(default)]
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
// v0.93.7: 容器级 serde default——LLM 返回的 JSON 缺任何字段/子块时回退默认值，不再整份 DNA
// 解析报废（真机：缺 temporal_quality 即失败）
#[serde(default)]
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

// ==================== Style Guard / Checker results ====================

/// 「手工艺滑块」单档建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftSliderHint {
    /// 维度名（如「句长偏好」）
    pub dimension: String,
    /// 当前档位（如「短句为主」/「长短交替」/「长句缠绕」）
    pub level: String,
    /// 写作要求（注入 prompt 的原文）
    pub directive: String,
}

/// 风格摘要清洗结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizeOutcome {
    /// 清洗后的文本（已去除在世作者姓名）
    pub sanitized: String,
    /// 命中的在世作者列表（用于日志/UI 提示）
    pub removed_authors: Vec<String>,
    /// 是否需要在 prompt 中追加「手工艺滑块」段
    pub require_craft_sliders: bool,
}

/// 风格检查结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleCheckResult {
    pub score: f32,
    pub passed: bool,
    pub issues: Vec<String>,
}
