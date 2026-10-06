#![allow(dead_code)]
//! 改稿级联影响报告（v0.60.0 P0-T4）。
//!
//! 场景重 ingest 完成后自动运行：
//! 1. **确定性影响分析**（零 LLM）：本场景提及的实体 → 反查其余场景的
//!    `entity_mentions` → 只保留后续章节 → 按 `Σconfidence × sqrt(count)`
//!    打分。
//! 2. **LLM 冲突扫描**（可降级）：改动章正文节选 + 下游章摘要 →
//!    结构化冲突清单。
//! 3. 写入 `cascade_impacts`（batch 内按目标场景 upsert），发射
//!    `SyncEvent::CascadeImpactDetected`。
//!
//! 设计原则（对齐 oh-story / V134 `user_created` 精神）：
//! **只报告与标记，不自动改写后文**——改写必须由作者在级联中心显式触发。

use std::collections::{HashMap, HashSet};

use chrono::Local;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::{
    creative_engine::cascade_rewriter::models::{ChangeType, EntityChangeEvent},
    db::DbPool,
    llm::LlmService,
    router::TaskType,
    state_sync::StateSync,
};

/// 后台 LLM 调用标签（须同时登记进 `is_silent_background_label`；
/// 含「后台」关键词自动路由到 Background 模型角色）。
pub const CASCADE_SCAN_LABEL: &str = "后台级联扫描";
pub const EVENT_CASCADE_CONFLICT_SCAN_DONE: &str = "cascade-conflict-scan-done";

/// 单批最多保留的目标场景数
const MAX_TARGETS: usize = 10;
/// 送入 LLM 冲突扫描的目标数上限
const MAX_LLM_TARGETS: usize = 5;
/// 改动章正文节选字符数
const SOURCE_EXCERPT_CHARS: usize = 2000;
/// 下游章摘要/节选字符数
const DOWNSTREAM_EXCERPT_CHARS: usize = 400;
/// 视为「无处不在实体」的阈值：出现在 ≥50% 场景且 ≥6 个场景
const UBIQUITOUS_MIN_SCENES: usize = 6;
/// 单个影响条目的严重度阈值（score ≥ 该值判 warning）
const WARNING_SCORE: f64 = 6.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CascadeImpact {
    pub id: String,
    pub story_id: String,
    pub batch_id: String,
    pub source_scene_id: String,
    pub source_chapter_number: Option<i32>,
    pub target_scene_id: String,
    pub target_chapter_number: Option<i32>,
    pub impact_score: f64,
    pub impact_kind: String,
    pub severity: String,
    pub entity_ids: Vec<String>,
    pub detail: Option<String>,
    pub evidence: Option<String>,
    pub decision: String,
    pub stale_flag: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// 确定性分析中间结果（未落库）。
#[derive(Debug, Clone, PartialEq)]
pub struct MentionImpact {
    pub target_scene_id: String,
    pub target_chapter_number: Option<i32>,
    pub score: f64,
    pub entity_ids: Vec<String>,
    pub entity_names: Vec<String>,
}

/// LLM 冲突扫描输出。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConflictScanOutput {
    #[serde(default)]
    pub conflicts: Vec<ConflictItem>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ConflictItem {
    #[serde(default)]
    pub target_chapter: Option<i32>,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub source_evidence: String,
    #[serde(default)]
    pub target_evidence: String,
    #[serde(default)]
    pub suggestion: String,
}

// ==================== 确定性影响分析 ====================

struct SceneRow {
    id: String,
    sequence: i32,
}

fn load_scene_rows(conn: &rusqlite::Connection, story_id: &str) -> Vec<SceneRow> {
    let mut stmt = match conn.prepare(
        "SELECT id, sequence_number FROM scenes WHERE story_id = ?1 ORDER BY sequence_number",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(params![story_id], |row| {
        Ok(SceneRow {
            id: row.get(0)?,
            sequence: row.get(1)?,
        })
    });
    rows.map(|r| r.flatten().collect()).unwrap_or_default()
}

fn load_entity_names(
    conn: &rusqlite::Connection,
    entity_ids: &[String],
) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for id in entity_ids {
        if let Ok(name) = conn.query_row(
            "SELECT name FROM kg_entities WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        ) {
            map.insert(id.clone(), name);
        }
    }
    map
}

/// 确定性影响分析：本场景实体 → 下游场景提及聚合。
///
/// - 只统计 `sequence_number > 源场景` 的目标（下游）；
/// - 「无处不在实体」（出现在 ≥50% 场景且 ≥6 个场景的角色/地点）不参与打分，
///   否则主角名一改会命中全书（噪声淹没有效信号）；
/// - 若过滤后无实体可分析（例如全为无处不在实体），退回使用全部实体，
///   但目标数上限收紧为 `MAX_LLM_TARGETS`。
pub fn compute_mention_impacts(
    pool: &DbPool,
    story_id: &str,
    source_scene_id: &str,
) -> Vec<MentionImpact> {
    use crate::creative_engine::cascade_rewriter::EntityMentionRepository;

    let Ok(conn) = pool.get() else {
        return Vec::new();
    };
    let scenes = load_scene_rows(&conn, story_id);
    let Some(source) = scenes.iter().find(|s| s.id == source_scene_id) else {
        return Vec::new();
    };
    let total_scenes = scenes.len().max(1);

    let mention_repo = EntityMentionRepository::new(pool.clone());
    let Ok(source_mentions) = mention_repo.get_by_scene(source_scene_id) else {
        return Vec::new();
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut entity_ids: Vec<String> = Vec::new();
    for mention in &source_mentions {
        if mention.entity_id.trim().is_empty() || !seen.insert(mention.entity_id.clone()) {
            continue;
        }
        entity_ids.push(mention.entity_id.clone());
    }
    if entity_ids.is_empty() {
        return Vec::new();
    }

    // 统计每个实体的场景覆盖度，剔除无处不在实体
    let mut entity_scene_counts: HashMap<String, usize> = HashMap::new();
    let mut per_entity_targets: HashMap<String, HashMap<String, (usize, f64)>> = HashMap::new();
    for entity_id in &entity_ids {
        let Ok(mentions) = mention_repo.get_by_entity(entity_id) else {
            continue;
        };
        let mut scene_ids: HashSet<String> = HashSet::new();
        for mention in mentions {
            if mention.story_id != story_id {
                continue;
            }
            scene_ids.insert(mention.scene_id.clone());
            if mention.scene_id == source_scene_id {
                continue;
            }
            let target = scenes.iter().find(|s| s.id == mention.scene_id);
            let Some(target) = target else { continue };
            if target.sequence <= source.sequence {
                continue;
            }
            let entry = per_entity_targets
                .entry(entity_id.clone())
                .or_default()
                .entry(mention.scene_id.clone())
                .or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += mention.confidence;
        }
        entity_scene_counts.insert(entity_id.clone(), scene_ids.len());
    }

    let ubiquitous: HashSet<&String> = entity_ids
        .iter()
        .filter(|id| {
            entity_scene_counts.get(*id).copied().unwrap_or(0) >= UBIQUITOUS_MIN_SCENES
                && entity_scene_counts.get(*id).copied().unwrap_or(0) * 2 >= total_scenes
        })
        .collect();
    let focused: Vec<&String> = entity_ids
        .iter()
        .filter(|id| !ubiquitous.contains(id))
        .collect();
    let (used_entities, target_cap): (Vec<&String>, usize) = if focused.is_empty() {
        (entity_ids.iter().collect(), MAX_LLM_TARGETS)
    } else {
        (focused, MAX_TARGETS)
    };

    let mut aggregated: HashMap<String, (f64, Vec<String>)> = HashMap::new();
    for entity_id in used_entities {
        let Some(targets) = per_entity_targets.get(entity_id) else {
            continue;
        };
        for (scene_id, (count, conf_sum)) in targets {
            let score = conf_sum * (*count as f64).sqrt();
            let entry = aggregated
                .entry(scene_id.clone())
                .or_insert((0.0, Vec::new()));
            entry.0 += score;
            entry.1.push(entity_id.clone());
        }
    }

    let all_ids: Vec<String> = aggregated
        .values()
        .flat_map(|(_, ids)| ids.clone())
        .collect();
    let names = load_entity_names(&conn, &all_ids);

    let mut impacts: Vec<MentionImpact> = aggregated
        .into_iter()
        .filter_map(|(scene_id, (score, ids))| {
            let scene = scenes.iter().find(|s| s.id == scene_id)?;
            Some(MentionImpact {
                target_scene_id: scene_id,
                target_chapter_number: Some(scene.sequence),
                score,
                entity_names: ids.iter().filter_map(|id| names.get(id).cloned()).collect(),
                entity_ids: ids,
            })
        })
        .collect();
    impacts.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.target_chapter_number.cmp(&b.target_chapter_number))
    });
    impacts.truncate(target_cap);
    impacts
}

// ==================== 落库 / 查询 / 状态迁移 ====================

fn severity_for_score(score: f64) -> &'static str {
    if score >= WARNING_SCORE {
        "warning"
    } else {
        "info"
    }
}

/// 写入一批影响行（按 (batch_id, target_scene_id) upsert）。
pub fn persist_impacts(
    pool: &DbPool,
    story_id: &str,
    batch_id: &str,
    source_scene_id: &str,
    source_chapter_number: Option<i32>,
    impacts: &[MentionImpact],
) -> Result<usize, rusqlite::Error> {
    if impacts.is_empty() {
        return Ok(0);
    }
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    let mut written = 0usize;
    for impact in impacts {
        let detail = if impact.entity_names.is_empty() {
            None
        } else {
            Some(format!(
                "目标章与改动章共享实体：{}",
                impact.entity_names.join("、")
            ))
        };
        let entity_ids = serde_json::to_string(&impact.entity_ids).unwrap_or_else(|_| "[]".into());
        conn.execute(
            "INSERT INTO cascade_impacts \
             (id, story_id, batch_id, source_scene_id, source_chapter_number, target_scene_id, \
              target_chapter_number, impact_score, impact_kind, severity, entity_ids, detail, \
              evidence, decision, stale_flag, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'mention', ?9, ?10, ?11, NULL, 'open', 1, ?12, ?12) \
             ON CONFLICT(batch_id, target_scene_id) DO UPDATE SET \
              impact_score = excluded.impact_score, severity = excluded.severity, \
              entity_ids = excluded.entity_ids, detail = excluded.detail, \
              target_chapter_number = excluded.target_chapter_number, updated_at = excluded.updated_at",
            params![
                uuid::Uuid::new_v4().to_string(),
                story_id,
                batch_id,
                source_scene_id,
                source_chapter_number,
                impact.target_scene_id,
                impact.target_chapter_number,
                impact.score,
                severity_for_score(impact.score),
                entity_ids,
                detail,
                &now
            ],
        )?;
        written += 1;
    }
    Ok(written)
}

pub fn list_impacts(
    pool: &DbPool,
    story_id: &str,
    decision: Option<&str>,
    limit: i64,
) -> Result<Vec<CascadeImpact>, rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let sql = if decision.is_some() {
        "SELECT id, story_id, batch_id, source_scene_id, source_chapter_number, target_scene_id, \
         target_chapter_number, impact_score, impact_kind, severity, entity_ids, detail, evidence, \
         decision, stale_flag, created_at, updated_at \
         FROM cascade_impacts WHERE story_id = ?1 AND decision = ?2 \
         ORDER BY created_at DESC, impact_score DESC LIMIT ?3"
    } else {
        "SELECT id, story_id, batch_id, source_scene_id, source_chapter_number, target_scene_id, \
         target_chapter_number, impact_score, impact_kind, severity, entity_ids, detail, evidence, \
         decision, stale_flag, created_at, updated_at \
         FROM cascade_impacts WHERE story_id = ?1 \
         ORDER BY created_at DESC, impact_score DESC LIMIT ?2"
    };
    let mut stmt = conn.prepare(sql)?;
    let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<CascadeImpact> {
        let entity_ids: String = row.get(10)?;
        Ok(CascadeImpact {
            id: row.get(0)?,
            story_id: row.get(1)?,
            batch_id: row.get(2)?,
            source_scene_id: row.get(3)?,
            source_chapter_number: row.get(4)?,
            target_scene_id: row.get(5)?,
            target_chapter_number: row.get(6)?,
            impact_score: row.get(7)?,
            impact_kind: row.get(8)?,
            severity: row.get(9)?,
            entity_ids: serde_json::from_str(&entity_ids).unwrap_or_default(),
            detail: row.get(11)?,
            evidence: row.get(12)?,
            decision: row.get(13)?,
            stale_flag: row.get::<_, i64>(14)? != 0,
            created_at: row.get(15)?,
            updated_at: row.get(16)?,
        })
    };
    let rows = match decision {
        Some(decision) => stmt.query_map(params![story_id, decision, limit], map_row)?,
        None => stmt.query_map(params![story_id, limit], map_row)?,
    };
    rows.collect()
}

pub fn load_impact(
    pool: &DbPool,
    impact_id: &str,
) -> Result<Option<CascadeImpact>, rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, story_id, batch_id, source_scene_id, source_chapter_number, target_scene_id, \
         target_chapter_number, impact_score, impact_kind, severity, entity_ids, detail, evidence, \
         decision, stale_flag, created_at, updated_at FROM cascade_impacts WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map(params![impact_id], |row| {
        let entity_ids: String = row.get(10)?;
        Ok(CascadeImpact {
            id: row.get(0)?,
            story_id: row.get(1)?,
            batch_id: row.get(2)?,
            source_scene_id: row.get(3)?,
            source_chapter_number: row.get(4)?,
            target_scene_id: row.get(5)?,
            target_chapter_number: row.get(6)?,
            impact_score: row.get(7)?,
            impact_kind: row.get(8)?,
            severity: row.get(9)?,
            entity_ids: serde_json::from_str(&entity_ids).unwrap_or_default(),
            detail: row.get(11)?,
            evidence: row.get(12)?,
            decision: row.get(13)?,
            stale_flag: row.get::<_, i64>(14)? != 0,
            created_at: row.get(15)?,
            updated_at: row.get(16)?,
        })
    })?;
    Ok(rows.next().transpose()?)
}

/// 作者动作：忽略该影响条目。
pub fn mark_impact_decision(
    pool: &DbPool,
    impact_id: &str,
    decision: &str,
) -> Result<usize, rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "UPDATE cascade_impacts SET decision = ?1, updated_at = ?2 WHERE id = ?3",
        params![decision, &now, impact_id],
    )
}

/// 目标场景被重新分析（re-ingest）后，清除其「分析可能已失效」标记。
pub fn clear_stale_for_target_scene(
    pool: &DbPool,
    scene_id: &str,
) -> Result<usize, rusqlite::Error> {
    let conn = pool
        .get()
        .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
    let now = Local::now().to_rfc3339();
    conn.execute(
        "UPDATE cascade_impacts SET stale_flag = 0, updated_at = ?1 \
         WHERE target_scene_id = ?2 AND stale_flag = 1",
        params![&now, scene_id],
    )
}

// ==================== LLM 冲突扫描 ====================

fn truncate_chars(text: &str, max: usize) -> String {
    let t: String = text.chars().take(max).collect();
    if text.chars().count() > max {
        format!("{t}…")
    } else {
        t
    }
}

fn load_chapter_summary(
    conn: &rusqlite::Connection,
    story_id: &str,
    chapter: i32,
) -> Option<String> {
    conn.query_row(
        "SELECT summary_text FROM scene_commits \
         WHERE story_id = ?1 AND chapter_number = ?2 AND summary_text IS NOT NULL \
         ORDER BY created_at DESC LIMIT 1",
        params![story_id, chapter],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .filter(|s| !s.trim().is_empty())
}

/// 构造冲突扫描 prompt（纯函数，便于单测与快照）。
pub fn build_conflict_scan_prompt(
    source_chapter: Option<i32>,
    source_excerpt: &str,
    downstream: &[(i32, String)],
) -> String {
    use crate::prompts::registry::resolve_prompt_default_with_vars;
    let downstream_text = downstream
        .iter()
        .map(|(chapter, text)| format!("【第{chapter}章】\n{text}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut vars = HashMap::new();
    vars.insert(
        "source_chapter".to_string(),
        source_chapter
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".to_string()),
    );
    vars.insert("source_excerpt".to_string(), source_excerpt.to_string());
    vars.insert("downstream".to_string(), downstream_text.clone());
    if let Some(prompt) = resolve_prompt_default_with_vars("cascade_conflict_scan", &vars) {
        return prompt;
    }
    // 资产缺失时的内置兜底（保持与资产同构，防 pipeline 断裂）
    let source_chapter_text = source_chapter
        .map(|c| c.to_string())
        .unwrap_or_else(|| "?".to_string());
    format!(
        "你是长篇小说的连续性审校。上游第{source_chapter_text}章正文刚被作者修改，\
         下面是修改后的节选与若干下游章节的摘要。请找出下游内容与修改后正文之间\
         可能存在的直接矛盾（事实/物品归属/角色知情/时间线/人物状态）。\n\n\
         【修改后的第{source_chapter_text}章节选】\n{source_excerpt}\n\n\
         【下游章节】\n{downstream_text}\n\n\
         仅输出一个合法 JSON 对象（不要 markdown 围栏、不要注释、不要尾随逗号）：\n\
         {{\"conflicts\": [{{\"target_chapter\": 7, \"severity\": \"warning\", \
         \"description\": \"下游第7章称玉佩在林晚手中，与修改后的第3章矛盾\", \
         \"source_evidence\": \"修改后正文中的相关句子\", \"target_evidence\": \"下游摘要中的相关句子\", \
         \"suggestion\": \"建议改法\"}}]}}\n\
         只报告有直接证据的矛盾；没有则返回 {{\"conflicts\": []}}。不要臆测。"
    )
}

/// 解析冲突扫描输出（容错：容忍围栏/尾随逗号/思考链）。
pub fn parse_conflict_scan(raw: &str) -> ConflictScanOutput {
    match crate::narrative::extract_and_sanitize_json(raw)
        .ok()
        .and_then(|json| serde_json::from_str::<ConflictScanOutput>(&json).ok())
    {
        Some(output) => output,
        None => ConflictScanOutput::default(),
    }
}

/// LLM 冲突扫描：读取本批影响行 + 下游摘要，产出冲突并更新影响行。
/// 返回 (冲突条数, 严重冲突条数)。失败仅告警（可降级）。
pub async fn scan_conflicts_with_llm(
    pool: &DbPool,
    app: &AppHandle,
    story_id: &str,
    batch_id: &str,
    source_scene_id: &str,
) -> (usize, usize) {
    let (source_chapter, source_content, source_seq) = {
        let Ok(conn) = pool.get() else {
            return (0, 0);
        };
        match conn.query_row(
            "SELECT sequence_number, COALESCE(content, '') FROM scenes WHERE id = ?1",
            params![source_scene_id],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, String>(1)?)),
        ) {
            Ok((seq, content)) => (Some(seq), content, seq),
            Err(_) => return (0, 0),
        }
    };
    if source_content.trim().is_empty() {
        return (0, 0);
    }

    // 本批影响行（仅确定性分析产生的 mention 行，按分数取前 N）
    let targets: Vec<(String, i32, f64)> = {
        let Ok(conn) = pool.get() else {
            return (0, 0);
        };
        let mut stmt = match conn.prepare(
            "SELECT target_scene_id, target_chapter_number, impact_score FROM cascade_impacts \
             WHERE batch_id = ?1 AND decision = 'open' AND target_chapter_number IS NOT NULL \
             ORDER BY impact_score DESC LIMIT ?2",
        ) {
            Ok(stmt) => stmt,
            Err(_) => return (0, 0),
        };
        let rows = stmt.query_map(params![batch_id, MAX_LLM_TARGETS as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)?,
                row.get::<_, f64>(2)?,
            ))
        });
        match rows {
            Ok(rows) => rows.flatten().collect(),
            Err(_) => return (0, 0),
        }
    };
    if targets.is_empty() {
        return (0, 0);
    }

    let downstream: Vec<(i32, String)> = {
        let Ok(conn) = pool.get() else {
            return (0, 0);
        };
        targets
            .iter()
            .filter(|(_, chapter, _)| *chapter > source_seq)
            .map(|(scene_id, chapter, _)| {
                let text = load_chapter_summary(&conn, story_id, *chapter).or_else(|| {
                    conn.query_row(
                        "SELECT COALESCE(content, '') FROM scenes WHERE id = ?1",
                        params![scene_id],
                        |row| row.get::<_, String>(0),
                    )
                    .ok()
                });
                (
                    *chapter,
                    truncate_chars(text.as_deref().unwrap_or(""), DOWNSTREAM_EXCERPT_CHARS),
                )
            })
            .filter(|(_, text)| !text.trim().is_empty())
            .collect()
    };
    if downstream.is_empty() {
        return (0, 0);
    }

    let prompt = build_conflict_scan_prompt(
        source_chapter,
        &truncate_chars(&source_content, SOURCE_EXCERPT_CHARS),
        &downstream,
    );
    let llm = LlmService::new(app.clone());
    let response = match llm
        .generate_for_task(
            TaskType::Analysis,
            prompt,
            Some(1200),
            Some(0.2),
            Some(CASCADE_SCAN_LABEL),
        )
        .await
    {
        Ok(response) => response,
        Err(e) => {
            log::warn!("[cascade_impact] 冲突扫描 LLM 失败（降级跳过）: {}", e);
            return (0, 0);
        }
    };
    let output = parse_conflict_scan(&response.content);
    if output.conflicts.is_empty() {
        return (0, 0);
    }

    let now = Local::now().to_rfc3339();
    let mut applied = 0usize;
    let mut critical = 0usize;
    if let Ok(conn) = pool.get() {
        for conflict in &output.conflicts {
            let Some(target_chapter) = conflict.target_chapter else {
                continue;
            };
            let Some((target_scene_id, _, score)) = targets
                .iter()
                .find(|(_, chapter, _)| *chapter == target_chapter)
            else {
                continue;
            };
            let severity = match conflict.severity.as_str() {
                "critical" => "critical",
                "info" => "info",
                _ => "warning",
            };
            let evidence = [
                conflict.source_evidence.trim(),
                conflict.target_evidence.trim(),
            ]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ｜ ");
            let detail = if conflict.suggestion.trim().is_empty() {
                conflict.description.trim().to_string()
            } else {
                format!(
                    "{}（建议：{}）",
                    conflict.description.trim(),
                    conflict.suggestion.trim()
                )
            };
            let updated = conn.execute(
                "UPDATE cascade_impacts SET impact_kind = 'conflict', severity = ?1, \
                 detail = ?2, evidence = ?3, impact_score = MAX(impact_score, ?4), updated_at = ?5 \
                 WHERE batch_id = ?6 AND target_scene_id = ?7",
                params![
                    severity,
                    detail,
                    if evidence.is_empty() {
                        None
                    } else {
                        Some(evidence)
                    },
                    score + 5.0,
                    &now,
                    batch_id,
                    target_scene_id
                ],
            );
            match updated {
                Ok(n) if n > 0 => {
                    applied += 1;
                    if severity == "critical" {
                        critical += 1;
                    }
                }
                _ => {}
            }
        }
    }
    (applied, critical)
}

// ==================== 编排与后台入口 ====================

/// 完整分析一轮：确定性影响 + LLM 冲突扫描 + 事件推送。返回 (影响数, 冲突数)。
pub async fn analyze_after_scene_ingest(
    pool: &DbPool,
    app: &AppHandle,
    story_id: &str,
    scene_id: &str,
) -> (usize, usize) {
    let batch_id = uuid::Uuid::new_v4().to_string();
    let source_chapter = pool.get().ok().and_then(|conn| {
        conn.query_row(
            "SELECT sequence_number FROM scenes WHERE id = ?1",
            params![scene_id],
            |row| row.get::<_, i32>(0),
        )
        .ok()
    });

    let impacts = compute_mention_impacts(pool, story_id, scene_id);
    if impacts.is_empty() {
        // 没有下游影响：本场景作为目标的旧标记也一并清除
        let _ = clear_stale_for_target_scene(pool, scene_id);
        return (0, 0);
    }
    if let Err(e) = persist_impacts(
        pool,
        story_id,
        &batch_id,
        scene_id,
        source_chapter,
        &impacts,
    ) {
        log::warn!("[cascade_impact] 写入影响报告失败: {}", e);
        return (0, 0);
    }

    let (conflicts, _critical) =
        scan_conflicts_with_llm(pool, app, story_id, &batch_id, scene_id).await;

    let total = impacts.len();
    let conflict_count = if conflicts > 0 {
        conflicts
    } else {
        impacts
            .iter()
            .filter(|i| severity_for_score(i.score) != "info")
            .count()
    };
    let _ = StateSync::emit_cascade_impact_detected(
        app,
        story_id,
        &batch_id,
        scene_id,
        source_chapter,
        total,
        conflict_count,
    );
    let _ = app.emit(
        EVENT_CASCADE_CONFLICT_SCAN_DONE,
        serde_json::json!({
            "story_id": story_id,
            "batch_id": batch_id,
            "source_scene_id": scene_id,
            "count": total,
            "conflicts": conflicts,
        }),
    );
    // 本场景自身的陈旧标记清除（它已被重新分析）
    let _ = clear_stale_for_target_scene(pool, scene_id);
    log::info!(
        "[cascade_impact] 场景 {} 改稿影响分析完成：{} 个目标，{} 条冲突",
        scene_id,
        total,
        conflicts
    );
    (total, conflicts)
}

/// 后台入口：在场景 re-ingest 完成后调用（scene_service 挂接点）。
/// 受全局后台 LLM 闸门约束；失败仅告警不影响主流程。
pub fn spawn_analyze_after_scene_ingest(
    app: AppHandle,
    pool: DbPool,
    story_id: String,
    scene_id: String,
) {
    tauri::async_runtime::spawn(async move {
        let bg_permit = crate::concurrency::BACKGROUND_LLM_SEMAPHORE.acquire().await;
        if bg_permit.is_err() {
            log::warn!(
                "[cascade_impact] 场景 {}: 获取后台闸门失败，跳过影响分析",
                scene_id
            );
            return;
        }
        let _bg_permit = bg_permit.unwrap();
        analyze_after_scene_ingest(&pool, &app, &story_id, &scene_id).await;
    });
}

/// 由影响条目触发级联改写任务（作者显式动作）。
///
/// 从 KG 解析实体名/类型（修复旧的 `entity_name = entity_id` TODO），
/// `after_json` 携带「上游章正文已修改」的说明与节选，让既有
/// RewriteEngine 的改写 prompt 获得变更上下文。
pub fn build_change_events_for_impact(
    pool: &DbPool,
    impact: &CascadeImpact,
) -> Vec<EntityChangeEvent> {
    let (source_excerpt, source_seq) = {
        let Ok(conn) = pool.get() else {
            return Vec::new();
        };
        conn.query_row(
            "SELECT COALESCE(content, ''), sequence_number FROM scenes WHERE id = ?1",
            params![impact.source_scene_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
        )
        .map(|(content, seq)| (truncate_chars(&content, 500), Some(seq)))
        .unwrap_or_default()
    };
    let now = Local::now().to_rfc3339();
    let mut events = Vec::new();
    for entity_id in &impact.entity_ids {
        let (name, entity_type) = {
            let Ok(conn) = pool.get() else {
                continue;
            };
            conn.query_row(
                "SELECT name, entity_type FROM kg_entities WHERE id = ?1",
                params![entity_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .unwrap_or_else(|_| (entity_id.clone(), "Concept".to_string()))
        };
        let after_json = serde_json::json!({
            "note": format!(
                "上游第{}章正文已被作者修改；本场景与该章共享实体「{}」，涉及该实体的事实可能已变化。",
                source_seq.map(|s| s.to_string()).unwrap_or_else(|| "?".into()),
                name
            ),
            "source_scene_id": impact.source_scene_id,
            "source_chapter": source_seq,
            "excerpt": source_excerpt,
        })
        .to_string();
        events.push(EntityChangeEvent {
            story_id: impact.story_id.clone(),
            entity_id: entity_id.clone(),
            entity_type,
            entity_name: name,
            change_type: ChangeType::AttributeModified,
            before_json: "{}".to_string(),
            after_json,
            changed_fields: vec!["content".to_string()],
            timestamp: now.clone(),
        });
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::create_test_pool;

    fn seed_story_scene(pool: &DbPool, sequence: i32, content: &str) -> (String, String) {
        let story_id = uuid::Uuid::new_v4().to_string();
        let scene_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, '级联测试', ?2, ?2)",
            params![&story_id, &now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, content, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![&scene_id, &story_id, sequence, content, &now],
        )
        .unwrap();
        (story_id, scene_id)
    }

    fn add_scene(pool: &DbPool, story_id: &str, sequence: i32, content: &str) -> String {
        let scene_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, content, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![&scene_id, story_id, sequence, content, &now],
        )
        .unwrap();
        scene_id
    }

    fn add_entity(pool: &DbPool, story_id: &str, name: &str) -> String {
        let entity_id = uuid::Uuid::new_v4().to_string();
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO kg_entities (id, story_id, name, entity_type, first_seen, last_updated) \
             VALUES (?1, ?2, ?3, 'Item', ?4, ?4)",
            params![&entity_id, story_id, name, &now],
        )
        .unwrap();
        entity_id
    }

    fn add_mention(pool: &DbPool, story_id: &str, scene_id: &str, entity_id: &str) {
        use crate::creative_engine::cascade_rewriter::{
            models::EntityMention, EntityMentionRepository,
        };
        let repo = EntityMentionRepository::new(pool.clone());
        let now = Local::now().to_rfc3339();
        repo.create(&EntityMention {
            id: uuid::Uuid::new_v4().to_string(),
            story_id: story_id.to_string(),
            scene_id: scene_id.to_string(),
            entity_id: entity_id.to_string(),
            entity_type: "Item".to_string(),
            start_pos: 0,
            end_pos: 4,
            mention_text: "玉佩".to_string(),
            confidence: 1.0,
            created_at: now.clone(),
            updated_at: now,
        })
        .unwrap();
    }

    /// 帖主测试③契约：改第 3 章 → 第 7 章（共享实体）产生影响行 + stale 标记，
    /// 第 9 章（不共享实体）不得被牵连。
    #[test]
    fn test_edit_early_chapter_creates_downstream_impacts_only_for_shared_entities() {
        let pool = create_test_pool().unwrap();
        let (story_id, ch3) = seed_story_scene(&pool, 3, "她系好羊脂玉佩。");
        let ch7 = add_scene(&pool, &story_id, 7, "他掏出羊脂玉佩。");
        let ch9 = add_scene(&pool, &story_id, 9, "两人对坐无言。");

        let jade = add_entity(&pool, &story_id, "羊脂玉佩");
        let other = add_entity(&pool, &story_id, "青铜灯");
        let mirror = add_entity(&pool, &story_id, "铜镜");
        add_mention(&pool, &story_id, &ch3, &jade);
        add_mention(&pool, &story_id, &ch3, &other);
        add_mention(&pool, &story_id, &ch7, &jade);
        // 第9章的实体与第3章无交集——不得被牵连
        add_mention(&pool, &story_id, &ch9, &mirror);

        let impacts = compute_mention_impacts(&pool, &story_id, &ch3);
        assert_eq!(impacts.len(), 1, "只有共享玉佩的第7章应受影响: {impacts:?}");
        assert_eq!(impacts[0].target_scene_id, ch7);
        assert_eq!(impacts[0].target_chapter_number, Some(7));
        assert!(impacts[0].entity_names.contains(&"羊脂玉佩".to_string()));

        let batch = "batch-1";
        persist_impacts(&pool, &story_id, batch, &ch3, Some(3), &impacts).unwrap();
        let rows = list_impacts(&pool, &story_id, Some("open"), 50).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].stale_flag, "改稿后目标章应标记分析可能已失效");
        assert_eq!(rows[0].severity, "info");
        assert_eq!(rows[0].decision, "open");

        // 前置章节不参与（不产生 ch3 之前的目标）
        let ch1 = add_scene(&pool, &story_id, 1, "玉佩初见。");
        add_mention(&pool, &story_id, &ch1, &jade);
        let impacts2 = compute_mention_impacts(&pool, &story_id, &ch3);
        assert!(
            impacts2.iter().all(|i| i.target_chapter_number != Some(1)),
            "上游章节不得计入下游影响: {impacts2:?}"
        );
    }

    #[test]
    fn test_ignore_and_stale_clear_decision_transitions() {
        let pool = create_test_pool().unwrap();
        let (story_id, ch3) = seed_story_scene(&pool, 3, "正文");
        let ch7 = add_scene(&pool, &story_id, 7, "下游");
        let item = add_entity(&pool, &story_id, "玉佩");
        add_mention(&pool, &story_id, &ch3, &item);
        add_mention(&pool, &story_id, &ch7, &item);

        let impacts = compute_mention_impacts(&pool, &story_id, &ch3);
        persist_impacts(&pool, &story_id, "b1", &ch3, Some(3), &impacts).unwrap();
        let row = list_impacts(&pool, &story_id, None, 10).unwrap().remove(0);

        assert_eq!(mark_impact_decision(&pool, &row.id, "ignored").unwrap(), 1);
        let ignored = list_impacts(&pool, &story_id, Some("ignored"), 10).unwrap();
        assert_eq!(ignored.len(), 1);

        assert_eq!(clear_stale_for_target_scene(&pool, &ch7).unwrap(), 1);
        let after = load_impact(&pool, &row.id).unwrap().unwrap();
        assert!(!after.stale_flag, "重跑分析后 stale 应清除");
        assert_eq!(after.decision, "ignored", "决策不因 stale 清除而改变");
    }

    #[test]
    fn test_ubiquitous_entity_is_filtered_out() {
        let pool = create_test_pool().unwrap();
        // 主角出现在全部 10 个场景；玉佩只在 3 章与 7 章
        let (story_id, ch3) = seed_story_scene(&pool, 3, "主角");
        let mut scenes = vec![ch3.clone()];
        for seq in (4..=12).filter(|s| *s != 7) {
            scenes.push(add_scene(&pool, &story_id, seq, "主角在场"));
        }
        let hero = add_entity(&pool, &story_id, "主角");
        let jade = add_entity(&pool, &story_id, "玉佩");
        for scene in &scenes {
            add_mention(&pool, &story_id, scene, &hero);
        }
        add_mention(&pool, &story_id, &ch3, &jade);
        let ch7 = add_scene(&pool, &story_id, 7, "玉佩");
        add_mention(&pool, &story_id, &ch7, &jade);

        let impacts = compute_mention_impacts(&pool, &story_id, &ch3);
        assert!(
            impacts.iter().any(|i| i.target_chapter_number == Some(7)),
            "非普遍实体（玉佩）应保留: {impacts:?}"
        );
        assert!(
            impacts
                .iter()
                .all(|i| i.entity_names != vec!["主角".to_string()])
                || impacts.len() < 10,
            "无处不在实体不应命中所有场景: {impacts:?}"
        );
    }

    #[test]
    fn test_parse_conflict_scan_tolerates_fences_and_trailing_commas() {
        let raw = "```json\n{\n  \"conflicts\": [\n    {\n      \"target_chapter\": 7,\n      \"severity\": \"critical\",\n      \"description\": \"物品归属矛盾\",\n      \"source_evidence\": \"第3章：玉佩在林晚手中\",\n      \"target_evidence\": \"第7章：苏亦铁掏出玉佩\",\n      \"suggestion\": \"改为林晚递出\",\n    },\n  ],\n}\n```";
        let parsed = parse_conflict_scan(raw);
        assert_eq!(parsed.conflicts.len(), 1);
        assert_eq!(parsed.conflicts[0].target_chapter, Some(7));
        assert_eq!(parsed.conflicts[0].severity, "critical");
    }

    #[test]
    fn test_parse_conflict_scan_returns_empty_on_garbage() {
        assert!(parse_conflict_scan("模型没有输出 JSON")
            .conflicts
            .is_empty());
    }

    #[test]
    fn test_build_change_events_resolves_entity_names_from_kg() {
        let pool = create_test_pool().unwrap();
        let (story_id, ch3) = seed_story_scene(&pool, 3, "她系好羊脂玉佩，抬头看见了他。");
        let ch7 = add_scene(&pool, &story_id, 7, "他掏出羊脂玉佩。");
        let jade = add_entity(&pool, &story_id, "羊脂玉佩");
        add_mention(&pool, &story_id, &ch3, &jade);
        add_mention(&pool, &story_id, &ch7, &jade);

        let impacts = compute_mention_impacts(&pool, &story_id, &ch3);
        persist_impacts(&pool, &story_id, "b1", &ch3, Some(3), &impacts).unwrap();
        let row = list_impacts(&pool, &story_id, None, 10).unwrap().remove(0);

        let events = build_change_events_for_impact(&pool, &row);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity_name, "羊脂玉佩", "实体名必须从 KG 解析");
        assert_eq!(events[0].entity_type, "Item");
        assert!(events[0].after_json.contains("上游第3章"));
        assert!(events[0].after_json.contains("羊脂玉佩"));
    }
}
