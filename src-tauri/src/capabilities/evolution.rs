#![allow(dead_code)]
//! Capability Evolution - 能力进化反馈环
//!
//! Records execution results and uses LLM to improve capability descriptions
//! over time.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tokio::time::timeout;

use crate::{llm::LlmService, router::TaskType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub capability_id: String,
    pub user_input: String,
    pub success: bool,
    pub user_feedback: Option<String>, // accept/reject/modify
    pub execution_time_ms: u64,
    pub timestamp: String, // ISO 8601
}

/// 执行记录存储（基于 JSON 文件，无需数据库迁移）
#[derive(Clone)]
pub struct ExecutionRecordStore {
    storage_path: PathBuf,
    cache: Arc<Mutex<Vec<ExecutionRecord>>>,
}

impl ExecutionRecordStore {
    pub fn new(app_data_dir: &PathBuf) -> Self {
        let storage_path = app_data_dir.join("capability_execution_records.json");
        let cache = Arc::new(Mutex::new(Self::load_records(&storage_path)));
        Self {
            storage_path,
            cache,
        }
    }

    pub fn from_app_handle(app_handle: &tauri::AppHandle) -> Self {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default());
        Self::new(&app_data_dir)
    }

    fn load_records(path: &PathBuf) -> Vec<ExecutionRecord> {
        if !path.exists() {
            return Vec::new();
        }
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(e) => {
                log::warn!("[ExecutionRecordStore] Failed to load records: {}", e);
                Vec::new()
            }
        }
    }

    fn save_records(&self, records: &[ExecutionRecord]) {
        if let Ok(json) = serde_json::to_string_pretty(records) {
            let _ = std::fs::write(&self.storage_path, json);
        }
    }

    pub fn append(&self, record: ExecutionRecord) {
        let mut records = self.cache.lock().unwrap();
        records.push(record);
        // Keep only last 500 records to prevent file bloat
        if records.len() > 500 {
            let split_idx = records.len() - 500;
            let trimmed: Vec<_> = records.drain(split_idx..).collect();
            *records = trimmed;
        }
        self.save_records(&records);
    }

    pub fn len(&self) -> usize {
        self.cache.lock().unwrap().len()
    }

    pub fn get_records(&self, capability_id: Option<&str>, limit: usize) -> Vec<ExecutionRecord> {
        let records = self.cache.lock().unwrap();
        let filtered: Vec<_> = records
            .iter()
            .filter(|r| {
                capability_id
                    .map(|id| r.capability_id == id)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        filtered.into_iter().rev().take(limit).collect()
    }

    pub fn get_statistics(&self) -> HashMap<String, (usize, usize)> {
        let records = self.cache.lock().unwrap();
        let mut stats: HashMap<String, (usize, usize)> = HashMap::new();
        for r in records.iter() {
            let entry = stats.entry(r.capability_id.clone()).or_insert((0, 0));
            entry.0 += 1; // total
            if r.success {
                entry.1 += 1; // success
            }
        }
        stats
    }
}

#[derive(Clone)]
pub struct CapabilityEvolutionEngine {
    llm_service: LlmService,
    store: ExecutionRecordStore,
}

impl CapabilityEvolutionEngine {
    pub fn new(llm_service: LlmService, app_handle: &tauri::AppHandle) -> Self {
        let store = ExecutionRecordStore::from_app_handle(app_handle);
        Self { llm_service, store }
    }

    pub fn new_with_path(llm_service: LlmService, app_data_dir: &PathBuf) -> Self {
        let store = ExecutionRecordStore::new(app_data_dir);
        Self { llm_service, store }
    }

    /// Record an execution result
    pub fn record_execution(&self, record: ExecutionRecord) -> Result<(), String> {
        log::info!(
            "[CapabilityEvolution] {} executed for '{}': success={}",
            record.capability_id,
            record.user_input,
            record.success
        );
        self.store.append(record);
        // v0.11.5-hotfix: 禁用每 5 条记录自动触发能力进化，避免在用户无感知时
        // 发起后台 LLM 调用导致界面长时间卡顿。能力进化改为仅手动触发。
        Ok(())
    }

    /// 加载已进化的能力描述（覆盖默认描述）
    /// P1-12 修复: 统一使用全局 EVOLVED_DESCRIPTIONS_PATH，避免路径不一致
    pub fn load_evolved_descriptions(&self) -> HashMap<String, String> {
        let path = match crate::capabilities::EVOLVED_DESCRIPTIONS_PATH.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };
        let path = match path {
            Some(p) => p,
            None => {
                log::warn!(
                    "[CapabilityEvolution] EVOLVED_DESCRIPTIONS_PATH not set, skipping load"
                );
                return HashMap::new();
            }
        };
        if !path.exists() {
            return HashMap::new();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(e) => {
                log::warn!(
                    "[CapabilityEvolution] Failed to load evolved descriptions: {}",
                    e
                );
                HashMap::new()
            }
        }
    }

    fn save_evolved_descriptions(
        &self,
        descriptions: &HashMap<String, String>,
    ) -> Result<(), String> {
        let path = match crate::capabilities::EVOLVED_DESCRIPTIONS_PATH.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => None,
        };
        let path = match path {
            Some(p) => p,
            None => return Err("EVOLVED_DESCRIPTIONS_PATH not set".to_string()),
        };
        let json = serde_json::to_string_pretty(descriptions)
            .map_err(|e| format!("Serialize failed: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("Write failed: {}", e))?;
        Ok(())
    }

    /// Analyze execution history and suggest improvements to capability
    /// descriptions
    pub async fn evolve_capability_descriptions(&self) -> Result<Vec<(String, String)>, String> {
        let stats = self.store.get_statistics();
        if stats.is_empty() {
            return Ok(vec![]);
        }

        let mut improvements = Vec::new();

        for (capability_id, (total, success)) in stats {
            if total < 5 {
                // Not enough data to analyze
                continue;
            }

            let success_rate = success as f64 / total as f64;
            let records = self.store.get_records(Some(&capability_id), 20);

            // Build analysis prompt
            let mut record_summary = String::new();
            for r in &records {
                let feedback = r.user_feedback.as_deref().unwrap_or("none");
                record_summary.push_str(&format!(
                    "- success={}, time={}ms, feedback={}, input={}\n",
                    r.success,
                    r.execution_time_ms,
                    feedback,
                    &r.user_input.chars().take(80).collect::<String>()
                ));
            }

            let prompt = format!(
                r#"You are an AI system optimizer. Analyze the execution history of a capability and suggest an improved "when_to_use" description.

Capability ID: {}
Total executions: {}
Success rate: {:.1}%

Recent execution history:
{}

Based on this data, what is the single most important improvement to the "when_to_use" description?
Respond with ONLY the improved description text (1-2 sentences). Do not include any explanation or formatting."#,
                capability_id,
                total,
                success_rate * 100.0,
                record_summary
            );

            // v0.11.5-hotfix: 单次能力进化分析最多等待
            // 60s，避免无响应模型拖垮整个流程
            let evolution_timeout = std::time::Duration::from_secs(60);
            let evolution_result = timeout(
                evolution_timeout,
                self.llm_service.generate_for_task(
                    TaskType::Analysis,
                    prompt,
                    Some(256),
                    Some(0.3),
                    Some("capability_evolution"),
                ),
            )
            .await;

            match evolution_result {
                Ok(Ok(response)) => {
                    // v0.13.1: 清洗 LLM 输出 —— 部分模型（如
                    // qwen/deepseek）会在正文前
                    // 输出 <think>...</think>
                    // 思考链。若不剥离，这段混乱文本会被当作
                    // when_to_use 注入 PlanGenerator 的
                    // prompt，污染计划生成、拖慢甚至
                    // 卡死智能创作的「准备上下文」阶段。
                    let improved = sanitize_evolved_description(&response.content);
                    if let Some(clean) = improved {
                        improvements.push((capability_id, clean));
                    }
                }
                Ok(Err(e)) => {
                    log::warn!(
                        "[CapabilityEvolution] LLM analysis failed for {}: {}",
                        capability_id,
                        e
                    );
                }
                Err(_) => {
                    log::warn!(
                        "[CapabilityEvolution] LLM analysis timed out after 60s for {}",
                        capability_id
                    );
                }
            }
        }

        // 自动保存进化后的描述
        if !improvements.is_empty() {
            let mut evolved = self.load_evolved_descriptions();
            for (id, desc) in &improvements {
                evolved.insert(id.clone(), desc.clone());
            }
            if let Err(e) = self.save_evolved_descriptions(&evolved) {
                log::warn!(
                    "[CapabilityEvolution] Failed to save evolved descriptions: {}",
                    e
                );
            } else {
                log::info!(
                    "[CapabilityEvolution] Saved {} evolved descriptions",
                    improvements.len()
                );
            }
        }

        log::info!(
            "[CapabilityEvolution] Generated {} improvement suggestions",
            improvements.len()
        );
        Ok(improvements)
    }

    /// Get execution statistics for all capabilities
    pub fn get_statistics(&self) -> HashMap<String, (usize, usize)> {
        self.store.get_statistics()
    }
}

/// 清洗 LLM 生成的能力描述，确保只有干净、简短的正文被保存为 when_to_use。
///
/// 处理：
/// 1. 剥离 `<think>...</think>` 思考链（部分推理模型默认输出）。
/// 2. 剥离 markdown 代码块包裹。
/// 3. 长度上限 300 字符（when_to_use 应是 1-2 句话）。
/// 4. 拒绝仍包含 `<think>` 残片或过短（<20 字符）的结果。
///
/// 返回 None 表示输出不可用，调用方应跳过此次进化。
pub fn sanitize_evolved_description(raw: &str) -> Option<String> {
    const MAX_LEN: usize = 300;
    const MIN_LEN: usize = 20;

    let mut text = raw.trim().to_string();

    // 反复剥离 <think>...</think>（可能有多段或未闭合）
    loop {
        if let Some(start) = text.find("<think>") {
            if let Some(end_rel) = text[start..].find("</think>") {
                let end = start + end_rel + "</think>".len();
                text.replace_range(start..end, "");
                continue;
            } else {
                // 未闭合的 <think>：丢弃从 <think> 到结尾的全部内容
                // （模型把正文写在了思考块里且没闭合）
                text.truncate(start);
            }
        }
        break;
    }

    // 剥离 markdown 代码块
    text = text
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    // 折叠多余空白
    let collapsed: String = text.split_whitespace().collect::<Vec<_>>().join(" ");

    let collapsed = collapsed.trim().to_string();

    // 长度与残留标签校验
    if collapsed.len() < MIN_LEN {
        log::warn!(
            "[CapabilityEvolution] Sanitized description too short ({} chars), skipping",
            collapsed.len()
        );
        return None;
    }
    if collapsed.contains("<think>") || collapsed.contains("</think>") {
        log::warn!(
            "[CapabilityEvolution] Sanitized description still contains <think> tags, skipping"
        );
        return None;
    }

    let result = if collapsed.chars().count() > MAX_LEN {
        // 按字符截断到上限，避免截断多字节字符
        let truncated: String = collapsed.chars().take(MAX_LEN).collect();
        truncated
    } else {
        collapsed
    };

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::sanitize_evolved_description;

    #[test]
    fn strips_think_block_and_keeps_body() {
        let raw = "<think>\nLet me analyze...\nThis is reasoning.\n</think>\nUse when writing new story content or continuing a scene.";
        let cleaned = sanitize_evolved_description(raw).unwrap();
        assert!(!cleaned.contains("<think>"));
        assert!(!cleaned.contains("reasoning"));
        assert!(cleaned.contains("writing new story content"));
    }

    #[test]
    fn rejects_unclosed_think_block() {
        // 未闭合 <think>：正文被当作思考链一并丢弃 → 结果过短 → None
        let raw = "<think>\nLet me analyze the execution history of writer";
        assert!(sanitize_evolved_description(raw).is_none());
    }

    #[test]
    fn truncates_overlong_description() {
        let long = "Use when writing. ".repeat(50);
        let cleaned = sanitize_evolved_description(&long).unwrap();
        assert!(cleaned.chars().count() <= 300);
    }

    #[test]
    fn rejects_too_short() {
        assert!(sanitize_evolved_description("ok").is_none());
    }

    #[test]
    fn strips_code_fences() {
        let raw = "```\nUse when you need to write new story content for the novel.\n```";
        let cleaned = sanitize_evolved_description(raw).unwrap();
        assert!(!cleaned.contains("```"));
        assert!(cleaned.contains("write new story content"));
    }
}
