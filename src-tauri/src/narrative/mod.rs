//! Narrative Element Model — 统一叙事元素模型
//!
//! 核心理念：无论正向生成（Bootstrap/创世）还是逆向分析（拆书），
//! 操作的叙事元素是同一套抽象。
//!
//! 模块结构：
//! - elements: 统一数据模型（CharacterElement, SceneElement 等）
//! - pipeline: Pipeline trait 和通用基础设施
//! - prompts: 统一 Prompt 模板（生成/提取两用）
//! - genesis: GenesisPipeline — 正向/创世流程
//! - analysis: AnalysisPipeline — 逆向/分析流程
//! - progress: 统一进度事件系统
//! - audit: StoryStructureAuditor — 故事结构审计
//! - health: StoryHealthAnalyzer — 故事健康检查
//! - event: 叙事事件模型（LitSeg E1）
//! - thread: 叙事线索追踪模型（LitSeg E1）
//! - structure: 叙事结构定位模型（LitSeg E1）
//! - segment: 叙事感知分段模型（LitSeg E1）

pub mod analysis;
pub mod audit;
pub mod chunker;
pub mod elements;
pub mod event;
pub mod genesis;
pub mod health;
pub mod intensity_mapper;
pub mod litseg_pipeline;
pub mod pipeline;
pub mod progress;
pub mod prompts;
pub mod search;
pub mod segment;
pub mod structure;
pub mod structure_analyzer;
pub mod thread;
pub mod thread_tracker;

/// 剥离推理模型在正文前输出的「思考链」块。
///
/// 部分本地推理模型（DeepSeek-R1/Qwen3 系微调、MN-Oblivion、Gemma 3 等）会在
/// 真正的答案前输出一段 `<think>...</think>` 或 `<thinking>...</thinking>`
/// 包裹的 chain-of-thought。这段思考里经常出现花括号（如 "用 {} 格式表示"、
/// "return {}"），若不剥离，[`extract_first_json_object`] 会把第一个 `{}`
/// 当成 JSON 对象提取出来 → serde 反序列化报 "missing field" 错误。
///
/// 只剥离**配对**的块。未闭合的标签（有开标签但无闭标签）保持原样：此时
/// 真正的 JSON 可能就出现在「未闭合思考」之后，留给后续括号匹配去发现。
///
/// 注意：标签以字节数组构造，避免在源码里出现完整标签字面量被工具链误处理。
pub(crate) fn strip_reasoning_blocks(content: &str) -> String {
    // <think>, </think>, <thinking>, </thinking>
    const THINK_OPEN: &[u8] = b"\x3c\x74\x68\x69\x6e\x6b\x3e";
    const THINK_CLOSE: &[u8] = b"\x3c\x2f\x74\x68\x69\x6e\x6b\x3e";
    const THINKING_OPEN: &[u8] = b"\x3c\x74\x68\x69\x6e\x6b\x69\x6e\x67\x3e";
    const THINKING_CLOSE: &[u8] = b"\x3c\x2f\x74\x68\x69\x6e\x6b\x69\x6e\x67\x3e";

    let mut text = content.to_string();
    for (open, close) in [(THINK_OPEN, THINK_CLOSE), (THINKING_OPEN, THINKING_CLOSE)] {
        let open_s = std::str::from_utf8(open).unwrap();
        let close_s = std::str::from_utf8(close).unwrap();
        loop {
            let Some(start) = text.find(open_s) else {
                break;
            };
            let after_open = start + open_s.len();
            let Some(rel) = text[after_open..].find(close_s) else {
                break;
            };
            let end = after_open + rel + close_s.len();
            text.replace_range(start..end, "");
        }
    }
    text
}

/// 用括号匹配从 LLM 响应中提取第一个完整的 JSON 对象。
///
/// 遍历字符，跟踪花括号深度（`{` +1, `}` -1），同时跳过字符串字面量
/// （`"..."` 内的 `{`/`}` 不计入深度）。当深度回到 0 时，即为 JSON 对象边界。
/// 这样即使 LLM 在 JSON 后输出包含 `}` 的额外文本，也不会误提取。
///
/// 会跳过空对象 `{}` / `{ }`：它们从不承载所需字段，通常来自思考链或前导
/// 文本里的杂散花括号。抓到空对象只会让 serde 报 "missing field"。真实
/// JSON 对象至少含一个键值对（`:`），据此跳过空候选继续向后扫描。
fn extract_first_json_object(content: &str) -> Result<&str, String> {
    let bytes = content.as_bytes();
    let mut search_from = 0usize;

    loop {
        let start = content[search_from..]
            .find('{')
            .map(|p| search_from + p)
            .ok_or_else(|| "No JSON object found in response".to_string())?;

        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escaped = false;
        let mut i = start;
        let mut closed = false;

        while i < bytes.len() {
            let ch = bytes[i] as char;
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
            } else if ch == '"' {
                in_string = true;
            } else if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    closed = true;
                    break;
                }
            }
            i += 1;
        }

        if !closed {
            // v0.23.53: 未闭合的对象（可能是思考链里的草稿片段），跳过继续找下一个。
            search_from = start + 1;
            continue;
        }

        let candidate = &content[start..=i];
        // 候选去掉首尾花括号后若不含 `:`，说明是 `{}` / `{ }` 这类空对象，
        // 不可能是有效答案，跳过继续找下一个 `{`。
        let inner = &candidate[1..candidate.len().saturating_sub(1)];
        if inner.contains(':') {
            return Ok(candidate);
        }
        search_from = i + 1;
    }
}

/// 从 LLM 响应中提取 JSON 对象，并修复常见语法错误（尾随逗号、空值、markdown
/// 围栏等）
pub fn extract_and_sanitize_json(content: &str) -> Result<String, String> {
    // 0. 剥离推理模型的思考链块（önh... / <thinking>...</thinking>）。
    //    思考链里的花括号会被括号匹配误当成 JSON 对象，必须先移除。 v0.23.53:
    //    但有些推理模型把 JSON 写在思考链内部，剥离后反而丢失。
    //    因此先尝试剥离后的内容，失败则回退到原始内容（从思考链内提取 JSON）。
    let stripped = strip_reasoning_blocks(content);
    let raw = extract_first_json_object(&stripped).or_else(|_| {
        // 剥离后无 JSON，回退到原始内容（JSON 可能在思考链内部）
        extract_first_json_object(content)
    })?;

    // 2. 移除 markdown 代码围栏标记（```json ... ```）
    let mut s = raw.to_string();
    for fence in ["```json", "```JSON", "```", "`"] {
        s = s.replace(fence, "");
    }

    // 3. 移除 UTF-8 BOM 和控制字符
    s = s.trim().to_string();
    s = s.replace('\u{feff}', "");

    // 4. 修复字符串内的未转义换行符和回车符（LLM 经常在 JSON 字符串值中直接换行）
    // 使用状态机：仅在字符串内部替换实际换行符为 \n
    {
        let mut result = String::with_capacity(s.len());
        let mut in_string = false;
        let mut escaped = false;
        for ch in s.chars() {
            if in_string {
                if escaped {
                    escaped = false;
                    result.push(ch);
                } else if ch == '\\' {
                    escaped = true;
                    result.push(ch);
                } else if ch == '"' {
                    in_string = false;
                    result.push(ch);
                } else if ch == '\n' {
                    result.push_str("\\n");
                } else if ch == '\r' {
                    // 跳过 \r，因为 \r\n 已经被处理为 \\n
                } else {
                    result.push(ch);
                }
            } else {
                if ch == '"' {
                    in_string = true;
                }
                result.push(ch);
            }
        }
        s = result;
    }

    // 5. 移除 C 风格注释（// 和 /* */）—— LLM 有时会在 JSON 中插入注释
    {
        let mut result = String::with_capacity(s.len());
        let mut in_string = false;
        let mut escaped = false;
        let mut chars = s.chars().peekable();
        while let Some(ch) = chars.next() {
            if in_string {
                if escaped {
                    escaped = false;
                    result.push(ch);
                } else if ch == '\\' {
                    escaped = true;
                    result.push(ch);
                } else if ch == '"' {
                    in_string = false;
                    result.push(ch);
                } else {
                    result.push(ch);
                }
            } else {
                if ch == '"' {
                    in_string = true;
                    result.push(ch);
                } else if ch == '/' && chars.peek() == Some(&'/') {
                    // 跳过单行注释
                    chars.next(); // skip second /
                    while let Some(c) = chars.next() {
                        if c == '\n' {
                            result.push('\n'); // 保留换行以保持行号
                            break;
                        }
                    }
                } else if ch == '/' && chars.peek() == Some(&'*') {
                    // 跳过多行注释
                    chars.next(); // skip *
                    while let Some(c) = chars.next() {
                        if c == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                } else {
                    result.push(ch);
                }
            }
        }
        s = result;
    }

    // 6. 修复尾随逗号：`,]` → `]` 和 `,}` → `}`
    let mut prev;
    loop {
        prev = s.clone();
        s = s.replace(",]", "]");
        s = s.replace(",}", "}");
        s = s.replace(", ]", "]");
        s = s.replace(", }", "}");
        if s == prev {
            break;
        }
    }

    // 7. 修复空值：`: ,` → `: null,`，`: ]` → `: null]`，`: }` → `: null}`
    for (bad, good) in [
        (": ,", ": null,"),
        (":,", ": null,"),
        (": ]", ": null]"),
        (": }", ": null}"),
        (":}", ": null}"),
    ] {
        s = s.replace(bad, good);
    }

    // 注意：不要替换中文引号「」『』为 ASCII 引号，这会破坏 JSON 字符串结构
    // 如果 JSON 键名或值边界使用了中文引号，那是 LLM 的格式错误，应由 LLM 修正

    Ok(s)
}

/// v0.23.55: 正则兜底提取故事概念字段。
///
/// 当 serde_json::from_str 因 JSON 语法错误（如字符串值内未转义双引号）失败时，
/// 用正则表达式逐字段提取 `StoryMetaElement` 的关键字段。容错性强，不依赖严格
/// JSON 语法。
///
/// 提取策略：匹配 `"key": "value"` 或 `"key": [array]` 模式，值取到下一个未转义
/// `"` 或行尾。
pub fn extract_story_meta_fallback(json_str: &str) -> Option<crate::domain::StoryMetaElement> {
    use crate::domain::{ElementSource, StoryMetaElement};

    // 提取字符串字段值：匹配 "key" : "value"，value 取到下一个未转义的 "
    fn extract_string(json: &str, key: &str) -> Option<String> {
        // 构造模式：匹配 "key" 后面的字符串值
        let pattern = format!(r#""{}"\s*:\s*"((?:[^"\\]|\\.)*)"#, regex::escape(key));
        regex::Regex::new(&pattern)
            .ok()
            .and_then(|re| re.captures(json))
            .and_then(|c| c.get(1))
            .map(|m| {
                // 反转义 \n, \", \\ 等
                m.as_str()
                    .replace("\\n", "\n")
                    .replace("\\\"", "\"")
                    .replace("\\\\", "\\")
            })
    }

    // 提取数组字段：匹配 "key" : [ "v1", "v2", ... ]
    fn extract_array(json: &str, key: &str) -> Vec<String> {
        let pattern = format!(r#""{}"\s*:\s*\[(.*?)\]"#, regex::escape(key));
        if let Some(re) = regex::Regex::new(&pattern).ok() {
            if let Some(c) = re.captures(json) {
                let arr_str = c.get(1).map(|m| m.as_str()).unwrap_or("");
                let item_re = regex::Regex::new(r#""((?:[^"\\]|\\.)*)""#).ok();
                if let Some(item_re) = item_re {
                    return item_re
                        .captures_iter(arr_str)
                        .filter_map(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                        .collect();
                }
            }
        }
        Vec::new()
    }

    let title = extract_string(json_str, "title")?;
    if title.trim().is_empty() {
        return None;
    }

    Some(StoryMetaElement {
        id: String::new(),
        title,
        description: extract_string(json_str, "description").unwrap_or_default(),
        genre: extract_string(json_str, "genre").unwrap_or_default(),
        genre_profile_ids: extract_array(json_str, "genre_profile_ids"),
        tone: extract_string(json_str, "tone").unwrap_or_default(),
        pacing: extract_string(json_str, "pacing").unwrap_or_default(),
        themes: extract_array(json_str, "themes"),
        target_length: extract_string(json_str, "target_length").unwrap_or_default(),
        source: ElementSource::Generated,
        source_ref_id: None,
    })
}

/// v0.23.66: 从自然语言散文中提取故事概念字段。
///
/// 部分本地量化模型无视"只输出 JSON"指令，以自然语言返回概念（如
/// "标题：《荒星纪元》\n简介：...\n题材：科幻末世..."）。
/// 本函数用中文/英文关键词匹配逐字段提取，作为 JSON 解析失败后的最后兜底。
pub fn extract_story_meta_from_prose(text: &str) -> Option<crate::domain::StoryMetaElement> {
    use crate::domain::{ElementSource, StoryMetaElement};

    /// 从文本中按中文/英文标签提取单行字符串值。
    /// 匹配模式：标签后跟冒号（中/英），然后取冒号后到行尾/下一标签的内容。
    fn extract_field(text: &str, labels: &[&str]) -> Option<String> {
        let text_lower = text.to_lowercase();
        for label in labels {
            let label_lower = label.to_lowercase();
            // 中英文冒号
            for sep in [":", "："] {
                let prefix = format!("{}{}", label_lower, sep);
                if let Some(pos) = text_lower.find(&prefix) {
                    let start = pos + prefix.len();
                    let rest = &text[start..];
                    // 取到行尾或下一明显标签
                    let end = rest.find('\n').unwrap_or(rest.len());
                    let mut val = rest[..end].trim().to_string();
                    // 去除包裹的引号、书名号
                    val = val
                        .trim_matches(|c: char| {
                            c == '"'
                                || c == '\''
                                || c == '《'
                                || c == '》'
                                || c == '【'
                                || c == '】'
                        })
                        .to_string();
                    // 去除 markdown 加粗标记
                    val = val.replace("**", "").replace('*', "");
                    val = val.trim().to_string();
                    if !val.is_empty() && val.len() < 200 {
                        return Some(val);
                    }
                }
            }
        }
        None
    }

    /// 提取行内《书名号》包裹的标题
    fn extract_title_fallback(text: &str) -> Option<String> {
        if let Some(start) = text.find('《') {
            if let Some(end) = text[start + 3..].find('》') {
                let title = &text[start + 3..start + 3 + end];
                if !title.is_empty() && title.len() < 100 {
                    return Some(title.to_string());
                }
            }
        }
        None
    }

    /// 提取逗号/顿号分隔的主题列表
    fn extract_themes_fallback(text: &str) -> Vec<String> {
        let raw = extract_field(text, &["主题", "themes", "核心主题"]);
        raw.map(|s| {
            s.split(|c: char| c == '、' || c == '，' || c == ',')
                .map(|t| {
                    t.trim()
                        .trim_matches(|c: char| c == '"' || c == '\'')
                        .to_string()
                })
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default()
    }

    // 按优先级尝试提取标题：标签 > 书名号 > 首行
    let title = extract_field(
        text,
        &["标题", "书名", "小说名", "title", "作品名", "故事标题"],
    )
    .or_else(|| extract_title_fallback(text))
    .or_else(|| {
        // 最后兜底：取第一行非空文本（前提是它看起来像标题：短于 30
        // 字且不含常见字段标签）
        text.lines()
            .find(|l| {
                let t = l.trim();
                !t.is_empty()
                    && t.len() > 3
                    && t.len() < 30
                    && !t.contains("标题")
                    && !t.contains("简介")
                    && !t.contains("题材")
                    && !t.contains("故事")
                    && !t.contains("小说")
            })
            .map(|l| {
                l.trim()
                    .trim_matches(|c: char| c == '#' || c == ' ')
                    .to_string()
            })
    })?;

    if title.is_empty() || title.len() > 100 {
        return None;
    }

    let description = extract_field(
        text,
        &[
            "简介",
            "一句话简介",
            "描述",
            "description",
            "概要",
            "故事简介",
        ],
    )
    .unwrap_or_default();

    let genre =
        extract_field(text, &["题材", "类型", "genre", "类别", "小说类型"]).unwrap_or_default();

    let tone =
        extract_field(text, &["基调", "文风", "tone", "风格", "文风基调"]).unwrap_or_default();

    let pacing = extract_field(text, &["节奏", "pacing", "叙事节奏"]).unwrap_or_default();

    let themes = extract_themes_fallback(text);

    let target_length = extract_field(
        text,
        &[
            "篇幅",
            "长度",
            "target_length",
            "预计篇幅",
            "字数",
            "目标字数",
        ],
    )
    .unwrap_or_default();

    log::warn!(
        "[extract_story_meta_from_prose] 从散文提取成功: title={}, genre={}",
        title,
        genre
    );

    Some(StoryMetaElement {
        id: String::new(),
        title,
        description,
        genre,
        genre_profile_ids: vec![],
        tone,
        pacing,
        themes,
        target_length,
        source: ElementSource::Generated,
        source_ref_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_with_trailing_text_containing_braces() {
        // LLM 返回 JSON 后附带额外文本，其中包含 } 字符
        let content = r#"{
  "title": "异星末世",
  "description": "一句话简介",
  "genre": "科幻",
  "tone": "暗黑",
  "pacing": "快节奏",
  "themes": ["生存", "希望"],
  "target_length": "长篇100万字"
}

## 详细说明
故事背景设定在 {年份: 2087} 的未来世界。
角色设定包含 {name: "主角"} 等属性。
"#;
        let result = extract_and_sanitize_json(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "异星末世");
        assert_eq!(parsed["target_length"], "长篇100万字");
    }

    #[test]
    fn test_extract_json_with_markdown_fence_and_trailing() {
        let content =
            "```json\n{\"title\": \"test\", \"genre\": \"科幻\"}\n```\n\n这是额外说明文字";
        let result = extract_and_sanitize_json(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "test");
    }

    #[test]
    fn test_extract_json_brace_in_string_value() {
        // JSON 字符串值中包含 } 字符
        let content = r#"{"title": "test } end", "genre": "科幻"}额外文本}"#;
        let result = extract_and_sanitize_json(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "test } end");
    }

    #[test]
    fn test_extract_json_nested_objects() {
        let content = r#"{"meta": {"title": "test"}, "genre": "科幻"}"#;
        let result = extract_and_sanitize_json(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["meta"]["title"], "test");
    }

    #[test]
    fn test_extract_json_no_object() {
        let result = extract_and_sanitize_json("没有 JSON 的纯文本");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_json_after_reasoning_think_block() {
        // 推理模型（如 MN-Oblivion-26B）在 JSON 前输出 <think>...</think>
        // 思考链，思考链里含 {} 花括号，此前会被当成 JSON 对象提取出空 {} →
        // serde "missing field 'title' at line 1 column 2"
        let think_open = std::str::from_utf8(b"\x3c\x74\x68\x69\x6e\x6b\x3e").unwrap();
        let think_close = std::str::from_utf8(b"\x3c\x2f\x74\x68\x69\x6e\x6b\x3e").unwrap();
        let content = format!(
            "{open}我需要为「异星末世生存」生成故事概念。\n输出格式用 {{}} 包裹 JSON 对象。\n先想标题……用 {{年份: 2087}} 设定背景。\n{close}{{\n  \"title\": \"锈蚀纪元\",\n  \"description\": \"幸存者在锈蚀星球上寻找最后的净水\",\n  \"genre\": \"科幻\",\n  \"tone\": \"沉重\",\n  \"pacing\": \"跌宕起伏\",\n  \"themes\": [\"生存\", \"希望\"],\n  \"target_length\": \"长篇100万字\"\n}}",
            open = think_open,
            close = think_close
        );
        let result = extract_and_sanitize_json(&content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "锈蚀纪元");
        assert_eq!(parsed["genre"], "科幻");
    }

    #[test]
    fn test_extract_json_after_reasoning_angle_thinking() {
        // <thinking>...</thinking> 形式的思考链
        let content = "<thinking>\n用户想要异星末世题材。返回 {} 即可。\n</thinking>\n{\"title\": \"X\", \"genre\": \"科幻\"}";
        let result = extract_and_sanitize_json(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "X");
    }

    #[test]
    fn test_extract_json_skips_leading_empty_object() {
        // 前导文本里的 {}（非思考链场景）也应被跳过，找到真正的 JSON
        let content = "占位 {} 然后 {\"title\": \"真\", \"genre\": \"科幻\"}";
        let result = extract_and_sanitize_json(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "真");
    }

    #[test]
    fn test_extract_json_inside_reasoning_block_fallback() {
        // v0.23.53: 推理模型把 JSON 写在思考链内部，剥离后无 JSON → 回退到原始内容提取
        let think_open = std::str::from_utf8(b"\x3c\x74\x68\x69\x6e\x6b\x3e").unwrap();
        let think_close = std::str::from_utf8(b"\x3c\x2f\x74\x68\x69\x6e\x6b\x3e").unwrap();
        let content = format!(
            "{open}让我构思一下故事概念。\n{{\n  \"title\": \"星尘废墟\",\n  \"description\": \"人类在异星废墟中求生\",\n  \"genre\": \"科幻\"\n}}\n好的就这样。\n{close}以上就是我的构思。",
            open = think_open,
            close = think_close
        );
        let result = extract_and_sanitize_json(&content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "星尘废墟");
    }

    #[test]
    fn test_extract_json_skips_unclosed_draft_in_reasoning() {
        // v0.23.53: 思考链里有未闭合的 JSON 草稿片段，应跳过找到真正完整的 JSON
        let content =
            "前导文本 {\"title\": \"草稿\" 未闭合\n然后 {\"title\": \"真\", \"genre\": \"科幻\"}";
        let result = extract_and_sanitize_json(content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["title"], "真");
    }

    #[test]
    fn test_extract_story_meta_fallback_malformed_json() {
        // v0.23.55: JSON 字符串值内有未转义双引号，serde 会失败，正则兜底提取
        let malformed = r#"{
  "title": "异星末世",
  "description": "主角说"快跑"然后逃离",
  "genre": "科幻",
  "tone": "暗黑",
  "pacing": "快节奏",
  "themes": ["生存", "希望"],
  "target_length": "长篇100万字"
}"#;
        // serde 应该失败
        assert!(serde_json::from_str::<serde_json::Value>(malformed).is_err());
        // 正则兜底应成功
        let meta = extract_story_meta_fallback(malformed).unwrap();
        assert_eq!(meta.title, "异星末世");
        assert_eq!(meta.genre, "科幻");
        assert_eq!(meta.tone, "暗黑");
        assert_eq!(meta.themes, vec!["生存", "希望"]);
    }

    // =========================================================================
    // v0.23.66: extract_story_meta_from_prose 测试
    // =========================================================================

    #[test]
    fn test_prose_chinese_labels() {
        // 模拟 MN-Oblivion-26B 典型的自然语言输出格式
        let prose = r#"好的，我为您构思一个异星末日生存拓荒的故事概念。

标题：《荒星纪元》
简介：在人类最后的殖民地，一群拓荒者面对未知星球的残酷生存挑战。
题材：科幻末世
基调：沉重中带着希望
节奏：慢热，逐步展开世界观
主题：生存、人性、希望、拓荒
篇幅：长篇100万字"#;

        let meta = extract_story_meta_from_prose(prose).unwrap();
        assert_eq!(meta.title, "荒星纪元");
        assert!(meta.description.contains("拓荒者"));
        assert!(meta.genre.contains("科幻"));
        assert!(meta.tone.contains("沉重"));
        assert_eq!(meta.themes.len(), 4);
        assert!(meta.themes.contains(&"生存".to_string()));
    }

    #[test]
    fn test_prose_english_labels() {
        // 部分模型混用中英文标签
        let prose = "Title: Alien Dawn\nDescription: A survival story on a hostile planet.\ngenre: Sci-Fi\ntone: Dark\npacing: fast\n主题: survival, hope";

        let meta = extract_story_meta_from_prose(prose).unwrap();
        assert_eq!(meta.title, "Alien Dawn");
        assert_eq!(meta.genre, "Sci-Fi");
        assert_eq!(meta.tone, "Dark");
        assert_eq!(meta.pacing, "fast");
    }

    #[test]
    fn test_prose_title_in_book_marks() {
        // 书名号提取
        let prose = "我推荐的故事叫《星海迷途》，这是一个关于星际探索的故事。题材：科幻冒险";

        let meta = extract_story_meta_from_prose(prose).unwrap();
        assert_eq!(meta.title, "星海迷途");
        assert!(meta.genre.contains("科幻"));
    }

    #[test]
    fn test_prose_with_thinking_block() {
        // 思考链内的内容被 strip_reasoning_blocks 处理后，
        // extract_story_meta_from_prose 接收的是去除了 <thinking> 的文本
        let prose = r#"<thinking>用户想要一个末世故事...</thinking>

标题：《末日纪元》
简介：核战后的废土世界，幸存者挣扎求生。
题材：末世
基调：暗黑"#;

        let meta = extract_story_meta_from_prose(prose).unwrap();
        assert_eq!(meta.title, "末日纪元");
        assert_eq!(meta.genre, "末世");
    }

    #[test]
    fn test_prose_minimal() {
        // 最少信息——只要有标题就能创建故事
        let prose = "标题：异星末日\n简介：末日星球上的生存故事";

        let meta = extract_story_meta_from_prose(prose).unwrap();
        assert_eq!(meta.title, "异星末日");
        assert!(meta.description.contains("末日"));
        // 未提供的字段应为空
        assert!(meta.genre.is_empty());
    }

    #[test]
    fn test_prose_no_title() {
        // 没有标题 → 返回 None
        let prose = "这是一个关于末世的故事，题材是科幻，基调暗黑沉重。";
        assert!(extract_story_meta_from_prose(prose).is_none());
    }
}

// pub use elements::*;
// pub use pipeline::*;
// pub use progress::*;
