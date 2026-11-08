//! v0.93.2 导出正文规范化：把存量正文统一转成「纯段落」再交给各格式生成器。
//!
//! 背景：幕前编辑器（tiptap）以 `getHTML()` 落库，`scenes.content` 的真实
//! 形态是 `<p>段落一</p><p>段落二</p>` 连写（无换行）。旧导出按 `\n\n`
//! 分段，整章 HTML 会被当成**一个**段落：
//! - EPUB/HTML：整段被 `html_escape` 转义 → 阅读器里出现字面 `</p><p>`；
//! - PDF/TXT：直接倾倒原始标签 + AI 残留的 ``` 围栏 → 乱码观感；
//! - MD：查看器恰好忽略游离标签才"看似正确"（文件里其实也带着标签）。
//!
//! 本模块是导出侧唯一的正文清洗入口，三个能力：
//! 1. HTML 解析：块级标签（p/div/h1-6/li/blockquote/pre/br 等）为段落边界，
//!    行内标签（strong/em/span/a…）剥壳留文本；
//! 2. 实体解码：`&nbsp;`/`&amp;`/数字实体等（先剥标签后解码，避免正文里
//!    被转义的 `&lt;p&gt;` 反过来被误吃）；
//! 3. 去 AI 残留：整行 ``` / ~~~ 围栏标记丢弃。
//!
//! 无 HTML 的旧数据回退为按换行分段。

/// 判断内容是否含块级 HTML 标记（决定走 HTML 解析还是纯文本回退）。
fn looks_like_html(content: &str) -> bool {
    const BLOCK_TAGS: [&str; 9] = [
        "<p",
        "<div",
        "<br",
        "<h1",
        "<h2",
        "<h3",
        "<li",
        "<blockquote",
        "<pre",
    ];
    let lower = content.to_ascii_lowercase();
    BLOCK_TAGS.iter().any(|t| lower.contains(t))
}

/// 去 markdown 围栏：整行 ``` / ```lang / ~~~ 标记丢弃，围栏内正文保留。
/// （模型偶发把正文包进代码围栏，真机 PDF 导出中的大量 ``` 行即此残留。）
fn strip_code_fences(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let t = line.trim();
            !(t.starts_with("```") || t.starts_with("~~~"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 解码常见 HTML 实体（命名 + 数字/十六进制）。未知或残缺实体原样保留。
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let end = after.find(';');
        let token = end.map(|e| &after[..=e]).unwrap_or(after);
        let decoded = match token {
            "&amp;" => Some('&'),
            "&lt;" => Some('<'),
            "&gt;" => Some('>'),
            "&quot;" => Some('"'),
            "&apos;" => Some('\''),
            "&nbsp;" | "&#160;" | "&#xa0;" | "&#XA0;" => Some('\u{00a0}'),
            "&#" => None, // 无编号的残缺实体
            _ => {
                // &#123; / &#x1F; 数字实体
                let body = token
                    .strip_prefix("&#")
                    .and_then(|b| b.strip_suffix(';'))
                    .filter(|b| !b.is_empty());
                match body {
                    Some(b) => {
                        let hex = b
                            .strip_prefix('x')
                            .or_else(|| b.strip_prefix('X'))
                            .map(|h| u32::from_str_radix(h, 16));
                        match hex {
                            Some(Ok(n)) => char::from_u32(n),
                            Some(Err(_)) => None,
                            None => b.parse::<u32>().ok().and_then(char::from_u32),
                        }
                    }
                    None => None,
                }
            }
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &after[token.len()..];
            }
            None => {
                // 未知/残缺实体：保留原 '&'，不吞字
                out.push('&');
                rest = &after[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// 标签名提取：剥掉开闭符与自闭合的 `/`（`<br/>` → `br`）。
fn tag_name(tag_body: &str) -> &str {
    tag_body
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or("")
}

/// HTML → 段落。块级标签开/闭均产生换行；行内标签剥壳；script/style/
/// 注释整块跳过。
fn html_to_paragraphs(html: &str) -> Vec<String> {
    let mut out = String::with_capacity(html.len());
    let lower = html.to_ascii_lowercase();
    let mut i = 0usize; // 字节索引（to_ascii_lowercase 不改变字节布局）
    while i < html.len() {
        let c = html[i..].chars().next().unwrap();
        if c == '<' && i + 1 < html.len() {
            let next = html[i + 1..].chars().next().unwrap();
            if next.is_ascii_alphabetic() || next == '/' || next == '!' {
                let tag_end = match lower[i..].find('>') {
                    Some(off) => i + off,
                    None => {
                        // 残缺标签：按文本处理
                        out.push('<');
                        i += c.len_utf8();
                        continue;
                    }
                };
                let tag = &lower[i..=tag_end];
                // 注释（可能内含 '>'，不能被首个 '>' 截断）
                if tag.starts_with("<!--") {
                    if let Some(rel) = lower[i..].find("-->") {
                        i += rel + 3;
                        continue;
                    }
                    i = tag_end + 1;
                    continue;
                }
                if tag.starts_with("<!") {
                    // DOCTYPE 等
                    i = tag_end + 1;
                    continue;
                }
                let tag_body = tag.trim_start_matches('<').trim_end_matches('>').trim();
                let name = tag_name(tag_body);
                if name == "script" || name == "style" {
                    let close_tag = format!("</{}", name);
                    if let Some(rel) = lower[i + 1..].find(&close_tag) {
                        if let Some(a) = lower[i + 1 + rel..].find('>') {
                            i = i + 1 + rel + a + 1;
                            continue;
                        }
                    }
                    i = tag_end + 1;
                    continue;
                }
                const BLOCK: [&str; 12] = [
                    "p",
                    "div",
                    "h1",
                    "h2",
                    "h3",
                    "h4",
                    "h5",
                    "h6",
                    "li",
                    "blockquote",
                    "pre",
                    "tr",
                ];
                if BLOCK.contains(&name) || name == "br" {
                    out.push('\n');
                }
                i = tag_end + 1;
                continue;
            }
        }
        out.push(c);
        i += c.len_utf8();
    }

    let decoded = decode_entities(&out);
    // &nbsp; 导出为普通空格，避免 PDF/TXT 出现不可断行字符
    let decoded = decoded.replace('\u{00a0}', " ");

    decoded
        .split('\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// 存量正文 → 段落列表（trim、去空段）。导出各格式的统一切分入口。
pub fn content_to_paragraphs(content: &str) -> Vec<String> {
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let fenced = strip_code_fences(&normalized);
    if looks_like_html(&fenced) {
        // HTML 路径：先剥标签后解码（html_to_paragraphs 内部处理）
        html_to_paragraphs(&fenced)
    } else {
        // 纯文本路径：只需解码实体（容错历史转义数据）
        let decoded = decode_entities(&fenced).replace('\u{00a0}', " ");
        decoded
            .split('\n')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    }
}

/// 存量正文 → 以空行连接的纯文本（MD / TXT / 模板上下文）。
pub fn content_to_plain_text(content: &str) -> String {
    content_to_paragraphs(content).join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiptap_html_splits_paragraphs() {
        // 真实存量形态：<p> 连写、无换行
        let content = "<p>崔绫道：\"把袖子空出来。\"</p><p>巡吏上前一步。</p>";
        let paras = content_to_paragraphs(content);
        assert_eq!(paras, vec!["崔绫道：\"把袖子空出来。\"", "巡吏上前一步。"]);
    }

    #[test]
    fn inline_tags_stripped_entities_decoded() {
        let content = "<p><strong>司宇</strong>退半步，&#8220;脚跟&#8221;撞上砖窖沿。&amp;</p>";
        let paras = content_to_paragraphs(content);
        assert_eq!(paras, vec!["司宇退半步，\u{201c}脚跟\u{201d}撞上砖窖沿。&"]);
    }

    #[test]
    fn escaped_lt_gt_in_text_survives() {
        // 正文里被转义的 &lt;p&gt; 是真文本，不能被二次吞掉
        let paras = content_to_paragraphs("<p>账上写 &lt;p&gt; 笔。</p>");
        assert_eq!(paras, vec!["账上写 <p> 笔。"]);
    }

    #[test]
    fn unknown_entity_preserved() {
        let paras = content_to_paragraphs("温度 &notanentity; 保持。");
        assert_eq!(paras, vec!["温度 &notanentity; 保持。"]);
        let paras = content_to_paragraphs("残缺 &#12q 实体。");
        assert_eq!(paras, vec!["残缺 &#12q 实体。"]);
    }

    #[test]
    fn code_fences_removed_but_inner_text_kept() {
        let content = "```\n炉里的拍砖声更急了。\n```\n水从裂口漫到鞋面。";
        let paras = content_to_paragraphs(content);
        assert_eq!(paras, vec!["炉里的拍砖声更急了。", "水从裂口漫到鞋面。"]);
    }

    #[test]
    fn plain_text_fallback_splits_newlines() {
        let paras = content_to_paragraphs("第一段。\n\n第二段。\n");
        assert_eq!(paras, vec!["第一段。", "第二段。"]);
    }

    #[test]
    fn br_and_heading_boundaries() {
        let content = "<h2>第4章</h2><p>他押。<br>她没接。</p><br/><p>水退半寸。</p>";
        let paras = content_to_paragraphs(content);
        assert_eq!(paras, vec!["第4章", "他押。", "她没接。", "水退半寸。"]);
    }

    #[test]
    fn script_and_comment_dropped() {
        let content = "<p>正文。</p><script>alert(1)</script><!-- 注释 --><p>续。</p>";
        let paras = content_to_paragraphs(content);
        assert_eq!(paras, vec!["正文。", "续。"]);
    }

    #[test]
    fn nbsp_becomes_plain_space() {
        let paras = content_to_paragraphs("<p>尺押&nbsp;在坊里。</p>");
        assert_eq!(paras, vec!["尺押 在坊里。"]);
    }

    #[test]
    fn empty_content_yields_no_paragraphs() {
        assert!(content_to_paragraphs("").is_empty());
        assert!(content_to_paragraphs("<p></p><p>  </p>").is_empty());
    }

    #[test]
    fn plain_text_joins_with_blank_line() {
        assert_eq!(content_to_plain_text("<p>a</p><p>b</p>"), "a\n\nb");
    }

    #[test]
    fn crlf_normalized() {
        let paras = content_to_paragraphs("甲。\r\n\r\n乙。\r");
        assert_eq!(paras, vec!["甲。", "乙。"]);
    }
}
