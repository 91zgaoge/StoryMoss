//! CJK分词器
//!
//! 基于二元组分词（bigram）的简单高效分词实现
//! 适合中文、日文、韩文等CJK字符

/// CJK分词器
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
}
