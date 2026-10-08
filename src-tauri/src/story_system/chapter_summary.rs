#![allow(dead_code)]
//! 章节语义摘要（P1-A，v0.61.0）。
//!
//! 此前 `scene_commits.summary_text` 是「正文前 1000 字截断」——既不是摘要，
//! 也不携带状态变化。本模块把 commit 的摘要升级为 LLM 语义摘要（100–150 字：
//! 人物做了什么、结果如何、状态/关系/物品归属变化、伏笔埋设与回收），
//! 失败时回退到截断（保底不阻塞、不产生空摘要）。
//!
//! 后台调用标签复用 `background-summary`（已在 `is_silent_background_label`
//! 名单中，不会向前端发假进度）。

use std::collections::HashMap;

use crate::{db::DbPool, llm::LlmService, router::TaskType};

/// 后台调用标签（静默 + 背景模型角色）
pub const SUMMARY_LABEL: &str = "background-summary";
/// 回退摘要长度（与旧行为一致）
pub const FALLBACK_CHARS: usize = 1000;
/// 送入 prompt 的正文上限
const PROMPT_CONTENT_CHARS: usize = 6000;
/// 摘要可接受的最短 / 最长字符数
const MIN_SUMMARY_CHARS: usize = 15;
const MAX_SUMMARY_CHARS: usize = 500;

/// 回退摘要：正文前 `FALLBACK_CHARS` 字符（旧行为保底）。
pub fn fallback_summary(content: &str) -> String {
    content.chars().take(FALLBACK_CHARS).collect()
}

/// 解析 LLM 摘要输出：剥思考链与代码围栏，校验形态与长度。
///
/// 拒绝：JSON 片段（模型返回了结构化输出）、过短（< 15 字）、
/// 过长（> 500 字，说明没有压缩）。
pub fn parse_summary_response(raw: &str) -> Option<String> {
    let stripped = crate::narrative::strip_reasoning_blocks(raw);
    let mut text = stripped.trim();
    // 去掉 markdown 围栏
    if let Some(rest) = text.strip_prefix("```") {
        let rest = rest
            .strip_prefix("markdown")
            .or_else(|| rest.strip_prefix("md"))
            .or_else(|| rest.strip_prefix("text"))
            .unwrap_or(rest);
        text = rest.trim_start_matches(['\n', '\r']);
        if let Some(end) = text.rfind("```") {
            text = &text[..end];
        }
    }
    let text = text.trim().trim_matches('`').trim();
    if text.is_empty() || text.starts_with('{') || text.starts_with('[') {
        return None;
    }
    let len = text.chars().count();
    if !(MIN_SUMMARY_CHARS..=MAX_SUMMARY_CHARS).contains(&len) {
        return None;
    }
    // 折叠空白（模型常输出换行分段）
    let collapsed = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.is_empty() {
        return None;
    }
    Some(collapsed)
}

/// 构造章节摘要 prompt（资产缺失时用内置兜底）。
pub fn build_prompt(pool: Option<&DbPool>, chapter_number: i32, content: &str) -> String {
    let mut vars = HashMap::new();
    vars.insert("chapter_number".to_string(), chapter_number.to_string());
    vars.insert(
        "content".to_string(),
        content
            .chars()
            .take(PROMPT_CONTENT_CHARS)
            .collect::<String>(),
    );
    let asset = pool
        .and_then(|pool| {
            crate::prompts::registry::resolve_prompt_with_vars(pool, "chapter_summary", &vars).ok()
        })
        .or_else(|| {
            crate::prompts::registry::resolve_prompt_default_with_vars("chapter_summary", &vars)
        });
    asset.unwrap_or_else(|| {
        format!(
            "你是长篇小说编辑。请把下面这一章压缩成 100-150 字的语义摘要，用于后续章节的一致性参照。\n\
             只写关键事实：谁做了什么、结果如何、状态/关系/物品归属发生了什么变化、埋下或回收了什么伏笔；\
             不评价、不剧透、不复制原句、不要 JSON、不要标题，只输出一段摘要正文。\n\n\
             【第{chapter_number}章正文】\n{}",
            content.chars().take(PROMPT_CONTENT_CHARS).collect::<String>()
        )
    })
}

/// 正文指纹（FNV-1a 64 位，十六进制）：用于判断「这条摘要对应的正文有没有变」。
///
/// 比对的是**剥离编辑器标记后的文本**，避免只改 HTML 包裹导致误判重算。
pub fn content_fingerprint(content: &str) -> String {
    let plain = crate::utils::death_text::strip_editor_markup(content);
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in plain.trim().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// 摘要产出质量：决定要不要把「这段摘要其实是截断」记进质量债
/// （v0.64.11：此前回退是静默的，真机第 9 章摘要为空、段摘要 0 行都没人知道）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SummaryQuality {
    /// LLM 产出的语义摘要
    Llm,
    /// LLM 不可用（未配置/未传入）
    NoLlm,
    /// LLM 调用失败
    LlmError(String),
    /// LLM 输出不合格（JSON/过短/过长）
    InvalidOutput(usize),
    /// 正文为空，无可摘要
    EmptyContent,
}

impl SummaryQuality {
    pub fn is_fallback(&self) -> bool {
        !matches!(self, Self::Llm)
    }

    /// 质量债/日志用的中文说明（Llm 级返回 None）。
    pub fn debt_detail(&self, chapter_number: i32) -> Option<String> {
        match self {
            Self::Llm => None,
            Self::NoLlm => Some(format!(
                "第{chapter_number}章摘要未走模型（无可用 LLM），当前存的是正文截断"
            )),
            Self::LlmError(e) => Some(format!(
                "第{chapter_number}章摘要模型调用失败，回退为正文截断：{e}"
            )),
            Self::InvalidOutput(len) => Some(format!(
                "第{chapter_number}章摘要模型输出不合格（{len} 字符），回退为正文截断"
            )),
            Self::EmptyContent => Some(format!(
                "第{chapter_number}章正文为空，摘要未生成——先确认该章是否真的没有内容"
            )),
        }
    }
}

/// 生成章节语义摘要 + 质量；LLM 不可用或输出不合格时回退为截断摘要。
pub async fn summarize_chapter_with_quality(
    pool: Option<&DbPool>,
    content: &str,
    chapter_number: i32,
    llm: Option<&LlmService>,
) -> (String, SummaryQuality) {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return (String::new(), SummaryQuality::EmptyContent);
    }
    let fallback = fallback_summary(trimmed);
    let Some(llm) = llm else {
        return (fallback, SummaryQuality::NoLlm);
    };
    let prompt = build_prompt(pool, chapter_number, trimmed);
    match llm
        .generate_for_task(
            TaskType::Summarization,
            prompt,
            Some(400),
            Some(0.3),
            Some(SUMMARY_LABEL),
        )
        .await
    {
        Ok(response) => match parse_summary_response(&response.content) {
            Some(summary) => (summary, SummaryQuality::Llm),
            None => {
                let len = response.content.chars().count();
                log::warn!(
                    "[chapter_summary] 第{}章摘要输出不合格（{} 字符），回退截断",
                    chapter_number,
                    len
                );
                (fallback, SummaryQuality::InvalidOutput(len))
            }
        },
        Err(e) => {
            log::warn!(
                "[chapter_summary] 第{}章摘要 LLM 失败，回退截断: {}",
                chapter_number,
                e
            );
            (fallback, SummaryQuality::LlmError(e.to_string()))
        }
    }
}

/// 生成章节语义摘要；LLM 不可用或输出不合格时回退为截断摘要。
pub async fn summarize_chapter(
    pool: Option<&DbPool>,
    content: &str,
    chapter_number: i32,
    llm: Option<&LlmService>,
) -> String {
    summarize_chapter_with_quality(pool, content, chapter_number, llm)
        .await
        .0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_is_head_truncation_and_never_empty() {
        let short = "短正文。";
        assert_eq!(fallback_summary(short), "短正文。");
        let long = "字".repeat(2500);
        assert_eq!(fallback_summary(&long).chars().count(), FALLBACK_CHARS);
    }

    #[test]
    fn parse_accepts_plain_summary_and_collapses_blank_lines() {
        let raw = "林晚把羊脂玉佩交给苏亦铁。\n\n两人约定三日后在渡口汇合。";
        let parsed = parse_summary_response(raw).expect("应接受");
        assert!(parsed.contains("羊脂玉佩"));
        assert!(!parsed.contains('\n'), "应折叠为单段: {parsed}");
    }

    #[test]
    fn parse_strips_reasoning_and_fences() {
        let raw =
            "<thinking>先想想怎么写</thinking>```markdown\n他推开门，看见尸体。烛火摇曳。\n```";
        let parsed = parse_summary_response(raw).expect("应接受");
        assert!(parsed.contains("尸体"));
        assert!(!parsed.contains("thinking"));
        assert!(!parsed.contains("```"));
    }

    #[test]
    fn parse_rejects_json_too_short_and_too_long() {
        assert!(parse_summary_response("{\"summary\": \"...\"}").is_none());
        assert!(parse_summary_response("太短").is_none());
        assert!(parse_summary_response(&"字".repeat(600)).is_none());
        assert!(parse_summary_response("").is_none());
    }

    #[test]
    fn quality_reports_debt_for_every_fallback_path() {
        assert!(!SummaryQuality::Llm.is_fallback());
        assert!(SummaryQuality::Llm.debt_detail(9).is_none());
        for q in [
            SummaryQuality::NoLlm,
            SummaryQuality::LlmError("超时".into()),
            SummaryQuality::InvalidOutput(12),
            SummaryQuality::EmptyContent,
        ] {
            let detail = q.debt_detail(9).expect("回退路径必须给出说明");
            assert!(detail.contains("第9章"), "{detail}");
        }
        assert!(SummaryQuality::InvalidOutput(12)
            .debt_detail(9)
            .unwrap()
            .contains("12 字符"));
    }

    #[test]
    fn fingerprint_ignores_markup_but_detects_prose_change() {
        let a = "<p>苏亦铁按住棺沿。</p><p>指节发白。</p>";
        let b = "苏亦铁按住棺沿。\n\n指节发白。";
        assert_eq!(
            content_fingerprint(a),
            content_fingerprint(b),
            "只改 HTML 包裹不应算变"
        );
        let c = "<p>苏亦铁按住棺沿。</p><p>指节雪白。</p>";
        assert_ne!(content_fingerprint(a), content_fingerprint(c));
        assert_eq!(content_fingerprint("").len(), 16);
    }
}
