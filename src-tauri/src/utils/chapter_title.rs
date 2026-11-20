//! v0.99.8：章节号与标题的统一格式化。
//!
//! 项目里同时存在「第 134 章」「第12章」「香炉边的银镯与铜钱」
//! 「第二章 铜钱与脚印」等多种形态。训练沙盒/自动取名/幕前展示
//! 需要一个单一规则：
//! - 章号统一用中文数字（第二章）；
//! - 有可读标题时写成「第二章 铜钱与脚印」；
//! - 只有占位章号时写成「第二章」。
//!
//! 解析器同时兼容阿拉伯数字、中文数字、数字与「章」之间的空格，
//! 以及「第N章修订」这类带后缀的 key。

/// 解析后的章节标题。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChapterTitleParts {
    /// 前缀里的章号（`第134章` / `第 134 章` / `第一百三十四章`）。
    pub number: Option<u32>,
    /// `Some`：`章` 后是分隔符/空白或已结束；`None`：`章` 后紧跟正文
    /// （如「第三章的雨」这类真实标题，不能误拆）。
    pub body: Option<String>,
    /// 数字与「章」之间出现过空白（旧训练沙盒标题「第 134 章」）。
    pub spaced_number: bool,
}

/// 标准中文数字（1..=99,999,999）。0 返回「零」。
pub fn chinese_numeral(n: u32) -> String {
    const DIGITS: [&str; 10] = ["零", "一", "二", "三", "四", "五", "六", "七", "八", "九"];
    if n == 0 {
        return "零".to_string();
    }
    if n >= 10_000 {
        let high = n / 10_000;
        let low = n % 10_000;
        let mut out = format!("{}万", chinese_numeral(high));
        if low > 0 {
            if low < 1_000 {
                out.push('零');
            }
            out.push_str(&chinese_numeral(low));
        }
        return out;
    }
    let mut out = String::new();
    let mut remainder = n;
    let mut zero_pending = false;
    for (unit, label) in [(1_000u32, "千"), (100, "百"), (10, "十")] {
        let digit = remainder / unit;
        remainder %= unit;
        if digit > 0 {
            if zero_pending && !out.is_empty() {
                out.push('零');
            }
            // 10 写作「十」而不是「一十」；110 仍写「一百一十」。
            if !(digit == 1 && unit == 10 && out.is_empty()) {
                out.push_str(DIGITS[digit as usize]);
            }
            out.push_str(label);
            zero_pending = false;
        } else if !out.is_empty() && remainder > 0 {
            zero_pending = true;
        }
    }
    if remainder > 0 {
        if zero_pending {
            out.push('零');
        }
        out.push_str(DIGITS[remainder as usize]);
    }
    if out.is_empty() {
        "零".to_string()
    } else {
        out
    }
}

/// 解析「第N章」前缀。识别阿拉伯数字、中文数字与空格。
pub fn parse_chapter_title(raw: &str) -> ChapterTitleParts {
    let trimmed = raw.trim();
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.first() != Some(&'第') {
        return ChapterTitleParts {
            number: None,
            body: Some(trimmed.to_string()),
            spaced_number: false,
        };
    }
    let mut numeral = String::new();
    let mut spaced_number = false;
    let mut chapter_end = None;
    let mut i = 1;
    while i < chars.len() {
        let c = chars[i];
        if c == '章' {
            chapter_end = Some(i);
            break;
        }
        if c.is_whitespace() {
            spaced_number = true;
            i += 1;
            continue;
        }
        if c.is_ascii_digit() || is_cn_numeral_char(c) {
            numeral.push(c);
            i += 1;
            continue;
        }
        break;
    }
    let Some(end) = chapter_end else {
        return ChapterTitleParts {
            number: None,
            body: Some(trimmed.to_string()),
            spaced_number: false,
        };
    };
    let number = parse_number_token(&numeral);
    let rest: String = chars[end + 1..].iter().collect();
    let body = if rest.trim().is_empty() {
        Some(String::new())
    } else if rest.chars().next().map(is_separator).unwrap_or(false) {
        Some(strip_title_separators(&rest))
    } else {
        None
    };
    ChapterTitleParts {
        number,
        body,
        spaced_number,
    }
}

/// 解析章号（`第134章`、`第 134 章`、`第一百三十四章`、`第12章修订`）。
pub fn parse_chapter_number(raw: &str) -> Option<u32> {
    parse_chapter_title(raw).number
}

/// 标题是否只是章号占位（「第12章」「第 134 章」「第二章」）。
pub fn is_generic_chapter_title(raw: &str) -> bool {
    let parts = parse_chapter_title(raw);
    parts.number.is_some() && parts.body.as_deref() == Some("")
}

/// 统一为「第二章 铜钱与脚印」。
///
/// - 已带标题：保留标题里的章号（训练沙盒上下文「第 134 章 魑魅魍魉横行」）；
/// - 纯占位：跟当前 `chapter_number`（修复自动分章后旧库标题过期）；
/// - 裸标题：补当前章号。
pub fn format_chapter_title(chapter_number: u32, raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return format!("第{}章", chinese_numeral(chapter_number));
    }
    let parts = parse_chapter_title(trimmed);
    let cn_number = prefix_uses_chinese_numeral(trimmed);
    let (number, body) = match (parts.number, parts.body) {
        (Some(number), Some(body)) if !body.is_empty() => (number, body),
        // 「第 134 章」是训练沙盒继承的源章号，保留 134；
        // 「第一百三十四章」也视为作者/源标题，不按 sandbox 章号覆盖。
        (Some(number), Some(_)) if parts.spaced_number || cn_number => (number, String::new()),
        // 「第12章」是系统占位，跟当前 chapter_number（分章重排后不照旧标题）。
        (Some(_), Some(_)) => (chapter_number, String::new()),
        // 「第三章的雨」这类章字后紧跟正文的真实标题：整段作为标题。
        _ => (chapter_number, trimmed.to_string()),
    };
    if body.is_empty() {
        format!("第{}章", chinese_numeral(number))
    } else {
        format!("第{}章 {}", chinese_numeral(number), body)
    }
}

/// 从原书章节正文首行提取标题并统一格式化。
///
/// 源章节正文通常是「第134章 魑魅魍魉横行\n…」；训练沙盒过去只写
/// 「第 134 章」，标题正文被丢掉。首行不是章号标题时回退纯章号。
pub fn format_source_chapter_title(fallback_number: u32, content: &str) -> String {
    source_chapter_title(content)
        .unwrap_or_else(|| format!("第{}章", chinese_numeral(fallback_number)))
}

/// 从正文首行提取「第N章 标题」；首行不是章节标题时返回 None。
pub fn source_chapter_title(content: &str) -> Option<String> {
    let first_line = content.lines().find(|line| !line.trim().is_empty())?;
    let parts = parse_chapter_title(first_line);
    match (parts.number, parts.body) {
        (Some(number), Some(body)) if !body.is_empty() => {
            Some(format!("第{}章 {}", chinese_numeral(number), body))
        }
        (Some(number), Some(_)) => Some(format!("第{}章", chinese_numeral(number))),
        _ => None,
    }
}

/// 迁移旧库标题：占位标题优先用正文首行的原书标题，其次按当前章号统一；
/// 已有人物标题则补章号。
pub fn normalize_existing_chapter_title(chapter_number: u32, raw: &str, content: &str) -> String {
    if is_generic_chapter_title(raw) {
        if let Some(source) = source_chapter_title(content) {
            return source;
        }
    }
    format_chapter_title(chapter_number, raw)
}

fn prefix_uses_chinese_numeral(raw: &str) -> bool {
    let mut chars = raw.trim().chars().skip(1);
    for c in chars.by_ref() {
        if c == '章' {
            return false;
        }
        if is_cn_numeral_char(c) {
            return true;
        }
    }
    false
}

/// 剥掉「第N章」前缀，只在章后是分隔符/空白时生效。
pub fn strip_chapter_prefix(raw: &str) -> Option<String> {
    let parts = parse_chapter_title(raw);
    parts
        .body
        .filter(|body| !body.is_empty())
        .map(|body| body.trim().to_string())
}

fn is_separator(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '：' | ':' | '·' | '、' | '-' | '—' | '－' | '）' | ')' | '】' | ']'
        )
}

fn strip_title_separators(s: &str) -> String {
    s.trim_start_matches(is_separator).trim().to_string()
}

fn is_cn_numeral_char(c: char) -> bool {
    matches!(
        c,
        '一' | '二'
            | '三'
            | '四'
            | '五'
            | '六'
            | '七'
            | '八'
            | '九'
            | '十'
            | '百'
            | '千'
            | '万'
            | '零'
            | '〇'
            | '两'
    )
}

fn parse_number_token(token: &str) -> Option<u32> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    if token.chars().all(|c| c.is_ascii_digit()) {
        return token.parse().ok();
    }
    parse_cn_number(token)
}

fn cn_digit(c: char) -> Option<u32> {
    Some(match c {
        '零' | '〇' => 0,
        '一' => 1,
        '二' | '两' => 2,
        '三' => 3,
        '四' => 4,
        '五' => 5,
        '六' => 6,
        '七' => 7,
        '八' => 8,
        '九' => 9,
        _ => return None,
    })
}

fn cn_unit(c: char) -> Option<u32> {
    Some(match c {
        '十' => 10,
        '百' => 100,
        '千' => 1_000,
        '万' => 10_000,
        _ => return None,
    })
}

fn parse_cn_number(token: &str) -> Option<u32> {
    let mut total = 0u32;
    let mut section = 0u32;
    let mut pending = 0u32;
    let mut saw_any = false;
    for c in token.chars() {
        if let Some(d) = cn_digit(c) {
            pending = d;
            saw_any = true;
            continue;
        }
        let unit = cn_unit(c)?;
        saw_any = true;
        if unit == 10_000 {
            let high = if pending == 0 {
                section
            } else {
                section + pending
            };
            total = total.saturating_add(high.max(1).saturating_mul(10_000));
            section = 0;
            pending = 0;
        } else {
            let digit = if pending == 0 { 1 } else { pending };
            section = section.saturating_add(digit.saturating_mul(unit));
            pending = 0;
        }
    }
    if !saw_any {
        return None;
    }
    Some(total.saturating_add(section).saturating_add(pending))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chinese_numerals_cover_common_chapter_numbers() {
        for (n, expected) in [
            (1, "一"),
            (2, "二"),
            (10, "十"),
            (11, "十一"),
            (20, "二十"),
            (23, "二十三"),
            (101, "一百零一"),
            (110, "一百一十"),
            (134, "一百三十四"),
            (1001, "一千零一"),
            (1010, "一千零一十"),
            (1100, "一千一百"),
        ] {
            assert_eq!(chinese_numeral(n), expected, "n={n}");
        }
    }

    #[test]
    fn parses_arabic_chinese_and_spaced_numbers() {
        assert_eq!(parse_chapter_number("第12章"), Some(12));
        assert_eq!(parse_chapter_number("第 134 章"), Some(134));
        assert_eq!(parse_chapter_number("第一百三十四章"), Some(134));
        assert_eq!(parse_chapter_number("第二章 铜钱与脚印"), Some(2));
        assert_eq!(parse_chapter_number("第12章修订"), Some(12));
        assert_eq!(parse_chapter_number("第三章的雨"), Some(3));
        assert_eq!(parse_chapter_number("番外"), None);
    }

    #[test]
    fn formats_titles_as_chinese_number_plus_body() {
        assert_eq!(format_chapter_title(2, "铜钱与脚印"), "第二章 铜钱与脚印");
        assert_eq!(
            format_chapter_title(2, "第二章 铜钱与脚印"),
            "第二章 铜钱与脚印"
        );
        assert_eq!(
            format_chapter_title(13, "香炉边的银镯与铜钱"),
            "第十三章 香炉边的银镯与铜钱"
        );
        assert_eq!(format_chapter_title(134, "第 134 章"), "第一百三十四章");
        assert_eq!(format_chapter_title(12, "第12章"), "第十二章");
        // 分章重排后旧标题过期：阿拉伯占位跟当前章号。
        assert_eq!(format_chapter_title(8, "第6章"), "第八章");
        // 中文占位视为作者/源标题，保留原章号。
        assert_eq!(format_chapter_title(8, "第一章"), "第一章");
        // 训练沙盒继承的源章号不丢。
        assert_eq!(
            format_chapter_title(1, "第 134 章 魑魅魍魉横行"),
            "第一百三十四章 魑魅魍魉横行"
        );
        assert_eq!(format_chapter_title(1, "第一百三十四章"), "第一百三十四章");
    }

    #[test]
    fn source_title_is_extracted_from_first_line() {
        assert_eq!(
            format_source_chapter_title(1, "第134章 魑魅魍魉横行\n哑巴老板捧着烟。"),
            "第一百三十四章 魑魅魍魉横行"
        );
        assert_eq!(
            format_source_chapter_title(135, "第135章 百花\n从小馆子出来。"),
            "第一百三十五章 百花"
        );
        assert_eq!(
            format_source_chapter_title(136, "本地精怪不经夸。"),
            "第一百三十六章"
        );
    }

    #[test]
    fn generic_titles_recognize_spaces_and_chinese_numerals() {
        assert!(is_generic_chapter_title("第12章"));
        assert!(is_generic_chapter_title("第 134 章"));
        assert!(is_generic_chapter_title("第二章"));
        assert!(!is_generic_chapter_title("第二章 铜钱与脚印"));
        assert!(!is_generic_chapter_title("香炉边的银镯与铜钱"));
        assert!(!is_generic_chapter_title("第三章的雨"));
    }

    #[test]
    fn strip_prefix_only_after_separator() {
        assert_eq!(
            strip_chapter_prefix("第3章：临江夜雨"),
            Some("临江夜雨".to_string())
        );
        assert_eq!(strip_chapter_prefix("第三章的雨"), None);
    }
}
