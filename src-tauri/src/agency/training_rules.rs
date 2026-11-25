//! v0.99.24：训练报告 → 通用规则提升层 + 生产回归探针。
//!
//! 训练报告里的「具体剧情/人物」不能直接搬到别的生产故事；这里只提取
//! **可泛化的规则**：
//! - 低分维度 → 通用写作约束（文风/口吻/推进/节奏/钩子/承接）；
//! - improvement 文本中出现「主线未落地」「文风修正」「AI 腔修正」等
//!   通用探针信号 → 对应的通用规则；
//! - 规则按 `rule_key` 去重并累计 `promotion_count`，跨重启保留。
//!
//! 生产路径读取 active 规则并注入 writer/gate；生产回归探针在每轮装配后
//! 用同一组规则核对正文（文风漂移、AI 腔、主线落地、钩子、承接重复等）。
//! 规则文本不包含书名、人名、章号或具体情节，避免训练过拟合。

use std::collections::HashMap;

use chrono::Local;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

use crate::{
    agency::{
        mainline::MainlineProjection,
        style_probe::{self, StyleProfile},
    },
    db::DbPool,
};

/// 生产规则块上限（字符）。只注入最重要的规则，避免 prompt 膨胀。
pub const PROMOTED_RULE_BLOCK_CHAR_CAP: usize = 1_800;
/// 通用规则提升阈值：与训练报告的 FEEDBACK_THRESHOLD 同口径（10 分制）。
const FEEDBACK_THRESHOLD: f64 = 6.0;
/// 最多注入的规则条数。
pub const PROMOTED_RULE_LIMIT: usize = 8;

/// 训练报告沉淀的通用规则（不含书名/人名/具体情节）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrainingRule {
    pub rule_key: String,
    pub dimension: String,
    pub probe_kind: String,
    pub rule_text: String,
    pub source_report_id: String,
    pub score: Option<f64>,
    pub promotion_count: u32,
    pub active: bool,
}

/// 生产回归探针结果。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProductionProbeResult {
    pub line: String,
    pub failures: Vec<String>,
    pub active_rules: usize,
    pub probed_rules: usize,
    pub unprobed_rules: usize,
}

pub struct TrainingRuleRepository {
    pool: DbPool,
}

impl TrainingRuleRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn ensure_table(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS training_rule_promotions (\
                id TEXT PRIMARY KEY,\
                rule_key TEXT NOT NULL UNIQUE,\
                dimension TEXT NOT NULL,\
                probe_kind TEXT NOT NULL,\
                rule_text TEXT NOT NULL,\
                source_report_id TEXT,\
                score REAL,\
                promotion_count INTEGER NOT NULL DEFAULT 1,\
                active INTEGER NOT NULL DEFAULT 1,\
                created_at TEXT NOT NULL,\
                updated_at TEXT NOT NULL\
            );\
            CREATE INDEX IF NOT EXISTS idx_training_rule_active \
             ON training_rule_promotions(active, promotion_count DESC);",
        )
    }

    pub fn list_active(&self) -> Result<Vec<TrainingRule>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        let mut stmt = conn.prepare(
            "SELECT rule_key, dimension, probe_kind, rule_text, COALESCE(source_report_id, ''), \
             score, promotion_count, active \
             FROM training_rule_promotions WHERE active = 1 \
             ORDER BY promotion_count DESC, updated_at DESC LIMIT 24",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TrainingRule {
                rule_key: row.get(0)?,
                dimension: row.get(1)?,
                probe_kind: row.get(2)?,
                rule_text: row.get(3)?,
                source_report_id: row.get(4)?,
                score: row.get(5)?,
                promotion_count: row.get(6)?,
                active: row.get::<_, i32>(7)? != 0,
            })
        })?;
        rows.collect()
    }

    /// 按 rule_key 去重累计；同一轮报告只提升一次同类规则。
    pub fn upsert(&self, rule: &TrainingRule) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        let now = Local::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO training_rule_promotions \
             (id, rule_key, dimension, probe_kind, rule_text, source_report_id, score, \
              promotion_count, active, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 1, ?8, ?8) \
             ON CONFLICT(rule_key) DO UPDATE SET \
                dimension = excluded.dimension, \
                probe_kind = excluded.probe_kind, \
                rule_text = excluded.rule_text, \
                source_report_id = excluded.source_report_id, \
                score = excluded.score, \
                promotion_count = training_rule_promotions.promotion_count + 1, \
                active = 1, \
                updated_at = excluded.updated_at",
            rusqlite::params![
                id,
                rule.rule_key,
                rule.dimension,
                rule.probe_kind,
                rule.rule_text,
                rule.source_report_id,
                rule.score,
                now
            ],
        )
    }

    pub fn by_key(&self, rule_key: &str) -> Result<Option<TrainingRule>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        Self::ensure_table(&conn)?;
        let mut stmt = conn.prepare(
            "SELECT rule_key, dimension, probe_kind, rule_text, COALESCE(source_report_id, ''), \
             score, promotion_count, active \
             FROM training_rule_promotions WHERE rule_key = ?1 LIMIT 1",
        )?;
        stmt.query_row([rule_key], |row| {
            Ok(TrainingRule {
                rule_key: row.get(0)?,
                dimension: row.get(1)?,
                probe_kind: row.get(2)?,
                rule_text: row.get(3)?,
                source_report_id: row.get(4)?,
                score: row.get(5)?,
                promotion_count: row.get(6)?,
                active: row.get::<_, i32>(7)? != 0,
            })
        })
        .optional()
    }
}

/// 从训练报告的 `scores_json` + `improvement_md` 提升通用规则。
///
/// 只读取维度均分与通用信号词；improvement 里的具体书名/人名/情节不复制。
pub fn promote_from_report(
    pool: &DbPool,
    report_id: &str,
    scores_json: &str,
    improvement_md: &str,
) -> usize {
    let dims = parse_dimension_avgs(scores_json);
    let mut candidates: HashMap<&'static str, (String, String, Option<f64>)> = HashMap::new();
    let mut for_dim = |dim: &str, key: &'static str| {
        if !is_corrective_dim(dim) {
            return;
        }
        if dims
            .get(dim)
            .copied()
            .map(|v| v < FEEDBACK_THRESHOLD)
            .unwrap_or(false)
        {
            let score = dims.get(dim).copied();
            if let Some((probe, text)) = generic_rule_spec(key) {
                candidates.insert(key, (probe.to_string(), text.to_string(), score));
            }
        }
    };
    for dim in ["style", "plot", "voice", "pacing", "hooks", "continuity"] {
        for_dim(dim, dimension_rule_key(dim));
    }

    let has_style = signal(
        improvement_md,
        &["文风修正", "文风漂移", "语体", "句长", "冷硬"],
    );
    let has_ai = signal(improvement_md, &["AI 腔修正", "AI 腔", "身体反应", "比喻"]);
    let has_mainline = signal(improvement_md, &["主线未落地", "主线修正", "原著主线"]);
    let has_continuity = signal(improvement_md, &["承接", "原地踏步", "复述"]);
    let has_plot = signal(improvement_md, &["推进", "状态变化", "信息密度"]);
    let has_voice = signal(improvement_md, &["口吻", "对白", "腔调"]);
    let has_pacing = signal(improvement_md, &["节奏", "详略", "注水"]);
    let has_hooks = signal(improvement_md, &["钩子", "悬念", "章末"]);
    for (flag, key, dim) in [
        (has_style, "style.voice_baseline", "style"),
        (has_ai, "style.ai_ism", "style"),
        (has_mainline, "mainline.landing", "plot"),
        (has_continuity, "continuity.anchor", "continuity"),
        (has_plot, "plot.progress", "plot"),
        (has_voice, "voice.dialogue", "voice"),
        (has_pacing, "pacing.density", "pacing"),
        (has_hooks, "hooks.ending", "hooks"),
    ] {
        if flag {
            if let Some((probe, text)) = generic_rule_spec(key) {
                let score = dims.get(dim).copied();
                candidates.insert(key, (probe.to_string(), text.to_string(), score));
            }
        }
    }

    let repo = TrainingRuleRepository::new(pool.clone());
    let mut promoted = 0usize;
    for (key, (probe, text, score)) in candidates {
        let rule = TrainingRule {
            rule_key: key.to_string(),
            dimension: dimension_for_key(key).to_string(),
            probe_kind: probe,
            rule_text: text,
            source_report_id: report_id.to_string(),
            score,
            promotion_count: 1,
            active: true,
        };
        match repo.upsert(&rule) {
            Ok(_) => promoted += 1,
            Err(e) => log::warn!("[TrainingRules] 提升 {} 失败: {}", key, e),
        }
    }
    promoted
}

fn dimension_for_key(key: &str) -> &'static str {
    match key {
        "style.voice_baseline" | "style.ai_ism" => "style",
        "mainline.landing" => "plot",
        "continuity.anchor" => "continuity",
        "plot.progress" => "plot",
        "voice.dialogue" => "voice",
        "pacing.density" => "pacing",
        "hooks.ending" => "hooks",
        _ => "style",
    }
}

fn dimension_rule_key(dim: &str) -> &'static str {
    match dim {
        "style" => "style.voice_baseline",
        "plot" => "plot.progress",
        "voice" => "voice.dialogue",
        "pacing" => "pacing.density",
        "hooks" => "hooks.ending",
        "continuity" => "continuity.anchor",
        _ => "style.voice_baseline",
    }
}

fn generic_rule_spec(key: &str) -> Option<(&'static str, &'static str)> {
    Some(match key {
        "style.voice_baseline" => (
            "style_drift",
            "每章按原文语体基线校准句长、对白占比、第一人称与口语词；恢复市井碎嘴/自嘲，禁止冷硬侦探腔、格言式旁白与书面总结腔；比喻≤3/章、「不是…而是…」≤1/章。",
        ),
        "style.ai_ism" => (
            "ai_ism",
            "禁止用后槽牙/发麻/发凉/心里咯噔/后脊梁等身体反应套语替代动作与对白；同章同类不得重复；比喻与明喻不得连用。",
        ),
        "mainline.landing" => (
            "mainline_landing",
            "本章必须正面落地故事计划的主线事件/地点/关键人物；不得推迟到后续章节，不得用电话/短信/转述/跟踪替代正面场面；不得另开平行支线。",
        ),
        "continuity.anchor" => (
            "continuity",
            "首段必须接上一章末尾现场之后的新动作（同地点/同人物/未结事件）；不复制、不复述锚点；禁止跳切新地点、新主角或新委托。",
        ),
        "plot.progress" => (
            "plot_state_delta",
            "每章至少一个可验证状态变化（信息/关系/目标/风险/情绪），并推进至少一条未结线索；不得原地踏步或用说明性对白替代场面。",
        ),
        "voice.dialogue" => (
            "voice_dialogue",
            "对白贴合角色身份、时代语感与防御；不得全员同一腔调；信息在对抗/利益交锋中逼出，不用单向长段解说替代。",
        ),
        "pacing.density" => (
            "pacing_density",
            "保持原文详略比例与信息密度；禁止注水式铺垫；章末控制为小悬念而非收束；单章以 1500–3000 字为主。",
        ),
        "hooks.ending" => (
            "hook_ending",
            "开头 300 字内建立张力；章末只留一个具体强钩（未解动作/一句威胁）；禁止电话/短信/独白多源堆叠与预告腔。",
        ),
        _ => return None,
    })
}

fn parse_dimension_avgs(scores_json: &str) -> HashMap<String, f64> {
    let mut out = HashMap::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(scores_json) else {
        return out;
    };
    if let Some(scores) = value.get("scores").and_then(|v| v.as_object()) {
        for (key, val) in scores {
            if let Some(n) = val.as_f64() {
                out.insert(key.clone(), n);
            }
        }
    }
    if let Some(n) = value.get("continuity").and_then(|v| v.as_f64()) {
        out.insert("continuity".to_string(), n);
    }
    out
}

fn signal(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn is_corrective_dim(dim: &str) -> bool {
    matches!(
        dim,
        "style" | "plot" | "voice" | "pacing" | "hooks" | "continuity"
    )
}

/// 生产规则提示块（active 规则按累计次数排序；上限保护 prompt）。
pub fn render_prompt_block(rules: &[TrainingRule]) -> (String, usize) {
    let mut picked = rules
        .iter()
        .filter(|rule| rule.active && !rule.rule_text.trim().is_empty())
        .collect::<Vec<_>>();
    picked.sort_by(|a, b| {
        b.promotion_count
            .cmp(&a.promotion_count)
            .then(a.dimension.cmp(&b.dimension))
            .then(a.rule_key.cmp(&b.rule_key))
    });
    picked.truncate(PROMOTED_RULE_LIMIT);
    if picked.is_empty() {
        return (String::new(), 0);
    }
    let mut out = String::from(
        "【训练报告沉淀通用规则（从历史训练报告泛化；禁止照搬具体书名/人名/情节；\
         优先级低于本章故事主线任务，高于一般文风建议）】\n",
    );
    for rule in &picked {
        let label = dimension_label(&rule.dimension);
        out.push_str(&format!("- 【{label}】{}\n", rule.rule_text.trim()));
        if out.chars().count() >= PROMOTED_RULE_BLOCK_CHAR_CAP {
            break;
        }
    }
    let block = crate::agency::continue_assets::truncate_chars(&out, PROMOTED_RULE_BLOCK_CHAR_CAP);
    (block, picked.len())
}

/// 首次升级时从最近训练报告补齐规则（旧报告不再触发训练也能受益）。
/// 只使用 scores_json + improvement_md 的通用信号，不复制具体情节。
pub fn backfill_from_recent_reports(pool: &DbPool, limit: usize) -> usize {
    let repo = TrainingRuleRepository::new(pool.clone());
    if repo
        .list_active()
        .map(|rules| !rules.is_empty())
        .unwrap_or(false)
    {
        return 0;
    }
    let Ok(conn) = pool.get() else {
        return 0;
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT id, COALESCE(scores_json, ''), COALESCE(improvement_md, '')          FROM continuation_training_reports          WHERE improvement_md IS NOT NULL AND improvement_md != ''          ORDER BY created_at DESC LIMIT ?1",
    ) else {
        return 0;
    };
    let rows = stmt.query_map(rusqlite::params![limit.max(1) as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    });
    let mut promoted = 0usize;
    if let Ok(rows) = rows {
        for row in rows.flatten() {
            promoted += promote_from_report(pool, &row.0, &row.1, &row.2);
        }
    }
    promoted
}

pub fn load_prompt_block(pool: &DbPool) -> (Option<String>, usize) {
    // 首次升级/尚无规则时，从最近报告补齐一次（幂等；有规则后不再扫描）。
    let _ = backfill_from_recent_reports(pool, 8);
    let repo = TrainingRuleRepository::new(pool.clone());
    let rules = repo.list_active().unwrap_or_default();
    let (block, count) = render_prompt_block(&rules);
    if block.trim().is_empty() {
        (None, 0)
    } else {
        (Some(block), count)
    }
}

/// 生产回归探针：用已提升规则核对当前生产章节。
pub fn run_production_probes(
    pool: &DbPool,
    chapters: &[(i32, String)],
    style_baseline: Option<&StyleProfile>,
    mainline: Option<&MainlineProjection>,
) -> ProductionProbeResult {
    let repo = TrainingRuleRepository::new(pool.clone());
    let rules = repo.list_active().unwrap_or_default();
    if rules.is_empty() || chapters.is_empty() {
        return ProductionProbeResult {
            line: "生产回归探针：无活跃规则（尚未从训练报告提升通用规则）".to_string(),
            failures: Vec::new(),
            active_rules: 0,
            probed_rules: 0,
            unprobed_rules: 0,
        };
    }
    let mut result = ProductionProbeResult {
        active_rules: rules.len(),
        ..Default::default()
    };
    let mut style_hits: Vec<i32> = Vec::new();
    let mut ai_hits: Vec<i32> = Vec::new();
    let mut continuity_hits: Vec<i32> = Vec::new();
    let mut plot_hits: Vec<i32> = Vec::new();
    let mut voice_hits: Vec<i32> = Vec::new();
    let mut pacing_hits: Vec<i32> = Vec::new();
    let mut hook_hits: Vec<i32> = Vec::new();
    let mut mainline_misses: Vec<i32> = Vec::new();

    for rule in &rules {
        match rule.probe_kind.as_str() {
            "style_drift" => {
                let Some(baseline) = style_baseline else {
                    result.unprobed_rules += 1;
                    continue;
                };
                result.probed_rules += 1;
                for (number, text) in chapters {
                    if style_probe::severe_drift_issue(baseline, text).is_some() {
                        style_hits.push(*number);
                    }
                }
            }
            "ai_ism" => {
                result.probed_rules += 1;
                for (number, text) in chapters {
                    if style_probe::ai_ism_issue(text).is_some() {
                        ai_hits.push(*number);
                    }
                }
            }
            "mainline_landing" => {
                let Some(projection) = mainline else {
                    result.unprobed_rules += 1;
                    continue;
                };
                result.probed_rules += 1;
                let coverage = projection.coverage(chapters);
                mainline_misses.extend(coverage.misses.iter().map(|(n, _)| *n));
            }
            "continuity" => {
                result.probed_rules += 1;
                for index in 1..chapters.len() {
                    let current = &chapters[index].1;
                    let previous = &chapters[index - 1].1;
                    if duplicate_opening(current, previous) {
                        continuity_hits.push(chapters[index].0);
                    }
                }
            }
            "plot_state_delta" => {
                result.probed_rules += 1;
                for (number, text) in chapters {
                    if !has_any_marker(text, STATE_MARKERS) {
                        plot_hits.push(*number);
                    }
                }
            }
            "voice_dialogue" => {
                result.probed_rules += 1;
                for (number, text) in chapters {
                    if !has_quoted_dialogue(text) {
                        voice_hits.push(*number);
                    }
                }
            }
            "pacing_density" => {
                result.probed_rules += 1;
                for (number, text) in chapters {
                    let chars = text.chars().count();
                    if chars < 600 {
                        pacing_hits.push(*number);
                    }
                }
            }
            "hook_ending" => {
                result.probed_rules += 1;
                for (number, text) in chapters {
                    let tail: String = text
                        .chars()
                        .rev()
                        .take(240)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect();
                    if !has_any_marker(&tail, HOOK_MARKERS) {
                        hook_hits.push(*number);
                    }
                }
            }
            _ => result.unprobed_rules += 1,
        }
    }

    let mut parts: Vec<String> = Vec::new();
    parts.push(format!("活跃规则 {} 条", rules.len()));
    if style_baseline.is_none() {
        parts.push("文风漂移：无基线".to_string());
    } else if !style_hits.is_empty() {
        parts.push(format!(
            "文风漂移 {} 章（{style_hits:?}）",
            style_hits.len()
        ));
    } else {
        parts.push("文风漂移 0".to_string());
    }
    if !ai_hits.is_empty() {
        parts.push(format!("AI 腔 {} 章（{ai_hits:?}）", ai_hits.len()));
    }
    if !mainline_misses.is_empty() {
        parts.push(format!(
            "主线缺失 {} 章（{mainline_misses:?}）",
            mainline_misses.len()
        ));
    }
    if !continuity_hits.is_empty() {
        parts.push(format!(
            "承接重复开篇 {} 章（{continuity_hits:?}）",
            continuity_hits.len()
        ));
    }
    if !plot_hits.is_empty() {
        parts.push(format!(
            "状态变化不足 {} 章（{plot_hits:?}）",
            plot_hits.len()
        ));
    }
    if !voice_hits.is_empty() {
        parts.push(format!(
            "对白不足 {} 章（{voice_hits:?}）",
            voice_hits.len()
        ));
    }
    if !pacing_hits.is_empty() {
        parts.push(format!("过短 {} 章（{pacing_hits:?}）", pacing_hits.len()));
    }
    if !hook_hits.is_empty() {
        parts.push(format!(
            "章末无强钩 {} 章（{hook_hits:?}）",
            hook_hits.len()
        ));
    }
    if result.unprobed_rules > 0 {
        parts.push(format!("未探针规则 {} 条", result.unprobed_rules));
    }
    if style_hits.is_empty()
        && ai_hits.is_empty()
        && style_baseline.is_some()
        && mainline_misses.is_empty()
        && continuity_hits.is_empty()
        && plot_hits.is_empty()
        && voice_hits.is_empty()
        && pacing_hits.is_empty()
        && hook_hits.is_empty()
    {
        parts.push("无回归失败".to_string());
    }

    result.line = format!("生产回归探针：{}", parts.join("｜"));
    result.failures = style_hits
        .iter()
        .map(|n| format!("第{n}章文风漂移"))
        .chain(ai_hits.iter().map(|n| format!("第{n}章 AI 腔")))
        .chain(mainline_misses.iter().map(|n| format!("第{n}章主线缺失")))
        .chain(
            continuity_hits
                .iter()
                .map(|n| format!("第{n}章承接开篇重复")),
        )
        .chain(plot_hits.iter().map(|n| format!("第{n}章无状态变化")))
        .chain(voice_hits.iter().map(|n| format!("第{n}章无对白互动")))
        .chain(pacing_hits.iter().map(|n| format!("第{n}章正文过短")))
        .chain(hook_hits.iter().map(|n| format!("第{n}章章末无强钩")))
        .collect();
    result
}

fn dimension_label(dim: &str) -> &'static str {
    match dim {
        "style" => "文笔风格",
        "plot" => "情节推进",
        "voice" => "人物口吻",
        "pacing" => "节奏密度",
        "hooks" => "钩子悬念",
        "continuity" => "承接",
        _ => "通用",
    }
}

const STATE_MARKERS: &[&str] = &[
    "发现", "决定", "答应", "拒绝", "交换", "抓住", "拦住", "威胁", "动手", "追", "逃", "赢", "输",
    "代价", "信息", "关系", "目标", "风险", "情绪",
];

const HOOK_MARKERS: &[&str] = &[
    "关门",
    "把门",
    "锁",
    "刀",
    "枪",
    "脚步声",
    "来人",
    "纸条",
    "短信",
    "电话",
    "威胁",
    "盯着",
    "突然",
    "下一秒",
    "发现",
];

fn has_any_marker(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn has_quoted_dialogue(text: &str) -> bool {
    text.contains('“')
        || text.contains('「')
        || text.contains('『')
        || text.contains("说道")
        || text.contains("问道")
        || style_probe::profile(text).dialogue_ratio > 0.01
}

fn duplicate_opening(current: &str, previous: &str) -> bool {
    let current = current.trim();
    let previous = previous.trim();
    if current.is_empty() || previous.is_empty() {
        return current.is_empty();
    }
    let cur_head: String = current.chars().take(120).collect();
    let prev_head: String = previous.chars().take(120).collect();
    if cur_head.chars().count() >= 80 && cur_head == prev_head {
        return true;
    }
    let prev_tail: String = previous
        .chars()
        .rev()
        .take(240)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let cur_prefix: String = current.chars().take(80).collect();
    cur_prefix.chars().count() >= 60 && prev_tail.contains(&cur_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agency::mainline::{MainlineBeat, MainlineProjection, MainlineSource},
        db::create_test_pool,
    };

    #[test]
    fn promotion_is_generic_idempotent_and_does_not_copy_names() {
        let pool = create_test_pool().unwrap();
        let scores =
            r#"{"overall":0.5,"scores":{"style":4.5,"voice":5.0,"plot":7.0},"continuity":4.0}"#;
        let improvement = "- 周成在福摩萨夜总会的主线未落地（第144章缺邵卫江）：下一轮先落地。\n- 文风修正：句长过短、冷硬腔。";
        let count = promote_from_report(&pool, "r1", scores, improvement);
        assert_eq!(count, 4, "style/voice/continuity + mainline signals");
        promote_from_report(&pool, "r2", scores, improvement);
        let repo = TrainingRuleRepository::new(pool.clone());
        let rules = repo.list_active().unwrap();
        assert!(rules.iter().any(|r| r.rule_key == "style.voice_baseline"));
        assert!(rules.iter().any(|r| r.rule_key == "voice.dialogue"));
        assert!(rules.iter().any(|r| r.rule_key == "mainline.landing"));
        assert!(
            rules
                .iter()
                .all(|r| !r.rule_text.contains("周成") && !r.rule_text.contains("福摩萨")),
            "规则不得复制具体人名/地点"
        );
        let style = rules
            .iter()
            .find(|r| r.rule_key == "style.voice_baseline")
            .unwrap();
        assert_eq!(style.promotion_count, 2, "重复报告累计次数");
    }

    #[test]
    fn prompt_block_uses_active_rules_and_is_bounded() {
        let pool = create_test_pool().unwrap();
        let scores = r#"{"scores":{"style":4.0,"hooks":4.5,"pacing":5.0}}"#;
        promote_from_report(&pool, "r1", scores, "- 章末钩子不足");
        let (block, count) = load_prompt_block(&pool);
        let block = block.unwrap();
        assert!(count >= 2);
        assert!(block.contains("训练报告沉淀通用规则"));
        assert!(block.contains("章末"));
        assert!(block.chars().count() <= PROMOTED_RULE_BLOCK_CHAR_CAP);
        assert!(!block.contains("《"));
    }

    #[test]
    fn production_probe_flags_style_ai_mainline_and_hook_regressions() {
        let pool = create_test_pool().unwrap();
        let scores = r#"{"scores":{"style":4.0,"hooks":4.5},"continuity":3.0}"#;
        promote_from_report(
            &pool,
            "r1",
            scores,
            "- AI 腔修正：后槽牙×2、比喻连用。\n- 主线未落地：下一轮先落地。",
        );
        let baseline = style_probe::profile(
            &"我蹲在门槛上，叼着半根烟，心里琢磨这买卖咋整，嘴上跟老曹扯两句啥，甭管值不值当。"
                .repeat(4),
        );
        let projection = MainlineProjection {
            arc: String::new(),
            beats: vec![MainlineBeat {
                chapter_number: 1,
                title: String::new(),
                goal: "周成去福摩萨找邵卫江。".into(),
                key_turn: String::new(),
                voice_cues: vec![],
                cast: vec!["邵卫江".into()],
                location: "福摩萨".into(),
                key_terms: vec!["邵卫江".into()],
                must_terms: vec!["邵卫江".into()],
            }],
            source: MainlineSource::StoryPlan,
        };
        let chapters = vec![
            (1, "他推门。灯灭了。风进来。街上没人。".to_string()),
            (2, "他推门。灯灭了。风进来。街上没人。".to_string()),
        ];
        let result = run_production_probes(&pool, &chapters, Some(&baseline), Some(&projection));
        assert!(result.active_rules >= 3);
        assert!(result.probed_rules >= 3);
        assert!(!result.failures.is_empty(), "{}", result.line);
        assert!(result.line.contains("文风") || result.line.contains("主线"));
        assert!(result.failures.iter().any(|f| f.contains("主线缺失")));
    }

    #[test]
    fn empty_or_clean_probe_reports_no_rules_or_no_failures() {
        let pool = create_test_pool().unwrap();
        let result = run_production_probes(&pool, &[(1, "正文".into())], None, None);
        assert_eq!(result.active_rules, 0);
        assert!(result.line.contains("无活跃规则"));
    }
}
