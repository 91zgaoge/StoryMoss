#![allow(dead_code)]
//! Tokenizer 封装
//!
//! 提供真实 tokenizer（基于 tiktoken-rs）的 token 计数与截断能力，
//! 同时保留原有 CJK 二元组分词器用于记忆检索。

use std::sync::Mutex;

use once_cell::sync::Lazy;

/// CJK分词器（保留：用于记忆检索阶段的 token 搜索）
///
/// 基于二元组分词（bigram）的简单高效分词实现
/// 适合中文、日文、韩文等CJK字符
pub struct CJKTokenizer;

impl CJKTokenizer {
    pub fn new() -> Self {
        Self
    }

    /// 对文本进行分词
    /// 返回二元组token列表
    pub fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = text.chars().collect();

        // 使用滑动窗口生成二元组
        for window in chars.windows(2) {
            // 只保留CJK字符的二元组
            if self.is_cjk(window[0]) || self.is_cjk(window[1]) {
                let token: String = window.iter().collect();
                tokens.push(token);
            }
        }

        // 同时添加单字token用于精确匹配
        for c in &chars {
            if self.is_cjk(*c) {
                tokens.push(c.to_string());
            }
        }

        tokens
    }

    /// 对查询进行分词（更激进的切分）
    pub fn tokenize_query(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let chars: Vec<char> = text.chars().collect();

        // 生成所有可能的子序列（用于模糊匹配）
        for i in 0..chars.len() {
            for j in i + 1..=std::cmp::min(i + 4, chars.len()) {
                let token: String = chars[i..j].iter().collect();
                if token.chars().any(|c| self.is_cjk(c)) {
                    tokens.push(token);
                }
            }
        }

        tokens
    }

    /// 判断字符是否为CJK字符
    pub fn is_cjk(&self, c: char) -> bool {
        matches!(c as u32,
            // CJK Unified Ideographs (中日韩统一表意文字)
            0x4E00..=0x9FFF |
            // CJK Unified Ideographs Extension A
            0x3400..=0x4DBF |
            // CJK Unified Ideographs Extension B
            0x20000..=0x2A6DF |
            // CJK Compatibility Ideographs
            0xF900..=0xFAFF |
            // Hiragana (平假名)
            0x3040..=0x309F |
            // Katakana (片假名)
            0x30A0..=0x30FF |
            // Hangul Syllables (韩文音节)
            0xAC00..=0xD7AF |
            // Hangul Jamo (韩文字母)
            0x1100..=0x11FF |
            // CJK Symbols and Punctuation
            0x3000..=0x303F
        )
    }

    /// 清洗文本（去除标点、空格等）
    pub fn clean_text(&self, text: &str) -> String {
        text.chars().filter(|c| !self.is_punctuation(*c)).collect()
    }

    /// 判断是否为标点符号
    fn is_punctuation(&self, c: char) -> bool {
        matches!(c as u32,
            // ASCII punctuation
            0x21..=0x2F | 0x3A..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E |
            // CJK Symbols and Punctuation
            0x3000..=0x303F |
            // Fullwidth ASCII variants
            0xFF01..=0xFF5E |
            // Halfwidth Katakana
            0xFF65..=0xFF9F
        )
    }
}

impl Default for CJKTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// 真实 tokenizer 封装（tiktoken-rs）
// =============================================================================

/// 支持的 tokenizer family
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenizerFamily {
    /// OpenAI cl100k_base：gpt-4 / gpt-3.5 / text-embedding / DeepSeek / Qwen
    /// 等
    Cl100k,
    /// OpenAI p50k_base：text-davinci-003 / 002 / code-davinci-002 等
    P50k,
    /// OpenAI r50k_base：早期 GPT-3 模型
    R50k,
}

impl TokenizerFamily {
    /// 根据模型名称或 family 字符串推断 tokenizer family
    pub fn from_model_family(model_family: &str) -> Self {
        let lower = model_family.to_lowercase();
        if lower.contains("davinci")
            || lower.contains("text-davinci")
            || lower.contains("code-davinci")
        {
            if lower.contains("text-davinci-001")
                || lower.contains("davinci") && !lower.contains("003") && !lower.contains("002")
            {
                return Self::R50k;
            }
            return Self::P50k;
        }
        // cl100k_base 作为默认与最广泛兼容的编码器
        Self::Cl100k
    }

    fn encoder(&self) -> Result<tiktoken_rs::CoreBPE, Box<dyn std::error::Error + Send + Sync>> {
        match self {
            Self::Cl100k => tiktoken_rs::cl100k_base().map_err(into_boxed_error),
            Self::P50k => tiktoken_rs::p50k_base().map_err(into_boxed_error),
            Self::R50k => tiktoken_rs::r50k_base().map_err(into_boxed_error),
        }
    }
}

fn into_boxed_error<E: std::fmt::Display>(e: E) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        e.to_string(),
    ))
}

/// 线程安全的 tokenizer 缓存（encoder 创建有 IO/计算开销，缓存可复用）
static ENCODER_CACHE: Lazy<
    Mutex<std::collections::HashMap<TokenizerFamily, tiktoken_rs::CoreBPE>>,
> = Lazy::new(|| Mutex::new(std::collections::HashMap::new()));

fn with_encoder<F, T>(
    family: TokenizerFamily,
    f: F,
) -> Result<T, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce(&tiktoken_rs::CoreBPE) -> T,
{
    {
        let cache = ENCODER_CACHE.lock().map_err(|e| {
            Box::<dyn std::error::Error + Send + Sync>::from(format!(
                "tokenizer cache poisoned: {}",
                e
            ))
        })?;
        if let Some(encoder) = cache.get(&family) {
            return Ok(f(encoder));
        }
    }

    let encoder = family.encoder()?;
    let result = f(&encoder);
    if let Ok(mut cache) = ENCODER_CACHE.lock() {
        cache.entry(family).or_insert(encoder);
    }
    Ok(result)
}

/// 计算文本的 token 数量
///
/// `model_family` 可为模型名（如 "gpt-4"、"deepseek-chat"）或 family 标识；
/// 无法识别时回退到 `cl100k_base`。
pub fn count_tokens(text: &str, model_family: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let family = TokenizerFamily::from_model_family(model_family);
    with_encoder(family, |enc| enc.encode_with_special_tokens(text).len()).unwrap_or_else(|e| {
        log::warn!(
            "[Tokenizer] count_tokens failed for family {:?}: {}, falling back to char/2",
            family,
            e
        );
        (text.chars().count() / 2).max(1)
    })
}

/// 将文本截断到不超过 `max_tokens` 个 token
///
/// 优先从字符串末尾截断（保留开头），适用于需要保留前缀的上下文；
/// 若需要保留最新内容，调用方应自行反转文本后再调用。
pub fn truncate_to_budget(text: &str, max_tokens: usize, model_family: &str) -> String {
    if text.is_empty() || max_tokens == 0 {
        return String::new();
    }
    let family = TokenizerFamily::from_model_family(model_family);
    with_encoder(family, |enc| {
        let tokens = enc.encode_with_special_tokens(text);
        if tokens.len() <= max_tokens {
            text.to_string()
        } else {
            let truncated = &tokens[..max_tokens];
            enc.decode(truncated.to_vec()).unwrap_or_else(|_| {
                text.chars().take(max_tokens).collect()
            })
        }
    })
    .unwrap_or_else(|e| {
        log::warn!("[Tokenizer] truncate_to_budget failed for family {:?}: {}, falling back to char truncation", family, e);
        text.chars().take(max_tokens).collect()
    })
}

/// 从文本末尾截断，保留最后 `max_tokens` 个 token 对应的内容
///
/// 适用于“保留最新内容”的场景（如当前章节已写内容）。
pub fn truncate_to_budget_from_end(text: &str, max_tokens: usize, model_family: &str) -> String {
    if text.is_empty() || max_tokens == 0 {
        return String::new();
    }
    let family = TokenizerFamily::from_model_family(model_family);
    with_encoder(family, |enc| {
        let tokens = enc.encode_with_special_tokens(text);
        if tokens.len() <= max_tokens {
            text.to_string()
        } else {
            let start = tokens.len() - max_tokens;
            let truncated = &tokens[start..];
            enc.decode(truncated.to_vec()).unwrap_or_else(|_| {
                text.chars().rev().take(max_tokens).collect::<String>().chars().rev().collect()
            })
        }
    })
    .unwrap_or_else(|e| {
        log::warn!("[Tokenizer] truncate_to_budget_from_end failed for family {:?}: {}, falling back to char truncation", family, e);
        text.chars().rev().take(max_tokens).collect::<String>().chars().rev().collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_chinese() {
        let tokenizer = CJKTokenizer::new();
        let tokens = tokenizer.tokenize("这是一个测试");

        // 应该包含二元组和单字
        assert!(tokens.contains(&"这是".to_string()));
        assert!(tokens.contains(&"是一".to_string()));
        assert!(tokens.contains(&"一个".to_string()));
        assert!(tokens.contains(&"个测".to_string()));
        assert!(tokens.contains(&"测试".to_string()));
        assert!(tokens.contains(&"这".to_string()));
        assert!(tokens.contains(&"是".to_string()));
    }

    #[test]
    fn test_is_cjk() {
        let tokenizer = CJKTokenizer::new();

        assert!(tokenizer.is_cjk('中'));
        assert!(tokenizer.is_cjk('日'));
        assert!(tokenizer.is_cjk('한')); // 韩文
        assert!(!tokenizer.is_cjk('A'));
        assert!(!tokenizer.is_cjk('1'));
    }

    #[test]
    fn test_clean_text() {
        let tokenizer = CJKTokenizer::new();
        let cleaned = tokenizer.clean_text("你好，世界！Hello, World!");

        assert_eq!(cleaned, "你好世界Hello World");
    }

    #[test]
    fn test_count_tokens_english() {
        // "hello world" -> roughly 2 tokens in cl100k
        let n = count_tokens("hello world", "gpt-4");
        assert!(n > 0 && n <= 4, "got {} tokens", n);
    }

    #[test]
    fn test_count_tokens_chinese() {
        let n = count_tokens("你好世界", "gpt-4");
        assert!(n > 0, "got {} tokens", n);
    }

    #[test]
    fn test_count_tokens_empty() {
        assert_eq!(count_tokens("", "gpt-4"), 0);
    }

    #[test]
    fn test_truncate_to_budget_preserves_prefix() {
        let text = "一二三四五六七八九十";
        let truncated = truncate_to_budget(text, 3, "deepseek-chat");
        assert!(!truncated.is_empty());
        assert!(truncated.len() <= text.len());
        assert!(text.starts_with(&truncated));
    }

    #[test]
    fn test_truncate_to_budget_from_end_preserves_suffix() {
        let text = "一二三四五六七八九十";
        let truncated = truncate_to_budget_from_end(text, 3, "qwen");
        assert!(!truncated.is_empty());
        assert!(text.ends_with(&truncated));
    }

    #[test]
    fn test_tokenizer_family_mapping() {
        assert_eq!(
            TokenizerFamily::from_model_family("gpt-4"),
            TokenizerFamily::Cl100k
        );
        assert_eq!(
            TokenizerFamily::from_model_family("deepseek-chat"),
            TokenizerFamily::Cl100k
        );
        assert_eq!(
            TokenizerFamily::from_model_family("qwen2.5"),
            TokenizerFamily::Cl100k
        );
        assert_eq!(
            TokenizerFamily::from_model_family("text-davinci-003"),
            TokenizerFamily::P50k
        );
        assert_eq!(
            TokenizerFamily::from_model_family("text-davinci-001"),
            TokenizerFamily::R50k
        );
    }

    #[test]
    fn test_count_tokens_mixed_chinese_english() {
        // Mixed text should return a positive, reasonable token count.
        let text = "Hello world 你好世界 this is a test 这是一个测试";
        let n = count_tokens(text, "gpt-4");
        assert!(n > 0, "got {} tokens", n);
        // cl100k encodes CJK characters at roughly 1-2 tokens per character and
        // English words at ~1 token per word, so 10+ tokens is expected.
        assert!(n >= 10, "got {} tokens", n);
    }

    #[test]
    fn test_truncate_to_budget_respects_budget() {
        // Use ASCII text where token boundaries are stable across encode/decode.
        let text = "The quick brown fox jumps over the lazy dog. ".repeat(100);
        let budget = 16;
        let truncated = truncate_to_budget(&text, budget, "gpt-4");
        let token_count = count_tokens(&truncated, "gpt-4");
        assert!(
            token_count <= budget,
            "truncated tokens {} exceed budget {}",
            token_count,
            budget
        );
        assert!(text.starts_with(&truncated));
    }

    #[test]
    fn test_truncate_to_budget_from_end_respects_budget() {
        let text = "The quick brown fox jumps over the lazy dog. ".repeat(100);
        let budget = 16;
        let truncated = truncate_to_budget_from_end(&text, budget, "gpt-4");
        let token_count = count_tokens(&truncated, "gpt-4");
        assert!(
            token_count <= budget,
            "truncated tokens {} exceed budget {}",
            token_count,
            budget
        );
        assert!(text.ends_with(&truncated));
    }
}
