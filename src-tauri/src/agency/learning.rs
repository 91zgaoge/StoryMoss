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
    app_dir: PathBuf,
}

impl ObservationLogger {
    pub fn new(app_dir: PathBuf) -> Self {
        Self { app_dir }
    }

    pub fn should_record(context_label: &str) -> bool {
        !context_label.contains("agency_observer")
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
    #[allow(dead_code)]
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
        let keep_from = content.len().saturating_sub(max_bytes as usize);
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
        assert!(ObservationLogger::should_record("agency_writer"));
        assert!(!ObservationLogger::should_record("agency_observer"));
        assert!(!ObservationLogger::should_record("agency_observer_analyzer"));
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
    fn test_payload_truncation() {
        let (logger, _tmp) = logger();
        let long = "长".repeat(2000);
        logger.log("s1", "user_edit", "human", serde_json::json!({"note": long}));
        let recent = logger.recent("s1", 1);
        let note = recent[0].payload["note"].as_str().unwrap();
        assert!(note.chars().count() <= 520, "payload 应截断: {}", note.len());
    }
}
