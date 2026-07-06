#![allow(dead_code)]
use regex::Regex;

pub struct TextUtils;

impl TextUtils {
    pub fn word_count(text: &str) -> usize {
        text.split_whitespace().count()
    }

    /// 中文-aware 字数统计：中文字符 + 英文单词
    /// 与前端 FrontstageApp.tsx 逻辑保持一致
    pub fn chinese_word_count(text: &str) -> usize {
        let chinese_chars = text
            .chars()
            .filter(|c| matches!(*c, '\u{4e00}'..='\u{9fff}'))
            .count();
        let english_words: usize = text
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|s| !s.is_empty())
            .count();
        chinese_chars + english_words
    }

    pub fn sentence_count(text: &str) -> usize {
        text.split(['.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .count()
    }

    pub fn reading_time_minutes(text: &str, wpm: u32) -> f32 {
        let words = Self::word_count(text) as f32;
        words / wpm as f32
    }

    pub fn truncate(text: &str, max_length: usize) -> String {
        if text.len() <= max_length {
            text.to_string()
        } else {
            format!("{}...", &text[..max_length.saturating_sub(3)])
        }
    }

    pub fn normalize_whitespace(text: &str) -> String {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    pub fn extract_dialogue(text: &str) -> Vec<String> {
        let mut dialogues = Vec::new();
        let mut in_quote = false;
        let mut current = String::new();

        for ch in text.chars() {
            if ch == '"' {
                if in_quote {
                    dialogues.push(current.clone());
                    current.clear();
                }
                in_quote = !in_quote;
            } else if in_quote {
                current.push(ch);
            }
        }

        dialogues
    }

    pub fn similarity(a: &str, b: &str) -> f32 {
        let a_words: std::collections::HashSet<&str> = a.split_whitespace().collect();
        let b_words: std::collections::HashSet<&str> = b.split_whitespace().collect();
        let intersection = a_words.intersection(&b_words).count() as f32;
        let union = a_words.union(&b_words).count() as f32;
        if union == 0.0 {
            0.0
        } else {
            intersection / union
        }
    }

    pub fn remove_markdown(text: &str) -> String {
        text.replace("**", "")
            .replace('*', "")
            .replace("__", "")
            .replace('_', "")
            .replace("## ", "")
            .replace("# ", "")
            .replace('`', "")
    }

    pub fn split_paragraphs(text: &str) -> Vec<&str> {
        text.split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .collect()
    }

    /// v0.26.15: 清理文本自身的重复。
    ///
    /// 与前端 `textCleanup.ts::trimSelfRepetition` 对齐：
    /// 1. 段落级检测（整章重复、末尾 k 段重复开头 k 段）；
    /// 2. KMP 最长 border 检测单段内或跨段长尾重复。
    pub fn trim_self_repetition(text: &str) -> String {
        let trimmed = text.trim();
        if trimmed.chars().count() < 40 {
            return text.to_string();
        }

        let paragraph_deduped = Self::trim_repeated_paragraphs(trimmed);
        if paragraph_deduped.len() < trimmed.len() {
            return paragraph_deduped;
        }

        Self::trim_by_longest_border(trimmed).unwrap_or_else(|| text.to_string())
    }

    fn trim_repeated_paragraphs(text: &str) -> String {
        let paragraphs: Vec<&str> = text
            .split("\n\n")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if paragraphs.len() < 2 {
            return text.to_string();
        }

        let normalized: Vec<String> = paragraphs
            .iter()
            .map(|p| Self::normalize_for_duplicate_check(p))
            .collect();

        // 情况 A：整章写了两遍（后半 == 前半）
        if normalized.len() % 2 == 0 {
            let half = normalized.len() / 2;
            if normalized[..half] == normalized[half..] {
                return paragraphs[..half].join("\n\n");
            }
        }

        // 情况 B：末尾连续 k 段重复开头连续 k 段
        let max_k = normalized.len() / 2;
        for k in (1..=max_k).rev() {
            if normalized[..k] == normalized[normalized.len() - k..] {
                let remaining = &paragraphs[..paragraphs.len() - k];
                if !remaining.is_empty() {
                    return remaining.join("\n\n");
                }
            }
        }

        text.to_string()
    }

    fn trim_by_longest_border(text: &str) -> Option<String> {
        let (normalized, indices) = Self::build_normalized_index(text);
        let norm_len = normalized.chars().count();
        if norm_len < 40 {
            return None;
        }

        let border_len = Self::longest_border_length(&normalized);
        if border_len == 0 {
            return None;
        }

        let min_border = 30.max(norm_len * 8 / 100);
        if border_len < min_border {
            return None;
        }

        let remaining = norm_len - border_len;
        if remaining < 30 {
            return None;
        }

        let cut_char_idx = norm_len - border_len;
        let cut_byte_index = *indices.get(cut_char_idx)?;
        if cut_byte_index == 0 {
            return None;
        }

        let mut result = text[..cut_byte_index].trim().to_string();
        if let Some(last_open) = result.rfind('<') {
            if result.rfind('>').unwrap_or(0) < last_open {
                result = result[..last_open].trim().to_string();
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    fn build_normalized_index(text: &str) -> (String, Vec<usize>) {
        let mut normalized = String::new();
        let mut indices = Vec::new();
        let mut byte_idx = 0;

        while byte_idx < text.len() {
            let rest = &text[byte_idx..];
            let Some(ch) = rest.chars().next() else {
                break;
            };
            let ch_len = ch.len_utf8();

            if ch == '<' {
                if let Some(rel_close) = rest.find('>') {
                    byte_idx += rel_close + 1;
                    continue;
                }
                break;
            }

            if !ch.is_whitespace() && !Self::is_duplicate_punctuation(ch) {
                normalized.push(ch);
                indices.push(byte_idx);
            }

            byte_idx += ch_len;
        }

        (normalized, indices)
    }

    fn longest_border_length(s: &str) -> usize {
        let chars: Vec<char> = s.chars().collect();
        let n = chars.len();
        if n == 0 {
            return 0;
        }

        let mut pi = vec![0usize; n];
        for i in 1..n {
            let mut j = pi[i - 1];
            while j > 0 && chars[i] != chars[j] {
                j = pi[j - 1];
            }
            if chars[i] == chars[j] {
                j += 1;
            }
            pi[i] = j;
        }
        pi[n - 1]
    }

    fn is_duplicate_punctuation(ch: char) -> bool {
        matches!(
            ch,
            '\u{3002}'
                | '\u{ff01}'
                | '\u{ff1f}'
                | '.'
                | '!'
                | '?'
                | '\u{ff0c}'
                | '\u{3001}'
                | '\u{ff1b}'
                | '\u{ff1a}'
                | '"'
                | '\''
                | '\u{ff08}'
                | '\u{ff09}'
                | '\u{300a}'
                | '\u{300b}'
                | '['
                | ']'
                | '\u{3010}'
                | '\u{3011}'
                | '\u{2026}'
                | '\u{2014}'
                | '\u{ff5e}'
                | '\u{00b7}'
                | '\u{201c}'
                | '\u{201d}'
                | '\u{2018}'
                | '\u{2019}'
        )
    }

    fn normalize_for_duplicate_check(s: &str) -> String {
        static HTML_RE: once_cell::sync::Lazy<Regex> =
            once_cell::sync::Lazy::new(|| Regex::new(r"<[^>]*>").unwrap());
        let s = HTML_RE.replace_all(s, "");
        s.chars()
            .filter(|c| !c.is_whitespace() && !Self::is_duplicate_punctuation(*c))
            .collect()
    }

    pub fn excerpt(text: &str, keyword: &str, context_words: usize) -> Option<String> {
        let words: Vec<&str> = text.split_whitespace().collect();
        for (i, word) in words.iter().enumerate() {
            if word.to_lowercase().contains(&keyword.to_lowercase()) {
                let start = i.saturating_sub(context_words);
                let end = (i + context_words + 1).min(words.len());
                return Some(words[start..end].join(" "));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_count_chinese() {
        assert_eq!(TextUtils::word_count("你好世界"), 1);
    }

    #[test]
    fn test_word_count_english() {
        assert_eq!(TextUtils::word_count("hello world"), 2);
    }

    #[test]
    fn test_word_count_mixed() {
        assert_eq!(TextUtils::word_count("hello 世界"), 2);
    }

    #[test]
    fn test_truncate_normal() {
        let text = "hello world";
        assert_eq!(TextUtils::truncate(text, 8), "hello...");
    }

    #[test]
    fn test_truncate_shorter_than_limit() {
        let text = "hi";
        assert_eq!(TextUtils::truncate(text, 10), "hi");
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(
            TextUtils::normalize_whitespace("  hello   world  "),
            "hello world"
        );
    }

    #[test]
    fn test_remove_markdown() {
        let text = "**bold** and _italic_ and ## heading";
        assert_eq!(
            TextUtils::remove_markdown(text),
            "bold and italic and heading"
        );
    }

    #[test]
    fn test_trim_self_repetition_short_text_unchanged() {
        let text = "这是一个短文本。";
        assert_eq!(TextUtils::trim_self_repetition(text), text);
    }

    #[test]
    fn test_trim_self_repetition_whole_chapter_duplicated() {
        let copy =
            "他穿过废墟，脚步在碎石上发出轻微的响动。天空是铅灰色的，空气中弥漫着焦灼的味道。\n\n\
                    远处传来一阵低沉的轰鸣，他停下脚步，握紧了手中的武器。";
        let text = format!("{copy}\n\n{copy}");
        assert_eq!(TextUtils::trim_self_repetition(&text), copy);
    }

    #[test]
    fn test_trim_self_repetition_trailing_k_paragraphs() {
        let p1 = "他不知道自己多少岁，但这种生活让他感受到时间的流逝。";
        let p2 = "当他的狗伴催促他抬头时，他顿悟了自己的位置。";
        let p3 = "这不是他的生活的终局，他要从这片凋零的地平线中夺回生命的意义。";
        let p4 = "辽东荒凉之中，一片狭窄的谷丘偶然掩蔽了世界的残留。";
        let text = format!("{p1}\n\n{p2}\n\n{p3}\n\n{p4}\n\n{p1}\n\n{p2}\n\n{p3}");
        let expected = format!("{p1}\n\n{p2}\n\n{p3}\n\n{p4}");
        assert_eq!(TextUtils::trim_self_repetition(&text), expected);
    }

    #[test]
    fn test_trim_self_repetition_long_suffix_in_single_paragraph() {
        let prefix = "在这个残酷的世界里，一个成功，也只是催生了更多的挑战。少年的目标是抓取一个正在勃勃生长的菌菇。";
        let middle = "他穿过狭窄的通道，避开那些潜伏在黑暗中的危险。";
        let text = format!("{prefix}{middle}{prefix}");
        assert_eq!(
            TextUtils::trim_self_repetition(&text),
            format!("{prefix}{middle}")
        );
    }

    #[test]
    fn test_trim_self_repetition_repeated_block_in_last_paragraph() {
        let p1 =
            "他不知道自己多少岁，但这种生活让他感受到时间的流逝。疾风中的寂寞催作了他的心理崩溃。";
        let p2 = "当他的狗伴在他身前伸出一根粗糙的嘴，催促他抬头时，他顿悟了自己的位置。";
        let p3 = "这不是他的生活的终局，他要从这片凋零的地平线中夺回生命的意义。";
        let p4prefix = "辽东荒凉之中，一片狭窄的谷丘偶然掩蔽了世界的残留。此地尽是干枯的植物。";
        let text = format!("{p1}\n\n{p2}\n\n{p3}\n\n{p4prefix}{p1}{p2}{p3}");
        let expected = format!("{p1}\n\n{p2}\n\n{p3}\n\n{p4prefix}");
        assert_eq!(TextUtils::trim_self_repetition(&text), expected);
    }

    // v0.26.19 Phase 3.3: 跨层共享 trim golden fixture。
    //   此测试加载仓库根 `tests/fixtures/trim_golden.json`，对每条用例断言
    //   Rust `trim_self_repetition` 输出与 expected 一致。同一 fixture 也由
    //   前端 vitest `textCleanup.golden.test.ts` 加载并断言 TS `trimSelfRepetition`
    //   输出一致——双跑通过即证明两实现对同输入同输出（跨层一致性契约）。
    #[derive(serde::Deserialize)]
    struct TrimGoldenCase {
        id: String,
        input: String,
        expected: String,
        #[serde(default)]
        description: String,
    }

    #[test]
    fn trim_self_repetition_matches_shared_golden_fixture() {
        let fixture_json: &str = include_str!("../../../tests/fixtures/trim_golden.json");
        let cases: Vec<TrimGoldenCase> =
            serde_json::from_str(fixture_json).expect("golden fixture must be valid JSON");
        assert!(
            cases.len() >= 7,
            "golden fixture should contain at least 7 cases, got {}",
            cases.len()
        );
        for case in &cases {
            let actual = TextUtils::trim_self_repetition(&case.input);
            assert_eq!(
                actual, case.expected,
                "golden case '{}' ({}) mismatch:\ninput: {}\nexpected: {}\nactual: {}",
                case.id, case.description, case.input, case.expected, actual
            );
        }
    }
}
