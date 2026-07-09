//! 自动划分章节（v0.26.57）
//!
//! 设置驱动：
//! - `chapter_split_mode =
//!   "word_count"`：当前章字数超过阈值时，在段落边界切出下一章
//! - `chapter_split_mode = "plot"`：在情节/场景转换点切章（启发式，无额外 LLM）
//!
//! 字数单位：中文「字」（`TextUtils::chinese_word_count`）。
//! 字数上限留空 → 使用 `DEFAULT_CHAPTER_SPLIT_MAX_CHARS`（3000）。
//!
//! 触发：场景内容保存后 30s 空闲（与 auto_commit
//! 同窗口），且仅处理故事最新一章， 避免改写中间章节时重排后续章号。

use tauri::AppHandle;

use crate::{
    config::{AppConfig, DEFAULT_CHAPTER_SPLIT_MAX_CHARS},
    db::{ChapterRepository, CreateChapterRequest, DbPool, SceneRepository, SceneUpdate},
    state_sync::StateSync,
    utils::text::TextUtils,
};

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

/// 解析有效字数上限：`None` / `0` / 负数 → 自动默认 3000 字。
pub fn resolve_max_chars(configured: Option<i32>) -> usize {
    match configured {
        Some(n) if n > 0 => n as usize,
        _ => DEFAULT_CHAPTER_SPLIT_MAX_CHARS,
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
    // 选第一个落在 [min_byte, soft_end*1.5] 的边界；否则选 min_byte 之后最近边界
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

/// 对故事最新一章执行一次自动划分（若需要）。
///
/// 返回 `Ok(Some(new_chapter_id))` 表示已切出新章；`Ok(None)` 表示无需切分。
pub fn maybe_split_latest_chapter(
    pool: &DbPool,
    app_handle: &AppHandle,
    story_id: &str,
    chapter_id: &str,
    config: &AppConfig,
) -> Result<Option<String>, String> {
    let mode = ChapterSplitMode::parse(&config.chapter_split_mode);
    let max_chars = resolve_max_chars(config.chapter_split_max_chars);

    let chapter_repo = ChapterRepository::new(pool.clone());
    let scene_repo = SceneRepository::new(pool.clone());

    let chapters = chapter_repo
        .get_by_story(story_id)
        .map_err(|e| e.to_string())?;
    let Some(latest) = chapters.iter().max_by_key(|c| c.chapter_number) else {
        return Ok(None);
    };
    // 仅最新章可自动切分，避免中间章改写触发重排
    if latest.id != chapter_id {
        return Ok(None);
    }

    let content = chapter_repo
        .get_content(chapter_id)
        .map_err(|e| e.to_string())?;
    let Some(plan) = plan_split(&content, mode, max_chars) else {
        return Ok(None);
    };

    let scenes = scene_repo
        .get_by_chapter(chapter_id)
        .map_err(|e| e.to_string())?;
    let Some(scene) = scenes.first() else {
        return Ok(None);
    };

    let keep_wc = TextUtils::chinese_word_count(&plan.keep) as i32;
    scene_repo
        .update(
            &scene.id,
            &SceneUpdate {
                content: Some(plan.keep.clone()),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
    chapter_repo
        .update(chapter_id, None, None, Some(keep_wc))
        .map_err(|e| e.to_string())?;

    let next_number = latest.chapter_number + 1;
    // 若下一章号已存在则放弃（并发/手工建章）
    if chapters.iter().any(|c| c.chapter_number == next_number) {
        log::warn!(
            "[ChapterSplitter] next chapter {} already exists for story {}, abort split",
            next_number,
            story_id
        );
        return Ok(None);
    }

    let new_chapter = chapter_repo
        .create(CreateChapterRequest {
            story_id: story_id.to_string(),
            chapter_number: next_number,
            title: Some(format!("第{}章", next_number)),
            outline: None,
            content: Some(plan.overflow),
        })
        .map_err(|e| e.to_string())?;

    let _ = StateSync::emit_chapter_updated(app_handle, chapter_id, None, story_id);
    let _ = StateSync::emit_scene_updated(app_handle, story_id, &scene.id, None, true);
    let _ = StateSync::emit_chapter_created(
        app_handle,
        story_id,
        &new_chapter.id,
        new_chapter.title.as_deref(),
    );

    log::info!(
        "[ChapterSplitter] split chapter {} → new {} (mode={:?}, max_chars={})",
        chapter_id,
        new_chapter.id,
        mode,
        max_chars
    );

    Ok(Some(new_chapter.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chinese_repeat(ch: char, n: usize) -> String {
        std::iter::repeat(ch).take(n).collect()
    }

    #[test]
    fn resolve_max_chars_empty_uses_default() {
        assert_eq!(resolve_max_chars(None), DEFAULT_CHAPTER_SPLIT_MAX_CHARS);
        assert_eq!(resolve_max_chars(Some(0)), DEFAULT_CHAPTER_SPLIT_MAX_CHARS);
        assert_eq!(resolve_max_chars(Some(-1)), DEFAULT_CHAPTER_SPLIT_MAX_CHARS);
        assert_eq!(resolve_max_chars(Some(2500)), 2500);
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
}
