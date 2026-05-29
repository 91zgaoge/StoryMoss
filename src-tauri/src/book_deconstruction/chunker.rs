//! Text Chunker - 文本分块策略
//!
//! 根据小说长度选择不同的分块策略，适配 LLM 上下文限制。
//!
//! 策略原则（A. 智能分块 + 增量归纳）：
//! - 短篇(<10万字): 全文一次性分析
//! - 中篇(10-50万字): 按章节分块，相邻短章节自动合并
//! - 长篇(>50万字): 按固定大小（~5000字）顺序分块，所有块覆盖，逐块提取后汇总
//!
//! 不设块数上限，所有内容都被分析。未来可通过心跳检测机制防止超长任务超时。

use super::models::{ChunkingStrategy, ParsedBook, ParsedChapter, TextChunk};

/// 短篇字数阈值（<10万字）
const SHORT_NOVEL_MAX: usize = 100_000;
/// 中篇字数阈值（10-50万字）
const MEDIUM_NOVEL_MAX: usize = 500_000;
/// 长篇固定分块大小（字符数）
const LONG_CHUNK_SIZE: usize = 5_000;
/// 中篇章节合并阈值：相邻章节合并的最小字数
const MEDIUM_MERGE_MIN_WORDS: usize = 3_000;

/// 根据字数确定分块策略
pub fn determine_strategy(word_count: usize) -> ChunkingStrategy {
    if word_count <= SHORT_NOVEL_MAX {
        ChunkingStrategy::Full
    } else if word_count <= MEDIUM_NOVEL_MAX {
        ChunkingStrategy::ByChapters
    } else {
        ChunkingStrategy::MergedBlocks
    }
}

/// 创建文本分块
pub fn create_chunks(book: &ParsedBook) -> Vec<TextChunk> {
    let strategy = determine_strategy(book.word_count);

    match strategy {
        ChunkingStrategy::Full => create_full_chunk(book),
        ChunkingStrategy::ByChapters => {
            // 中篇：章节数过多时（>200章）按固定大小分块，否则保留章节结构
            if book.chapters.len() > 200 {
                split_by_fixed_size(&book.raw_text, LONG_CHUNK_SIZE)
            } else {
                split_by_chapters(&book.chapters)
            }
        }
        ChunkingStrategy::MergedBlocks => {
            // 长篇：按固定大小顺序分块，覆盖全部内容，不跳过任何部分
            split_by_fixed_size(&book.raw_text, LONG_CHUNK_SIZE)
        }
        // 兼容旧代码
        ChunkingStrategy::SampledBlocks => split_by_fixed_size(&book.raw_text, LONG_CHUNK_SIZE),
    }
}

/// 短篇：整本作为一个 chunk
fn create_full_chunk(book: &ParsedBook) -> Vec<TextChunk> {
    vec![TextChunk {
        index: 0,
        title: book.title.clone(),
        content: book.raw_text.clone(),
        word_count: book.word_count,
    }]
}

/// 中篇：按章节分块（保留原始章节结构）
fn split_by_chapters(chapters: &[ParsedChapter]) -> Vec<TextChunk> {
    chapters
        .iter()
        .enumerate()
        .map(|(i, ch)| TextChunk {
            index: i,
            title: ch.title.clone(),
            content: ch.content.clone(),
            word_count: ch.word_count,
        })
        .collect()
}

/// 长篇：按固定字符大小顺序切分，覆盖全部文本，不跳过任何内容
///
/// 算法：从文本开头开始，每 `chunk_size` 个字符切分为一个块，
/// 确保所有字符都被包含，最后一个块可能小于 `chunk_size`。
fn split_by_fixed_size(text: &str, chunk_size: usize) -> Vec<TextChunk> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut chunks: Vec<TextChunk> = Vec::new();
    let mut start = 0;
    let mut index = 0;

    while start < text.len() {
        // 计算当前块的结束位置（确保在字符边界上）
        let mut end = (start + chunk_size).min(text.len());
        while end < text.len() && !text.is_char_boundary(end) {
            end -= 1;
        }

        // 提取块内容
        let content = text[start..end].to_string();
        let word_count = count_chinese_words(&content);

        chunks.push(TextChunk {
            index,
            title: Some(format!("第{}部分", index + 1)),
            content,
            word_count,
        });

        start = end;
        index += 1;
    }

    chunks
}

/// 合并相邻的短章节（用于中篇，避免单个 chunk 过短）
pub fn merge_short_chapters(chapters: &[ParsedChapter], min_words: usize) -> Vec<TextChunk> {
    let mut chunks: Vec<TextChunk> = Vec::new();
    let mut current_buffer: Vec<String> = Vec::new();
    let mut current_titles: Vec<String> = Vec::new();
    let mut current_words: usize = 0;
    let mut chunk_index: usize = 0;

    for ch in chapters {
        current_buffer.push(ch.content.clone());
        if let Some(ref t) = ch.title {
            current_titles.push(t.clone());
        }
        current_words += ch.word_count;

        // 如果当前积累超过最小字数，生成一个 chunk
        if current_words >= min_words {
            let content = current_buffer.join("\n\n");
            let title = if current_titles.len() == 1 {
                current_titles.first().cloned()
            } else {
                Some(format!(
                    "{} - {}",
                    current_titles.first().unwrap_or(&"".to_string()),
                    current_titles.last().unwrap_or(&"".to_string())
                ))
            };

            chunks.push(TextChunk {
                index: chunk_index,
                title,
                content,
                word_count: current_words,
            });

            chunk_index += 1;
            current_buffer.clear();
            current_titles.clear();
            current_words = 0;
        }
    }

    // 处理剩余缓冲
    if !current_buffer.is_empty() {
        let content = current_buffer.join("\n\n");
        let title = current_titles.first().cloned();

        chunks.push(TextChunk {
            index: chunk_index,
            title,
            content,
            word_count: current_words,
        });
    }

    chunks
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

/// 提取文本的前 N 个字符作为样本
pub fn extract_sample(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        text.to_string()
    } else {
        let mut end = max_chars;
        while !text.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        text[..end].to_string()
    }
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_chapters(count: usize, words_per_chapter: usize) -> Vec<ParsedChapter> {
        (0..count)
            .map(|i| ParsedChapter {
                title: Some(format!("第{}章", i + 1)),
                content: "测试内容 ".repeat(words_per_chapter),
                word_count: words_per_chapter,
            })
            .collect()
    }

    #[test]
    fn test_determine_strategy() {
        assert_eq!(determine_strategy(50_000), ChunkingStrategy::Full);
        assert_eq!(determine_strategy(200_000), ChunkingStrategy::ByChapters);
        assert_eq!(
            determine_strategy(1_000_000),
            ChunkingStrategy::MergedBlocks
        );
    }

    #[test]
    fn test_split_by_fixed_size_covers_all() {
        // 构造一个长文本
        let text = "abcdefg".repeat(1000); // 7000 字符
        let chunks = split_by_fixed_size(&text, 1000);

        // 验证所有字符都被覆盖
        let reconstructed: String = chunks.iter().map(|c| &c.content as &str).collect();
        assert_eq!(reconstructed, text);

        // 验证块数自然由长度决定
        assert_eq!(chunks.len(), 7);

        // 验证索引连续
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
    }

    #[test]
    fn test_split_by_fixed_size_empty() {
        assert!(split_by_fixed_size("", 1000).is_empty());
    }

    #[test]
    fn test_split_by_fixed_size_unicode_boundary() {
        // 中文文本，确保不会在中文字符中间切断
        let text = "你好世界".repeat(100);
        let chunks = split_by_fixed_size(&text, 10);
        let reconstructed: String = chunks.iter().map(|c| &c.content as &str).collect();
        assert_eq!(reconstructed, text);
    }

    #[test]
    fn test_extract_sample() {
        assert_eq!(extract_sample("短文本", 100), "短文本");
        let long = "a".repeat(10000);
        assert_eq!(extract_sample(&long, 100).len(), 100);
    }
}
