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
use serde::Deserialize;

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

#[derive(Debug, Deserialize)]
pub struct TrainingPayload {
    pub book_title: String,
    #[serde(default = "default_chapter_count")]
    pub chapter_count: usize,
    /// 随机取章时用作上界（不知道总章数时给个估计值即可）
    #[serde(default)]
    pub est_total_chapters: usize,
    /// random | fixed（start_chapter 起连取）；默认 fixed 从头取
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub start_chapter: Option<usize>,
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
    10
}

/// v0.93.16: 续写训练的上下文切片。`chapters` 末位是「真实后续基准章」，
/// 只用于评分，绝不能进入续写指令——旧版 `build_style_anchor` 取末两章，
/// 把基准章结尾泄漏给模型（训练失真，且模型可直接照抄答案）。
fn context_chapters(chapters: &[Chapter]) -> &[Chapter] {
    &chapters[..chapters.len().saturating_sub(1)]
}

/// v0.93.5: 风格 DNA 失败时的降级文风锚点——取最后 2 个**上下文**章的
/// 末尾各 ~600 字（合计 ≤1200），让模型直接模仿原文的遣词、句式、对话
/// 风格与细节密度，而不是只收到一句抽象的「文风一致」。
fn build_style_anchor(chapters: &[Chapter]) -> String {
    const PER_CHAPTER_CHARS: usize = 600;
    let mut parts: Vec<String> = Vec::new();
    for (_, content) in context_chapters(chapters).iter().rev().take(2).rev() {
        let start = content
            .char_indices()
            .nth_back(PER_CHAPTER_CHARS.saturating_sub(1))
            .map(|(i, _)| i)
            .unwrap_or(0);
        parts.push(content[start..].trim().to_string());
    }
    parts.join("\n……\n")
}

/// v0.93.16: 训练指令的硬承接锚点——最后一个**上下文**章（即 `chapters` 的
/// 倒数第二项）结尾原文。真机事故：沙盒没有世界观/角色卡/大纲资产，
/// writer 在「创作资产区域为空」时拒绝写作或另起现代都市故事（陈默/铜钱），
/// 与《阴脉先生》第 106 章的「桐人镇魇」现场完全脱节。把上一章末段直接
/// 放进指令，模型即使没有资产可读也有可承接的具体文本。
/// v0.94.0：判官/报告用的上一章结尾窗口（只用于判断续写是否承接，绝不能
/// 作为“真实基准章”泄漏给 writer——writer 侧走 build_continuity_anchor）。
const JUDGE_PRIOR_TAIL_CHARS: usize = 800;

fn judge_prior_baseline(chapters: &[Chapter]) -> (usize, String) {
    let context = context_chapters(chapters);
    let Some((number, content)) = context.last() else {
        return (0, String::new());
    };
    let text = crate::export::content::content_to_plain_text(content);
    let total = text.chars().count();
    let start = text
        .char_indices()
        .nth_back(JUDGE_PRIOR_TAIL_CHARS.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    (*number, text[start..].trim().to_string())
}

fn build_continuity_anchor(chapters: &[Chapter]) -> String {
    const TAIL_CHARS: usize = 900;
    let context = context_chapters(chapters);
    let Some((number, content)) = context.last() else {
        return String::new();
    };
    let start = content
        .char_indices()
        .nth_back(TAIL_CHARS.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let tail = content[start..].trim();
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
        let n_chapters = payload.chapter_count.clamp(2, 20);

        // ── 1. 取源章节 ──────────────────────────────────────────
        ctx.update_progress("fetch", 5, "正在获取原书章节…");
        let (chapters, source_kind) = self.fetch_chapters(&payload, &ctx, n_chapters).await?;
        if chapters.len() < 2 {
            let msg = format!(
                "取到的章节数不足（{} 章，至少 2 章：1 章上下文 + 1 章基准）",
                chapters.len()
            );
            ctx.log("error", &msg);
            return Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some(msg),
            });
        }
        ctx.log(
            "info",
            &format!("已取 {} 章作为上下文 + 基准", chapters.len()),
        );

        // ── 2. 独立沙盒故事 ─────────────────────────────────────
        ctx.update_progress("sandbox", 20, "正在创建训练沙盒故事…");
        let story_repo = StoryRepository::new(self.pool.clone());
        let stamp = Local::now().format("%m%d-%H%M");
        let story = story_repo.create(crate::db::dto::CreateStoryRequest {
            title: format!("[训练]《{}》·{stamp}", payload.book_title),
            description: Some(format!(
                "续写训练沙盒：{} 的第 {}–{} 章灌入 + 续写对比。由定时任务自动创建与清理。",
                payload.book_title,
                chapters.first().map(|c| c.0).unwrap_or(0),
                chapters.first().map(|c| c.0).unwrap_or(0) + chapters.len() - 2
            )),
            genre: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })?;
        self.seed_scenes(&story.id, &chapters[..chapters.len() - 1], &ctx)
            .await?;

        // ── 3. 风格 DNA（真实章节 → StyleAnalyzer → 绑定故事）────
        ctx.update_progress("style", 30, "正在从真实章节学习文风…");
        let style_applied = self
            .apply_style_dna(&story.id, &payload.book_title, &chapters, &ctx)
            .await;

        // ── 3.5 训练资产重建（v0.94.0）─────────────────────────
        // 训练沙盒不是“从零创世”：先把上下文章归一为世界观/故事大纲/角色/
        // 关系/逐章梗概，再进入既有续写与编审管线。这样 writer 拿到的资产
        // 与正作续写同源，不再因资产区为空拒绝或另起新故事。
        ctx.update_progress("assets", 35, "正在重建世界观/大纲/角色/故事线…");
        let asset_llm = crate::llm::LlmService::new(self.app_handle.clone());
        let asset_input: Vec<(usize, String)> = context_chapters(&chapters)
            .iter()
            .map(|(number, content)| (*number, content.clone()))
            .collect();
        let rebuilt = crate::agency::training_assets::rebuild_from_context(
            &self.pool,
            &asset_llm,
            &story.id,
            &payload.book_title,
            &asset_input,
        )
        .await;
        if let Some(err) = rebuilt.llm_error.as_deref() {
            ctx.log("warn", &format!("资产重建失败（继续训练）：{err}"));
        } else {
            ctx.log(
                "info",
                &format!(
                    "资产重建完成：世界观{}｜故事大纲{}｜角色{}｜关系{}｜章节梗概{}",
                    if rebuilt.world { "✓" } else { "—" },
                    if rebuilt.story_outline { "✓" } else { "—" },
                    rebuilt.characters,
                    rebuilt.relationships,
                    rebuilt.chapter_outlines
                ),
            );
        }
        for warning in rebuilt.warnings.iter().take(4) {
            ctx.log("warn", warning);
        }

        // ── 4. 承接锚点 + 上轮改进建议注入（自动闭环）──────────
        // v0.93.16: 沙盒没有世界观/角色卡/大纲资产，通用管线会因「创作资产
        // 为空」拒绝写作，或把续写写成新故事开篇。训练指令必须携带上一章
        // 的真实结尾；且只用上下文章（末位基准章禁止泄漏）。
        let style_anchor = if style_applied {
            String::new()
        } else {
            let anchor = build_style_anchor(&chapters);
            if !anchor.is_empty() {
                ctx.log(
                    "info",
                    "风格 DNA 不可用，已注入真实章节末尾节选作为文风锚点",
                );
            }
            anchor
        };
        let context_start = chapters.first().map(|c| c.0).unwrap_or(0);
        let continuity_anchor = build_continuity_anchor(&chapters);
        if !continuity_anchor.is_empty() {
            ctx.log("info", "已注入上一章结尾原文作为硬承接锚点");
        }
        // v0.93.16: 只采用与当前承接区间重叠的上轮报告；跨区间（如上一轮
        // 训练第 1–10 章、本轮第 97–106 章）的剧情建议会直接污染续写。
        let prior = self.load_prior_insights(&payload.book_title, context_start);
        let base_instruction = format!(
            "这是续写训练沙盒：前 {} 章是小说《{}》的真实内容。请以与原文一致的文风、\
             叙事节奏和人物口吻，自然续写下一章（不要总结、不要复述、不要写大纲）。",
            chapters.len() - 1,
            payload.book_title
        );
        let base_instruction = if style_anchor.is_empty() {
            base_instruction
        } else {
            format!(
                "{base_instruction}\n\n【文风锚点（原文节选，务必模仿其遣词造句、对话口吻、\
                 叙事节奏与生活细节密度，不得改换语体）】\n{style_anchor}"
            )
        };
        let mut instruction = match &prior {
            Some(insights) => {
                format!("{base_instruction}\n\n历史训练结论（务必落实）：\n{insights}")
            }
            None => base_instruction,
        };
        // 承接锚点放最后（指令 recency 最高）；旧版 style anchor 截断也
        // 不会再把它挤掉。
        if !continuity_anchor.is_empty() {
            instruction = format!("{instruction}\n\n【必须承接的原文结尾】\n{continuity_anchor}");
        }

        // ── 5. 走既有续写管线（含 gate/装配）────────────────────
        ctx.update_progress("continue", 40, "正在按原文风格续写下一章…");
        let run_id = uuid::Uuid::new_v4().to_string();
        let coordinator = crate::agency::coordinator::AgencyCoordinator::new(
            self.app_handle.clone(),
            self.pool.clone(),
        );
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
        let next_chapter = (chapters.len() - 1 + 1) as i32; // 上下文章节数 + 1
        let batch = coordinator
            .run_continue_batch_with_cancel(
                &run_id,
                &story.id,
                next_chapter,
                1,
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
        // 生成正文已装配进 scene（batch 只回 scene_id/章号），从 scene 读
        let scene_repo =
            crate::db::repositories::scene_repository::SceneRepository::new(self.pool.clone());
        let generated_raw = batch
            .chapters
            .first()
            .map(|c| {
                scene_repo
                    .get_by_id(&c.scene_id)
                    .ok()
                    .flatten()
                    .and_then(|sc| sc.content)
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        // v0.93.16：空稿必须是「视觉空」而不是仅字符串空——tiptap 的空文档
        // 序列化为 `<p></p>`（非空字符串），旧代码会把它当正文送去判官，
        // 落出 0.0/scores:null 的假报告。拒绝话术（「创作资产区域为空」）
        // 同样是模型元评论，不是正文。
        let generated = crate::export::content::content_to_plain_text(&generated_raw);
        if let Err(reason) = validate_generated_prose(&generated) {
            let msg = format!("续写产出不可用：{reason}");
            ctx.log("error", &msg);
            // 沙盒保留供排查；报告不落（无产出可比）
            return Ok(TaskResult {
                success: false,
                result_json: None,
                error_message: Some(msg),
            });
        }
        ctx.update_progress("compare", 75, "正在取真实后续章并对比评审…");

        // ── 6. 真实后续章作基准 ─────────────────────────────────
        let truth_index = chapters.last().map(|c| c.0).unwrap_or(0);
        let truth = crate::export::content::content_to_plain_text(
            &chapters.last().map(|c| c.1.clone()).unwrap_or_default(),
        );

        // ── 7. LLM 对比评审（五维 + 承接基准）─────────────────
        // v0.94.0：判官除对比真实后续章，还要看到上一章结尾现场，专门检查
        // AI 是否紧接该现场；这样 improvement 能沉淀“衔接失败”的经验。
        let (prior_chapter, prior_tail) = judge_prior_baseline(&chapters);
        let llm = crate::llm::LlmService::new(self.app_handle.clone());
        let (report_md, scores_json, improvement_md) = self
            .compare_and_report(
                &llm,
                &payload.book_title,
                &truth,
                &generated,
                truth_index,
                next_chapter,
                prior_chapter,
                &prior_tail,
            )
            .await?;
        // v0.94.0：报告固定记录本轮“资产重建 → 编审 → 对比”三段经验，
        // 让下一轮注入的建议能同时优化资产管理、承接与写作。
        let gate_verdict = batch
            .chapters
            .first()
            .map(|c| c.verdict.verdict.as_str())
            .unwrap_or("-");
        let revised = batch.chapters.first().map(|c| c.revised).unwrap_or(false);
        let asset_line = if let Some(err) = rebuilt.llm_error.as_deref() {
            format!(
                "资产重建失败（继续训练）：{}",
                Self::preview_chars(err, 160)
            )
        } else {
            format!(
                "世界观{}｜故事大纲{}｜角色{}｜关系{}｜章节梗概{}",
                if rebuilt.world { "✓" } else { "—" },
                if rebuilt.story_outline { "✓" } else { "—" },
                rebuilt.characters,
                rebuilt.relationships,
                rebuilt.chapter_outlines
            )
        };
        let report_md = format!(
            "{report_md}\n\n## 本轮训练流程\n\
             - 资产重建：{asset_line}\n\
             - 编审：质量门 verdict={gate_verdict}｜修订={revised}\n\
             - 对比评审：五维评分 + 上一章结尾承接基准\n\
             - 下一轮：自动注入本报告 improvement 与重叠区间历史建议\n"
        );
        ctx.update_progress("report", 90, "正在保存训练报告…");

        // ── 8. 报告落库 + 清理旧沙盒 ───────────────────────────
        let report_id = uuid::Uuid::new_v4().to_string();
        {
            let conn = self.pool.get().map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT INTO continuation_training_reports \
                 (id, task_id, book_title, source_kind, story_id, run_id, start_chapter, \
                  context_chapters, generated_chapter, truth_content, scores_json, report_md, \
                  improvement_md, style_dna_applied) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                rusqlite::params![
                    report_id,
                    task.id,
                    payload.book_title,
                    source_kind,
                    story.id,
                    run_id,
                    chapters.first().map(|c| c.0 as i64).unwrap_or(0),
                    (chapters.len() - 1) as i64,
                    next_chapter,
                    truth,
                    scores_json,
                    report_md,
                    improvement_md,
                    style_applied as i64,
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        self.cleanup_old_sandboxes(&payload.book_title, KEEP_SANDBOXES);

        ctx.update_progress("done", 100, "训练完成");
        let summary = format!(
            "训练完成：沙盒 {}，续写沙盒第 {} 章（对应真实第 {} 章），报告 {report_id}（保留最近 {KEEP_SANDBOXES} 个沙盒）",
            story.title, next_chapter, truth_index
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
    /// 取源章节：末位一章是「真实后续」基准，其余为上下文
    async fn fetch_chapters(
        &self,
        payload: &TrainingPayload,
        ctx: &TaskExecutionContext,
        n: usize,
    ) -> Result<(Vec<Chapter>, &'static str), String> {
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
                let end = (start + n).min(total);
                let slice = &parsed.chapters[start.min(total)..end];
                let out = slice
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (start + i + 1, Self::cap(&c.content)))
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
                // 连续取 n+1 章（n 上下文 + 1 基准）；连续两次空结果即止
                let mut out: Vec<Chapter> = Vec::new();
                let mut empty_streak = 0;
                for offset in 0..=n {
                    if ctx.is_cancelled() {
                        break;
                    }
                    let index = start + offset;
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
                    out.push((index, Self::cap(&trimmed)));
                    ctx.update_progress(
                        "fetch",
                        5 + (offset * 10 / n.max(1)) as i32,
                        &format!("已取第 {index} 章（{} 字）", trimmed.chars().count()),
                    );
                }
                Ok((out, "mcp"))
            }
        }
    }

    fn pick_start(payload: &TrainingPayload, total: usize) -> usize {
        match payload.strategy.as_str() {
            "random" => {
                let upper = if payload.est_total_chapters > 0 {
                    payload.est_total_chapters.min(total)
                } else {
                    total
                };
                let window = upper.saturating_sub(payload.chapter_count + 1).max(1);
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
        let mut template = serde_json::Map::new();
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

    /// 最近 2 轮报告的 improvement 建议拼成闭环提示。
    ///
    /// v0.93.16：只取与当前承接区间（`start_chapter..start+context_chapters`）
    /// 重叠的报告。跨区间建议是具体剧情/人设（如「从包玉芹送房继续」），
    /// 被注入到第 97 章之后的训练会直接污染续写（真机事故：现代都市陈默）。
    fn load_prior_insights(&self, book_title: &str, start_chapter: usize) -> Option<String> {
        let conn = self.pool.get().ok()?;
        let mut stmt = conn
            .prepare(
                "SELECT improvement_md FROM continuation_training_reports \
                 WHERE book_title = ?1 AND improvement_md IS NOT NULL AND improvement_md != '' \
                 AND start_chapter <= ?2 \
                 AND ?2 < start_chapter + context_chapters \
                 ORDER BY created_at DESC LIMIT 2",
            )
            .ok()?;
        let rows: Vec<String> = stmt
            .query_map(rusqlite::params![book_title, start_chapter as i64,], |r| {
                r.get::<_, String>(0)
            })
            .ok()?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        if rows.is_empty() {
            return None;
        }
        // rows 按 created_at DESC 取 2 条后反转：i=0 是更早一轮，i=1
        // 是最近一轮。
        Some(
            rows.iter()
                .rev()
                .enumerate()
                .map(|(i, r)| {
                    let label = if i == 0 {
                        "更早一轮"
                    } else {
                        "最近一轮"
                    };
                    format!("{label}训练建议：\n{r}")
                })
                .collect::<Vec<_>>()
                .join("\n\n"),
        )
    }

    /// 五维对比评审 → (report_md, scores_json, improvement_md)
    async fn compare_and_report(
        &self,
        llm: &crate::llm::LlmService,
        book_title: &str,
        truth: &str,
        generated: &str,
        truth_chapter: usize,
        generated_chapter: i32,
        prior_chapter: usize,
        prior_tail: &str,
    ) -> Result<(String, String, String), String> {
        let truth_cap = Self::cap(&crate::export::content::content_to_plain_text(truth));
        let gen_cap = Self::cap(&crate::export::content::content_to_plain_text(generated));

        // v0.93.15：判官曾返回 {"scores":null,...}，旧代码照样落库成
        // 总分 0.00、五维「-」的假报告。评分不可用时先重试一次；
        // 仍不可用则降级存「评分不可用」报告（overall=null），不污染趋势。
        let mut judge: Option<JudgeReport> = None;
        let mut last_raw = String::new();
        for attempt in 0..=1 {
            let prompt = Self::judge_prompt(
                book_title,
                &truth_cap,
                &gen_cap,
                truth_chapter,
                generated_chapter,
                prior_chapter,
                prior_tail,
                attempt > 0,
            );
            let resp = llm
                .generate_for_task(
                    crate::router::TaskType::Analysis,
                    prompt,
                    Some(1024),
                    Some(0.2),
                    Some("continuation_training_report"),
                )
                .await
                .map_err(|e| format!("对比评审 LLM 调用失败：{e}"))?;
            last_raw = resp.content.clone();
            judge = parse_judge_report(&last_raw);
            if judge.is_some() {
                break;
            }
            log::warn!(
                "[ContinuationTraining] 判官输出缺少可用五维评分（第 {} 次）: raw={}",
                attempt + 1,
                Self::preview_chars(&last_raw, 240)
            );
        }
        // 质检失败降级保产出（对齐既有「质检失败不判死」口径）：判官两轮
        // 都不可用时不落假 0.00 分，改存一条「评分不可用」报告，正文与沙盒
        // 保留；overall 为 null，趋势图会跳过这一轮。
        let judge = match judge {
            Some(judge) => judge,
            None => {
                let message = format!(
                    "判官未返回可用的五维评分（已重试）；输出前 240 字: {}",
                    Self::preview_chars(&last_raw, 240)
                );
                log::warn!("[ContinuationTraining] 对比评审降级: {}", message);
                let prior_line = if prior_chapter > 0 {
                    format!(
                        "- 衔接基准：原书第 {prior_chapter} 章结尾（判官据此判断是否紧接现场）\n"
                    )
                } else {
                    String::new()
                };
                let report_md = format!(
                    "# 续写训练报告 · 《{book_title}》\n\n\
                     - 基准：真实第 {truth_chapter} 章 ｜ AI 续写：沙盒第 {generated_chapter} 章（对应真实第 {truth_chapter} 章）\n\
                     {prior_line}\
                     - **本轮评分不可用**：判官未返回可用的五维评分（已重试），不计入训练趋势。\n\n\
                     ## 判官输出诊断\n\n{}\n",
                    Self::preview_chars(&last_raw, 240)
                );
                return Ok((
                    report_md,
                    serde_json::json!({
                        "scores": serde_json::Value::Null,
                        "overall": serde_json::Value::Null,
                        "judge_error": message,
                    })
                    .to_string(),
                    String::new(),
                ));
            }
        };

        let prior_line = if prior_chapter > 0 {
            format!("- 衔接基准：原书第 {prior_chapter} 章结尾（判官据此判断是否紧接现场）\n")
        } else {
            String::new()
        };
        let report_md = format!(
            "# 续写训练报告 · 《{book_title}》\n\n\
             - 基准：真实第 {truth_chapter} 章 ｜ AI 续写：沙盒第 {generated_chapter} 章（对应真实第 {truth_chapter} 章）\n\
             {prior_line}\
             - 加权总分：**{:.2}**\n\n\
             ## 五维评分\n\n\
             - 文笔风格：{}\n\
             - 情节推进：{}\n\
             - 人物口吻：{}\n\
             - 节奏密度：{}\n\
             - 钩子悬念：{}\n\n\
             ## 评语\n\n{}\n\n\
             ## 下一轮改进\n\n{}\n",
            judge.overall,
            judge
                .scores
                .get("style")
                .map(fmt_num)
                .unwrap_or_else(|| "-".into()),
            judge
                .scores
                .get("plot")
                .map(fmt_num)
                .unwrap_or_else(|| "-".into()),
            judge
                .scores
                .get("voice")
                .map(fmt_num)
                .unwrap_or_else(|| "-".into()),
            judge
                .scores
                .get("pacing")
                .map(fmt_num)
                .unwrap_or_else(|| "-".into()),
            judge
                .scores
                .get("hooks")
                .map(fmt_num)
                .unwrap_or_else(|| "-".into()),
            judge.comments,
            judge.improvement,
        );
        Ok((
            report_md,
            serde_json::json!({
                "scores": judge.scores,
                "overall": judge.overall,
            })
            .to_string(),
            judge.improvement,
        ))
    }

    /// 判官提示词。`strict` 为二次尝试：上一次输出无法解析时，要求只给
    /// 纯 JSON 对象，不再夹带解释文字或代码围栏。
    fn judge_prompt(
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
                "【上一章结尾（衔接基准；只用于判断续写是否紧接该现场，不是评分基准）】\n\
                 {prior_tail}\n\n"
            )
        };
        format!(
            r#"{strict_line}你是网文编辑。下面是《{book_title}》第 {truth_chapter} 章的真实原文，以及 AI 按前文续写出的下一章（沙盒第 {generated_chapter} 章，对应真实第 {truth_chapter} 章）。请从五个维度给 AI 续写打分（1-10，10=与作者本人水准不可区分），并给出下一轮训练要落实的改进建议。

{prior_block}【真实原文（基准）】
{truth_cap}

【AI 续写】
{gen_cap}

特别检查：AI 续写是否紧接「上一章结尾」的现场、人物与未完结事件（不得换场、新主角、新委托、新事件）。若衔接失败，comments 与 improvement 必须明确指出并从下一轮起修正。

仅输出 JSON：
{{"scores":{{"style":文笔风格相似度,"plot":情节推进合理性,"voice":人物口吻一致性,"pacing":节奏密度匹配,"hooks":钩子与悬念经营}},"overall":0.0-1.0加权总分,"comments":"每维 1-2 句依据（含承接判断）","improvement":"下一轮续写必须落实的 2-3 条具体改进（含承接修正，给模型看，不超过 150 字）"}}"#
        )
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
        if ids.len() <= keep {
            return;
        }
        let story_repo = StoryRepository::new(self.pool.clone());
        for id in ids.into_iter().skip(keep) {
            match story_repo.delete(&id) {
                Ok(_) => log::info!("[ContinuationTraining] 已清理旧沙盒 {id}"),
                Err(e) => log::warn!("[ContinuationTraining] 清理旧沙盒 {id} 失败：{e}"),
            }
        }
    }
}

/// 判官输出（已校验）：五维分数 + 归一化总分 + 评语/改进。
struct JudgeReport {
    scores: serde_json::Map<String, serde_json::Value>,
    overall: f64,
    comments: String,
    improvement: String,
}

const JUDGE_DIMENSIONS: [&str; 5] = ["style", "plot", "voice", "pacing", "hooks"];

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
}

/// 解析并校验判官输出：五维必须齐全且为 1-10 的数值（或数字字符串/
/// "6/10"）；缺失任一分维度即视为不可用（返回 None），由调用方重试，
/// 不再落库「总分 0.00、五维 -」的假报告。overall 缺失时按五维均值折算。
fn parse_judge_report(raw: &str) -> Option<JudgeReport> {
    let parsed: RawJudgeReport = crate::agency::coordinator::parse_lenient(raw)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_anchor_takes_tail_of_last_two_context_chapters() {
        let chapters: Vec<Chapter> = (1..=3)
            .map(|i| (i, format!("第{i}章开头铺垫。{}", "市井细节。".repeat(200))))
            .collect();
        let anchor = build_style_anchor(&chapters);
        // 只取上下文最后两章（3 是基准章，禁止泄漏）
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
        let anchor = build_style_anchor(&chapters);
        assert!(anchor.contains("桐人镇魇现场"));
        assert!(
            !anchor.contains("答案标记不得泄漏"),
            "锚点泄漏基准章: {anchor}"
        );
    }

    #[test]
    fn style_anchor_handles_short_context_chapters() {
        let chapters: Vec<Chapter> = vec![
            (1, "短章一".into()),
            (2, "短章二".into()),
            (3, "基准章".into()),
        ];
        let anchor = build_style_anchor(&chapters);
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
        let anchor = build_continuity_anchor(&chapters);
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
    fn judge_prompt_names_sandbox_and_source_chapter() {
        let prompt = ContinuationTrainingExecutor::judge_prompt(
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
    }

    #[test]
    fn judge_prior_baseline_uses_last_context_chapter_tail_only() {
        let chapters: Vec<Chapter> = vec![
            (106, "第106章结尾桐人镇魇。".into()),
            (107, "第107章真实基准答案。".into()),
        ];
        let (number, tail) = judge_prior_baseline(&chapters);
        assert_eq!(number, 106);
        assert!(tail.contains("桐人镇魇"));
        assert!(!tail.contains("真实基准答案"), "判官衔接基准不得含答案");
    }
}
