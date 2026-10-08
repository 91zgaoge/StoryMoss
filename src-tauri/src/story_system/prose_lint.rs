#![allow(dead_code)]
//! 确定性文本质检（P2-A，v0.62.0）。
//!
//! 对照外部项目经验（oh-story 的 check-ai-patterns、ainovel 的 style_stats）：
//! LLM 评审对「章均几十次的句式 tic」与「流水线术语泄漏」天然失明，只有确定性
//! 规则能在每次 commit 时低成本地兜住这些形态。
//!
//! 两档：
//! - **blocking**：注入术语泄漏（【必须改变】【本拍】等 prompt
//!   头直接出现在正文）、 「不是 A 而是
//!   B」否定排比、章尾总结/预告腔——命中即应在修订中处理；
//! - **advisory**：破折号密度、章内逐字重复句、章尾极短句收束、开篇时间跳跃——
//!   供编辑器审计参考，不强制改写。
//!
//! 规则纪律（来自 oh-story 的教训）：规则必须能被真实语料校准。本模块只放
//! 误报率低、可解释的形态；全部是纯函数，便于单测与后续语料校准。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LintFinding {
    /// 规则 id（稳定标识，便于统计与忽略）
    pub rule: String,
    /// blocking | advisory
    pub severity: String,
    /// 人类可读说明
    pub detail: String,
    /// 命中片段（截断，供定位）
    pub excerpt: String,
}

impl LintFinding {
    fn new(rule: &str, severity: &str, detail: &str, excerpt: &str) -> Self {
        Self {
            rule: rule.to_string(),
            severity: severity.to_string(),
            detail: detail.to_string(),
            excerpt: excerpt.chars().take(40).collect(),
        }
    }
}

/// 正文中绝不该出现的注入术语（prompt 头与流水线词汇）。
/// 第一档：带方括号的注入头（模型抄了 prompt 结构）；第二档：流程词。
const PIPELINE_HEADERS: &[&str] = &[
    "【必须改变",
    "【本章节拍任务】",
    "【本拍状态网】",
    "【本拍信息差",
    "【在场物品",
    "【故事纲要",
    "【已推进进度",
    "【待回收伏笔",
    "【逾期伏笔",
    "【角色当前状态】",
    "【故事大纲",
    "【本章大纲",
    "【前文】",
    "【时间线",
];

const PIPELINE_TERMS: &[&str] = &[
    "节拍卡",
    "必须改变项",
    "change_delta",
    "expansion_quota",
    "字数目标",
    "章首钩子",
    "爽点密度",
    "细纲",
];

/// 章尾总结/预告腔（oh-story 的 trailer-ending / trailer-summary）
const TRAILER_PHRASES: &[&str] = &[
    "这才刚刚开始",
    "才刚刚开始",
    "一切才刚刚开始",
    "而这，只是开始",
    "而这只是开始",
    "殊不知",
    "然而他不知道",
    "然而她不知道",
    "然而他们不知道",
    "他却不知道",
    "她却不知道",
    "命运的齿轮",
    "没有人知道，",
    "无人知晓，",
];

/// 开篇时间跳跃词（advisory）
const OPENING_TIME_WORDS: &[&str] = &[
    "第二天",
    "次日",
    "翌日",
    "三日后",
    "三天后",
    "一个月后",
    "半年后",
    "一年后",
    "三年后",
    "多年后",
    "数日后",
    "几日后",
];

const MIN_DUPLICATE_SENTENCE_CHARS: usize = 12;
const SHORT_ENDING_CHARS: usize = 8;
const MIN_TEXT_FOR_ENDING_CHECK: usize = 600;
/// 破折号密度阈值（每千字）
const EM_DASH_PER_1000_LIMIT: f64 = 3.0;

/// 句末标点切分（保留标点便于展示）。
fn split_sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | '…' | '；') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
            current.clear();
        }
    }
    let tail = current.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

fn last_nonempty_paragraph(text: &str) -> Option<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .next_back()
}

/// 确定性文本质检入口。
pub fn lint_text(text: &str) -> Vec<LintFinding> {
    let mut findings: Vec<LintFinding> = Vec::new();
    if text.trim().is_empty() {
        return findings;
    }

    // 1) 注入术语泄漏（blocking）
    for header in PIPELINE_HEADERS {
        if let Some(pos) = text.find(header) {
            let excerpt: String = text[pos..].chars().take(30).collect();
            findings.push(LintFinding::new(
                "pipeline-header-leak",
                "blocking",
                &format!("正文出现注入术语「{header}」——prompt 结构被抄进了正文"),
                &excerpt,
            ));
            break;
        }
    }
    for term in PIPELINE_TERMS {
        if let Some(pos) = text.find(term) {
            let excerpt: String = text[pos..].chars().take(30).collect();
            findings.push(LintFinding::new(
                "pipeline-term-leak",
                "blocking",
                &format!("正文出现流水线术语「{term}」"),
                &excerpt,
            ));
            break;
        }
    }

    // 2) 「不是 A 而是 B」否定排比（聚集才报，v0.65.0 校准）
    //
    // sepia 汇总的中文实测：人类语料 30% 的文章含该形态（6.3/10 万字），
    // 机器是 2.9–7.5 倍率——**单例是正常语域，聚集才是信号**。旧版命中即
    // blocking 会误伤正常行文，且「过度纠正本身是新的指纹」。
    let sentences = split_sentences(text);
    let not_x_but_y: Vec<&String> = sentences
        .iter()
        .filter(|s| (s.contains("不是") || s.contains("并非")) && s.contains("而是"))
        .collect();
    if not_x_but_y.len() >= 2 {
        findings.push(LintFinding::new(
            "not-x-but-y",
            "blocking",
            &format!(
                "「不是…而是…」否定排比 {} 处（单例属正常语域，聚集即为 AI 腔高发形态）",
                not_x_but_y.len()
            ),
            not_x_but_y[1],
        ));
    }

    // 3) 章尾总结/预告腔（blocking）
    if let Some(last) = last_nonempty_paragraph(text) {
        for phrase in TRAILER_PHRASES {
            if last.contains(phrase) {
                findings.push(LintFinding::new(
                    "trailer-ending",
                    "blocking",
                    &format!("章尾出现总结/预告腔「{phrase}」"),
                    last,
                ));
                break;
            }
        }
        // 4) 章尾极短句收束（advisory）
        if text.chars().count() >= MIN_TEXT_FOR_ENDING_CHECK {
            if let Some(last_sentence) = split_sentences(last).last() {
                let core: String = last_sentence
                    .chars()
                    .filter(|c| !"。！？…；".contains(*c))
                    .collect();
                if !core.is_empty() && core.chars().count() < SHORT_ENDING_CHARS {
                    findings.push(LintFinding::new(
                        "short-ending",
                        "advisory",
                        "章尾以极短句收束（易形成套路化节奏）",
                        last_sentence,
                    ));
                }
            }
        }
    }

    // 5) 破折号密度（advisory）
    let char_count = text.chars().count();
    if char_count >= 500 {
        let dashes = text.matches("——").count();
        let density = dashes as f64 * 1000.0 / char_count as f64;
        if density > EM_DASH_PER_1000_LIMIT {
            findings.push(LintFinding::new(
                "em-dash-density",
                "advisory",
                &format!("破折号密度 {density:.1}/千字（阈值 {EM_DASH_PER_1000_LIMIT}）"),
                "",
            ));
        }
    }

    // 6) 章内逐字重复句（advisory）
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for sentence in &sentences {
        let core = sentence.trim_end_matches(['。', '！', '？', '…', '；']);
        if core.chars().count() >= MIN_DUPLICATE_SENTENCE_CHARS {
            *counts.entry(core).or_insert(0) += 1;
        }
    }
    if let Some((repeated, _)) = counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .max_by_key(|(sentence, _)| sentence.chars().count())
    {
        findings.push(LintFinding::new(
            "duplicate-sentence",
            "advisory",
            "同一句在正文中逐字重复",
            repeated,
        ));
    }

    // 7) 开篇时间跳跃（advisory）
    let opening: String = text.trim().chars().take(60).collect();
    for word in OPENING_TIME_WORDS {
        if opening.contains(word) {
            findings.push(LintFinding::new(
                "opening-time-jump",
                "advisory",
                &format!("开篇以时间跳跃「{word}」起句（易形成套路化开场）"),
                &opening,
            ));
            break;
        }
    }

    findings
}

/// 仅保留 blocking 档（用于阻断性判断，例如是否需要进入修订）。
pub fn blocking_findings(findings: &[LintFinding]) -> Vec<&LintFinding> {
    findings
        .iter()
        .filter(|f| f.severity == "blocking")
        .collect()
}

/// 渲染给编辑器审计的核对块；无发现时返回 None（不产生空块）。
pub fn render_lint_block(findings: &[LintFinding], max: usize) -> Option<String> {
    if findings.is_empty() {
        return None;
    }
    let lines: Vec<String> = findings
        .iter()
        .take(max)
        .map(|f| {
            if f.excerpt.is_empty() {
                format!("- [{}] {}", f.severity, f.detail)
            } else {
                format!("- [{}] {}｜命中：{}", f.severity, f.detail, f.excerpt)
            }
        })
        .collect();
    Some(format!(
        "【确定性文本质检（机器检出，逐条核查；blocking 需在裁决中体现）】\n{}",
        lines.join("\n")
    ))
}

/// 供 commit 记录落库的紧凑文本（每条一行，最多 8 条）。
pub fn summarize_findings(findings: &[LintFinding]) -> Vec<String> {
    findings
        .iter()
        .take(8)
        .map(|f| format!("[文本质检/{}] {}｜{}", f.severity, f.rule, f.detail))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_pipeline_header_leak_as_blocking() {
        let text = "他把刀收回鞘里。\n【必须改变：信息 — 夜宴破裂】\n他转身离开。";
        let findings = lint_text(text);
        assert!(
            findings
                .iter()
                .any(|f| f.rule == "pipeline-header-leak" && f.severity == "blocking"),
            "{findings:?}"
        );
    }

    #[test]
    fn flags_pipeline_term_without_brackets() {
        let findings = lint_text("这一段要写足字数目标，再补一个爽点密度。");
        assert!(findings.iter().any(|f| f.rule == "pipeline-term-leak"));
    }

    #[test]
    fn flags_not_x_but_y_only_when_clustered() {
        // v0.65.0 校准：单例是人类语料的正常语域（人类 30% 的文章含此形态）
        let single = lint_text("他不是害怕，而是终于明白了。");
        assert!(
            !single.iter().any(|f| f.rule == "not-x-but-y"),
            "单例不得报: {single:?}"
        );

        // 聚集（≥2 处）才是信号
        let clustered = lint_text("他不是害怕，而是终于明白了。这不是偶然，而是必然。");
        assert!(
            clustered
                .iter()
                .any(|f| f.rule == "not-x-but-y" && f.severity == "blocking"),
            "{clustered:?}"
        );

        let clean = lint_text("他不是害怕。他只是累了。");
        assert!(!clean.iter().any(|f| f.rule == "not-x-but-y"));
    }

    #[test]
    fn flags_trailer_ending_only_at_tail() {
        let tail = "他推开门，走了出去。这一切才刚刚开始。";
        let findings = lint_text(tail);
        assert!(findings.iter().any(|f| f.rule == "trailer-ending"));

        let middle =
            "这一切才刚刚开始。\n三天后，他推开门，走了出去，风灌进领口，他缩了缩脖子，把门带上。";
        let middle_findings = lint_text(middle);
        assert!(
            !middle_findings.iter().any(|f| f.rule == "trailer-ending"),
            "章中不判 trailer: {middle_findings:?}"
        );
    }

    #[test]
    fn reports_advisory_density_rules_without_false_blocking() {
        let body = "他站着。".repeat(40);
        let with_dashes = format!("{body}——{body}——{body}——{body}——{body}");
        let findings = lint_text(&with_dashes);
        assert!(findings.iter().any(|f| f.rule == "em-dash-density"));
        assert!(
            !blocking_findings(&findings)
                .iter()
                .any(|f| f.rule == "em-dash-density"),
            "密度只应是 advisory"
        );
    }

    #[test]
    fn detects_repeated_sentence_within_chapter() {
        let text = "雨落在青石板上，声音很轻。他站着不动。雨落在青石板上，声音很轻。";
        let findings = lint_text(text);
        assert!(findings.iter().any(|f| f.rule == "duplicate-sentence"));
    }

    #[test]
    fn clean_literary_text_produces_no_blocking() {
        let text = "她把玉佩收进袖中，指腹在温润的边角上停了一瞬。\n\
                    窗外有卖花人经过，吆喝声被雨声切碎。她没有抬头。\n\
                    过了很久，她才低声说：走吧。";
        let findings = lint_text(text);
        assert!(
            blocking_findings(&findings).is_empty(),
            "文学性文本不得误报 blocking: {findings:?}"
        );
    }

    #[test]
    fn render_block_and_summary_are_bounded() {
        let findings = lint_text("他不是害怕，而是明白了。这一切才刚刚开始。");
        let block = render_lint_block(&findings, 10).expect("应有块");
        assert!(block.contains("确定性文本质检"));
        let summary = summarize_findings(&findings);
        assert!(!summary.is_empty() && summary.len() <= 8);
        assert!(render_lint_block(&[], 10).is_none());
    }
}
