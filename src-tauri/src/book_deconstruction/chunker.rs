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
/// 长篇固定分块大小（字符数）— 仅作为 fallback
const _LONG_CHUNK_SIZE: usize = 5_000;
/// 中篇章节合并阈值：相邻章节合并的最小字数
const MEDIUM_MERGE_MIN_WORDS: usize = 3_000;
/// 大章节阈值：超过此字数的章节需要按场景转换点再分
const LARGE_CHAPTER_THRESHOLD: usize = 8_000;
/// 短章节阈值：低于此字数的章节会累积合并
const SHORT_CHAPTER_THRESHOLD: usize = 2_000;
/// 合并缓冲目标字数：累积到此后生成一个 chunk
const MERGE_BUFFER_TARGET: usize = 3_000;

/// 根据字数确定分块策略
pub fn determine_strategy(word_count: usize) -> ChunkingStrategy {
    if word_count <= SHORT_NOVEL_MAX {
        ChunkingStrategy::Full
    } else if word_count <= MEDIUM_NOVEL_MAX {
        ChunkingStrategy::ByChapters
    } else {
        ChunkingStrategy::NarrativeAware
    }
}

/// 创建文本分块
pub fn create_chunks(book: &ParsedBook) -> Vec<TextChunk> {
    let strategy = determine_strategy(book.word_count);

    match strategy {
        ChunkingStrategy::Full => create_full_chunk(book),
        ChunkingStrategy::ByChapters => {
            // 中篇：章节数过多时（>200章）用叙事感知分块，否则保留章节结构
            if book.chapters.len() > 200 {
                split_narrative_aware(book)
            } else {
                split_by_chapters(&book.chapters)
            }
        }
        ChunkingStrategy::NarrativeAware => {
            // 长篇：叙事感知分块——以章节边界为叙事边界
            split_narrative_aware(book)
        }
        // 兼容旧代码
        ChunkingStrategy::MergedBlocks | ChunkingStrategy::SampledBlocks => {
            split_narrative_aware(book)
        }
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

// ==================== LitSeg 叙事感知分块 ====================

/// 叙事感知分块 — LitSeg 核心洞察
///
/// 原则:
/// 1. 章节边界是首要叙事边界，绝不切断章节
/// 2. 超过 LARGE_CHAPTER_THRESHOLD 字符的章节，按场景转换点再分
/// 3. 相邻短章节（<SHORT_CHAPTER_THRESHOLD 字符）合并到同一块
/// 4. 合并缓冲目标为 MERGE_BUFFER_TARGET 字符
pub fn split_narrative_aware(book: &ParsedBook) -> Vec<TextChunk> {
    if book.chapters.is_empty() {
        return vec![TextChunk {
            index: 0,
            title: book.title.clone(),
            content: book.raw_text.clone(),
            word_count: book.word_count,
        }];
    }

    let mut chunks: Vec<TextChunk> = Vec::new();
    let mut buffer_chapters: Vec<&ParsedChapter> = Vec::new();
    let mut buffer_words: usize = 0;
    let mut chunk_index: usize = 0;

    for chapter in &book.chapters {
        // 单章超过阈值: 先 flush buffer，再单独处理大章节
        if chapter.word_count > LARGE_CHAPTER_THRESHOLD {
            // flush 当前 buffer
            if !buffer_chapters.is_empty() {
                chunks.push(build_chunk(&buffer_chapters, chunk_index));
                chunk_index += 1;
                buffer_chapters.clear();
                buffer_words = 0;
            }
            // 大章节按场景转换点再分
            let sub_chunks = split_large_chapter(chapter, chunk_index);
            chunk_index += sub_chunks.len();
            chunks.extend(sub_chunks);
        }
        // 短章节: 累积到 buffer
        else {
            buffer_chapters.push(chapter);
            buffer_words += chapter.word_count;
            // buffer 超过目标字数，生成 chunk
            if buffer_words >= MERGE_BUFFER_TARGET {
                chunks.push(build_chunk(&buffer_chapters, chunk_index));
                chunk_index += 1;
                buffer_chapters.clear();
                buffer_words = 0;
            }
        }
    }

    // flush 剩余缓冲
    if !buffer_chapters.is_empty() {
        chunks.push(build_chunk(&buffer_chapters, chunk_index));
    }

    chunks
}

/// 大章节内部按场景转换点分块
fn split_large_chapter(chapter: &ParsedChapter, start_index: usize) -> Vec<TextChunk> {
    let boundaries = detect_scene_boundaries(&chapter.content);

    if boundaries.len() <= 1 {
        // 无明确场景转换，整章作为一个 chunk
        return vec![TextChunk {
            index: start_index,
            title: chapter.title.clone(),
            content: chapter.content.clone(),
            word_count: chapter.word_count,
        }];
    }

    let mut chunks = Vec::new();
    let mut prev_boundary = 0;

    for (i, &boundary) in boundaries.iter().enumerate().skip(1) {
        let content = chapter.content[prev_boundary..boundary].to_string();
        let word_count = count_chinese_words(&content);

        chunks.push(TextChunk {
            index: start_index + i - 1,
            title: chapter.title.as_ref().map(|t| format!("{} (场景{})", t, i)),
            content,
            word_count,
        });

        prev_boundary = boundary;
    }

    // 最后一段
    if prev_boundary < chapter.content.len() {
        let content = chapter.content[prev_boundary..].to_string();
        let word_count = count_chinese_words(&content);
        chunks.push(TextChunk {
            index: start_index + chunks.len(),
            title: chapter
                .title
                .as_ref()
                .map(|t| format!("{} (场景{})", t, boundaries.len())),
            content,
            word_count,
        });
    }

    chunks
}

/// 检测文本中的场景转换点
///
/// 场景转换信号（按优先级排序）:
/// 1. 连续两个以上空行（段落间距）
/// 2. 时间/地点转换词（"三天后", "与此同时", "回到"...）
/// 3. 视角转换词（"另一边", "与此同时"...）
fn detect_scene_boundaries(content: &str) -> Vec<usize> {
    let mut boundaries = vec![0]; // 始终从开头开始
    let lines: Vec<&str> = content.lines().collect();

    let time_transition_markers = [
        "三天后",
        "一周后",
        "一个月后",
        "一年后",
        "几年后",
        "数年后",
        "第二天",
        "次日",
        "当晚",
        "翌日",
        "翌晨",
        "黄昏",
        "黎明",
        "与此同时",
        "同一时间",
        "不久",
        "过了一会儿",
        "片刻之后",
        "翌年",
        "翌月",
        "数日后",
        "几日后",
        "次日清晨",
        "翌日黄昏",
        "翌日凌晨",
        "翌日清晨",
        "翌日中午",
        "翌日傍晚",
    ];

    let location_transition_markers = [
        "回到",
        "来到",
        "抵达",
        "进入",
        "离开",
        "走出",
        "走进",
        "与此同时",
        "另一边",
        "在",
        "位于",
    ];

    let mut empty_line_count = 0;
    let mut last_boundary_line = 0;

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // 信号 1: 连续空行
        if trimmed.is_empty() {
            empty_line_count += 1;
            if empty_line_count >= 2 && line_idx - last_boundary_line > 5 {
                // 计算字符位置
                let pos = lines[..line_idx].join("\n").len();
                if pos > *boundaries.last().unwrap_or(&0) + 20 {
                    boundaries.push(pos);
                    last_boundary_line = line_idx;
                }
            }
            continue;
        }

        empty_line_count = 0;

        // 信号 2: 时间/地点转换词（行首出现）
        if line_idx - last_boundary_line > 10 {
            let is_time_transition = time_transition_markers
                .iter()
                .any(|&m| trimmed.starts_with(m));
            let is_location_transition = location_transition_markers
                .iter()
                .any(|&m| trimmed.starts_with(m));

            if is_time_transition || is_location_transition {
                let pos = lines[..line_idx].join("\n").len();
                if pos > *boundaries.last().unwrap_or(&0) + 50 {
                    boundaries.push(pos);
                    last_boundary_line = line_idx;
                }
            }
        }
    }

    boundaries
}

/// 从章节列表构建 TextChunk
fn build_chunk(chapters: &[&ParsedChapter], index: usize) -> TextChunk {
    let content = chapters
        .iter()
        .map(|ch| {
            let title = ch.title.as_deref().unwrap_or("");
            if title.is_empty() {
                ch.content.clone()
            } else {
                format!("{title}\n\n{}", ch.content)
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    let word_count = chapters.iter().map(|ch| ch.word_count).sum();

    let title = match chapters.len() {
        0 => None,
        1 => chapters[0].title.clone(),
        _ => {
            let first = chapters
                .first()
                .and_then(|c| c.title.as_deref())
                .unwrap_or("");
            let last = chapters
                .last()
                .and_then(|c| c.title.as_deref())
                .unwrap_or("");
            if first == last || last.is_empty() {
                Some(first.to_string())
            } else {
                Some(format!("{} - {}", first, last))
            }
        }
    };

    TextChunk {
        index,
        title,
        content,
        word_count,
    }
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
            ChunkingStrategy::NarrativeAware
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

    #[test]
    fn test_split_narrative_aware_preserves_chapter_boundaries() {
        // 构造一本 3 章的小说，每章 3000 字
        let chapters = vec![
            ParsedChapter {
                title: Some("第一章 开端".to_string()),
                content: "测试内容 ".repeat(1500),
                word_count: 1500,
            },
            ParsedChapter {
                title: Some("第二章 发展".to_string()),
                content: "测试内容 ".repeat(1500),
                word_count: 1500,
            },
            ParsedChapter {
                title: Some("第三章 结局".to_string()),
                content: "测试内容 ".repeat(1500),
                word_count: 1500,
            },
        ];
        let book = ParsedBook {
            title: Some("测试小说".to_string()),
            author: None,
            chapters,
            raw_text: String::new(),
            word_count: 4500,
        };

        let chunks = split_narrative_aware(&book);

        // 所有章节都短于阈值，应该合并成 1-2 个 chunk
        assert!(!chunks.is_empty());
        // 验证没有 chunk 切断章节（通过检查章节标题完整性）
        for chunk in &chunks {
            if let Some(ref title) = chunk.title {
                // 标题应该包含完整章节名，不包含半截
                assert!(!title.ends_with("第"));
                assert!(!title.ends_with("章"));
            }
        }
    }

    #[test]
    fn test_split_narrative_aware_large_chapter_split() {
        // 构造一章超大章节（>8000字）
        let mut large_content = String::new();
        large_content.push_str("这是一个大章节的开头。\n\n");
        // 生成足够多的段落使总字数超过 8000
        for i in 0..600 {
            large_content.push_str(&format!(
                "段落{} 测试内容测试内容测试内容测试内容测试内容。\n",
                i
            ));
            if i == 200 || i == 400 {
                // 模拟场景转换
                large_content.push_str("\n\n三天后，主角来到新的地方。\n\n");
            }
        }

        let word_count = count_chinese_words(&large_content);
        assert!(
            word_count > 8000,
            "测试章节字数应超过 8000，实际 {}",
            word_count
        );

        let chapters = vec![ParsedChapter {
            title: Some("大章节".to_string()),
            word_count,
            content: large_content,
        }];
        let book = ParsedBook {
            title: Some("测试".to_string()),
            author: None,
            chapters,
            raw_text: String::new(),
            word_count: 10000,
        };

        let chunks = split_narrative_aware(&book);

        // 大章节应该被分成多个 chunk
        assert!(
            chunks.len() >= 2,
            "大章节应该被场景转换点切分成至少 2 个 chunk，实际 {} 个",
            chunks.len()
        );
    }

    #[test]
    fn test_detect_scene_boundaries() {
        let text = "第一章开头。\n\n一些内容。\n\n\n三天后，主角来到新的地方。\n\n后续内容。\n\n\n与此同时，反派在密谋。\n\n结尾。";
        let boundaries = detect_scene_boundaries(text);

        // 应该检测到至少开头 + 2 个转换点
        assert!(boundaries.len() >= 2, "应该检测到场景转换点");
        assert_eq!(boundaries[0], 0, "第一个边界应该在开头");
    }
}
