//! 章节切分原语（事务级）。
//!
//! 本模块提供纯粹的章节切分原语：`plan_split` 计算切分方案，
//! `split_chapter_in_tx` 在事务内执行一刀切分（重排 + 插新章 + 截断，
//! 调用方负责提交/回滚）。它们被
//! `story_system::chapter_splitter`（编排层：防抖触发、循环切分、事件发射）
//! 与 V127 修复迁移（`db::migrations::V127__split_overlong_chapters`）共用。
//! 放在 db 层是因为架构分层规则禁止 db 依赖 story_system
//! （story_system → db 方向是允许的）。

use chrono::Local;
use rusqlite::params;
use uuid::Uuid;

use crate::{domain::contracts::ChapterContract, utils::text::TextUtils};

/// 划分方式（与 AppConfig.chapter_split_mode 对齐）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChapterSplitMode {
    WordCount,
    Plot,
}

impl ChapterSplitMode {
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "plot" | "情节" => Self::Plot,
            _ => Self::WordCount,
        }
    }
}

/// 纯函数：在内容中找切分点（字节偏移）。找不到则返回 None。
///
/// - WordCount：超过阈值后，优先在最近段落边界切；找不到则在阈值附近的句末切。
/// - Plot：在超过「半阈值」后的情节边界切（空行 /
///   时间地点转换词）；无边界则回退字数切。
pub fn find_split_offset(content: &str, mode: ChapterSplitMode, max_chars: usize) -> Option<usize> {
    let total = TextUtils::chinese_word_count(content);
    if total <= max_chars || content.is_empty() {
        return None;
    }

    match mode {
        ChapterSplitMode::WordCount => find_word_count_split(content, max_chars),
        ChapterSplitMode::Plot => find_plot_split(content, max_chars)
            .or_else(|| find_word_count_split(content, max_chars)),
    }
}

fn char_offset_to_byte(content: &str, char_offset: usize) -> usize {
    content
        .char_indices()
        .nth(char_offset)
        .map(|(i, _)| i)
        .unwrap_or(content.len())
}

/// 将「字」计数映射到近似字符偏移（中文 1 字≈1 char；英文词按字符扫描近似）。
fn approx_char_index_for_word_budget(content: &str, budget: usize) -> usize {
    let mut counted = 0usize;
    let mut last_idx = 0usize;
    let mut in_english = false;

    for (i, ch) in content.char_indices() {
        last_idx = i + ch.len_utf8();
        if matches!(ch, '\u{4e00}'..='\u{9fff}') {
            in_english = false;
            counted += 1;
            if counted >= budget {
                return i + ch.len_utf8();
            }
        } else if ch.is_ascii_alphabetic() {
            if !in_english {
                in_english = true;
                counted += 1;
                if counted >= budget {
                    // 吃完当前英文词
                    let rest = &content[i..];
                    let word_end = rest
                        .find(|c: char| !c.is_ascii_alphabetic())
                        .map(|o| i + o)
                        .unwrap_or(content.len());
                    return word_end;
                }
            }
        } else {
            in_english = false;
        }
    }
    last_idx
}

fn find_word_count_split(content: &str, max_chars: usize) -> Option<usize> {
    let soft_end = approx_char_index_for_word_budget(content, max_chars);
    if soft_end == 0 || soft_end >= content.len() {
        return None;
    }

    // 优先：soft_end 之前最近的双换行 / 单换行段落边界
    let head = &content[..soft_end];
    if let Some(rel) = head.rfind("\n\n") {
        let at = rel + 2;
        if at > 0 && at < content.len() {
            return Some(at);
        }
    }
    if let Some(rel) = head.rfind('\n') {
        let at = rel + 1;
        if at > 0 && at < content.len() {
            return Some(at);
        }
    }

    // 次选：句末标点
    for (i, ch) in head.char_indices().rev() {
        if matches!(ch, '。' | '！' | '？' | '.' | '!' | '?') {
            let at = i + ch.len_utf8();
            if at > 0 && at < content.len() {
                return Some(at);
            }
        }
    }

    // 兜底：soft_end（保证在字符边界）
    let at = char_offset_to_byte(content, content[..soft_end].chars().count());
    if at > 0 && at < content.len() {
        Some(at)
    } else {
        None
    }
}

fn find_plot_split(content: &str, max_chars: usize) -> Option<usize> {
    let min_keep = (max_chars / 2).max(500);
    let min_byte = approx_char_index_for_word_budget(content, min_keep);
    let boundaries = detect_plot_boundaries(content);
    // 选第一个落在 [min_byte, soft_end*1.5] 的边界；否则选 min_byte
    // 之后最近边界
    let soft_end = approx_char_index_for_word_budget(content, max_chars);
    let upper = approx_char_index_for_word_budget(content, max_chars.saturating_mul(3) / 2)
        .max(soft_end + 1);

    let in_window: Vec<usize> = boundaries
        .into_iter()
        .filter(|&b| b >= min_byte && b < content.len() && b > 0)
        .collect();

    in_window
        .iter()
        .copied()
        .find(|&b| b <= upper)
        .or_else(|| in_window.into_iter().next())
}

/// 情节边界启发式（自包含，避免 story_system → book_deconstruction 依赖）。
fn detect_plot_boundaries(content: &str) -> Vec<usize> {
    let mut boundaries = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    let time_markers = [
        "三天后",
        "一周后",
        "一个月后",
        "一年后",
        "几年后",
        "数年后",
        "第二天",
        "次日",
        "当晚",
        "翌日",
        "翌晨",
        "与此同时",
        "同一时间",
        "不久",
        "过了一会儿",
        "片刻之后",
        "数日后",
        "几日后",
        "次日清晨",
    ];
    let location_markers = [
        "回到",
        "来到",
        "抵达",
        "进入",
        "离开",
        "走出",
        "走进",
        "另一边",
    ];

    let mut empty_line_count = 0usize;
    let mut last_boundary_line = 0usize;
    let mut byte_pos = 0usize;

    for (line_idx, line) in lines.iter().enumerate() {
        let line_bytes = line.len();
        let trimmed = line.trim();

        if trimmed.is_empty() {
            empty_line_count += 1;
            if empty_line_count >= 2 && line_idx.saturating_sub(last_boundary_line) > 5 {
                let pos = byte_pos;
                if boundaries.last().map(|b| pos > *b + 20).unwrap_or(true) {
                    boundaries.push(pos);
                    last_boundary_line = line_idx;
                }
            }
            // +1 for '\n' except possibly last line — lines() strips newlines;
            // reconstruct: after each line except we track via join length.
            byte_pos += line_bytes;
            if line_idx + 1 < lines.len() {
                byte_pos += 1; // newline
            }
            continue;
        }

        empty_line_count = 0;

        if line_idx.saturating_sub(last_boundary_line) > 10 {
            let is_time = time_markers.iter().any(|m| trimmed.starts_with(m));
            let is_loc = location_markers.iter().any(|m| trimmed.starts_with(m));
            if is_time || is_loc {
                if boundaries
                    .last()
                    .map(|b| byte_pos > *b + 20)
                    .unwrap_or(true)
                {
                    boundaries.push(byte_pos);
                    last_boundary_line = line_idx;
                }
            }
        }

        byte_pos += line_bytes;
        if line_idx + 1 < lines.len() {
            byte_pos += 1;
        }
    }

    boundaries
}

/// 切分结果（纯数据，便于单测）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterSplitPlan {
    pub keep: String,
    pub overflow: String,
    pub split_offset: usize,
}

pub fn plan_split(
    content: &str,
    mode: ChapterSplitMode,
    max_chars: usize,
) -> Option<ChapterSplitPlan> {
    let offset = find_split_offset(content, mode, max_chars)?;
    if offset == 0 || offset >= content.len() {
        return None;
    }
    let keep = content[..offset].to_string();
    let overflow = content[offset..].trim_start().to_string();
    if overflow.is_empty() || TextUtils::chinese_word_count(&keep) == 0 {
        return None;
    }
    Some(ChapterSplitPlan {
        keep,
        overflow,
        split_offset: offset,
    })
}

pub(crate) fn canonical_chapter_title(chapter_number: i32) -> String {
    format!("第{}章", chapter_number)
}

/// 「第N章」「第一章」这类由章号派生的标题。自定义名（「临江夜雨」）不是。
pub(crate) fn is_generic_chapter_title(title: &str) -> bool {
    let t = title.trim();
    let mut chars = t.chars();
    if chars.next() != Some('第') || !t.ends_with('章') {
        return false;
    }
    let mid: String = t
        .chars()
        .skip(1)
        .take(t.chars().count().saturating_sub(2))
        .collect();
    !mid.is_empty()
        && mid
            .chars()
            .all(|c| c.is_ascii_digit() || "一二三四五六七八九十百千零〇两".contains(c))
}

/// 纯函数：由合约 goal 推导章节标题（可单测）。
fn title_from_goal(goal: Option<&str>, chapter_number: i32) -> String {
    const MAX_TITLE_CHARS: usize = 30;
    match goal.map(str::trim).filter(|g| !g.is_empty()) {
        Some(g) => g.chars().take(MAX_TITLE_CHARS).collect(),
        None => canonical_chapter_title(chapter_number),
    }
}

/// 事务内查询**指定章号**的章节合约 goal；查询失败时告警并返回 None。
/// v0.33.7 fix：此前取"最新（chapter_number 最大）合约"的 goal 作为所有
/// 新切章的标题——循环切分时合约不随新章更新，导致一次切出的几十个新章
/// 全部共用同一个标题。合约按章号精确匹配，没有对应合约就回退 `第{N}章`。
///
/// 合约跟随内容（I-2）：`split_chapter_in_tx` 重排后章号 N+1 必无合约，
/// 本查询自然落空、回退 `第{N+1}章`；保留查询仅为兼容未参与重排的残留
/// 数据（如解析失败未被顺延的合约行）。
fn chapter_contract_goal_in_tx(
    tx: &rusqlite::Transaction,
    story_id: &str,
    chapter_number: i32,
) -> Option<String> {
    let mut stmt = match tx.prepare(
        "SELECT contract_json FROM story_contracts \
         WHERE story_id = ?1 AND contract_type = 'CHAPTER'",
    ) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("[ChapterSplitter] 查询章节合约失败，回退默认命名: {}", e);
            return None;
        }
    };
    let rows = match stmt.query_map(params![story_id], |r| r.get::<_, String>(0)) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("[ChapterSplitter] 查询章节合约失败，回退默认命名: {}", e);
            return None;
        }
    };
    for row in rows.flatten() {
        if let Ok(c) = serde_json::from_str::<ChapterContract>(&row) {
            if c.chapter_number == chapter_number {
                return Some(c.chapter_directive.goal);
            }
        }
    }
    None
}

/// 事务内执行一刀切分（与 V127 修复迁移共用；调用方负责提交/回滚）：
///
/// 0. 多场景守卫（I-3）：章关联 scene 数 > 1 时告警并返回 `Ok(None)`
///    （不截断、不新建）——章内容是全场景拼接，截断只写首场景会造成 正文重复；
/// 1. 重排：`chapter_number > N` 的章逐行 +1（按章号降序，避开
///    `UNIQUE(story_id, chapter_number)` 冲突），并同步重排 story 内
///    `sequence_number > N` 的 scenes（同样降序）；
/// 2. 合约跟随内容（I-2）：`chapter_number > N` 的 CHAPTER 合约随其章
///    一并顺延——读出 contract_json、把 JSON 内的 `chapter_number` 字段 +1
///    后写回（按章号降序逐条处理，保持顺序清晰）；
/// 3. 新章插入 `N + 1`（重排后必无冲突），溢出内容写入其新建场景。 新溢出章 N+1
///    必无合约（> N 的合约已随内容顺延），标题经 `chapter_contract_goal_in_tx`
///    查询落空后回退 `第{N+1}章`；
/// 4. 截断原章首场景为 `plan.keep`，更新原章 `word_count`。
///
/// 先重排/插入再写截断，且全部在同一事务内：任何一步失败整体回滚，
/// 不留半切分状态。返回 `Some((新章 id, 被重排的旧章 id 列表, 新章标题))`；
/// 多场景章跳过时返回 `None`。
pub(crate) fn split_chapter_in_tx(
    tx: &rusqlite::Transaction,
    story_id: &str,
    chapter_id: &str,
    chapter_number: i32,
    plan: &ChapterSplitPlan,
    first_scene_id: &str,
) -> Result<Option<(String, Vec<String>, String)>, rusqlite::Error> {
    let now = Local::now().to_rfc3339();
    let new_number = chapter_number + 1;

    // 0. 多场景守卫：章关联 scene 数 > 1 时跳过（I-3）
    let scene_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM scenes WHERE chapter_id = ?1",
        [chapter_id],
        |r| r.get(0),
    )?;
    if scene_count > 1 {
        log::warn!(
            "[ChapterSplitter] chapter {} 关联 {} 个场景，多场景章不支持切分，跳过",
            chapter_id,
            scene_count
        );
        return Ok(None);
    }

    // 1. 重排后续章（降序逐行 +1）
    let renumbered_chapter_ids: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT id FROM chapters WHERE story_id = ?1 AND chapter_number > ?2 \
             ORDER BY chapter_number DESC",
        )?;
        let rows = stmt.query_map(params![story_id, chapter_number], |r| r.get(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        out
    };
    for id in &renumbered_chapter_ids {
        tx.execute(
            "UPDATE chapters SET chapter_number = chapter_number + 1, updated_at = ?2 \
             WHERE id = ?1",
            params![id, now],
        )?;
    }
    // 派生标题跟随新章号，否则列表会出现「第7章」后面还挂一个「第6章」。
    // 手写标题（临江夜雨）随内容走，不改。
    for id in &renumbered_chapter_ids {
        let (num, title): (i32, Option<String>) = tx.query_row(
            "SELECT chapter_number, title FROM chapters WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if is_generic_chapter_title(title.as_deref().unwrap_or("")) {
            let new_title = canonical_chapter_title(num);
            tx.execute(
                "UPDATE chapters SET title = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, &new_title, now],
            )?;
            tx.execute(
                "UPDATE scenes SET title = ?2, updated_at = ?3 WHERE chapter_id = ?1",
                params![id, &new_title, now],
            )?;
        }
    }
    // 同步重排 scenes.sequence_number（本库其与章号对齐；按 story 范围
    // 降序逐行 +1，避开 UNIQUE(story_id, sequence_number) 冲突）
    let renumbered_scene_ids: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT id FROM scenes WHERE story_id = ?1 AND sequence_number > ?2 \
             ORDER BY sequence_number DESC",
        )?;
        let rows = stmt.query_map(params![story_id, chapter_number], |r| r.get(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        out
    };
    for id in &renumbered_scene_ids {
        tx.execute(
            "UPDATE scenes SET sequence_number = sequence_number + 1, updated_at = ?2 \
             WHERE id = ?1",
            params![id, now],
        )?;
    }

    // 2. 合约跟随内容（I-2）：chapter_number > N 的 CHAPTER 合约随其章 顺延
    //    +1。chapter_number 存于 contract_json 内部而非独立列， 需读出 → 改字段
    //    → 写回；解析失败的合约行不动。
    let shifted_contracts: Vec<(String, ChapterContract)> = {
        let mut stmt = tx.prepare(
            "SELECT id, contract_json FROM story_contracts \
             WHERE story_id = ?1 AND contract_type = 'CHAPTER'",
        )?;
        let rows = stmt.query_map(params![story_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, json) = row?;
            if let Ok(contract) = serde_json::from_str::<ChapterContract>(&json) {
                if contract.chapter_number > chapter_number {
                    out.push((id, contract));
                }
            }
        }
        // JSON 无唯一约束，降序仅保持处理顺序与章重排一致、清晰可读
        out.sort_by_key(|(_, c)| std::cmp::Reverse(c.chapter_number));
        out
    };
    for (id, mut contract) in shifted_contracts {
        contract.chapter_number += 1;
        let json = serde_json::to_string(&contract)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        tx.execute(
            "UPDATE story_contracts SET contract_json = ?2, version = version + 1, \
             updated_at = ?3 WHERE id = ?1",
            params![id, json, now],
        )?;
    }

    // 3. 新章插入 N+1（内容写入其场景，与 ChapterRepository::create 同构）。
    //    标题：重排后 N+1 必无合约（合约跟随内容），自然回退 `第{N+1}章`。
    let new_title = title_from_goal(
        chapter_contract_goal_in_tx(tx, story_id, new_number).as_deref(),
        new_number,
    );
    let new_chapter_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO chapters (id, story_id, chapter_number, title, outline, word_count, \
         model_used, cost, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, '', 0.0, ?6, ?6)",
        params![
            &new_chapter_id,
            story_id,
            new_number,
            &new_title,
            TextUtils::chinese_word_count(&plan.overflow) as i32,
            now
        ],
    )?;
    let new_scene_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO scenes (id, story_id, sequence_number, title, content, \
         characters_present, character_conflicts, execution_stage, chapter_id, \
         created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, '[]', '[]', 'drafting', ?6, ?7, ?7)",
        params![
            &new_scene_id,
            story_id,
            new_number,
            &new_title,
            plan.overflow,
            &new_chapter_id,
            now
        ],
    )?;

    // 4. 截断原章首场景 + 更新原章字数
    let keep_wc = TextUtils::chinese_word_count(&plan.keep) as i32;
    tx.execute(
        "UPDATE scenes SET content = ?2, updated_at = ?3 WHERE id = ?1",
        params![first_scene_id, plan.keep, now],
    )?;
    tx.execute(
        "UPDATE chapters SET word_count = ?2, updated_at = ?3 WHERE id = ?1",
        params![chapter_id, keep_wc, now],
    )?;

    Ok(Some((new_chapter_id, renumbered_chapter_ids, new_title)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chinese_repeat(ch: char, n: usize) -> String {
        std::iter::repeat(ch).take(n).collect()
    }

    #[test]
    fn mode_parse_defaults_to_word_count() {
        assert_eq!(
            ChapterSplitMode::parse("word_count"),
            ChapterSplitMode::WordCount
        );
        assert_eq!(ChapterSplitMode::parse("plot"), ChapterSplitMode::Plot);
        assert_eq!(ChapterSplitMode::parse("情节"), ChapterSplitMode::Plot);
        assert_eq!(ChapterSplitMode::parse(""), ChapterSplitMode::WordCount);
        assert_eq!(
            ChapterSplitMode::parse("weird"),
            ChapterSplitMode::WordCount
        );
    }

    #[test]
    fn word_count_below_threshold_no_split() {
        let text = chinese_repeat('测', 100);
        assert!(plan_split(&text, ChapterSplitMode::WordCount, 3000).is_none());
    }

    #[test]
    fn word_count_over_threshold_splits_at_paragraph() {
        let para1 = chinese_repeat('甲', 2000);
        let para2 = chinese_repeat('乙', 2000);
        let text = format!("{}\n\n{}", para1, para2);
        let plan = plan_split(&text, ChapterSplitMode::WordCount, 3000).expect("should split");
        assert!(plan.keep.contains('甲'));
        assert!(plan.overflow.contains('乙'));
        assert!(TextUtils::chinese_word_count(&plan.keep) <= 3000);
        assert!(!plan.overflow.is_empty());
    }

    #[test]
    fn plot_mode_prefers_transition_marker() {
        let before = chinese_repeat('前', 800);
        let after = chinese_repeat('后', 800);
        let text = format!("{}\n\n第二天\n{}", before, after);
        let plan = plan_split(&text, ChapterSplitMode::Plot, 1000).expect("plot split");
        assert!(
            plan.overflow.starts_with("第二天"),
            "overflow should start at plot boundary, got prefix: {:?}",
            plan.overflow.chars().take(12).collect::<String>()
        );
        assert!(plan.keep.contains('前'));
    }

    #[test]
    fn plot_mode_does_not_use_word_threshold_incorrectly_when_under() {
        // 情节模式：总字数未超阈值时不切
        let text = format!(
            "{}\n\n第二天\n{}",
            chinese_repeat('前', 100),
            chinese_repeat('后', 100)
        );
        assert!(plan_split(&text, ChapterSplitMode::Plot, 3000).is_none());
    }

    #[test]
    fn word_count_mode_ignores_plot_markers_when_under_threshold() {
        let text = format!("开头\n\n第二天\n{}", chinese_repeat('续', 50));
        assert!(plan_split(&text, ChapterSplitMode::WordCount, 3000).is_none());
    }

    #[test]
    fn title_from_goal_uses_goal_when_present() {
        assert_eq!(
            title_from_goal(Some("潜入敌营救出同伴"), 3),
            "潜入敌营救出同伴"
        );
    }

    #[test]
    fn title_from_goal_truncates_at_char_boundary() {
        let long_goal = chinese_repeat('长', 40);
        let title = title_from_goal(Some(&long_goal), 3);
        assert_eq!(title.chars().count(), 30);
    }

    #[test]
    fn title_from_goal_falls_back_when_missing_or_blank() {
        assert_eq!(title_from_goal(None, 7), "第7章");
        assert_eq!(title_from_goal(Some("   "), 7), "第7章");
    }

    #[test]
    fn generic_chapter_title_matches_arabic_and_chinese_numerals() {
        assert!(is_generic_chapter_title("第6章"));
        assert!(is_generic_chapter_title("第一章"));
        assert!(is_generic_chapter_title(" 第12章 "));
        assert!(!is_generic_chapter_title("临江夜雨"));
        assert!(!is_generic_chapter_title("第一章 开端"));
        assert!(!is_generic_chapter_title(""));
    }
}
