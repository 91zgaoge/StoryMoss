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
    normalize_summary_text(text)
}

/// 摘要正文形态校验（`parse_summary_response` 与合并调用共用）：
/// 非空、非 JSON、15..=500 字，并折叠为单段。
fn normalize_summary_text(text: &str) -> Option<String> {
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

// =============================================================================
// v0.73.0：提交摘要 + mini review 合并为一次 LLM 调用（chapter_commit_digest）
// =============================================================================
//
// 此前每章提交固定两次 LLM：`chapter_summary`（400 tok）与 `mini_review`
// （800 tok），两个提示词吃的是同一段正文与同一套合同变量，只是产出不同字段。
// 合并为一次 JSON 调用（summary + 评分维度 + issues），happy path 省掉一次
// 串行后台 LLM；解析失败/接口异常则**完整回退**到原来的两步（行为与旧版一致），
// 连续失败 3 次后本会话不再尝试（避免在不适配的模型上反复多花一次调用）。

/// 合并调用标签：含 `background` 关键字 → Background（后台）档 + 静默后台。
pub const COMMIT_DIGEST_LABEL: &str = "background-commit-digest";
/// 合并调用的输出预算（摘要 150 字 + 评审 JSON，旧两步合计 1200）。
const DIGEST_MAX_TOKENS: i32 = 1600;
/// 连续解析/调用失败达到此值 → 本会话停用合并路径（回退即旧行为）。
const DIGEST_FAIL_STREAK_LIMIT: u32 = 3;

/// 连续失败计数（进程级；成功即清零）。
static DIGEST_FAIL_STREAK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// 合并路径是否仍可用（测试/诊断用）。
pub fn commit_digest_available() -> bool {
    DIGEST_FAIL_STREAK.load(std::sync::atomic::Ordering::Relaxed) < DIGEST_FAIL_STREAK_LIMIT
}

/// 测试辅助：重置失败计数（进程级状态需跨用例隔离）。
pub fn reset_commit_digest_streak() {
    DIGEST_FAIL_STREAK.store(0, std::sync::atomic::Ordering::Relaxed);
}

/// 合并输出：摘要正文 + 评审字段（与 `mini_review::ReviewResult` 对齐）。
#[derive(Debug, serde::Deserialize)]
struct CommitDigestPayload {
    summary: String,
    score: f64,
    #[serde(default)]
    dimensions: Vec<crate::story_system::mini_review::ReviewDimension>,
    #[serde(default)]
    review_summary: String,
    #[serde(default)]
    issues: Vec<String>,
}

/// 构造合并提示词：合同变量与 `mini_review_system` 同源，输出结构同时含
/// 摘要与评审。资产缺失时用内置兜底（与两步各自的内置兜底同口气）。
pub fn build_digest_prompt(
    pool: Option<&DbPool>,
    chapter_number: i32,
    content: &str,
    contract: &crate::domain::contracts::RuntimeContract,
) -> String {
    let truncated: String = content.chars().take(PROMPT_CONTENT_CHARS).collect();
    let mut vars = crate::story_system::mini_review::build_prompt_vars(&truncated, contract);
    vars.insert("chapter_number".to_string(), chapter_number.to_string());
    let asset = pool
        .and_then(|pool| {
            crate::prompts::registry::resolve_prompt_with_vars(pool, "chapter_commit_digest", &vars)
                .ok()
        })
        .or_else(|| {
            crate::prompts::registry::resolve_prompt_default_with_vars(
                "chapter_commit_digest",
                &vars,
            )
        });
    asset.unwrap_or_else(|| {
        let digest = |key: &str| vars.get(key).cloned().unwrap_or_default();
        format!(
            "你是长篇小说编辑。阅读下面这一章正文，一次完成两件事并以 JSON 输出：\n\
             （1）压成 100-150 字的语义摘要（谁做了什么、结果如何、状态/关系/物品归属的变化、埋下或回收的伏笔）；\n\
             （2）按合同快速评估（score 0.0-1.0、dimensions 维度分、review_summary 一两句总评、issues 具体问题）。\n\
             只输出 JSON，不要 markdown 代码块、不要解释：\n\
             {{\"summary\":\"…\",\"score\":0.82,\"dimensions\":[{{\"name\":\"合同目标达成\",\"score\":0.9,\"comment\":\"…\"}}],\"review_summary\":\"…\",\"issues\":[\"…\"]}}\n\n\
             【题材】{}\n【基调】{}\n【节奏策略】{}\n【世界观规则】\n{}\n【本章目标】{}\n【必须覆盖节点】{}\n【禁忌区】{}\n\n\
             【第{}章正文】\n{}",
            digest("genre"),
            digest("core_tone"),
            digest("pacing_strategy"),
            digest("world_rules"),
            digest("chapter_goal"),
            digest("must_cover_nodes"),
            digest("forbidden_zones"),
            chapter_number,
            truncated,
        )
    })
}

/// 解析合并输出：剥思考链，取首 `{` 到末 `}`，校验摘要形态 + 归一评审分数。
/// 任一部分不合格返回 None（调用方回退到旧两步）。
pub fn parse_commit_digest(
    raw: &str,
) -> Option<(String, crate::story_system::mini_review::ReviewResult)> {
    let stripped = crate::narrative::strip_reasoning_blocks(raw);
    let start = stripped.find('{')?;
    let end = stripped.rfind('}')?;
    if end <= start {
        return None;
    }
    let payload: CommitDigestPayload = serde_json::from_str(&stripped[start..=end]).ok()?;
    let summary = normalize_summary_text(&payload.summary)?;
    let review = crate::story_system::mini_review::clamp_review(
        crate::story_system::mini_review::ReviewResult {
            score: payload.score,
            dimensions: payload.dimensions,
            summary: payload.review_summary,
            issues: payload.issues,
        },
    );
    Some((summary, review))
}

/// 合并调用主入口：成功返回 (摘要, 质量, 评审)；任何异常返回 None，由
/// `auto_commit` 回退到原来的两步（摘要 + mini_review 各自独立调用）。
pub async fn summarize_and_review_chapter(
    pool: Option<&DbPool>,
    content: &str,
    chapter_number: i32,
    contract: &crate::domain::contracts::RuntimeContract,
    llm: Option<&LlmService>,
) -> Option<(
    String,
    SummaryQuality,
    crate::story_system::mini_review::ReviewResult,
)> {
    use std::sync::atomic::Ordering;
    if content.trim().is_empty() || !commit_digest_available() {
        return None;
    }
    let llm = llm?;
    let prompt = build_digest_prompt(pool, chapter_number, content, contract);
    let response = match llm
        .generate_for_task(
            TaskType::Summarization,
            prompt,
            Some(DIGEST_MAX_TOKENS),
            Some(0.3),
            Some(COMMIT_DIGEST_LABEL),
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let streak = DIGEST_FAIL_STREAK.fetch_add(1, Ordering::Relaxed) + 1;
            log::warn!(
                "[commit_digest] 第{}章合并调用失败（第 {} 次连续失败），回退两步：{}",
                chapter_number,
                streak,
                e
            );
            return None;
        }
    };
    match parse_commit_digest(&response.content) {
        Some((summary, review)) => {
            DIGEST_FAIL_STREAK.store(0, Ordering::Relaxed);
            log::info!(
                "[commit_digest] 第{}章摘要+评审合并为一次调用（摘要 {} 字，评分 {:.2}）",
                chapter_number,
                summary.chars().count(),
                review.score
            );
            Some((summary, SummaryQuality::Llm, review))
        }
        None => {
            let streak = DIGEST_FAIL_STREAK.fetch_add(1, Ordering::Relaxed) + 1;
            log::warn!(
                "[commit_digest] 第{}章合并输出不合格（第 {} 次连续失败，{} 字符），回退两步",
                chapter_number,
                streak,
                response.content.chars().count()
            );
            None
        }
    }
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

    // ---- v0.73.0：摘要 + mini review 合并调用（chapter_commit_digest） ----

    const DIGEST_OK: &str = r#"```json
{
  "summary": "苏福贵把匣子推到案上，景亲王没看匣只看人；兵符暂守住了，府权却被夺去两成。江顾然奉命全权查办此案，三日内要给朝廷一个答复。",
  "score": 0.82,
  "dimensions": [
    {"name": "合同目标达成", "score": 0.9, "comment": "覆盖主要节点"},
    {"name": "世界规则一致", "score": 0.8, "comment": "未发现违反"}
  ],
  "review_summary": "整体推进有力，对白简练。",
  "issues": ["灯油价钱的细节略打断节奏"]
}
```"#;

    #[test]
    fn commit_digest_parses_summary_and_review_together() {
        let (summary, review) = parse_commit_digest(DIGEST_OK).expect("合并输出应解析成功");
        assert!(summary.contains("匣子"), "{summary}");
        assert!(!summary.contains('\n'), "摘要应折叠为单段");
        assert!((review.score - 0.82).abs() < 1e-9, "score={}", review.score);
        assert_eq!(review.dimensions.len(), 2);
        assert_eq!(review.summary, "整体推进有力，对白简练。");
        assert_eq!(review.issues.len(), 1);
    }

    #[test]
    fn commit_digest_strips_reasoning_and_fences() {
        let raw = format!("<thinking>先想想</thinking>\n{DIGEST_OK}");
        let (summary, _) = parse_commit_digest(&raw).expect("应剥思考链");
        assert!(summary.contains("匣子"));
        assert!(!summary.contains("thinking"));
    }

    #[test]
    fn commit_digest_rejects_bad_shapes_and_clamps_scores() {
        // 摘要过短
        assert!(parse_commit_digest(r#"{"summary":"太短","score":0.5}"#).is_none());
        // 缺 score（必填字段）→ serde 失败
        assert!(parse_commit_digest(
            r#"{"summary":"这是一段足够长的摘要正文，用来通过长度校验。"}"#
        )
        .is_none());
        // 非 JSON
        assert!(parse_commit_digest("这不是 JSON").is_none());
        // 分数越界 → 钳到 0..=1
        let (_, review) = parse_commit_digest(
            r#"{"summary":"这是一段足够长的摘要正文，用来通过长度校验。","score":9.0,
                "dimensions":[{"name":"一致","score":-3.0,"comment":""}]}"#,
        )
        .expect("应解析并钳分");
        assert_eq!(review.score, 1.0);
        assert_eq!(review.dimensions[0].score, 0.0);
    }

    #[test]
    fn commit_digest_prompt_carries_schema_and_contract_vars() {
        // 用 JSON 构造合同（字段随 schema 演进也不会让本测试失修）。
        let contract: crate::domain::contracts::RuntimeContract =
            serde_json::from_value(serde_json::json!({
                "master_setting": {
                    "schema_version": "story-system/v1",
                    "contract_type": "MASTER_SETTING",
                    "generator_version": "v0",
                    "genre": "古代权谋",
                    "core_tone": "冷峻",
                    "pacing_strategy": "前紧后缓",
                    "anti_patterns": [],
                    "world_rules": ["血桉剧毒，唯赤雪牛可解"]
                },
                "chapter_contract": {
                    "schema_version": "story-system/v1",
                    "contract_type": "CHAPTER",
                    "generator_version": "v0",
                    "chapter_number": 3,
                    "chapter_directive": {
                        "goal": "守住兵符",
                        "must_cover_nodes": ["景亲王验符"],
                        "forbidden_zones": ["不得复活已死角色"],
                        "time_anchor": null,
                        "chapter_span": null
                    }
                }
            }))
            .expect("合同反序列化");
        let prompt = build_digest_prompt(None, 3, "苏福贵把匣子推到案上。", &contract);
        assert!(prompt.contains("\"summary\""), "必须给出 summary 字段契约");
        assert!(
            prompt.contains("\"dimensions\""),
            "必须给出 dimensions 字段契约"
        );
        assert!(prompt.contains("第3章正文"), "必须带章节号");
        assert!(prompt.contains("血桉剧毒"), "必须带世界观规则");
        assert!(prompt.contains("守住兵符"), "必须带本章目标");
        assert!(!prompt.contains("{{"), "变量必须全部渲染（无残留占位符）");
    }

    #[test]
    fn commit_digest_streak_gates_availability() {
        use std::sync::atomic::Ordering;
        reset_commit_digest_streak();
        assert!(commit_digest_available());
        for _ in 0..DIGEST_FAIL_STREAK_LIMIT {
            DIGEST_FAIL_STREAK.fetch_add(1, Ordering::Relaxed);
        }
        assert!(!commit_digest_available(), "连续失败达上限后应停用合并路径");
        reset_commit_digest_streak();
        assert!(commit_digest_available(), "重置后恢复");
        reset_commit_digest_streak();
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
