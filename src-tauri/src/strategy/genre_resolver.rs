//! GenreResolver — 题材解析服务
//!
//! 将用户的自然语言输入（或 LLM 提取的 genre hint）解析为后台已存在的
//! GenreProfile ID 列表，解决自由格式题材词与资产表之间的断链问题。
//!
//! 设计原则：
//! - 规则优先，本地计算，不增加 LLM 调用延迟
//! - 支持精确匹配、子串匹配、关键词共现评分
//! - 支持复合题材（如「异星球末世生存」）解析为多个 GenreProfile
//! - 低分或空结果时可选择 fallback 到 LLM

use std::collections::HashMap;

use crate::{
    db::{GenreProfile, GenreProfileRepository},
    error::AppError,
};

/// 接受现有题材画像的最低分。精确匹配约 10；弱子串/共现通常 < 8。
/// 低于此分视为「目录无可用项」，应由上游生成新画像入库。
pub const ACCEPT_SCORE: f64 = 8.0;

/// 单个题材匹配结果
#[derive(Debug, Clone, PartialEq)]
pub struct GenreMatch {
    pub profile_id: String,
    pub genre_name: String,
    pub canonical_name: String,
    /// 匹配得分，越高越相关
    pub score: f64,
    /// 匹配原因，用于调试
    pub reason: String,
}

/// 题材解析器
#[derive(Debug, Clone, Default)]
pub struct GenreResolver {
    /// 同义词扩展表：口语词 / 变体 -> 标准别名
    synonyms: HashMap<String, Vec<String>>,
}

impl GenreResolver {
    /// 创建带默认同义词表的解析器
    pub fn new() -> Self {
        let mut synonyms = HashMap::new();

        // 末世相关
        synonyms.insert(
            "末世".to_string(),
            vec!["post-apocalyptic".to_string(), "apocalyptic".to_string()],
        );
        synonyms.insert(
            "末日".to_string(),
            vec!["post-apocalyptic".to_string(), "apocalyptic".to_string()],
        );
        synonyms.insert(
            "废土".to_string(),
            vec![
                "post-apocalyptic".to_string(),
                "apocalyptic".to_string(),
                "post-apocalyptic pioneer".to_string(),
            ],
        );
        synonyms.insert(
            "生存".to_string(),
            vec![
                "post-apocalyptic".to_string(),
                "post-apocalyptic pioneer".to_string(),
            ],
        );

        // 科幻 / 星际相关
        synonyms.insert(
            "异星球".to_string(),
            vec![
                "mecha".to_string(),
                "stellar warfare".to_string(),
                "sci-fi".to_string(),
                "science fiction".to_string(),
            ],
        );
        synonyms.insert(
            "异星".to_string(),
            vec![
                "mecha".to_string(),
                "stellar warfare".to_string(),
                "sci-fi".to_string(),
            ],
        );
        synonyms.insert(
            "星际".to_string(),
            vec![
                "mecha".to_string(),
                "stellar warfare".to_string(),
                "sci-fi".to_string(),
            ],
        );
        synonyms.insert(
            "外星".to_string(),
            vec![
                "mecha".to_string(),
                "stellar warfare".to_string(),
                "sci-fi".to_string(),
            ],
        );
        synonyms.insert(
            "机甲".to_string(),
            vec!["mecha".to_string(), "stellar warfare".to_string()],
        );
        synonyms.insert(
            "科幻".to_string(),
            vec!["sci-fi".to_string(), "science fiction".to_string()],
        );
        synonyms.insert(
            "未来".to_string(),
            vec!["sci-fi".to_string(), "science fiction".to_string()],
        );
        synonyms.insert(
            "太空".to_string(),
            vec!["sci-fi".to_string(), "science fiction".to_string()],
        );

        // 拓荒相关
        synonyms.insert(
            "拓荒".to_string(),
            vec![
                "post-apocalyptic pioneer".to_string(),
                "doomsday pioneer".to_string(),
            ],
        );
        synonyms.insert(
            "荒野求生".to_string(),
            vec![
                "post-apocalyptic pioneer".to_string(),
                "doomsday pioneer".to_string(),
            ],
        );

        Self { synonyms }
    }

    /// 从用户完整输入中解析题材（会先清洗无关词）
    pub fn resolve_from_text(
        &self,
        input: &str,
        repo: &GenreProfileRepository,
    ) -> Result<Vec<GenreMatch>, AppError> {
        let cleaned = Self::clean_input(input);
        self.resolve_from_hint(&cleaned, repo)
    }

    /// 从已清洗的题材 hint 中解析（如 LLM 提取的 genre 字段）
    pub fn resolve_from_hint(
        &self,
        hint: &str,
        repo: &GenreProfileRepository,
    ) -> Result<Vec<GenreMatch>, AppError> {
        let profiles = repo.get_all().map_err(AppError::from)?;
        Ok(self.resolve_from_profiles(hint, &profiles))
    }

    /// 直接从内存中的 GenreProfile 列表解析（便于测试和复用）
    pub fn resolve_from_profiles(&self, hint: &str, profiles: &[GenreProfile]) -> Vec<GenreMatch> {
        if hint.trim().is_empty() {
            return vec![];
        }

        let mut scores: HashMap<String, f64> = HashMap::new();
        let mut reasons: HashMap<String, Vec<String>> = HashMap::new();

        let hint_lower = hint.to_lowercase();
        let hint_chars: Vec<char> = hint_lower.chars().collect();

        // 1. 提取候选关键词（按常见分隔符切分）
        let tokens = Self::tokenize(hint);

        for profile in profiles {
            let pid = profile.id.clone();
            let mut score = 0.0;
            let mut reason_parts = Vec::new();

            // 收集所有可匹配名称
            let mut names = vec![
                profile.genre_name.to_lowercase(),
                profile.canonical_name.to_lowercase(),
            ];
            let aliases: Vec<String> = profile
                .aliases_json
                .as_deref()
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                .unwrap_or_default()
                .into_iter()
                .map(|a| a.to_lowercase())
                .collect();
            names.extend(aliases.clone());

            // 2. 精确匹配 hint 与任意名称
            for name in &names {
                if name == &hint_lower {
                    score += 10.0;
                    reason_parts.push(format!("精确匹配: {}", name));
                }
            }

            // 3. 子串匹配：hint 包含名称，或名称包含 hint
            for name in &names {
                if hint_lower.contains(name) && !name.is_empty() {
                    let len_ratio = name.len() as f64 / hint_lower.len() as f64;
                    score += 3.0 + len_ratio * 4.0;
                    reason_parts.push(format!("子串匹配: {}", name));
                }
                if name.contains(&hint_lower) && hint_lower.len() >= 2 {
                    score += 2.0;
                    reason_parts.push(format!("反向子串: {}", name));
                }
            }

            // 4. 按 token 精确匹配名称或同义词
            for token in &tokens {
                let token_lower = token.to_lowercase();
                if token_lower.len() < 2 {
                    continue;
                }

                // 直接匹配名称
                for name in &names {
                    if name == &token_lower {
                        score += 5.0;
                        reason_parts.push(format!("关键词精确匹配: {}", token));
                    } else if name.contains(&token_lower) {
                        score += 1.5;
                        reason_parts.push(format!("关键词子串: {} in {}", token, name));
                    }
                }

                // 同义词扩展匹配
                if let Some(expanded) = self.synonyms.get(&token_lower) {
                    for expanded_alias in expanded {
                        let exp_lower = expanded_alias.to_lowercase();
                        for name in &names {
                            if name == &exp_lower {
                                score += 4.0;
                                reason_parts
                                    .push(format!("同义词扩展: {} -> {}", token, expanded_alias));
                            } else if name.contains(&exp_lower) {
                                score += 1.0;
                                reason_parts
                                    .push(format!("同义词子串: {} -> {}", token, expanded_alias));
                            }
                        }
                    }
                }
            }

            // 5. 按字符连续出现加分（处理无空格中文复合词）
            for name in &names {
                if Self::contains_in_order(&hint_chars, name) {
                    score += 1.0;
                    reason_parts.push(format!("字符有序出现: {}", name));
                }
            }

            if score > 0.0 {
                scores.insert(pid.clone(), score);
                reasons.insert(pid, reason_parts);
            }
        }

        // 6. 排序并构建结果
        let mut matches: Vec<GenreMatch> = scores
            .into_iter()
            .filter_map(|(pid, score)| {
                profiles.iter().find(|p| p.id == pid).map(|p| GenreMatch {
                    profile_id: pid.clone(),
                    genre_name: p.genre_name.clone(),
                    canonical_name: p.canonical_name.clone(),
                    score,
                    reason: reasons.get(&pid).map(|r| r.join("; ")).unwrap_or_default(),
                })
            })
            .collect();

        matches.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matches
    }

    /// 从目录中选出「足够贴近」的现有画像。
    ///
    /// 优先采用 `preferred_ids` 中得分 ≥ [`ACCEPT_SCORE`] 的项；否则取
    /// `resolve_from_profiles` 中达标的匹配。空结果表示应生成新画像。
    pub fn select_existing(
        &self,
        hint: &str,
        preferred_ids: &[String],
        profiles: &[GenreProfile],
    ) -> Vec<GenreMatch> {
        let all = self.resolve_from_profiles(hint, profiles);
        if all.is_empty() {
            return vec![];
        }

        if !preferred_ids.is_empty() {
            let preferred_good: Vec<GenreMatch> = preferred_ids
                .iter()
                .filter_map(|id| {
                    all.iter()
                        .find(|m| m.profile_id == *id && m.score >= ACCEPT_SCORE)
                        .cloned()
                })
                .collect();
            if !preferred_good.is_empty() {
                return preferred_good;
            }
        }

        all.into_iter()
            .filter(|m| m.score >= ACCEPT_SCORE)
            .collect()
    }

    /// 清洗用户输入，去掉常见噪音词
    fn clean_input(input: &str) -> String {
        let noise_words = [
            "写一部",
            "写一本",
            "写一篇",
            "写个",
            "创作一部",
            "创作一本",
            "创作一篇",
            "创作个",
            "生成一部",
            "生成一本",
            "生成一篇",
            "新建",
            "创建",
            "新开",
            "我想",
            "我要",
            "帮我",
            "请",
            "题材",
            "类型",
            "风格",
            "小说",
            "的",
            "一部",
            "一本",
            "一篇",
            "一个",
            "关于",
            "有关",
        ];

        let mut result = input.to_string();
        for word in &noise_words {
            result = result.replace(word, " ");
        }
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// 分词：按常见分隔符切分，保留中文词
    fn tokenize(text: &str) -> Vec<String> {
        let separators: &[char] = &[
            ' ', '/', '\\', '，', ',', '、', '；', ';', '·', '|', '+', '&',
        ];
        let mut tokens: Vec<String> = text
            .split(separators)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // 额外按连续中文字符提取 2-4 字词
        let mut chinese_tokens = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if Self::is_chinese(chars[i]) {
                let mut j = i;
                while j < chars.len() && Self::is_chinese(chars[j]) {
                    j += 1;
                }
                let seg: String = chars[i..j].iter().collect();
                for len in 2..=seg.chars().count().min(4) {
                    for start in 0..=seg.chars().count() - len {
                        let word: String = seg.chars().skip(start).take(len).collect();
                        chinese_tokens.push(word);
                    }
                }
                i = j;
            } else {
                i += 1;
            }
        }

        tokens.extend(chinese_tokens);
        tokens
    }

    fn is_chinese(c: char) -> bool {
        (c >= '\u{4e00}' && c <= '\u{9fff}')
            || (c >= '\u{3400}' && c <= '\u{4dbf}')
            || (c >= '\u{20000}' && c <= '\u{2a6df}')
    }

    /// 检查 name 的字符是否按顺序出现在 hint 中
    fn contains_in_order(hint_chars: &[char], name: &str) -> bool {
        let name_chars: Vec<char> = name.to_lowercase().chars().collect();
        if name_chars.is_empty() {
            return false;
        }
        let mut ni = 0;
        for &hc in hint_chars {
            if hc == name_chars[ni] {
                ni += 1;
                if ni >= name_chars.len() {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_profiles() -> Vec<GenreProfile> {
        vec![
            GenreProfile {
                id: "apocalyptic".to_string(),
                genre_name: "末世流".to_string(),
                canonical_name: "Post-apocalyptic".to_string(),
                aliases_json: Some("[\"post-apocalyptic\", \"apocalyptic\", \"末世\", \"末日\", \"废土\", \"末世生存\"]".to_string()),
                core_tone: Some("core".to_string()),
                pacing_strategy: Some("pacing".to_string()),
                anti_patterns_json: Some("[]".to_string()),
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: None,
                recommended_style_dna_ids: None,
                recommended_methodology_id: None,
                recommended_skill_ids: None,
                min_quality_tier: None,
                is_builtin: true,
                created_at: chrono::Local::now(),
            },
            GenreProfile {
                id: "doomsday-pioneer".to_string(),
                genre_name: "末日拓荒".to_string(),
                canonical_name: "Doomsday Pioneer".to_string(),
                aliases_json: Some("[\"doomsday pioneer\", \"post-apocalyptic pioneer\", \"废土拓荒\", \"荒野求生\", \"拓荒\"]".to_string()),
                core_tone: Some("core".to_string()),
                pacing_strategy: Some("pacing".to_string()),
                anti_patterns_json: Some("[]".to_string()),
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: None,
                recommended_style_dna_ids: None,
                recommended_methodology_id: None,
                recommended_skill_ids: None,
                min_quality_tier: None,
                is_builtin: true,
                created_at: chrono::Local::now(),
            },
            GenreProfile {
                id: "scifi".to_string(),
                genre_name: "科幻".to_string(),
                canonical_name: "Sci-Fi".to_string(),
                aliases_json: Some("[\"sci-fi\", \"scifi\", \"科幻\", \"未来\", \"太空\", \"异星\"]".to_string()),
                core_tone: Some("core".to_string()),
                pacing_strategy: Some("pacing".to_string()),
                anti_patterns_json: Some("[]".to_string()),
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: None,
                recommended_style_dna_ids: None,
                recommended_methodology_id: None,
                recommended_skill_ids: None,
                min_quality_tier: None,
                is_builtin: true,
                created_at: chrono::Local::now(),
            },
            GenreProfile {
                id: "mecha-stellar".to_string(),
                genre_name: "星际机甲".to_string(),
                canonical_name: "Mecha / Stellar Warfare".to_string(),
                aliases_json: Some("[\"mecha\", \"stellar warfare\", \"mecha stellar\", \"星际\", \"异星球\", \"外星\", \"机甲\"]".to_string()),
                core_tone: Some("core".to_string()),
                pacing_strategy: Some("pacing".to_string()),
                anti_patterns_json: Some("[]".to_string()),
                reference_tables_json: None,
                typical_structure_json: None,
                reader_promise: None,
                recommended_style_dna_ids: None,
                recommended_methodology_id: None,
                recommended_skill_ids: None,
                min_quality_tier: None,
                is_builtin: true,
                created_at: chrono::Local::now(),
            },
        ]
    }

    #[test]
    fn test_resolve_compound_alien_postapocalyptic() {
        let resolver = GenreResolver::new();
        let matches =
            resolver.resolve_from_profiles("写一部异星球末世生存题材的小说", &test_profiles());

        assert!(!matches.is_empty(), "应至少解析出一个题材");

        let ids: Vec<String> = matches.iter().map(|m| m.profile_id.clone()).collect();
        assert!(
            ids.contains(&"apocalyptic".to_string()),
            "应解析出 apocalyptic, 实际: {:?}",
            ids
        );
        assert!(
            ids.contains(&"scifi".to_string()) || ids.contains(&"mecha-stellar".to_string()),
            "应解析出 scifi 或 mecha-stellar, 实际: {:?}",
            ids
        );
    }

    #[test]
    fn test_resolve_single_keywords() {
        let resolver = GenreResolver::new();
        let profiles = test_profiles();

        let m1 = resolver.resolve_from_profiles("末世", &profiles);
        assert_eq!(
            m1.first().map(|m| m.profile_id.as_str()),
            Some("apocalyptic")
        );

        let m2 = resolver.resolve_from_profiles("科幻", &profiles);
        assert_eq!(m2.first().map(|m| m.profile_id.as_str()), Some("scifi"));

        let m3 = resolver.resolve_from_profiles("异星球", &profiles);
        assert!(
            m3.iter().any(|m| m.profile_id == "mecha-stellar"),
            "异星球应匹配 mecha-stellar"
        );
    }

    #[test]
    fn test_resolve_compound_sci_fi_postapocalyptic() {
        let resolver = GenreResolver::new();
        let matches = resolver.resolve_from_profiles("末世科幻", &test_profiles());
        let ids: Vec<String> = matches.iter().map(|m| m.profile_id.clone()).collect();

        assert!(
            ids.contains(&"apocalyptic".to_string()),
            "末世科幻应含 apocalyptic"
        );
        assert!(ids.contains(&"scifi".to_string()), "末世科幻应含 scifi");
    }

    #[test]
    fn test_resolve_empty_hint() {
        let resolver = GenreResolver::new();
        let matches = resolver.resolve_from_profiles("", &test_profiles());
        assert!(matches.is_empty());
    }

    #[test]
    fn test_resolve_from_text_cleans_noise() {
        let resolver = GenreResolver::new();
        let matches = resolver
            .resolve_from_profiles("帮我写一部关于异星球末世生存的科幻小说", &test_profiles());
        let ids: Vec<String> = matches.iter().map(|m| m.profile_id.clone()).collect();
        assert!(ids.contains(&"apocalyptic".to_string()));
        assert!(ids.contains(&"scifi".to_string()) || ids.contains(&"mecha-stellar".to_string()));
    }

    #[test]
    fn select_existing_accepts_strong_match() {
        let resolver = GenreResolver::new();
        let profiles = test_profiles();
        let matches = resolver.select_existing("末世", &[], &profiles);
        assert_eq!(
            matches.first().map(|m| m.profile_id.as_str()),
            Some("apocalyptic")
        );
        assert!(matches[0].score >= ACCEPT_SCORE);
    }

    #[test]
    fn select_existing_rejects_weak_or_empty() {
        let resolver = GenreResolver::new();
        let profiles = test_profiles();
        // 与目录无实质重叠的冷门复合题材 → 应触发上游生成新画像
        let matches = resolver.select_existing("阿卡狄亚牧歌式量子茶道", &[], &profiles);
        assert!(
            matches.is_empty() || matches.iter().all(|m| m.score < ACCEPT_SCORE),
            "unexpected strong match: {:?}",
            matches
        );
        let empty = resolver.select_existing("", &[], &profiles);
        assert!(empty.is_empty());
    }

    #[test]
    fn select_existing_prefers_valid_preferred_ids() {
        let resolver = GenreResolver::new();
        let profiles = test_profiles();
        let preferred = vec!["apocalyptic".to_string()];
        let matches = resolver.select_existing("末世生存", &preferred, &profiles);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].profile_id, "apocalyptic");
    }

    #[test]
    fn select_existing_ignores_wrong_preferred_mecha_for_military() {
        let resolver = GenreResolver::new();
        let mut profiles = test_profiles();
        profiles.push(GenreProfile {
            id: "military".to_string(),
            genre_name: "军事".to_string(),
            canonical_name: "Military".to_string(),
            aliases_json: Some("[\"military\", \"军事\", \"军旅\"]".to_string()),
            core_tone: Some("core".to_string()),
            pacing_strategy: Some("pacing".to_string()),
            anti_patterns_json: Some("[]".to_string()),
            reference_tables_json: None,
            typical_structure_json: None,
            reader_promise: None,
            recommended_style_dna_ids: None,
            recommended_methodology_id: None,
            recommended_skill_ids: None,
            min_quality_tier: None,
            is_builtin: true,
            created_at: chrono::Local::now(),
        });
        let preferred = vec!["mecha-stellar".to_string()];
        let matches = resolver.select_existing("军事谍战", &preferred, &profiles);
        // 错误 preferred 不得强行采用；应回落到军事或空（再由上游生成）
        assert!(
            matches
                .iter()
                .all(|m| m.profile_id != "mecha-stellar" || m.score >= ACCEPT_SCORE),
            "mecha must not win on 军事谍战: {:?}",
            matches
        );
        if let Some(first) = matches.first() {
            assert_eq!(first.profile_id, "military");
        }
    }
}
