//! 统一前端事件通道
//!
//! 提供跨模块共享的生成状态事件类型与发射辅助函数，
//! 将原本分散在 orchestrator-step / agent-stage-update /
//! llm-generating-progress 等事件中的进度信息聚合为单一的 `generation-status`
//! 事件。

use std::{collections::HashMap, sync::Mutex, time::Instant};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

/// 生成任务各阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationPhase {
    PreparingContext,
    GeneratingCandidates,
    Inspecting,
    Rewriting,
    FinalOutput,
    SavingMemory,
    Completed,
    Error,
    Cancelled,
}

impl GenerationPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            GenerationPhase::PreparingContext => "准备上下文",
            GenerationPhase::GeneratingCandidates => "候选生成",
            GenerationPhase::Inspecting => "内容审校",
            GenerationPhase::Rewriting => "润色改写",
            GenerationPhase::FinalOutput => "最终输出",
            GenerationPhase::SavingMemory => "保存记忆",
            GenerationPhase::Completed => "已完成",
            GenerationPhase::Error => "出错",
            GenerationPhase::Cancelled => "已取消",
        }
    }
}

/// 统一生成状态事件 payload
///
/// 前端 `useBackendActivityListener` 与 `FrontstageApp` 主要消费此事件，
/// 原有事件保持发射以兼容旧版前端/其他消费者。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationStatusEvent {
    pub phase: String,
    pub progress: f32,
    pub message: String,
    pub elapsed_ms: u64,
    pub task_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// 每个生成任务的启动时间，用于计算 elapsed_ms
static TASK_START_TIMES: Lazy<Mutex<HashMap<String, Instant>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn record_or_get_start(task_id: &str) -> Instant {
    let mut map = TASK_START_TIMES.lock().unwrap_or_else(|e| e.into_inner());
    *map.entry(task_id.to_string()).or_insert_with(Instant::now)
}

/// 发射统一的 `generation-status` 事件
///
/// 调用方只需提供当前阶段、进度、文案与任务标识；已用时间由本模块自动维护。
/// 当 phase 为 Completed / Error / Cancelled 时会记录任务结束，但保留开始时间
/// 以便最后一条事件仍能给出合理耗时。
pub fn emit_generation_status<R: Runtime>(
    app_handle: &AppHandle<R>,
    task_id: &str,
    phase: GenerationPhase,
    progress: f32,
    message: impl Into<String>,
    request_id: Option<String>,
) {
    let start = record_or_get_start(task_id);
    let elapsed = start.elapsed().as_millis() as u64;

    let event = GenerationStatusEvent {
        phase: phase.as_str().to_string(),
        progress: progress.clamp(0.0, 1.0),
        message: message.into(),
        elapsed_ms: elapsed,
        task_id: task_id.to_string(),
        request_id,
    };

    let _ = app_handle.emit("generation-status", event);

    // 任务结束时清理，避免内存无限增长
    if matches!(
        phase,
        GenerationPhase::Completed | GenerationPhase::Error | GenerationPhase::Cancelled
    ) {
        let mut map = TASK_START_TIMES.lock().unwrap_or_else(|e| e.into_inner());
        map.remove(task_id);
    }
}

/// 手动记录任务开始时间（可选）
///
/// 当调用方在真正发射事件前已经知道任务开始时，可调用此方法让后续事件的
/// elapsed_ms 更准确。
pub fn record_generation_start(task_id: &str) {
    let mut map = TASK_START_TIMES.lock().unwrap_or_else(|e| e.into_inner());
    map.entry(task_id.to_string()).or_insert_with(Instant::now);
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use tauri::Listener;

    use super::*;

    #[test]
    fn test_emit_generation_status_elapsed_and_serialization() {
        // Tauri test feature provides a lightweight mock app.
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();

        let task_id = "test-task-1";
        record_generation_start(task_id);
        std::thread::sleep(Duration::from_millis(50));

        let payload = Arc::new(std::sync::Mutex::new(None));
        let payload_captured = payload.clone();
        let _listener = handle.listen("generation-status", move |event| {
            let parsed: GenerationStatusEvent = serde_json::from_str(event.payload()).unwrap();
            *payload_captured.lock().unwrap() = Some(parsed);
        });

        emit_generation_status(
            &handle,
            task_id,
            GenerationPhase::GeneratingCandidates,
            0.42,
            "生成候选中",
            Some("req-123".to_string()),
        );

        // Give the mock emitter a moment to deliver the event.
        std::thread::sleep(Duration::from_millis(10));

        let event = payload
            .lock()
            .unwrap()
            .clone()
            .expect("event should be emitted");
        assert_eq!(event.task_id, task_id);
        assert_eq!(event.phase, "候选生成");
        assert!((event.progress - 0.42).abs() < f32::EPSILON);
        assert_eq!(event.message, "生成候选中");
        assert_eq!(event.request_id, Some("req-123".to_string()));
        assert!(
            event.elapsed_ms >= 50,
            "elapsed_ms should be at least 50ms, got {}",
            event.elapsed_ms
        );

        // Serialization should not include request_id when None.
        let without_req = GenerationStatusEvent {
            phase: "completed".to_string(),
            progress: 1.0,
            message: "完成".to_string(),
            elapsed_ms: 100,
            task_id: task_id.to_string(),
            request_id: None,
        };
        let json = serde_json::to_string(&without_req).unwrap();
        assert!(
            !json.contains("request_id"),
            "request_id should be skipped when None"
        );
    }
}
