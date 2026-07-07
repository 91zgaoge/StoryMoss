#![allow(dead_code)]
use regex::Regex;

pub struct TextUtils;

/// v0.26.24: 句子切分片段，保留原始定界符用于无损重建。
#[derive(Debug, Clone)]
struct SentenceSegment {
    /// 原始片段（含句末标点与相邻空白/换行），用于重建。
    raw: String,
    /// 去掉首尾空白的句子正文，用于归一化比较。
    body: String,
}

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
    /// 2. 散布式句子块重复检测（同一句子/多句块在文中以不同上下文出现 ≥2 次）；
    /// 3. KMP 最长 border 检测单段内或跨段长尾重复。
    pub fn trim_self_repetition(text: &str) -> String {
        let trimmed = text.trim();
        if trimmed.chars().count() < 40 {
            return text.to_string();
        }

        let paragraph_deduped = Self::trim_repeated_paragraphs(trimmed);
        if paragraph_deduped.len() < trimmed.len() {
            return paragraph_deduped;
        }

        // v0.26.24: 散布式块重复——同一句子或多句块在文中非首尾位置重复出现。
        // 典型症状：续写时模型陷入意象循环（冥界/牢笼/苦楚），同一描写句以
        // 不同上下文在文中重复 2-3 次。段落级和 border 检测都抓不到这种模式。
        let interspersed_deduped = Self::trim_interspersed_repeated_blocks(trimmed);
        if interspersed_deduped.len() < trimmed.len() {
            return interspersed_deduped;
        }

        Self::trim_by_longest_border(trimmed).unwrap_or_else(|| text.to_string())
    }

    /// v0.26.24: 续写跨内容重叠剥离。
    ///
    /// 根因（creative_workflow.log 2026-07-07 09:05）：续写时 TriShot Writer 把
    /// `build_continuation_context`
    /// 注入的「当前正文尾部预览」重新输出一遍，而非
    /// 衔接续写。`trim_self_repetition` 与 8%
    /// 自重复闸门只管单次生成**内部**的重复，
    /// 管不到「生成内容开篇复述已有正文」这条跨内容路径。
    ///
    /// 算法：归一化两者，找 `generated` 归一化文本的最长前缀 L，使其是
    /// `existing` 归一化文本（取尾部 3000
    /// 字比对，重叠几乎总发生在尾部）的子串；若 L ≥ `MIN_OVERLAP`（25
    /// 归一化字），在原文本中截断到该前缀之后，返回剩余的新内容。
    /// 若剥离后剩余 < 10 字（生成几乎全是复述），返回原文交上层重试闸门 / 前端
    /// `isTextDuplicate` 处理，避免吃掉全部导致空内容。
    pub fn strip_existing_overlap(generated: &str, existing: &str) -> String {
        let gen_trimmed = generated.trim();
        let existing_trimmed = existing.trim();
        if gen_trimmed.is_empty() || existing_trimmed.is_empty() {
            return generated.to_string();
        }

        // 只取已有正文尾部 3000 字做比对（重叠几乎总发生在尾部，远端无需参与）。
        let existing_tail: String = {
            let total = existing_trimmed.chars().count();
            if total > 3000 {
                existing_trimmed.chars().skip(total - 3000).collect()
            } else {
                existing_trimmed.to_string()
            }
        };

        let (norm_gen, gen_idx) = Self::build_normalized_index(gen_trimmed);
        let (norm_existing, _) = Self::build_normalized_index(&existing_tail);
        if norm_gen.is_empty() || norm_existing.is_empty() {
            return generated.to_string();
        }

        const MIN_OVERLAP: usize = 25;
        let gen_norm_chars: Vec<char> = norm_gen.chars().collect();
        let upper = gen_norm_chars.len().min(norm_existing.chars().count());
        // 从最长前缀递减找第一个被 existing 包含的——即最长重叠前缀。
        // O(n²) 上界对 ≤3000 字的续写尾部可接受（一次性后处理，非热路径）。
        let mut best = 0usize;
        for l in (MIN_OVERLAP..=upper).rev() {
            let prefix: String = gen_norm_chars[..l].iter().collect();
            if norm_existing.contains(&prefix) {
                best = l;
                break;
            }
        }

        if best < MIN_OVERLAP {
            return generated.to_string();
        }

        let cut_byte = gen_idx.get(best).copied().unwrap_or(gen_trimmed.len());
        let remaining = gen_trimmed[cut_byte..].trim_start();
        // 剥离后剩余过短 → 生成几乎全是复述，保留原文交上层处理。
        if remaining.chars().count() < 10 {
            return generated.to_string();
        }
        remaining.to_string()
    }

    /// v0.26.24: 裁掉 token/超时截断留下的极短末句。
    ///
    /// 典型症状（creative_workflow.log 2026-07-07）：续写因 60s
    /// 超时在「冥界的阴霾更。」 处硬截断，该半句被追加进正文，
    /// 污染后续续写上下文。仅当末句归一化后 < 12 字且 全文至少 2 句时才裁，
    /// 避免误伤正常短句收束。
    pub fn trim_dangling_tail(text: &str) -> String {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return text.to_string();
        }

        let sentences = Self::split_sentences_with_delimiters(trimmed);
        if sentences.len() < 2 {
            return text.to_string();
        }

        let last = &sentences[sentences.len() - 1];
        let last_norm_len = Self::normalize_for_duplicate_check(&last.body)
            .chars()
            .count();
        const MIN_DANGLING_CHARS: usize = 12;
        if last_norm_len >= MIN_DANGLING_CHARS {
            return text.to_string();
        }

        let mut result = String::new();
        for s in sentences.iter().take(sentences.len() - 1) {
            result.push_str(&s.raw);
        }
        let result = result.trim().to_string();
        if result.is_empty() {
            text.to_string()
        } else {
            result
        }
    }

    /// v0.26.24: 检测并裁剪散布式句子块重复。
    ///
    /// 把文本按句末标点（。！？\n）切成句子序列，归一化后查找在文中出现 ≥2 次
    /// 的句子或多句块，保留首次出现，裁掉后续重复。只处理归一化后 ≥ 15 字符
    /// 的块，避免误伤首尾呼应等良性短句重复。
    ///
    /// 算法：对每对起点 (i, j) i<j，计算句子序列的最长公共前缀 L；若块的归一化
    /// 拼接 ≥ 15 字符，则标记 [j, j+L) 为重复。最后按标记裁剪并保留原文本格式。
    fn trim_interspersed_repeated_blocks(text: &str) -> String {
        let sentences = Self::split_sentences_with_delimiters(text);
        if sentences.len() < 3 {
            return text.to_string();
        }
        // 超长输入降级（O(n²) 代价控制）：>300 句交给 border 兜底。
        if sentences.len() > 300 {
            return text.to_string();
        }

        let normalized: Vec<String> = sentences
            .iter()
            .map(|s| Self::normalize_for_duplicate_check(&s.body))
            .collect();
        let n = sentences.len();
        let mut removed = vec![false; n];

        const MIN_BLOCK_CHARS: usize = 15;

        for i in 0..n {
            if removed[i] {
                continue;
            }
            for j in (i + 1)..n {
                if removed[j] {
                    continue;
                }
                // 计算从 i 与 j 起的最长公共句子块长度 L（完整扩展，不提前截断）
                let mut l = 0usize;
                while i + l < n && j + l < n && !removed[i + l] && !removed[j + l] {
                    if normalized[i + l] != normalized[j + l] {
                        break;
                    }
                    l += 1;
                }
                if l == 0 {
                    continue;
                }
                // 块归一化字符数须 ≥ 15，避免误伤首尾呼应等良性短句重复
                let total_block_chars: usize =
                    (0..l).map(|k| normalized[i + k].chars().count()).sum();
                if total_block_chars < MIN_BLOCK_CHARS {
                    continue;
                }
                // 标记后续出现的整块为重复
                for k in 0..l {
                    removed[j + k] = true;
                }
            }
        }

        if !removed.iter().any(|&r| r) {
            return text.to_string();
        }

        let mut result = String::new();
        for (i, s) in sentences.iter().enumerate() {
            if !removed[i] {
                result.push_str(&s.raw);
            }
        }
        let result = result.trim().to_string();
        if result.is_empty() {
            text.to_string()
        } else {
            result
        }
    }

    /// 切分句子，保留每个句子的正文与原始定界符（含句末标点或换行/空行）。
    /// 返回的 `raw` 即原始片段，`body` 为去掉定界符的句子正文（用于归一化）。
    fn split_sentences_with_delimiters(text: &str) -> Vec<SentenceSegment> {
        let mut segments = Vec::new();
        let mut start = 0usize;
        let delimiters: &[char] = &['。', '？', '！', '.', '?', '!'];

        for (idx, ch) in text.char_indices() {
            if delimiters.contains(&ch) {
                let end = idx + ch.len_utf8();
                let raw = &text[start..end];
                let body = raw.trim();
                if !body.is_empty() {
                    segments.push(SentenceSegment {
                        raw: raw.to_string(),
                        body: body.to_string(),
                    });
                }
                start = end;
            }
        }
        // 尾部无句末标点的剩余片段
        if start < text.len() {
            let raw = &text[start..];
            let body = raw.trim();
            if !body.is_empty() {
                segments.push(SentenceSegment {
                    raw: raw.to_string(),
                    body: body.to_string(),
                });
            }
        }
        segments
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

    // v0.26.24: 散布式句子块重复——同一多句块以不同上下文在文中出现 ≥2 次。
    // 这是续写时模型陷入意象循环的典型症状（冥界/牢笼/苦楚块重复），段落级
    // 和 border 检测都抓不到。归一化块 ≥ 15 字才裁，避免误伤首尾呼应短句。
    #[test]
    fn test_trim_self_repetition_interspersed_block() {
        let block = "冥界的阴霾更加浓烈，在这个苦难的奋斗中，主角与恶魔的共同牢笼被沉沦在更深的冥界。幻境的沉淀，坚定的决心。一场惨烈的冒险，即将开始。";
        let lead = "他握紧恶魔的喉咙，咆哮着宣告契约的成立。";
        let mid = "深渊的阴暗中回荡着叩门的沉闷声。";
        let text = format!("{lead}{block}{mid}{block}");
        let expected = format!("{lead}{block}{mid}");
        assert_eq!(TextUtils::trim_self_repetition(&text), expected);
    }

    // v0.26.24: 散布式单长句重复——同一长句（归一化 ≥ 15 字）在文中出现两次。
    #[test]
    fn test_trim_self_repetition_interspersed_single_long_sentence() {
        let s = "恶魔的眼眶中闪过一丝恐惧，但在他的决心中，牢牢捆绑了他的挣脱。";
        let text = format!("{s}他咆哮不止。{s}");
        let expected = format!("{s}他咆哮不止。");
        assert_eq!(TextUtils::trim_self_repetition(&text), expected);
    }

    // v0.26.24: 短句重复不裁剪（< 15 归一化字），避免误伤首尾呼应。
    #[test]
    fn test_trim_self_repetition_interspersed_short_sentence_unchanged() {
        let text = "清晨的阳光洒在窗台上，新的一天开始了。我喝了一杯咖啡，准备出门。清晨的阳光洒在窗台上。";
        assert_eq!(TextUtils::trim_self_repetition(text), text);
    }

    // v0.26.24: 续写跨内容重叠剥离——生成内容开篇复述已有正文段落时，剥离重叠前缀。
    // 典型症状（creative_workflow.log 2026-07-07
    // 09:05）：续写生成以「恶魔的嘴唇弯曲 出一个苦涎的笑…」开头，而该段已在
    // 08:44:26 追加进已有正文 → 追加后重复。
    #[test]
    fn test_strip_existing_overlap_strips_regenerated_passage() {
        let existing = "冥府的牢笼牢牢锁住了他。他正在等待巅峰。\n\n恶魔的嘴唇弯曲出一个苦涎的笑，一颗棘刺般的闪烁在其眼中。我是你的牺牲，为你的愿望牺牲。";
        let generated = "恶魔的嘴唇弯曲出一个苦涎的笑，一颗棘刺般的闪烁在其眼中。我是你的牺牲，为你的愿望牺牲。主角深吸一口气，朝着冥界巅峰奔跑。";
        let result = TextUtils::strip_existing_overlap(generated, existing);
        assert!(
            !result.contains("恶魔的嘴唇弯曲出一个苦涎的笑"),
            "regenerated passage must be stripped, got: {}",
            result
        );
        assert!(
            result.contains("朝着冥界巅峰奔跑"),
            "new content must remain, got: {}",
            result
        );
    }

    // v0.26.24: 无重叠时原文原样返回。
    #[test]
    fn test_strip_existing_overlap_no_overlap_unchanged() {
        let existing = "他穿过废墟，脚步在碎石上发出轻微的响动。";
        let generated = "远处传来一阵低沉的轰鸣，他停下脚步。";
        let result = TextUtils::strip_existing_overlap(generated, existing);
        assert_eq!(result, generated);
    }

    // v0.26.24: 重叠 < 25 归一化字（良性短句呼应）不剥离。
    #[test]
    fn test_strip_existing_overlap_short_overlap_not_stripped() {
        let existing = "夜色深沉。他握紧了武器。";
        let generated = "夜色深沉。全新的情节在这里展开，主角继续前行。";
        let result = TextUtils::strip_existing_overlap(generated, existing);
        // "夜色深沉" 归一化仅 4 字 < 25，不触发剥离。
        assert_eq!(result, generated);
    }

    // v0.26.24: 生成几乎全是复述（剥离后剩余 < 10 字）时保留原文，交上层处理。
    #[test]
    fn test_strip_existing_overlap_near_total_overlap_keeps_original() {
        let existing = "冥府的牢笼牢牢锁住了他，但这并不能遏制他的追逐欲望。凝固的冰霜流动，化作黑暗的涌流，漫泻着他的幻想。他正在等待巅峰。";
        let generated = "冥府的牢笼牢牢锁住了他，但这并不能遏制他的追逐欲望。凝固的冰霜流动，化作黑暗的涌流，漫泻着他的幻想。他正在等待巅峰。";
        let result = TextUtils::strip_existing_overlap(generated, existing);
        // 剥离后剩余为空 < 10 字 → 保留原文。
        assert_eq!(result, generated);
    }

    // v0.26.24: 只比对已有正文尾部 3000 字——远端（>3000 字之前）重叠不剥离。
    #[test]
    fn test_strip_existing_overlap_only_checks_tail() {
        // 头部 > 3000 字的填充，使开头的「远古契约」段落在尾部窗口之外。
        let far_head = "远古的契约已经签订，众神陨落于深渊。".repeat(120); // > 3000 字
        let tail = "尾声终于到来，主角站在冥界巅峰之前。";
        let existing = format!("{}{}", far_head, tail);
        // 生成内容复述的是已有正文**头部**的段落（在尾部 3000 字窗口外）+ 新内容。
        let generated = "远古的契约已经签订，众神陨落于深渊。全新的情节在这里展开。";
        let result = TextUtils::strip_existing_overlap(generated, &existing);
        // 头部重叠在尾部窗口外，不应剥离。
        assert_eq!(
            result, generated,
            "overlap outside tail window must not be stripped"
        );
    }

    // v0.26.24: 已有正文尾部的重叠会被剥离（尾部窗口内）。
    #[test]
    fn test_strip_existing_overlap_tail_within_window_stripped() {
        let far_head = "远古的契约已经签订，众神陨落于深渊。".repeat(120);
        let overlap = "恶魔的嘴唇弯曲出一个苦涎的笑，一颗棘刺般的闪烁在其眼中。";
        let existing = format!("{}{}主角继续前行。", far_head, overlap);
        let generated = format!("{}新的情节在这里继续展开，主角向前迈进。", overlap);
        let result = TextUtils::strip_existing_overlap(&generated, &existing);
        assert!(
            !result.contains("恶魔的嘴唇弯曲出一个苦涎的笑"),
            "tail-window overlap must be stripped, got: {}",
            result
        );
        assert!(result.contains("主角向前迈进"));
    }

    // v0.26.24: token/超时截断留下的极短末句应被裁掉。
    #[test]
    fn test_trim_dangling_tail_strips_truncated_last_sentence() {
        let text = "主角与恶魔浸入到一个更深的冥境中，在那里，他们将面对更糟糕的冥府巅峰之谜。在牢笼前，恶魔停止了咬堪，牢牢捆绑在主角的手中。冥界的阴霾更。";
        let result = TextUtils::trim_dangling_tail(text);
        assert!(
            !result.contains("冥界的阴霾更"),
            "dangling tail must be stripped, got: {}",
            result
        );
        assert!(result.ends_with("牢牢捆绑在主角的手中。"));
    }

    // v0.26.24: 正常短句收束（≥ 12 归一化字）不裁剪。
    #[test]
    fn test_trim_dangling_tail_keeps_normal_short_ending() {
        let text = "他穿过废墟，脚步在碎石上发出轻微的响动。远处传来一阵低沉的轰鸣，他停下脚步。";
        assert_eq!(TextUtils::trim_dangling_tail(text), text);
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
