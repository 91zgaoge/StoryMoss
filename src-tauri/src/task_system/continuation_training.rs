//! 续写训练执行器（v0.88.0）
//!
//! 定时任务：取原书若干章 → 灌入独立沙盒故事 → 用真实章节生成风格 DNA →
//! 走既有 Agency 续写管线写「下一章」→ 取真实后续章作基准 → LLM 对比评审
//! （文笔/情节/人物/节奏/伏笔五维）→ 报告落 `continuation_training_reports`
//! （V157）→ 把上一轮的改进建议注入本轮续写指令（自动闭环）。
//!
//! 沙盒故事标题带 `[训练]` 前缀；每轮新建，同书只保留最近 3 个，更早的
//! 连同其数据级联清理（story_repository 的删除事务）。
//!
//! 源支持两类：
//! - `mcp`：通过已连接的 MCP 服务器（如栗子读书 legado）按章取正文。
//!   `chapter_tool` 为空时自动选择章节正文工具；`args_template` 为空时按 工具
//!   JSON Schema 自动生成 `$book`/`$index`/开关参数。`$book` 会在执行时 经 MCP
//!   `get_book` 解析为源系统的 book_url（失败回退书名）。
//! - `file`：本地 epub/txt/pdf，复用
//!   `book_deconstruction::parser::parse_book`。

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use async_trait::async_trait;
use chrono::Local;
use serde::{Deserialize, Serialize};

use super::{
    executor::{spawn_cancel_monitor, TaskExecutionContext, TaskExecutor},
    models::{Task, TaskResult, TaskType},
};
use crate::db::{repositories::story_repository::StoryRepository, DbPool};

/// 单章灌入沙盒的最大字符数（超长截断；12k 与语料分段口径一致）
const MAX_CHAPTER_CHARS: usize = 12_000;
/// 同书保留的最近沙盒故事数
const KEEP_SANDBOXES: usize = 3;
/// 风格分析采样上限（字符）
const STYLE_SAMPLE_CHARS: usize = 12_000;
/// 训练判分前正文的最低实质字数（低于此值视为空稿/拒绝话术）
const MIN_GENERATED_CHARS: usize = 300;
/// 上下文默认章数（灌入沙盒的前文）
const DEFAULT_CONTEXT_CHAPTERS: usize = 10;
/// 续写默认章数（v0.98.0：训练要连续续写 10 章）
const DEFAULT_CONTINUATION_CHAPTERS: usize = 10;
/// 上下文/续写章数上限：防止单次训练把整本书塞进一个任务
const MAX_CONTEXT_CHAPTERS: usize = 30;
const MAX_CONTINUATION_CHAPTERS: usize = 20;
/// 编审逐章对比时每章原文/续写的采样上限（控制单次提示词体积）
const REVIEW_CHAPTER_CHARS: usize = 6_000;
/// 编审总评时每章开头/结尾摘录上限（跨章走势分析用）
const SYNTH_HEAD_CHARS: usize = 220;
const SYNTH_TAIL_CHARS: usize = 220;
/// 上一轮报告维度均分低于该值（10 分制）时，本轮自动加码对应修正
const FEEDBACK_THRESHOLD: f64 = 6.0;

/// v0.99.0：训练写作红线。真机 1118 报告：跨章复制/原地踏步、文风从写实
/// 江湖漂移到玄幻惊悚、自创「引魂蛆/骨刺/阴脉起手式」体系。跨章复制已由
/// 管线串稿修复；这里约束模型不自创体系、不堆砌身体反应、每章必须推进。
/// 语体不写死某一本书的标签，统一以【文风锚点】/风格 DNA 为准。
/// 放在指令前部，确保章纲规划（只读前 3200 字）也能看到。
const TRAINING_REDLINES: &str = "【原作基调与设定红线（最高优先级；违反即判定该章不合格）】\n\
1. 设定边界：只沿用前文真实原文已经出现的术法、门派、行话与器物；以【前情提要】里的「已确立设定白名单」为唯一合法名单。禁止新增前文未建立的修炼/力量体系、异变器官、超自然规则等体系性设定，禁止超能力对轰。\n\
2. 语体边界：严格遵守【文风锚点】与风格 DNA 的语体、句式长度、对白比例与生活细节密度；保持主角原有的口吻与松弛感，对白要有身份感与生活质感；禁止翻译腔与西幻舶来词，禁止用「令人作呕的酥麻感/钻心剧痛」等身体反应堆砌替代动作与对话。\n\
3. 推进边界：每章必须推进【前情提要】中的至少一条未结线索（或在正文中写明本章暂不收的理由），并制造至少一个可验证的状态变化（信息/关系/目标/风险/情绪）；不得整章重复上一章的地点、动作与对白，不得原地踏步。\n\
4. 承接边界：承接锚点与上一章结尾只用于对齐现场与人物状态，禁止复制、改写或复述锚点原文；本章第一段必须写在锚点最后一句话之后发生的新动作/新信息。\n\
5. 设定白名单：只有【前情提要】的「已确立设定白名单」才算「已有设定」；沙盒里 AI 自己新造的名词（异变器官、法器、神秘组织等）不算既有设定，后续章节不得沿用、放大或写成规则。\n\
6. 长程边界：本批续写必须整体推进【前情提要】的总脉络与未结线索，不得另起新故事、新主角、新委托；每章只能在不偏离前情提要的方向上制造新事件。";

/// 训练阶段显式枚举：进度不再是散落的 5/20/30/35/40/75/90/100 常量，
/// 而是每个阶段唯一的进度/step/文案。新增阶段必须在这里登记，保证单调。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrainingStage {
    Fetch,
    Sandbox,
    Style,
    Assets,
    Continue,
    Compare,
    Review,
    Report,
    Done,
}

impl TrainingStage {
    fn progress(self) -> i32 {
        match self {
            TrainingStage::Fetch => 5,
            TrainingStage::Sandbox => 20,
            TrainingStage::Style => 30,
            TrainingStage::Assets => 35,
            TrainingStage::Continue => 40,
            TrainingStage::Compare => 72,
            TrainingStage::Review => 85,
            TrainingStage::Report => 92,
            TrainingStage::Done => 100,
        }
    }

    fn step(self) -> &'static str {
        match self {
            TrainingStage::Fetch => "fetch",
            TrainingStage::Sandbox => "sandbox",
            TrainingStage::Style => "style",
            TrainingStage::Assets => "assets",
            TrainingStage::Continue => "continue",
            TrainingStage::Compare => "compare",
            TrainingStage::Review => "review",
            TrainingStage::Report => "report",
            TrainingStage::Done => "done",
        }
    }

    fn label(self) -> &'static str {
        match self {
            TrainingStage::Fetch => "正在获取原书章节…",
            TrainingStage::Sandbox => "正在创建训练沙盒故事…",
            TrainingStage::Style => "正在从真实章节学习文风…",
            TrainingStage::Assets => "正在重建世界观/大纲/角色/故事线…",
            TrainingStage::Continue => "正在按原文风格连续续写…",
            TrainingStage::Compare => "正在取真实后续章并逐章对比…",
            TrainingStage::Review => "编审 Agent 正在汇总跨章分析并出质量报告…",
            TrainingStage::Report => "正在保存训练报告…",
            TrainingStage::Done => "训练完成",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TrainingPayload {
    pub book_title: String,
    /// 灌入沙盒的上下文（前文）章数
    #[serde(default = "default_chapter_count")]
    pub chapter_count: usize,
    /// 本轮连续续写的章数（v0.98.0；默认 10）
    #[serde(default = "default_continuation_chapters")]
    pub continuation_chapters: usize,
    /// 随机取章时用作上界（不知道总章数时给个估计值即可）
    #[serde(default)]
    pub est_total_chapters: usize,
    /// random | fixed（start_chapter 起连取）；默认 fixed 从头取
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub start_chapter: Option<usize>,
    /// v0.99.2：反馈模式。`truth_hints`（默认，允许注入上一轮“真实后续节点”）/
    /// `blind`（盲测：过滤含真实后续节点的建议，避免用原书后续给 writer
    /// 提示）。
    #[serde(default)]
    pub feedback_mode: Option<String>,
    /// v0.99.3：prompt 模式 `full`（默认）/`compact`（本地小模型）；compact
    /// 缩 brief/ledger/progress 与资产预算，降低本地 GPU OOM。
    #[serde(default)]
    pub prompt_mode: Option<String>,
    #[serde(default)]
    pub source: SourceSpec,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceSpec {
    Mcp {
        server_id: String,
        /// 章节正文工具名（留空时自动选择）
        chapter_tool: String,
        /// 参数模板：值为字符串时 `$book`/`$index` 被替换；留空时按工具 schema
        /// 自动生成。`$index` 单独出现时转为数值。
        #[serde(default)]
        args_template: serde_json::Map<String, serde_json::Value>,
    },
    File {
        path: String,
    },
}

impl Default for SourceSpec {
    fn default() -> Self {
        SourceSpec::File {
            path: String::new(),
        }
    }
}

fn default_chapter_count() -> usize {
    DEFAULT_CONTEXT_CHAPTERS
}

fn default_continuation_chapters() -> usize {
    DEFAULT_CONTINUATION_CHAPTERS
}

/// v0.93.16: 续写训练的上下文切片。`chapters` 的后 `truth_n` 位是「真实后续
/// 基准章」，只用于评分，绝不能进入续写指令——旧版 `build_style_anchor` 取末
/// 两章，把基准章结尾泄漏给模型（训练失真，且模型可直接照抄答案）。
/// v0.98.0：基准章可以是多章（默认 10 章），上下文 = 前 `len - truth_n` 项
/// （execute 内直接切片，基准章永不进入 writer 指令）。
///
/// 取文本末尾 `limit` 字符（按字符数，不按字节）。
fn tail_chars(text: &str, limit: usize) -> String {
    let start = text
        .char_indices()
        .nth_back(limit.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    text[start..].trim().to_string()
}

/// v0.93.5: 风格 DNA 失败时的降级文风锚点——取最后 2 个**上下文**章的
/// 末尾各 ~600 字（合计 ≤1200），让模型直接模仿原文的遣词、句式、对话
/// 风格与细节密度，而不是只收到一句抽象的「文风一致」。
fn build_style_anchor(context: &[Chapter]) -> String {
    const PER_CHAPTER_CHARS: usize = 600;
    let mut parts: Vec<String> = Vec::new();
    for (_, content) in context.iter().rev().take(2).rev() {
        parts.push(tail_chars(content, PER_CHAPTER_CHARS));
    }
    parts.join("\n……\n")
}

/// v0.93.16: 训练指令的硬承接锚点——最后一个**上下文**章结尾原文。真机
/// 事故：沙盒没有世界观/角色卡/大纲资产，writer 在「创作资产区域为空」时
/// 拒绝写作或另起现代都市故事（陈默/铜钱），与《阴脉先生》第 106 章的
/// 「桐人镇魇」现场完全脱节。把上一章末段直接放进指令，模型即使没有资产
/// 可读也有可承接的具体文本。
/// v0.94.0：判官/报告用的上一章结尾窗口（只用于判断续写是否承接，绝不能
/// 作为"真实基准章"泄漏给 writer——writer 侧走 build_continuity_anchor）。
const JUDGE_PRIOR_TAIL_CHARS: usize = 800;

fn judge_prior_baseline(context: &[Chapter]) -> (usize, String) {
    let Some((number, content)) = context.last() else {
        return (0, String::new());
    };
    let text = crate::export::content::content_to_plain_text(content);
    (*number, tail_chars(&text, JUDGE_PRIOR_TAIL_CHARS))
}

fn build_continuity_anchor(context: &[Chapter], tail_chars_limit: usize) -> String {
    let Some((number, content)) = context.last() else {
        return String::new();
    };
    let tail = tail_chars(content, tail_chars_limit);
    if tail.is_empty() {
        return String::new();
    }
    format!(
        "上一章（原书第 {number} 章）结尾原文（真实前文，不是要预测的基准章）：\n\
         {tail}\n\
         续写必须从这段之后落笔：沿用同一地点、同一批人物、同一时代与所有未完结事件；\
         禁止换场，禁止新主角/新委托/新事件，禁止把本章写成新故事开篇。"
    )
}

/// v0.93.16：判分前校验「这真的是故事正文」。
/// - 空 HTML（`<p></p>`）/纯空白 → 空稿；
/// - 过短（<300 字）→ gate 失败或模型只给了片段；
/// - 开头命中拒绝/元评论话术 → 不是正文（真机失败 run：writer 输出
///   「我无法按当前输入完成这一章。原因是：提示中「创作资产」区域为空…」）。
fn validate_generated_prose(plain: &str) -> Result<(), String> {
    let trimmed = plain.trim();
    let chars = trimmed.chars().count();
    if chars == 0 {
        return Err("正文为空（HTML 空文档或 gate 未产出）".to_string());
    }
    if chars < MIN_GENERATED_CHARS {
        return Err(format!(
            "正文过短（{chars} 字，至少 {MIN_GENERATED_CHARS} 字；gate 可能未通过）"
        ));
    }
    let head: String = trimmed.chars().take(400).collect();
    const REFUSAL_MARKERS: [&str; 5] = [
        "无法按当前输入完成",
        "创作资产区域为空",
        "请提供创作资产",
        "无法完成这一章",
        "作为 AI",
    ];
    if let Some(marker) = REFUSAL_MARKERS.iter().find(|marker| head.contains(*marker)) {
        return Err(format!(
            "正文是模型拒绝/说明话术而非故事正文（命中「{marker}」）"
        ));
    }
    Ok(())
}

/// v0.99.0：跨章复制检测（判分前的最后一道闸）。
/// 真机 1118 报告：第 14/17/20 章与上一章正文逐字相同，编审仍按普通章节
/// 打分。根因（修订熔断按任意 key 取稿）已修；这里再兜底：正文与上一章
/// 完全相同，或开头 120 字原样出现在上一章正文里，判为「未推进」，不送
/// 判官、不计入有效续写。
fn duplicates_previous_chapter(plain: &str, prev_plain: &str) -> bool {
    fn normalize(text: &str) -> String {
        text.chars().filter(|c| !c.is_whitespace()).collect()
    }
    let current = normalize(plain);
    let previous = normalize(prev_plain);
    if current.is_empty() || previous.is_empty() {
        return false;
    }
    if current == previous {
        return true;
    }
    let head: String = current.chars().take(120).collect();
    if head.chars().count() < 120 {
        return false;
    }
    previous.contains(&head)
}

pub struct ContinuationTrainingExecutor {
    pub pool: DbPool,
    pub app_handle: tauri::AppHandle,
}

impl ContinuationTrainingExecutor {
    pub fn new(pool: DbPool, app_handle: tauri::AppHandle) -> Self {
        Self { pool, app_handle }
    }
}

#[async_trait]
impl TaskExecutor for ContinuationTrainingExecutor {
    async fn execute(&self, task: &Task) -> Result<TaskResult, Box<dyn std::error::Error>> {
        let ctx =
            TaskExecutionContext::new(task.id.clone(), self.pool.clone(), self.app_handle.clone());
        ctx.log("info", "续写训练任务开始");

        let payload: TrainingPayload = match task.payload.as_deref() {
            Some(p) => serde_json::from_str(p)
                .map_err(|e| format!("payload 解析失败（需要 book_title/source 等字段）: {e}"))?,
            None => {
                return Ok(TaskResult {
                    success: false,
                    result_json: None,
                    error_message: Some("payload 为空".into()),
                })
            }
        };
        let context_want = payload.chapter_count.clamp(2, MAX_CONTEXT_CHAPTERS);
        let continuation_want = payload
            .continuation_chapters
            .clamp(1, MAX_CONTINUATION_CHAPTERS);
        let blind = payload.feedback_mode.as_deref() == Some("blind");
        let compact_prompt = payload.prompt_mode.as_deref() == Some("compact");

        // ── 1. 取源章节（上下文 + 真实后续基准）──────────────────
        ctx.update_progress(
            TrainingStage::Fetch.step(),
            TrainingStage::Fetch.progress(),
            TrainingStage::Fetch.label(),
        );
        let (chapters, source_kind) = self
            .fetch_chapters(&payload, &ctx, context_want, continuation_want)
            .await?;
        if chapters.len() < 3 {
            let msg = format!(
                "取到的章节数不足（{} 章，至少 3 章：2 章上下文 + 1 章真实基准）",
                chapters.len()
            );
            ctx.log("error", &msg);
            return Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some(msg),
            });
        }
        // 上下文取请求的前 context_want 章；其余全部作为真实基准章
        // （源书剩余章数不足时基准章数自动收缩，不伪造缺章）。
        let context_n = context_want.min(chapters.len().saturating_sub(1));
        if context_n < 2 {
            let msg = format!(
                "取到的上下文不足 2 章（{} 章），无法形成续写前文",
                chapters.len()
            );
            ctx.log("error", &msg);
            return Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some(msg),
            });
        }
        let truth_n = chapters.len() - context_n;
        if truth_n < 1 {
            let msg = "未取到任何真实后续基准章，无法对比".to_string();
            ctx.log("error", &msg);
            return Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some(msg),
            });
        }
        let context = &chapters[..context_n];
        let truths = &chapters[context_n..];
        ctx.log(
            "info",
            &format!(
                "已取 {} 章：上下文 {} 章（原书第 {}–{} 章）+ 真实后续基准 {} 章（第 {}–{} 章）",
                chapters.len(),
                context_n,
                context.first().map(|c| c.0).unwrap_or(0),
                context.last().map(|c| c.0).unwrap_or(0),
                truth_n,
                truths.first().map(|c| c.0).unwrap_or(0),
                truths.last().map(|c| c.0).unwrap_or(0),
            ),
        );

        // ── 2. 独立沙盒故事 ─────────────────────────────────────
        ctx.update_progress(
            TrainingStage::Sandbox.step(),
            TrainingStage::Sandbox.progress(),
            TrainingStage::Sandbox.label(),
        );
        let story_repo = StoryRepository::new(self.pool.clone());
        let stamp = Local::now().format("%m%d-%H%M");
        let story = story_repo.create(crate::db::dto::CreateStoryRequest {
            title: format!("[训练]《{}》·{stamp}", payload.book_title),
            description: Some(format!(
                "续写训练沙盒：《{}》第 {}–{} 章（{} 章上下文）灌入，连续续写 {} 章并与原书第 {}–{} 章编审对比。由定时任务自动创建与清理。",
                payload.book_title,
                context.first().map(|c| c.0).unwrap_or(0),
                context.last().map(|c| c.0).unwrap_or(0),
                context_n,
                truth_n,
                truths.first().map(|c| c.0).unwrap_or(0),
                truths.last().map(|c| c.0).unwrap_or(0),
            )),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })?;
        self.seed_scenes(&story.id, context, &ctx).await?;

        // ── 3. 风格 DNA（真实章节 → StyleAnalyzer → 绑定故事）────
        ctx.update_progress(
            TrainingStage::Style.step(),
            TrainingStage::Style.progress(),
            TrainingStage::Style.label(),
        );
        let style_applied = self
            .apply_style_dna(&story.id, &payload.book_title, context, &ctx)
            .await;

        // ── 3.5 训练资产重建（v0.94.0）─────────────────────────
        // 训练沙盒不是"从零创世"：先把上下文章归一为世界观/故事大纲/角色/
        // 关系/逐章梗概，再进入既有续写与编审管线。这样 writer 拿到的资产
        // 与正作续写同源，不再因资产区为空拒绝或另起新故事。
        ctx.update_progress(
            TrainingStage::Assets.step(),
            TrainingStage::Assets.progress(),
            TrainingStage::Assets.label(),
        );
        let asset_llm = crate::llm::LlmService::new(self.app_handle.clone());
        let asset_input: Vec<(usize, String)> = context
            .iter()
            .map(|(number, content)| (*number, content.clone()))
            .collect();
        let asset_progress: crate::agency::training_assets::AssetProgress = {
            let task_id = task.id.clone();
            let pool = self.pool.clone();
            let app = self.app_handle.clone();
            std::sync::Arc::new(move |done, total| {
                let ctx = crate::task_system::executor::TaskExecutionContext::new(
                    task_id.clone(),
                    pool.clone(),
                    app.clone(),
                );
                let total = total.max(1);
                let p = 35 + ((done.min(total) as i32 * 5) / total as i32).min(5);
                let label = format!("逐章摘要 {done}/{total} 章完成");
                ctx.update_progress("assets", p, &label);
                ctx.log("info", &label);
            })
        };
        let rebuilt = crate::agency::training_assets::rebuild_from_context(
            &self.pool,
            &asset_llm,
            &story.id,
            &payload.book_title,
            &asset_input,
            Some(asset_progress),
        )
        .await;
        if let Some(err) = rebuilt.llm_error.as_deref() {
            ctx.log("warn", &format!("资产重建失败（继续训练）：{err}"));
        } else {
            ctx.log(
                "info",
                &format!(
                    "资产重建完成（两阶段）：逐章摘要{}/{}（缓存{}、兜底{}）｜聚合{}｜世界观{}｜故事大纲{}｜角色{}｜关系{}｜章节梗概{}｜前情提要{}章/{}线索/{}白名单",
                    rebuilt.chapter_summaries,
                    asset_input.len(),
                    rebuilt.summary_cached,
                    rebuilt.summary_fallback,
                    if rebuilt.aggregate_ok { "✓" } else if rebuilt.aggregate_fallback { "兜底" } else { "—" },
                    if rebuilt.world { "✓" } else { "—" },
                    if rebuilt.story_outline { "✓" } else { "—" },
                    rebuilt.characters,
                    rebuilt.relationships,
                    rebuilt.chapter_outlines,
                    rebuilt.brief_chapters,
                    rebuilt.brief_threads,
                    rebuilt.brief_terms
                ),
            );
        }
        for warning in rebuilt.warnings.iter().take(4) {
            ctx.log("warn", warning);
        }
        // v0.99.1：长程记忆。资产重建成功时 persist 已写《前情提要》；
        // 失败时这里用 scenes 梗概 + 人物/规则确定性兜底；再用前情提要写一份
        // 确定性全局大纲，避免 ensure_story_outline 的「头600+尾1800」伪大纲。
        let brief =
            crate::agency::training_assets::ensure_story_brief(&self.pool, &story.id, &asset_input)
                .await;
        let outline_rewritten = crate::agency::training_assets::ensure_deterministic_outline(
            &self.pool, &story.id, &brief,
        );
        let brief_rendered = brief.render(crate::agency::story_brief::STORY_BRIEF_CHAR_CAP);
        ctx.log(
            "info",
            &format!(
                "前情提要就绪：{} 章逐章事实｜{} 条未结线索｜{} 项设定白名单｜{} 人人物态｜{} 字{}",
                brief.chapters.len(),
                brief.threads.len(),
                brief.canon_terms.len(),
                brief.cast_states.len(),
                brief_rendered.chars().count(),
                if outline_rewritten {
                    "｜已替换被规划文档污染的故事大纲"
                } else {
                    ""
                }
            ),
        );

        // ── 4. 承接锚点 + 上轮编审结论注入（自动闭环）──────────
        // v0.93.16: 沙盒没有世界观/角色卡/大纲资产，通用管线会因「创作资产
        // 为空」拒绝写作，或把续写写成新故事开篇。训练指令必须携带上一章
        // 的真实结尾；且只用上下文章（真实基准章禁止泄漏）。
        // v0.98.0: 上轮报告不只给文本建议，还按维度均分自动加码本轮修正
        // （承接/文笔/情节/口吻/节奏/钩子），把"报告→能力优化"落成指令。
        let context_start = context.first().map(|c| c.0).unwrap_or(1);
        // 上下文的纯文本只用于人名接地判定（禁止用生成的幻觉人名污染指令）。
        let context_prose: String = context
            .iter()
            .map(|(_, content)| content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let roster = self.load_canonical_roster(&story.id);
        let roster_block = build_roster_block(&roster);
        let feedback = self.load_training_feedback(&payload.book_title, context_start);
        let force_style_anchor = !style_applied || feedback.style_low();
        let style_anchor = if force_style_anchor {
            let anchor = build_style_anchor(context);
            if !anchor.is_empty() {
                ctx.log(
                    "info",
                    if style_applied {
                        "上一轮文笔分偏低，本轮强制注入真实章节末尾作风格锚点"
                    } else {
                        "风格 DNA 不可用，已注入真实章节末尾节选作为文风锚点"
                    },
                );
            }
            anchor
        } else {
            String::new()
        };
        let continuity_limit = if feedback.continuity_low() { 1400 } else { 900 };
        let continuity_anchor = build_continuity_anchor(context, continuity_limit);
        if !continuity_anchor.is_empty() {
            ctx.log(
                "info",
                &format!(
                    "已注入上一章结尾原文作为硬承接锚点（{} 字，上一轮承接分偏低={}）",
                    continuity_limit,
                    feedback.continuity_low()
                ),
            );
        }
        let prior =
            self.load_prior_insights(&payload.book_title, context_start, &context_prose, blind);
        let first_gen = context_n + 1;
        let last_gen = context_n + truth_n;
        let base_instruction = format!(
            "这是续写训练沙盒：前 {context_n} 章是小说《{}》的真实内容。\
             本轮连续续写 {truth_n} 章（沙盒第 {first_gen}–{last_gen} 章）：\
             每一章都以与原文一致的文风、叙事节奏和人物口吻，紧接本沙盒上一章自然承接\
             （不要总结、不要复述、不要写大纲），并保持人物、地点、时代与未完结事件连续。",
            payload.book_title
        );
        // v0.98.7：名册放在指令前部（章纲规划只看前 3200 字，也必须读到
        // 「唯一可用具名人物」约束）；编审/历史建议中的名册外人名一律视为
        // 幻觉忽略，避免上一轮报告把杨先生/何小满写进本轮设定。
        let mut instruction = base_instruction;
        if !roster_block.is_empty() {
            instruction = format!("{instruction}\n\n{roster_block}");
        }
        // v0.99.0：基调/设定/推进/承接四条红线同样必须进入章纲规划视野。
        instruction = format!("{instruction}\n\n{TRAINING_REDLINES}");
        if let Some(insights) = &prior {
            instruction = format!(
                "{instruction}\n\n历史训练结论（务必落实；其中名册外的人名一律忽略）：\n{insights}"
            );
        }
        // 维度修正：上一轮报告的低分维度→本轮硬性写作要求。
        let corrections = feedback.correction_block();
        if !corrections.is_empty() {
            ctx.log(
                "info",
                &format!("已按上一轮编审报告注入 {} 条维度修正", corrections.len()),
            );
            instruction = format!(
                "{instruction}\n\n【本轮重点修正（来自上一轮编审报告）】\n{}",
                corrections.join("\n")
            );
        }
        if !style_anchor.is_empty() {
            instruction = format!(
                "{instruction}\n\n【文风锚点（原文节选，务必模仿其遣词造句、对话口吻、\
                 叙事节奏与生活细节密度，不得改换语体）】\n{style_anchor}"
            );
        }
        // 承接锚点放最后（指令 recency 最高）；旧版 style anchor 截断也
        // 不会再把它挤掉。
        if !continuity_anchor.is_empty() {
            instruction = format!(
                "{instruction}\n\n【必须承接的原文结尾】\n{continuity_anchor}\n\
                 （该结尾只约束首章（沙盒第 {first_gen} 章）；沙盒第 {} 章及以后\
                 一律承接本沙盒上一章生成的正文，不重复从这段原文起笔。）",
                first_gen + 1
            );
            // 锚点以 recency 优势放在最后，再把红线压轴重申一次：否则模型
            // 只盯着「承接」而复制结尾（真机报告：把上一章结尾当本章开头）。
            instruction.push_str(
                "\n\n【最后提醒】锚点只用于对齐现场，禁止复制/复述；本章必须推进新事件；\
                 不得新增体系性设定；保持原文语体与主角原有口吻。",
            );
        }

        // ── 5. 走既有续写管线（含 gate/装配）────────────────────
        ctx.update_progress(
            TrainingStage::Continue.step(),
            TrainingStage::Continue.progress(),
            TrainingStage::Continue.label(),
        );
        let run_id = uuid::Uuid::new_v4().to_string();
        // v0.99.2：批内每章装配后同步 task progress/log/heartbeat。
        // v0.98.9 的前端刷新机制会随 `task-progress` 事件同步任务列表与日志。
        let task_id_for_progress = task.id.clone();
        let pool_for_progress = self.pool.clone();
        let app_for_progress = self.app_handle.clone();
        let progress_sink: std::sync::Arc<dyn Fn(i32, usize, usize, usize) + Send + Sync> =
            std::sync::Arc::new(move |chapter_number, done, total, chars| {
                let ctx = crate::task_system::executor::TaskExecutionContext::new(
                    task_id_for_progress.clone(),
                    pool_for_progress.clone(),
                    app_for_progress.clone(),
                );
                let total = total.max(1);
                let progress = 40 + ((done.min(total) as i32 * 32) / total as i32).min(32);
                let label =
                    format!("续写第 {done}/{total} 章完成（第 {chapter_number} 章，{chars} 字）");
                ctx.update_progress("continue", progress, &label);
                ctx.log("info", &label);
            });
        // v0.98.7：训练沙盒关闭后台 ingest——生成正文不是原书事实，
        // 反向资产桥接会把臆造人名/大纲写回，后续章节持续放大幻觉。
        let coordinator = crate::agency::coordinator::AgencyCoordinator::new(
            self.app_handle.clone(),
            self.pool.clone(),
        )
        .with_asset_ingest_enabled(false)
        // v0.99.1：训练资产（前情提要/大纲/角色）已在上方显式就绪，
        // 禁止 ensure_assets 再用「正文头600+尾1800」重造大纲或从生成正文
        // 提取幻觉角色写回沙盒。
        .with_assets_seeded(true)
        .with_batch_progress(progress_sink)
        .with_compact_prompt(compact_prompt)
        // v0.99.7：训练批量质检直连 JSON，绕开本地模型永不遵从的
        // ReAct action 格式（真机 v0.99.6 9/10 章质检降级）。
        .with_direct_gate_json(true);
        let cancel = Arc::new(AtomicBool::new(false));
        // 取消传播：任务被取消时同步中断续写管线（统一取消监听器，500ms）。
        // 监听句柄 Drop 随 execute 结束自动停止轮询，不再泄漏协程。
        let _cancel_monitor = spawn_cancel_monitor(task.id.clone(), self.pool.clone(), {
            let cancel = cancel.clone();
            move || {
                cancel.store(true, Ordering::Relaxed);
                log::info!("[ContinuationTraining] 任务取消，已传播到续写管线");
            }
        });
        // 沙盒章号连续：上下文占 1..=context_n，续写从 context_n+1 起，
        // 一次批量写 truth_n 章（gate/装配/章号仍走同一管线）。
        let next_chapter = (context_n + 1) as i32;
        let batch = coordinator
            .run_continue_batch_with_cancel(
                &run_id,
                &story.id,
                next_chapter,
                truth_n,
                &instruction,
                cancel,
            )
            .await;
        let batch = match batch {
            Ok(b) => b,
            Err(e) => {
                let msg = format!("续写管线失败：{e}");
                ctx.log("error", &msg);
                // 沙盒保留供排查；报告不落（无产出可比）
                return Ok(TaskResult {
                    success: false,
                    result_json: None,
                    error_message: Some(msg),
                });
            }
        };
        // 生成正文已装配进 scene（batch 只回 scene_id/章号），逐章从 scene 读
        let scene_repo =
            crate::db::repositories::scene_repository::SceneRepository::new(self.pool.clone());
        // v0.93.16：空稿必须是「视觉空」而不是仅字符串空——tiptap 的空文档
        // 序列化为 `<p></p>`（非空字符串），旧代码会把它当正文送去判官，
        // 落出 0.0/scores:null 的假报告。拒绝话术（「创作资产区域为空」）
        // 同样是模型元评论，不是正文。v0.98.0：逐章校验；某章不可用只降级
        // 该章（报告标注"续写缺失"），全部不可用才判训练失败。
        let mut generated: Vec<(i32, String)> = Vec::new();
        let mut unusable: Vec<(i32, String)> = Vec::new();
        let mut prev_plain: Option<String> = None;
        for item in &batch.chapters {
            let raw = scene_repo
                .get_by_id(&item.scene_id)
                .ok()
                .flatten()
                .and_then(|sc| sc.content)
                .unwrap_or_default();
            let plain = crate::export::content::content_to_plain_text(&raw);
            let copied = prev_plain
                .as_deref()
                .map(|prev| duplicates_previous_chapter(&plain, prev))
                .unwrap_or(false);
            if !plain.trim().is_empty() {
                // 先把本轮正文记为「上一章」，再做有效性归类（plain 随后被
                // move 进 generated）。重复稿仍作为下一章的对比基线。
                prev_plain = Some(plain.clone());
            }
            let verdict = match validate_generated_prose(&plain) {
                Ok(()) if copied => Err("与上一章正文重复（整段复制，未推进）".to_string()),
                other => other,
            };
            match verdict {
                Ok(()) => generated.push((item.chapter_number, plain)),
                Err(reason) => {
                    ctx.log(
                        "warn",
                        &format!("第 {} 章续写产出不可用：{reason}", item.chapter_number),
                    );
                    unusable.push((item.chapter_number, reason));
                    generated.push((item.chapter_number, String::new()));
                }
            }
        }
        if generated.iter().all(|(_, text)| text.trim().is_empty()) {
            let msg = format!(
                "全部 {} 章续写产出不可用，无法对比（{}）",
                truth_n,
                unusable
                    .iter()
                    .take(3)
                    .map(|(num, reason)| format!("第 {num} 章：{reason}"))
                    .collect::<Vec<_>>()
                    .join("；")
            );
            ctx.log("error", &msg);
            // 沙盒保留供排查；报告不落（无产出可比）
            return Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some(msg),
            });
        }
        ctx.update_progress(
            TrainingStage::Compare.step(),
            TrainingStage::Compare.progress(),
            TrainingStage::Compare.label(),
        );

        // ── 7. 编审 Agent 逐章对比（AI 续写 vs 原书真实后续章）──
        // 真实基准：原书后 truth_n 章；AI 续写：沙盒第 next_chapter 起 truth_n
        // 章。
        // 判官用 EditorAuditor（编审）角色提示词（支持用户在设置页覆盖的人设）。
        let prior_baseline = judge_prior_baseline(context);
        let llm = crate::llm::LlmService::new(self.app_handle.clone());
        let editor_system = self.editor_system_prompt(&payload.book_title).await;
        let mut reviews: Vec<ChapterReviewOutcome> = Vec::with_capacity(truth_n);
        for (i, (truth_ch, truth_raw)) in truths.iter().enumerate() {
            let gen_ch = next_chapter + i as i32;
            let gen_text = generated
                .get(i)
                .map(|(_, text)| text.clone())
                .unwrap_or_default();
            // 承接基准：第 1 章看上下文末章末段；后续章看本沙盒上一章末段
            let (prior_ch, prior_tail) = if i == 0 {
                prior_baseline.clone()
            } else {
                let prev_num = next_chapter + i as i32 - 1;
                let prev_text = generated
                    .get(i - 1)
                    .map(|(_, text)| text.clone())
                    .unwrap_or_default();
                if prev_text.trim().is_empty() {
                    (prev_num as usize, String::new())
                } else {
                    (
                        prev_num as usize,
                        tail_chars(&prev_text, JUDGE_PRIOR_TAIL_CHARS),
                    )
                }
            };
            let truth_text = crate::export::content::content_to_plain_text(truth_raw);
            ctx.update_progress(
                TrainingStage::Compare.step(),
                72 + (i * 8 / truth_n.max(1)) as i32,
                &format!(
                    "编审对比第 {}/{} 对：沙盒第 {} 章 vs 原书第 {} 章…",
                    i + 1,
                    truth_n,
                    gen_ch,
                    truth_ch
                ),
            );
            let mut review = self
                .review_chapter(
                    &llm,
                    &editor_system,
                    &payload.book_title,
                    *truth_ch,
                    gen_ch,
                    &truth_text,
                    &gen_text,
                    prior_ch,
                    &prior_tail,
                )
                .await;
            // 空稿原因（含 v0.99.0 跨章重复）写进编审行，报告不再只说
            // 「空稿/gate 失败」而说不出到底发生了什么。
            if !review.prose_available {
                if let Some((_, reason)) = unusable.iter().find(|(num, _)| *num == gen_ch) {
                    review.comments = format!("本章 AI 续写不可用：{reason}");
                }
            }
            reviews.push(review);
        }

        // ── 8. 编审 Agent 跨章总评（全面对比分析 + 改进项）────
        ctx.update_progress(
            TrainingStage::Review.step(),
            TrainingStage::Review.progress(),
            TrainingStage::Review.label(),
        );
        let synthesis = self
            .review_synthesis(
                &llm,
                &editor_system,
                &payload.book_title,
                context_n,
                truth_n,
                truths,
                &generated,
                &reviews,
            )
            .await;
        if synthesis.is_none() {
            ctx.log("warn", "编审总评不可用（降级为逐章数据汇总）");
        }

        // ── 9. 报告生成（确定性排版 + 编审结论）────────────────
        // 每章只归入一类，避免「修订 + 降级」重叠把总数对不上（真机 1118
        // 报告 3+0+2=5 与 10 章不符：5 章编审判修订、修订轮熔断后保留首稿）。
        let (mut gate_passed, mut revised_count, mut gate_degraded, mut gate_unrevised) =
            (0usize, 0usize, 0usize, 0usize);
        for c in &batch.chapters {
            if is_degraded_gate_comment(&c.verdict.comments) {
                gate_degraded += 1;
            } else if c.revised {
                revised_count += 1;
            } else if c.verdict.verdict == "pass" {
                gate_passed += 1;
            } else {
                gate_unrevised += 1;
            }
        }
        // 幻觉人名集合：当前沙盒 ingest/模型生成的角色 + 编审文本里引用的
        // 名册外人名（上一轮建议过滤也复用同一口径）。
        let mut ungrounded = std::collections::HashSet::new();
        for name in self.ungrounded_character_names(&story.id, &context_prose) {
            ungrounded.insert(name);
        }
        for review in &reviews {
            collect_unquoted_ungrounded_names(&review.comments, &context_prose, &mut ungrounded);
            collect_unquoted_ungrounded_names(&review.improvement, &context_prose, &mut ungrounded);
        }
        let summary_diag = format!(
            "逐章摘要{}/{}（缓存{}、兜底{}）｜聚合{}",
            rebuilt.chapter_summaries,
            asset_input.len(),
            rebuilt.summary_cached,
            rebuilt.summary_fallback,
            if rebuilt.aggregate_ok {
                "✓"
            } else if rebuilt.aggregate_fallback {
                "兜底"
            } else {
                "—"
            }
        );
        let asset_line = if let Some(err) = rebuilt.llm_error.as_deref() {
            format!(
                "资产重建失败（继续训练）：{}｜{}",
                Self::preview_chars(err, 160),
                summary_diag
            )
        } else {
            let outline_fallback = rebuilt
                .warnings
                .iter()
                .any(|w| w.contains("原文首尾窗口兜底"));
            format!(
                "{}｜世界观{}｜故事大纲{}｜角色{}｜关系{}｜章节梗概{}{}",
                summary_diag,
                if rebuilt.world { "✓" } else { "—" },
                if rebuilt.story_outline { "✓" } else { "—" },
                rebuilt.characters,
                rebuilt.relationships,
                rebuilt.chapter_outlines,
                if outline_fallback {
                    "（模型未返回，首尾兜底）"
                } else {
                    ""
                }
            )
        };
        // v0.99.1：报告显式记录长程记忆是否真正进入提示词，便于下一轮定位
        // 「有数据但没进 prompt」而不是继续给抽象修正。
        let memory_line = format!(
            "前情提要：{} 章逐章事实｜{} 条未结线索｜{} 项设定白名单｜{} 人人物态｜注入 {} 字（writer+planner 全量）",
            brief.chapters.len(),
            brief.threads.len(),
            brief.canon_terms.len(),
            brief.cast_states.len(),
            brief_rendered.chars().count()
        );
        let feedback_line = if blind {
            "- 反馈模式：blind（过滤真实后续节点，避免基准泄漏）"
        } else {
            "- 反馈模式：truth_hints（允许注入上一轮真实后续节点）"
        };
        let prompt_line = if compact_prompt {
            "- Prompt 模式：compact（本地小模型）"
        } else {
            "- Prompt 模式：full"
        };
        let probe_line = memory_probes_line(&brief, &generated, &context_prose);
        let asset_line = format!(
            "{asset_line}\n- 长程记忆：{memory_line}\n{feedback_line}\n{prompt_line}\n- 记忆探针：{probe_line}"
        );
        let (report_md, scores_json, improvement_md) = Self::build_report(
            &payload.book_title,
            context_n,
            truth_n,
            context,
            truths,
            &reviews,
            synthesis.as_ref(),
            &asset_line,
            gate_passed,
            gate_degraded,
            revised_count,
            gate_unrevised,
            style_applied,
            &feedback,
            &ungrounded,
        );
        ctx.update_progress(
            TrainingStage::Report.step(),
            TrainingStage::Report.progress(),
            TrainingStage::Report.label(),
        );

        // ── 10. 报告落库 + 清理旧沙盒 ──────────────────────────
        let report_id = uuid::Uuid::new_v4().to_string();
        let truth_chapters_json = serde_json::to_string(
            &truths
                .iter()
                .map(|(num, text)| {
                    serde_json::json!({
                        "chapter": num,
                        "content": crate::export::content::content_to_plain_text(text),
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[]".into());
        let chapter_reviews_json = serde_json::to_string(&reviews).unwrap_or_else(|_| "[]".into());
        {
            let conn = self.pool.get().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO continuation_training_reports \
                 (id, task_id, book_title, source_kind, story_id, run_id, start_chapter, \
                  context_chapters, continuation_chapters, generated_chapter, truth_content, \
                  truth_chapters_json, chapter_reviews_json, scores_json, report_md, \
                  improvement_md, style_dna_applied) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
                rusqlite::params![
                    report_id,
                    task.id,
                    payload.book_title,
                    source_kind,
                    story.id,
                    run_id,
                    context.first().map(|c| c.0 as i64).unwrap_or(0),
                    context_n as i64,
                    truth_n as i64,
                    next_chapter,
                    truths
                        .first()
                        .map(|(_, text)| crate::export::content::content_to_plain_text(text))
                        .unwrap_or_default(),
                    truth_chapters_json,
                    chapter_reviews_json,
                    scores_json,
                    report_md,
                    improvement_md,
                    style_applied as i64,
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        self.cleanup_old_sandboxes(&payload.book_title, KEEP_SANDBOXES);

        ctx.update_progress(
            TrainingStage::Done.step(),
            TrainingStage::Done.progress(),
            TrainingStage::Done.label(),
        );
        let first_truth = truths.first().map(|c| c.0).unwrap_or(0);
        let summary = format!(
            "训练完成：沙盒 {}，连续续写 {} 章（沙盒第 {}–{} 章）对比原书第 {}–{} 章，报告 {report_id}（保留最近 {KEEP_SANDBOXES} 个沙盒）",
            story.title,
            truth_n,
            next_chapter,
            next_chapter + truth_n as i32 - 1,
            first_truth,
            truths.last().map(|c| c.0).unwrap_or(first_truth),
        );
        ctx.log("info", &summary);
        Ok(TaskResult {
            success: true,
            result_json: Some(
                serde_json::json!({
                    "report_id": report_id,
                    "story_id": story.id,
                    "run_id": run_id,
                    "generated_chapter": next_chapter,
                    "continuation_chapters": truth_n,
                    "truth_chapters": truth_n,
                })
                .to_string(),
            ),
            error_message: None,
        })
    }

    fn can_handle(&self, task_type: &TaskType) -> bool {
        matches!(task_type, TaskType::ContinuationTraining)
    }
}

/// (章号, 正文)
type Chapter = (usize, String);

impl ContinuationTrainingExecutor {
    /// 取源章节：先取 `context_n` 章上下文，再取 `truth_n` 章「真实后续」基准。
    /// 返回顺序 = 上下文在前、基准在后（基准章绝不进入续写指令）。
    async fn fetch_chapters(
        &self,
        payload: &TrainingPayload,
        ctx: &TaskExecutionContext,
        context_n: usize,
        truth_n: usize,
    ) -> Result<(Vec<Chapter>, &'static str), String> {
        let total_want = context_n + truth_n;
        match &payload.source {
            SourceSpec::File { path } => {
                if path.trim().is_empty() {
                    return Err("file 源的 path 为空".into());
                }
                let parsed = crate::book_deconstruction::parser::parse_book(
                    std::path::Path::new(path),
                    None,
                )
                .map_err(|e| format!("解析书籍失败：{e}"))?;
                if parsed.chapters.is_empty() {
                    return Err("书籍未解析出任何章节".into());
                }
                let total = parsed.chapters.len();
                let start = Self::pick_start(payload, total);
                let (begin, end) = Self::file_slice_bounds(start, total_want, total);
                if begin >= end {
                    return Err(format!("起始章号 {start} 超出书籍总章数 {total}，无法取章"));
                }
                let slice = &parsed.chapters[begin..end];
                // v0.98.1：start 是 1-based 章号，切片起点为 start-1；此前
                // `[start..end]` 会跳过首章并整体错一位。
                let out = slice
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (start + i, Self::cap(&c.content)))
                    .collect::<Vec<_>>();
                Ok((out, "file"))
            }
            SourceSpec::Mcp {
                server_id,
                chapter_tool,
                args_template,
            } => {
                let start = payload.start_chapter.unwrap_or(1);
                let tool = self.pick_mcp_chapter_tool(server_id, chapter_tool).await?;
                let template = if args_template.is_empty() {
                    self.default_mcp_args_template(server_id, &tool).await
                } else {
                    args_template.clone()
                };
                let book_ref = self
                    .resolve_mcp_book_ref(server_id, &payload.book_title)
                    .await
                    .unwrap_or_else(|| payload.book_title.clone());
                // 连续取 context_n + truth_n 章；连续两次空结果即止。
                // v0.98.5：同一本书反复训练时优先读本地源章节缓存，命中则
                // 跳过 MCP 调用（章节正文是稳定的只读源材料）。
                let mut out: Vec<Chapter> = Vec::new();
                let mut empty_streak = 0;
                let mut cache_hits = 0usize;
                let mut fetched = 0usize;
                for offset in 0..total_want {
                    if ctx.is_cancelled() {
                        break;
                    }
                    let index = start + offset;
                    if let Some(cached) =
                        read_cached_chapter(&self.pool, server_id, &book_ref, index)
                    {
                        cache_hits += 1;
                        empty_streak = 0;
                        let chars = cached.chars().count();
                        out.push((index, cached));
                        ctx.update_progress(
                            "fetch",
                            5 + (offset * 10 / total_want.max(1)) as i32,
                            &format!("已取第 {index} 章（{chars} 字，缓存）"),
                        );
                        continue;
                    }
                    let args = Self::render_args(&template, &book_ref, index);
                    let content = self.call_mcp_chapter_tool(server_id, &tool, args).await?;
                    let trimmed = content.trim().to_string();
                    if trimmed.is_empty() {
                        empty_streak += 1;
                        if empty_streak >= 2 {
                            break;
                        }
                        continue;
                    }
                    empty_streak = 0;
                    fetched += 1;
                    let content = Self::cap(&trimmed);
                    if let Err(error) =
                        write_cached_chapter(&self.pool, server_id, &book_ref, index, &content)
                    {
                        ctx.log(
                            "warn",
                            &format!("源章节缓存写入失败（第 {index} 章）：{error}"),
                        );
                    }
                    out.push((index, content));
                    ctx.update_progress(
                        "fetch",
                        5 + (offset * 10 / total_want.max(1)) as i32,
                        &format!("已取第 {index} 章（{} 字）", trimmed.chars().count()),
                    );
                }
                if cache_hits > 0 || fetched > 0 {
                    ctx.log(
                        "info",
                        &format!("源章节缓存：命中 {cache_hits} 章，新读取 {fetched} 章"),
                    );
                }
                Ok((out, "mcp"))
            }
        }
    }

    /// 1-based 起始章号 → 0-based 切片边界（v0.98.1 回归修复）。
    fn file_slice_bounds(start: usize, total_want: usize, total: usize) -> (usize, usize) {
        let begin = start.saturating_sub(1).min(total);
        let end = (begin + total_want).min(total);
        (begin, end)
    }

    fn pick_start(payload: &TrainingPayload, total: usize) -> usize {
        match payload.strategy.as_str() {
            "random" => {
                let upper = if payload.est_total_chapters > 0 {
                    payload.est_total_chapters.min(total)
                } else {
                    total
                };
                let total_want = payload.chapter_count + payload.continuation_chapters;
                let window = upper.saturating_sub(total_want + 1).max(1);
                (rand::random::<usize>() % window) + 1
            }
            _ => payload.start_chapter.unwrap_or(1).max(1),
        }
    }

    fn cap(text: &str) -> String {
        if text.chars().count() <= MAX_CHAPTER_CHARS {
            text.to_string()
        } else {
            let cut: String = text.chars().take(MAX_CHAPTER_CHARS).collect();
            format!("{cut}\n\n[章节过长，训练时截断到 {MAX_CHAPTER_CHARS} 字符]")
        }
    }

    /// 编审对比用采样上限（6k/章，控制 10 对对比的提示词体积）。
    fn cap_review(text: &str) -> String {
        Self::cap_review_with_limit(text, REVIEW_CHAPTER_CHARS)
    }

    /// v0.99.7：编审第三次尝试用更短采样，避免推理模型在 7k+ prompt 上
    /// 只产 CoT、content 为空。
    fn cap_review_with_limit(text: &str, limit: usize) -> String {
        if text.chars().count() <= limit {
            text.to_string()
        } else {
            let cut: String = text.chars().take(limit).collect();
            format!("{cut}\n\n[编审采样截断到 {limit} 字符]")
        }
    }

    fn render_args(
        template: &serde_json::Map<String, serde_json::Value>,
        book: &str,
        index: usize,
    ) -> serde_json::Value {
        let mut out = serde_json::Map::new();
        for (k, v) in template {
            let replaced = match v {
                serde_json::Value::String(s) if s == "$index" => {
                    serde_json::Value::Number(index.into())
                }
                serde_json::Value::String(s) => serde_json::Value::String(
                    s.replace("$book", book)
                        .replace("$index", &index.to_string()),
                ),
                other => other.clone(),
            };
            out.insert(k.clone(), replaced);
        }
        serde_json::Value::Object(out)
    }

    /// 未显式指定章节工具时，从服务器连接自动挑一个正文工具。
    async fn pick_mcp_chapter_tool(
        &self,
        server_id: &str,
        preferred: &str,
    ) -> Result<String, String> {
        let preferred = preferred.trim();
        if !preferred.is_empty() {
            return Ok(preferred.to_string());
        }
        let connections = crate::MCP_CONNECTIONS.lock().await;
        let client = connections.get(server_id).ok_or_else(|| {
            format!(
                "MCP 服务器 {server_id} 未连接：请确认栗子读书等源已启动，并在 MCP 页连接该服务器后重试"
            )
        })?;
        Self::pick_chapter_tool_from_tools(client.get_tools()).ok_or_else(|| {
            format!("MCP 服务器 {server_id} 未提供章节正文工具；请在 MCP 页确认工具列表")
        })
    }

    fn pick_chapter_tool_from_tools(tools: &[crate::mcp::McpTool]) -> Option<String> {
        if tools.iter().any(|tool| tool.name == "get_chapter_content") {
            return Some("get_chapter_content".to_string());
        }
        let keywords = ["chapter", "content", "正文", "章节"];
        tools
            .iter()
            .find(|tool| {
                let haystack = format!("{} {}", tool.name, tool.description).to_ascii_lowercase();
                keywords.iter().any(|keyword| haystack.contains(keyword))
            })
            .map(|tool| tool.name.clone())
    }

    /// 参数模板为空时，按工具 JSON Schema 自动生成 $book/$index/开关。
    async fn default_mcp_args_template(
        &self,
        server_id: &str,
        tool_name: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        let template = serde_json::Map::new();
        let connections = crate::MCP_CONNECTIONS.lock().await;
        let client = match connections.get(server_id) {
            Some(client) => client,
            None => return template,
        };
        let tool = client
            .get_tools()
            .iter()
            .find(|tool| tool.name == tool_name);
        let Some(tool) = tool else {
            return template;
        };
        Self::default_args_from_tool(tool)
    }

    fn default_args_from_tool(
        tool: &crate::mcp::McpTool,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut template = serde_json::Map::new();
        let properties = tool
            .parameters
            .get("properties")
            .and_then(|value| value.as_object());
        let Some(properties) = properties else {
            return template;
        };
        for (name, schema) in properties {
            let lower = name.to_ascii_lowercase();
            let schema_type = schema
                .get("type")
                .and_then(|value| value.as_str())
                .unwrap_or("string");
            if lower.contains("book") && schema_type == "string" {
                template.insert(name.clone(), serde_json::json!("$book"));
            } else if (lower.contains("index") || lower.contains("chapter"))
                && (schema_type == "integer" || schema_type == "number")
            {
                template.insert(name.clone(), serde_json::json!("$index"));
            } else if lower == "fetch_online" || lower == "include_content" {
                template.insert(name.clone(), serde_json::Value::Bool(true));
            }
        }
        template
    }

    /// 书名 → 源系统 book_url（优先走 MCP get_book；失败则回退书名）。
    async fn resolve_mcp_book_ref(&self, server_id: &str, book_title: &str) -> Option<String> {
        let mut connections = crate::MCP_CONNECTIONS.lock().await;
        let client = connections.get_mut(server_id)?;
        if !client
            .get_tools()
            .iter()
            .any(|tool| tool.name == "get_book")
        {
            return None;
        }
        let result = client
            .call_tool("get_book", serde_json::json!({ "name": book_title }))
            .await
            .ok()?;
        let text = Self::extract_text(&result);
        let parsed: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
        Self::find_book_url(&parsed)
    }

    fn find_book_url(value: &serde_json::Value) -> Option<String> {
        if let Some(url) = value.get("book_url").and_then(|item| item.as_str()) {
            return Some(url.to_string());
        }
        if let Some(book) = value.get("book") {
            if let Some(url) = Self::find_book_url(book) {
                return Some(url);
            }
        }
        for key in ["books", "candidates", "items"] {
            if let Some(items) = value.get(key).and_then(|item| item.as_array()) {
                for item in items {
                    if let Some(url) = Self::find_book_url(item) {
                        return Some(url);
                    }
                }
            }
        }
        if let Some(items) = value.as_array() {
            for item in items {
                if let Some(url) = Self::find_book_url(item) {
                    return Some(url);
                }
            }
        }
        None
    }

    async fn call_mcp_chapter_tool(
        &self,
        server_id: &str,
        tool: &str,
        args: serde_json::Value,
    ) -> Result<String, String> {
        let mut connections = crate::MCP_CONNECTIONS.lock().await;
        let client = connections
            .get_mut(server_id)
            .ok_or_else(|| format!("MCP 服务器 {server_id} 未连接（请先在 MCP 页连接并保存）"))?;
        let result = client
            .call_tool(tool, args)
            .await
            .map_err(|e| format!("调用 {tool} 失败：{e}"))?;
        Ok(Self::extract_text(&result))
    }

    /// MCP 工具结果 → 正文文本（兼容 content[].text / text / 字符串）
    fn extract_text(value: &serde_json::Value) -> String {
        if let Some(s) = value.as_str() {
            return s.to_string();
        }
        if let Some(s) = value.get("text").and_then(|t| t.as_str()) {
            return s.to_string();
        }
        if let Some(items) = value.get("content").and_then(|c| c.as_array()) {
            let mut out = String::new();
            for item in items {
                if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                    out.push_str(t);
                }
            }
            return out;
        }
        serde_json::to_string_pretty(value).unwrap_or_default()
    }

    /// 灌入沙盒：每章一个 scene（sequence_number = 章号），正文走 SceneUpdate
    async fn seed_scenes(
        &self,
        story_id: &str,
        chapters: &[Chapter],
        ctx: &TaskExecutionContext,
    ) -> Result<(), String> {
        let repo =
            crate::db::repositories::scene_repository::SceneRepository::new(self.pool.clone());
        for (i, (num, content)) in chapters.iter().enumerate() {
            let scene = repo
                .create(story_id, (i + 1) as i32, Some(&format!("第 {num} 章")))
                .map_err(|e| format!("创建场景失败：{e}"))?;
            repo.update(
                &scene.id,
                &crate::db::repositories::scene_repository::SceneUpdate {
                    content: Some(content.clone()),
                    source: Some("continuation_training".to_string()),
                    ..Default::default()
                },
            )
            .map_err(|e| format!("写入第 {num} 章正文失败：{e}"))?;
        }
        ctx.log("info", &format!("已灌入 {} 章上下文", chapters.len()));
        Ok(())
    }

    /// StyleAnalyzer（真实章节采样）→ style_dnas 表 → 绑定沙盒故事
    async fn apply_style_dna(
        &self,
        story_id: &str,
        book_title: &str,
        chapters: &[Chapter],
        ctx: &TaskExecutionContext,
    ) -> bool {
        let sample: String = {
            let mut buf = String::new();
            for (_, content) in chapters {
                if buf.chars().count() >= STYLE_SAMPLE_CHARS {
                    break;
                }
                let remain = STYLE_SAMPLE_CHARS - buf.chars().count();
                buf.push_str(&content.chars().take(remain).collect::<String>());
                buf.push('\n');
            }
            buf
        };
        let llm = crate::llm::LlmService::new(self.app_handle.clone());
        match crate::creative_engine::style::StyleAnalyzer::analyze_with_llm(
            &sample,
            &format!("[训练] {book_title}"),
            &llm,
        )
        .await
        {
            Ok(dna) => {
                let dna_json = serde_json::to_string(&dna).unwrap_or_default();
                let repo = crate::db::repositories::style_dna_repository::StyleDnaRepository::new(
                    self.pool.clone(),
                );
                match repo.create(
                    &format!("[训练] {book_title}"),
                    dna.meta.author.as_deref(),
                    &dna_json,
                    false,
                ) {
                    Ok(record) => {
                        let story_repo = StoryRepository::new(self.pool.clone());
                        let req = crate::db::dto::UpdateStoryRequest {
                            style_dna_id: Some(record.id),
                            ..Default::default()
                        };
                        let _ = story_repo.update(story_id, &req);
                        ctx.log("info", "风格 DNA 已从真实章节生成并绑定沙盒");
                        true
                    }
                    Err(e) => {
                        ctx.log("warn", &format!("风格 DNA 保存失败（继续无风格训练）：{e}"));
                        false
                    }
                }
            }
            Err(e) => {
                ctx.log("warn", &format!("风格分析失败（继续无风格训练）：{e}"));
                false
            }
        }
    }

    /// 最近一轮报告的 improvement 建议拼成闭环提示。
    ///
    /// v0.93.16：只取与当前承接区间（`start_chapter..start+context_chapters`）
    /// 重叠的报告。跨区间建议是具体剧情/人设（如「从包玉芹送房继续」），
    /// 被注入到第 97 章之后的训练会直接污染续写（真机事故：现代都市陈默）。
    /// v0.98.7：只取最近一轮（此前取两轮，上一轮臆造的「杨先生/何小满」
    /// 与更早一轮的真实主线建议互相矛盾）；且过滤掉含上下文未接地人名的
    /// 建议行——上一轮编审描述的是**它自己的幻觉世界**，不能当本轮事实。
    fn load_prior_insights(
        &self,
        book_title: &str,
        start_chapter: usize,
        context_prose: &str,
        blind: bool,
    ) -> Option<String> {
        let conn = self.pool.get().ok()?;
        let mut stmt = conn
            .prepare(
                "SELECT improvement_md, story_id, chapter_reviews_json \
                 FROM continuation_training_reports \
                 WHERE book_title = ?1 AND improvement_md IS NOT NULL AND improvement_md != '' \
                 AND start_chapter <= ?2 \
                 AND ?2 < start_chapter + context_chapters \
                 ORDER BY created_at DESC LIMIT 1",
            )
            .ok()?;
        let row = stmt
            .query_row(rusqlite::params![book_title, start_chapter as i64,], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .ok();
        drop(stmt);
        let (improvement, story_id, reviews_json) = row?;
        let mut ungrounded = std::collections::HashSet::new();
        if let Some(story_id) = story_id.as_deref() {
            for name in self.ungrounded_character_names(story_id, context_prose) {
                ungrounded.insert(name);
            }
        }
        if let Some(value) = reviews_json
            .as_deref()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        {
            if let Some(items) = value.as_array() {
                for item in items {
                    for field in ["comments", "improvement"] {
                        if let Some(text) = item.get(field).and_then(|v| v.as_str()) {
                            collect_unquoted_ungrounded_names(text, context_prose, &mut ungrounded);
                        }
                    }
                }
            }
        }
        let mut filtered = filter_improvement_lines(&improvement, &ungrounded);
        if blind {
            filtered = strip_truth_hint_lines(&filtered);
        }
        if filtered.trim().is_empty() {
            return None;
        }
        Some(format!("最近一轮训练建议：\n{filtered}"))
    }

    /// 训练沙盒当前故事中「非权威」角色名（ingest/模型生成引入的）。
    /// 只在名字未出现在真实上下文时视为幻觉人名（「燕子」等原文已有称谓
    /// 不误伤）。
    fn ungrounded_character_names(&self, story_id: &str, context_prose: &str) -> Vec<String> {
        let Ok(conn) = self.pool.get() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT name FROM characters WHERE story_id = ?1 \
             AND COALESCE(source, '') NOT IN ('training_assets', 'user_created')",
        ) else {
            return Vec::new();
        };
        stmt.query_map(rusqlite::params![story_id], |r| r.get::<_, String>(0))
            .ok()
            .map(|rows| {
                rows.flatten()
                    .filter(|name| {
                        !name.trim().is_empty()
                            && !crate::agency::prose_ground::name_in_prose(name, context_prose)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 沙盒的权威人物名册（资产重建 source='training_assets'）。
    fn load_canonical_roster(&self, story_id: &str) -> Vec<(String, String)> {
        let Ok(conn) = self.pool.get() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT name, COALESCE(background, '') FROM characters \
             WHERE story_id = ?1 AND source = 'training_assets' \
             ORDER BY created_at LIMIT 12",
        ) else {
            return Vec::new();
        };
        stmt.query_map(rusqlite::params![story_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .ok()
        .map(|rows| rows.flatten().collect())
        .unwrap_or_default()
    }

    /// 上一轮编审报告 → 本轮修正（同书同区间；跨区间建议会污染剧情）。
    /// 读取最近一份重叠区间报告的维度均分与改进文本，供本轮指令自动加码。
    fn load_training_feedback(&self, book_title: &str, context_start: usize) -> TrainingFeedback {
        let mut feedback = TrainingFeedback::default();
        let Ok(conn) = self.pool.get() else {
            return feedback;
        };
        let mut stmt = match conn.prepare(
            "SELECT chapter_reviews_json, scores_json \
             FROM continuation_training_reports \
             WHERE book_title = ?1 AND start_chapter <= ?2 \
             AND ?2 < start_chapter + context_chapters + COALESCE(continuation_chapters, 1) \
             ORDER BY created_at DESC LIMIT 1",
        ) {
            Ok(s) => s,
            Err(_) => return feedback,
        };
        let row = stmt
            .query_row(rusqlite::params![book_title, context_start as i64], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                ))
            })
            .ok();
        drop(stmt);
        let Some((reviews_json, scores_json)) = row else {
            return feedback;
        };
        // 逐章维度得分 → 均分（优先）；旧版单章报告回退 scores_json。
        let mut dims: std::collections::HashMap<String, Vec<f64>> =
            std::collections::HashMap::new();
        if let Some(reviews) = reviews_json
            .as_deref()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
            .and_then(|value| value.as_array().cloned())
        {
            for review in reviews {
                for dim in JUDGE_DIMENSIONS {
                    push_dim(
                        &mut dims,
                        dim,
                        review.get("scores").and_then(|s| map_score(s, dim)),
                    );
                }
                push_dim(
                    &mut dims,
                    "continuity",
                    review.get("continuity").and_then(|v| v.as_f64()),
                );
            }
        }
        if dims.is_empty() {
            if let Some(scores) = scores_json
                .as_deref()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
            {
                for dim in JUDGE_DIMENSIONS {
                    push_dim(&mut dims, dim, map_score(&scores, dim));
                }
                push_dim(
                    &mut dims,
                    "continuity",
                    scores.get("continuity").and_then(|v| v.as_f64()),
                );
            }
        }
        for (key, values) in dims {
            if !values.is_empty() {
                feedback
                    .dimension_avgs
                    .insert(key, values.iter().sum::<f64>() / values.len() as f64);
            }
        }
        feedback
    }

    /// 编审 Agent 系统提示词（`agency_editor_auditor_system`，支持用户覆盖）。
    /// 追加「对比评审模式」：本次不走工具循环，直接按任务 JSON 输出。
    async fn editor_system_prompt(&self, premise: &str) -> String {
        let mut vars = std::collections::HashMap::new();
        vars.insert("premise".to_string(), premise.to_string());
        let pool = self.pool.clone();
        let resolved = tokio::task::spawn_blocking(move || {
            crate::prompts::registry::resolve_prompt_with_vars(
                &pool,
                "agency_editor_auditor_system",
                &vars,
            )
        })
        .await
        .ok()
        .and_then(|r| r.ok());
        let base = resolved.unwrap_or_else(|| {
            "你是「编辑审计」（编审），创作团队的终审编辑。你不改写正文，只出具裁决；\
             每条问题必须引用原文证据，没有证据的意见是噪音。"
                .to_string()
        });
        format!(
            "{base}\n\n【本次为续写对比评审模式】你没有黑板/工具可用，不要调用 board_read。\
             你会拿到 AI 续写、原作者的后续真实章与上一章结尾；按上述审查维度逐条对照，\
             但输出必须严格是任务要求的 JSON 对象。"
        )
    }

    /// 单章编审：AI 续写 vs 原书真实后续章。两轮尝试，仍不可用则降级为
    /// 「评分不可用」行（不落假 0 分、不判死整轮训练）。
    #[allow(clippy::too_many_arguments)]
    async fn review_chapter(
        &self,
        llm: &crate::llm::LlmService,
        system: &str,
        book_title: &str,
        truth_chapter: usize,
        generated_chapter: i32,
        truth_text: &str,
        generated_text: &str,
        prior_chapter: usize,
        prior_tail: &str,
    ) -> ChapterReviewOutcome {
        if generated_text.trim().is_empty() {
            return ChapterReviewOutcome {
                generated_chapter,
                truth_chapter,
                verdict: "unavailable".into(),
                overall: None,
                scores: serde_json::Map::new(),
                continuity: None,
                reference_gap: None,
                comments: "本章 AI 续写缺失（gate 失败或空稿），无法对比".into(),
                improvement: String::new(),
                prose_available: false,
            };
        }
        let truth_plain = crate::export::content::content_to_plain_text(truth_text);
        let truth_cap = Self::cap_review(&truth_plain);
        let gen_cap = Self::cap_review(generated_text);
        let mut judge: Option<JudgeReport> = None;
        let mut last_raw = String::new();
        // v0.99.7：第三次改用更短采样 + 换任务类型（CreativeWriting 路由到
        // 非审计模型）。真机 v0.99.6 deepseek-v4 在 ~7.6k prompt 上连续
        // CoT-only（completion 8-9k、content 空），第三次必须降 prompt
        // 并换路由。
        for attempt in 0..=2 {
            let (truth_arg, gen_arg) = if attempt == 2 {
                (
                    Self::cap_review_with_limit(&truth_plain, 2_500),
                    Self::cap_review_with_limit(generated_text, 2_500),
                )
            } else {
                (truth_cap.clone(), gen_cap.clone())
            };
            let prompt = Self::review_prompt(
                book_title,
                &truth_arg,
                &gen_arg,
                truth_chapter,
                generated_chapter,
                prior_chapter,
                prior_tail,
                attempt > 0,
            );
            // v0.98.7：JSON mode + 3200 tokens。旧版 1600 tokens 常在
            // comments/improvement 中途截断（真机 3/10 章「编审输出无法
            // 解析」）；JSON mode 也让推理模型在 content 为空时从思维链
            // 提取完整裁决。
            let (task, max_tokens) = if attempt == 2 {
                (crate::router::TaskType::CreativeWriting, 4_096)
            } else {
                (crate::router::TaskType::Proofreading, 3_200)
            };
            let (_, result) = llm
                .generate_for_task_with_system_prompt_and_format(
                    task,
                    prompt,
                    Some(max_tokens),
                    Some(0.2),
                    Some("continuation_training_chapter_review"),
                    Some(system.to_string()),
                    Some(crate::llm::adapter::ResponseFormat::JsonObject),
                    None,
                )
                .await;
            match result {
                Ok(resp) => {
                    last_raw = resp.content;
                    judge = parse_judge_report(&last_raw);
                    if judge.is_some() {
                        break;
                    }
                }
                Err(e) => {
                    log::warn!(
                        "[ContinuationTraining] 第 {} 对编审调用失败（第 {} 次）：{}",
                        generated_chapter,
                        attempt + 1,
                        e
                    );
                }
            }
        }
        let Some(judge) = judge else {
            log::warn!(
                "[ContinuationTraining] 第 {} 对编审输出无可用五维（已重试）：{}",
                generated_chapter,
                Self::preview_chars(&last_raw, 240)
            );
            return ChapterReviewOutcome {
                generated_chapter,
                truth_chapter,
                verdict: "unavailable".into(),
                overall: None,
                scores: serde_json::Map::new(),
                continuity: None,
                reference_gap: None,
                comments: format!(
                    "评分不可用（编审输出无法解析）：{}",
                    Self::preview_chars(&last_raw, 120)
                ),
                improvement: String::new(),
                prose_available: true,
            };
        };
        ChapterReviewOutcome {
            generated_chapter,
            truth_chapter,
            verdict: judge.verdict,
            overall: Some(judge.overall),
            scores: judge.scores,
            continuity: judge.continuity,
            reference_gap: judge.reference_gap,
            comments: judge.comments,
            improvement: judge.improvement,
            prose_available: true,
        }
    }

    /// 逐章对比提示词。`strict` 为二次尝试：上一次输出无法解析时，要求只给
    /// 纯 JSON 对象，不再夹带解释文字或代码围栏。
    #[allow(clippy::too_many_arguments)]
    fn review_prompt(
        book_title: &str,
        truth_cap: &str,
        gen_cap: &str,
        truth_chapter: usize,
        generated_chapter: i32,
        prior_chapter: usize,
        prior_tail: &str,
        strict: bool,
    ) -> String {
        let strict_line = if strict {
            "上一次输出无法解析。这次严格只输出一个 JSON 对象（不要 markdown、不要解释文字、不要代码围栏）。\n\n"
        } else {
            ""
        };
        let prior_block = if prior_tail.trim().is_empty() || prior_chapter == 0 {
            String::new()
        } else {
            format!(
                "【上一章结尾（沙盒第 {prior_chapter} 章；唯一的承接基准）】\n\
                 {prior_tail}\n\n"
            )
        };
        format!(
            r#"{strict_line}你是编审。下面是《{book_title}》第 {truth_chapter} 章的原作者真实原文（仅用于衡量「与原书后续差距」，不是承接基准），以及 AI 按前文续写出的对应章（沙盒第 {generated_chapter} 章，对应真实第 {truth_chapter} 章）。请做「续写质量对比评审」：按五维打分（1-10，10=与作者本人水准不可区分），并分别判断沙盒承接与原书差距。

评分口径：
- style 文笔风格：句式长度分布、用词、语感、时代感、生活细节密度；
- plot 情节推进：本章**内部**是否完成真实的状态变化（信息/关系/目标/风险/情绪），而非原地踏步或说明性对白；不以是否撞见原书后续剧情扣分；
- voice 人物口吻：对白与反应是否符合各角色身份、伤口/需求/防御，而非全员同一腔调；
- pacing 节奏密度：详略比例、信息密度、有无注水铺垫；
- hooks 钩子悬念：开头抓力与章末钩子经营。
承接 continuity（1-10）：**只相对上一沙盒章**——AI 续写是否紧接第 {prior_chapter} 章结尾的现场、人物、时代与未完结事件（禁止换场、新主角、新委托、新事件）。即使 AI 未采用原书后续的真实走向，只要紧接上一沙盒章，continuity 仍应给高分。
原书差距 reference_gap（1-10）：AI 续写与原作者后续真实章的世界线/事件/人物距离；10=几乎一致，1=完全另开一条线。只做诊断，不参与 continuity。

{prior_block}【原作者的后续真实章（仅用于 reference_gap，不是承接基准）】
{truth_cap}

【AI 续写】
{gen_cap}

仅输出 JSON：
{{"verdict":"pass 或 revise","scores":{{"style":1-10,"plot":1-10,"voice":1-10,"pacing":1-10,"hooks":1-10}},"continuity":1-10,"reference_gap":1-10,"overall":0.0-1.0加权总分,"comments":"每维 1-2 句依据（含沙盒承接判断与原书差距，≤200字）","improvement":"下一轮续写必须落实的 2-3 条具体改进（含承接修正，给模型看，不超过 150 字）","blocking_issues":["带原文证据的阻断问题（可空）"]}}"#
        )
    }

    /// 跨章总评：编审 Agent 汇总逐章对比，做全面对比分析并给出总体改进。
    #[allow(clippy::too_many_arguments)]
    async fn review_synthesis(
        &self,
        llm: &crate::llm::LlmService,
        system: &str,
        book_title: &str,
        context_n: usize,
        truth_n: usize,
        truths: &[Chapter],
        generated: &[(i32, String)],
        reviews: &[ChapterReviewOutcome],
    ) -> Option<SynthesisReport> {
        let prompt =
            Self::synthesis_prompt(book_title, context_n, truth_n, truths, generated, reviews);
        let (_, result) = llm
            .generate_for_task_with_system_prompt_and_format(
                crate::router::TaskType::Analysis,
                prompt,
                Some(4096),
                Some(0.2),
                Some("continuation_training_synthesis"),
                Some(system.to_string()),
                Some(crate::llm::adapter::ResponseFormat::JsonObject),
                None,
            )
            .await;
        parse_synthesis(&result.ok()?.content)
    }

    fn synthesis_prompt(
        book_title: &str,
        context_n: usize,
        truth_n: usize,
        truths: &[Chapter],
        generated: &[(i32, String)],
        reviews: &[ChapterReviewOutcome],
    ) -> String {
        let first_truth = truths.first().map(|c| c.0).unwrap_or(0);
        let last_truth = truths.last().map(|c| c.0).unwrap_or(first_truth);
        let mut summary = String::new();
        for review in reviews {
            let dims = JUDGE_DIMENSIONS
                .iter()
                .map(|dim| {
                    format!(
                        "{dim}={}",
                        review
                            .scores
                            .get(*dim)
                            .map(fmt_num)
                            .unwrap_or_else(|| "-".into())
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            summary.push_str(&format!(
                "- 沙盒第 {} 章 vs 原书第 {} 章：overall={}｜{}｜承接={}｜裁决={}\n  摘要：{}\n",
                review.generated_chapter,
                review.truth_chapter,
                review
                    .overall
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_else(|| "-".into()),
                dims,
                review
                    .continuity
                    .map(|v| format!("{v:.0}"))
                    .unwrap_or_else(|| "-".into()),
                review.verdict,
                Self::preview_chars(&review.comments, 160).replace('\n', " ")
            ));
        }
        // 跨章走势需要开头/结尾摘录：每章取真实章与续写的开头/结尾各一小段。
        let mut excerpts = String::new();
        for (i, (truth_num, truth_text)) in truths.iter().take(truth_n).enumerate() {
            let truth_plain = crate::export::content::content_to_plain_text(truth_text);
            let gen = generated
                .get(i)
                .map(|(_, text)| text.as_str())
                .unwrap_or("");
            excerpts.push_str(&format!(
                "【原书第 {truth_num} 章】开头：{}｜结尾：{}\n",
                Self::preview_chars(&truth_plain, SYNTH_HEAD_CHARS).replace('\n', " "),
                tail_chars(&truth_plain, SYNTH_TAIL_CHARS).replace('\n', " ")
            ));
            if !gen.trim().is_empty() {
                excerpts.push_str(&format!(
                    "【AI 沙盒第 {} 章】开头：{}｜结尾：{}\n",
                    (context_n + 1 + i) as i32,
                    Self::preview_chars(gen, SYNTH_HEAD_CHARS).replace('\n', " "),
                    tail_chars(gen, SYNTH_TAIL_CHARS).replace('\n', " ")
                ));
            }
        }
        format!(
            r#"你是编审（EditorAuditor），现在对一次「连续续写训练」做全面对比分析。
《{book_title}》原书第 {first_truth}–{last_truth} 章是作者真实后续（基准）；AI 在 {context_n} 章上下文后连续续写 {truth_n} 章，逐章对比结果如下。

【逐章对比汇总】
{summary}

【开头/结尾摘录（跨章走势用）】
{excerpts}

请做跨章全面对比分析：
1. 走势：AI 连续续写是否像原作者一样推进（人物弧、冲突升级、线索经营），还是逐章风格漂移/原地踏步；
2. 跨章一致性：人物口吻、时代语感、地点与未完结事件是否连续，有无自相矛盾或记忆丢失；
3. 与真实后续的差距：结构、节奏、钩子、信息密度、情感回路哪里最不像作者；
4. 可执行改进：给出下一轮续写必须落实的具体指令（给 writer 看，不是泛泛而谈）。

仅输出 JSON（不要 markdown、不要解释文字）：
{{"overall":0.0-1.0总体分,"dimensions":{{"style":1-10,"plot":1-10,"voice":1-10,"pacing":1-10,"hooks":1-10,"continuity":1-10}},"cross_chapter":"跨章走势的 2-4 条发现（分号分隔）","strengths":["做得好的具体点"],"weaknesses":["具体短板"],"improvement":["下一轮必须落实的具体指令（3-6 条，每条 ≤60 字）"],"comments":"总评（≤200字）"}}"#
        )
    }

    /// 报告排版（确定性）+ 趋势分 + 下一轮改进文本。
    #[allow(clippy::too_many_arguments)]
    fn build_report(
        book_title: &str,
        context_n: usize,
        truth_n: usize,
        context: &[Chapter],
        truths: &[Chapter],
        reviews: &[ChapterReviewOutcome],
        synthesis: Option<&SynthesisReport>,
        asset_line: &str,
        gate_passed: usize,
        gate_degraded: usize,
        revised_count: usize,
        gate_unrevised: usize,
        style_applied: bool,
        feedback: &TrainingFeedback,
        ungrounded: &std::collections::HashSet<String>,
    ) -> (String, String, String) {
        let first_gen = (context_n + 1) as i32;
        let last_gen = first_gen + truth_n as i32 - 1;
        let first_truth = truths.first().map(|c| c.0).unwrap_or(0);
        let last_truth = truths.last().map(|c| c.0).unwrap_or(first_truth);
        let dim_avgs: Vec<(&str, Option<f64>)> = JUDGE_DIMENSIONS
            .iter()
            .map(|dim| (*dim, avg_dim(reviews, dim)))
            .chain(std::iter::once((
                "continuity",
                avg_dim(reviews, "continuity"),
            )))
            .chain(std::iter::once((
                "reference_gap",
                avg_dim(reviews, "reference_gap"),
            )))
            .collect();
        let overalls: Vec<f64> = reviews.iter().filter_map(|r| r.overall).collect();
        let overall = if overalls.is_empty() {
            None
        } else {
            Some(overalls.iter().sum::<f64>() / overalls.len() as f64)
        };
        // v0.98.7：gate 的「降级放行」（编辑审计熔断/解析失败）不再冒充
        // pass；编审判裁单独统计，避免「10 章通过」与逐章 7 章 revise 的
        // 自相矛盾报告。
        let audit_pass = reviews
            .iter()
            .filter(|r| r.verdict == "pass" && r.overall.is_some())
            .count();
        let audit_revise = reviews
            .iter()
            .filter(|r| r.verdict == "revise" && r.overall.is_some())
            .count();
        let audit_unavailable = reviews
            .iter()
            .filter(|r| r.verdict == "unavailable" || r.overall.is_none())
            .count();
        let gate_line = render_gate_line(gate_passed, revised_count, gate_degraded, gate_unrevised);

        let mut rows = String::new();
        for review in reviews {
            let dims = JUDGE_DIMENSIONS
                .iter()
                .map(|dim| {
                    review
                        .scores
                        .get(*dim)
                        .map(fmt_num)
                        .unwrap_or_else(|| "-".into())
                })
                .collect::<Vec<_>>()
                .join(" | ");
            rows.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                review.generated_chapter,
                review.truth_chapter,
                review.verdict,
                review
                    .overall
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_else(|| "-".into()),
                dims,
                review
                    .continuity
                    .map(|v| format!("{v:.0}"))
                    .unwrap_or_else(|| "-".into()),
                review
                    .reference_gap
                    .map(|v| format!("{v:.0}"))
                    .unwrap_or_else(|| "-".into()),
                Self::preview_chars(&review.comments, 70).replace('|', "/")
            ));
        }

        let dim_lines = dim_avgs
            .iter()
            .map(|(dim, avg)| {
                format!(
                    "- **{}**：{}{}",
                    dimension_label(dim),
                    avg.map(|v| format!("{v:.1}")).unwrap_or_else(|| "-".into()),
                    dimension_hint(dim)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let cross = synthesis
            .map(|s| s.cross_chapter.clone())
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| deterministic_cross(reviews, &dim_avgs));
        let strengths = synthesis
            .map(|s| s.strengths.clone())
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| {
                dim_avgs
                    .iter()
                    .filter_map(|(dim, avg)| {
                        avg.filter(|v| *v >= 7.0)
                            .map(|v| format!("{}（{v:.1}/10）", dimension_label(dim)))
                    })
                    .collect()
            });
        let weaknesses = synthesis
            .map(|s| s.weaknesses.clone())
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| {
                dim_avgs
                    .iter()
                    .filter_map(|(dim, avg)| {
                        avg.filter(|v| *v < FEEDBACK_THRESHOLD).map(|v| {
                            format!(
                                "{}（{v:.1}/10）：{}",
                                dimension_label(dim),
                                dimension_hint(dim)
                            )
                        })
                    })
                    .collect()
            });
        let comments = synthesis.map(|s| s.comments.clone()).unwrap_or_default();
        let improvement_md = derive_improvements(reviews, synthesis, &dim_avgs, ungrounded);

        let mut md = format!(
            "# 续写质量分析报告 · 《{book_title}》\n\n\
             - 训练区间：原书第 {}–{} 章（{} 章上下文）；连续续写 {} 章（沙盒第 {}–{} 章）对比原书第 {}–{} 章\n\
             - 编审 Agent：EditorAuditor（agency_editor_auditor_system）逐章对比 + 跨章总评\n\
             - 加权总分：{}\n\
             - 生成质量门：{gate_line}\n\
             - 编审判裁：{audit_pass} 章 pass / {audit_revise} 章 revise / {audit_unavailable} 章不可用（逐章对比裁定）\n\n\
             ## 五维均值\n\n{}\n\n\
             ## 逐章对比\n\n\
             | 沙盒章 | 原书章 | 裁决 | 总分 | {} | 承接（沙盒） | 原书差距 | 摘要 |\n\
             |---|---|---|---|---|---|---|---|\n\
             {rows}\n\
             ## 跨章全面对比\n\n",
            context.first().map(|c| c.0).unwrap_or(0),
            context.last().map(|c| c.0).unwrap_or(0),
            context_n,
            truth_n,
            first_gen,
            last_gen,
            first_truth,
            last_truth,
            overall
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "评分不可用".into()),
            dim_lines,
            JUDGE_DIMENSIONS
                .iter()
                .map(|dim| dimension_label(dim).to_string())
                .collect::<Vec<_>>()
                .join(" | "),
        );
        for item in cross.iter() {
            md.push_str(&format!("- {item}\n"));
        }
        md.push_str("\n## 强项\n\n");
        if strengths.is_empty() {
            md.push_str("- 本轮未发现明显强项\n");
        } else {
            for item in &strengths {
                md.push_str(&format!("- {item}\n"));
            }
        }
        md.push_str("\n## 短板\n\n");
        if weaknesses.is_empty() {
            md.push_str("- 本轮无低于阈值的明显短板\n");
        } else {
            for item in &weaknesses {
                md.push_str(&format!("- {item}\n"));
            }
        }
        if !comments.trim().is_empty() {
            md.push_str(&format!("\n## 编审评语\n\n{}\n", comments));
        }
        md.push_str(&format!(
            "\n## 下一轮改进（自动注入续写指令）\n\n{improvement_md}\n\n\
             ## 本轮训练流程\n\
             - 资产重建：{asset_line}\n\
             - 编审：逐章对比 {} 对（编审 Agent）+ 跨章总评；生成质量门 {gate_line}；\
             编审判裁 {audit_pass} pass / {audit_revise} revise / {audit_unavailable} 不可用\n\
             - 风格：{}\n\
             - 下一轮：自动注入本报告改进项，并按上一轮低分维度自动加码修正\n",
            reviews.len(),
            if style_applied {
                "StyleDNA 已绑定"
            } else {
                "StyleDNA 不可用（已用原文锚点）"
            },
        ));
        if !feedback.dimension_avgs.is_empty() {
            md.push_str(&format!(
                "- 闭环基线：上一轮报告维度均分 {}\n",
                feedback
                    .dimension_avgs
                    .iter()
                    .map(|(k, v)| format!("{k}={v:.1}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }

        let scores_json = serde_json::json!({
            "overall": overall,
            "scores": dim_avgs.iter().filter(|(dim, _)| *dim != "continuity" && *dim != "reference_gap").map(|(dim, avg)| (dim.to_string(), avg.map(|v| serde_json::json!(v)))).collect::<std::collections::HashMap<_, _>>(),
            "continuity": dim_avgs.iter().find(|(dim, _)| *dim == "continuity").and_then(|(_, avg)| *avg),
            "reference_gap": dim_avgs.iter().find(|(dim, _)| *dim == "reference_gap").and_then(|(_, avg)| *avg),
            "chapters": truth_n,
            "reviewed": reviews.iter().filter(|r| r.overall.is_some()).count(),
        })
        .to_string();
        (md, scores_json, improvement_md)
    }

    fn preview_chars(text: &str, limit: usize) -> String {
        text.chars().take(limit).collect()
    }

    /// 清理同书更早的沙盒（保留最近 keep 个；级联清理走 story 删除事务）
    fn cleanup_old_sandboxes(&self, book_title: &str, keep: usize) {
        let conn = match self.pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let prefix = format!("[训练]《{book_title}》");
        let ids: Vec<String> = {
            let mut stmt = match conn.prepare(
                "SELECT id FROM stories WHERE title LIKE ?1 || '%' ORDER BY created_at DESC",
            ) {
                Ok(s) => s,
                Err(_) => return,
            };
            let rows = stmt
                .query_map(rusqlite::params![prefix], |r| r.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<String>>());
            match rows {
                Ok(v) => {
                    drop(stmt);
                    v
                }
                Err(_) => return,
            }
        };
        if ids.len() > keep {
            let story_repo = StoryRepository::new(self.pool.clone());
            for id in ids.into_iter().skip(keep) {
                match story_repo.delete(&id) {
                    Ok(_) => log::info!("[ContinuationTraining] 已清理旧沙盒 {id}"),
                    Err(e) => log::warn!("[ContinuationTraining] 清理旧沙盒 {id} 失败：{e}"),
                }
            }
        }
        // v0.99.1：前情提要随沙盒删除级联清理，避免孤儿 story_briefs 残留。
        let _ = crate::agency::story_brief::StoryBriefRepository::new(self.pool.clone())
            .prune_orphans();
    }
}

/// 判官输出（已校验）：五维分数 + 归一化总分 + 评语/改进 + 承接分/裁决。
struct JudgeReport {
    scores: serde_json::Map<String, serde_json::Value>,
    overall: f64,
    comments: String,
    improvement: String,
    verdict: String,
    continuity: Option<f64>,
    /// v0.99.2：与原书后续真实章的差距（1-10，10=几乎一致），仅作参考诊断；
    /// 不得混入 continuity（承接只看上一沙盒章）。
    reference_gap: Option<f64>,
}

const JUDGE_DIMENSIONS: [&str; 5] = ["style", "plot", "voice", "pacing", "hooks"];

/// 逐章编审结果（落 `chapter_reviews_json`，也是总评/趋势的输入）。
#[derive(Debug, Clone, Serialize)]
struct ChapterReviewOutcome {
    generated_chapter: i32,
    truth_chapter: usize,
    verdict: String,
    overall: Option<f64>,
    scores: serde_json::Map<String, serde_json::Value>,
    continuity: Option<f64>,
    /// 与原书后续差距（诊断）。
    reference_gap: Option<f64>,
    comments: String,
    improvement: String,
    prose_available: bool,
}

/// 编审跨章总评（结构化；解析失败时报告降级为逐章汇总）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SynthesisReport {
    #[serde(default)]
    overall: Option<serde_json::Value>,
    #[serde(default)]
    dimensions: Option<serde_json::Value>,
    #[serde(default)]
    cross_chapter: Vec<String>,
    #[serde(default)]
    strengths: Vec<String>,
    #[serde(default)]
    weaknesses: Vec<String>,
    #[serde(default)]
    improvement: Vec<String>,
    #[serde(default)]
    comments: String,
}

/// 生成质量门一行。每章只归一类（降级 > 已修订 > 通过 > 判修订未完成），
/// 四项之和等于批量章数——旧报告 3+0+2=5 与 10 章不符，读者无法判断其余
/// 5 章去向。
fn render_gate_line(passed: usize, revised: usize, degraded: usize, unrevised: usize) -> String {
    let mut line = format!(
        "{passed} 章通过 / {revised} 章修订 / {degraded} 章质检降级（编辑审计未完成，已放行保产出）"
    );
    if unrevised > 0 {
        line.push_str(&format!(
            " / {unrevised} 章编审判修订但修订未完成（保留首稿装配）"
        ));
    }
    line
}

/// 上一轮编审报告 → 本轮指令加码（自动闭环）。
#[derive(Debug, Clone, Default)]
struct TrainingFeedback {
    dimension_avgs: std::collections::HashMap<String, f64>,
}

impl TrainingFeedback {
    fn dim(&self, key: &str) -> Option<f64> {
        self.dimension_avgs.get(key).copied()
    }

    fn dim_low(&self, key: &str) -> bool {
        self.dim(key)
            .map(|value| value < FEEDBACK_THRESHOLD)
            .unwrap_or(false)
    }

    fn style_low(&self) -> bool {
        self.dim_low("style")
    }

    fn continuity_low(&self) -> bool {
        self.dim_low("continuity")
    }

    /// 低分维度 → 本轮硬性写作修正（仅列有数据的维度；无数据不伪造）。
    fn correction_block(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for dim in ["continuity", "style", "plot", "voice", "pacing", "hooks"] {
            if self.dim_low(dim) {
                let avg = self.dim(dim).unwrap_or(0.0);
                lines.push(format!(
                    "{}（上一轮 {avg:.1}/10）：{}",
                    dimension_label(dim),
                    dimension_correction(dim)
                ));
            }
        }
        lines
    }
}

/// 维度中文名（报告/指令统一口径）。
fn dimension_label(dim: &str) -> &'static str {
    match dim {
        "style" => "文笔风格",
        "plot" => "情节推进",
        "voice" => "人物口吻",
        "pacing" => "节奏密度",
        "hooks" => "钩子悬念",
        "continuity" => "承接（沙盒）",
        "reference_gap" => "原书差距（诊断）",
        _ => "未知维度",
    }
}

/// 维度修正提示（静态、可执行；用于报告短板与下一轮指令）。
fn dimension_correction(dim: &str) -> &'static str {
    match dim {
        "continuity" => "紧接上一沙盒章末尾现场（同地点/同人物/同时代/未完结事件），禁止跳切、新主角、新委托、新场景。",
        "style" => "逐段模仿原文的句式长度、用词与生活细节密度；禁止书面化、翻译腔、总结句与抒情收尾。",
        "plot" => "每章必须发生可验证的状态变化（信息/关系/目标/风险/情绪），不得只写说明性对白或原地踏步。",
        "voice" => "对白必须贴合各角色身份、时代语感与伤口/防御，不得全员同一腔调或腔调轮换式标签。",
        "pacing" => "保留原文详略比例与信息密度；禁止注水式铺垫，章末控制为小悬念而非收束。",
        "hooks" => "开头 300 字内建立张力，章末留未完结钩子；禁止解释、预告或总结式收尾。",
        _ => "",
    }
}

/// 报告中的维度提示（简短，用于均值行）。
fn dimension_hint(dim: &str) -> &'static str {
    match dim {
        "style" => "（句式/用词/细节密度）",
        "plot" => "（状态变化/推进）",
        "voice" => "（对白/反应）",
        "pacing" => "（详略/信息密度）",
        "hooks" => "（开头抓力/章末钩子）",
        "continuity" => "（紧接上一沙盒章现场）",
        "reference_gap" => "（与原书后续差距，仅诊断）",
        _ => "",
    }
}

#[derive(serde::Deserialize)]
struct RawJudgeReport {
    #[serde(default)]
    scores: Option<serde_json::Value>,
    #[serde(default)]
    overall: Option<serde_json::Value>,
    #[serde(default)]
    comments: Option<serde_json::Value>,
    #[serde(default)]
    improvement: Option<serde_json::Value>,
    #[serde(default)]
    verdict: Option<serde_json::Value>,
    #[serde(default)]
    continuity: Option<serde_json::Value>,
    #[serde(default)]
    reference_gap: Option<serde_json::Value>,
}

/// 结构化评审输出的宽容解析：宽松 JSON → 补全截断括号 → 取最后一个完整
/// 对象。v0.98.7：续写训练编审曾因输出截断 3/10 章判不可用。
fn parse_json_lenient<T: serde::de::DeserializeOwned>(raw: &str) -> Option<T> {
    // 截断修复必须排在宽松解析前面：parse_lenient 对截断 JSON 可能吐出
    // 一个「全字段 default 的空对象」（RawJudgeReport 全字段 serde default），
    // 空壳会被当成功、导致五维缺失。先补括号才能拿到真实字段。
    if let Some(fixed) = crate::utils::json::complete_truncated_json(raw) {
        if let Ok(v) = serde_json::from_str::<T>(&fixed) {
            return Some(v);
        }
    }
    if let Some(v) = crate::agency::coordinator::parse_lenient::<T>(raw) {
        return Some(v);
    }
    if let Some(json) = crate::utils::json::extract_last_json_object(raw) {
        if let Ok(v) = serde_json::from_str::<T>(&json) {
            return Some(v);
        }
    }
    None
}

/// 解析并校验判官输出：五维必须齐全且为 1-10 的数值（或数字字符串/
/// "6/10"）；缺失任一分维度即视为不可用（返回 None），由调用方重试，
/// 不再落库「总分 0.00、五维 -」的假报告。overall 缺失时按五维均值折算。
fn parse_judge_report(raw: &str) -> Option<JudgeReport> {
    // v0.98.7：编审输出常在 JSON 中途被截断（真机 comments 后断句）。
    // 先走既有宽松解析；失败时补全括号，再取最后一个完整对象。
    let parsed: RawJudgeReport = parse_json_lenient(raw)?;
    let scores_obj = parsed.scores?.as_object()?.clone();
    let mut scores = serde_json::Map::new();
    let mut sum = 0.0;
    for dimension in JUDGE_DIMENSIONS {
        let score = coerce_dimension_score(scores_obj.get(dimension)?)?;
        sum += score;
        scores.insert(
            dimension.to_string(),
            if score.fract() == 0.0 {
                serde_json::json!(score as i64)
            } else {
                serde_json::json!(score)
            },
        );
    }
    let mean_10 = sum / JUDGE_DIMENSIONS.len() as f64;
    let overall = coerce_overall(parsed.overall.as_ref()).unwrap_or(mean_10 / 10.0);
    Some(JudgeReport {
        scores,
        overall: overall.clamp(0.0, 1.0),
        comments: value_as_text(parsed.comments.as_ref()).unwrap_or_default(),
        improvement: value_as_text(parsed.improvement.as_ref()).unwrap_or_default(),
        verdict: value_as_text(parsed.verdict.as_ref()).unwrap_or_else(|| "pass".into()),
        continuity: parsed.continuity.as_ref().and_then(coerce_dimension_score),
        reference_gap: parsed
            .reference_gap
            .as_ref()
            .and_then(coerce_dimension_score),
    })
}

fn coerce_dimension_score(value: &serde_json::Value) -> Option<f64> {
    let number = match value {
        serde_json::Value::Number(number) => number.as_f64()?,
        serde_json::Value::String(text) => {
            let head = text
                .trim()
                .split('/')
                .next()?
                .trim()
                .trim_end_matches('分')
                .trim();
            head.parse::<f64>().ok()?
        }
        _ => return None,
    };
    if (1.0..=10.0).contains(&number) {
        Some(number)
    } else {
        None
    }
}

fn coerce_overall(value: Option<&serde_json::Value>) -> Option<f64> {
    let number = match value? {
        serde_json::Value::Number(number) => number.as_f64()?,
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok()?,
        _ => return None,
    };
    if (0.0..=1.0).contains(&number) {
        Some(number)
    } else if (0.0..=10.0).contains(&number) {
        Some(number / 10.0)
    } else {
        None
    }
}

fn value_as_text(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn fmt_num(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn push_dim(dims: &mut std::collections::HashMap<String, Vec<f64>>, key: &str, value: Option<f64>) {
    if let Some(value) = value {
        dims.entry(key.to_string()).or_default().push(value);
    }
}

fn map_score(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|item| match item {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    })
}

/// 逐章维度均分（1-10）；无有效样本返回 None（不落假 0）。
fn avg_dim(reviews: &[ChapterReviewOutcome], dim: &str) -> Option<f64> {
    let values: Vec<f64> = reviews
        .iter()
        .filter_map(|review| {
            if dim == "continuity" {
                review.continuity
            } else if dim == "reference_gap" {
                review.reference_gap
            } else {
                review.scores.get(dim).and_then(|value| match value {
                    serde_json::Value::Number(number) => number.as_f64(),
                    serde_json::Value::String(text) => text.trim().parse::<f64>().ok(),
                    _ => None,
                })
            }
        })
        .collect();
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn text_list(value: Option<&serde_json::Value>) -> Vec<String> {
    match value {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .filter_map(|item| value_as_text(Some(item)))
            .filter(|text| !text.trim().is_empty())
            .collect(),
        Some(serde_json::Value::String(text)) => text
            .split([';', '；', '\n'])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// 编审跨章总评（宽容解析：数组可为字符串；任一字段可用即接受）。
fn parse_synthesis(raw: &str) -> Option<SynthesisReport> {
    let value: serde_json::Value = parse_json_lenient(raw)?;
    let object = value.as_object()?;
    let mut report = SynthesisReport {
        overall: object.get("overall").cloned(),
        dimensions: object.get("dimensions").cloned(),
        cross_chapter: text_list(object.get("cross_chapter")),
        strengths: text_list(object.get("strengths")),
        weaknesses: text_list(object.get("weaknesses")),
        improvement: text_list(object.get("improvement")),
        comments: value_as_text(object.get("comments")).unwrap_or_default(),
    };
    if report.improvement.is_empty()
        && report.cross_chapter.is_empty()
        && report.comments.trim().is_empty()
        && report.dimensions.is_none()
    {
        return None;
    }
    report.improvement = report
        .improvement
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .map(|item| item.trim().to_string())
        .collect();
    Some(report)
}

/// 无编审总评时的确定性跨章分析（逐章走势 + 低分维度），保证报告不空。
fn deterministic_cross(
    reviews: &[ChapterReviewOutcome],
    dim_avgs: &[(&str, Option<f64>)],
) -> Vec<String> {
    let mut out = Vec::new();
    let trend = reviews
        .iter()
        .map(|review| {
            format!(
                "第{}章 {}",
                review.generated_chapter,
                review
                    .overall
                    .map(|v| format!("{v:.2}"))
                    .unwrap_or_else(|| "-".into())
            )
        })
        .collect::<Vec<_>>()
        .join(" → ");
    out.push(format!("逐章总分走势：{trend}"));
    for (dim, avg) in dim_avgs {
        if let Some(value) = avg.filter(|v| *v < FEEDBACK_THRESHOLD) {
            out.push(format!(
                "{}均值 {value:.1}/10，低于阈值：{}",
                dimension_label(dim),
                dimension_correction(dim)
            ));
        }
    }
    out
}

/// v0.98.7：gate「降级放行」判定。编辑审计熔断/解析失败时
/// salvage_failed_gate 会合成 pass，但 comments 带降级标记——报告必须
/// 区分真实通过、修订与「质检未完成」。
fn is_degraded_gate_comment(comments: &str) -> bool {
    const MARKERS: [&str; 4] = ["降级放行", "熔断", "审查未完成", "编辑审计失败"];
    MARKERS.iter().any(|marker| comments.contains(marker))
}

/// v0.98.7：构建权威人物名册块。名册来自资产重建（training_assets），
/// 全部在实际上下文中出现过；把「唯一可用具名人物 + 主角不得另起姓名」
/// 写成硬约束。此前 writer 在护士场景自行命名「杨先生/何小满」，随后被
/// ingest 写回资产并逐章放大。
fn build_roster_block(roster: &[(String, String)]) -> String {
    if roster.is_empty() {
        // v0.99.1：即使资产重建 0 角色（真机 1118），「第一人称不得另起姓名 /
        // 路人不得命名」也必须在 prompt 里；否则 writer 会自行发明「杨先生」。
        return "【权威人物名册（本轮未登记具名角色）】\n写作硬约束：\n\
                - 原文用第一人称「我」叙述时，不得给主角另起姓名称呼；\n\
                - 不得把路人/次要角色命名（护士、摊主、警员、司机只用身份称呼）；\n\
                - 只可使用前情提要中已经出现的人名；名册外的新姓名一律视为幻觉。"
            .to_string();
    }
    let mut lines = String::new();
    for (idx, (name, background)) in roster.iter().take(12).enumerate() {
        let role = if idx == 0 { "（主角）" } else { "" };
        let bg: String = background.trim().chars().take(60).collect();
        if bg.is_empty() {
            lines.push_str(&format!("- {name}{role}\n"));
        } else {
            lines.push_str(&format!("- {name}{role}：{bg}\n"));
        }
    }
    format!(
        "【权威人物名册（唯一可用具名人物；不得发明名册外姓名）】\n{lines}\
         写作硬约束：\n\
         - 主角沿用原文已有姓名；原文用第一人称「我」叙述时，不得给主角另起姓名称呼；\n\
         - 不得把路人/次要角色命名（护士、摊主、警员、司机只用身份称呼）；\n\
         - 上一轮编审建议或历史结论中出现名册外的人名，一律视为幻觉，忽略该名字及其设定。"
    )
}

/// 从文本中的引号内容提取像人名但未在上下文中出现的词（用于过滤上一轮
/// 编审建议：它们描述的是自己的幻觉世界，不能当事实注入下一轮）。
fn collect_unquoted_ungrounded_names(
    text: &str,
    context_prose: &str,
    out: &mut std::collections::HashSet<String>,
) {
    for token in quoted_tokens(text) {
        let name = token.trim();
        if !character_like(name) || name.is_empty() {
            continue;
        }
        if !crate::agency::prose_ground::name_in_prose(name, context_prose) {
            out.insert(name.to_string());
        }
    }
}

/// 提取常见中文引号块内容（“…”/「…」/『…』/"…"）。
fn quoted_tokens(text: &str) -> Vec<String> {
    let pairs = [('“', '”'), ('「', '」'), ('『', '』'), ('"', '"')];
    let mut out = Vec::new();
    for (open, close) in pairs {
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i] != open {
                i += 1;
                continue;
            }
            let start = i + 1;
            let mut j = start;
            while j < chars.len() && chars[j] != close {
                j += 1;
            }
            if j > start {
                let token: String = chars[start..j].iter().collect();
                out.push(token.trim_matches(|c: char| c.is_whitespace()).to_string());
            }
            i = j + 1;
        }
    }
    out
}

/// 像人名（先生/哥/叔/老头等称谓，或老X/小X）——只用于幻觉名过滤，
/// 不用于正向生成。
fn character_like(token: &str) -> bool {
    let token = token.trim();
    let len = token.chars().count();
    if len == 0 || len > 6 {
        return false;
    }
    const SUFFIXES: [&str; 18] = [
        "先生", "女士", "哥", "兄弟", "叔", "爷", "老头", "老爹", "大娘", "婶", "太太", "师傅",
        "师父", "道长", "道姑", "姑娘", "丫头", "小子",
    ];
    if SUFFIXES.iter().any(|suffix| token.ends_with(suffix)) {
        return true;
    }
    (token.starts_with('老') || token.starts_with('小')) && len <= 4
}

/// 过滤含幻觉人名的建议行（保留其余可执行条目）。
fn strip_truth_hint_lines(text: &str) -> String {
    text.lines()
        .filter(|line| !line.contains("真实后续节点"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn filter_improvement_lines(text: &str, ungrounded: &std::collections::HashSet<String>) -> String {
    text.lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() && !ungrounded.iter().any(|name| line.contains(name))
        })
        .map(|line| line.trim().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 从逐章编审 comments 中提取「真实后续」节点（只保留不含幻觉人名的句子）。
/// 这是训练闭环里最有价值的对比产物：下一轮 writer/章纲才能对齐作者原书
/// 的后续主线，而不是在自造支线里越走越远。
fn extract_truth_beats(
    reviews: &[ChapterReviewOutcome],
    ungrounded: &std::collections::HashSet<String>,
) -> Vec<String> {
    const MARKERS: [&str; 6] = [
        "真实剧情是",
        "真实章是",
        "基准章为",
        "基准章是",
        "真实后续是",
        "原书第",
    ];
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for review in reviews {
        for sentence in review.comments.split(['。', '；', '\n']) {
            let sentence = sentence.trim();
            if sentence.is_empty() {
                continue;
            }
            let Some(pos) = MARKERS
                .iter()
                .filter_map(|marker| sentence.find(marker))
                .min()
            else {
                continue;
            };
            let marker_len = MARKERS
                .iter()
                .filter(|marker| sentence[pos..].starts_with(*marker))
                .map(|marker| marker.len())
                .max()
                .unwrap_or(0);
            if marker_len == 0 {
                continue;
            }
            let mut beat = sentence[pos + marker_len..].trim().to_string();
            for pivot in [
                "，AI续写",
                "，AI",
                "；AI",
                "。AI",
                "，但AI",
                "但AI",
                "AI却",
                "，上一章结尾",
                "，而AI",
                "，完全无视",
                "完全无视上一章",
            ] {
                if let Some(q) = beat.find(pivot) {
                    beat = beat[..q].trim().to_string();
                }
            }
            let t = beat.trim_matches(|c: char| c == '，' || c == ',' || c.is_whitespace());
            if t.chars().count() < 4 {
                continue;
            }
            if ungrounded.iter().any(|name| t.contains(name)) {
                continue;
            }
            let key: String = t.chars().take(24).collect();
            if seen.insert(key) {
                out.push(format!(
                    "真实后续节点：{}",
                    t.chars().take(120).collect::<String>()
                ));
            }
        }
    }
    out.truncate(6);
    out
}

/// v0.99.3：训练报告的记忆探针（确定性，不调用 LLM）。
/// - 人物留存：brief cast 中的人物在 10 章生成里是否再出现；
/// - 线索推进：brief thread 的关键词是否在生成里出现；
/// - 地点跳切：brief location 在两个生成章之间切换但无过渡动词；
/// - 疑似新专名：生成文本中重复 ≥2 次、上下文/白名单里不存在的 3–4 字中文串。
fn memory_probes_line(
    brief: &crate::agency::story_brief::StoryBrief,
    generated: &[(i32, String)],
    context_prose: &str,
) -> String {
    let all: Vec<&str> = generated
        .iter()
        .map(|(_, text)| text.as_str())
        .filter(|text| !text.trim().is_empty())
        .collect();
    let mut cast_missing: Vec<String> = Vec::new();
    for cast in brief.cast_states.iter().take(12) {
        if !cast.name.trim().is_empty() && !all.iter().any(|text| text.contains(cast.name.trim())) {
            cast_missing.push(cast.name.clone());
        }
    }
    let mut threads_unadvanced = Vec::new();
    for thread in brief.threads.iter().take(8) {
        let key = thread
            .next_expected
            .trim()
            .chars()
            .take(4)
            .collect::<String>();
        let desc_key = thread
            .description
            .trim()
            .chars()
            .filter(|c| !c.is_whitespace() && !")_）】，。；：!?！？".contains(*c))
            .take(4)
            .collect::<String>();
        let advanced = all.iter().any(|text| {
            (!key.is_empty() && text.contains(&key))
                || (!desc_key.is_empty() && text.contains(&desc_key))
        });
        if !advanced {
            threads_unadvanced.push(thread.id.clone());
        }
    }
    let locations: Vec<String> = brief
        .chapters
        .iter()
        .filter(|chapter| !chapter.location.trim().is_empty())
        .map(|chapter| chapter.location.trim().to_string())
        .collect();
    let mut location_shifts = 0usize;
    let mut prev_loc: Option<String> = None;
    if locations.len() > 1 {
        for (_, text) in generated {
            let current = locations
                .iter()
                .find(|loc| text.contains(loc.as_str()))
                .cloned();
            if let (Some(prev), Some(cur)) = (prev_loc.as_ref(), current.as_ref()) {
                if prev != cur {
                    let transition = ["去", "到", "进", "出", "回", "离开", "路上", "来到"]
                        .iter()
                        .any(|word| text.contains(word));
                    if !transition {
                        location_shifts += 1;
                    }
                }
            }
            if current.is_some() {
                prev_loc = current;
            }
        }
    }
    let suspected = suspect_new_terms(&all.join("\n"), context_prose, &brief.canon_terms);
    let mut parts = Vec::new();
    if !cast_missing.is_empty() {
        parts.push(format!(
            "人物留存缺失 {} 人（{}）",
            cast_missing.len(),
            cast_missing
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if !threads_unadvanced.is_empty() {
        parts.push(format!(
            "线索未推进 {} 条（{}）",
            threads_unadvanced.len(),
            threads_unadvanced
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if location_shifts > 0 {
        parts.push(format!("疑似地点跳切 {location_shifts} 次"));
    }
    if !suspected.is_empty() {
        parts.push(format!(
            "疑似新专名：{}",
            suspected
                .iter()
                .take(6)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if parts.is_empty() {
        "人物/线索/地点/专名探针未发现异常".to_string()
    } else {
        parts.join("｜")
    }
}

fn suspect_new_terms(
    generated: &str,
    context_prose: &str,
    canon_terms: &[crate::agency::story_brief::BriefTerm],
) -> Vec<String> {
    const STOPWORDS: [&str; 36] = [
        "时候", "已经", "自己", "什么", "一个", "这个", "那个", "里面", "外面", "的话", "就是",
        "可是", "然后", "因为", "所以", "不是", "没有", "他们", "我们", "起来", "出来", "过去",
        "现在", "心里", "脸上", "身上", "手里", "眼睛", "声音", "地方", "东西", "事情", "感觉",
        "知道", "看着", "听见",
    ];
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let chars: Vec<char> = generated.chars().collect();
    for n in 3..=4 {
        if chars.len() < n {
            continue;
        }
        for i in 0..=chars.len() - n {
            let term: String = chars[i..i + n].iter().collect();
            if term
                .chars()
                .all(|c| ('\u{4e00}'..='\u{9fff}').contains(&c) || c == '·')
                && !STOPWORDS.contains(&term.as_str())
            {
                *counts.entry(term).or_insert(0) += 1;
            }
        }
    }
    let mut out: Vec<(String, usize)> = counts
        .into_iter()
        .filter(|(term, count)| {
            *count >= 2
                && !context_prose.contains(term)
                && !canon_terms.iter().any(|t| t.term == *term)
        })
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out.into_iter().map(|(term, _)| term).take(12).collect()
}

/// 去重、限长（<=1600 字符），保证可被下一轮指令完整注入。
fn derive_improvements(
    reviews: &[ChapterReviewOutcome],
    synthesis: Option<&SynthesisReport>,
    dim_avgs: &[(&str, Option<f64>)],
    ungrounded: &std::collections::HashSet<String>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    // 真实后续节点先放前面（下一轮主线对齐），且已排除幻觉人名。
    lines.extend(extract_truth_beats(reviews, ungrounded));
    if let Some(synthesis) = synthesis {
        lines.extend(
            synthesis
                .improvement
                .iter()
                .take(6)
                .filter(|text| !ungrounded.iter().any(|name| text.contains(name)))
                .cloned(),
        );
    }
    for (dim, avg) in dim_avgs {
        if let Some(value) = avg.filter(|v| *v < FEEDBACK_THRESHOLD) {
            lines.push(format!(
                "{}（{value:.1}/10）：{}",
                dimension_label(dim),
                dimension_correction(dim)
            ));
        }
    }
    for review in reviews {
        let text = review.improvement.trim();
        if !text.is_empty() && !ungrounded.iter().any(|name| text.contains(name)) {
            lines.push(text.to_string());
        }
    }
    let mut seen = std::collections::HashSet::new();
    let mut unique: Vec<String> = Vec::new();
    for line in lines {
        let shortened: String = line.chars().take(180).collect();
        let key: String = shortened.chars().take(32).collect();
        if seen.insert(key) {
            unique.push(shortened);
        }
    }
    if unique.is_empty() {
        return "- 本轮未产出具体改进；下一轮按逐章短板继续修正（承接/文笔优先）".to_string();
    }
    let mut total = 0usize;
    let mut out = String::new();
    for line in unique.iter().take(12) {
        let chars = line.chars().count();
        if total + chars > 1600 {
            break;
        }
        total += chars;
        out.push_str(&format!("- {line}\n"));
    }
    if out.is_empty() {
        "- 本轮改进项过长，已截断；请按逐章短板继续修正".to_string()
    } else {
        out
    }
}

/// 源章节缓存读取：命中返回已缓存的正文（写入时已按 `MAX_CHAPTER_CHARS`
/// 截断）。缓存按 `(server_id, book_ref, chapter_index)` 唯一，不同书/不同源
/// 互不复用；`fetched_at` 仅供诊断，不参与命中判定。
fn read_cached_chapter(
    pool: &DbPool,
    server_id: &str,
    book_ref: &str,
    chapter_index: usize,
) -> Option<String> {
    let conn = pool.get().ok()?;
    let mut stmt = conn
        .prepare(
            "SELECT content FROM continuation_training_chapter_cache \
             WHERE server_id = ?1 AND book_ref = ?2 AND chapter_index = ?3",
        )
        .ok()?;
    stmt.query_row(
        rusqlite::params![server_id, book_ref, chapter_index as i64],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

/// 源章节缓存写入/更新：同一本书同章再次取到正文时刷新内容与时间。
fn write_cached_chapter(
    pool: &DbPool,
    server_id: &str,
    book_ref: &str,
    chapter_index: usize,
    content: &str,
) -> Result<(), String> {
    let conn = pool.get().map_err(|e| format!("获取缓存连接失败：{e}"))?;
    conn.execute(
        "INSERT INTO continuation_training_chapter_cache \
         (server_id, book_ref, chapter_index, content, char_count) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(server_id, book_ref, chapter_index) DO UPDATE SET \
         content = excluded.content, char_count = excluded.char_count, \
         fetched_at = datetime('now','localtime')",
        rusqlite::params![
            server_id,
            book_ref,
            chapter_index as i64,
            content,
            content.chars().count() as i64,
        ],
    )
    .map_err(|e| format!("写入缓存失败：{e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_line_accounts_for_every_chapter() {
        let line = render_gate_line(3, 0, 2, 5);
        assert!(line.contains("3 章通过 / 0 章修订 / 2 章质检降级"));
        assert!(
            line.contains("5 章编审判修订但修订未完成"),
            "未完成修订的章必须显式计数: {line}"
        );
        let full = render_gate_line(10, 0, 0, 0);
        assert!(
            !full.contains("编审判修订但修订未完成"),
            "全通过时不加噪音: {full}"
        );
    }

    #[test]
    fn duplicate_chapter_detection_catches_full_copy_and_copied_ending() {
        let prev = "上一章结尾，他推开门，走廊尽头那辆换药小车没了。".repeat(20);
        // 整章逐字复制
        assert!(duplicates_previous_chapter(&prev, &prev));
        // 开头原样复述上一章结尾（≥120 字）
        let copied_head = format!("{prev}本章继续……");
        assert!(duplicates_previous_chapter(&copied_head, &prev));
        // 正常承接：新动作起笔
        let fresh = format!(
            "他盯着走廊尽头，先没动，反手把门掩上。{}",
            "新的对峙从这里开始。".repeat(30)
        );
        assert!(!duplicates_previous_chapter(&fresh, &prev));
        // 空稿不参与重复判定
        assert!(!duplicates_previous_chapter("", &prev));
    }

    #[test]
    fn style_anchor_takes_tail_of_last_two_context_chapters() {
        let chapters: Vec<Chapter> = (1..=3)
            .map(|i| (i, format!("第{i}章开头铺垫。{}", "市井细节。".repeat(200))))
            .collect();
        let anchor = build_style_anchor(&chapters[..chapters.len() - 1]);
        // 只取上下文最后两章（末位是基准章，禁止泄漏）
        assert!(!anchor.contains("第1章开头铺垫。"), "不应包含第 1 章");
        // 每章取的是末尾（含最后一句）
        assert!(anchor.contains("市井细节。"), "应包含章节末尾原文");
        assert!(anchor.contains("……\n"), "两章之间应有省略分隔");
        // 长度受 600 字/章约束（两章 + 分隔 ≈ 1200 余）
        let chars = anchor.chars().count();
        assert!(chars <= 1400, "锚点应受长度约束，实际 {chars}");
    }

    #[test]
    fn style_anchor_excludes_truth_chapter() {
        // v0.93.16：末位是真实后续基准章（答案），不得进入续写指令。
        let chapters: Vec<Chapter> = vec![
            (106, "第106章正文，末尾是桐人镇魇现场。".into()),
            (107, "第107章真实基准，答案标记不得泄漏。".into()),
        ];
        let anchor = build_style_anchor(&chapters[..chapters.len() - 1]);
        assert!(anchor.contains("桐人镇魇现场"));
        assert!(
            !anchor.contains("答案标记不得泄漏"),
            "锚点泄漏基准章: {anchor}"
        );
        // v0.98.0：多章基准（10 章）全部不得进入锚点。
        let multi: Vec<Chapter> = (1..=12).map(|i| (i, format!("第{i}章正文。"))).collect();
        let anchor = build_style_anchor(&multi[..multi.len() - 10]);
        assert!(!anchor.contains("第11章正文") && !anchor.contains("第12章正文"));
    }

    #[test]
    fn style_anchor_handles_short_context_chapters() {
        let chapters: Vec<Chapter> = vec![
            (1, "短章一".into()),
            (2, "短章二".into()),
            (3, "基准章".into()),
        ];
        let anchor = build_style_anchor(&chapters[..chapters.len() - 1]);
        assert!(anchor.contains("短章一") && anchor.contains("短章二"));
        assert!(!anchor.contains("基准章"), "基准章不得进入锚点: {anchor}");
    }

    #[test]
    fn continuity_anchor_uses_last_context_chapter_not_truth() {
        // v0.93.16：承接锚点只取 chapters 倒数第二项（第 106 章），
        // 末位第 107 章是评分基准，绝不能出现在续写指令里。
        let chapters: Vec<Chapter> = vec![
            (106, "桐人晃了晃栽在地。".into()),
            (107, "扑通，扑通，扑通。基准章答案。".into()),
        ];
        let anchor = build_continuity_anchor(&chapters[..chapters.len() - 1], 900);
        assert!(anchor.contains("第 106 章"));
        assert!(anchor.contains("桐人晃了晃栽在地"));
        assert!(
            !anchor.contains("基准章答案"),
            "承接锚点泄漏基准章: {anchor}"
        );
    }

    #[test]
    fn generated_prose_validation_rejects_empty_html_and_refusal() {
        // 空稿：tiptap 空文档 <p></p> 经导出清洗后为空
        let empty = crate::export::content::content_to_plain_text("<p></p>");
        assert!(validate_generated_prose(&empty).is_err());
        // 拒绝话术：真机 run 的 409 字元评论（这里重复到 ≥300 字，
        // 确保命中拒绝判定而非先被「过短」挡住）
        let refusal = "我无法按当前输入完成这一章。原因是：提示中「创作资产」区域为空。\
                       请提供世界观、人物设定与大纲后再试。"
            .repeat(6);
        let err = validate_generated_prose(&refusal).unwrap_err();
        assert!(err.contains("拒绝"), "err={err}");
        // 正常正文 ≥300 字
        let ok = "雨敲窗。他把铜钱放回掌心。\n".repeat(40);
        assert!(
            validate_generated_prose(&ok).is_ok(),
            "chars={}",
            ok.chars().count()
        );
        // 过短正文
        assert!(validate_generated_prose("短正文。").is_err());
    }

    #[test]
    fn render_args_coerces_index_and_replaces_book_url() {
        let mut template = serde_json::Map::new();
        template.insert("book_url".to_string(), serde_json::json!("$book"));
        template.insert("index".to_string(), serde_json::json!("$index"));
        template.insert("fetch_online".to_string(), serde_json::Value::Bool(true));

        let args =
            ContinuationTrainingExecutor::render_args(&template, "https://example.test/book/1", 7);
        assert_eq!(args["book_url"], "https://example.test/book/1");
        assert_eq!(args["index"], 7);
        assert_eq!(args["fetch_online"], true);
    }

    #[test]
    fn find_book_url_recurses_candidates() {
        let value = serde_json::json!({
            "candidates": [
                { "name": "阴脉先生", "book_url": "https://example.test/book/1" }
            ]
        });
        assert_eq!(
            ContinuationTrainingExecutor::find_book_url(&value),
            Some("https://example.test/book/1".to_string())
        );
    }

    #[test]
    fn auto_selects_chapter_tool_and_schema_args() {
        let tools = vec![
            crate::mcp::McpTool {
                name: "list_books".into(),
                description: "书架".into(),
                parameters: serde_json::json!({ "type": "object" }),
                annotations: None,
            },
            crate::mcp::McpTool {
                name: "get_chapter_content".into(),
                description: "取章节正文".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "book_url": { "type": "string" },
                        "index": { "type": "integer" },
                        "fetch_online": { "type": "boolean" }
                    }
                }),
                annotations: None,
            },
        ];
        assert_eq!(
            ContinuationTrainingExecutor::pick_chapter_tool_from_tools(&tools),
            Some("get_chapter_content".to_string())
        );

        let template = ContinuationTrainingExecutor::default_args_from_tool(&tools[1]);
        assert_eq!(template["book_url"], "$book");
        assert_eq!(template["index"], "$index");
        assert_eq!(template["fetch_online"], true);
    }

    #[test]
    fn judge_parses_five_numeric_scores_and_computes_overall() {
        let raw = r#"{"scores":{"style":6,"plot":7,"voice":5,"pacing":8,"hooks":9},"comments":"依据","improvement":"改进"}"#;
        let judge = parse_judge_report(raw).expect("五维齐全应解析");
        assert_eq!(judge.scores.len(), 5);
        assert_eq!(judge.scores["style"], serde_json::json!(6));
        // overall 缺失 → 五维均值 (6+7+5+8+9)/5/10 = 0.70
        assert!((judge.overall - 0.70).abs() < 1e-9);
        assert_eq!(judge.comments, "依据");
        assert_eq!(judge.improvement, "改进");
    }

    #[test]
    fn judge_rejects_null_scores_instead_of_fake_zero_report() {
        // v0.93.15 真机事故形态：判官返回 {"scores":null,...}，旧代码落库
        // 0.00。
        let raw = r#"{"scores":null,"overall":0.0,"comments":"","improvement":""}"#;
        assert!(parse_judge_report(raw).is_none());
    }

    #[test]
    fn judge_rejects_missing_dimension() {
        let raw = r#"{"scores":{"style":6,"plot":7,"voice":5,"pacing":8},"overall":0.7}"#;
        assert!(parse_judge_report(raw).is_none());
    }

    #[test]
    fn judge_accepts_string_and_ratio_scores() {
        let raw = r#"{"scores":{"style":"6","plot":"7/10","voice":"5分","pacing":8,"hooks":9},"overall":"0.9","comments":"c","improvement":"i"}"#;
        let judge = parse_judge_report(raw).expect("数字字符串应宽容解析");
        assert_eq!(judge.scores["plot"], serde_json::json!(7));
        assert!((judge.overall - 0.9).abs() < 1e-9);
    }

    #[test]
    fn judge_overall_falls_back_to_mean_and_normalizes_ten_point_value() {
        let raw =
            r#"{"scores":{"style":4,"plot":4,"voice":4,"pacing":4,"hooks":4},"improvement":"i"}"#;
        let judge = parse_judge_report(raw).unwrap();
        assert!((judge.overall - 0.4).abs() < 1e-9);

        let raw_ten = r#"{"scores":{"style":4,"plot":4,"voice":4,"pacing":4,"hooks":4},"overall":6.5,"improvement":"i"}"#;
        let judge = parse_judge_report(raw_ten).unwrap();
        assert!((judge.overall - 0.65).abs() < 1e-9);
    }

    #[test]
    fn review_prompt_names_sandbox_and_source_chapter() {
        let prompt = ContinuationTrainingExecutor::review_prompt(
            "阴脉先生",
            "真",
            "仿",
            107,
            11,
            106,
            "桐人晃了晃栽在地。",
            false,
        );
        assert!(prompt.contains("沙盒第 11 章"));
        assert!(prompt.contains("对应真实第 107 章"));
        assert!(prompt.contains("上一章结尾"));
        assert!(prompt.contains("桐人晃了晃栽在地"));
        assert!(prompt.contains("紧接"));
        assert!(prompt.contains("continuity"));
        assert!(prompt.contains("编审"));
    }

    #[test]
    fn judge_prior_baseline_uses_last_context_chapter_tail_only() {
        let chapters: Vec<Chapter> = vec![
            (106, "第106章结尾桐人镇魇。".into()),
            (107, "第107章真实基准答案。".into()),
        ];
        let (number, tail) = judge_prior_baseline(&chapters[..chapters.len() - 1]);
        assert_eq!(number, 106);
        assert!(tail.contains("桐人镇魇"));
        assert!(!tail.contains("真实基准答案"), "判官衔接基准不得含答案");
    }

    #[test]
    fn training_stages_are_unique_and_monotonic() {
        let stages = [
            TrainingStage::Fetch,
            TrainingStage::Sandbox,
            TrainingStage::Style,
            TrainingStage::Assets,
            TrainingStage::Continue,
            TrainingStage::Compare,
            TrainingStage::Review,
            TrainingStage::Report,
            TrainingStage::Done,
        ];
        let progresses: Vec<i32> = stages.iter().map(|s| s.progress()).collect();
        assert_eq!(progresses, vec![5, 20, 30, 35, 40, 72, 85, 92, 100]);
        assert!(progresses.windows(2).all(|w| w[0] < w[1]));
        let steps: Vec<&str> = stages.iter().map(|s| s.step()).collect();
        assert_eq!(
            steps,
            vec![
                "fetch", "sandbox", "style", "assets", "continue", "compare", "review", "report",
                "done"
            ]
        );
    }

    fn review_row(
        generated_chapter: i32,
        truth_chapter: usize,
        score: f64,
        continuity: Option<f64>,
    ) -> ChapterReviewOutcome {
        let mut scores = serde_json::Map::new();
        for dim in JUDGE_DIMENSIONS {
            scores.insert(dim.to_string(), serde_json::json!(score));
        }
        ChapterReviewOutcome {
            generated_chapter,
            truth_chapter,
            verdict: "pass".into(),
            overall: Some(score / 10.0),
            scores,
            continuity,
            reference_gap: None,
            comments: "承接紧接上一章现场；人物口吻一致".into(),
            improvement: "加强章末钩子".into(),
            prose_available: true,
        }
    }

    #[test]
    fn payload_defaults_to_ten_continuation_chapters() {
        let payload: TrainingPayload = serde_json::from_str(
            r#"{"book_title":"阴脉先生","source":{"kind":"file","path":"a.txt"}}"#,
        )
        .unwrap();
        assert_eq!(payload.chapter_count, DEFAULT_CONTEXT_CHAPTERS);
        assert_eq!(payload.continuation_chapters, DEFAULT_CONTINUATION_CHAPTERS);
        assert_eq!(payload.continuation_chapters, 10);

        let custom: TrainingPayload = serde_json::from_str(
            r#"{"book_title":"阴脉先生","chapter_count":12,"continuation_chapters":3,"source":{"kind":"file","path":"a.txt"}}"#,
        )
        .unwrap();
        assert_eq!(custom.chapter_count, 12);
        assert_eq!(custom.continuation_chapters, 3);
    }

    #[test]
    fn chapter_review_parses_continuity_and_verdict() {
        let raw = r#"{"verdict":"revise","scores":{"style":6,"plot":7,"voice":5,"pacing":8,"hooks":9},"continuity":4,"overall":0.58,"comments":"承接断裂","improvement":"先承接现场","blocking_issues":["跳切新委托"]}"#;
        let judge = parse_judge_report(raw).expect("五维齐全应解析");
        assert_eq!(judge.verdict, "revise");
        assert_eq!(judge.continuity, Some(4.0));
        assert!((judge.overall - 0.58).abs() < 1e-9);
        assert_eq!(judge.improvement, "先承接现场");
    }

    #[test]
    fn synthesis_parses_structured_report() {
        let raw = r#"{"overall":0.6,"dimensions":{"style":6,"plot":6,"voice":6,"pacing":6,"hooks":6,"continuity":5},"cross_chapter":"章末钩子逐章走弱；人物口吻稳定","strengths":["承接稳定"],"weaknesses":["钩子弱"],"improvement":["章末留未完结钩子","控制信息密度"],"comments":"总体可用"}"#;
        let synthesis = parse_synthesis(raw).expect("结构化总评应解析");
        assert_eq!(synthesis.cross_chapter.len(), 2);
        assert_eq!(synthesis.strengths, vec!["承接稳定"]);
        assert_eq!(synthesis.improvement.len(), 2);
        assert!(synthesis.dimensions.is_some());
        assert_eq!(synthesis.comments, "总体可用");
    }

    #[test]
    fn feedback_low_dimensions_drive_corrections() {
        let mut feedback = TrainingFeedback::default();
        feedback.dimension_avgs.insert("style".into(), 4.0);
        feedback.dimension_avgs.insert("continuity".into(), 3.0);
        feedback.dimension_avgs.insert("plot".into(), 7.0);
        assert!(feedback.style_low());
        assert!(feedback.continuity_low());
        let corrections = feedback.correction_block();
        assert_eq!(corrections.len(), 2, "只修正低分维度");
        assert!(corrections
            .iter()
            .any(|line| line.contains("承接") && line.contains("3.0")));
        assert!(corrections
            .iter()
            .any(|line| line.contains("文笔风格") && line.contains("4.0")));
        assert!(!corrections.iter().any(|line| line.contains("情节推进")));
    }

    #[test]
    fn build_report_multi_chapter_is_deterministic_and_complete() {
        let context = vec![(1, "前文一".to_string()), (2, "前文二".to_string())];
        let truths = vec![(3, "真三".to_string()), (4, "真四".to_string())];
        let reviews = vec![
            review_row(3, 3, 5.0, Some(4.0)),
            review_row(4, 4, 6.5, Some(7.0)),
        ];
        let (report, scores, improvement) = ContinuationTrainingExecutor::build_report(
            "阴脉先生",
            2,
            2,
            &context,
            &truths,
            &reviews,
            None,
            "世界观✓",
            2,
            0,
            0,
            0,
            true,
            &TrainingFeedback::default(),
            &std::collections::HashSet::new(),
        );
        assert!(report.contains("续写质量分析报告"));
        assert!(report.contains("连续续写 2 章（沙盒第 3–4 章）对比原书第 3–4 章"));
        assert!(report.contains("| 3 | 3 | pass | 0.50 |"));
        assert!(report.contains("| 4 | 4 | pass | 0.65 |"));
        assert!(report.contains("承接"));
        assert!(report.contains("下一轮改进"));
        assert!(!improvement.trim().is_empty());
        assert!(report.contains("编审判裁：2 章 pass"));
        assert!(report.contains("生成质量门：2 章通过 / 0 章修订 / 0 章质检降级"));
        // 逐章表头只能有一个「承接（沙盒）」列 + 独立的「原书差距」列。
        assert_eq!(report.matches("| 承接（沙盒） |").count(), 1, "{report}");
        assert_eq!(report.matches("| 原书差距 |").count(), 1, "{report}");
        assert!(report.contains("|---|---|---|---|---|---|---|---|"));
        let parsed: serde_json::Value = serde_json::from_str(&scores).unwrap();
        assert_eq!(parsed["chapters"], 2);
        assert_eq!(parsed["reviewed"], 2);
        assert!(parsed["overall"].as_f64().unwrap() > 0.5);
        assert_eq!(parsed["continuity"], 5.5);
    }

    #[test]
    fn derive_improvements_dedupes_and_caps_length() {
        let reviews = vec![review_row(3, 3, 5.0, Some(4.0))];
        let dims: Vec<(&str, Option<f64>)> = vec![
            ("style", Some(4.0)),
            ("plot", Some(7.0)),
            ("continuity", Some(4.0)),
        ];
        let text = derive_improvements(&reviews, None, &dims, &std::collections::HashSet::new());
        assert!(text.contains("文笔风格"));
        assert!(text.contains("承接"));
        assert!(!text.contains("情节推进"), "高分维度不落改进");
        assert!(text.chars().count() <= 1600);
    }

    #[test]
    fn judge_repairs_truncated_output_instead_of_unavailable() {
        // 真机原文：JSON 在 comments 中途截断（3/10 章曾判不可用）。
        let raw = r#"{"verdict":"revise","scores":{"style":3,"plot":4,"voice":3,"pacing":4,"hooks":3},"continuity":3,"overall":0.33,"comments"#;
        let judge = parse_judge_report(raw).expect("截断 JSON 应补全后解析");
        assert_eq!(judge.scores.len(), 5);
        assert_eq!(judge.continuity, Some(3.0));
        assert!((judge.overall - 0.33).abs() < 1e-9);
        assert_eq!(judge.verdict, "revise");
    }

    #[test]
    fn roster_block_forbids_invented_names_and_marks_protagonist() {
        let roster = vec![
            ("周成".to_string(), "阴脉先生，三脉堂术士".to_string()),
            ("张宝山".to_string(), "刑警队长".to_string()),
        ];
        let block = build_roster_block(&roster);
        assert!(block.contains("周成（主角）"), "{block}");
        assert!(block.contains("不得发明名册外姓名"), "{block}");
        assert!(block.contains("主角沿用原文已有姓名"), "{block}");
        assert!(block.contains("名册外的人名，一律视为幻觉"), "{block}");
    }

    #[test]
    fn improvement_filter_drops_hallucinated_names_and_keeps_truth_beats() {
        let mut ungrounded = std::collections::HashSet::new();
        ungrounded.insert("杨先生".to_string());
        ungrounded.insert("何小满".to_string());

        let mut review = review_row(11, 144, 4.0, Some(2.0));
        review.comments = "真实剧情是主角离开医院去夜总会找邵卫江，AI却强行留在医院审问。".into();
        review.improvement = "主角固定为杨先生，同伴何小满，严禁出现老陈。".into();
        let reviews = vec![review];
        let dims: Vec<(&str, Option<f64>)> = vec![("continuity", Some(1.2))];
        let text = derive_improvements(&reviews, None, &dims, &ungrounded);

        assert!(
            text.contains("真实后续节点：主角离开医院去夜总会找邵卫江"),
            "{text}"
        );
        assert!(!text.contains("杨先生"), "{text}");
        assert!(!text.contains("何小满"), "{text}");
        assert!(text.contains("承接"), "{text}");

        let filtered = filter_improvement_lines(
            "1. 主角为杨先生。\n2. 严格承接上一章结尾。\n3. 恢复玄学世界观。",
            &ungrounded,
        );
        assert!(!filtered.contains("杨先生"), "{filtered}");
        assert!(filtered.contains("严格承接"), "{filtered}");
        assert!(filtered.contains("恢复玄学世界观"), "{filtered}");
    }

    #[test]
    fn truth_beat_cuts_generated_clause_and_keeps_chapter_title() {
        let mut ungrounded = std::collections::HashSet::new();
        ungrounded.insert("何小满".to_string());
        let mut review = review_row(14, 147, 2.0, Some(1.0));
        review.comments =
            "基准章为第147章《登山》，上一章结尾主角与何小满在铺子发现铜锁。AI续写却换场。".into();
        let reviews = vec![review];
        let beats = extract_truth_beats(&reviews, &ungrounded);
        assert!(
            beats.iter().any(|b| b.contains("第147章《登山》")),
            "{beats:?}"
        );
        assert!(!beats.iter().any(|b| b.contains("何小满")), "{beats:?}");
    }

    #[test]
    fn degraded_gate_comments_are_not_counted_as_real_pass() {
        assert!(is_degraded_gate_comment(
            "编辑审计失败，已降级放行保产出：编辑审计 Agent 被熔断（达到最大轮数）"
        ));
        assert!(is_degraded_gate_comment("审查未完成，已熔断"));
        assert!(!is_degraded_gate_comment("后台质检 pass（round 1）"));
    }

    #[test]
    fn file_slice_bounds_include_requested_start_chapter() {
        // v0.98.1：固定起点训练曾把 `[start..end]` 当切片起点，跳过第 1 章
        // 并整体错一位；1-based 章号必须先转 0-based 索引。
        assert_eq!(
            ContinuationTrainingExecutor::file_slice_bounds(1, 20, 100),
            (0, 20)
        );
        assert_eq!(
            ContinuationTrainingExecutor::file_slice_bounds(10, 20, 100),
            (9, 29)
        );
        assert_eq!(
            ContinuationTrainingExecutor::file_slice_bounds(95, 20, 100),
            (94, 100)
        );
        assert_eq!(
            ContinuationTrainingExecutor::file_slice_bounds(101, 20, 100),
            (100, 100)
        );
    }

    #[test]
    fn chapter_cache_roundtrip_refreshes_and_isolates_books() {
        let pool = crate::db::create_test_pool().expect("test pool");
        let book = "https://m.po18.xyz/novel/115880.html";
        assert!(read_cached_chapter(&pool, "legado", book, 134).is_none());

        write_cached_chapter(&pool, "legado", book, 134, "第134章正文").expect("首次写入");
        assert_eq!(
            read_cached_chapter(&pool, "legado", book, 134),
            Some("第134章正文".to_string())
        );

        // 同键再次写入应刷新内容与字符数。
        write_cached_chapter(&pool, "legado", book, 134, "第134章修订正文").expect("刷新");
        assert_eq!(
            read_cached_chapter(&pool, "legado", book, 134),
            Some("第134章修订正文".to_string())
        );
        let conn = pool.get().unwrap();
        let (content, chars): (String, i64) = conn
            .query_row(
                "SELECT content, char_count FROM continuation_training_chapter_cache \
                 WHERE server_id='legado' AND book_ref=?1 AND chapter_index=134",
                rusqlite::params![book],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(chars as usize, content.chars().count());

        // 不同章号、不同书/源不得互相命中。
        assert!(read_cached_chapter(&pool, "legado", book, 135).is_none());
        assert!(read_cached_chapter(&pool, "legado", "https://other/book", 134).is_none());
        assert!(read_cached_chapter(&pool, "another", book, 134).is_none());
    }

    #[test]
    fn judge_parses_reference_gap_separately_from_continuity() {
        let raw = r#"{"scores":{"style":6,"plot":7,"voice":5,"pacing":8,"hooks":9},"continuity":9,"reference_gap":2,"comments":"依据","improvement":"改进"}"#;
        let judge = parse_judge_report(raw).expect("应解析");
        assert_eq!(judge.continuity, Some(9.0));
        assert_eq!(judge.reference_gap, Some(2.0));
    }

    #[test]
    fn blind_feedback_strips_truth_hints() {
        let text = "真实后续节点：周成主动破局、立威\n禁止另开长期悬疑线\n每章必须主动出手";
        let kept = strip_truth_hint_lines(text);
        assert!(!kept.contains("真实后续节点"));
        assert!(kept.contains("禁止另开长期悬疑线"));
        assert!(kept.contains("每章必须主动出手"));
    }

    #[test]
    fn memory_probes_flag_missing_cast_thread_and_new_terms() {
        use crate::agency::story_brief::{BriefCastState, BriefTerm, BriefThread, StoryBrief};
        let brief = StoryBrief {
            cast_states: vec![BriefCastState {
                name: "周成".into(),
                ..Default::default()
            }],
            threads: vec![BriefThread {
                id: "t-fomosa".into(),
                description: "福摩萨夜总会道场".into(),
                next_expected: "福摩萨夜总会".into(),
                status: "open".into(),
            }],
            canon_terms: vec![BriefTerm {
                term: "三理教".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let generated = vec![
            (
                11,
                "张宝山在门口等着。引魂蛆钻进骨缝，引魂蛆又动了一下。骨刺也冒了出来。".to_string(),
            ),
            (12, "院里的霜落了一地。".to_string()),
        ];
        let context = "周成在饭局里见到赵开来；三理教已经退出金城。";
        let line = memory_probes_line(&brief, &generated, context);
        assert!(line.contains("人物留存缺失"), "{line}");
        assert!(line.contains("线索未推进"), "{line}");
        assert!(line.contains("疑似新专名"), "{line}");
        assert!(line.contains("引魂蛆") || line.contains("骨刺"), "{line}");
    }

    #[test]
    fn memory_probes_clear_when_all_present() {
        use crate::agency::story_brief::{BriefCastState, BriefThread, StoryBrief};
        let brief = StoryBrief {
            cast_states: vec![BriefCastState {
                name: "周成".into(),
                ..Default::default()
            }],
            threads: vec![BriefThread {
                id: "t-fomosa".into(),
                description: "福摩萨夜总会道场".into(),
                next_expected: "福摩萨".into(),
                status: "open".into(),
            }],
            ..Default::default()
        };
        let generated = vec![(11, "周成去了福摩萨夜总会，门口有人等着。".to_string())];
        let line = memory_probes_line(&brief, &generated, "周成与福摩萨相关。");
        assert!(!line.contains("人物留存缺失"), "{line}");
        assert!(!line.contains("线索未推进"), "{line}");
    }
}
