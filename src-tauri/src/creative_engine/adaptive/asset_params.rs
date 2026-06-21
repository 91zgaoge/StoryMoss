//! 资产 → 生成参数规则映射
//!
//! v0.22.0: 让 StyleDNA/GenreProfile/Methodology 不仅注入提示词，
//! 还影响 temperature/top_p/max_tokens 等生成参数。
//!
//! 使用规则映射表（可预测、可调试、可配置），所有调整值
//! clamp 到合理范围（temperature [0.5, 1.0]）。
//!
//! 修复审计报告缺口 5。

use crate::creative_engine::style::dna::StyleDNA;

/// 资产参数映射器
pub struct AssetParamMapper;

impl AssetParamMapper {
    /// StyleDNA 六维量化指标 → temperature 调整
    ///
    /// - 极简风格（短句长 <12 + 低比喻密度）→ 降 temperature（确定性高）
    /// - 感官轰炸（高比喻密度 >2.0）→ 升 temperature（创意性高）
    /// - 内心独白高（>0.4）→ 略升 temperature
    pub fn style_dna_to_temperature(base: f32, dna: &StyleDNA) -> f32 {
        let mut adjusted = base;

        if dna.syntax.avg_sentence_length < 12
            && dna.rhetoric.metaphor_density < 0.5
        {
            adjusted -= 0.15;
        }
        if dna.rhetoric.metaphor_density > 2.0 {
            adjusted += 0.15;
        }
        if dna.perspective.interior_monologue_ratio > 0.4 {
            adjusted += 0.05;
        }

        adjusted.clamp(0.5, 1.0)
    }

    /// 方法论 → max_tokens 倍率
    ///
    /// - 雪花法（多轮展开）→ ×1.3
    /// - 高密度世界构建 → ×1.4
    pub fn methodology_max_tokens_boost(methodology_id: &str) -> f32 {
        match methodology_id {
            "snowflake" => 1.3,
            "high_density_world_building" => 1.4,
            _ => 1.0,
        }
    }

    /// GenreProfile → max_tokens 建议
    ///
    /// - 史诗/玄幻/武侠 → 3000（需更长输出）
    /// - 短篇/闪小说 → 1200
    /// - 其他 → None（不覆盖）
    pub fn genre_max_tokens(genre_name: &str) -> Option<i32> {
        match genre_name.to_lowercase().as_str() {
            "epic" | "fantasy" | "wuxia" | "xianxia" => Some(3000),
            "short_story" | "flash_fiction" | "微小说" => Some(1200),
            _ => None,
        }
    }

    /// 题材 → quality_tier 映射
    pub fn genre_quality_tier(genre_name: &str) -> Option<&'static str> {
        match genre_name.to_lowercase().as_str() {
            "epic" | "xianxia" | "sci-fi" | "fantasy" => Some("high"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_dna() -> StyleDNA {
        serde_json::from_str(r#"{"meta":{"name":"test"},"vocabulary":{"density":"medium","abstraction":"medium","temporal_quality":"present","preferred_categories":[],"signature_words":[],"avoided_patterns":[]},"syntax":{"avg_sentence_length":25,"clause_complexity":"medium","rhythm_pattern":"balanced","preferred_structures":[],"opening_variety":"medium","punctuation_style":"standard"},"rhetoric":{"metaphor_density":1.0,"preferred_devices":[],"imagery_preference":[],"parallelism_frequency":"medium","irony_usage":"none"},"perspective":{"pov_type":"third_person","narrative_distance":"medium","interior_monologue_ratio":0.2,"omniscience_level":0.5,"temporal_handling":"linear"},"emotion":{"expressiveness":"balanced","emotion_word_density":0.5,"dominant_mood":"neutral","emotional_arc_pattern":"balanced","humor_style":"none"},"dialogue":{"dialogue_ratio":0.3,"dialogue_length":"medium","subtext_ratio":0.2,"signature_patterns":[],"tag_style":"standard"}}"#).unwrap()
    }

    #[test]

    #[test]
    fn test_snowflake_max_tokens_boost() {
        assert!((AssetParamMapper::methodology_max_tokens_boost("snowflake") - 1.3).abs() < 0.01);
        assert!((AssetParamMapper::methodology_max_tokens_boost("other") - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_genre_max_tokens() {
        assert_eq!(AssetParamMapper::genre_max_tokens("epic"), Some(3000));
        assert_eq!(AssetParamMapper::genre_max_tokens("unknown"), None);
    }
}
