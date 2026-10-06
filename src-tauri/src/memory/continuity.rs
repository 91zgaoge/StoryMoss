#![allow(dead_code)]
//! 连续性事实：知识边界（三层信息分离）与物品归属账本。
//!
//! 对应实施计划 P0-T2 / P0-T3
//! （docs/plans/2026-10-06-p0-p3-roadmap-implementation.md）。
//!
//! - **知识边界**：`story_timeline_events` 每条事件同时记录世界真相
//!   （objective_fact）与读者此刻认知（reader_knowledge），并以 reveal_status
//!   状态机追踪揭示进度；`character_knowledge_log` 是 append-only 的知情流水；
//!   `character_states.secrets_known/secrets_unknown` 由 ingest
//!   抽取的信息流更新， 修复此前「COALESCE 永久冻结」的断链（v0.59.x 及以前
//!   secrets 只能靠手动写）。
//! - **物品归属**：`item_holdings` 只登记跨章影响行动边界的关键资源及其持有者
//!   （玉佩账本），遵循 ani-book-skill「个人短期状态回写角色档案，不建账本」的
//!   克制原则。
//! - 检测器（`detect_knowledge_leaks` /
//!   `detect_possession_conflicts`）是纯函数，
//!   供续写探针与编辑器审计复用，便于单测。

use chrono::Local;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::DbPool;

// ==================== DTO（ingest 分析 schema 的增量部分）
// ====================

/// 信息流：某角色在本次内容中获知了某条事实。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnowledgeUpdate {
    #[serde(default)]
    pub character: String,
    #[serde(default)]
    pub fact: String,
    #[serde(default)]
    pub evidence: String,
}

/// 时间线事件增量（世界真相 / 读者认知 / 揭示状态）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TimelineEventDelta {
    #[serde(default)]
    pub objective_fact: String,
    #[serde(default)]
    pub reader_knowledge: String,
    #[serde(default = "default_reveal_status")]
    pub reveal_status: String,
    #[serde(default)]
    pub participants: Vec<String>,
    #[serde(default)]
    pub evidence: String,
}

fn default_reveal_status() -> String {
    "hidden".to_string()
}

/// 关键资源归属增量（只登记跨章影响行动边界的物品）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ItemHoldingDelta {
    #[serde(default)]
    pub item: String,
    #[serde(default)]
    pub holder: String,
    /// acquire（获得）/ transfer（转手）/ lose（遗失）/ destroy（损毁）
    #[serde(default = "default_holding_action")]
    pub action: String,
    #[serde(default)]
    pub evidence: String,
}

fn default_holding_action() -> String {
    "acquire".to_string()
}

// ==================== 读模型 ====================

/// 某角色当前已知 / 尚不知道的秘密。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CharacterKnowledge {
    pub name: String,
    pub known: Vec<String>,
    pub unknown: Vec<String>,
}

/// 尚未向读者揭示的世界真相。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HiddenTruth {
    pub fact: String,
    pub reader_knowledge: Option<String>,
    pub reveal_status: String,
}

/// 物品归属账本条目。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HeldItem {
    pub item_name: String,
    pub holder_name: Option<String>,
    pub status: String,
}

// ==================== 落库 ====================

fn pool_conn(
    pool: &DbPool,
) -> Result<impl std::ops::Deref<Target = rusqlite::Connection>, rusqlite::Error> {
    pool.get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))
}

/// 解析规范化的章号：优先显式传入，否则由 scene_id 反查
/// scenes.sequence_number。
fn resolve_chapter_number(
    conn: &rusqlite::Connection,
    scene_id: Option<&str>,
    explicit: Option<i32>,
) -> Option<i32> {
    if explicit.is_some() {
        return explicit;
    }
    let scene_id = scene_id?;
    conn.query_row(
        "SELECT sequence_number FROM scenes WHERE id = ?1",
        params![scene_id],
        |row| row.get(0),
    )
    .ok()
}

fn normalize_fact(s: &str) -> String {
    s.trim()
        .trim_end_matches(|c: char| "。！？；，、.!?;,".contains(c))
        .trim()
        .to_string()
}

/// 两条事实是否指同一件事：完全相等，或（长度 ≥6）一方包含另一方。
fn facts_equivalent(a: &str, b: &str) -> bool {
    let (a, b) = (normalize_fact(a), normalize_fact(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    if a == b {
        return true;
    }
    let (short, long) = if a.chars().count() <= b.chars().count() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    short.chars().count() >= 6 && long.contains(short.as_str())
}

/// 应用信息流：把角色新获知的事实写入 secrets_known，并从 secrets_unknown
/// 移除。
///
/// 这是「角色知道了什么」的唯一自动更新链路——此前 ingest 用 COALESCE 保留旧值，
/// secrets 永不变化。返回实际更新的条数。
pub fn persist_knowledge_updates(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
    chapter_number: Option<i32>,
    updates: &[KnowledgeUpdate],
) -> Result<usize, rusqlite::Error> {
    if updates.is_empty() {
        return Ok(0);
    }
    let conn = pool_conn(pool)?;
    let chapter_number = resolve_chapter_number(&conn, scene_id, chapter_number);
    let now = Local::now().to_rfc3339();
    let mut applied = 0usize;

    for update in updates {
        let name = update.character.trim();
        let fact = normalize_fact(&update.fact);
        if name.is_empty() || fact.is_empty() {
            continue;
        }
        let character_id: Option<String> = conn
            .query_row(
                "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2",
                params![story_id, name],
                |row| row.get(0),
            )
            .ok();
        let Some(character_id) = character_id else {
            continue; // 角色未登记（资产桥会先注册），跳过
        };

        let existing: Option<(String, String)> = conn
            .query_row(
                "SELECT COALESCE(secrets_known, '[]'), COALESCE(secrets_unknown, '[]') \
                 FROM character_states WHERE character_id = ?1",
                params![&character_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        let (known_json, unknown_json) = existing
            .clone()
            .unwrap_or_else(|| ("[]".to_string(), "[]".to_string()));
        let mut known: Vec<String> = serde_json::from_str(&known_json).unwrap_or_default();
        let mut unknown: Vec<String> = serde_json::from_str(&unknown_json).unwrap_or_default();

        // 幂等：已知则不再重复登记
        if known.iter().any(|k| facts_equivalent(k, &fact)) {
            continue;
        }
        known.push(fact.clone());
        unknown.retain(|u| !facts_equivalent(u, &fact));

        let known_new = serde_json::to_string(&known).unwrap_or_else(|_| "[]".to_string());
        let unknown_new = serde_json::to_string(&unknown).unwrap_or_else(|_| "[]".to_string());

        if existing.is_none() {
            conn.execute(
                "INSERT INTO character_states \
                 (id, story_id, character_id, secrets_known, secrets_unknown, arc_progress, last_updated) \
                 VALUES (?1, ?2, ?3, '[]', '[]', 0.0, ?4)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    story_id,
                    &character_id,
                    &now
                ],
            )?;
        }
        conn.execute(
            "UPDATE character_states SET secrets_known = ?1, secrets_unknown = ?2, last_updated = ?3 \
             WHERE character_id = ?4",
            params![known_new, unknown_new, &now, &character_id],
        )?;
        conn.execute(
            "INSERT INTO character_knowledge_log \
             (id, story_id, character_id, fact, change_type, source_scene_id, chapter_number, evidence, created_at) \
             VALUES (?1, ?2, ?3, ?4, 'learned', ?5, ?6, ?7, ?8)",
            params![
                uuid::Uuid::new_v4().to_string(),
                story_id,
                &character_id,
                &fact,
                scene_id,
                chapter_number,
                update.evidence.trim(),
                &now
            ],
        )?;
        applied += 1;
    }

    if applied > 0 {
        log::info!(
            "[continuity] 更新 {} 条角色知情（story_id={}）",
            applied,
            story_id
        );
    }
    Ok(applied)
}

fn reveal_rank(status: &str) -> i32 {
    match status {
        "revealed" => 2,
        "partial" => 1,
        _ => 0,
    }
}

/// 写入/更新时间线事件（世界真相 + 读者认知 + 揭示状态）。
///
/// 按 (story_id, objective_fact) 去重：已存在时仅在信息更靠后时升级
/// （reader_knowledge 有值时覆盖；reveal_status 只向 hidden → partial →
/// revealed 单向推进）。
pub fn persist_timeline_events(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
    chapter_number: Option<i32>,
    events: &[TimelineEventDelta],
) -> Result<usize, rusqlite::Error> {
    if events.is_empty() {
        return Ok(0);
    }
    let conn = pool_conn(pool)?;
    let chapter_number = resolve_chapter_number(&conn, scene_id, chapter_number);
    let now = Local::now().to_rfc3339();
    let mut written = 0usize;

    for (index, event) in events.iter().enumerate() {
        let fact = event.objective_fact.trim();
        if fact.is_empty() {
            continue;
        }
        let status = match event.reveal_status.trim() {
            "partial" => "partial",
            "revealed" => "revealed",
            _ => "hidden",
        };
        let reveal_chapter = if status == "hidden" {
            None
        } else {
            chapter_number
        };
        let reader = {
            let text = event.reader_knowledge.trim();
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        };
        let participants =
            serde_json::to_string(&event.participants).unwrap_or_else(|_| "[]".to_string());

        let existing: Option<(String, String, Option<i32>)> = conn
            .query_row(
                "SELECT reveal_status, COALESCE(reader_knowledge, ''), reveal_chapter \
                 FROM story_timeline_events WHERE story_id = ?1 AND objective_fact = ?2 LIMIT 1",
                params![story_id, fact],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .ok();

        match existing {
            Some((old_status, old_reader, old_chapter)) => {
                let new_status = if reveal_rank(status) > reveal_rank(&old_status) {
                    status
                } else {
                    old_status.as_str()
                };
                let new_reader = if reader.is_some() {
                    reader
                } else if old_reader.is_empty() {
                    None
                } else {
                    Some(old_reader)
                };
                let new_chapter = if new_status == "hidden" {
                    None
                } else {
                    old_chapter.or(reveal_chapter)
                };
                conn.execute(
                    "UPDATE story_timeline_events \
                     SET reveal_status = ?1, reader_knowledge = ?2, reveal_chapter = ?3, \
                         participants = ?4, updated_at = ?5 \
                     WHERE story_id = ?6 AND objective_fact = ?7",
                    params![
                        new_status,
                        new_reader,
                        new_chapter,
                        participants,
                        &now,
                        story_id,
                        fact
                    ],
                )?;
                written += 1;
            }
            None => {
                let sequence = chapter_number.map(|c| c * 1000 + index as i32);
                conn.execute(
                    "INSERT INTO story_timeline_events \
                     (id, story_id, chapter_number, scene_id, sequence_number, objective_fact, \
                      reader_knowledge, reveal_status, reveal_chapter, participants, source, \
                      created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'ingest', ?11, ?11)",
                    params![
                        uuid::Uuid::new_v4().to_string(),
                        story_id,
                        chapter_number,
                        scene_id,
                        sequence,
                        fact,
                        reader,
                        status,
                        reveal_chapter,
                        participants,
                        &now
                    ],
                )?;
                written += 1;
            }
        }
    }

    if written > 0 {
        log::info!(
            "[continuity] 写入 {} 条时间线事件（story_id={}）",
            written,
            story_id
        );
    }
    Ok(written)
}

/// 物品归属账本 upsert：按 (story_id, item_name) 更新持有者与状态。
pub fn persist_item_holdings(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
    chapter_number: Option<i32>,
    holdings: &[ItemHoldingDelta],
) -> Result<usize, rusqlite::Error> {
    if holdings.is_empty() {
        return Ok(0);
    }
    let conn = pool_conn(pool)?;
    let chapter_number = resolve_chapter_number(&conn, scene_id, chapter_number);
    let now = Local::now().to_rfc3339();
    let mut written = 0usize;

    for holding in holdings {
        let item = holding.item.trim();
        if item.is_empty() {
            continue;
        }
        let action = holding.action.trim().to_ascii_lowercase();
        let holder_name = holding.holder.trim();
        let (status, holder): (&str, Option<String>) = match action.as_str() {
            "transfer" => ("held", non_empty(holder_name)),
            "lose" => ("lost", None),
            "destroy" => ("destroyed", None),
            _ => ("held", non_empty(holder_name)),
        };
        let holder_character_id: Option<String> = holder.as_ref().and_then(|name| {
            conn.query_row(
                "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2",
                params![story_id, name],
                |row| row.get(0),
            )
            .ok()
        });
        let item_entity_id: Option<String> = conn
            .query_row(
                "SELECT id FROM kg_entities WHERE story_id = ?1 AND name = ?2 AND entity_type = 'Item' \
                 AND is_archived = 0 LIMIT 1",
                params![story_id, item],
                |row| row.get(0),
            )
            .ok();

        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM item_holdings WHERE story_id = ?1 AND item_name = ?2",
                params![story_id, item],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if exists {
            conn.execute(
                "UPDATE item_holdings SET holder_name = ?1, holder_character_id = ?2, status = ?3, \
                 evidence = ?4, item_entity_id = COALESCE(?5, item_entity_id), \
                 acquired_chapter = COALESCE(acquired_chapter, ?6), updated_at = ?7 \
                 WHERE story_id = ?8 AND item_name = ?9",
                params![
                    holder,
                    holder_character_id,
                    status,
                    holding.evidence.trim(),
                    item_entity_id,
                    chapter_number,
                    &now,
                    story_id,
                    item
                ],
            )?;
        } else {
            conn.execute(
                "INSERT INTO item_holdings \
                 (id, story_id, item_name, item_entity_id, holder_name, holder_character_id, \
                  status, acquired_chapter, evidence, source_scene_id, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    story_id,
                    item,
                    item_entity_id,
                    holder,
                    holder_character_id,
                    status,
                    chapter_number,
                    holding.evidence.trim(),
                    scene_id,
                    &now
                ],
            )?;
        }
        written += 1;
    }

    if written > 0 {
        log::info!(
            "[continuity] 更新 {} 条物品归属（story_id={}）",
            written,
            story_id
        );
    }
    Ok(written)
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

// ==================== 查询 ====================

/// 读取指定角色的知情状态（name 为空则返回全部角色）。
pub fn load_character_knowledge(
    pool: &DbPool,
    story_id: &str,
    names: &[String],
) -> Vec<CharacterKnowledge> {
    let Ok(conn) = pool_conn(pool) else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(
        "SELECT c.name, COALESCE(cs.secrets_known, '[]'), COALESCE(cs.secrets_unknown, '[]') \
         FROM characters c LEFT JOIN character_states cs ON cs.character_id = c.id \
         WHERE c.story_id = ?1",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id], |row| {
        let name: String = row.get(0)?;
        let known: String = row.get(1)?;
        let unknown: String = row.get(2)?;
        Ok((name, known, unknown))
    });
    let Ok(rows) = rows else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (name, known, unknown) in rows.flatten() {
        if !names.is_empty() && !names.iter().any(|n| n.trim() == name.trim()) {
            continue;
        }
        out.push(CharacterKnowledge {
            name,
            known: serde_json::from_str(&known).unwrap_or_default(),
            unknown: serde_json::from_str(&unknown).unwrap_or_default(),
        });
    }
    out
}

/// 读取尚未完全揭示的世界真相（hidden / partial）。
pub fn load_hidden_truths(pool: &DbPool, story_id: &str) -> Vec<HiddenTruth> {
    let Ok(conn) = pool_conn(pool) else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(
        "SELECT objective_fact, reader_knowledge, reveal_status \
         FROM story_timeline_events \
         WHERE story_id = ?1 AND reveal_status <> 'revealed' \
         ORDER BY sequence_number DESC LIMIT 50",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id], |row| {
        Ok(HiddenTruth {
            fact: row.get(0)?,
            reader_knowledge: row.get(1)?,
            reveal_status: row.get(2)?,
        })
    });
    rows.map(|r| r.flatten().collect()).unwrap_or_default()
}

/// 读取全量物品归属账本。
pub fn load_item_holdings(pool: &DbPool, story_id: &str) -> Vec<HeldItem> {
    let Ok(conn) = pool_conn(pool) else {
        return Vec::new();
    };
    let mut stmt = match conn.prepare(
        "SELECT item_name, holder_name, status FROM item_holdings \
         WHERE story_id = ?1 ORDER BY updated_at DESC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id], |row| {
        Ok(HeldItem {
            item_name: row.get(0)?,
            holder_name: row.get(1)?,
            status: row.get(2)?,
        })
    });
    rows.map(|r| r.flatten().collect()).unwrap_or_default()
}

/// 读取指定持有者当前持有的物品（写作注入用）。
pub fn load_item_holdings_for(
    pool: &DbPool,
    story_id: &str,
    holder_names: &[String],
) -> Vec<HeldItem> {
    load_item_holdings(pool, story_id)
        .into_iter()
        .filter(|h| {
            h.status == "held"
                && h.holder_name
                    .as_deref()
                    .map(|name| holder_names.iter().any(|n| n.trim() == name.trim()))
                    .unwrap_or(false)
        })
        .collect()
}

// ==================== 纯函数检测器 ====================

const FRAGMENT_SEPARATORS: &str =
    " \t\n\r，。！？；：、,.!?;:\"'“”‘’（）()《》〈〉【】[]{}—－-…·|/\\";

/// 从自由文本事实中提取「高区分度片段」：最长的一段不含标点/空白的连续文本。
/// 用于把「这封信是哥哥寄的」这类句子变成可做字面匹配的最小泄漏特征。
pub fn distinctive_fragment(fact: &str, min_chars: usize) -> Option<String> {
    let mut best: Option<&str> = None;
    for segment in fact.split(|c: char| FRAGMENT_SEPARATORS.contains(c)) {
        let seg = segment.trim();
        if seg.chars().count() < min_chars {
            continue;
        }
        if best
            .map(|b| seg.chars().count() > b.chars().count())
            .unwrap_or(true)
        {
            best = Some(seg);
        }
    }
    best.map(|s| s.to_string())
}

/// 泄密检测：正文是否说出了某在场角色尚不知道的信息，或提前陈述了未揭示的真相。
///
/// `planned` 是本拍的规划文本（大纲节点/必须改变项/待兑现审查）——若泄漏特征已在
/// 规划里出现，视为「计划内揭示」，不告警（控制误报）。
pub fn detect_knowledge_leaks(
    text: &str,
    knowledge: &[CharacterKnowledge],
    hidden: &[HiddenTruth],
    planned: &str,
) -> Vec<String> {
    const MIN_FRAGMENT_CHARS: usize = 8;
    let mut leaks: Vec<String> = Vec::new();

    for entry in knowledge {
        for unknown in &entry.unknown {
            let Some(fragment) = distinctive_fragment(unknown, MIN_FRAGMENT_CHARS) else {
                continue;
            };
            if !text.contains(&fragment) || planned.contains(&fragment) {
                continue;
            }
            // 若该事实已在 known 里（等价），说明是已更新的记录，跳过
            if entry.known.iter().any(|k| facts_equivalent(k, unknown)) {
                continue;
            }
            leaks.push(format!(
                "「{}」可能说出了尚不知道的信息：{}",
                entry.name, fragment
            ));
        }
    }

    for truth in hidden {
        let Some(fragment) = distinctive_fragment(&truth.fact, MIN_FRAGMENT_CHARS) else {
            continue;
        };
        if text.contains(&fragment) && !planned.contains(&fragment) {
            leaks.push(format!("正文可能提前揭示未公开真相：{}", fragment));
        }
    }

    leaks.sort();
    leaks.dedup();
    leaks.truncate(3);
    leaks
}

const USE_VERBS: &[&str] = &[
    "拿出", "掏出", "取出", "握着", "递给", "交给", "收好", "塞进", "举起", "挥舞", "佩戴", "戴上",
    "翻开", "打开",
];
const TRANSFER_VERBS: &[&str] = &["递给", "交给", "送给", "塞给", "抛给", "扔给", "转交"];

/// 物品归属矛盾检测：正文使用了某物品，但它的持有者不在场（且没有转手动作）。
///
/// 规则（控制误报）：
/// - 只检查 status=held 且有明确持有者的物品；
/// - 物品名出现在使用动词 ±40 字符窗口内才算「被使用」；
/// - 持有者已在场 → 正常；正文同段出现持有者 + 转手动词 → 视为当场转手，跳过；
/// - 已遗失/损毁的物品再次被使用 → 告警。
pub fn detect_possession_conflicts(
    text: &str,
    present_names: &[String],
    holdings: &[HeldItem],
) -> Vec<String> {
    let mut gaps: Vec<String> = Vec::new();
    for holding in holdings {
        let Some(pos) = text.find(&holding.item_name) else {
            continue;
        };
        let window_start = text[..pos]
            .char_indices()
            .rev()
            .nth(40)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let window_end = text[pos..]
            .char_indices()
            .nth(40)
            .map(|(i, _)| pos + i)
            .unwrap_or(text.len());
        let window = &text[window_start..window_end];
        let used = USE_VERBS.iter().any(|v| window.contains(v));
        if !used {
            continue;
        }

        match holding.status.as_str() {
            "lost" | "destroyed" => {
                gaps.push(format!(
                    "物品「{}」已{}，却在本拍被再次使用",
                    holding.item_name,
                    if holding.status == "lost" {
                        "遗失"
                    } else {
                        "损毁"
                    }
                ));
            }
            _ => {
                let Some(holder) = holding.holder_name.as_deref() else {
                    continue;
                };
                if present_names.iter().any(|n| n.trim() == holder) {
                    continue;
                }
                // 当场转手：持有者名字与转手动词同时出现在窗口内
                let transferred =
                    window.contains(holder) && TRANSFER_VERBS.iter().any(|v| window.contains(v));
                if transferred {
                    continue;
                }
                gaps.push(format!(
                    "物品「{}」由「{}」持有，但该角色不在场，正文却在使用它",
                    holding.item_name, holder
                ));
            }
        }
    }
    gaps.sort();
    gaps.dedup();
    gaps.truncate(2);
    gaps
}

/// 渲染写作注入用的连续性约束块（在场物品 / 本拍信息差 / 未公开真相）。
///
/// 这是「预防优于检测」的落点：在生成前把不该出现的信息明确列为禁令，
/// 而不是等生成后再回退。`planned` 为本拍规划文本（大纲节点/必须改变项），
/// 已列入本拍计划的信息不作为禁令（用于计划内揭示/获知）。
pub fn render_continuity_blocks(
    pool: &DbPool,
    story_id: &str,
    character_names: &[String],
    planned: &str,
) -> Vec<String> {
    const MAX_LINES_PER_BLOCK: usize = 6;
    const MAX_FACT_CHARS: usize = 48;
    let shorten = |s: &str| -> String {
        let t: String = s.chars().take(MAX_FACT_CHARS).collect();
        if s.chars().count() > MAX_FACT_CHARS {
            format!("{t}…")
        } else {
            t
        }
    };
    let mut blocks: Vec<String> = Vec::new();

    let items = load_item_holdings_for(pool, story_id, character_names);
    if !items.is_empty() {
        let lines: Vec<String> = items
            .iter()
            .take(MAX_LINES_PER_BLOCK)
            .map(|h| {
                format!(
                    "  - {}：{}",
                    h.holder_name.as_deref().unwrap_or("（未定）"),
                    h.item_name
                )
            })
            .collect();
        blocks.push(format!(
            "【在场物品（归属必须一致：非持有者不得使用，除非本拍明确转手）】\n{}",
            lines.join("\n")
        ));
    }

    let knowledge = load_character_knowledge(pool, story_id, character_names);
    let lines: Vec<String> = knowledge
        .iter()
        .filter(|k| !k.unknown.is_empty())
        .map(|k| {
            let facts: Vec<String> = k
                .unknown
                .iter()
                .filter(|f| {
                    // 计划内获知的不列为禁令
                    !planned.contains(f.as_str())
                        && distinctive_fragment(f, 8)
                            .map(|frag| !planned.contains(&frag))
                            .unwrap_or(true)
                })
                .take(4)
                .map(|f| shorten(f))
                .collect();
            (k.name.clone(), facts)
        })
        .filter(|(_, facts)| !facts.is_empty())
        .take(4)
        .map(|(name, facts)| {
            format!(
                "  - 「{}」尚不知道：{}（不得说出、不得由叙述点破）",
                name,
                facts.join("；")
            )
        })
        .collect();
    if !lines.is_empty() {
        blocks.push(format!(
            "【本拍信息差（绝不可泄露）】\n{}",
            lines.join("\n")
        ));
    }

    let hidden = load_hidden_truths(pool, story_id);
    let hlines: Vec<String> = hidden
        .iter()
        .filter(|t| {
            distinctive_fragment(&t.fact, 8)
                .map(|frag| !planned.contains(&frag))
                .unwrap_or(true)
        })
        .take(MAX_LINES_PER_BLOCK)
        .map(|t| format!("  - {}", shorten(&t.fact)))
        .collect();
    if !hlines.is_empty() {
        blocks.push(format!(
            "【未公开真相（未经大纲明确安排不得写进正文）】\n{}",
            hlines.join("\n")
        ));
    }

    blocks
}

/// 续写探针入口：汇总知识边界与物品归属两类 gap。
/// - 泄密：在场角色尚不知道的信息被说出（计划内揭示豁免）；
/// - 归属：物品被不在场的持有者之外的人使用，或已遗失物品再次出现。
pub fn continuity_gaps(
    pool: &DbPool,
    story_id: &str,
    increment: &str,
    present_names: &[String],
    planned: &str,
) -> Vec<String> {
    let mut gaps: Vec<String> = Vec::new();
    let holdings = load_item_holdings(pool, story_id);
    gaps.extend(detect_possession_conflicts(
        increment,
        present_names,
        &holdings,
    ));
    let knowledge = load_character_knowledge(pool, story_id, present_names);
    let hidden = load_hidden_truths(pool, story_id);
    gaps.extend(detect_knowledge_leaks(
        increment, &knowledge, &hidden, planned,
    ));
    gaps.truncate(4);
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed_story_and_characters(pool: &DbPool, characters: &[&str]) -> String {
        let story_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '测试故事', ?2, ?2)",
            params![&story_id, &now],
        )
        .unwrap();
        for name in characters {
            conn.execute(
                "INSERT INTO characters (id, story_id, name, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![uuid::Uuid::new_v4().to_string(), &story_id, name, &now],
            )
            .unwrap();
        }
        story_id
    }

    fn seed_scene(pool: &DbPool, story_id: &str, sequence: i32) -> String {
        let scene_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![&scene_id, story_id, sequence, &now],
        )
        .unwrap();
        scene_id
    }

    #[test]
    fn test_persist_knowledge_updates_moves_secret_from_unknown_to_known() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_and_characters(&pool, &["徐棠", "谭守义"]);
        let scene_id = seed_scene(&pool, &story_id, 3);

        // 预置：徐棠不知道「这封信是哥哥寄的」
        {
            let conn = pool.get().unwrap();
            let character_id: String = conn
                .query_row(
                    "SELECT id FROM characters WHERE story_id = ?1 AND name = '徐棠'",
                    params![&story_id],
                    |row| row.get(0),
                )
                .unwrap();
            conn.execute(
                "INSERT INTO character_states (id, story_id, character_id, secrets_known, secrets_unknown, arc_progress, last_updated) \
                 VALUES (?1, ?2, ?3, '[]', '[\"这封信是哥哥寄的\"]', 0.0, '2026-01-01T00:00:00Z')",
                params![uuid::Uuid::new_v4().to_string(), &story_id, &character_id],
            )
            .unwrap();
        }

        let applied = persist_knowledge_updates(
            &pool,
            &story_id,
            Some(&scene_id),
            None,
            &[KnowledgeUpdate {
                character: "徐棠".into(),
                fact: "这封信是哥哥寄的".into(),
                evidence: "她认出了哥哥的笔迹".into(),
            }],
        )
        .unwrap();
        assert_eq!(applied, 1);

        let knowledge = load_character_knowledge(&pool, &story_id, &["徐棠".into()]);
        assert_eq!(knowledge.len(), 1);
        assert!(knowledge[0].known.iter().any(|k| k == "这封信是哥哥寄的"));
        assert!(knowledge[0].unknown.is_empty(), "未知应从 unknown 移除");

        // 幂等：重复应用不产生第二条
        let again = persist_knowledge_updates(
            &pool,
            &story_id,
            Some(&scene_id),
            None,
            &[KnowledgeUpdate {
                character: "徐棠".into(),
                fact: "这封信是哥哥寄的".into(),
                evidence: String::new(),
            }],
        )
        .unwrap();
        assert_eq!(again, 0);

        // 审计流水留痕
        let conn = pool.get().unwrap();
        let log_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_knowledge_log WHERE story_id = ?1",
                params![&story_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(log_count, 1);
    }

    #[test]
    fn test_persist_timeline_events_upgrades_reveal_status_only_forward() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_and_characters(&pool, &[]);

        persist_timeline_events(
            &pool,
            &story_id,
            None,
            Some(2),
            &[TimelineEventDelta {
                objective_fact: "谭守义的同班战友在戈壁事故中牺牲".into(),
                reader_knowledge: "读者只知道谭守义有个没人听全的故事".into(),
                reveal_status: "hidden".into(),
                participants: vec!["谭守义".into()],
                evidence: String::new(),
            }],
        )
        .unwrap();

        // 低等级写入不得回退（hidden 覆盖 revealed 被拒绝）
        persist_timeline_events(
            &pool,
            &story_id,
            None,
            Some(5),
            &[TimelineEventDelta {
                objective_fact: "谭守义的同班战友在戈壁事故中牺牲".into(),
                reader_knowledge: String::new(),
                reveal_status: "hidden".into(),
                participants: vec![],
                evidence: String::new(),
            }],
        )
        .unwrap();
        persist_timeline_events(
            &pool,
            &story_id,
            None,
            Some(7),
            &[TimelineEventDelta {
                objective_fact: "谭守义的同班战友在戈壁事故中牺牲".into(),
                reader_knowledge: "读者已知事故的官方说法".into(),
                reveal_status: "partial".into(),
                participants: vec![],
                evidence: String::new(),
            }],
        )
        .unwrap();

        let hidden = load_hidden_truths(&pool, &story_id);
        assert_eq!(hidden.len(), 1);
        assert_eq!(hidden[0].reveal_status, "partial");
        assert_eq!(
            hidden[0].reader_knowledge.as_deref(),
            Some("读者已知事故的官方说法")
        );

        let conn = pool.get().unwrap();
        let reveal_chapter: Option<i32> = conn
            .query_row(
                "SELECT reveal_chapter FROM story_timeline_events WHERE story_id = ?1",
                params![&story_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(reveal_chapter, Some(7));
        let rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM story_timeline_events WHERE story_id = ?1",
                params![&story_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, 1, "同一事实应 upsert 而非重复插入");
    }

    #[test]
    fn test_persist_item_holdings_upserts_by_item_and_tracks_status() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_and_characters(&pool, &["林晚", "苏亦铁"]);
        let scene_a = seed_scene(&pool, &story_id, 1);
        let scene_b = seed_scene(&pool, &story_id, 2);

        persist_item_holdings(
            &pool,
            &story_id,
            Some(&scene_a),
            Some(1),
            &[ItemHoldingDelta {
                item: "羊脂玉佩".into(),
                holder: "林晚".into(),
                action: "acquire".into(),
                evidence: "她在灯下系好玉佩".into(),
            }],
        )
        .unwrap();
        let holdings = load_item_holdings(&pool, &story_id);
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].holder_name.as_deref(), Some("林晚"));

        // 转手：同一物品 upsert，不新增行
        persist_item_holdings(
            &pool,
            &story_id,
            Some(&scene_b),
            Some(2),
            &[ItemHoldingDelta {
                item: "羊脂玉佩".into(),
                holder: "苏亦铁".into(),
                action: "transfer".into(),
                evidence: "林晚把玉佩塞进他手里".into(),
            }],
        )
        .unwrap();
        let holdings = load_item_holdings(&pool, &story_id);
        assert_eq!(holdings.len(), 1, "同一物品应 upsert");
        assert_eq!(holdings[0].holder_name.as_deref(), Some("苏亦铁"));

        // 遗失：状态迁移，持有者清空
        persist_item_holdings(
            &pool,
            &story_id,
            Some(&scene_b),
            Some(2),
            &[ItemHoldingDelta {
                item: "羊脂玉佩".into(),
                holder: String::new(),
                action: "lose".into(),
                evidence: String::new(),
            }],
        )
        .unwrap();
        let holdings = load_item_holdings(&pool, &story_id);
        assert_eq!(holdings[0].status, "lost");
        assert!(holdings[0].holder_name.is_none());
    }

    #[test]
    fn test_knowledge_boundary_detects_unknown_secret_leak() {
        let knowledge = vec![CharacterKnowledge {
            name: "徐棠".into(),
            known: vec![],
            unknown: vec!["这封信是哥哥寄的".into()],
        }];
        let hidden = vec![];
        let text = "徐棠把信展开：这封信是哥哥寄的。她忽然明白了什么。";
        let leaks = detect_knowledge_leaks(text, &knowledge, &hidden, "");
        assert!(
            leaks
                .iter()
                .any(|l| l.contains("徐棠") && l.contains("尚不知道")),
            "应检出角色泄密 leaks={:?}",
            leaks
        );

        // 计划内揭示豁免
        let planned = "本拍徐棠发现这封信是哥哥寄的";
        let leaks_planned = detect_knowledge_leaks(text, &knowledge, &hidden, planned);
        assert!(leaks_planned.is_empty(), "计划内揭示不得告警");

        // 已知不告警
        let known = vec![CharacterKnowledge {
            name: "徐棠".into(),
            known: vec!["这封信是哥哥寄的".into()],
            unknown: vec![],
        }];
        assert!(detect_knowledge_leaks(text, &known, &hidden, "").is_empty());
    }

    #[test]
    fn test_knowledge_boundary_detects_hidden_truth_reveal() {
        let hidden = vec![HiddenTruth {
            fact: "谭守义的同班战友在戈壁事故中牺牲".into(),
            reader_knowledge: None,
            reveal_status: "hidden".into(),
        }];
        let text = "老谭终于开口：谭守义的同班战友在戈壁事故中牺牲，这件事压了他五十年。";
        let leaks = detect_knowledge_leaks(text, &[], &hidden, "");
        assert!(
            leaks.iter().any(|l| l.contains("提前揭示")),
            "应检出提前揭示 leaks={:?}",
            leaks
        );
        let planned = "本拍揭示谭守义的同班战友在戈壁事故中牺牲";
        assert!(detect_knowledge_leaks(text, &[], &hidden, planned).is_empty());
    }

    #[test]
    fn test_possession_conflict_flags_absent_holder_but_allows_transfer() {
        let holdings = vec![HeldItem {
            item_name: "羊脂玉佩".into(),
            holder_name: Some("林晚".into()),
            status: "held".into(),
        }];
        let present = vec!["苏亦铁".to_string()];

        let bad = "苏亦铁从袖中掏出羊脂玉佩，在灯下掂了掂。";
        let gaps = detect_possession_conflicts(bad, &present, &holdings);
        assert!(
            gaps.iter()
                .any(|g| g.contains("羊脂玉佩") && g.contains("林晚")),
            "应检出归属矛盾 gaps={:?}",
            gaps
        );

        let transfer = "林晚把羊脂玉佩塞给苏亦铁，苏亦铁握着玉佩发了会儿呆。";
        let ok = detect_possession_conflicts(transfer, &present, &holdings);
        assert!(ok.is_empty(), "当场转手不得告警 gaps={:?}", ok);

        let with_holder = vec!["林晚".to_string(), "苏亦铁".to_string()];
        assert!(detect_possession_conflicts(bad, &with_holder, &holdings).is_empty());
    }

    #[test]
    fn test_possession_conflict_flags_lost_item_reuse() {
        let holdings = vec![HeldItem {
            item_name: "羊脂玉佩".into(),
            holder_name: None,
            status: "lost".into(),
        }];
        let gaps = detect_possession_conflicts(
            "他掏出羊脂玉佩，脸色骤变。",
            &["苏亦铁".to_string()],
            &holdings,
        );
        assert!(
            gaps.iter().any(|g| g.contains("遗失")),
            "遗失物品再现应告警 gaps={:?}",
            gaps
        );
    }

    #[test]
    fn test_continuity_gaps_reads_db_and_respects_planned_text() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_and_characters(&pool, &["徐棠", "林晚", "苏亦铁"]);
        let scene_id = seed_scene(&pool, &story_id, 4);

        persist_item_holdings(
            &pool,
            &story_id,
            Some(&scene_id),
            Some(4),
            &[ItemHoldingDelta {
                item: "羊脂玉佩".into(),
                holder: "林晚".into(),
                action: "acquire".into(),
                evidence: String::new(),
            }],
        )
        .unwrap();
        persist_timeline_events(
            &pool,
            &story_id,
            Some(&scene_id),
            Some(4),
            &[TimelineEventDelta {
                objective_fact: "将军早已知道密道入口在佛堂".into(),
                reader_knowledge: String::new(),
                reveal_status: "hidden".into(),
                participants: vec![],
                evidence: String::new(),
            }],
        )
        .unwrap();

        let increment = "苏亦铁掏出羊脂玉佩。徐棠低声道：将军早已知道密道入口在佛堂。";
        let gaps = continuity_gaps(
            &pool,
            &story_id,
            increment,
            &["苏亦铁".to_string(), "徐棠".to_string()],
            "",
        );
        assert!(
            gaps.iter().any(|g| g.contains("羊脂玉佩")),
            "物品归属 gap 缺失 gaps={:?}",
            gaps
        );
        assert!(
            gaps.iter().any(|g| g.contains("提前揭示")),
            "真相揭示 gap 缺失 gaps={:?}",
            gaps
        );

        // 计划内揭示 + 转手场景豁免
        let planned = "本拍揭示将军早已知道密道入口在佛堂";
        let gaps2 = continuity_gaps(
            &pool,
            &story_id,
            increment,
            &["苏亦铁".to_string(), "徐棠".to_string()],
            planned,
        );
        assert!(
            !gaps2.iter().any(|g| g.contains("提前揭示")),
            "计划内揭示不得 gap gaps={:?}",
            gaps2
        );
    }
}
