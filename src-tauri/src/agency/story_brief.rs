//! v0.99.1：续写长程记忆（前情提要 / 本批账本的数据结构、渲染与存储）。
//!
//! 背景（2026-11-19 分析报告
//! `docs/plans/2026-11-19-continuation-training-long-range-memory-analysis.
//! md`）： 训练/续写此前只有「上一章结尾 900–1400 字」这一条硬承接锚点，前 10
//! 章的 逐章事实、未结线索、人物当前态与设定白名单从未作为一个整体进入 writer /
//! planner。资产重建即使生成了 10 条逐章梗概，writer 的进度块也只读最近 3 章
//! 正文末 220 字；planner 只读最近 3
//! 条梗概。结果就是「局部接得住、长程记不住」。
//!
//! 本模块把这份长程记忆收敛成一个一等数据结构：
//! - `StoryBrief`：整批固定的前情提要（逐章事实 + 未结线索 + 人物态 +
//!   设定白名单）
//! - 渲染为 protected 提示词块，进入 writer 与 planner；
//! - 落 `story_briefs` 表，训练沙盒随故事删除/清理。
//!
//! 设计约束：
//! - 只描述「已发生」的事实，不写「下一拍/场景一/目标→冲突→灾难」等规划；
//! - 所有人物/专名必须来自前文原文；名册外的人名不得进入；
//! - 整批固定（stable），不得注入每章变化的 `next_node`，避免打断前缀缓存。

use chrono::Local;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;

/// 前情提要渲染上限（字符）。10 章 × 120 字 + 线索/人物/白名单约 2.5–3.5k。
pub const STORY_BRIEF_CHAR_CAP: usize = 3_600;
/// 总脉络上限。
const ARC_SUMMARY_CHAR_CAP: usize = 320;
/// 单章事实行上限。
const CHAPTER_LINE_CHAR_CAP: usize = 200;
/// 单条线索上限。
const THREAD_LINE_CHAR_CAP: usize = 160;
/// 单个人物态上限。
const CAST_LINE_CHAR_CAP: usize = 140;
/// 单个设定白名单条目上限。
const TERM_LINE_CHAR_CAP: usize = 100;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct StoryBrief {
    #[serde(default)]
    pub start_chapter: i32,
    #[serde(default)]
    pub end_chapter: i32,
    #[serde(default)]
    pub context_chapters: usize,
    /// v0.99.12：第一人称主角的权威姓名（如「周成」）。资产重建的 cast 可能
    /// 同时出现「我/主角」与真实姓名，若不合并，writer 与编审会把同一人当
    /// 成两个角色（真机：编审把主角被称「周成」判成「换成配角」）。
    #[serde(default)]
    pub protagonist: String,
    #[serde(default)]
    pub arc_summary: String,
    #[serde(default)]
    pub chapters: Vec<BriefChapter>,
    #[serde(default)]
    pub threads: Vec<BriefThread>,
    #[serde(default)]
    pub cast_states: Vec<BriefCastState>,
    #[serde(default)]
    pub canon_terms: Vec<BriefTerm>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BriefChapter {
    #[serde(default)]
    pub chapter: i32,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub cast: Vec<String>,
    #[serde(default)]
    pub result: String,
    #[serde(default)]
    pub open_thread: String,
    /// v0.99.2：本章最大转折（只写已发生事实；供聚合 brief/评分探针）。
    #[serde(default)]
    pub turning_point: String,
    /// v0.99.2：本章真实出现的专名（术法/门派/器物/地名/组织），用于
    /// 资产重建失败时的确定性 canon 白名单兜底。
    #[serde(default)]
    pub key_terms: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BriefThread {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub next_expected: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BriefCastState {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub goal: String,
    #[serde(default)]
    pub knows: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BriefTerm {
    #[serde(default)]
    pub term: String,
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub note: String,
}

/// 资产摘要里用来指代第一人称主角的占位名（不是真实姓名）。
pub const PROTAGONIST_ALIASES: [&str; 6] = ["我", "主角", "主人公", "主角我", "本人", "阴脉先生"];

pub fn is_protagonist_alias(name: &str) -> bool {
    let name = name.trim();
    PROTAGONIST_ALIASES.iter().any(|alias| *alias == name)
}

/// v0.99.12：从原文里确定第一人称主角的姓名。
///
/// 不能依赖「角色表第一项」——确定性 cast 兜底按出场顺序创建角色（真机
/// 12:38 训练第一项是「大波浪卷女人」），旧 roster 会把配角标成主角。
/// 这里只认原文的显式自称/身份句式（我叫X / 我姓X / 以X身份 / X这个身份 /
/// X自报 / 自称X），无把握时返回 None，宁可不标注也不标错。不做「姓名附近
/// 有『我/身份』」的宽窗口计分——配角高频出现时会反超真正主角。
pub fn detect_protagonist_name(context: &str, candidates: &[String]) -> Option<String> {
    let mut best: Option<(String, usize)> = None;
    let mut tie = false;
    for name in candidates {
        let name = name.trim();
        if name.chars().count() < 2 || is_protagonist_alias(name) {
            continue;
        }
        let mut score = 0usize;
        for needle in [
            format!("我叫{name}"),
            format!("我名{name}"),
            format!("我姓{name}"),
            format!("我是{name}"),
            format!("{name}这个身份"),
            format!("以{name}身份"),
            format!("用{name}的身份"),
            format!("{name}自报"),
            format!("自称{name}"),
        ] {
            if context.contains(&needle) {
                score += 4;
            }
        }
        // v0.99.12：只认显式自称/身份句式，不做「姓名附近有『我』」的宽
        // 窗口计分。真机上下文里配角黄玄然出现 30 次、附近大量「我/身份」
        // 字样，窗口计分会反超真正主角周成，把配角标成主角。
        if score == 0 {
            continue;
        }
        match &best {
            Some((_, best_score)) if score < *best_score => {}
            Some((_, best_score)) if score == *best_score => tie = true,
            _ => {
                best = Some((name.to_string(), score));
                tie = false;
            }
        }
    }
    if tie {
        return None;
    }
    // 至少要命中一条自称/身份句式（+4）；只靠「与『我』同段」的弱信号
    // 不标注，宁可不标也不把配角当主角。
    best.filter(|(_, score)| *score >= 4).map(|(name, _)| name)
}

impl StoryBrief {
    /// 把「我/主角/主人公」等占位名合并到权威主角姓名，并去重 cast。
    /// 编审会据此判断「周成」不是配角；writer 也不会再把主角与占位名当成
    /// 两个人。
    pub fn canonicalize_protagonist(&mut self, protagonist: &str) {
        let name = protagonist.trim();
        if name.is_empty() || is_protagonist_alias(name) {
            return;
        }
        self.protagonist = name.to_string();
        for chapter in &mut self.chapters {
            let mut merged: Vec<String> = Vec::new();
            for cast in chapter.cast.drain(..) {
                let cast = cast.trim();
                if cast.is_empty() {
                    continue;
                }
                let mapped = if is_protagonist_alias(cast) {
                    name
                } else {
                    cast
                };
                if !merged.iter().any(|existing| existing == mapped) {
                    merged.push(mapped.to_string());
                }
            }
            chapter.cast = merged;
        }
        let mut states: Vec<BriefCastState> = Vec::new();
        let mut protagonist_idx: Option<usize> = None;
        for state in self.cast_states.drain(..) {
            let state_name = state.name.trim();
            let is_alias = is_protagonist_alias(state_name);
            let is_protagonist = state_name == name;
            if is_alias || is_protagonist {
                if let Some(idx) = protagonist_idx {
                    merge_cast_state(&mut states[idx], state);
                } else {
                    let mut state = state;
                    state.name = name.to_string();
                    states.push(state);
                    protagonist_idx = Some(states.len() - 1);
                }
            } else {
                states.push(state);
            }
        }
        if let Some(idx) = protagonist_idx {
            let protagonist_state = states.remove(idx);
            states.insert(0, protagonist_state);
        }
        self.cast_states = states;
    }

    pub fn is_empty(&self) -> bool {
        self.chapters.is_empty()
            && self.threads.is_empty()
            && self.cast_states.is_empty()
            && self.canon_terms.is_empty()
            && self.arc_summary.trim().is_empty()
    }

    /// 渲染为提示词块。按「逐章事实 → 未结线索 → 人物态 → 设定白名单」顺序，
    /// 超长时从尾部（白名单 → 人物态 → 线索）先截，逐章事实最后才动。
    pub fn render(&self, max_chars: usize) -> String {
        if self.is_empty() {
            return String::new();
        }
        let start = self
            .chapters
            .first()
            .map(|c| c.chapter)
            .filter(|n| *n > 0)
            .unwrap_or(self.start_chapter);
        let end = self
            .chapters
            .last()
            .map(|c| c.chapter)
            .filter(|n| *n > 0)
            .unwrap_or(self.end_chapter);
        let n = self.context_chapters.max(self.chapters.len());
        let mut out = format!(
            "【前情提要（原书第 {start}–{end} 章，共 {n} 章；本批固定，必须通读后再写）】\n"
        );
        if !self.protagonist.trim().is_empty() {
            out.push_str(&format!(
                "主角：{}（第一人称一律用「我」；他人称其姓名时即为同一人，禁止当配角）\n",
                take_chars(self.protagonist.trim(), 40)
            ));
        }
        if !self.arc_summary.trim().is_empty() {
            out.push_str(&format!(
                "总脉络：{}\n",
                take_chars(self.arc_summary.trim(), ARC_SUMMARY_CHAR_CAP)
            ));
        }
        if !self.chapters.is_empty() {
            out.push_str("逐章事实（只写已发生，不是计划；禁止据此复述原文）：\n");
            for c in &self.chapters {
                out.push_str(&format!("- {}", render_chapter_line(c)));
                out.push('\n');
            }
        }
        if !self.threads.is_empty() {
            out.push_str(
                "未结线索（本章必须推进至少一条，或在正文中明确写出「暂不收」的理由）：\n",
            );
            for t in &self.threads {
                let id = if t.id.trim().is_empty() {
                    "thread".to_string()
                } else {
                    t.id.trim().to_string()
                };
                let status = if t.status.trim().is_empty() {
                    "open"
                } else {
                    t.status.trim()
                };
                let next = if t.next_expected.trim().is_empty() {
                    String::new()
                } else {
                    format!("；下一预期：{}", t.next_expected.trim())
                };
                out.push_str(&format!(
                    "- [{id}] {}（状态：{status}{next}）\n",
                    take_chars(t.description.trim(), THREAD_LINE_CHAR_CAP)
                ));
            }
        }
        if !self.cast_states.is_empty() {
            out.push_str(
                "人物当前态（只可沿用，禁止改名/新造；原文第一人称「我」不得另起姓名）：\n",
            );
            for c in &self.cast_states {
                out.push_str(&format!("- {}\n", render_cast_line(c)));
            }
        }
        if !self.canon_terms.is_empty() {
            out.push_str(
                "已确立设定白名单（只可沿用，不得新增体系；名单外新名词不得写成规则）：\n",
            );
            for t in &self.canon_terms {
                let kind = if t.kind.trim().is_empty() {
                    "设定"
                } else {
                    t.kind.trim()
                };
                let note = if t.note.trim().is_empty() {
                    String::new()
                } else {
                    format!("：{}", t.note.trim())
                };
                out.push_str(&format!(
                    "- {}（{}）{}\n",
                    take_chars(t.term.trim(), TERM_LINE_CHAR_CAP),
                    kind,
                    take_chars(&note, TERM_LINE_CHAR_CAP)
                ));
            }
        }

        // 超长时优先牺牲尾部块：白名单 → 人物态 → 线索；逐章事实最后才截。
        if out.chars().count() > max_chars {
            out = self.render_with_limits(max_chars);
        }
        out
    }

    fn render_with_limits(&self, max_chars: usize) -> String {
        // 保留 header + 总脉络 + 逐章事实；按可用空间追加线索/人物/白名单。
        let mut out = format!(
            "【前情提要（原书第 {}–{} 章，共 {} 章；本批固定，必须通读后再写）】\n",
            self.chapters
                .first()
                .map(|c| c.chapter)
                .unwrap_or(self.start_chapter),
            self.chapters
                .last()
                .map(|c| c.chapter)
                .unwrap_or(self.end_chapter),
            self.context_chapters.max(self.chapters.len())
        );
        if !self.protagonist.trim().is_empty() {
            out.push_str(&format!(
                "主角：{}（第一人称一律用「我」；他人称其姓名时即为同一人，禁止当配角）\n",
                take_chars(self.protagonist.trim(), 40)
            ));
        }
        if !self.arc_summary.trim().is_empty() {
            out.push_str(&format!(
                "总脉络：{}\n",
                take_chars(self.arc_summary.trim(), ARC_SUMMARY_CHAR_CAP)
            ));
        }
        if !self.chapters.is_empty() {
            out.push_str("逐章事实（只写已发生，不是计划；禁止据此复述原文）：\n");
            for c in &self.chapters {
                out.push_str(&format!("- {}\n", render_chapter_line(c)));
            }
        }

        let mut sections: Vec<(String, Vec<String>)> = Vec::new();
        if !self.threads.is_empty() {
            let lines: Vec<String> = self
                .threads
                .iter()
                .map(|t| {
                    let id = if t.id.trim().is_empty() {
                        "thread"
                    } else {
                        t.id.trim()
                    };
                    let status = if t.status.trim().is_empty() {
                        "open"
                    } else {
                        t.status.trim()
                    };
                    let next = if t.next_expected.trim().is_empty() {
                        String::new()
                    } else {
                        format!("；下一预期：{}", t.next_expected.trim())
                    };
                    format!(
                        "- [{id}] {}（状态：{status}{next}）",
                        take_chars(t.description.trim(), THREAD_LINE_CHAR_CAP)
                    )
                })
                .collect();
            sections.push((
                "未结线索（本章必须推进至少一条，或在正文中明确写出「暂不收」的理由）：\n"
                    .to_string(),
                lines,
            ));
        }
        if !self.cast_states.is_empty() {
            let lines: Vec<String> = self
                .cast_states
                .iter()
                .map(|c| format!("- {}", render_cast_line(c)))
                .collect();
            sections.push((
                "人物当前态（只可沿用，禁止改名/新造；原文第一人称「我」不得另起姓名）：\n"
                    .to_string(),
                lines,
            ));
        }
        if !self.canon_terms.is_empty() {
            let lines: Vec<String> = self
                .canon_terms
                .iter()
                .map(|t| {
                    let kind = if t.kind.trim().is_empty() {
                        "设定"
                    } else {
                        t.kind.trim()
                    };
                    let note = if t.note.trim().is_empty() {
                        String::new()
                    } else {
                        format!("：{}", t.note.trim())
                    };
                    format!(
                        "- {}（{}）{}",
                        take_chars(t.term.trim(), TERM_LINE_CHAR_CAP),
                        kind,
                        take_chars(&note, TERM_LINE_CHAR_CAP)
                    )
                })
                .collect();
            sections.push((
                "已确立设定白名单（只可沿用，不得新增体系；名单外新名词不得写成规则）：\n"
                    .to_string(),
                lines,
            ));
        }

        // 从最后一个 section 往前加，空间不够就整段/逐行放弃。
        for (header, lines) in sections {
            let header_chars = header.chars().count();
            if out.chars().count() + header_chars >= max_chars {
                break;
            }
            let mut block = header;
            let mut kept = 0usize;
            for line in lines {
                if out.chars().count() + block.chars().count() + line.chars().count() + 2
                    > max_chars
                {
                    break;
                }
                block.push_str(&line);
                block.push('\n');
                kept += 1;
            }
            if kept > 0 {
                out.push_str(&block);
            }
        }
        take_chars_max(&out, max_chars)
    }
}

fn render_chapter_line(c: &BriefChapter) -> String {
    let summary = c.summary.trim();
    let result = c.result.trim();
    let open = c.open_thread.trim();
    let mut text = if summary.is_empty() {
        "（本章无摘要）".to_string()
    } else {
        take_chars(summary, CHAPTER_LINE_CHAR_CAP)
    };
    if !result.is_empty() && !text.contains(result) {
        text.push_str(&format!(" → {}", take_chars(result, 80)));
    }
    if !open.is_empty() && !text.contains(open) {
        text.push_str(&format!("（未决：{}）", take_chars(open, 80)));
    }
    format!("第{}章：{}", c.chapter, text)
}

fn render_cast_line(c: &BriefCastState) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !c.role.trim().is_empty() {
        parts.push(c.role.trim().to_string());
    }
    if !c.location.trim().is_empty() {
        parts.push(format!("所在地：{}", c.location.trim()));
    }
    if !c.goal.trim().is_empty() {
        parts.push(format!("目标：{}", c.goal.trim()));
    }
    if !c.knows.is_empty() {
        parts.push(format!("已知：{}", c.knows.join("、")));
    }
    let body = if parts.is_empty() {
        "（无补充状态）".to_string()
    } else {
        parts.join("；")
    };
    format!(
        "{}：{}",
        c.name.trim(),
        take_chars(&body, CAST_LINE_CHAR_CAP)
    )
}

/// 从逐章摘要构建一份确定性 brief（LLM 不可用/资产重建失败时的兜底）。
/// `summaries` 可以少于 `context_chapters`；缺章直接跳过，不伪造。
pub fn brief_from_summaries(
    summaries: Vec<BriefChapter>,
    arc_hint: &str,
    threads: Vec<BriefThread>,
    cast_states: Vec<BriefCastState>,
    canon_terms: Vec<BriefTerm>,
) -> StoryBrief {
    let start_chapter = summaries.first().map(|c| c.chapter).unwrap_or(0);
    let end_chapter = summaries.last().map(|c| c.chapter).unwrap_or(0);
    let context_chapters = summaries.len();
    let arc_summary = if arc_hint.trim().is_empty() {
        match (summaries.first(), summaries.last()) {
            (Some(first), Some(last)) if summaries.len() > 1 => format!(
                "从第{}章的「{}」推进到第{}章的「{}」。",
                first.chapter,
                take_chars(first.summary.trim(), 90),
                last.chapter,
                take_chars(last.summary.trim(), 90)
            ),
            (Some(first), _) => take_chars(first.summary.trim(), 180),
            _ => String::new(),
        }
    } else {
        take_chars(arc_hint.trim(), ARC_SUMMARY_CHAR_CAP)
    };
    StoryBrief {
        start_chapter,
        end_chapter,
        context_chapters,
        protagonist: String::new(),
        arc_summary,
        chapters: summaries,
        threads,
        cast_states,
        canon_terms,
        source: "deterministic".to_string(),
        updated_at: Local::now().to_rfc3339(),
    }
}

/// 合并同一主角的两份人物态：保留更长/更具体的角色、目标与已知信息。
fn merge_cast_state(into: &mut BriefCastState, other: BriefCastState) {
    if into.role.trim().is_empty() || other.role.chars().count() > into.role.chars().count() {
        into.role = other.role;
    }
    if into.location.trim().is_empty() {
        into.location = other.location;
    }
    if into.goal.trim().is_empty() || other.goal.chars().count() > into.goal.chars().count() {
        into.goal = other.goal;
    }
    for know in other.knows {
        let know = know.trim();
        if !know.is_empty() && !into.knows.iter().any(|existing| existing == know) {
            into.knows.push(know.to_string());
        }
    }
}

fn take_chars(text: &str, max: usize) -> String {
    let n = text.chars().count();
    if n <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn take_chars_max(text: &str, max: usize) -> String {
    let n = text.chars().count();
    if n <= max {
        return text.to_string();
    }
    text.chars().take(max).collect()
}

pub struct StoryBriefRepository {
    pool: DbPool,
}

impl StoryBriefRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn ensure_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS story_briefs (\
                story_id TEXT PRIMARY KEY,\
                content_json TEXT NOT NULL,\
                source TEXT NOT NULL DEFAULT 'unknown',\
                created_at TEXT NOT NULL,\
                updated_at TEXT NOT NULL\
            );",
        )
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Option<StoryBrief>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        let raw: Option<String> = conn
            .query_row(
                "SELECT content_json FROM story_briefs WHERE story_id = ?1",
                rusqlite::params![story_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.and_then(|json| serde_json::from_str::<StoryBrief>(&json).ok()))
    }

    pub fn upsert(&self, story_id: &str, brief: &StoryBrief) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        let now = Local::now().to_rfc3339();
        let json = serde_json::to_string(brief)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO story_briefs (story_id, content_json, source, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?4) \
             ON CONFLICT(story_id) DO UPDATE SET \
                content_json = excluded.content_json, \
                source = excluded.source, \
                updated_at = excluded.updated_at",
            rusqlite::params![story_id, json, brief.source, now],
        )
    }

    pub fn delete_by_story(&self, story_id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        conn.execute(
            "DELETE FROM story_briefs WHERE story_id = ?1",
            rusqlite::params![story_id],
        )
    }

    /// 清理已不存在故事的孤儿 brief（训练沙盒级联删除后调用）。
    pub fn prune_orphans(&self) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        conn.execute(
            "DELETE FROM story_briefs WHERE story_id NOT IN (SELECT id FROM stories)",
            [],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(n: i32, summary: &str) -> BriefChapter {
        BriefChapter {
            chapter: n,
            summary: summary.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn render_contains_every_chapter_in_order() {
        let brief = brief_from_summaries(
            vec![
                chapter(134, "赵开来点破主角借刀杀人的来意。"),
                chapter(135, "主角醉酒回家，救下百花女子。"),
                chapter(136, "主角在梦中斩退精怪，人面斑转移。"),
            ],
            "主角从饭局走到医院。",
            vec![BriefThread {
                id: "t-fomosa".into(),
                description: "赵开来承诺赠送福摩萨夜总会道场，需去见邵卫江。".into(),
                status: "open".into(),
                next_expected: "近期去福摩萨夜总会。".into(),
            }],
            vec![BriefCastState {
                name: "周成".into(),
                role: "主角/阴脉先生".into(),
                goal: "查清小护士身份".into(),
                ..Default::default()
            }],
            vec![BriefTerm {
                term: "福摩萨夜总会".into(),
                kind: "地点".into(),
                note: "赵开来承诺的道场线索".into(),
            }],
        );
        let out = brief.render(STORY_BRIEF_CHAR_CAP);
        assert!(out.contains("第134章"));
        assert!(out.contains("第135章"));
        assert!(out.contains("第136章"));
        assert!(out.contains("t-fomosa"));
        assert!(out.contains("福摩萨夜总会"));
        assert!(out.contains("周成"));
        // 顺序：134 在 135 前
        let i134 = out.find("第134章").unwrap();
        let i135 = out.find("第135章").unwrap();
        assert!(i134 < i135);
    }

    #[test]
    fn render_respects_cap_and_keeps_chapters() {
        let summaries: Vec<BriefChapter> = (1..=10)
            .map(|n| chapter(n, &"很长的章节摘要。".repeat(60)))
            .collect();
        let brief = brief_from_summaries(summaries, &"总脉络。".repeat(80), vec![], vec![], vec![]);
        let out = brief.render(1_200);
        assert!(out.chars().count() <= 1_200);
        assert!(out.contains("第1章"));
    }

    #[test]
    fn empty_brief_renders_empty() {
        let brief = StoryBrief::default();
        assert!(brief.is_empty());
        assert!(brief.render(STORY_BRIEF_CHAR_CAP).is_empty());
    }

    #[test]
    fn protagonist_detection_prefers_self_identification() {
        let context = "赵开来问：“你到底是谁？”我道：“免贵姓周，三脉堂周成。”\
                       董强“哦”了一声。周成这个身份还得在金城继续用。";
        let candidates = vec![
            "大波浪卷女人".to_string(),
            "妙姐".to_string(),
            "周成".to_string(),
            "赵开来".to_string(),
        ];
        assert_eq!(
            detect_protagonist_name(context, &candidates).as_deref(),
            Some("周成")
        );
        // 无自称句式时不瞎猜（宁可不标主角，也不把配角标成主角）。
        let plain = "赵开来与董强在饭馆喝酒，妙姐推门进来。";
        assert_eq!(detect_protagonist_name(plain, &candidates), None);
        assert_eq!(detect_protagonist_name(context, &[]), None);
    }

    #[test]
    fn protagonist_detection_ignores_frequent_third_person_mentions() {
        // v0.99.12 真机误检：黄玄然出现 30+ 次且附近常有「我/身份」字样，
        // 宽窗口计分曾把配角标成主角；显式自称句式才应获胜。
        let mut context = String::new();
        for _ in 0..40 {
            context.push_str("黄玄然说高天观的事。我问他身份。");
        }
        context.push_str("后来我以周成身份立足金城；周成自报三脉堂周成。");
        let candidates = vec![
            "黄玄然".to_string(),
            "周成".to_string(),
            "赵开来".to_string(),
        ];
        assert_eq!(
            detect_protagonist_name(&context, &candidates).as_deref(),
            Some("周成")
        );
    }

    #[test]
    fn canonicalize_merges_alias_cast_into_protagonist() {
        let mut brief = brief_from_summaries(
            vec![BriefChapter {
                chapter: 136,
                cast: vec!["主角".into(), "我".into(), "周成".into(), "杨晓雯".into()],
                ..Default::default()
            }],
            "",
            vec![],
            vec![
                BriefCastState {
                    name: "我".into(),
                    role: "第一人称".into(),
                    ..Default::default()
                },
                BriefCastState {
                    name: "周成".into(),
                    role: "主角/阴脉先生".into(),
                    goal: "查清小护士身份".into(),
                    ..Default::default()
                },
                BriefCastState {
                    name: "杨晓雯".into(),
                    ..Default::default()
                },
            ],
            vec![],
        );
        brief.canonicalize_protagonist("周成");
        assert_eq!(brief.protagonist, "周成");
        assert_eq!(brief.chapters[0].cast, vec!["周成", "杨晓雯"]);
        assert_eq!(brief.cast_states.len(), 2);
        assert_eq!(brief.cast_states[0].name, "周成");
        assert!(brief.cast_states[0].goal.contains("小护士"));
        let rendered = brief.render(STORY_BRIEF_CHAR_CAP);
        assert!(rendered.contains("主角：周成"), "{rendered}");
        assert!(!rendered.contains("主角：我"), "{rendered}");
    }
}
