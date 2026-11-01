//! 章节自动取名。
//!
//! 续写/创世落库时章节标题通常只是「第N章」占位。本模块用一次很短的
//! Background 档 LLM 调用，从本章大纲与正文首尾提炼一个可读标题，并只在
//! 当前标题仍是占位/空标题时写回；作者手写的标题永不被自动覆盖。
//!
//! 设计约束：
//! - 纯 best-effort：失败/超时/解析不出都保留原占位标题，不影响正文落库。
//! - LLM 输入只取章节大纲 + 正文首尾片段，避免大章把提示词撑爆。
//! - 写回同时更新 `chapters.title` 与关联 `scenes.title`（标题是共享元数据）。
//! - 只在测试环境之外由 coordinator 触发（`for_test` 无 app_handle），避免 既有
//!   MockLlm 队列被额外调用打乱。

use std::{collections::HashMap, time::Duration};

use chrono::Local;
use rusqlite::{params, OptionalExtension};
use serde_json::Value;

use crate::{
    agency::tool_loop::LoopLlm,
    db::{chapter_split::is_generic_chapter_title, DbPool},
    error::AppError,
    router::TaskType,
};

const MAX_TITLE_CHARS: usize = 20;
const MIN_TITLE_CHARS: usize = 2;
const MIN_CONTENT_CHARS: usize = 200;
const PROMPT_HEAD_CHARS: usize = 1600;
const PROMPT_TAIL_CHARS: usize = 600;
const PROMPT_OUTLINE_CHARS: usize = 1200;
const EXISTING_TITLE_LIMIT: usize = 20;
const TITLE_LLM_TIMEOUT: Duration = Duration::from_secs(60);

const TITLE_SYSTEM_PROMPT: &str =
    "你是一位资深中文小说编辑。你只输出一个章节标题，不解释、不寒暄、不加引号。";

/// `(id, story_id, chapter_number, title, outline)`
type ChapterRow = (String, String, i32, Option<String>, Option<String>);

/// 生成标题所需的章节上下文。
#[derive(Debug, Clone)]
pub struct ChapterTitleContext {
    pub chapter_id: String,
    pub scene_id: Option<String>,
    pub story_id: String,
    pub chapter_number: i32,
    pub story_title: String,
    pub current_title: Option<String>,
    pub outline: String,
    pub content: String,
    /// 同故事其他章节已用的可读标题，用于避免重名。
    pub existing_titles: Vec<String>,
}

impl ChapterTitleContext {
    fn is_placeholder_title(&self) -> bool {
        self.current_title
            .as_deref()
            .map(|t| t.trim().is_empty() || is_generic_chapter_title(t))
            .unwrap_or(true)
    }
}

/// 按章节 id 载入命名上下文（取该章全部 Scene 的正文与大纲）。
pub fn load_by_chapter(
    pool: &DbPool,
    chapter_id: &str,
) -> Result<Option<ChapterTitleContext>, AppError> {
    let conn = pool
        .get()
        .map_err(|e| AppError::from(format!("pool: {}", e)))?;

    let chapter: Option<ChapterRow> = conn
        .query_row(
            "SELECT id, story_id, chapter_number, title, outline FROM chapters WHERE id = ?1",
            [chapter_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    let Some((chapter_id, story_id, chapter_number, current_title, chapter_outline)) = chapter
    else {
        return Ok(None);
    };

    let mut stmt = conn.prepare(
        "SELECT id, COALESCE(content, ''), COALESCE(outline_content, '') \
         FROM scenes WHERE chapter_id = ?1 ORDER BY sequence_number",
    )?;
    let scenes: Vec<(String, String, String)> = stmt
        .query_map([&chapter_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let scene_id = scenes.first().map(|s| s.0.clone());
    let content = plain_text(
        &scenes
            .iter()
            .map(|s| s.1.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
    );

    let mut outline_parts = Vec::new();
    if let Some(o) = chapter_outline.filter(|s| !s.trim().is_empty()) {
        outline_parts.push(o);
    }
    for (_, _, scene_outline) in &scenes {
        if !scene_outline.trim().is_empty() && !outline_parts.iter().any(|o| o == scene_outline) {
            outline_parts.push(scene_outline.clone());
        }
    }
    let outline = outline_parts.join("\n\n");

    let story_title: Option<String> = conn
        .query_row(
            "SELECT title FROM stories WHERE id = ?1",
            [&story_id],
            |row| row.get(0),
        )
        .optional()?;

    let existing_titles = load_existing_titles(&conn, &story_id, &chapter_id)?;

    Ok(Some(ChapterTitleContext {
        chapter_id,
        scene_id,
        story_id,
        chapter_number,
        story_title: story_title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "未命名".to_string()),
        current_title,
        outline,
        content,
        existing_titles,
    }))
}

/// 按场景 id 载入命名上下文；场景未关联章节时按故事+序号回退查找章节。
pub fn load_by_scene(
    pool: &DbPool,
    scene_id: &str,
) -> Result<Option<ChapterTitleContext>, AppError> {
    let (story_id, sequence_number, chapter_id) = {
        let conn = pool
            .get()
            .map_err(|e| AppError::from(format!("pool: {}", e)))?;
        let row: Option<(String, i32, Option<String>)> = conn
            .query_row(
                "SELECT story_id, sequence_number, chapter_id FROM scenes WHERE id = ?1",
                [scene_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        match row {
            Some(row) => row,
            None => return Ok(None),
        }
    };

    let chapter_id = match chapter_id {
        Some(id) if !id.trim().is_empty() => id,
        _ => {
            let conn = pool
                .get()
                .map_err(|e| AppError::from(format!("pool: {}", e)))?;
            let id: Option<String> = conn
                .query_row(
                    "SELECT id FROM chapters WHERE story_id = ?1 AND chapter_number = ?2",
                    params![&story_id, sequence_number],
                    |row| row.get(0),
                )
                .optional()?;
            match id {
                Some(id) => id,
                None => return Ok(None),
            }
        }
    };

    let mut ctx = match load_by_chapter(pool, &chapter_id)? {
        Some(ctx) => ctx,
        None => return Ok(None),
    };
    ctx.scene_id = Some(scene_id.to_string());
    Ok(Some(ctx))
}

fn load_existing_titles(
    conn: &rusqlite::Connection,
    story_id: &str,
    current_chapter_id: &str,
) -> Result<Vec<String>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT title FROM chapters \
         WHERE story_id = ?1 AND id <> ?2 AND title IS NOT NULL \
         ORDER BY chapter_number DESC LIMIT 40",
    )?;
    let titles = stmt
        .query_map(params![story_id, current_chapter_id], |row| {
            row.get::<_, Option<String>>(0)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    for title in titles.into_iter().flatten() {
        let t = title.trim();
        if t.is_empty() || is_generic_chapter_title(t) {
            continue;
        }
        if !out.iter().any(|x: &String| x == t) {
            out.push(t.to_string());
        }
        if out.len() >= EXISTING_TITLE_LIMIT {
            break;
        }
    }
    Ok(out)
}

/// 自动命名只处理占位标题；作者手写标题默认不覆盖。
pub fn should_auto_name(ctx: &ChapterTitleContext, overwrite: bool) -> bool {
    if ctx.content.trim().chars().count() < MIN_CONTENT_CHARS && ctx.outline.trim().is_empty() {
        return false;
    }
    overwrite || ctx.is_placeholder_title()
}

/// 渲染命名提示词（优先用户覆盖的 `chapter_title`，缺失时用内置默认）。
pub fn render_title_prompt(pool: &DbPool, ctx: &ChapterTitleContext) -> String {
    let mut vars = HashMap::new();
    vars.insert("story_title".to_string(), ctx.story_title.clone());
    vars.insert("chapter_number".to_string(), ctx.chapter_number.to_string());
    vars.insert(
        "chapter_outline".to_string(),
        if ctx.outline.trim().is_empty() {
            "（无）".to_string()
        } else {
            excerpt_head(&ctx.outline, PROMPT_OUTLINE_CHARS)
        },
    );
    vars.insert(
        "chapter_beginning".to_string(),
        if ctx.content.trim().is_empty() {
            "（无正文）".to_string()
        } else {
            excerpt_head(&ctx.content, PROMPT_HEAD_CHARS)
        },
    );
    vars.insert(
        "chapter_ending".to_string(),
        if ctx.content.trim().is_empty() {
            "（无正文）".to_string()
        } else {
            excerpt_tail(&ctx.content, PROMPT_TAIL_CHARS)
        },
    );
    vars.insert(
        "existing_titles".to_string(),
        if ctx.existing_titles.is_empty() {
            "（无）".to_string()
        } else {
            ctx.existing_titles.join("、")
        },
    );

    crate::prompts::registry::resolve_prompt_with_vars(pool, "chapter_title", &vars)
        .ok()
        .or_else(|| {
            crate::prompts::registry::resolve_prompt_default_with_vars("chapter_title", &vars)
        })
        .unwrap_or_else(|| inline_title_prompt(ctx))
}

/// 调一次 Background 档模型。任何失败/超时都返回 None，由调用方决定回退。
pub async fn generate_title(llm: &dyn LoopLlm, prompt: &str) -> Option<String> {
    match tokio::time::timeout(
        TITLE_LLM_TIMEOUT,
        llm.complete(TITLE_SYSTEM_PROMPT, prompt, TaskType::Summarization, 256),
    )
    .await
    {
        Ok(Ok(raw)) => sanitize_generated_title(&raw),
        Ok(Err(e)) => {
            log::warn!("[chapter-title] LLM 取名失败: {e}");
            None
        }
        Err(_) => {
            log::warn!("[chapter-title] LLM 取名超时");
            None
        }
    }
}

/// 清洗模型输出：支持纯文本、带「标题：」前缀、JSON、markdown 围栏。
pub fn sanitize_generated_title(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    let unfenced = raw
        .trim_start_matches("```json")
        .trim_start_matches("```JSON")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(unfenced) {
        if let Some(title) = obj.get("title").and_then(Value::as_str) {
            if let Some(title) = sanitize_title_line(title) {
                return Some(title);
            }
        }
    }

    raw.lines().find_map(sanitize_title_line)
}

fn sanitize_title_line(line: &str) -> Option<String> {
    let mut s: String = line
        .trim_start_matches(|c: char| c.is_whitespace() || "#>*-•·".contains(c))
        .trim()
        .to_string();
    if s.is_empty() {
        return None;
    }
    s = strip_leading_number(&s).to_string();
    s = strip_title_label(&s).to_string();
    s = strip_wrapping_quotes(&s).trim().to_string();
    if let Some(rest) = strip_chapter_prefix(&s) {
        s = rest;
    }
    s = strip_wrapping_quotes(&s).trim().to_string();
    s = s
        .trim_end_matches(|c: char| "。！？!?；;，,、：:".contains(c))
        .trim()
        .to_string();

    let lower = s.to_lowercase();
    if s.chars().count() < MIN_TITLE_CHARS
        || s.contains('\n')
        || is_generic_chapter_title(&s)
        || !s.chars().any(|c| c.is_alphanumeric())
        || ["抱歉", "无法", "以下", "好的", "当然", "标题如下", "作为"]
            .iter()
            .any(|p| lower.starts_with(p))
    {
        return None;
    }

    let mut title: String = s.chars().take(MAX_TITLE_CHARS).collect();
    title = title
        .trim_end_matches(|c: char| "。！？!?；;，,、：:".contains(c))
        .trim()
        .to_string();
    title = strip_wrapping_quotes(&title).trim().to_string();
    if title.chars().count() < MIN_TITLE_CHARS || is_generic_chapter_title(&title) {
        return None;
    }
    Some(title)
}

/// 从章纲的关键事件/戏剧目标中抽一个短标题，作为 LLM 失败时的确定性回退。
pub fn fallback_title_from_outline(outline: &str) -> Option<String> {
    const KEYS: &[&str] = &["关键事件", "戏剧目标", "核心事件", "本章事件", "剧情节拍"];
    for line in outline.lines() {
        let line = line.trim();
        if line.is_empty() || !KEYS.iter().any(|k| line.contains(k)) {
            continue;
        }
        let body = line
            .split_once(['：', ':'])
            .map(|(_, right)| right)
            .unwrap_or(line);
        for clause in body.split(['；', ';', '。', '|', '\n']) {
            let candidate = clause.split(['，', ',']).next().unwrap_or(clause);
            let candidate = candidate.trim_start_matches(|c: char| {
                c.is_whitespace() || "#>*-•·0123456789.、)）".contains(c)
            });
            if let Some(title) = sanitize_generated_title(candidate) {
                if title.chars().count() <= 16 {
                    return Some(title);
                }
            }
        }
    }
    None
}

/// 标题是否可用于本章（非空、非占位、不与故事名或其他章重名）。
pub fn is_acceptable_title(ctx: &ChapterTitleContext, title: &str) -> bool {
    let title = title.trim();
    title.chars().count() >= MIN_TITLE_CHARS
        && !is_generic_chapter_title(title)
        && title != ctx.story_title.trim()
        && !ctx.existing_titles.iter().any(|t| t.trim() == title)
}

/// 写回标题。`overwrite=false` 时只替换空/占位标题（防作者在 LLM 调用
/// 期间手改标题被覆盖）。返回是否真的写了。
pub fn apply_title(
    pool: &DbPool,
    ctx: &ChapterTitleContext,
    title: &str,
    overwrite: bool,
) -> Result<bool, AppError> {
    let title = title.trim();
    if title.is_empty() {
        return Ok(false);
    }
    let mut conn = pool
        .get()
        .map_err(|e| AppError::from(format!("pool: {}", e)))?;
    let tx = conn.transaction()?;

    let current: Option<Option<String>> = tx
        .query_row(
            "SELECT title FROM chapters WHERE id = ?1",
            [&ctx.chapter_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(current) = current else {
        return Ok(false);
    };
    let still_placeholder = current
        .as_deref()
        .map(|t| t.trim().is_empty() || is_generic_chapter_title(t))
        .unwrap_or(true);
    if !overwrite && !still_placeholder {
        return Ok(false);
    }

    let now = Local::now().to_rfc3339();
    tx.execute(
        "UPDATE chapters SET title = ?2, updated_at = ?3 WHERE id = ?1",
        params![&ctx.chapter_id, title, &now],
    )?;
    tx.execute(
        "UPDATE scenes SET title = ?2, updated_at = ?3 WHERE chapter_id = ?1",
        params![&ctx.chapter_id, title, &now],
    )?;
    tx.commit()?;
    Ok(true)
}

/// 幕前保存的正文可能是编辑器 HTML；取名只喂纯文本。
fn plain_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn excerpt_head(text: &str, max_chars: usize) -> String {
    let mut out: String = text.trim().chars().take(max_chars).collect();
    if text.trim().chars().count() > max_chars {
        out.push('…');
    }
    out
}

fn excerpt_tail(text: &str, max_chars: usize) -> String {
    let text = text.trim();
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    let skipped = total - max_chars;
    format!("…{}", text.chars().skip(skipped).collect::<String>())
}

fn strip_leading_number(s: &str) -> &str {
    let mut chars = s.char_indices();
    let Some((_, first)) = chars.next() else {
        return s;
    };
    if !first.is_ascii_digit() {
        return s;
    }
    let mut end = first.len_utf8();
    for (idx, c) in chars {
        if c.is_ascii_digit() {
            end = idx + c.len_utf8();
            continue;
        }
        if matches!(c, '.' | '、' | ')' | '）') {
            return s[end + c.len_utf8()..].trim_start();
        }
        break;
    }
    s
}

fn strip_title_label(s: &str) -> &str {
    for label in ["章节标题", "标题", "章节名", "章名"] {
        for sep in ["：", ":", " "] {
            if let Some(rest) = s.strip_prefix(&format!("{label}{sep}")) {
                return rest.trim();
            }
        }
    }
    s
}

fn strip_wrapping_quotes(s: &str) -> &str {
    const PAIRS: &[(char, char)] = &[
        ('"', '"'),
        ('\'', '\''),
        ('“', '”'),
        ('‘', '’'),
        ('「', '」'),
        ('『', '』'),
        ('《', '》'),
    ];
    if s.chars().count() < 2 {
        return s;
    }
    let first = s.chars().next();
    let last = s.chars().last();
    for (open, close) in PAIRS {
        if first == Some(*open) && last == Some(*close) {
            let start = open.len_utf8();
            let end = s.len() - close.len_utf8();
            if start <= end {
                return &s[start..end];
            }
        }
    }
    s
}

/// 仅当「第N章」后紧跟分隔符时剥掉章号前缀；「第三章的雨」这类真实标题
/// 不剥。
fn strip_chapter_prefix(s: &str) -> Option<String> {
    let mut chars = s.char_indices();
    let (_, first) = chars.next()?;
    if first != '第' {
        return None;
    }
    let mut chapter_end = None;
    for (idx, c) in chars {
        if c == '章' {
            chapter_end = Some(idx);
            break;
        }
        if !(c.is_ascii_digit() || "一二三四五六七八九十百千零〇两".contains(c)) {
            return None;
        }
    }
    let end = chapter_end?;
    let rest = &s[end + '章'.len_utf8()..];
    let first = rest.chars().next()?;
    if !matches!(
        first,
        ' ' | '　' | '：' | ':' | '·' | '、' | '-' | '—' | '－' | '）' | ')' | '】' | ']'
    ) {
        return None;
    }
    Some(
        rest.trim_start_matches(|c: char| {
            c.is_whitespace()
                || matches!(
                    c,
                    '：' | ':' | '·' | '、' | '-' | '—' | '－' | '）' | ')' | '】' | ']'
                )
        })
        .trim()
        .to_string(),
    )
}

fn inline_title_prompt(ctx: &ChapterTitleContext) -> String {
    format!(
        "你是一位资深中文小说编辑，只负责为下面这一章拟一个标题。\n\
         【要求】4-14 个汉字；具体、有画面感；禁止以“第N章”开头；\
         禁止书名号、引号、冒号解释、markdown；只输出标题本身。\n\n\
         【作品】《{}》\n【章节】第{}章\n\n【本章大纲】\n{}\n\n\
         【本章开头】\n{}\n\n【本章结尾】\n{}\n\n\
         【已有章节标题（避免重复）】\n{}",
        ctx.story_title,
        ctx.chapter_number,
        if ctx.outline.trim().is_empty() {
            "（无）".to_string()
        } else {
            excerpt_head(&ctx.outline, PROMPT_OUTLINE_CHARS)
        },
        if ctx.content.trim().is_empty() {
            "（无正文）".to_string()
        } else {
            excerpt_head(&ctx.content, PROMPT_HEAD_CHARS)
        },
        if ctx.content.trim().is_empty() {
            "（无正文）".to_string()
        } else {
            excerpt_tail(&ctx.content, PROMPT_TAIL_CHARS)
        },
        if ctx.existing_titles.is_empty() {
            "（无）".to_string()
        } else {
            ctx.existing_titles.join("、")
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        create_test_pool, ChapterRepository, CreateChapterRequest, CreateStoryRequest,
        SceneRepository, StoryRepository,
    };

    fn setup_chapter(
        title: Option<&str>,
        outline: Option<&str>,
        content: Option<&str>,
    ) -> (DbPool, ChapterTitleContext) {
        let pool = create_test_pool().unwrap();
        let story = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "雨夜归人".to_string(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let chapter = ChapterRepository::new(pool.clone())
            .create(CreateChapterRequest {
                story_id: story.id.clone(),
                chapter_number: 2,
                title: title.map(str::to_string),
                outline: outline.map(str::to_string),
                content: content.map(str::to_string),
            })
            .unwrap();
        let ctx = load_by_chapter(&pool, &chapter.id)
            .unwrap()
            .expect("chapter context");
        (pool, ctx)
    }

    #[test]
    fn sanitize_strips_labels_quotes_and_chapter_prefix() {
        assert_eq!(
            sanitize_generated_title("标题：临江夜雨"),
            Some("临江夜雨".to_string())
        );
        assert_eq!(
            sanitize_generated_title("第3章：临江夜雨"),
            Some("临江夜雨".to_string())
        );
        assert_eq!(
            sanitize_generated_title("《雨夜来客》"),
            Some("雨夜来客".to_string())
        );
        assert_eq!(
            sanitize_generated_title("```json\n{\"title\":\"灯下故人\"}\n```"),
            Some("灯下故人".to_string())
        );
    }

    #[test]
    fn sanitize_rejects_placeholder_and_meta() {
        assert_eq!(sanitize_generated_title("第3章"), None);
        assert_eq!(sanitize_generated_title("第三章"), None);
        assert_eq!(sanitize_generated_title("  "), None);
        assert_eq!(sanitize_generated_title("好的，标题如下："), None);
        assert_eq!(sanitize_generated_title("抱歉，我无法完成"), None);
    }

    #[test]
    fn sanitize_keeps_real_title_starting_with_chapter_word() {
        assert_eq!(
            sanitize_generated_title("第三章的雨"),
            Some("第三章的雨".to_string())
        );
    }

    #[test]
    fn plain_text_strips_editor_html() {
        let text = plain_text("<p>雨声敲窗。&nbsp;</p><p><strong>阿岩</strong>握紧刀柄。</p>");
        assert!(!text.contains('<'));
        assert!(!text.contains("&nbsp;"));
        assert!(text.contains("雨声敲窗"));
        assert!(text.contains("阿岩"));
        assert!(text.contains("握紧刀柄"));
    }

    #[test]
    fn fallback_extracts_first_key_event_clause() {
        let outline = "1. **关键事件**：阿岩潜入敌营；薇拉在门外接应\n2. 出场角色：阿岩、薇拉";
        assert_eq!(
            fallback_title_from_outline(outline),
            Some("阿岩潜入敌营".to_string())
        );
    }

    #[test]
    fn apply_only_overwrites_placeholder_unless_forced() {
        let (pool, ctx) = setup_chapter(
            Some("第2章"),
            Some("关键事件：雨夜叩门"),
            Some("雨声敲窗。"),
        );
        assert!(apply_title(&pool, &ctx, "雨夜叩门", false).unwrap());
        let chapter = ChapterRepository::new(pool.clone())
            .get_by_id(&ctx.chapter_id)
            .unwrap()
            .unwrap();
        assert_eq!(chapter.title.as_deref(), Some("雨夜叩门"));

        let scene = SceneRepository::new(pool.clone())
            .get_by_chapter(&ctx.chapter_id)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(scene.title.as_deref(), Some("雨夜叩门"));

        let mut ctx_after = load_by_chapter(&pool, &ctx.chapter_id).unwrap().unwrap();
        ctx_after.story_title = "雨夜归人".to_string();
        assert!(!apply_title(&pool, &ctx_after, "旧名", false).unwrap());
        let chapter = ChapterRepository::new(pool.clone())
            .get_by_id(&ctx.chapter_id)
            .unwrap()
            .unwrap();
        assert_eq!(chapter.title.as_deref(), Some("雨夜叩门"));
        assert!(apply_title(&pool, &ctx_after, "旧名", true).unwrap());
    }

    #[test]
    fn render_prompt_uses_registered_template_and_excerpts() {
        let (pool, ctx) = setup_chapter(
            Some("第2章"),
            Some("关键事件：雨夜叩门"),
            Some("雨声敲窗。阿岩握紧刀柄。"),
        );
        assert!(
            crate::prompts::registry::resolve_prompt_default("chapter_title").is_some(),
            "chapter_title 提示词必须注册"
        );
        let prompt = render_title_prompt(&pool, &ctx);
        assert!(prompt.contains("《雨夜归人》"));
        assert!(prompt.contains("第2章"));
        assert!(prompt.contains("关键事件"));
        assert!(prompt.contains("阿岩握紧刀柄"));
    }

    struct StaticLlm {
        reply: String,
    }

    #[async_trait::async_trait]
    impl LoopLlm for StaticLlm {
        async fn complete(
            &self,
            _system: &str,
            _user: &str,
            _task: TaskType,
            _max_tokens: i32,
        ) -> Result<String, AppError> {
            Ok(self.reply.clone())
        }
    }

    struct FailingLlm;

    #[async_trait::async_trait]
    impl LoopLlm for FailingLlm {
        async fn complete(
            &self,
            _system: &str,
            _user: &str,
            _task: TaskType,
            _max_tokens: i32,
        ) -> Result<String, AppError> {
            Err(AppError::from("mock failure"))
        }
    }

    #[tokio::test]
    async fn coordinator_manual_auto_name_persists_title() {
        let (pool, ctx) = setup_chapter(
            Some("第2章"),
            Some("关键事件：雨夜叩门"),
            Some("雨声敲窗。阿岩握紧刀柄。"),
        );
        let coordinator = crate::agency::coordinator::AgencyCoordinator::for_test(
            pool.clone(),
            std::sync::Arc::new(StaticLlm {
                reply: "雨夜叩门".to_string(),
            }),
        );
        let title = coordinator
            .auto_name_chapter(&ctx.chapter_id, true)
            .await
            .unwrap();
        assert_eq!(title.as_deref(), Some("雨夜叩门"));
        let chapter = ChapterRepository::new(pool)
            .get_by_id(&ctx.chapter_id)
            .unwrap()
            .unwrap();
        assert_eq!(chapter.title.as_deref(), Some("雨夜叩门"));
    }

    #[tokio::test]
    async fn generate_title_sanitizes_llm_reply() {
        let llm = StaticLlm {
            reply: "第2章：雨夜来客".to_string(),
        };
        assert_eq!(
            generate_title(&llm, "prompt").await,
            Some("雨夜来客".to_string())
        );
    }

    #[tokio::test]
    async fn generate_title_returns_none_on_llm_error() {
        assert_eq!(generate_title(&FailingLlm, "prompt").await, None);
    }
}
