//! 持续学习·观察层（ECC observe 模式）：四类观察点 →
//! .storymoss/learning/observations.jsonl。 双轨制的文件轨：JSONL 追加写、10MB
//! 轮转、防自观察、payload 截断脱敏。

use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

// ---- analyzer in-flight 注册表（镜像 coordinator AGENCY_CANCEL_FLAGS 模式）
// ---- 同一 story 同一时刻至多一个 analyzer（手动 IPC 与 coordinator
// 自动触发互斥）； 缺失时分析在飞期间每条新观察都会再 spawn 一个 analyzer。

static ANALYZER_IN_FLIGHT: Lazy<Mutex<std::collections::HashSet<String>>> =
    Lazy::new(|| Mutex::new(std::collections::HashSet::new()));

/// 尝试标记 story 的 analyzer 在飞；已在飞返回 false（调用方应跳过本轮触发）。
pub(crate) fn analyzer_try_mark(story_id: &str) -> bool {
    let mut set = ANALYZER_IN_FLIGHT.lock().unwrap_or_else(|p| p.into_inner());
    set.insert(story_id.to_string())
}

pub(crate) fn analyzer_unmark(story_id: &str) {
    let mut set = ANALYZER_IN_FLIGHT.lock().unwrap_or_else(|p| p.into_inner());
    set.remove(story_id);
}

pub const LEARNING_DIR: &str = "learning";
pub const OBSERVATIONS_FILE: &str = "observations.jsonl";
pub const ANALYZER_STATE_FILE: &str = "analyzer_state.json";
pub const INSTINCTS_DIR: &str = "instincts";
const ROTATE_BYTES: u64 = 10 * 1024 * 1024; // 10MB
const PAYLOAD_MAX_CHARS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub ts: String,
    pub story_id: String,
    pub kind: String,
    pub actor: String,
    pub payload: serde_json::Value,
}

#[derive(Clone)]
pub struct ObservationLogger {
    /// T4 promotion 需要定位故事根目录（由 app_dir 推导）。
    pub(crate) app_dir: PathBuf,
}

impl ObservationLogger {
    pub fn new(app_dir: PathBuf) -> Self {
        Self { app_dir }
    }

    /// 契约：任何含 "observer" 的 label 不记录观察——用于 analyzer 自身调用
    /// 防自观察（其余 agency_* 角色 label 不含 observer，正常记录）。
    pub fn should_record(context_label: &str) -> bool {
        !context_label.contains("observer")
    }

    pub fn observations_path(&self, story_id: &str) -> PathBuf {
        self.app_dir
            .join("stories")
            .join(story_id)
            .join(".storymoss")
            .join(LEARNING_DIR)
            .join(OBSERVATIONS_FILE)
    }

    /// T2 analyzer/promotion 使用（直觉落盘目录）。
    fn instincts_path(&self, story_id: &str) -> PathBuf {
        self.app_dir
            .join("stories")
            .join(story_id)
            .join(".storymoss")
            .join(LEARNING_DIR)
            .join(INSTINCTS_DIR)
    }

    fn state_path(&self, story_id: &str) -> PathBuf {
        self.observations_path(story_id)
            .parent()
            .unwrap()
            .join(ANALYZER_STATE_FILE)
    }

    /// 追加一条观察（同步、best-effort：任何 IO 错误只 warn 不传播）。
    pub fn log(&self, story_id: &str, kind: &str, actor: &str, payload: serde_json::Value) {
        if let Err(e) = self.log_inner(story_id, kind, actor, payload) {
            log::warn!("observation log 失败（忽略）: {}", e);
        }
    }

    fn log_inner(
        &self,
        story_id: &str,
        kind: &str,
        actor: &str,
        payload: serde_json::Value,
    ) -> Result<(), String> {
        let path = self.observations_path(story_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        self.rotate_if_needed(&path, ROTATE_BYTES)?;
        let observation = Observation {
            ts: chrono::Local::now().to_rfc3339(),
            story_id: story_id.to_string(),
            kind: kind.to_string(),
            actor: actor.to_string(),
            payload: truncate_payload(payload),
        };
        let line = serde_json::to_string(&observation).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        writeln!(file, "{}", line).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 10MB 轮转：保留尾部 ~max_bytes（按行对齐）。
    pub fn rotate_if_needed(&self, path: &Path, max_bytes: u64) -> Result<(), String> {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return Ok(()),
        };
        if meta.len() <= max_bytes {
            return Ok(());
        }
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut keep_from = content.len().saturating_sub(max_bytes as usize);
        while keep_from < content.len() && !content.is_char_boundary(keep_from) {
            keep_from += 1;
        }
        // 对齐到行边界
        let aligned = content[keep_from..]
            .find('\n')
            .map(|i| keep_from + i + 1)
            .unwrap_or(keep_from);
        let tail = &content[aligned.min(content.len())..];
        std::fs::write(path, tail).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn recent(&self, story_id: &str, n: usize) -> Vec<Observation> {
        let path = self.observations_path(story_id);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut items: Vec<Observation> = content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        if items.len() > n {
            items = items.split_off(items.len() - n);
        }
        items
    }

    pub fn count_unanalyzed(&self, story_id: &str) -> usize {
        self.total_lines(story_id)
            .saturating_sub(self.analyzed_through(story_id))
    }

    /// 观察文件当前总行数（游标语义：已分析观察 = 前 analyzed_through 行）。
    fn total_lines(&self, story_id: &str) -> usize {
        self.observations_path(story_id)
            .exists()
            .then(|| {
                std::fs::read_to_string(self.observations_path(story_id))
                    .map(|c| c.lines().count())
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    fn analyzed_through(&self, story_id: &str) -> usize {
        let path = self.state_path(story_id);
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
            .and_then(|v| v.get("analyzed_through").and_then(|x| x.as_u64()))
            .map(|x| x as usize)
            .unwrap_or(0)
    }

    /// 推进分析游标到 `through`（调用方在分析开始时快照的总行数；分析期间
    /// 新增的观察不计入本轮，避免被误标为已分析）。
    pub fn mark_analyzed(
        &self,
        story_id: &str,
        through: usize,
    ) -> Result<(), crate::error::AppError> {
        let path = self.state_path(story_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(crate::error::AppError::from)?;
        }
        let state = serde_json::json!({
            "analyzed_through": through,
            "last_analysis_ts": chrono::Local::now().to_rfc3339(),
        });
        std::fs::write(&path, state.to_string()).map_err(crate::error::AppError::from)?;
        Ok(())
    }
}

fn truncate_payload(payload: serde_json::Value) -> serde_json::Value {
    match payload {
        serde_json::Value::Object(map) => {
            let truncated = map
                .into_iter()
                .map(|(k, v)| {
                    let v = match &v {
                        serde_json::Value::String(s) if s.chars().count() > PAYLOAD_MAX_CHARS => {
                            serde_json::Value::String(format!(
                                "{}…(截断)",
                                s.chars().take(PAYLOAD_MAX_CHARS).collect::<String>()
                            ))
                        }
                        other => other.clone(),
                    };
                    (k, v)
                })
                .collect();
            serde_json::Value::Object(truncated)
        }
        other => other,
    }
}

// ---- T2 analyzer：观察 → instinct（后台分析，best-effort）----

/// 未分析观察累计阈值：达到后由 coordinator.log_observation 自动触发后台分析。
pub const ANALYZE_THRESHOLD: usize = 20;
/// 手动/自动分析的最小新观察数（低于则不调用 LLM、不推进游标）。
/// 前端经 LearningOverview.analyze_min_new 透出（学习中心按钮阈值）。
pub const ANALYZE_MIN_NEW: usize = 2;
/// analyzer 自身的路由/观察标签（双约束，test_analyzer_label_dual_constraint
/// 锁死）： strip "agency_" → "editor_observer" → starts_with("editor") 命中
/// Background 档； contains("observer") → should_record 过滤其 llm_call
/// 埋点（防自观察）。
pub const ANALYZER_LABEL: &str = "agency_editor_observer";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Instinct {
    pub id: String,
    pub trigger: String,
    pub action: String,
    pub confidence: f64,
    pub evidence_count: u32,
    pub scope: String,  // story | global
    pub status: String, // pending | candidate | promoted | rejected
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub evolved_from: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AnalyzeOutcome {
    pub new_instincts: usize,
    pub updated_instincts: usize,
    pub analyzed: usize,
}

pub fn confidence_for_evidence(count: u32) -> f64 {
    match count {
        0..=2 => 0.3,
        3..=5 => 0.5,
        6..=10 => 0.7,
        _ => 0.85,
    }
}

pub fn list_instincts(
    logger: &ObservationLogger,
    story_id: &str,
) -> Result<Vec<Instinct>, crate::error::AppError> {
    let dir = logger.instincts_path(story_id);
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(out),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|x| x == "md").unwrap_or(false) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Some(inst) = parse_instinct(&text) {
                    out.push(inst);
                }
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub fn parse_instinct(text: &str) -> Option<Instinct> {
    let (fm, _body) = crate::prompts::registry::split_frontmatter(text)?;
    #[derive(serde::Deserialize)]
    struct Fm {
        id: String,
        trigger: String,
        action: String,
        confidence: f64,
        evidence_count: u32,
        scope: String,
        status: String,
        created_at: String,
        updated_at: String,
        #[serde(default)]
        evolved_from: Vec<String>,
    }
    let fm: Fm = serde_yaml::from_str(fm).ok()?;
    Some(Instinct {
        id: fm.id,
        trigger: fm.trigger,
        action: fm.action,
        confidence: fm.confidence,
        evidence_count: fm.evidence_count,
        scope: fm.scope,
        status: fm.status,
        created_at: fm.created_at,
        updated_at: fm.updated_at,
        evolved_from: fm.evolved_from,
    })
}

fn render_instinct(inst: &Instinct, body: &str) -> String {
    // evolved_from 用 JSON 渲染（YAML flow 序列兼容 JSON）：块式 YAML 嵌进单行
    // 会破坏 frontmatter 行结构导致 parse_instinct 失败。
    let evolved = serde_json::to_string(&inst.evolved_from).unwrap_or_else(|_| "[]".into());
    // 字符串字段经 serde_yaml 序列化（自动加引号/转义），替代 {:?} 的 Rust 调试
    // 转义——后者遇控制字符可能产生非法 YAML。serde_yaml 输出带尾部换行，去掉。
    let yaml_scalar = |s: &str| -> String {
        serde_yaml::to_string(s)
            .map(|y| y.trim_end_matches('\n').to_string())
            .unwrap_or_else(|_| format!("{:?}", s))
    };
    format!(
        "---\nid: {}\ntrigger: {}\naction: {}\nconfidence: {}\nevidence_count: {}\nscope: {}\nstatus: {}\ncreated_at: {:?}\nupdated_at: {:?}\nevolved_from: {}\n---\n\n{}\n",
        inst.id, yaml_scalar(&inst.trigger), yaml_scalar(&inst.action), inst.confidence, inst.evidence_count,
        inst.scope, inst.status, inst.created_at, inst.updated_at,
        evolved,
        body
    )
}

pub async fn analyze_story(
    llm: std::sync::Arc<dyn crate::agency::tool_loop::LoopLlm>,
    logger: &ObservationLogger,
    story_id: &str,
) -> Result<AnalyzeOutcome, crate::error::AppError> {
    // 游标快照：本轮只覆盖此刻之前的观察；分析进行中新增的观察留给下一轮
    //（结束后 mark_analyzed(through) 不把它们误标为已分析）。
    let through = logger.total_lines(story_id);
    let new_count = through.saturating_sub(logger.analyzed_through(story_id));
    if new_count < ANALYZE_MIN_NEW {
        return Ok(AnalyzeOutcome {
            new_instincts: 0,
            updated_instincts: 0,
            analyzed: 0,
        });
    }
    let observations = logger.recent(story_id, 50);
    let existing = list_instincts(logger, story_id).unwrap_or_default();
    let digest: String = observations
        .iter()
        .map(|o| {
            format!(
                "- [{}] {} by {}: {}",
                o.ts.get(..10).unwrap_or(&o.ts),
                o.kind,
                o.actor,
                o.payload
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let existing_digest: String = existing
        .iter()
        .map(|i| format!("- {}（evidence:{}）", i.trigger, i.evidence_count))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "你是创作模式分析器。以下是小说创作过程的最近观察与既有模式（instinct）。\n\
         任务：归纳出 0-3 条可复用的创作模式（trigger=何时适用，action=可操作的创作指导，evolved_from=相关观察 kind）。\n\
         规则：只输出 YAML 列表（```yaml 包裹）；不要泛化无依据的模式；与既有模式重复的给出相同 trigger 以便归并。\n\n\
         最近观察：\n{}\n\n既有模式：\n{}",
        digest, if existing_digest.is_empty() { "（无）".into() } else { existing_digest }
    );
    let raw = llm
        .complete(
            "你是创作模式分析器，只输出 YAML。",
            &prompt,
            crate::router::TaskType::Analysis,
            1500,
        )
        .await?;
    let proposals = parse_analyzer_yaml(&raw);
    let mut new_instincts = 0usize;
    let mut updated_instincts = 0usize;
    for proposal in proposals {
        let dir = logger.instincts_path(story_id);
        std::fs::create_dir_all(&dir).map_err(crate::error::AppError::from)?;
        if let Some(mut hit) = existing
            .iter()
            .find(|e| e.trigger == proposal.trigger)
            .cloned()
        {
            hit.evidence_count += new_count as u32;
            hit.confidence = confidence_for_evidence(hit.evidence_count);
            hit.updated_at = chrono::Local::now().to_rfc3339();
            std::fs::write(
                dir.join(format!("{}.md", hit.id)),
                render_instinct(&hit, "（更新：证据累积）"),
            )
            .map_err(crate::error::AppError::from)?;
            updated_instincts += 1;
        } else {
            let now = chrono::Local::now().to_rfc3339();
            // 新建 instinct 从 1 条证据起（本轮归纳出的模式本身即第一条证据）；
            // 后续同 trigger 轮次按 new_count 累积。
            let inst = Instinct {
                id: format!(
                    "inst-{}-{:06x}",
                    now.get(..10).unwrap_or(&now).replace('-', ""),
                    crc32_simple(&proposal.trigger)
                ),
                trigger: proposal.trigger.clone(),
                action: proposal.action.clone(),
                confidence: confidence_for_evidence(1),
                evidence_count: 1,
                scope: "story".to_string(),
                status: "pending".to_string(),
                created_at: now.clone(),
                updated_at: now,
                evolved_from: proposal.evolved_from.clone(),
            };
            let body = format!(
                "## 模式描述\n{}\n\n## 证据摘要\n（来自最近 {} 条观察）",
                proposal.action, new_count
            );
            std::fs::write(
                dir.join(format!("{}.md", inst.id)),
                render_instinct(&inst, &body),
            )
            .map_err(crate::error::AppError::from)?;
            new_instincts += 1;
        }
    }
    logger.mark_analyzed(story_id, through)?;
    Ok(AnalyzeOutcome {
        new_instincts,
        updated_instincts,
        analyzed: new_count,
    })
}

#[derive(Debug)]
struct AnalyzerProposal {
    trigger: String,
    action: String,
    evolved_from: Vec<String>,
}

fn parse_analyzer_yaml(raw: &str) -> Vec<AnalyzerProposal> {
    // 截取 ```yaml ... ``` 或首个 '- trigger' 起的列表
    let body = if let (Some(s), Some(e)) = (raw.find("```yaml"), raw.rfind("```")) {
        raw.get(s + 7..e).unwrap_or(raw)
    } else {
        raw
    };
    #[derive(serde::Deserialize)]
    struct P {
        trigger: String,
        action: String,
        #[serde(default)]
        evolved_from: Vec<String>,
    }
    let items: Vec<P> = serde_yaml::from_str(body).unwrap_or_default();
    items
        .into_iter()
        .map(|p| AnalyzerProposal {
            trigger: p.trigger,
            action: p.action,
            evolved_from: p.evolved_from,
        })
        .collect()
}

fn crc32_simple(s: &str) -> u32 {
    // 简单稳定散列（非加密）：FNV-1a
    let mut hash: u32 = 2166136261;
    for b in s.as_bytes() {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash & 0xFFFFFF
}

// ---- T3 置信度引擎：反馈 / 周衰减 / prune（best-effort）----

pub const FEEDBACK_ACCEPT: f64 = 0.05;
pub const FEEDBACK_REJECT: f64 = -0.1;
pub const WEEKLY_DECAY: f64 = -0.02;
pub const PRUNE_CONFIDENCE: f64 = 0.2;
pub const PRUNE_TTL_DAYS: i64 = 90;

fn read_instinct_file(
    logger: &ObservationLogger,
    story_id: &str,
    id: &str,
) -> Result<(Instinct, std::path::PathBuf), crate::error::AppError> {
    let path = logger.instincts_path(story_id).join(format!("{}.md", id));
    let text = std::fs::read_to_string(&path).map_err(|e| {
        crate::error::AppError::validation_failed(
            format!("instinct 不存在: {} ({})", id, e),
            None::<String>,
        )
    })?;
    let inst = parse_instinct(&text).ok_or_else(|| {
        crate::error::AppError::validation_failed(
            format!("instinct 解析失败: {}", id),
            None::<String>,
        )
    })?;
    Ok((inst, path))
}

fn write_instinct_file(
    path: &std::path::Path,
    inst: &Instinct,
) -> Result<(), crate::error::AppError> {
    std::fs::write(path, render_instinct(inst, "（反馈/衰减更新）"))
        .map_err(crate::error::AppError::from)
}

pub fn apply_feedback(
    logger: &ObservationLogger,
    story_id: &str,
    instinct_id: &str,
    accepted: bool,
) -> Result<Instinct, crate::error::AppError> {
    let (mut inst, path) = read_instinct_file(logger, story_id, instinct_id)?;
    let delta = if accepted {
        FEEDBACK_ACCEPT
    } else {
        FEEDBACK_REJECT
    };
    inst.confidence = (inst.confidence + delta).clamp(0.0, 1.0);
    inst.updated_at = chrono::Local::now().to_rfc3339();
    write_instinct_file(&path, &inst)?;
    Ok(inst)
}

pub fn apply_weekly_decay(
    logger: &ObservationLogger,
    story_id: &str,
) -> Result<usize, crate::error::AppError> {
    let instincts = list_instincts(logger, story_id)?;
    let now = chrono::Local::now();
    let mut decayed = 0usize;
    for mut inst in instincts {
        if inst.status == "promoted" {
            continue; // T4：晋升产物不被衰减管道误伤
        }
        let updated = chrono::DateTime::parse_from_rfc3339(&inst.updated_at)
            .map(|d| d.with_timezone(&chrono::Local))
            .unwrap_or(now);
        let weeks = (now - updated).num_days() / 7;
        if weeks >= 1 {
            inst.confidence = (inst.confidence + weeks as f64 * WEEKLY_DECAY).clamp(0.0, 1.0);
            inst.updated_at = now.to_rfc3339();
            let path = logger
                .instincts_path(story_id)
                .join(format!("{}.md", inst.id));
            write_instinct_file(&path, &inst)?;
            decayed += 1;
        }
    }
    Ok(decayed)
}

pub fn prune_instincts(
    logger: &ObservationLogger,
    story_id: &str,
) -> Result<usize, crate::error::AppError> {
    let instincts = list_instincts(logger, story_id)?;
    let now = chrono::Local::now();
    let mut pruned = 0usize;
    for inst in instincts {
        if inst.status == "promoted" {
            continue; // T4：晋升产物不被清理管道误伤
        }
        let updated = chrono::DateTime::parse_from_rfc3339(&inst.updated_at)
            .map(|d| d.with_timezone(&chrono::Local))
            .unwrap_or(now);
        let stale_days = (now - updated).num_days();
        let should_prune = inst.confidence < PRUNE_CONFIDENCE
            || (inst.status == "pending" && stale_days >= PRUNE_TTL_DAYS);
        if should_prune {
            let path = logger
                .instincts_path(story_id)
                .join(format!("{}.md", inst.id));
            if std::fs::remove_file(&path).is_ok() {
                pruned += 1;
            }
        }
    }
    Ok(pruned)
}

// ---- T4 晋升管线：候选 → confirm/reject → 物化为目录技能（best-effort）----

pub const PROMOTE_CONFIDENCE: f64 = 0.8;
pub const PROMOTE_MIN_STORIES: usize = 2;

pub fn promotion_candidates(
    logger: &ObservationLogger,
    story_id: &str,
) -> Result<Vec<Instinct>, crate::error::AppError> {
    // 跨 story 统计 trigger 出现次数
    let counts = trigger_story_counts(logger)?;
    let instincts = list_instincts(logger, story_id)?;
    Ok(instincts
        .into_iter()
        .filter(|i| {
            (i.status == "pending" || i.status == "candidate")
                && i.confidence >= PROMOTE_CONFIDENCE
                && counts.get(&i.trigger).copied().unwrap_or(0) >= PROMOTE_MIN_STORIES
        })
        .collect())
}

/// 扫描全部 story 的 learning/instincts，统计每个 trigger 出现在多少个 story。
fn trigger_story_counts(
    logger: &ObservationLogger,
) -> Result<std::collections::HashMap<String, usize>, crate::error::AppError> {
    let stories_dir = logger.app_dir.join("stories");
    let mut map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let entries = match std::fs::read_dir(&stories_dir) {
        Ok(e) => e,
        Err(_) => return Ok(map),
    };
    for entry in entries.flatten() {
        let story_dir = entry.path();
        if !story_dir.is_dir() {
            continue;
        }
        let story_id = entry.file_name().to_string_lossy().to_string();
        let instincts = list_instincts(logger, &story_id).unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        for inst in instincts {
            seen.insert(inst.trigger);
        }
        for trigger in seen {
            *map.entry(trigger).or_insert(0) += 1;
        }
    }
    Ok(map)
}

pub fn reject_promotion(
    logger: &ObservationLogger,
    story_id: &str,
    instinct_id: &str,
) -> Result<Instinct, crate::error::AppError> {
    let (mut inst, path) = read_instinct_file(logger, story_id, instinct_id)?;
    inst.confidence = (inst.confidence + FEEDBACK_REJECT).clamp(0.0, 1.0);
    inst.status = "rejected".to_string();
    inst.updated_at = chrono::Local::now().to_rfc3339();
    write_instinct_file(&path, &inst)?;
    Ok(inst)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PromoteOutcome {
    pub instinct: Instinct,
    pub skill_id: String,
}

pub fn confirm_promotion(
    logger: &ObservationLogger,
    story_id: &str,
    instinct_id: &str,
    skills_dir: &std::path::Path,
) -> Result<PromoteOutcome, crate::error::AppError> {
    // 校验候选资格
    let candidates = promotion_candidates(logger, story_id)?;
    if !candidates.iter().any(|i| i.id == instinct_id) {
        return Err(crate::error::AppError::validation_failed(
            format!(
                "instinct {} 不满足晋升条件（需 confidence≥{} 且跨 {} 个 story 复现）",
                instinct_id, PROMOTE_CONFIDENCE, PROMOTE_MIN_STORIES
            ),
            None::<String>,
        ));
    }
    let skill_dir = materialize_as_skill(logger, story_id, instinct_id, skills_dir)?;
    // instinct 状态与作用域更新
    let (mut inst, path) = read_instinct_file(logger, story_id, instinct_id)?;
    inst.status = "promoted".to_string();
    inst.scope = "global".to_string();
    inst.updated_at = chrono::Local::now().to_rfc3339();
    write_instinct_file(&path, &inst)?;
    Ok(PromoteOutcome {
        skill_id: skill_dir.file_name().unwrap().to_string_lossy().to_string(),
        instinct: inst,
    })
}

/// 物化为 skill.yaml 目录技能（纯文件操作；注册由 commands 层经
/// SkillManager::import_skill 完成）。
pub fn materialize_as_skill(
    logger: &ObservationLogger,
    story_id: &str,
    instinct_id: &str,
    skills_dir: &std::path::Path,
) -> Result<std::path::PathBuf, crate::error::AppError> {
    let (inst, _) = read_instinct_file(logger, story_id, instinct_id)?;
    let skill_id = format!("learned.{}", inst.id);
    let skill_dir = skills_dir.join(&skill_id);
    std::fs::create_dir_all(&skill_dir).map_err(crate::error::AppError::from)?;
    let name: String = inst.trigger.chars().take(30).collect();
    let manifest = format!(
        "id: {}\nname: \"学到的模式：{}\"\nversion: \"0.1.0\"\ndescription: {:?}\nauthor: \"StoryMoss Learning\"\ncategory: custom\nentry_point: \"main.prompt\"\nparameters: []\ncapabilities: []\nhooks: []\nconfig:\n  evolved_from: {:?}\n  confidence: {}\n",
        skill_id, name, inst.trigger, inst.id, inst.confidence
    );
    std::fs::write(skill_dir.join("skill.yaml"), manifest).map_err(crate::error::AppError::from)?;
    let prompt = format!(
        "你是小说创作助手。以下是从创作过程学到的模式，请在适用时遵循：\n\n触发条件：{}\n指导动作：{}\n---\n{{{{instruction}}}}\n",
        inst.trigger, inst.action
    );
    std::fs::write(skill_dir.join("main.prompt"), prompt).map_err(crate::error::AppError::from)?;
    Ok(skill_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logger() -> (ObservationLogger, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        (ObservationLogger::new(tmp.path().to_path_buf()), tmp)
    }

    #[test]
    fn test_log_and_recent() {
        let (logger, _tmp) = logger();
        logger.log(
            "s1",
            "gate",
            "editor_auditor",
            serde_json::json!({"outcome": "pass", "weighted": 0.82}),
        );
        logger.log(
            "s1",
            "llm_call",
            "lead_writer",
            serde_json::json!({"tokens": 100}),
        );
        logger.log(
            "s2",
            "gate",
            "editor_auditor",
            serde_json::json!({"outcome": "revise"}),
        );
        let recent = logger.recent("s1", 10);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].kind, "gate");
        assert_eq!(recent[1].kind, "llm_call");
        assert_eq!(recent[0].payload["weighted"].as_f64().unwrap(), 0.82);
        assert_eq!(logger.recent("s2", 10).len(), 1);
    }

    #[test]
    fn test_count_unanalyzed() {
        let (logger, _tmp) = logger();
        for i in 0..5 {
            logger.log("s1", "gate", "editor_auditor", serde_json::json!({"i": i}));
        }
        assert_eq!(logger.count_unanalyzed("s1"), 5);
        logger.mark_analyzed("s1", 5).unwrap();
        assert_eq!(logger.count_unanalyzed("s1"), 0);
        logger.log("s1", "gate", "editor_auditor", serde_json::json!({"i": 9}));
        assert_eq!(logger.count_unanalyzed("s1"), 1);
    }

    #[test]
    fn test_should_record() {
        // 契约：任何含 "observer" 的 label 不记录；其余 agency_* 角色正常记录
        assert!(ObservationLogger::should_record("agency_writer"));
        assert!(ObservationLogger::should_record("agency_producer"));
        assert!(ObservationLogger::should_record("agency_editor"));
        assert!(!ObservationLogger::should_record("agency_observer"));
        assert!(!ObservationLogger::should_record(
            "agency_observer_analyzer"
        ));
        assert!(!ObservationLogger::should_record("agency_editor_observer"));
    }

    #[test]
    fn test_analyzer_label_dual_constraint() {
        // 双约束锁死：Background 档路由 + 防自观察过滤，缺一则回归
        assert_eq!(
            crate::llm::service::derive_model_role_from_label(Some(ANALYZER_LABEL)),
            Some(crate::config::settings::ModelRole::Background)
        );
        assert!(!ObservationLogger::should_record(ANALYZER_LABEL));
    }

    #[test]
    fn test_rotation_keeps_tail() {
        let (logger, _tmp) = logger();
        let path = logger.observations_path("s1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let big_line = serde_json::json!({"ts":"t","story_id":"s1","kind":"gate","actor":"x","payload":{"blob":"x".repeat(1000)}}).to_string();
        // 写超过阈值（测试用小阈值注入）
        let mut content = String::new();
        for _ in 0..200 {
            content.push_str(&big_line);
            content.push('\n');
        }
        std::fs::write(&path, &content).unwrap();
        logger.rotate_if_needed(&path, 10_000).unwrap(); // 测试阈值 10KB
        let after = std::fs::read_to_string(&path).unwrap();
        assert!(
            after.len() <= 10_000 + 1200,
            "轮转后应接近阈值: {}",
            after.len()
        );
        assert!(after.ends_with('\n'));
        // 保留的是尾部行
        let lines: Vec<&str> = after.lines().collect();
        assert!(!lines.is_empty());
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["kind"], "gate");
    }

    #[test]
    fn test_rotation_multibyte_char_boundary() {
        let (logger, _tmp) = logger();
        let path = logger.observations_path("s1");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // 全多字节字符行（'汉' 3 字节）：keep_from=1 落在首字符内部，
        // 修复前 content[keep_from..] 必 panic
        let line = "汉".repeat(100);
        let mut content = String::new();
        for _ in 0..10 {
            content.push_str(&line);
            content.push('\n');
        }
        std::fs::write(&path, &content).unwrap();
        let max = content.len() as u64 - 1; // 文件超阈值 1 字节 → keep_from=1（非 char 边界）
        logger.rotate_if_needed(&path, max).unwrap();
        let after = std::fs::read_to_string(&path).unwrap();
        // 尾部完整 UTF-8、按行对齐（丢弃首行，保留 9 行完整行）
        assert!(after.ends_with('\n'));
        assert_eq!(after.lines().count(), 9);
        assert_eq!(after.lines().next().unwrap(), line);
    }

    #[test]
    fn test_payload_truncation() {
        let (logger, _tmp) = logger();
        let long = "长".repeat(2000);
        logger.log(
            "s1",
            "user_edit",
            "human",
            serde_json::json!({"note": long}),
        );
        let recent = logger.recent("s1", 1);
        let note = recent[0].payload["note"].as_str().unwrap();
        assert!(
            note.chars().count() <= 520,
            "payload 应截断: {}",
            note.len()
        );
    }

    struct MockAnalyzerLlm {
        response: String,
    }

    #[async_trait::async_trait]
    impl crate::agency::tool_loop::LoopLlm for MockAnalyzerLlm {
        async fn complete(
            &self,
            _s: &str,
            _u: &str,
            _t: crate::router::TaskType,
            _m: i32,
        ) -> Result<String, crate::error::AppError> {
            Ok(self.response.clone())
        }
    }

    fn analyzer_mock() -> std::sync::Arc<MockAnalyzerLlm> {
        std::sync::Arc::new(MockAnalyzerLlm {
            response: r#"```yaml
- trigger: "当编辑审计连续两轮判定 revise"
  action: "修订前先复读资产区角色卡与大纲"
  evolved_from: ["gate", "revision"]
```"#
                .to_string(),
        })
    }

    #[tokio::test]
    async fn test_analyze_creates_instinct_files() {
        let (logger, _tmp) = logger();
        for i in 0..3 {
            logger.log(
                "s1",
                "gate",
                "editor_auditor",
                serde_json::json!({"outcome": "revise", "i": i}),
            );
        }
        let outcome = analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
        assert_eq!(outcome.new_instincts, 1);
        assert_eq!(outcome.analyzed, 3);
        assert_eq!(logger.count_unanalyzed("s1"), 0);
        let instincts = list_instincts(&logger, "s1").unwrap();
        assert_eq!(instincts.len(), 1);
        let inst = &instincts[0];
        assert!(inst.trigger.contains("连续两轮"));
        assert!(inst.action.contains("复读"));
        assert!((inst.confidence - 0.3).abs() < 0.001); // evidence_count=1 → 0.3
        assert_eq!(inst.status, "pending");
        assert_eq!(inst.scope, "story");
    }

    #[tokio::test]
    async fn test_analyze_updates_existing_instinct() {
        let (logger, _tmp) = logger();
        // 第一轮：≥ANALYZE_MIN_NEW 条观察触发分析，建立 instinct（evidence=1 → 0.3）
        for i in 0..3 {
            logger.log(
                "s1",
                "gate",
                "editor_auditor",
                serde_json::json!({"outcome": "revise", "i": i}),
            );
        }
        analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
        // 同 trigger 再来一轮观察 + 分析 → 同 trigger instinct 的 evidence_count
        // 递增、confidence 升档
        for _ in 0..4 {
            logger.log(
                "s1",
                "revision",
                "editor_auditor",
                serde_json::json!({"chapter": 1}),
            );
        }
        let outcome = analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
        assert_eq!(outcome.updated_instincts, 1);
        assert_eq!(outcome.new_instincts, 0);
        let instincts = list_instincts(&logger, "s1").unwrap();
        assert_eq!(instincts.len(), 1);
        assert_eq!(instincts[0].evidence_count, 5);
        assert!((instincts[0].confidence - 0.5).abs() < 0.001); // 3-5 → 0.5
    }

    #[tokio::test]
    async fn test_analyze_skips_when_insufficient() {
        let (logger, _tmp) = logger();
        logger.log(
            "s1",
            "gate",
            "editor_auditor",
            serde_json::json!({"outcome": "pass"}),
        );
        let outcome = analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
        assert_eq!(outcome.analyzed, 0);
        assert_eq!(outcome.new_instincts, 0);
        // 未达到最小样本（<2 条新观察）不调用 LLM、不推进游标
        assert_eq!(logger.count_unanalyzed("s1"), 1);
    }

    /// 模拟 cursor race：LLM 调用（分析进行）期间有新观察到账。
    struct MidFlightLoggingLlm {
        logger: ObservationLogger,
        story_id: String,
        response: String,
    }

    #[async_trait::async_trait]
    impl crate::agency::tool_loop::LoopLlm for MidFlightLoggingLlm {
        async fn complete(
            &self,
            _s: &str,
            _u: &str,
            _t: crate::router::TaskType,
            _m: i32,
        ) -> Result<String, crate::error::AppError> {
            self.logger.log(
                &self.story_id,
                "gate",
                "editor_auditor",
                serde_json::json!({"mid_flight": true}),
            );
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn test_analyze_cursor_excludes_mid_flight_observations() {
        let (logger, _tmp) = logger();
        for i in 0..3 {
            logger.log("s1", "gate", "editor_auditor", serde_json::json!({"i": i}));
        }
        let llm = std::sync::Arc::new(MidFlightLoggingLlm {
            logger: logger.clone(),
            story_id: "s1".to_string(),
            response: "```yaml\n[]\n```".to_string(),
        });
        let outcome = analyze_story(llm, &logger, "s1").await.unwrap();
        assert_eq!(outcome.analyzed, 3);
        // 分析期间新增的观察不被误标为已分析（count_unanalyzed 仍计它）
        assert_eq!(logger.count_unanalyzed("s1"), 1);
    }

    #[test]
    fn test_render_instinct_roundtrip_special_chars() {
        // trigger/action 含引号/换行/制表符等：serde_yaml 序列化保证 round-trip，
        // 不产生非法 YAML（修复前 {:?} 调试转义遇控制字符可能断裂 frontmatter）
        let inst = Instinct {
            id: "inst-special".to_string(),
            trigger: "含\"引号\"与\n换行、制表\t符".to_string(),
            action: "动作：带冒号 #井号".to_string(),
            confidence: 0.5,
            evidence_count: 3,
            scope: "story".to_string(),
            status: "pending".to_string(),
            created_at: "2026-07-19T00:00:00+08:00".to_string(),
            updated_at: "2026-07-19T00:00:00+08:00".to_string(),
            evolved_from: vec!["gate".to_string()],
        };
        let text = render_instinct(&inst, "body");
        let parsed = parse_instinct(&text).expect("含特殊字符的 instinct 渲染后应可解析");
        assert_eq!(parsed.trigger, inst.trigger);
        assert_eq!(parsed.action, inst.action);
        assert_eq!(parsed.evolved_from, inst.evolved_from);
    }

    #[test]
    fn test_analyzer_in_flight_try_mark_unmark() {
        // 唯一 story_id 避免与并发用例互相污染
        let sid = "test-analyzer-in-flight-dedup";
        analyzer_unmark(sid); // 起始干净
        assert!(analyzer_try_mark(sid), "首次标记应成功");
        assert!(!analyzer_try_mark(sid), "在飞期间第二次标记应被拒绝");
        analyzer_unmark(sid);
        assert!(analyzer_try_mark(sid), "unmark 后应可再标记");
        analyzer_unmark(sid);
    }

    fn seed_instinct(
        logger: &ObservationLogger,
        story_id: &str,
        id: &str,
        confidence: f64,
        updated_at: &str,
    ) {
        let inst = Instinct {
            id: id.to_string(),
            trigger: "测试触发".to_string(),
            action: "测试动作".to_string(),
            confidence,
            evidence_count: 3,
            scope: "story".to_string(),
            status: "pending".to_string(),
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
            evolved_from: vec![],
        };
        let dir = logger.instincts_path(story_id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{}.md", id)),
            render_instinct(&inst, "body"),
        )
        .unwrap();
    }

    /// 置为 promoted 状态（T4 豁免断言用：晋升产物不被衰减/清理管道误伤）。
    fn seed_promoted(
        logger: &ObservationLogger,
        story_id: &str,
        id: &str,
        confidence: f64,
        updated_at: &str,
    ) {
        seed_instinct(logger, story_id, id, confidence, updated_at);
        let (mut inst, path) = read_instinct_file(logger, story_id, id).unwrap();
        inst.status = "promoted".to_string();
        write_instinct_file(&path, &inst).unwrap();
    }

    #[test]
    fn test_feedback_accept_and_reject() {
        let (logger, _tmp) = logger();
        seed_instinct(&logger, "s1", "inst-a", 0.5, "2026-07-01T00:00:00+08:00");
        let updated = apply_feedback(&logger, "s1", "inst-a", true).unwrap();
        assert!((updated.confidence - 0.55).abs() < 0.001);
        let updated2 = apply_feedback(&logger, "s1", "inst-a", false).unwrap();
        assert!((updated2.confidence - 0.45).abs() < 0.001);
        // 下限 clamp
        seed_instinct(&logger, "s1", "inst-b", 0.05, "2026-07-01T00:00:00+08:00");
        let clamped = apply_feedback(&logger, "s1", "inst-b", false).unwrap();
        assert!(clamped.confidence >= 0.0);
    }

    #[test]
    fn test_weekly_decay() {
        let (logger, _tmp) = logger();
        let old = chrono::Local::now() - chrono::Duration::days(14);
        seed_instinct(&logger, "s1", "inst-old", 0.5, &old.to_rfc3339());
        let fresh = chrono::Local::now() - chrono::Duration::days(3);
        seed_instinct(&logger, "s1", "inst-fresh", 0.5, &fresh.to_rfc3339());
        // T4：promoted 晋升产物豁免衰减
        seed_promoted(&logger, "s1", "inst-promoted", 0.9, &old.to_rfc3339());
        let decayed = apply_weekly_decay(&logger, "s1").unwrap();
        assert_eq!(decayed, 1); // 只有 14 天前的那条衰减（每满 7 天 -0.02，14 天 -0.04）
        let instincts = list_instincts(&logger, "s1").unwrap();
        let old_inst = instincts.iter().find(|i| i.id == "inst-old").unwrap();
        assert!((old_inst.confidence - 0.46).abs() < 0.001);
        let fresh_inst = instincts.iter().find(|i| i.id == "inst-fresh").unwrap();
        assert!((fresh_inst.confidence - 0.5).abs() < 0.001);
        let promoted = instincts.iter().find(|i| i.id == "inst-promoted").unwrap();
        assert!(
            (promoted.confidence - 0.9).abs() < 0.001,
            "promoted instinct 不应被周衰减"
        );
    }

    #[test]
    fn test_prune() {
        let (logger, _tmp) = logger();
        seed_instinct(
            &logger,
            "s1",
            "inst-weak",
            0.1,
            &chrono::Local::now().to_rfc3339(),
        );
        let old = chrono::Local::now() - chrono::Duration::days(100);
        seed_instinct(&logger, "s1", "inst-stale", 0.5, &old.to_rfc3339());
        seed_instinct(
            &logger,
            "s1",
            "inst-good",
            0.5,
            &chrono::Local::now().to_rfc3339(),
        );
        // T4：promoted 晋升产物豁免清理（即便 confidence 低于阈值也不删）
        seed_promoted(
            &logger,
            "s1",
            "inst-promoted",
            0.1,
            &chrono::Local::now().to_rfc3339(),
        );
        let pruned = prune_instincts(&logger, "s1").unwrap();
        assert_eq!(pruned, 2);
        let remaining = list_instincts(&logger, "s1").unwrap();
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().any(|i| i.id == "inst-good"));
        assert!(
            remaining.iter().any(|i| i.id == "inst-promoted"),
            "promoted instinct 不应被 prune"
        );
    }

    #[test]
    fn test_promotion_candidates_cross_story() {
        let (logger, _tmp) = logger();
        // 同 trigger 在 s1/s2 各一条（s1 confidence 0.85，s2 0.8）
        seed_instinct(
            &logger,
            "s1",
            "inst-x",
            0.85,
            &chrono::Local::now().to_rfc3339(),
        );
        seed_instinct(
            &logger,
            "s2",
            "inst-y",
            0.8,
            &chrono::Local::now().to_rfc3339(),
        );
        // s3 只有一条同 trigger（不重复出现 → 不算跨 story）
        seed_instinct(
            &logger,
            "s3",
            "inst-z",
            0.3,
            &chrono::Local::now().to_rfc3339(),
        );
        let candidates = promotion_candidates(&logger, "s1").unwrap();
        assert!(
            candidates.iter().any(|i| i.id == "inst-x"),
            "s1 的高置信跨 story instinct 应为候选"
        );
        // s3 的 inst-z confidence 0.3 不达标
        assert!(!candidates.iter().any(|i| i.id == "inst-z"));
    }

    #[test]
    fn test_reject_promotion() {
        let (logger, _tmp) = logger();
        seed_instinct(
            &logger,
            "s1",
            "inst-r",
            0.85,
            &chrono::Local::now().to_rfc3339(),
        );
        let rejected = reject_promotion(&logger, "s1", "inst-r").unwrap();
        assert_eq!(rejected.status, "rejected");
        assert!((rejected.confidence - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_materialize_as_skill_files() {
        let (logger, tmp) = logger();
        seed_instinct(
            &logger,
            "s1",
            "inst-m",
            0.85,
            &chrono::Local::now().to_rfc3339(),
        );
        let skills_dir = tmp.path().join("skills");
        let skill_dir = materialize_as_skill(&logger, "s1", "inst-m", &skills_dir).unwrap();
        let manifest = std::fs::read_to_string(skill_dir.join("skill.yaml")).unwrap();
        assert!(manifest.contains("id: learned.inst-m"));
        assert!(
            manifest.contains("evolved_from: \"inst-m\"")
                || manifest.contains("evolved_from: 'inst-m'")
                || manifest.contains("evolved_from: inst-m")
        );
        let prompt = std::fs::read_to_string(skill_dir.join("main.prompt")).unwrap();
        assert!(prompt.contains("---"));
        assert!(prompt.contains("{{instruction}}"));
        // skill.yaml 可被 serde_yaml 解析为 SkillManifest（loader 兼容）
        let parsed: crate::skills::SkillManifest = serde_yaml::from_str(&manifest).unwrap();
        assert_eq!(parsed.id, "learned.inst-m");
        assert_eq!(parsed.entry_point, "main.prompt");
    }

    #[test]
    fn test_confirm_promotion_end_to_end() {
        let (logger, tmp) = logger();
        seed_instinct(
            &logger,
            "s1",
            "inst-c",
            0.85,
            &chrono::Local::now().to_rfc3339(),
        );
        seed_instinct(
            &logger,
            "s2",
            "inst-c2",
            0.8,
            &chrono::Local::now().to_rfc3339(),
        ); // 同 trigger
        let skills_dir = tmp.path().join("skills");
        let outcome = confirm_promotion(&logger, "s1", "inst-c", &skills_dir).unwrap();
        assert_eq!(outcome.skill_id, "learned.inst-c");
        assert_eq!(outcome.instinct.status, "promoted");
        assert_eq!(outcome.instinct.scope, "global");
        // 技能目录已生成
        assert!(skills_dir.join("learned.inst-c/skill.yaml").exists());
        // 失败路径：confidence 不足
        seed_instinct(
            &logger,
            "s1",
            "inst-low",
            0.3,
            &chrono::Local::now().to_rfc3339(),
        );
        assert!(confirm_promotion(&logger, "s1", "inst-low", &skills_dir).is_err());
    }
}
