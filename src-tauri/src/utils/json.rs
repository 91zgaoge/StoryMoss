//! LLM 输出 JSON 的容错工具（v0.98.3）。
//!
//! 推理模型（DeepSeek 等）在结构化任务上常见两种失败：
//! 1. `content` 为空、完整答案留在 `reasoning_content` 思维链里；
//! 2. `content` 在 JSON 中途被截断（输出额度/网关截断）。
//!
//! 这里提供两个纯函数：从文本中取最后一个完整 JSON 对象（供 CoT 提取），
//! 以及补全被截断 JSON 的未闭合括号（供解析器 salvage）。都不做业务校验，
//! 调用方仍需用 serde 反序列化并判断字段是否有实质内容。

use serde_json::Value;

/// 从文本中提取**最后一个**完整且可解析的 JSON 对象。
///
/// 思维链里常有草稿对象、示例对象，最终答案通常在最后；逐个提取后取
/// 最后一个能 `serde_json::from_str::<Value>` 成功的对象。
pub fn extract_last_json_object(raw: &str) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    let mut search_from = 0usize;
    while let Some(rel) = raw[search_from..].find('{') {
        let start = search_from + rel;
        match scan_object_end(raw, start) {
            Some(end) => {
                let candidate = &raw[start..=end];
                if serde_json::from_str::<Value>(candidate).is_ok() {
                    candidates.push(candidate.to_string());
                }
                search_from = end + 1;
            }
            None => search_from = start + 1,
        }
    }
    candidates.pop()
}

/// 补全被截断的 JSON 对象；原本已完整或无法修复时返回 `None`。
///
/// 从第一个 `{` 起跟踪字符串/转义与 `{}`/`[]` 嵌套深度；若到文本末尾仍未
/// 闭合，则：
/// - 仍在字符串内 → 先补 `"`；
/// - 末尾是 `,` → 去掉（尾随逗号非法）；
/// - 末尾是 `:` → 补 `null`；
/// - 按栈逆序补齐 `}` / `]`。
///
/// 补全后必须能解析为 JSON object 才返回，避免把散文/半截数字硬修成
/// 看似合法的垃圾。
pub fn complete_truncated_json(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let text = &raw[start..];
    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    let mut string_is_key = false;
    let mut expect_colon = false;
    let mut last_sig = '\0';

    for ch in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
                if string_is_key {
                    expect_colon = true;
                }
                last_sig = '"';
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                // 对象的键紧随 `{` 或 `,`；数组里的字符串是值。
                string_is_key = stack.last() == Some(&'}') && (last_sig == '{' || last_sig == ',');
                expect_colon = false;
                last_sig = '"';
            }
            '{' => {
                stack.push('}');
                expect_colon = false;
                last_sig = '{';
            }
            '[' => {
                stack.push(']');
                expect_colon = false;
                last_sig = '[';
            }
            '}' | ']' => {
                if stack.last() != Some(&ch) {
                    return None;
                }
                stack.pop();
                expect_colon = false;
                last_sig = ch;
                if stack.is_empty() {
                    // 顶层对象已闭合——原文本身就是完整 JSON，无需修复。
                    return None;
                }
            }
            ':' | ',' => {
                expect_colon = false;
                last_sig = ch;
            }
            c if !c.is_whitespace() => last_sig = c,
            _ => {}
        }
    }

    if stack.is_empty() && !in_string {
        return None;
    }

    let mut out = text.trim_end().to_string();
    if in_string {
        // 末尾若是未完成的转义（`\"` 之前只有 `\`），先补一个反斜杠完成转义。
        if escaped {
            out.push('\\');
        }
        out.push('"');
        if string_is_key {
            // 截断在键名中间：补成 `"部分键": null`，保证对象结构合法。
            out.push_str(":null");
        }
    } else if expect_colon {
        // 键已闭合但还没写冒号/值。
        out.push_str(":null");
    }
    while out.ends_with(|c: char| c.is_whitespace()) {
        out.pop();
    }
    if out.ends_with(',') {
        out.pop();
        while out.ends_with(|c: char| c.is_whitespace()) {
            out.pop();
        }
    } else if out.ends_with(':') {
        out.push_str("null");
    }
    for closer in stack.iter().rev() {
        out.push(*closer);
    }
    let value = serde_json::from_str::<Value>(&out).ok()?;
    value.is_object().then_some(out)
}

/// 从 `start`（必须是 `{` 的字节位置）扫描到配对的 `}`，返回闭括号字节位置。
fn scan_object_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut i = start;
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
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_truncated_nested_object() {
        let raw = r#"{"meta":{"name":"市井江湖"},"vocabulary":{"density":"high","preferred_categories":["市井口语","江湖术语"],"#;
        let fixed = complete_truncated_json(raw).expect("应能补全");
        let value: Value = serde_json::from_str(&fixed).expect("补全后应合法");
        assert_eq!(value["vocabulary"]["preferred_categories"][1], "江湖术语");
        assert!(value["vocabulary"]["density"].is_string());
    }

    #[test]
    fn completes_truncated_key_after_comma() {
        // 真机风格 DNA：`"preferred_categories": [...], "` 处截断（下一个键名
        // 刚开引号）。补成 `"": null` 保证对象合法，未知字段由 serde 忽略。
        let raw = "{\"meta\":{\"name\":\"市井\"},\"vocabulary\":{\"density\":\"high\",\"preferred_categories\":[\"a\",\"b\"],\"";
        let fixed = complete_truncated_json(raw).expect("应能补全截断的键");
        let value: Value = serde_json::from_str(&fixed).unwrap();
        assert_eq!(value["vocabulary"]["preferred_categories"][1], "b");
    }

    #[test]
    fn completes_truncated_string_and_dangling_colon() {
        let fixed = complete_truncated_json(r#"{"a":"未写完的句子"#).expect("应能补全字符串");
        let value: Value = serde_json::from_str(&fixed).unwrap();
        assert_eq!(value["a"], "未写完的句子");

        let fixed = complete_truncated_json(r#"{"a":"#).expect("应能补 null");
        let value: Value = serde_json::from_str(&fixed).unwrap();
        assert!(value["a"].is_null());
    }

    #[test]
    fn complete_object_is_not_repaired() {
        assert!(complete_truncated_json(r#"{"a":1}"#).is_none());
        assert!(complete_truncated_json(r#"前言 {"a":1} 后记"#).is_none());
        assert!(complete_truncated_json("没有 JSON").is_none());
    }

    #[test]
    fn unrepairable_truncation_returns_none() {
        // 截断在裸字面量中间：补 `}` 仍非法。
        assert!(complete_truncated_json(r#"{"a": tru"#).is_none());
    }

    #[test]
    fn extract_last_json_object_prefers_final_answer() {
        let raw = r#"先想一个草稿 {"verdict":"draft"}，最终答案：
            {"verdict":"pass","score":4}"#;
        let json = extract_last_json_object(raw).expect("应提取最后一个对象");
        let value: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["verdict"], "pass");
        assert_eq!(value["score"], 4);
    }

    #[test]
    fn extract_last_json_object_ignores_unbalanced_draft() {
        let raw = r#"草稿 {"a": {"b": 1} 还没闭合，最后 {"ok":true}"#;
        let json = extract_last_json_object(raw).expect("应跳过未闭合草稿");
        assert_eq!(json, r#"{"ok":true}"#);
    }
}
