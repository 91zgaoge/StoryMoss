//! Harness 生成链路可观测性
//!
//! 为每次生成请求建立 `trace_id`，记录 gateway 路由、候选模型探测、LLM 调用、
//! 计划步骤等全链路事件，供前端 Tracing 面板与日志分析使用。

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::AppError;

const MAX_MEMORY_TRACES: usize = 200;

/// 单个 Trace 步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub name: String,
    pub phase: String,
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl TraceStep {
    pub fn new(name: impl Into<String>, phase: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            phase: phase.into(),
            start_ms: Self::now_monotonic_ms(),
            end_ms: None,
            duration_ms: None,
            model_id: None,
            provider: None,
            input_tokens: None,
            output_tokens: None,
            status: Some("running".to_string()),
            error: None,
            details: None,
        }
    }

    fn now_monotonic_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    pub fn finish(mut self, status: &str) -> Self {
        let end = Self::now_monotonic_ms();
        self.end_ms = Some(end);
        self.duration_ms = Some(end.saturating_sub(self.start_ms));
        self.status = Some(status.to_string());
        self
    }

    pub fn with_model(mut self, model_id: impl Into<String>, provider: impl Into<String>) -> Self {
        self.model_id = Some(model_id.into());
        self.provider = Some(provider.into());
        self
    }

    pub fn with_tokens(mut self, input_tokens: u32, output_tokens: u32) -> Self {
        self.input_tokens = Some(input_tokens);
        self.output_tokens = Some(output_tokens);
        self
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self.status = Some("failed".to_string());
        self
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// 生成链路 Trace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationTrace {
    pub trace_id: String,
    pub request_id: Option<String>,
    pub story_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_input: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    pub steps: Vec<TraceStep>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl GenerationTrace {
    pub fn new(trace_id: impl Into<String>) -> Self {
        Self {
            trace_id: trace_id.into(),
            request_id: None,
            story_id: None,
            user_input: None,
            created_at: Utc::now().to_rfc3339(),
            finished_at: None,
            steps: Vec::new(),
            status: "running".to_string(),
            error_message: None,
        }
    }

    pub fn add_step(
        &mut self,
        name: impl Into<String>,
        phase: impl Into<String>,
    ) -> &mut TraceStep {
        let step = TraceStep::new(name, phase);
        self.steps.push(step);
        self.steps.last_mut().unwrap()
    }

    pub fn finish(&mut self, status: &str, error: Option<String>) {
        self.finished_at = Some(Utc::now().to_rfc3339());
        self.status = status.to_string();
        self.error_message = error;
    }

    /// 关闭所有仍在 running 的步骤，避免 UI 悬挂
    pub fn close_open_steps(&mut self, status: &str) {
        for step in self.steps.iter_mut() {
            if step.status.as_deref() == Some("running") || step.end_ms.is_none() {
                let end = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                step.end_ms = Some(end);
                step.duration_ms = Some(end.saturating_sub(step.start_ms));
                step.status = Some(status.to_string());
            }
        }
    }
}

#[derive(Debug, Clone)]
struct TraceStoreInner {
    traces: VecDeque<GenerationTrace>,
    request_id_to_trace: HashMap<String, String>,
}

/// 全局 Trace 存储
#[derive(Clone, Debug)]
pub struct TraceStore {
    inner: Arc<Mutex<TraceStoreInner>>,
    log_dir: PathBuf,
}

impl TraceStore {
    pub fn new(app: &AppHandle) -> Result<Self, AppError> {
        let app_dir = app.path().app_data_dir().map_err(|e| AppError::Internal {
            message: format!("无法获取应用数据目录: {}", e),
        })?;
        let log_dir = app_dir.join("logs").join("traces");
        std::fs::create_dir_all(&log_dir).map_err(|e| AppError::Internal {
            message: format!("创建 trace 日志目录失败: {}", e),
        })?;
        Ok(Self {
            inner: Arc::new(Mutex::new(TraceStoreInner {
                traces: VecDeque::with_capacity(MAX_MEMORY_TRACES),
                request_id_to_trace: HashMap::new(),
            })),
            log_dir,
        })
    }

    fn trace_file_path(&self, trace_id: &str) -> PathBuf {
        self.log_dir.join(format!("{}.json", trace_id))
    }

    fn persist(&self, trace: &GenerationTrace) -> Result<(), AppError> {
        let path = self.trace_file_path(&trace.trace_id);
        let json = serde_json::to_string_pretty(trace).map_err(|e| AppError::Internal {
            message: format!("序列化 trace 失败: {}", e),
        })?;
        std::fs::write(&path, json).map_err(|e| AppError::Internal {
            message: format!("写入 trace 文件失败: {}", e),
        })?;
        Ok(())
    }

    /// 开启一个新的 trace，返回 trace_id
    pub fn start_trace(
        &self,
        trace_id: impl Into<String>,
        request_id: Option<String>,
        story_id: Option<String>,
        user_input: Option<String>,
    ) -> Result<String, AppError> {
        let trace_id = trace_id.into();
        let mut trace = GenerationTrace::new(&trace_id);
        trace.request_id = request_id.clone();
        trace.story_id = story_id;
        trace.user_input = user_input;
        trace.add_step("generation.start", "orchestrator");
        self.persist(&trace)?;

        let mut inner = self.inner.lock().map_err(|e| AppError::Internal {
            message: format!("TraceStore 锁中毒: {}", e),
        })?;
        if let Some(req_id) = request_id {
            inner.request_id_to_trace.insert(req_id, trace_id.clone());
        }
        if inner.traces.len() >= MAX_MEMORY_TRACES {
            inner.traces.pop_front();
        }
        inner.traces.push_back(trace);
        Ok(trace_id)
    }

    /// 将 request_id 关联到 trace_id，供 LLM 进度事件反向查找
    pub fn associate_request_id(
        &self,
        trace_id: impl Into<String>,
        request_id: impl Into<String>,
    ) -> Result<(), AppError> {
        let trace_id = trace_id.into();
        let request_id = request_id.into();
        let mut inner = self.inner.lock().map_err(|e| AppError::Internal {
            message: format!("TraceStore 锁中毒: {}", e),
        })?;
        inner.request_id_to_trace.insert(request_id, trace_id);
        Ok(())
    }

    /// 根据 request_id 查找 trace_id
    pub fn trace_id_for_request(&self, request_id: &str) -> Option<String> {
        let inner = self.inner.lock().ok()?;
        inner.request_id_to_trace.get(request_id).cloned()
    }

    fn with_trace_mut<F, R>(&self, trace_id: &str, f: F) -> Result<R, AppError>
    where
        F: FnOnce(&mut GenerationTrace) -> R,
    {
        let mut trace = {
            let mut inner = self.inner.lock().map_err(|e| AppError::Internal {
                message: format!("TraceStore 锁中毒: {}", e),
            })?;
            if let Some(pos) = inner.traces.iter().position(|t| t.trace_id == trace_id) {
                let mut trace = inner.traces.remove(pos).unwrap();
                let result = f(&mut trace);
                inner.traces.push_back(trace.clone());
                (result, trace)
            } else {
                // 尝试从文件加载
                drop(inner);
                let path = self.trace_file_path(trace_id);
                if path.exists() {
                    let content =
                        std::fs::read_to_string(&path).map_err(|e| AppError::Internal {
                            message: format!("读取 trace 文件失败: {}", e),
                        })?;
                    let mut trace: GenerationTrace =
                        serde_json::from_str(&content).map_err(|e| AppError::Internal {
                            message: format!("反序列化 trace 失败: {}", e),
                        })?;
                    let result = f(&mut trace);
                    (result, trace)
                } else {
                    return Err(AppError::NotFound {
                        resource: "trace".to_string(),
                        id: trace_id.to_string(),
                    });
                }
            }
        };
        self.persist(&trace.1)?;
        Ok(trace.0)
    }

    /// 添加一个步骤并返回可修改的引用（调用方需自行 finish）
    pub fn add_step(
        &self,
        trace_id: &str,
        name: impl Into<String>,
        phase: impl Into<String>,
    ) -> Result<TraceStep, AppError> {
        let name = name.into();
        let phase = phase.into();
        self.with_trace_mut(trace_id, |trace| {
            trace.add_step(name.clone(), phase.clone());
            trace.steps.last().cloned().unwrap()
        })
    }

    /// 完成一个步骤
    pub fn finish_step(
        &self,
        trace_id: &str,
        step_idx: usize,
        status: &str,
        error: Option<String>,
    ) -> Result<(), AppError> {
        self.with_trace_mut(trace_id, |trace| {
            if let Some(step) = trace.steps.get_mut(step_idx) {
                let end = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                step.end_ms = Some(end);
                step.duration_ms = Some(end.saturating_sub(step.start_ms));
                step.status = Some(status.to_string());
                if let Some(err) = error {
                    step.error = Some(err);
                }
            }
        })
    }

    /// 直接完成最后一个步骤
    pub fn finish_last_step(
        &self,
        trace_id: &str,
        status: &str,
        error: Option<String>,
    ) -> Result<(), AppError> {
        self.with_trace_mut(trace_id, |trace| {
            if let Some(step) = trace.steps.last_mut() {
                let end = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                step.end_ms = Some(end);
                step.duration_ms = Some(end.saturating_sub(step.start_ms));
                step.status = Some(status.to_string());
                if let Some(err) = error {
                    step.error = Some(err);
                }
            }
        })
    }

    /// 标记 trace 完成
    pub fn finish_trace(
        &self,
        trace_id: &str,
        status: &str,
        error: Option<String>,
    ) -> Result<(), AppError> {
        self.with_trace_mut(trace_id, |trace| {
            trace.close_open_steps(status);
            trace.finish(status, error);
        })
    }

    /// 获取单个 trace
    pub fn get_trace(&self, trace_id: &str) -> Result<GenerationTrace, AppError> {
        {
            let inner = self.inner.lock().map_err(|e| AppError::Internal {
                message: format!("TraceStore 锁中毒: {}", e),
            })?;
            if let Some(trace) = inner.traces.iter().find(|t| t.trace_id == trace_id) {
                return Ok(trace.clone());
            }
        }
        let path = self.trace_file_path(trace_id);
        if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(|e| AppError::Internal {
                message: format!("读取 trace 文件失败: {}", e),
            })?;
            let trace: GenerationTrace =
                serde_json::from_str(&content).map_err(|e| AppError::Internal {
                    message: format!("反序列化 trace 失败: {}", e),
                })?;
            Ok(trace)
        } else {
            Err(AppError::NotFound {
                resource: "trace".to_string(),
                id: trace_id.to_string(),
            })
        }
    }

    /// 列出内存中最近 N 条 trace
    pub fn list_recent(&self, limit: usize) -> Result<Vec<GenerationTrace>, AppError> {
        let inner = self.inner.lock().map_err(|e| AppError::Internal {
            message: format!("TraceStore 锁中毒: {}", e),
        })?;
        Ok(inner.traces.iter().rev().take(limit).cloned().collect())
    }

    /// 添加步骤细节（用于 gateway 候选探测/调用）
    pub fn record_step_detail(
        &self,
        trace_id: &str,
        step_idx: usize,
        detail_fn: impl FnOnce(&mut TraceStep),
    ) -> Result<(), AppError> {
        self.with_trace_mut(trace_id, |trace| {
            if let Some(step) = trace.steps.get_mut(step_idx) {
                detail_fn(step);
            }
        })
    }

    /// v0.26.40: 追加已完成步骤并写入 details（如 prompt_coverage）
    pub fn add_completed_step_with_details(
        &self,
        trace_id: &str,
        name: impl Into<String>,
        phase: impl Into<String>,
        details: serde_json::Value,
    ) -> Result<(), AppError> {
        self.with_trace_mut(trace_id, |trace| {
            let mut step = TraceStep::new(name, phase).finish("completed");
            step.details = Some(details);
            trace.steps.push(step);
        })
    }
}

/// 便捷函数：生成 UUIDv7 风格的 trace_id
pub fn new_trace_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// 便捷函数：为当前请求获取或创建 trace_id
pub fn ensure_trace_id(request_trace_id: &Option<String>) -> String {
    request_trace_id.clone().unwrap_or_else(new_trace_id)
}
