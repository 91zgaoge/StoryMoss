//! Capability Evolution - 能力进化反馈环
//!
//! Records execution results and uses LLM to improve capability descriptions over time.

use serde::{Deserialize, Serialize};
use crate::llm::LlmService;
use std::collections::HashMap;
use tauri::Manager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
        Self { storage_path, cache }
    }

    pub fn from_app_handle(app_handle: &tauri::AppHandle) -> Self {
        let app_data_dir = app_handle.path().app_data_dir()
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
        let filtered: Vec<_> = records.iter()
            .filter(|r| capability_id.map(|id| r.capability_id == id).unwrap_or(true))
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
            record.capability_id, record.user_input, record.success
        );
        self.store.append(record);

        // v5.6.4 修复: 每积累 5 条执行记录，自动触发一次能力进化
        let total_records = self.store.len();
        if total_records > 0 && total_records % 5 == 0 {
            let engine = self.clone();
            tauri::async_runtime::spawn(async move {
                match engine.evolve_capability_descriptions().await {
                    Ok(improvements) if !improvements.is_empty() => {
                        log::info!("[CapabilityEvolution] Periodic evolution triggered ({} records), {} descriptions improved", total_records, improvements.len());
                    }
                    Ok(_) => {}
                    Err(e) => {
                        log::warn!("[CapabilityEvolution] Periodic evolution failed: {}", e);
                    }
                }
            });
        }

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
                log::warn!("[CapabilityEvolution] EVOLVED_DESCRIPTIONS_PATH not set, skipping load");
                return HashMap::new();
            }
        };
        if !path.exists() {
            return HashMap::new();
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(e) => {
                log::warn!("[CapabilityEvolution] Failed to load evolved descriptions: {}", e);
                HashMap::new()
            }
        }
    }

    fn save_evolved_descriptions(&self, descriptions: &HashMap<String, String>) -> Result<(), String> {
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
        std::fs::write(&path, json)
            .map_err(|e| format!("Write failed: {}", e))?;
        Ok(())
    }

    /// Analyze execution history and suggest improvements to capability descriptions
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
                    r.success, r.execution_time_ms, feedback,
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
                capability_id, total, success_rate * 100.0, record_summary
            );

            match self.llm_service.generate(prompt, Some(256), Some(0.3)).await {
                Ok(response) => {
                    let improved = response.content.trim().to_string();
                    if !improved.is_empty() && improved.len() > 20 {
                        improvements.push((capability_id, improved));
                    }
                }
                Err(e) => {
                    log::warn!("[CapabilityEvolution] LLM analysis failed for {}: {}", capability_id, e);
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
                log::warn!("[CapabilityEvolution] Failed to save evolved descriptions: {}", e);
            } else {
                log::info!("[CapabilityEvolution] Saved {} evolved descriptions", improvements.len());
            }
        }

        log::info!("[CapabilityEvolution] Generated {} improvement suggestions", improvements.len());
        Ok(improvements)
    }

    /// Get execution statistics for all capabilities
    pub fn get_statistics(&self) -> HashMap<String, (usize, usize)> {
        self.store.get_statistics()
    }
}
