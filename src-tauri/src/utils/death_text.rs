//! 死亡文本判定（纯文本，零依赖）：从 `agency::continue_assets` 下沉，
//! 供 db 层（V142 存量回填）与 story_system（生死状态）共用，保证判定只有一份。
//!
//! v0.64.7：《帝国的烟火》第 2 章写死的明成公主在第 10 章被续写成活人——
//! 判定本身没错，错在死亡只按章末窗口临时推断、从未落库。

/// 幕前编辑器传 HTML。提示词必须用纯正文，否则 800/2400 字预算会被标签吃掉。
pub fn strip_editor_markup(text: &str) -> String {
    let raw = text.trim();
    if raw.is_empty() {
        return String::new();
    }
    if !raw.contains('<') && !raw.contains('&') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    let mut tag = String::new();
    for c in raw.chars() {
        if c == '<' {
            in_tag = true;
            tag.clear();
            continue;
        }
        if in_tag {
            if c == '>' {
                in_tag = false;
                let t = tag.trim().trim_start_matches('/').to_ascii_lowercase();
                let name = t.split(|ch: char| ch.is_whitespace()).next().unwrap_or("");
                if matches!(
                    name,
                    "p" | "div" | "br" | "h1" | "h2" | "h3" | "li" | "tr" | "blockquote"
                ) {
                    out.push('\n');
                }
            } else {
                tag.push(c);
            }
            continue;
        }
        out.push(c);
    }
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");
    let mut collapsed = decoded;
    while collapsed.contains("\n\n\n") {
        collapsed = collapsed.replace("\n\n\n", "\n\n");
    }
    collapsed.trim().to_string()
}

pub fn prose_has_completed_death(text: &str) -> bool {
    [
        "的尸体",
        "气绝",
        "身亡",
        "崩裂开来",
        "化为白骨",
        "白森森的头骨",
    ]
    .iter()
    .any(|m| text.contains(m))
}

pub(crate) fn titles_of(name: &str) -> Vec<&'static str> {
    const TITLES: &[&str] = &["公主", "亲王", "王爷", "将军", "夫人", "郡主"];
    TITLES
        .iter()
        .copied()
        .filter(|t| name.ends_with(t))
        .collect()
}

fn sentence_negates_death(sent: &str) -> bool {
    sent.contains("未气绝")
        || sent.contains("没有死")
        || sent.contains("假死")
        || sent.contains("诈死")
}

fn window_before(sent: &str, marker_byte: usize, max_chars: usize) -> &str {
    let before = &sent[..marker_byte];
    let start = before
        .char_indices()
        .rev()
        .nth(max_chars.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    &before[start..]
}

/// 近文是否已把该角色写成不可逆死亡（尸体 / 气绝 / 头骨崩裂）。
/// 同句里出现别人的尸体不算；称号（公主）只在全文已出现全名、
/// 且该人是受击对象时算死。
pub fn name_is_dead_in_text(name: &str, text: &str) -> bool {
    let name = name.trim();
    if name.is_empty() || name.chars().count() < 2 {
        return false;
    }
    if text.contains(&format!("{name}的尸体")) {
        return true;
    }
    const BODY_MARKERS: &[&str] = &["崩裂开来", "化为白骨", "白森森的头骨", "头骨上"];
    for sent in text.split(['。', '！', '？', '\n']) {
        if sentence_negates_death(sent) {
            continue;
        }
        for m in BODY_MARKERS {
            if let Some(idx) = sent.find(m) {
                if window_before(sent, idx, 40).contains(name) {
                    return true;
                }
            }
        }
        let last_breath = sent.find("气绝").or_else(|| sent.find("身亡"));
        if let Some(idx) = last_breath {
            if window_before(sent, idx, 12).contains(name) {
                return true;
            }
            if sent.contains(&format!("击中{name}")) || sent.contains(&format!("{name}横飞")) {
                return true;
            }
            for t in titles_of(name) {
                if sent.contains(&format!("击中{t}"))
                    || sent.contains(&format!("打得{t}"))
                    || (sent.contains(t) && sent.contains("将其打得"))
                {
                    return true;
                }
            }
        }
    }
    false
}

pub fn dead_names_in_text(names: &[impl AsRef<str>], text: &str) -> Vec<String> {
    names
        .iter()
        .map(|n| n.as_ref().trim().to_string())
        .filter(|n| !n.is_empty() && name_is_dead_in_text(n, text))
        .collect()
}
