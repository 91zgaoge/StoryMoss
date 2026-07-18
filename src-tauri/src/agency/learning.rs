//! 持续学习·观察层（ECC observe 模式）：四类观察点 → .storymoss/learning/observations.jsonl。
//! 双轨制的文件轨：JSONL 追加写、10MB 轮转、防自观察、payload 截断脱敏。

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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

    fn log_inner(&self, story_id: &str, kind: &str, actor: &str, payload: serde_json::Value) -> Result<(), String> {
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
        let total = self.observations_path(story_id)
            .exists()
            .then(|| {
                std::fs::read_to_string(self.observations_path(story_id))
                    .map(|c| c.lines().count())
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let analyzed = self.analyzed_through(story_id);
        total.saturating_sub(analyzed)
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

    pub fn mark_analyzed(&self, story_id: &str) -> Result<(), crate::error::AppError> {
        let total = self.observations_path(story_id)
            .exists()
            .then(|| {
                std::fs::read_to_string(self.observations_path(story_id))
                    .map(|c| c.lines().count())
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        let path = self.state_path(story_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(crate::error::AppError::from)?;
        }
        let state = serde_json::json!({
            "analyzed_through": total,
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
                            serde_json::Value::String(format!("{}…(截断)", s.chars().take(PAYLOAD_MAX_CHARS).collect::<String>()))
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
const ANALYZE_MIN_NEW: usize = 2;
/// analyzer 自身的路由/观察标签（双约束，test_analyzer_label_dual_constraint 锁死）：
/// strip "agency_" → "editor_observer" → starts_with("editor") 命中 Background 档；
/// contains("observer") → should_record 过滤其 llm_call 埋点（防自观察）。
pub const ANALYZER_LABEL: &str = "agency_editor_observer";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Instinct {
    pub id: String,
    pub trigger: String,
    pub action: String,
    pub confidence: f64,
    pub evidence_count: u32,
    pub scope: String,   // story | global
    pub status: String,  // pending | candidate | promoted | rejected
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

pub fn list_instincts(logger: &ObservationLogger, story_id: &str) -> Result<Vec<Instinct>, crate::error::AppError> {
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
        id: fm.id, trigger: fm.trigger, action: fm.action,
        confidence: fm.confidence, evidence_count: fm.evidence_count,
        scope: fm.scope, status: fm.status,
        created_at: fm.created_at, updated_at: fm.updated_at,
        evolved_from: fm.evolved_from,
    })
}

fn render_instinct(inst: &Instinct, body: &str) -> String {
    // evolved_from 用 JSON 渲染（YAML flow 序列兼容 JSON）：块式 YAML 嵌进单行
    // 会破坏 frontmatter 行结构导致 parse_instinct 失败。
    let evolved = serde_json::to_string(&inst.evolved_from).unwrap_or_else(|_| "[]".into());
    format!(
        "---\nid: {}\ntrigger: {:?}\naction: {:?}\nconfidence: {}\nevidence_count: {}\nscope: {}\nstatus: {}\ncreated_at: {:?}\nupdated_at: {:?}\nevolved_from: {}\n---\n\n{}\n",
        inst.id, inst.trigger, inst.action, inst.confidence, inst.evidence_count,
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
    let new_count = logger.count_unanalyzed(story_id);
    if new_count < ANALYZE_MIN_NEW {
        return Ok(AnalyzeOutcome { new_instincts: 0, updated_instincts: 0, analyzed: 0 });
    }
    let observations = logger.recent(story_id, 50);
    let existing = list_instincts(logger, story_id).unwrap_or_default();
    let digest: String = observations
        .iter()
        .map(|o| format!("- [{}] {} by {}: {}", o.ts.get(..10).unwrap_or(&o.ts), o.kind, o.actor, o.payload))
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
    let raw = llm.complete(
        "你是创作模式分析器，只输出 YAML。",
        &prompt,
        crate::router::TaskType::Analysis,
        1500,
    ).await?;
    let proposals = parse_analyzer_yaml(&raw);
    let mut new_instincts = 0usize;
    let mut updated_instincts = 0usize;
    for proposal in proposals {
        let dir = logger.instincts_path(story_id);
        std::fs::create_dir_all(&dir).map_err(crate::error::AppError::from)?;
        if let Some(mut hit) = existing.iter().find(|e| e.trigger == proposal.trigger).cloned() {
            hit.evidence_count += new_count as u32;
            hit.confidence = confidence_for_evidence(hit.evidence_count);
            hit.updated_at = chrono::Local::now().to_rfc3339();
            std::fs::write(dir.join(format!("{}.md", hit.id)), render_instinct(&hit, "（更新：证据累积）"))
                .map_err(crate::error::AppError::from)?;
            updated_instincts += 1;
        } else {
            let now = chrono::Local::now().to_rfc3339();
            // 新建 instinct 从 1 条证据起（本轮归纳出的模式本身即第一条证据）；
            // 后续同 trigger 轮次按 new_count 累积。
            let inst = Instinct {
                id: format!("inst-{}-{:06x}",
                    now.get(..10).unwrap_or(&now).replace('-', ""),
                    crc32_simple(&proposal.trigger)),
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
            let body = format!("## 模式描述\n{}\n\n## 证据摘要\n（来自最近 {} 条观察）", proposal.action, new_count);
            std::fs::write(dir.join(format!("{}.md", inst.id)), render_instinct(&inst, &body))
                .map_err(crate::error::AppError::from)?;
            new_instincts += 1;
        }
    }
    logger.mark_analyzed(story_id)?;
    Ok(AnalyzeOutcome { new_instincts, updated_instincts, analyzed: new_count })
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
        .map(|p| AnalyzerProposal { trigger: p.trigger, action: p.action, evolved_from: p.evolved_from })
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

fn read_instinct_file(logger: &ObservationLogger, story_id: &str, id: &str) -> Result<(Instinct, std::path::PathBuf), crate::error::AppError> {
    let path = logger.instincts_path(story_id).join(format!("{}.md", id));
    let text = std::fs::read_to_string(&path)
        .map_err(|e| crate::error::AppError::validation_failed(format!("instinct 不存在: {} ({})", id, e), None::<String>))?;
    let inst = parse_instinct(&text)
        .ok_or_else(|| crate::error::AppError::validation_failed(format!("instinct 解析失败: {}", id), None::<String>))?;
    Ok((inst, path))
}

fn write_instinct_file(path: &std::path::Path, inst: &Instinct) -> Result<(), crate::error::AppError> {
    std::fs::write(path, render_instinct(inst, "（反馈/衰减更新）")).map_err(crate::error::AppError::from)
}

pub fn apply_feedback(logger: &ObservationLogger, story_id: &str, instinct_id: &str, accepted: bool) -> Result<Instinct, crate::error::AppError> {
    let (mut inst, path) = read_instinct_file(logger, story_id, instinct_id)?;
    let delta = if accepted { FEEDBACK_ACCEPT } else { FEEDBACK_REJECT };
    inst.confidence = (inst.confidence + delta).clamp(0.0, 1.0);
    inst.updated_at = chrono::Local::now().to_rfc3339();
    write_instinct_file(&path, &inst)?;
    Ok(inst)
}

pub fn apply_weekly_decay(logger: &ObservationLogger, story_id: &str) -> Result<usize, crate::error::AppError> {
    let instincts = list_instincts(logger, story_id)?;
    let now = chrono::Local::now();
    let mut decayed = 0usize;
    for mut inst in instincts {
        let updated = chrono::DateTime::parse_from_rfc3339(&inst.updated_at)
            .map(|d| d.with_timezone(&chrono::Local))
            .unwrap_or(now);
        let weeks = (now - updated).num_days() / 7;
        if weeks >= 1 {
            inst.confidence = (inst.confidence + weeks as f64 * WEEKLY_DECAY).clamp(0.0, 1.0);
            inst.updated_at = now.to_rfc3339();
            let path = logger.instincts_path(story_id).join(format!("{}.md", inst.id));
            write_instinct_file(&path, &inst)?;
            decayed += 1;
        }
    }
    Ok(decayed)
}

pub fn prune_instincts(logger: &ObservationLogger, story_id: &str) -> Result<usize, crate::error::AppError> {
    let instincts = list_instincts(logger, story_id)?;
    let now = chrono::Local::now();
    let mut pruned = 0usize;
    for inst in instincts {
        let updated = chrono::DateTime::parse_from_rfc3339(&inst.updated_at)
            .map(|d| d.with_timezone(&chrono::Local))
            .unwrap_or(now);
        let stale_days = (now - updated).num_days();
        let should_prune = inst.confidence < PRUNE_CONFIDENCE
            || (inst.status == "pending" && stale_days >= PRUNE_TTL_DAYS);
        if should_prune {
            let path = logger.instincts_path(story_id).join(format!("{}.md", inst.id));
            if std::fs::remove_file(&path).is_ok() {
                pruned += 1;
            }
        }
    }
    Ok(pruned)
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
        logger.log("s1", "gate", "editor_auditor", serde_json::json!({"outcome": "pass", "weighted": 0.82}));
        logger.log("s1", "llm_call", "lead_writer", serde_json::json!({"tokens": 100}));
        logger.log("s2", "gate", "editor_auditor", serde_json::json!({"outcome": "revise"}));
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
        logger.mark_analyzed("s1").unwrap();
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
        assert!(!ObservationLogger::should_record("agency_observer_analyzer"));
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
        assert!(after.len() <= 10_000 + 1200, "轮转后应接近阈值: {}", after.len());
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
        logger.log("s1", "user_edit", "human", serde_json::json!({"note": long}));
        let recent = logger.recent("s1", 1);
        let note = recent[0].payload["note"].as_str().unwrap();
        assert!(note.chars().count() <= 520, "payload 应截断: {}", note.len());
    }

    struct MockAnalyzerLlm {
        response: String,
    }

    #[async_trait::async_trait]
    impl crate::agency::tool_loop::LoopLlm for MockAnalyzerLlm {
        async fn complete(&self, _s: &str, _u: &str, _t: crate::router::TaskType, _m: i32) -> Result<String, crate::error::AppError> {
            Ok(self.response.clone())
        }
    }

    fn analyzer_mock() -> std::sync::Arc<MockAnalyzerLlm> {
        std::sync::Arc::new(MockAnalyzerLlm {
            response: r#"```yaml
- trigger: "当编辑审计连续两轮判定 revise"
  action: "修订前先复读资产区角色卡与大纲"
  evolved_from: ["gate", "revision"]
```"#.to_string(),
        })
    }

    #[tokio::test]
    async fn test_analyze_creates_instinct_files() {
        let (logger, _tmp) = logger();
        for i in 0..3 {
            logger.log("s1", "gate", "editor_auditor", serde_json::json!({"outcome": "revise", "i": i}));
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
            logger.log("s1", "gate", "editor_auditor", serde_json::json!({"outcome": "revise", "i": i}));
        }
        analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
        // 同 trigger 再来一轮观察 + 分析 → 同 trigger instinct 的 evidence_count 递增、confidence 升档
        for _ in 0..4 {
            logger.log("s1", "revision", "editor_auditor", serde_json::json!({"chapter": 1}));
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
        logger.log("s1", "gate", "editor_auditor", serde_json::json!({"outcome": "pass"}));
        let outcome = analyze_story(analyzer_mock(), &logger, "s1").await.unwrap();
        assert_eq!(outcome.analyzed, 0);
        assert_eq!(outcome.new_instincts, 0);
        // 未达到最小样本（<2 条新观察）不调用 LLM、不推进游标
        assert_eq!(logger.count_unanalyzed("s1"), 1);
    }

    fn seed_instinct(logger: &ObservationLogger, story_id: &str, id: &str, confidence: f64, updated_at: &str) {
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
        std::fs::write(dir.join(format!("{}.md", id)), render_instinct(&inst, "body")).unwrap();
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
        let decayed = apply_weekly_decay(&logger, "s1").unwrap();
        assert_eq!(decayed, 1); // 只有 14 天前的那条衰减（每满 7 天 -0.02，14 天 -0.04）
        let instincts = list_instincts(&logger, "s1").unwrap();
        let old_inst = instincts.iter().find(|i| i.id == "inst-old").unwrap();
        assert!((old_inst.confidence - 0.46).abs() < 0.001);
        let fresh_inst = instincts.iter().find(|i| i.id == "inst-fresh").unwrap();
        assert!((fresh_inst.confidence - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_prune() {
        let (logger, _tmp) = logger();
        seed_instinct(&logger, "s1", "inst-weak", 0.1, &chrono::Local::now().to_rfc3339());
        let old = chrono::Local::now() - chrono::Duration::days(100);
        seed_instinct(&logger, "s1", "inst-stale", 0.5, &old.to_rfc3339());
        seed_instinct(&logger, "s1", "inst-good", 0.5, &chrono::Local::now().to_rfc3339());
        let pruned = prune_instincts(&logger, "s1").unwrap();
        assert_eq!(pruned, 2);
        let remaining = list_instincts(&logger, "s1").unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "inst-good");
    }
}
