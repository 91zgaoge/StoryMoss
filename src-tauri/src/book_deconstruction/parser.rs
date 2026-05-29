//! Book Parser - 小说文件解析器
//!
//! 支持 txt/pdf/epub 格式，提取纯文本和章节结构。

use std::path::Path;

use super::models::{ParseError, ParsedBook, ParsedChapter};

// 章节标题正则表达式（中文 + 英文）
const CHAPTER_PATTERNS: &[&str] = &[
    r"^\s*第[一二三四五六七八九十百千万零\d]+[章节回卷集部篇]\s*[：:\-—]?\s*(.*)$",
    r"^\s*Chapter\s+\d+[\.:：\-—]?\s*(.*)$",
    r"^\s*CHAPTER\s+\d+[\.:：\-—]?\s*(.*)$",
    r"^\s*\d+[\.:：\-—]\s+(.*)$",
    r"^\s*[★☆◆◇■□▲△●○]\s*(.*)$",
];

/// 解析小说文件（自动检测格式）
///
/// `progress_callback`: 可选的进度回调，参数为 (已处理字数, 估计总字数)
pub fn parse_book(
    file_path: &Path,
    progress_callback: Option<&dyn Fn(usize, usize)>,
) -> Result<ParsedBook, ParseError> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "txt" => TxtParser::parse(file_path, progress_callback),
        "pdf" => PdfParser::parse(file_path, progress_callback),
        "epub" => EpubParser::parse(file_path, progress_callback),
        _ => Err(ParseError::InvalidFormat(format!(
            "Unsupported file format: {}",
            ext
        ))),
    }
}

/// 检测章节标题
fn detect_chapter_title(line: &str) -> Option<String> {
    for pattern in CHAPTER_PATTERNS {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(line) {
                let title = caps.get(1).map(|m| m.as_str().trim().to_string());
                return title.filter(|t| !t.is_empty());
            }
        }
    }
    None
}

/// 按章节拆分文本
fn split_into_chapters_with_progress(
    text: &str,
    progress_callback: Option<&dyn Fn(usize, usize)>,
    total_estimate: usize,
) -> Vec<ParsedChapter> {
    let lines: Vec<&str> = text.lines().collect();
    let mut chapters: Vec<ParsedChapter> = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_content: Vec<String> = Vec::new();
    let mut processed_lines = 0usize;
    let total_lines = lines.len().max(1);

    for line in lines {
        if let Some(title) = detect_chapter_title(line) {
            // 保存上一个章节
            if !current_content.is_empty() {
                let content = current_content.join("\n").trim().to_string();
                let word_count = count_chinese_words(&content);
                chapters.push(ParsedChapter {
                    title: current_title,
                    content,
                    word_count,
                });
            }
            current_title = Some(title);
            current_content = vec![line.to_string()];
        } else {
            current_content.push(line.to_string());
        }
        processed_lines += 1;
        if let Some(cb) = progress_callback {
            let progress_words = (total_estimate * processed_lines) / total_lines;
            cb(progress_words, total_estimate);
        }
    }

    // 保存最后一个章节
    if !current_content.is_empty() {
        let content = current_content.join("\n").trim().to_string();
        let word_count = count_chinese_words(&content);
        chapters.push(ParsedChapter {
            title: current_title,
            content,
            word_count,
        });
    }

    // 如果没有检测到章节，将整个文本作为一个章节
    if chapters.is_empty() {
        let word_count = count_chinese_words(text);
        chapters.push(ParsedChapter {
            title: None,
            content: text.trim().to_string(),
            word_count,
        });
    }

    chapters
}

/// 统计中文字数（中文字符 + 英文单词）
fn count_chinese_words(text: &str) -> usize {
    let chinese_chars = text.chars().filter(|c| !c.is_ascii()).count();
    let english_words = text
        .split_whitespace()
        .filter(|w| {
            w.chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false)
        })
        .count();
    chinese_chars + english_words
}

// ==================== TXT 解析器 ====================

pub struct TxtParser;

impl TxtParser {
    pub fn parse(
        file_path: &Path,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> Result<ParsedBook, ParseError> {
        use std::fs;

        use encoding::{
            all::{GBK, UTF_8},
            DecoderTrap, Encoding,
        };

        let bytes = fs::read(file_path)
            .map_err(|e| ParseError::IoError(format!("Failed to read txt file: {}", e)))?;

        // 尝试 UTF-8
        let text = if let Ok(s) = String::from_utf8(bytes.clone()) {
            s
        } else if let Ok(s) = GBK.decode(&bytes, DecoderTrap::Replace) {
            s
        } else if let Ok(s) = UTF_8.decode(&bytes, DecoderTrap::Replace) {
            s
        } else {
            return Err(ParseError::EncodingError(
                "Unable to detect file encoding".to_string(),
            ));
        };

        let total_estimate = count_chinese_words(&text);
        let chapters = split_into_chapters_with_progress(&text, progress_callback, total_estimate);
        let word_count = chapters.iter().map(|c| c.word_count).sum();

        Ok(ParsedBook {
            title: None,
            author: None,
            chapters,
            raw_text: text,
            word_count,
        })
    }
}

// ==================== PDF 解析器 ====================

pub struct PdfParser;

impl PdfParser {
    pub fn parse(
        file_path: &Path,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> Result<ParsedBook, ParseError> {
        use pdf_extract::extract_text;

        if let Some(cb) = progress_callback {
            cb(0, 0);
        }

        let text = extract_text(file_path)
            .map_err(|e| ParseError::NoTextExtracted(format!("PDF extraction failed: {}", e)))?;

        if text.trim().is_empty() {
            return Err(ParseError::NoTextExtracted(
                "PDF contains no extractable text (possibly scanned image)".to_string(),
            ));
        }

        let total_estimate = count_chinese_words(&text);
        let chapters = split_into_chapters_with_progress(&text, progress_callback, total_estimate);
        let word_count = chapters.iter().map(|c| c.word_count).sum();

        Ok(ParsedBook {
            title: None,
            author: None,
            chapters,
            raw_text: text,
            word_count,
        })
    }
}

// ==================== EPUB 解析器 ====================

pub struct EpubParser;

impl EpubParser {
    pub fn parse(
        file_path: &Path,
        progress_callback: Option<&dyn Fn(usize, usize)>,
    ) -> Result<ParsedBook, ParseError> {
        use epub::doc::EpubDoc;
        let mut doc = EpubDoc::new(file_path)
            .map_err(|e| ParseError::InvalidFormat(format!("Failed to open EPUB: {:?}", e)))?;

        let mut chapters: Vec<ParsedChapter> = Vec::new();
        let mut full_text = String::new();
        let mut processed_words = 0usize;

        // 获取元数据
        let title = doc
            .mdata("title")
            .map(|item| item.value.clone())
            .filter(|v| !v.is_empty());
        let author = doc
            .mdata("creator")
            .map(|item| item.value.clone())
            .filter(|v| !v.is_empty());

        // 遍历 spine（阅读顺序）
        let spine = doc.spine.clone();
        let total_chapters = spine.len();
        for (i, itemref) in spine.iter().enumerate() {
            if let Some((bytes, _mime)) = doc.get_resource(&itemref.idref) {
                let text = String::from_utf8_lossy(&bytes);
                // 简单 HTML 标签清理
                let clean_text = strip_html_tags(&text);
                let word_count = count_chinese_words(&clean_text);

                chapters.push(ParsedChapter {
                    title: Some(format!("第{}章", i + 1)),
                    content: clean_text.clone(),
                    word_count,
                });

                full_text.push_str(&clean_text);
                full_text.push('\n');

                processed_words += word_count;
                if let Some(cb) = progress_callback {
                    cb(processed_words, total_chapters * 3000);
                }
            }
        }

        if chapters.is_empty() {
            return Err(ParseError::NoTextExtracted(
                "EPUB contains no readable chapters".to_string(),
            ));
        }

        let word_count = chapters.iter().map(|c| c.word_count).sum();

        Ok(ParsedBook {
            title,
            author,
            chapters,
            raw_text: full_text,
            word_count,
        })
    }
}

/// 简单 HTML 标签清理
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;

    for line in html.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<script") || trimmed.starts_with("<style") {
            in_script = true;
            continue;
        }
        if trimmed == "</script>" || trimmed == "</style>" {
            in_script = false;
            continue;
        }
        if in_script {
            continue;
        }

        for ch in line.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    if !result.is_empty() && !result.ends_with(' ') {
                        result.push(' ');
                    }
                }
                _ if !in_tag => result.push(ch),
                _ => {}
            }
        }
        result.push('\n');
    }

    // 清理多余空白
    result
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_chapter_title() {
        assert!(detect_chapter_title("第一章 初入江湖").is_some());
        assert!(detect_chapter_title("第1章：初入江湖").is_some());
        assert!(detect_chapter_title("Chapter 1: The Beginning").is_some());
        assert!(detect_chapter_title("1. The Beginning").is_some());
        assert!(detect_chapter_title("普通段落").is_none());
    }

    #[test]
    fn test_count_chinese_words() {
        assert_eq!(count_chinese_words("你好 world"), 3); // 2中文 + 1英文
        assert_eq!(count_chinese_words("Hello world"), 2);
    }

    #[test]
    fn test_strip_html_tags() {
        let html = "<p>Hello <b>world</b></p>";
        assert_eq!(strip_html_tags(html), "Hello world");
    }
}
