#![allow(dead_code)]
//! 作者风格逆向学习（P2-B，v0.62.0）。
//!
//! 对照外部项目经验（ainovel-cli 的 `/sync style_delta`、webnovel 的
//! `project-memory`）：作者对 AI 稿的手改是最强的文风信号。本模块在
//! `update_scene`（人类编辑）后**防抖 120 秒**收集前后版本差异，后台调用一次
//! LLM 提炼「可执行的文风规则」，按故事累积进 `style_preferences`，
//! 续写时以【作者文风偏好】注入。
//!
//! 成本约束：
//! - 同一故事最多一个处理器在跑（`RUNNING` 集合）；防抖期内多写
//!   last-write-wins；
//! - 提炼走全局后台闸门（`BACKGROUND_LLM_SEMAPHORE`），
//!   标签「后台风格提炼」静默；
//! - 前后文都需 ≥200 字且差异 ≥20 字才排队（过滤光标抖动级噪音）。

use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
    time::Duration,
};

use chrono::Local;
use once_cell::sync::Lazy;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::{db::DbPool, llm::LlmService, router::TaskType};

/// 防抖窗口：作者连续修改时合并为一次提炼
pub const STYLE_LEARN_DEBOUNCE_SECS: u64 = 120;
/// 注入上限（条数）
pub const MAX_ACTIVE_PREFERENCES: usize = 8;
/// 后台调用标签（须登记进 is_silent_background_label）
pub const STYLE_LEARN_LABEL: &str = "后台风格提炼";
const MIN_TEXT_CHARS: usize = 200;
const MIN_DIFF_CHARS: usize = 20;
const PROMPT_EXCERPT_CHARS: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StylePreference {
    pub id: String,
    pub story_id: String,
    pub pattern: String,
    pub evidence: Option<String>,
    pub source: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StyleDeltaOutput {
    #[serde(default)]
    pub patterns: Vec<StyleDeltaItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StyleDeltaItem {
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub evidence: String,
}

/// 解析风格提炼输出（容错：围栏/尾随逗号/思考链），逐条做形态校验。
pub fn parse_style_delta(raw: &str) -> Vec<(String, String)> {
    let parsed = crate::narrative::extract_and_sanitize_json(raw)
        .ok()
        .and_then(|json| serde_json::from_str::<StyleDeltaOutput>(&json).ok())
        .unwrap_or_default();
    let mut out: Vec<(String, String)> = Vec::new();
    for item in parsed.patterns {
        let pattern = item.pattern.trim().trim_matches('"').trim().to_string();
        let len = pattern.chars().count();
        if !(4..=60).contains(&len) || pattern.starts_with('{') || pattern.starts_with('[') {
            continue;
        }
        if out.iter().any(|(existing, _)| existing == &pattern) {
            continue;
        }
        out.push((pattern, item.evidence.trim().to_string()));
        if out.len() >= 5 {
            break;
        }
    }
    out
}

/// 是否值得排队提炼：前后文都够长、确实被修改、改动量非光标级。
pub fn looks_like_style_signal(before: &str, after: &str) -> bool {
    let b = before.chars().count();
    let a = after.chars().count();
    if b < MIN_TEXT_CHARS || a < MIN_TEXT_CHARS || before == after {
        return false;
    }
    (b as i64 - a as i64).unsigned_abs() as usize >= MIN_DIFF_CHARS
}

/// 读取启用的文风偏好（新→旧）。
pub fn load_active_preferences(
    pool: &DbPool,
    story_id: &str,
    limit: usize,
) -> Vec<StylePreference> {
    let Ok(conn) = pool.get() else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(
        "SELECT id, story_id, pattern, evidence, source, status, created_at, updated_at \
         FROM style_preferences WHERE story_id = ?1 AND status = 'active' \
         ORDER BY updated_at DESC LIMIT ?2",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id, limit as i64], |row| {
        Ok(StylePreference {
            id: row.get(0)?,
            story_id: row.get(1)?,
            pattern: row.get(2)?,
            evidence: row.get(3)?,
            source: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    });
    rows.map(|r| r.flatten().collect()).unwrap_or_default()
}

/// 列出偏好（可按状态过滤；status=None 表示全部），供「运行维护」页管理。
pub fn list_preferences(
    pool: &DbPool,
    story_id: &str,
    status: Option<&str>,
    limit: i64,
) -> Vec<StylePreference> {
    let Ok(conn) = pool.get() else {
        return Vec::new();
    };
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<StylePreference> {
        Ok(StylePreference {
            id: row.get(0)?,
            story_id: row.get(1)?,
            pattern: row.get(2)?,
            evidence: row.get(3)?,
            source: row.get(4)?,
            status: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    };
    let sql_all = "SELECT id, story_id, pattern, evidence, source, status, created_at, updated_at \
                   FROM style_preferences WHERE story_id = ?1 \
                   ORDER BY updated_at DESC LIMIT ?2";
    let sql_status =
        "SELECT id, story_id, pattern, evidence, source, status, created_at, updated_at \
         FROM style_preferences WHERE story_id = ?1 AND status = ?2 \
         ORDER BY updated_at DESC LIMIT ?3";
    let rows: Vec<StylePreference> = match status {
        Some(status) => match conn.prepare(sql_status) {
            Ok(mut stmt) => stmt
                .query_map(params![story_id, status, limit], map_row)
                .map(|r| r.flatten().collect())
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        },
        None => match conn.prepare(sql_all) {
            Ok(mut stmt) => stmt
                .query_map(params![story_id, limit], map_row)
                .map(|r| r.flatten().collect())
                .unwrap_or_default(),
            Err(_) => Vec::new(),
        },
    };
    rows
}

/// 重新启用一条被停用的偏好。
pub fn reactivate_preference(pool: &DbPool, preference_id: &str) -> Result<usize, rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "UPDATE style_preferences SET status = 'active', updated_at = ?1 WHERE id = ?2",
        params![now, preference_id],
    )
}

/// upsert 一条偏好（同故事同 pattern 去重，刷新证据与时间）。
pub fn upsert_preference(
    pool: &DbPool,
    story_id: &str,
    pattern: &str,
    evidence: &str,
) -> Result<(), rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO style_preferences \
         (id, story_id, pattern, evidence, source, status, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'user_edit', 'active', ?5, ?5) \
         ON CONFLICT(story_id, pattern) DO UPDATE SET \
          evidence = excluded.evidence, status = 'active', updated_at = excluded.updated_at",
        params![
            uuid::Uuid::new_v4().to_string(),
            story_id,
            pattern,
            if evidence.is_empty() {
                None
            } else {
                Some(evidence)
            },
            now
        ],
    )?;
    Ok(())
}

/// 停用一条偏好（作者可在后续 UI/命令中撤销某条规则）。
pub fn deactivate_preference(pool: &DbPool, preference_id: &str) -> Result<usize, rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "UPDATE style_preferences SET status = 'disabled', updated_at = ?1 WHERE id = ?2",
        params![now, preference_id],
    )
}

/// 渲染续写注入块；没有偏好时返回 None（不产生空块）。
pub fn render_style_block(pool: &DbPool, story_id: &str) -> Option<String> {
    let prefs = load_active_preferences(pool, story_id, MAX_ACTIVE_PREFERENCES);
    if prefs.is_empty() {
        return None;
    }
    let lines: Vec<String> = prefs.iter().map(|p| format!("  - {}", p.pattern)).collect();
    Some(format!(
        "【作者文风偏好（从你的手改中提炼，优先遵守）】\n{}",
        lines.join("\n")
    ))
}

/// 构造提炼 prompt（资产缺失时内置兜底）。
pub fn build_prompt(pool: &DbPool, before: &str, after: &str) -> String {
    let before_excerpt: String = before.chars().take(PROMPT_EXCERPT_CHARS).collect();
    let after_excerpt: String = after.chars().take(PROMPT_EXCERPT_CHARS).collect();
    let mut vars = HashMap::new();
    vars.insert("before_excerpt".to_string(), before_excerpt.clone());
    vars.insert("after_excerpt".to_string(), after_excerpt.clone());
    crate::prompts::registry::resolve_prompt_with_vars(pool, "style_delta_extraction", &vars)
        .ok()
        .or_else(|| {
            crate::prompts::registry::resolve_prompt_default_with_vars(
                "style_delta_extraction",
                &vars,
            )
        })
        .unwrap_or_else(|| {
            format!(
                "你是文风分析师。下面是同一段文字的两个版本：before 是 AI 生成的原文，\
                 after 是作者手改后的版本。请提炼作者偏好的**可执行文风规则**（最多 5 条，\
                 每条 6-30 字，祈使句，例如「删掉解释性副词」「对话不加修饰语」）。\
                 只提炼能从差异中直接看出的偏好，不要评价，不要复述剧情。\
                 仅输出 JSON：{{\"patterns\":[{{\"pattern\":\"...\",\"evidence\":\"对照片段\"}}]}}\n\n\
                 【before】\n{before_excerpt}\n\n【after】\n{after_excerpt}"
            )
        })
}

/// 提炼一次并落库；返回写入条数。
pub async fn extract_and_store(
    pool: &DbPool,
    llm: &LlmService,
    story_id: &str,
    before: &str,
    after: &str,
) -> usize {
    let prompt = build_prompt(pool, before, after);
    let response = match llm
        .generate_for_task(
            TaskType::Analysis,
            prompt,
            Some(600),
            Some(0.2),
            Some(STYLE_LEARN_LABEL),
        )
        .await
    {
        Ok(response) => response,
        Err(e) => {
            log::warn!("[style_learning] 提炼失败（跳过）: {}", e);
            return 0;
        }
    };
    let patterns = parse_style_delta(&response.content);
    let mut written = 0usize;
    for (pattern, evidence) in patterns {
        match upsert_preference(pool, story_id, &pattern, &evidence) {
            Ok(()) => written += 1,
            Err(e) => log::warn!("[style_learning] 写入偏好失败: {}", e),
        }
    }
    if written > 0 {
        log::info!(
            "[style_learning] 从作者手改中提炼 {} 条文风偏好（story_id={}）",
            written,
            story_id
        );
    }
    written
}

// ==================== 防抖调度 ====================

#[derive(Clone)]
struct PendingEdit {
    before: String,
    after: String,
}

static PENDING: Lazy<Mutex<HashMap<String, PendingEdit>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static RUNNING: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// 人类编辑入口：排队（防抖 + last-write-wins），必要时启动处理器。
pub fn note_user_edit(
    app: tauri::AppHandle,
    pool: DbPool,
    story_id: String,
    before: &str,
    after: &str,
) {
    if !looks_like_style_signal(before, after) {
        return;
    }
    let should_spawn = {
        let mut pending = PENDING.lock().unwrap();
        pending.insert(
            story_id.clone(),
            PendingEdit {
                before: before.to_string(),
                after: after.to_string(),
            },
        );
        let mut running = RUNNING.lock().unwrap();
        if running.contains(&story_id) {
            false
        } else {
            running.insert(story_id.clone());
            true
        }
    };
    if !should_spawn {
        return; // 已有处理器在跑，醒来时会取到最新一次编辑
    }
    spawn_processor(app, pool, story_id);
}

fn spawn_processor(app: tauri::AppHandle, pool: DbPool, story_id: String) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(STYLE_LEARN_DEBOUNCE_SECS)).await;
            let pending = {
                let mut map = PENDING.lock().unwrap();
                map.remove(&story_id)
            };
            let Some(edit) = pending else {
                break;
            };
            let bg_permit = crate::concurrency::BACKGROUND_LLM_SEMAPHORE.acquire().await;
            if bg_permit.is_err() {
                log::warn!("[style_learning] 获取后台闸门失败，跳过本轮提炼");
                break;
            }
            let _bg_permit = bg_permit.unwrap();
            let llm = LlmService::new(app.clone());
            extract_and_store(&pool, &llm, &story_id, &edit.before, &edit.after).await;
            // 处理期间若又有新编辑排队，继续下一轮；否则释放占用
            let has_more = PENDING.lock().unwrap().contains_key(&story_id);
            if !has_more {
                RUNNING.lock().unwrap().remove(&story_id);
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed_story(pool: &DbPool) -> String {
        let story_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '文风', ?2, ?2)",
            params![&story_id, &now],
        )
        .unwrap();
        story_id
    }

    #[test]
    fn parse_style_delta_validates_and_dedupes() {
        let raw = "```json\n{\"patterns\":[\
            {\"pattern\":\"删掉解释性副词\",\"evidence\":\"他慢慢地走→他走\"},\
            {\"pattern\":\"删掉解释性副词\",\"evidence\":\"重复\"},\
            {\"pattern\":\"太短\"},\
            {\"pattern\":\"{\\\"json\\\": true}\"},\
            {\"pattern\":\"对话不加修饰语\",\"evidence\":\"a→b\"},\
            {\"pattern\":\"段落更短，一句一段\",\"evidence\":\"c→d\"}\
        ],}\n```";
        let parsed = parse_style_delta(raw);
        assert_eq!(parsed.len(), 3, "{parsed:?}");
        assert_eq!(parsed[0].0, "删掉解释性副词");
        assert!(parsed.iter().any(|(p, _)| p == "段落更短，一句一段"));
    }

    #[test]
    fn parse_style_delta_rejects_garbage() {
        assert!(parse_style_delta("模型没吐 JSON").is_empty());
        assert!(parse_style_delta("{\"patterns\": []}").is_empty());
    }

    #[test]
    fn style_signal_gate_filters_noise() {
        let short = "短文本。";
        assert!(!looks_like_style_signal(short, "短文本改。"));
        let base = "字".repeat(300);
        assert!(!looks_like_style_signal(&base, &base), "未修改不排队");
        let minor = format!("{base}。");
        assert!(!looks_like_style_signal(&base, &minor), "光标级改动不排队");
        let changed = "字".repeat(250);
        assert!(looks_like_style_signal(&base, &changed));
    }

    #[test]
    fn upsert_is_idempotent_and_deactivate_works() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        upsert_preference(&pool, &story_id, "删掉解释性副词", "e1").unwrap();
        upsert_preference(&pool, &story_id, "删掉解释性副词", "e2").unwrap();
        upsert_preference(&pool, &story_id, "对话不加修饰语", "").unwrap();

        let prefs = load_active_preferences(&pool, &story_id, 10);
        assert_eq!(prefs.len(), 2, "同 pattern 应 upsert");
        let target = prefs
            .iter()
            .find(|p| p.pattern == "删掉解释性副词")
            .unwrap();
        assert_eq!(target.evidence.as_deref(), Some("e2"));

        assert_eq!(deactivate_preference(&pool, &target.id).unwrap(), 1);
        let after = load_active_preferences(&pool, &story_id, 10);
        assert_eq!(after.len(), 1);
    }

    #[test]
    fn render_block_is_none_when_empty_and_lists_patterns() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story(&pool);
        assert!(render_style_block(&pool, &story_id).is_none());
        upsert_preference(&pool, &story_id, "对话不加修饰语", "").unwrap();
        let block = render_style_block(&pool, &story_id).expect("应有块");
        assert!(block.contains("作者文风偏好"));
        assert!(block.contains("对话不加修饰语"));
    }
}
