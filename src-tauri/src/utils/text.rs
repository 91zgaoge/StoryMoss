#![allow(dead_code)]
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
