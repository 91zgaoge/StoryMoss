//! 资产回流桥：把 IngestPipeline 的内容分析结果（ContentAnalysis）单向同步到
//! 续写 writer 实际读取的生产资产表——characters / character_relationships /
//! world_buildings / scenes.outline_content / scenes.characters_present /
//! story_outlines。
//!
//! 设计要点：
//! - upsert 模式复用 agency/materialize.rs 的 UPDATE-then-INSERT 思路；
//! - 源感知合并：仅填空字段，或精炼 source IN
//!   ('ingest','agency','auto_placeholder')
//!   的行；source='user_created'/'manual' 的既有字段一律保留（用户编辑优先）；
//! - 新角色自动注册到 characters（source='ingest', is_auto_generated=1），修复
//!   此前 persist_character_states 对未注册角色直接丢弃的问题；
//! - 桥接单向：正文 → 生产资产表，不回写 kg 记忆层；
//! - 任何单条失败只 log::warn，不中断整体 ingest。

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use rusqlite::params;

use crate::{
    db::DbPool,
    memory::ingest::{AnalyzedEntity, AnalyzedRelation, ContentAnalysis, SceneOutlineDelta},
};

/// 进程内 per-story 互斥锁注册表。characters 表无 UNIQUE(story_id,name)
/// 约束，同一 story 的并发 ingest（scene_service 每场景 spawn 一个）会对
/// 同名新角色产生 SELECT-then-INSERT 的 TOCTOU 竞争、插入重复行；本应用
/// 是单进程桌面应用，进程内锁即可保证同 story 的同步串行执行。
static STORY_LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();

fn story_lock(story_id: &str) -> Arc<Mutex<()>> {
    let registry = STORY_LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = registry.lock().unwrap_or_else(|p| p.into_inner());
    map.entry(story_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn now() -> String {
    chrono::Local::now().to_rfc3339()
}

/// 允许被自动精炼（覆盖非空字段）的来源；其余来源（user_created/manual
/// 等）只填空。
const REFINABLE_SOURCES: [&str; 3] = ["ingest", "agency", "auto_placeholder"];

pub(crate) fn is_refinable(source: Option<&str>) -> bool {
    source
        .map(|s| REFINABLE_SOURCES.contains(&s))
        .unwrap_or(false)
}

/// 非空字符串转 Option（trim 后为空视为无值）
fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn opt_str(s: &Option<String>) -> Option<&str> {
    s.as_deref()
}

/// 校验 LLM 给出的年龄：非正数或 >150 视为无效（不落库）
fn sanitize_age(age: Option<i32>) -> Option<i32> {
    age.filter(|a| (1..=150).contains(a))
}

/// 源感知文本合并：新值为空 → 保留旧值；旧值为空或行可精炼 →
/// 取新值；否则保留旧值。
fn merge_text(existing: &Option<String>, new: &str, refinable: bool) -> Option<String> {
    let new = match non_empty(new) {
        Some(v) => v,
        None => return existing.clone(),
    };
    let existing_filled = existing
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if refinable || !existing_filled {
        Some(new)
    } else {
        existing.clone()
    }
}

/// 关系强度合并：emotional_intensity 列有 DEFAULT 0.5（V124），读回永为
/// Some(0.5)，单纯 `.or(new)` 是死分支、既有行强度会永远停在无意义的
/// 0.5。当既有值等于默认 0.5、且对应 emotional_bond 为空（该行从未被
/// 真正赋过强度）时，允许新值覆盖；其余情况保留既有值。
fn merge_intensity(existing: Option<f64>, bond: &Option<String>, new: f64) -> Option<f64> {
    let bond_filled = bond
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    match existing {
        Some(v) if (v - 0.5).abs() < f64::EPSILON && !bond_filled => Some(new),
        other => other.or(Some(new)),
    }
}

/// 把 ContentAnalysis 同步到生产资产表，返回写入/更新的条目数。
pub fn sync_assets_from_analysis(
    pool: &DbPool,
    story_id: &str,
    scene_id: Option<&str>,
    analysis: &ContentAnalysis,
) -> usize {
    // 整个函数持有 per-story 锁：characters 的 SELECT-then-INSERT 依赖
    // 同 story 串行，否则并发 ingest 会插入同名重复角色、关系挂错 id。
    let lock = story_lock(story_id);
    let _guard = lock.lock().unwrap_or_else(|p| p.into_inner());
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("[AssetBridge] pool 获取失败: {}", e);
            return 0;
        }
    };
    let mut count = 0usize;
    count += sync_characters(&conn, story_id, &analysis.entities);
    count += sync_relationships(&conn, story_id, &analysis.relationships);
    if let Some(wb) = &analysis.world_building {
        count += sync_world_building(&conn, story_id, wb);
    }
    if let (Some(sid), Some(so)) = (scene_id, &analysis.scene_outline) {
        count += sync_scene_outline(&conn, story_id, sid, so);
    }
    if let Some(sd) = &analysis.story_delta {
        count += sync_story_delta(&conn, story_id, sd);
    }
    count
}

// ==================== 角色 ====================

fn sync_characters(
    conn: &rusqlite::Connection,
    story_id: &str,
    entities: &[AnalyzedEntity],
) -> usize {
    let mut count = 0usize;
    let ts = now();
    for e in entities
        .iter()
        .filter(|e| e.entity_type.eq_ignore_ascii_case("Character"))
    {
        let name = e.name.trim();
        if name.is_empty() {
            continue;
        }
        // 读取既有行（含 source，决定合并策略）
        let existing = conn
            .query_row(
                "SELECT id, background, personality, goals, appearance, gender, age, \
                 emotional_core, emotional_trigger, emotional_wound, emotional_need, source \
                 FROM characters WHERE story_id = ?1 AND name = ?2",
                params![story_id, name],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<i32>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                    ))
                },
            )
            .ok();

        match existing {
            Some((
                id,
                background,
                personality,
                goals,
                appearance,
                gender,
                age,
                emo_core,
                emo_trigger,
                emo_wound,
                emo_need,
                source,
            )) => {
                let refinable = is_refinable(source.as_deref());
                let m_background = merge_text(&background, &e.background, refinable);
                let m_personality = merge_text(&personality, &e.personality, refinable);
                let m_goals = merge_text(&goals, &e.goals, refinable);
                let m_appearance = merge_text(&appearance, &e.appearance, refinable);
                let m_gender = merge_text(&gender, &e.gender, refinable);
                let m_age = match sanitize_age(e.age) {
                    Some(a) if refinable || age.is_none() => Some(a),
                    _ => age,
                };
                let m_emo_core = merge_text(&emo_core, &e.emotional_core, refinable);
                let m_emo_trigger = merge_text(&emo_trigger, &e.emotional_trigger, refinable);
                let m_emo_wound = merge_text(&emo_wound, &e.emotional_wound, refinable);
                let m_emo_need = merge_text(&emo_need, &e.emotional_need, refinable);

                let changed = m_background != background
                    || m_personality != personality
                    || m_goals != goals
                    || m_appearance != appearance
                    || m_gender != gender
                    || m_age != age
                    || m_emo_core != emo_core
                    || m_emo_trigger != emo_trigger
                    || m_emo_wound != emo_wound
                    || m_emo_need != emo_need;
                if !changed {
                    continue;
                }
                match conn.execute(
                    "UPDATE characters SET background = ?2, personality = ?3, goals = ?4, \
                     appearance = ?5, gender = ?6, age = ?7, emotional_core = ?8, \
                     emotional_trigger = ?9, emotional_wound = ?10, emotional_need = ?11, \
                     updated_at = ?12 WHERE id = ?1",
                    params![
                        id,
                        opt_str(&m_background),
                        opt_str(&m_personality),
                        opt_str(&m_goals),
                        opt_str(&m_appearance),
                        opt_str(&m_gender),
                        m_age,
                        opt_str(&m_emo_core),
                        opt_str(&m_emo_trigger),
                        opt_str(&m_emo_wound),
                        opt_str(&m_emo_need),
                        ts
                    ],
                ) {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("[AssetBridge] 更新角色 {} 失败: {}", name, e),
                }
            }
            None => {
                // 新角色自动注册（source='ingest'），修复此前未注册角色被
                // persist_character_states 丢弃的问题
                let id = uuid::Uuid::new_v4().to_string();
                match conn.execute(
                    "INSERT INTO characters (id, story_id, name, background, personality, goals, \
                     appearance, gender, age, emotional_core, emotional_trigger, emotional_wound, \
                     emotional_need, source, is_auto_generated, created_at, updated_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'ingest', 1, ?14, ?15)",
                    params![
                        id,
                        story_id,
                        name,
                        opt_str(&non_empty(&e.background)),
                        opt_str(&non_empty(&e.personality)),
                        opt_str(&non_empty(&e.goals)),
                        opt_str(&non_empty(&e.appearance)),
                        opt_str(&non_empty(&e.gender)),
                        sanitize_age(e.age),
                        opt_str(&non_empty(&e.emotional_core)),
                        opt_str(&non_empty(&e.emotional_trigger)),
                        opt_str(&non_empty(&e.emotional_wound)),
                        opt_str(&non_empty(&e.emotional_need)),
                        ts,
                        ts
                    ],
                ) {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!("[AssetBridge] 注册新角色 {} 失败: {}", name, e),
                }
            }
        }
    }
    count
}

// ==================== 关系 ====================

/// 按角色名查 characters 表 id（与 materialize.rs 同款）
fn find_character_id_by_name(
    conn: &rusqlite::Connection,
    story_id: &str,
    name: &str,
) -> Option<String> {
    conn.query_row(
        "SELECT id FROM characters WHERE story_id = ?1 AND name = ?2 LIMIT 1",
        params![story_id, name],
        |r| r.get(0),
    )
    .ok()
}

fn sync_relationships(
    conn: &rusqlite::Connection,
    story_id: &str,
    relations: &[AnalyzedRelation],
) -> usize {
    let mut count = 0usize;
    let ts = now();
    for rel in relations {
        let source_name = rel.source.trim();
        let target_name = rel.target.trim();
        if source_name.is_empty() || target_name.is_empty() || rel.relation_type.trim().is_empty() {
            continue;
        }
        let source_id = find_character_id_by_name(conn, story_id, source_name);
        let target_id = find_character_id_by_name(conn, story_id, target_name);
        let (Some(sid), Some(tid)) = (source_id, target_id) else {
            log::warn!(
                "[AssetBridge] 关系 {} -> {} 找不到角色，跳过",
                source_name,
                target_name
            );
            continue;
        };

        // 按 (story_id, source, target) 去重：已存在则只填空字段，不覆盖
        let existing = conn
            .query_row(
                "SELECT id, description, dynamic, emotional_bond, emotional_intensity, \
                 reverse_emotional_bond, reverse_emotional_intensity \
                 FROM character_relationships \
                 WHERE story_id = ?1 AND source_character_id = ?2 AND target_character_id = ?3",
                params![story_id, sid, tid],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<f64>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<f64>>(6)?,
                    ))
                },
            )
            .ok();

        match existing {
            Some((id, description, dynamic, bond, intensity, rev_bond, rev_intensity)) => {
                // 关系表无 source 列，保守起见只填空，不覆盖任何既有值
                let m_description = merge_text(&description, &rel.description, false);
                let m_dynamic = merge_text(&dynamic, &rel.dynamic, false);
                let m_bond = merge_text(&bond, &rel.emotional_bond, false);
                let m_rev_bond = merge_text(&rev_bond, &rel.reverse_emotional_bond, false);
                let m_intensity = merge_intensity(intensity, &bond, rel.emotional_intensity as f64);
                let m_rev_intensity = merge_intensity(
                    rev_intensity,
                    &rev_bond,
                    rel.reverse_emotional_intensity as f64,
                );
                let changed = m_description != description
                    || m_dynamic != dynamic
                    || m_bond != bond
                    || m_rev_bond != rev_bond
                    || m_intensity != intensity
                    || m_rev_intensity != rev_intensity;
                if !changed {
                    continue;
                }
                match conn.execute(
                    "UPDATE character_relationships SET description = ?2, dynamic = ?3, \
                     emotional_bond = ?4, emotional_intensity = ?5, reverse_emotional_bond = ?6, \
                     reverse_emotional_intensity = ?7 WHERE id = ?1",
                    params![
                        id,
                        opt_str(&m_description),
                        opt_str(&m_dynamic),
                        opt_str(&m_bond),
                        m_intensity,
                        opt_str(&m_rev_bond),
                        m_rev_intensity
                    ],
                ) {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!(
                        "[AssetBridge] 更新关系 {} -> {} 失败: {}",
                        source_name,
                        target_name,
                        e
                    ),
                }
            }
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                match conn.execute(
                    "INSERT INTO character_relationships (id, story_id, source_character_id, \
                     target_character_id, relationship_type, description, dynamic, emotional_bond, \
                     emotional_intensity, reverse_emotional_bond, reverse_emotional_intensity, \
                     created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                    params![
                        id,
                        story_id,
                        sid,
                        tid,
                        rel.relation_type.trim(),
                        opt_str(&non_empty(&rel.description)),
                        opt_str(&non_empty(&rel.dynamic)),
                        opt_str(&non_empty(&rel.emotional_bond)),
                        rel.emotional_intensity as f64,
                        opt_str(&non_empty(&rel.reverse_emotional_bond)),
                        rel.reverse_emotional_intensity as f64,
                        ts
                    ],
                ) {
                    Ok(_) => count += 1,
                    Err(e) => log::warn!(
                        "[AssetBridge] 写入关系 {} -> {} 失败: {}",
                        source_name,
                        target_name,
                        e
                    ),
                }
            }
        }
    }
    count
}

// ==================== 世界观 ====================

/// 把自由文本 rule_type 映射到 WorldRule/RuleType 的序列化形式
fn map_rule_type(rule_type: &str) -> &'static str {
    match rule_type.trim().to_lowercase().as_str() {
        "magic" | "魔法" => "Magic",
        "technology" | "科技" => "Technology",
        "social" | "社会" => "Social",
        "physical" | "物理" => "Physical",
        "biological" | "生物" => "Biological",
        "historical" | "历史" => "Historical",
        "cultural" | "文化" => "Cultural",
        _ => "Custom",
    }
}

fn sync_world_building(
    conn: &rusqlite::Connection,
    story_id: &str,
    wb: &crate::memory::ingest::WbDelta,
) -> usize {
    let has_delta = !wb.concept.trim().is_empty()
        || !wb.rules.is_empty()
        || !wb.history.trim().is_empty()
        || !wb.cultures.is_empty();
    if !has_delta {
        return 0;
    }

    let new_rules_json: Vec<serde_json::Value> = wb
        .rules
        .iter()
        .filter(|r| !r.name.trim().is_empty())
        .map(|r| {
            serde_json::json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "name": r.name.trim(),
                "description": non_empty(&r.description),
                "rule_type": map_rule_type(&r.rule_type),
                "importance": r.importance,
            })
        })
        .collect();
    let new_cultures_json: Vec<serde_json::Value> = wb
        .cultures
        .iter()
        .filter(|c| !c.name.trim().is_empty())
        .map(|c| {
            serde_json::json!({
                "name": c.name.trim(),
                "description": c.description,
                "customs": c.customs,
                "values": c.values,
            })
        })
        .collect();

    let existing = conn
        .query_row(
            "SELECT id, concept, rules, history, cultures, source FROM world_buildings \
             WHERE story_id = ?1",
            params![story_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .ok();

    let ts = now();
    match existing {
        None => {
            // 无行则建（concept NOT NULL，允许空串占位）
            let id = uuid::Uuid::new_v4().to_string();
            let rules_str = serde_json::to_string(&new_rules_json).unwrap_or_else(|_| "[]".into());
            let cultures_str =
                serde_json::to_string(&new_cultures_json).unwrap_or_else(|_| "[]".into());
            match conn.execute(
                "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, \
                 source, is_auto_generated, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ingest', 1, ?7, ?8)",
                params![
                    id,
                    story_id,
                    wb.concept.trim(),
                    rules_str,
                    opt_str(&non_empty(&wb.history)),
                    cultures_str,
                    ts,
                    ts
                ],
            ) {
                Ok(_) => 1,
                Err(e) => {
                    log::warn!("[AssetBridge] 写入世界观失败: {}", e);
                    0
                }
            }
        }
        Some((id, concept, rules, history, cultures, source)) => {
            let refinable = is_refinable(source.as_deref());

            // concept：不覆盖用户值；可精炼行或空值时才填
            let concept_filled = !concept.trim().is_empty();
            let m_concept = if !wb.concept.trim().is_empty() && (refinable || !concept_filled) {
                wb.concept.trim().to_string()
            } else {
                concept.clone()
            };

            // rules：按 name 去重追加（既有规则内容不覆盖）
            let mut rules_vec: Vec<serde_json::Value> =
                serde_json::from_str(rules.as_deref().unwrap_or("[]")).unwrap_or_default();
            let mut changed = m_concept != concept;
            for nr in &new_rules_json {
                let new_name = nr.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let dup = rules_vec
                    .iter()
                    .any(|r| r.get("name").and_then(|v| v.as_str()) == Some(new_name));
                if !dup {
                    rules_vec.push(nr.clone());
                    changed = true;
                }
            }

            // history：仅空时填
            let m_history = merge_text(&history, &wb.history, false);
            if m_history != history {
                changed = true;
            }

            // cultures：仅空时填
            let cultures_vec: Vec<serde_json::Value> =
                serde_json::from_str(cultures.as_deref().unwrap_or("[]")).unwrap_or_default();
            let m_cultures = if cultures_vec.is_empty() && !new_cultures_json.is_empty() {
                serde_json::to_string(&new_cultures_json).unwrap_or_else(|_| "[]".into())
            } else {
                cultures.clone().unwrap_or_else(|| "[]".into())
            };
            if Some(&m_cultures) != cultures.as_ref() && !(cultures.is_none() && m_cultures == "[]")
            {
                changed = true;
            }

            if !changed {
                return 0;
            }
            let rules_str = serde_json::to_string(&rules_vec).unwrap_or_else(|_| "[]".into());
            match conn.execute(
                "UPDATE world_buildings SET concept = ?2, rules = ?3, history = ?4, \
                 cultures = ?5, updated_at = ?6 WHERE id = ?1",
                params![
                    id,
                    m_concept,
                    rules_str,
                    opt_str(&m_history),
                    m_cultures,
                    ts
                ],
            ) {
                Ok(_) => 1,
                Err(e) => {
                    log::warn!("[AssetBridge] 更新世界观失败: {}", e);
                    0
                }
            }
        }
    }
}

// ==================== 场景大纲 ====================

/// 把 SceneOutlineDelta 渲染为 scenes.outline_content 文本（writer 按文本读取）
fn render_scene_outline(so: &SceneOutlineDelta) -> String {
    let mut lines: Vec<String> = Vec::new();
    if let Some(v) = non_empty(&so.dramatic_goal) {
        lines.push(format!("戏剧目标：{}", v));
    }
    let key_events: Vec<&str> = so
        .key_events
        .iter()
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .collect();
    if !key_events.is_empty() {
        lines.push(format!("关键事件：{}", key_events.join("；")));
    }
    if let Some(v) = non_empty(&so.conflict_type) {
        lines.push(format!("冲突类型：{}", v));
    }
    if let Some(v) = non_empty(&so.setting_location) {
        lines.push(format!("场景地点：{}", v));
    }
    if let Some(v) = non_empty(&so.setting_time) {
        lines.push(format!("场景时间：{}", v));
    }
    if let Some(v) = non_empty(&so.atmosphere) {
        lines.push(format!("氛围：{}", v));
    }
    let characters: Vec<&str> = so
        .characters_present
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .collect();
    if !characters.is_empty() {
        lines.push(format!("出场角色：{}", characters.join("、")));
    }
    if let Some(v) = non_empty(&so.emotional_tone) {
        lines.push(format!("情感基调：{}", v));
    }
    lines.join("\n")
}

fn sync_scene_outline(
    conn: &rusqlite::Connection,
    story_id: &str,
    scene_id: &str,
    so: &SceneOutlineDelta,
) -> usize {
    let text = render_scene_outline(so);
    let ts = now();
    let mut wrote = 0usize;
    if !text.is_empty() {
        let existing: String = conn
            .query_row(
                "SELECT COALESCE(outline_content, '') FROM scenes WHERE id = ?1 AND story_id = ?2",
                params![scene_id, story_id],
                |row| row.get(0),
            )
            .unwrap_or_default();
        // BeatCard 写成的当前场大纲是续写真相源，ingest 不得覆盖。
        if existing.contains("【当前场大纲】") {
            // keep BeatCard outline
        } else {
            match conn.execute(
                "UPDATE scenes SET outline_content = ?3, updated_at = ?4 \
             WHERE id = ?1 AND story_id = ?2 AND ( \
                 outline_content IS NULL OR TRIM(outline_content) = '' \
                 OR COALESCE(source, 'user_created') IN ('ingest', 'agency', 'auto_placeholder') \
             )",
                params![scene_id, story_id, text, ts],
            ) {
                Ok(n) => wrote += n,
                Err(e) => {
                    log::warn!("[AssetBridge] 写入场景大纲失败: {}", e);
                }
            }
        }
    }
    let names: Vec<String> = so
        .characters_present
        .iter()
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .collect();
    if !names.is_empty() {
        let json = serde_json::to_string(&names).unwrap_or_else(|_| "[]".into());
        match conn.execute(
            "UPDATE scenes SET characters_present = ?3, updated_at = ?4 \
             WHERE id = ?1 AND story_id = ?2 AND ( \
                 characters_present IS NULL OR TRIM(characters_present) = '' \
                 OR TRIM(characters_present) = '[]' \
                 OR COALESCE(source, 'user_created') IN ('ingest', 'agency', 'auto_placeholder') \
             )",
            params![scene_id, story_id, json, ts],
        ) {
            Ok(n) => wrote += n,
            Err(e) => {
                log::warn!("[AssetBridge] 写入出场角色失败: {}", e);
            }
        }
    }
    wrote.min(1)
}

// ==================== 故事大纲 ====================

/// 回流只保留一条核心冲突 + 最近若干转折点，避免 story_outlines 无界追加。
const STORY_OUTLINE_TURNING_KEEP: usize = 5;
const STORY_OUTLINE_CHAR_CAP: usize = 4000;

pub(crate) fn cap_story_outline_content(content: &str) -> String {
    let mut core: Option<String> = None;
    let mut turning: Vec<String> = Vec::new();
    let mut other: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('【') && t.contains('】') && !current.is_empty() {
            classify_outline_block(&mut core, &mut turning, &mut other, current.trim());
            current.clear();
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.trim().is_empty() {
        classify_outline_block(&mut core, &mut turning, &mut other, current.trim());
    }
    if turning.len() > STORY_OUTLINE_TURNING_KEEP {
        turning = turning.split_off(turning.len() - STORY_OUTLINE_TURNING_KEEP);
    }
    let mut parts: Vec<String> = Vec::new();
    if let Some(c) = core {
        parts.push(c);
    }
    parts.extend(turning);
    parts.extend(other.into_iter().take(2));
    let joined = parts.join("\n");
    if joined.chars().count() <= STORY_OUTLINE_CHAR_CAP {
        joined
    } else {
        joined.chars().take(STORY_OUTLINE_CHAR_CAP).collect()
    }
}

fn classify_outline_block(
    core: &mut Option<String>,
    turning: &mut Vec<String>,
    other: &mut Vec<String>,
    block: &str,
) {
    if block.contains("【核心冲突】") {
        if core.is_none() {
            *core = Some(block.to_string());
        }
    } else if block.contains("【转折点】") || block.contains("【关键转折点】") {
        turning.push(block.to_string());
    } else if other.len() < 2 {
        other.push(block.to_string());
    }
}

fn sync_story_delta(
    conn: &rusqlite::Connection,
    story_id: &str,
    sd: &crate::memory::ingest::StoryDelta,
) -> usize {
    // 待追加的段落：(去重匹配用原文, 追加文本)
    let mut sections: Vec<(String, String)> = Vec::new();
    if let Some(cc) = non_empty(&sd.core_conflict) {
        sections.push((cc.clone(), format!("【核心冲突】{}", cc)));
    }
    for tp in &sd.turning_points {
        if let Some(tp) = non_empty(tp) {
            sections.push((tp.clone(), format!("【转折点】{}", tp)));
        }
    }
    if sections.is_empty() {
        return 0;
    }

    let existing = conn
        .query_row(
            "SELECT id, content, COALESCE(source, 'unknown') FROM story_outlines WHERE story_id = ?1",
            params![story_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .ok();

    let ts = now();
    match existing {
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            let content = cap_story_outline_content(
                &sections
                    .iter()
                    .map(|(_, s)| s.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            match conn.execute(
                "INSERT INTO story_outlines (id, story_id, content, act_count, created_at, \
                 updated_at) VALUES (?1, ?2, ?3, 3, ?4, ?5)",
                params![id, story_id, content, ts, ts],
            ) {
                Ok(_) => 1,
                Err(e) => {
                    log::warn!("[AssetBridge] 写入故事大纲失败: {}", e);
                    0
                }
            }
        }
        Some((id, content, source)) => {
            // v0.59.3：作者手写/已确认的大纲不追加机器内容——此前 ingest
            // 会把提取出的 冲突/转折点直接拼进作者原文，
            // 手写资产被改动而用户无从察觉（见 V134）。
            if source == "user_created" {
                log::info!("[AssetBridge] 故事大纲为作者手写（source=user_created），跳过机器追加");
                return 0;
            }
            let mut new_content = content.clone();
            let mut changed = false;
            for (raw, section) in &sections {
                // 按原文去重：已包含相同冲突/转折点描述则跳过
                if new_content.contains(raw.as_str()) {
                    continue;
                }
                if !new_content.is_empty() {
                    new_content.push('\n');
                }
                new_content.push_str(section);
                changed = true;
            }
            if !changed {
                return 0;
            }
            new_content = cap_story_outline_content(&new_content);
            match conn.execute(
                "UPDATE story_outlines SET content = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, new_content, ts],
            ) {
                Ok(_) => 1,
                Err(e) => {
                    log::warn!("[AssetBridge] 更新故事大纲失败: {}", e);
                    0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{create_test_pool, dto::CreateStoryRequest, repositories::StoryRepository};

    fn story(pool: &crate::db::DbPool, id: &str) {
        let s = StoryRepository::new(pool.clone())
            .create(CreateStoryRequest {
                title: "测试书".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let conn = pool.get().unwrap();
        conn.execute(
            "UPDATE stories SET id = ?1 WHERE id = ?2",
            rusqlite::params![id, s.id],
        )
        .unwrap();
    }

    /// 用 JSON 构造 ContentAnalysis（同时覆盖反序列化默认值路径）
    fn analysis_from_json(v: serde_json::Value) -> ContentAnalysis {
        serde_json::from_value(v).unwrap()
    }

    fn character_entity(name: &str, extra: serde_json::Value) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "name": name,
            "entity_type": "Character",
        });
        obj.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        obj
    }

    fn analysis_with_entities(entities: Vec<serde_json::Value>) -> ContentAnalysis {
        analysis_from_json(serde_json::json!({
            "entities": entities,
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
        }))
    }

    fn insert_character(pool: &crate::db::DbPool, story_id: &str, name: &str, source: &str) {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO characters (id, story_id, name, source, is_auto_generated, created_at, \
             updated_at) VALUES (?1, ?2, ?3, ?4, 0, '2026-01-01', '2026-01-01')",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), story_id, name, source],
        )
        .unwrap();
    }

    fn char_field(pool: &crate::db::DbPool, name: &str, field: &str) -> Option<String> {
        let conn = pool.get().unwrap();
        conn.query_row(
            &format!("SELECT {} FROM characters WHERE name = ?1", field),
            rusqlite::params![name],
            |r| r.get(0),
        )
        .unwrap()
    }

    // ---------- 角色 ----------

    #[test]
    fn test_sync_registers_new_character() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let analysis = analysis_with_entities(vec![character_entity(
            "阿苔",
            serde_json::json!({
                "personality": "坚韧",
                "background": "拾荒者出身",
                "goals": "找到星环",
                "emotional_core": "压抑的愤怒",
                "emotional_trigger": "被背叛时暴怒",
                "emotional_wound": "目睹母亲惨死",
                "emotional_need": "被认可"
            }),
        )]);
        let n = sync_assets_from_analysis(&pool, "s1", None, &analysis);
        assert_eq!(n, 1);
        let conn = pool.get().unwrap();
        let (source, auto): (String, i32) = conn
            .query_row(
                "SELECT source, is_auto_generated FROM characters WHERE story_id='s1' AND name='阿苔'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(source, "ingest");
        assert_eq!(auto, 1);
        assert_eq!(
            char_field(&pool, "阿苔", "personality").as_deref(),
            Some("坚韧")
        );
        assert_eq!(
            char_field(&pool, "阿苔", "emotional_wound").as_deref(),
            Some("目睹母亲惨死")
        );
    }

    #[test]
    fn test_sync_refines_auto_sourced_character() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        // agency 来源的占位角色：personality 已有一个粗糙值，background 为空
        insert_character(&pool, "s1", "阿苔", "agency");
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "UPDATE characters SET personality = '待定' WHERE story_id='s1' AND name='阿苔'",
                [],
            )
            .unwrap();
        }
        let analysis = analysis_with_entities(vec![character_entity(
            "阿苔",
            serde_json::json!({"personality": "外冷内热", "background": "拾荒者出身"}),
        )]);
        let n = sync_assets_from_analysis(&pool, "s1", None, &analysis);
        assert_eq!(n, 1);
        // 可精炼来源：非空字段被刷新，空字段被填充
        assert_eq!(
            char_field(&pool, "阿苔", "personality").as_deref(),
            Some("外冷内热")
        );
        assert_eq!(
            char_field(&pool, "阿苔", "background").as_deref(),
            Some("拾荒者出身")
        );
    }

    #[test]
    fn test_sync_preserves_user_created_fields_but_fills_empty() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        insert_character(&pool, "s1", "阿苔", "user_created");
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "UPDATE characters SET personality = '用户设定的性格' WHERE story_id='s1' AND name='阿苔'",
                [],
            )
            .unwrap();
        }
        let analysis = analysis_with_entities(vec![character_entity(
            "阿苔",
            serde_json::json!({"personality": "模型猜的性格", "background": "拾荒者出身"}),
        )]);
        let n = sync_assets_from_analysis(&pool, "s1", None, &analysis);
        assert_eq!(n, 1); // 只填充了空字段 background
                          // 用户字段保留
        assert_eq!(
            char_field(&pool, "阿苔", "personality").as_deref(),
            Some("用户设定的性格")
        );
        // 空字段允许填充
        assert_eq!(
            char_field(&pool, "阿苔", "background").as_deref(),
            Some("拾荒者出身")
        );
    }

    #[test]
    fn test_sync_sanitizes_invalid_age() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let analysis = analysis_with_entities(vec![
            character_entity("负年龄", serde_json::json!({"age": -5})),
            character_entity("超长年龄", serde_json::json!({"age": 200})),
            character_entity("正常年龄", serde_json::json!({"age": 19})),
        ]);
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis), 3);
        let conn = pool.get().unwrap();
        let age_of = |name: &str| -> Option<i32> {
            conn.query_row(
                "SELECT age FROM characters WHERE story_id='s1' AND name = ?1",
                rusqlite::params![name],
                |r| r.get(0),
            )
            .unwrap()
        };
        // 非正数 / >150 视为 None 不落库；合法值正常写入
        assert_eq!(age_of("负年龄"), None);
        assert_eq!(age_of("超长年龄"), None);
        assert_eq!(age_of("正常年龄"), Some(19));
    }

    // ---------- 关系 ----------

    #[test]
    fn test_sync_relationship_dedup_and_fill() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let analysis = analysis_from_json(serde_json::json!({
            "entities": [
                character_entity("甲", serde_json::json!({})),
                character_entity("乙", serde_json::json!({})),
            ],
            "relationships": [
                {
                    "source": "甲", "target": "乙", "relation_type": "师徒",
                    "emotional_bond": "欺骗", "emotional_intensity": 0.9,
                    "reverse_emotional_bond": "崇拜", "reverse_emotional_intensity": 0.7
                }
            ],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
        }));
        // 2 角色 + 1 关系
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis), 3);
        let conn = pool.get().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_relationships WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let bond: String = conn
            .query_row(
                "SELECT emotional_bond FROM character_relationships WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(bond, "欺骗");
        drop(conn);

        // 重复同步（同 source/target）：不新增行；description 原本为空则填空
        let analysis2 = analysis_from_json(serde_json::json!({
            "relationships": [
                {"source": "甲", "target": "乙", "relation_type": "师徒", "description": "面和心不和"}
            ],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
        }));
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis2), 1);
        let conn = pool.get().unwrap();
        let (count, desc): (i64, String) = conn
            .query_row(
                "SELECT COUNT(*), MAX(description) FROM character_relationships WHERE story_id='s1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(desc, "面和心不和");
        // 既有 emotional_bond 不被覆盖
        let bond: String = conn
            .query_row(
                "SELECT emotional_bond FROM character_relationships WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(bond, "欺骗");
        drop(conn);

        // 完全相同的重复同步：无任何变更
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis2), 0);
    }

    #[test]
    fn test_sync_relationship_intensity_overrides_meaningless_default() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        insert_character(&pool, "s1", "甲", "user_created");
        insert_character(&pool, "s1", "乙", "user_created");
        // 既有关系行：bond 为空、intensity 停留在列默认 0.5（V124 DEFAULT），
        // 等价于"从未被真正赋过强度"
        {
            let conn = pool.get().unwrap();
            let (sid, tid): (String, String) = conn
                .query_row(
                    "SELECT (SELECT id FROM characters WHERE story_id='s1' AND name='甲'), \
                            (SELECT id FROM characters WHERE story_id='s1' AND name='乙')",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            conn.execute(
                "INSERT INTO character_relationships (id, story_id, source_character_id, \
                 target_character_id, relationship_type, created_at) \
                 VALUES ('rel1', 's1', ?1, ?2, '盟友', '2026-01-01')",
                rusqlite::params![sid, tid],
            )
            .unwrap();
        }
        let analysis = analysis_from_json(serde_json::json!({
            "relationships": [
                {"source": "甲", "target": "乙", "relation_type": "盟友",
                 "emotional_bond": "信任", "emotional_intensity": 0.9}
            ],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
        }));
        // bond 原本为空且 intensity 是无意义默认 0.5：允许新值覆盖
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis), 1);
        let conn = pool.get().unwrap();
        let (bond, intensity): (String, f64) = conn
            .query_row(
                "SELECT emotional_bond, emotional_intensity FROM character_relationships \
                 WHERE id='rel1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(bond, "信任");
        assert!((intensity - 0.9).abs() < 1e-6);
        drop(conn);

        // bond 已填之后：既有强度是有意义的值，新分析不再覆盖
        let analysis2 = analysis_from_json(serde_json::json!({
            "relationships": [
                {"source": "甲", "target": "乙", "relation_type": "盟友",
                 "description": "并肩作战", "emotional_intensity": 0.3}
            ],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
        }));
        // 只填空字段 description
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis2), 1);
        let conn = pool.get().unwrap();
        let (desc, intensity): (String, f64) = conn
            .query_row(
                "SELECT description, emotional_intensity FROM character_relationships \
                 WHERE id='rel1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(desc, "并肩作战");
        assert!((intensity - 0.9).abs() < 1e-6);
    }

    // ---------- 并发（TOCTOU 回归） ----------

    #[test]
    fn test_concurrent_sync_same_story_inserts_no_duplicates() {
        // create_test_pool 的 :memory: 每连接一个独立库，无法验证并发；
        // 用文件库让两个线程各自拿连接、真实并发（模拟 scene_service
        // 每场景 spawn 一个 ingest）
        let tmp = tempfile::tempdir().unwrap();
        let pool = crate::db::init_db(tmp.path(), None).unwrap();
        story(&pool, "s1");
        let analysis_json = serde_json::json!({
            "entities": [
                character_entity("甲", serde_json::json!({})),
                character_entity("乙", serde_json::json!({})),
            ],
            "relationships": [
                {"source": "甲", "target": "乙", "relation_type": "师徒",
                 "emotional_bond": "信任", "emotional_intensity": 0.8}
            ],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
        });
        let mut handles = Vec::new();
        for _ in 0..2 {
            let pool = pool.clone();
            let json = analysis_json.clone();
            handles.push(std::thread::spawn(move || {
                let analysis = analysis_from_json(json);
                sync_assets_from_analysis(&pool, "s1", None, &analysis)
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let conn = pool.get().unwrap();
        // 同名新角色只插入一行
        for name in ["甲", "乙"] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM characters WHERE story_id='s1' AND name = ?1",
                    rusqlite::params![name],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "角色 {} 出现重复行", name);
        }
        // 关系只建一行
        let rel_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_relationships WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(rel_count, 1);
    }

    // ---------- 世界观 ----------

    #[test]
    fn test_sync_world_rules_append_dedup_by_name() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let analysis = analysis_from_json(serde_json::json!({
            "entities": [],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
            "world_building": {
                "concept": "双星废土",
                "rules": [{"name": "磁力风暴", "description": "周期性磁暴", "rule_type": "physical", "importance": 8}],
                "history": "旧文明崩坏百年",
                "cultures": [{"name": "拾荒者", "description": "废墟淘金", "customs": ["以物易物"], "values": ["生存"]}]
            },
        }));
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis), 1);
        // 追加新规则 + 同名规则去重
        let analysis2 = analysis_from_json(serde_json::json!({
            "entities": [],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
            "world_building": {
                "rules": [
                    {"name": "磁力风暴", "description": "被改写也不应覆盖", "rule_type": "magic", "importance": 3},
                    {"name": "水资源配给", "description": "水由城邦统一配给", "rule_type": "social", "importance": 7}
                ]
            },
        }));
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis2), 1);
        let conn = pool.get().unwrap();
        let rules_str: String = conn
            .query_row(
                "SELECT rules FROM world_buildings WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let rules: Vec<serde_json::Value> = serde_json::from_str(&rules_str).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["name"], "磁力风暴");
        // 同名规则保留原描述（不覆盖）
        assert_eq!(rules[0]["description"], "周期性磁暴");
        assert_eq!(rules[1]["name"], "水资源配给");
        assert_eq!(rules[1]["rule_type"], "Social");
        // concept/history 已填，第二次无增量时不被清空
        let concept: String = conn
            .query_row(
                "SELECT concept FROM world_buildings WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(concept, "双星废土");
        let history: String = conn
            .query_row(
                "SELECT history FROM world_buildings WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(history, "旧文明崩坏百年");
        drop(conn);

        // 完全重复同步：无变更
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis2), 0);
    }

    #[test]
    fn test_sync_world_does_not_override_user_concept() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO world_buildings (id, story_id, concept, rules, history, cultures, \
                 source, is_auto_generated, created_at, updated_at) \
                 VALUES ('wb1', 's1', '用户手写世界观', '[]', NULL, '[]', 'user_created', 0, '2026-01-01', '2026-01-01')",
                [],
            )
            .unwrap();
        }
        let analysis = analysis_from_json(serde_json::json!({
            "entities": [],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
            "world_building": {"concept": "模型猜的世界观", "history": "补全的历史"},
        }));
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis), 1);
        let conn = pool.get().unwrap();
        let (concept, history): (String, String) = conn
            .query_row(
                "SELECT concept, history FROM world_buildings WHERE story_id='s1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        // 用户 concept 保留；空 history 允许填充
        assert_eq!(concept, "用户手写世界观");
        assert_eq!(history, "补全的历史");
    }

    // ---------- 场景大纲 ----------

    fn insert_scene(
        pool: &crate::db::DbPool,
        story_id: &str,
        scene_id: &str,
        outline: Option<&str>,
        source: &str,
    ) {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO scenes (id, story_id, sequence_number, title, outline_content, source, \
             created_at, updated_at) VALUES (?1, ?2, 1, '第一章', ?3, ?4, '2026-01-01', '2026-01-01')",
            rusqlite::params![scene_id, story_id, outline, source],
        )
        .unwrap();
    }

    fn scene_outline(pool: &crate::db::DbPool, scene_id: &str) -> Option<String> {
        let conn = pool.get().unwrap();
        conn.query_row(
            "SELECT outline_content FROM scenes WHERE id = ?1",
            rusqlite::params![scene_id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn test_sync_scene_outline_fill_and_preserve() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        insert_scene(&pool, "s1", "sc1", None, "user_created");
        let analysis = analysis_from_json(serde_json::json!({
            "entities": [],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
            "scene_outline": {
                "dramatic_goal": "主角潜入核电站",
                "key_events": ["破解门禁", "发现阴谋"],
                "characters_present": ["甲", "乙"],
                "emotional_tone": "紧张"
            },
        }));
        assert_eq!(
            sync_assets_from_analysis(&pool, "s1", Some("sc1"), &analysis),
            1
        );
        let outline = scene_outline(&pool, "sc1").unwrap();
        assert!(outline.contains("戏剧目标：主角潜入核电站"));
        assert!(outline.contains("关键事件：破解门禁；发现阴谋"));
        assert!(outline.contains("出场角色：甲、乙"));
        assert!(outline.contains("情感基调：紧张"));
        {
            let conn = pool.get().unwrap();
            let present: String = conn
                .query_row(
                    "SELECT characters_present FROM scenes WHERE id='sc1'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(present.contains("甲"));
            assert!(present.contains("乙"));
        }

        // 用户已设 outline 的场景：保留大纲；出场列为空时仍可填名字
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO scenes (id, story_id, sequence_number, title, outline_content, source, \
                 created_at, updated_at) VALUES ('sc2', 's1', 2, '第二章', '用户手写大纲', 'user_created', \
                 '2026-01-01', '2026-01-01')",
                [],
            )
            .unwrap();
        }
        let n = sync_assets_from_analysis(&pool, "s1", Some("sc2"), &analysis);
        assert_eq!(scene_outline(&pool, "sc2").as_deref(), Some("用户手写大纲"));
        assert!(n <= 1);
        {
            let conn = pool.get().unwrap();
            let present: Option<String> = conn
                .query_row(
                    "SELECT characters_present FROM scenes WHERE id='sc2'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            let present = present.unwrap_or_default();
            assert!(present.contains("甲") && present.contains("乙"));
        }
    }

    // ---------- 故事大纲 ----------

    #[test]
    /// v0.59.3：作者手写大纲（source=user_created）不再被 ingest 追加机器内容。
    #[test]
    fn test_sync_story_delta_skips_user_created_outline() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO story_outlines (id, story_id, content, act_count, created_at, updated_at, source)
                 VALUES ('o1', 's1', '作者手写：三幕结构自行安排', 3, 'now', 'now', 'user_created')",
                [],
            )
            .unwrap();
        }
        let analysis = analysis_from_json(serde_json::json!({
            "entities": [],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
            "story_delta": {
                "core_conflict": "机器提取的核心冲突",
                "turning_points": ["机器提取的转折点"]
            },
        }));
        assert_eq!(
            sync_assets_from_analysis(&pool, "s1", None, &analysis),
            0,
            "手写大纲不得被追加"
        );
        let conn = pool.get().unwrap();
        let (content, source): (String, String) = conn
            .query_row(
                "SELECT content, source FROM story_outlines WHERE story_id='s1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(content, "作者手写：三幕结构自行安排");
        assert_eq!(source, "user_created");
    }

    fn test_sync_story_delta_append_and_dedup() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let analysis = analysis_from_json(serde_json::json!({
            "entities": [],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
            "story_delta": {
                "core_conflict": "人类与 AI 的生存博弈",
                "turning_points": ["主角发现自己是克隆体"]
            },
        }));
        // 无行则建
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis), 1);
        let conn = pool.get().unwrap();
        let content: String = conn
            .query_row(
                "SELECT content FROM story_outlines WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(content.contains("【核心冲突】人类与 AI 的生存博弈"));
        assert!(content.contains("【转折点】主角发现自己是克隆体"));
        drop(conn);

        // 追加新转折点；重复的 core_conflict 不再追加
        let analysis2 = analysis_from_json(serde_json::json!({
            "entities": [],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
            "story_delta": {
                "core_conflict": "人类与 AI 的生存博弈",
                "turning_points": ["AI 城市首次开门"]
            },
        }));
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis2), 1);
        let conn = pool.get().unwrap();
        let content: String = conn
            .query_row(
                "SELECT content FROM story_outlines WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(content.matches("【核心冲突】").count(), 1);
        assert!(content.contains("【转折点】AI 城市首次开门"));
        drop(conn);

        // 完全重复：无变更
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis2), 0);
    }

    #[test]
    fn test_sync_story_delta_caps_old_turning_points() {
        let pool = create_test_pool().unwrap();
        story(&pool, "s1");
        let tps: Vec<String> = (0..8).map(|i| format!("转折点{i}独有标记")).collect();
        let analysis = analysis_from_json(serde_json::json!({
            "entities": [],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
            "story_delta": {
                "core_conflict": "核心不丢",
                "turning_points": tps,
            },
        }));
        assert_eq!(sync_assets_from_analysis(&pool, "s1", None, &analysis), 1);
        let conn = pool.get().unwrap();
        let content: String = conn
            .query_row(
                "SELECT content FROM story_outlines WHERE story_id='s1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(content.contains("【核心冲突】核心不丢"));
        assert_eq!(content.matches("【转折点】").count(), 5);
        assert!(
            !content.contains("转折点0独有标记"),
            "旧转折应丢掉 content={content}"
        );
        assert!(content.contains("转折点7独有标记"), "content={content}");
    }

    #[test]
    fn cap_story_outline_keeps_last_five() {
        let mut raw = String::from("【核心冲突】C\n");
        for i in 0..8 {
            raw.push_str(&format!("【转折点】T{i}\n"));
        }
        let capped = cap_story_outline_content(&raw);
        assert!(capped.contains("【核心冲突】C"));
        assert_eq!(capped.matches("【转折点】").count(), 5);
        assert!(!capped.contains("T0"));
        assert!(capped.contains("T7"));
    }

    // ---------- 反序列化兼容 ----------

    #[test]
    fn test_legacy_json_deserialization_compat() {
        // 旧版输出（无新字段）：必须能反序列化，新字段取默认值
        let legacy = serde_json::json!({
            "entities": [
                {"name": "林枫", "entity_type": "Character", "mentions": ["林枫站在青云山顶"],
                 "attributes": {"location": "青云山顶"}}
            ],
            "relationships": [
                {"source": "林枫", "target": "师父", "relation_type": "师徒",
                 "evidence": "要找到杀害师父的凶手", "strength": 0.9}
            ],
            "events": [{"description": "林枫发誓复仇", "participants": ["林枫"], "importance": 9}],
            "sentiment": {"overall": "negative", "intensity": 0.8, "arc": []},
            "foreshadowing": [{"content": "复仇", "type_": "setup", "related_to": []}],
            "themes": ["复仇"]
        });
        let analysis = analysis_from_json(legacy);
        let e = &analysis.entities[0];
        assert_eq!(e.name, "林枫");
        assert!(e.personality.is_empty());
        assert!(e.emotional_core.is_empty());
        assert!(e.role_type.is_empty());
        assert_eq!(e.age, None);
        let r = &analysis.relationships[0];
        assert_eq!(r.relation_type, "师徒");
        assert!(r.description.is_empty());
        assert!(r.emotional_bond.is_empty());
        assert!(r.reverse_emotional_bond.is_empty());
        assert!(analysis.world_building.is_none());
        assert!(analysis.scene_outline.is_none());
        assert!(analysis.story_delta.is_none());
    }

    #[test]
    fn test_new_fields_deserialize_from_full_json() {
        let full = serde_json::json!({
            "entities": [
                {"name": "阿苔", "entity_type": "Character", "role_type": "主角",
                 "personality": "坚韧", "background": "拾荒者", "goals": "找到星环",
                 "fears": "被抛弃", "appearance": "短发", "gender": "女", "age": 19,
                 "emotional_core": "愤怒", "emotional_trigger": "背叛",
                 "emotional_wound": "丧母", "emotional_need": "被认可",
                 "importance_score": 0.95}
            ],
            "relationships": [
                {"source": "阿苔", "target": "老周", "relation_type": "盟友",
                 "description": "互相利用", "dynamic": "渐行渐远",
                 "emotional_bond": "依赖", "emotional_intensity": 0.6,
                 "reverse_emotional_bond": "怜悯", "reverse_emotional_intensity": 0.4}
            ],
            "sentiment": {"overall": "neutral", "intensity": 0.5, "arc": []},
            "world_building": {"concept": "废土", "rules": [], "history": "", "cultures": []},
            "scene_outline": {"dramatic_goal": "潜入", "key_events": ["开门"]},
            "story_delta": {"core_conflict": "生存", "turning_points": ["背叛"]}
        });
        let analysis = analysis_from_json(full);
        let e = &analysis.entities[0];
        assert_eq!(e.role_type, "主角");
        assert_eq!(e.age, Some(19));
        assert_eq!(e.emotional_need, "被认可");
        assert!((e.importance_score - 0.95).abs() < 1e-6);
        let r = &analysis.relationships[0];
        assert_eq!(r.dynamic, "渐行渐远");
        assert_eq!(r.reverse_emotional_bond, "怜悯");
        assert!((r.emotional_intensity - 0.6).abs() < 1e-6);
        assert!(analysis.world_building.is_some());
        assert!(analysis.scene_outline.is_some());
        assert_eq!(
            analysis.story_delta.as_ref().unwrap().turning_points,
            vec!["背叛".to_string()]
        );
    }
}
