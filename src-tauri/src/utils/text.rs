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

        // 只取已有正文尾部 3000
        // 字做比对（重叠几乎总发生在尾部，远端无需参与）。
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

    /// 闭合向标点字符集（与前端 `format.ts` 对齐；开向字符不在其列）。
    const CLOSING_PUNCT: &[char] = &[
        '"', '\'', '\u{2019}', '\u{201D}', '」', '』', '）', '】', '》', '〉', ']', '}',
    ];

    /// 合并「换行后紧跟闭合标点」的悬挂闭合标点回上一行。
    ///
    /// 与前端 `format.ts::mergeHangingClosingPunct` 同规则同字符集：
    /// LLM 生成长对话时软换行，闭合引号（" ' 」 』 等）单独占一行，
    /// 落库/分段后会产生只含闭合引号的孤段落。只合并闭合向字符
    /// （" ' ’ ” 」 』 ） 】 》 〉 ] }），开向字符（「 『 （ 【 《 〈 [
    /// {）不合并。 边界：上一行不存在（文本以换行+闭合标点开头）时不合并。
    /// 换行与闭合标点之间的空白/全角缩进/零宽字符一并丢掉，否则
    /// 「禁军。\\n\\n　　”」会落成带段首缩进的孤段落。
    /// 已知限制：直引号 " ' 无方向信息，刻意以闭合引号开场的段落或
    /// 跨段引语（每段以 " 开场）会被误并——中文小说语料下罕见，属启发式取舍。
    pub fn merge_hanging_closing_punct(text: &str) -> String {
        if !text.contains('\n') {
            return text.to_string();
        }
        let mut out = String::with_capacity(text.len());
        let mut pending_newlines = 0usize;
        let mut pending_ws = String::new();
        for c in text.chars() {
            if c == '\n' {
                pending_newlines += 1;
                pending_ws.clear();
                continue;
            }
            if pending_newlines > 0 {
                if Self::is_hanging_indent(c) {
                    pending_ws.push(c);
                    continue;
                }
                // out 中不会有换行（换行一律缓冲），非空即说明上一行存在
                let merge = Self::CLOSING_PUNCT.contains(&c) && !out.is_empty();
                if !merge {
                    out.push_str(&"\n".repeat(pending_newlines));
                    out.push_str(&pending_ws);
                }
                pending_newlines = 0;
                pending_ws.clear();
            }
            out.push(c);
        }
        out.push_str(&"\n".repeat(pending_newlines));
        out.push_str(&pending_ws);
        out
    }

    fn is_hanging_indent(c: char) -> bool {
        c != '\n' && (c.is_whitespace() || c == '\u{200b}')
    }

    /// HTML 级：把「整段内容仅为闭合标点+空白」的 `<p>` 并入上一段。
    ///
    /// 与前端 `format.ts::mergeLoneClosingPunctParagraphs` 同规则：覆盖段落内是
    /// 字符或 HTML 实体（&rdquo; &quot; &#x201D; &#8221; 等）与首尾空白；
    /// 仅当前面存在可并入的段落（</p>）时才合并；无 </p> 的输入原样返回。
    /// 供 V128 迁移复用。手写扫描，不引入新 crate；仅匹配小写 <p>/</p>。
    pub fn merge_lone_closing_punct_paragraphs(html: &str) -> String {
        if !html.contains("</p>") {
            return html.to_string();
        }
        let mut out = String::with_capacity(html.len());
        let mut rest = html;
        loop {
            // out 以 </p> 结尾且 rest 起始（跳过空白）是孤闭合标字段 →
            // 并入上一段。
            // 回到循环顶部可继续覆盖连续多个孤闭合标字段。
            let trimmed = rest.trim_start();
            if out.ends_with("</p>") && trimmed.starts_with("<p>") {
                if let Some(inner_end) = trimmed[3..].find("</p>") {
                    let inner = &trimmed[3..3 + inner_end];
                    if Self::is_lone_closing_punct(inner) {
                        out.truncate(out.len() - 4);
                        out.push_str(&Self::strip_lone_punct_ws(inner));
                        out.push_str("</p>");
                        rest = &trimmed[3 + inner_end + 4..];
                        continue;
                    }
                }
            }
            match rest.find("</p>") {
                Some(idx) => {
                    out.push_str(&rest[..idx + 4]);
                    rest = &rest[idx + 4..];
                }
                None => {
                    out.push_str(rest);
                    break;
                }
            }
        }
        out
    }

    /// 判断 `<p>` 段内是否仅为闭合标点（字符或 HTML 实体）+
    /// 空白，且至少一个标点。
    fn is_lone_closing_punct(inner: &str) -> bool {
        // 闭合标点的实体形态（含数据里误编码的开向实体 &ldquo;/&lsquo;，
        // 以及只算空白的 &nbsp;），与前端 format.ts 对齐。
        const PUNCT_ENTITIES: &[&str] = &[
            "&rdquo;", "&ldquo;", "&quot;", "&apos;", "&rsquo;", "&lsquo;", "&#x201D;", "&#x201d;",
            "&#x2019;", "&#8221;", "&#8217;", "&#34;", "&#39;",
        ];
        let mut found_punct = false;
        let mut rest = inner;
        while !rest.is_empty() {
            let c = rest.chars().next().unwrap();
            if c.is_whitespace() || c == '\u{200b}' {
                rest = &rest[c.len_utf8()..];
            } else if Self::CLOSING_PUNCT.contains(&c) {
                found_punct = true;
                rest = &rest[c.len_utf8()..];
            } else if let Some(stripped) = rest.strip_prefix("&nbsp;") {
                rest = stripped;
            } else if c == '&' {
                match PUNCT_ENTITIES.iter().find(|e| rest.starts_with(**e)) {
                    Some(e) => {
                        found_punct = true;
                        rest = &rest[e.len()..];
                    }
                    None => return false,
                }
            } else {
                return false;
            }
        }
        found_punct
    }

    fn strip_lone_punct_ws(inner: &str) -> String {
        inner
            .replace("&nbsp;", "")
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '\u{200b}')
            .collect()
    }

    /// HTML 级：把**段首**的闭合标点并回上一段末尾。
    ///
    /// 与前端 `format.ts::mergeLeadingClosingPunctParagraphs`
    /// 同规则。根因：句子切分 在句末标点（。！？）处断开，
    /// 把紧随其后的收尾引号切给了下一句；段落组装正好在
    /// 那一处断段时，就落成
    /// `<p>”\n正文…</p>`——孤引号独占一行（真机《帝国的烟火》
    /// 第 2 章）。V128
    /// 只覆盖「整段仅闭合标点」的形态，引号后面还跟着正文的漏掉了。
    ///
    /// 段首只认**有方向的**收尾符（” ’ 」 』 ） 】 》 〉 及其实体形态）：ASCII
    /// 直引号 `"` `'` 可能是开引号（英文式对话每段以 "
    /// 开场），保持原样。收尾符与正文之间的
    /// 空白/换行一并丢掉（只是孤引号的排版残留）。供 V139
    /// 迁移复用；手写扫描，不引入 新 crate；仅匹配小写 <p>/</p>。
    pub fn merge_leading_closing_punct_paragraphs(html: &str) -> String {
        if !html.contains("</p>") {
            return html.to_string();
        }
        let mut out = String::with_capacity(html.len());
        let mut rest = html;
        loop {
            let trimmed = rest.trim_start();
            if out.ends_with("</p>") && trimmed.starts_with("<p>") {
                if let Some(inner_end) = trimmed[3..].find("</p>") {
                    let inner = &trimmed[3..3 + inner_end];
                    if let Some((marks, body)) = Self::split_leading_closing_punct(inner) {
                        out.truncate(out.len() - 4);
                        out.push_str(&marks);
                        out.push_str("</p>");
                        if !body.is_empty() {
                            out.push_str("<p>");
                            out.push_str(body);
                            out.push_str("</p>");
                        }
                        rest = &trimmed[3 + inner_end + 4..];
                        continue;
                    }
                }
            }
            match rest.find("</p>") {
                Some(idx) => {
                    out.push_str(&rest[..idx + 4]);
                    rest = &rest[idx + 4..];
                }
                None => {
                    out.push_str(rest);
                    break;
                }
            }
        }
        out
    }

    /// 段内以有方向的收尾符开头时返回 `(收尾符串, 其余正文)`。
    /// 收尾符与正文之间的空白/零宽字符丢掉；整段只有收尾符时正文为空串。
    fn split_leading_closing_punct(inner: &str) -> Option<(String, &str)> {
        const DIRECTIONAL: &[char] = &['\u{201D}', '\u{2019}', '」', '』', '）', '】', '》', '〉'];
        const ENTITIES: &[&str] = &[
            "&rdquo;", "&#x201D;", "&#x201d;", "&#8221;", "&rsquo;", "&#x2019;", "&#8217;",
        ];
        let mut marks = String::new();
        let mut rest = inner;
        loop {
            let Some(c) = rest.chars().next() else { break };
            if DIRECTIONAL.contains(&c) {
                marks.push(c);
                rest = &rest[c.len_utf8()..];
                continue;
            }
            if let Some(e) = ENTITIES.iter().find(|e| rest.starts_with(**e)) {
                marks.push_str(e);
                rest = &rest[e.len()..];
                continue;
            }
            if c.is_whitespace() || c == '\u{200b}' {
                rest = rest.trim_start_matches(|ch: char| ch.is_whitespace() || ch == '\u{200b}');
                if rest.is_empty() {
                    break;
                }
                continue;
            }
            break;
        }
        if marks.is_empty() {
            None
        } else {
            Some((marks, rest))
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
                // 计算从 i 与 j 起的最长公共句子块长度
                // L（完整扩展，不提前截断）
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

    // v0.26.24: 续写跨内容重叠剥离——生成内容开篇复述已有正文段落时，
    // 剥离重叠前缀。 典型症状（creative_workflow.log 2026-07-07
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
        // 生成内容复述的是已有正文**头部**的段落（在尾部 3000 字窗口外）+
        // 新内容。
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
    //   前端 vitest `textCleanup.golden.test.ts` 加载并断言 TS
    // `trimSelfRepetition`
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

    #[test]
    fn test_merge_hanging_closing_punct_merges_hanging_quote() {
        assert_eq!(
            TextUtils::merge_hanging_closing_punct("他喊道：\"快走吧。\n\""),
            "他喊道：\"快走吧。\""
        );
        assert_eq!(
            TextUtils::merge_hanging_closing_punct("……控制'。\n”"),
            "……控制'。”"
        );
        // 连续多个换行也并回
        assert_eq!(
            TextUtils::merge_hanging_closing_punct("上一行\n\n\n」"),
            "上一行」"
        );
        // 全角缩进 / 半角空格 / 零宽字符夹在换行与下引号之间也并回
        assert_eq!(
            TextUtils::merge_hanging_closing_punct("京城的禁军。\n\n　　”"),
            "京城的禁军。”"
        );
        assert_eq!(
            TextUtils::merge_hanging_closing_punct("莫要再自误了。\n ”"),
            "莫要再自误了。”"
        );
        assert_eq!(
            TextUtils::merge_hanging_closing_punct("上一行\n\u{200b}”"),
            "上一行”"
        );
        // 闭合字符集全员
        for c in [
            '"', '\'', '\u{2019}', '\u{201D}', '」', '』', '）', '】', '》', '〉', ']', '}',
        ] {
            let input = format!("上一行\n{c}");
            let expected = format!("上一行{c}");
            assert_eq!(
                TextUtils::merge_hanging_closing_punct(&input),
                expected,
                "closing {c}"
            );
        }
    }

    #[test]
    fn test_merge_hanging_closing_punct_opening_not_merged() {
        for c in ['「', '『', '（', '【', '《', '〈', '[', '{'] {
            let input = format!("上一行\n{c}对话");
            assert_eq!(
                TextUtils::merge_hanging_closing_punct(&input),
                input,
                "opening {c}"
            );
        }
    }

    #[test]
    fn test_merge_hanging_closing_punct_no_previous_line() {
        // 上一行不存在（文本以换行+闭合标点开头）时不合并
        assert_eq!(
            TextUtils::merge_hanging_closing_punct("\n\"开头"),
            "\n\"开头"
        );
        assert_eq!(
            TextUtils::merge_hanging_closing_punct("\n\n”开头"),
            "\n\n”开头"
        );
        assert_eq!(TextUtils::merge_hanging_closing_punct(""), "");
        assert_eq!(TextUtils::merge_hanging_closing_punct("无换行"), "无换行");
    }

    #[test]
    fn test_merge_lone_closing_punct_paragraphs_basic() {
        assert_eq!(
            TextUtils::merge_lone_closing_punct_paragraphs("<p>他控制着局面。</p><p>”</p>"),
            "<p>他控制着局面。”</p>"
        );
        // 连续孤闭合标字段
        assert_eq!(
            TextUtils::merge_lone_closing_punct_paragraphs("<p>甲。</p><p>”</p><p>’</p>"),
            "<p>甲。”’</p>"
        );
        // 段内首尾空白 / 全角缩进在并入时丢掉，避免「禁军。　　”」
        assert_eq!(
            TextUtils::merge_lone_closing_punct_paragraphs("<p>段落。</p><p> ” </p>"),
            "<p>段落。”</p>"
        );
        assert_eq!(
            TextUtils::merge_lone_closing_punct_paragraphs("<p>段落。</p><p>　　”</p>"),
            "<p>段落。”</p>"
        );
    }

    #[test]
    fn test_merge_lone_closing_punct_paragraphs_entities() {
        for entity in ["&rdquo;", "&quot;", "&#x201D;", "&#8221;"] {
            let input = format!("<p>段落。</p><p>{entity}</p>");
            let expected = format!("<p>段落。{entity}</p>");
            assert_eq!(
                TextUtils::merge_lone_closing_punct_paragraphs(&input),
                expected,
                "entity {entity}"
            );
        }
    }

    #[test]
    fn test_merge_lone_closing_punct_paragraphs_no_preceding_paragraph() {
        let input = "<p>”</p><p>段落。</p>";
        assert_eq!(TextUtils::merge_lone_closing_punct_paragraphs(input), input);
    }

    #[test]
    fn test_merge_lone_closing_punct_paragraphs_non_lone_unchanged() {
        // 段内不止闭合标点 → 不动
        let input = "<p>甲。</p><p>”他说</p>";
        assert_eq!(TextUtils::merge_lone_closing_punct_paragraphs(input), input);
        // 纯空白段（无标点）→ 不动
        let input = "<p>甲。</p><p> </p>";
        assert_eq!(TextUtils::merge_lone_closing_punct_paragraphs(input), input);
        // 无 <p> → 原样返回
        let input = "纯文本\n\"无段落";
        assert_eq!(TextUtils::merge_lone_closing_punct_paragraphs(input), input);
    }

    #[test]
    fn test_merge_leading_closing_punct_paragraphs_basic() {
        // 真机《帝国的烟火》第 2 章形态：段首孤引号 + 换行 + 正文
        assert_eq!(
            TextUtils::merge_leading_closing_punct_paragraphs(
                "<p>…还是你苏家的命。</p><p>”\n大堂内的空气仿佛凝固成了实质。</p>"
            ),
            "<p>…还是你苏家的命。”</p><p>大堂内的空气仿佛凝固成了实质。</p>"
        );
        // 整段只有闭合标点 → 并段
        assert_eq!(
            TextUtils::merge_leading_closing_punct_paragraphs("<p>甲。</p><p>”</p>"),
            "<p>甲。”</p>"
        );
        assert_eq!(
            TextUtils::merge_leading_closing_punct_paragraphs("<p>甲。</p><p>”</p><p>’</p>"),
            "<p>甲。”’</p>"
        );
        // 段首全角缩进 + 换行 + 正文
        assert_eq!(
            TextUtils::merge_leading_closing_punct_paragraphs("<p>甲。</p><p>　　”\n乙。</p>"),
            "<p>甲。”</p><p>乙。</p>"
        );
    }

    #[test]
    fn test_merge_leading_closing_punct_paragraphs_entities() {
        assert_eq!(
            TextUtils::merge_leading_closing_punct_paragraphs("<p>甲。</p><p>&rdquo;\n乙。</p>"),
            "<p>甲。&rdquo;</p><p>乙。</p>"
        );
        assert_eq!(
            TextUtils::merge_leading_closing_punct_paragraphs("<p>甲。</p><p>&#x201D;乙。</p>"),
            "<p>甲。&#x201D;</p><p>乙。</p>"
        );
    }

    #[test]
    fn test_merge_leading_closing_punct_paragraphs_directional_only() {
        // ASCII 直引号可能是开引号（英文式对话）→ 不动
        for c in ['"', '\''] {
            let input = format!("<p>甲。</p><p>{c}\n乙。</p>");
            assert_eq!(
                TextUtils::merge_leading_closing_punct_paragraphs(&input),
                input,
                "ascii {c}"
            );
        }
        // 开向字符 → 不动
        for c in ['「', '『', '（', '【', '《', '“', '‘'] {
            let input = format!("<p>甲。</p><p>{c}乙。</p>");
            assert_eq!(
                TextUtils::merge_leading_closing_punct_paragraphs(&input),
                input,
                "opening {c}"
            );
        }
        // 没有上一段 → 不动
        let input = "<p>”\n甲。</p>";
        assert_eq!(
            TextUtils::merge_leading_closing_punct_paragraphs(input),
            input
        );
        // 无 <p> → 原样返回
        let input = "纯文本\n”无段落";
        assert_eq!(
            TextUtils::merge_leading_closing_punct_paragraphs(input),
            input
        );
    }
}
