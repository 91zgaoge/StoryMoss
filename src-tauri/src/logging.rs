#![allow(dead_code)]
//! StoryMoss 结构化日志系统
//!
//! 使用 tracing + tracing-subscriber + tracing-appender 实现：
//! - 文件日志按日期轮转（daily rotation）
//! - 兼容现有 log:: 宏（通过 tracing-log bridge）
//! - 开发环境同时输出到 stderr（带颜色）
//! - 自动清理超过 7 天的日志文件

use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::Manager;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

use crate::error::AppError;

/// 日志文件保留天数
const LOG_RETENTION_DAYS: u64 = 7;
/// 单日志文件大小上限（字节）
const LOG_FILE_MAX_SIZE: u64 = 10 * 1024 * 1024; // 10MB

/// 组装 panic 现场报告（消息 + 位置 + backtrace）。
/// 抽出为独立函数以便单测——set_hook 全局唯一，无法直接测试 hook 安装。
pub(crate) fn format_panic_report(info: &std::panic::PanicHookInfo) -> String {
    let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
        *s
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.as_str()
    } else {
        "unknown panic"
    };
    let location = info
        .location()
        .map(|l| format!("{}:{}", l.file(), l.line()))
        .unwrap_or_else(|| "unknown location".to_string());
    let backtrace = std::backtrace::Backtrace::force_capture();
    format!(
        "APPLICATION PANIC: {} at {}\n\nBacktrace:\n{}\n",
        payload, location, backtrace
    )
}

/// 安装 panic hook：绕过 tracing 直接写文件，确保崩溃现场留存。
/// 在 setup 最早期（init_logger 之前）调用。
pub fn install_panic_hook(app_dir: &Path) {
    let log_dir = app_dir.join("logs");
    std::panic::set_hook(Box::new(move |info| {
        let content = format_panic_report(info);
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let _ = fs::create_dir_all(&log_dir);
        let _ = fs::write(log_dir.join(format!("panic-{}.log", ts)), &content);
        // 首行即 "APPLICATION PANIC: <msg> at <loc>"，供日志通道复述
        log::error!("{}", content.lines().next().unwrap_or("APPLICATION PANIC"));
        crate::startup_trace::trace(content.lines().next().unwrap_or("APPLICATION PANIC"));
        eprintln!("{}", content);
    }));
}

/// 初始化日志系统
///
/// # 参数
/// - `app_dir`: 应用数据目录，日志将写入 `app_dir/logs/`
///
/// # 返回
/// - `WorkerGuard`: 必须保持存活以确保非阻塞写入器刷新到磁盘
///
/// # 日志级别
/// - 开发环境（debug_assertions）: `debug`
/// - 生产环境: `info`
/// - 可通过 `RUST_LOG` 环境变量覆盖
pub fn init_logger(app_dir: &Path) -> WorkerGuard {
    let log_dir = app_dir.join("logs");
    if let Err(e) = fs::create_dir_all(&log_dir) {
        eprintln!("[StoryMoss] Failed to create log directory: {}", e);
    }

    // 清理过期日志
    cleanup_old_logs(&log_dir);

    // 文件追加器：按日期轮转
    let file_appender = tracing_appender::rolling::daily(&log_dir, "storymoss");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // 构建 EnvFilter
    let default_level = if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    };
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "{}={},storymoss_lib={}",
            default_level, default_level, default_level,
        ))
    });

    // 文件日志层：JSON 结构化格式（生产）或紧凑格式（开发）
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_thread_ids(true)
        .with_target(true)
        .with_level(true)
        .with_line_number(true)
        .with_file(true);

    let file_layer = if cfg!(debug_assertions) {
        file_layer.compact().boxed()
    } else {
        file_layer.json().boxed()
    };

    // stderr 日志层（开发环境带颜色，生产环境可选）
    let stderr_layer = if cfg!(debug_assertions) {
        Some(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_thread_ids(false)
                .with_target(true)
                .with_level(true)
                .with_line_number(true)
                .pretty(),
        )
    } else {
        None
    };

    // 初始化 subscriber
    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer);

    if let Some(stderr) = stderr_layer {
        registry.with(stderr).init();
    } else {
        registry.init();
    }

    // 将 log crate 的日志桥接到 tracing。
    // 注意：tracing-subscriber 启用 tracing-log feature 时（Cargo.lock
    // 已确认启用）， 上面的 registry.
    // init()（SubscriberInitExt::init）会自动初始化 LogTracer，
    // 此处的显式调用必然以 "already initialized" 失败——这是预期行为，仅记
    // debug， 不再输出误导性的 WARN。保留显式调用作为兜底：若未来该 feature
    // 被移除， 此处仍能保证 log:: 记录被桥接，而不是静默丢失。
    if let Err(e) = tracing_log::LogTracer::init() {
        tracing::debug!(
            "[logging] LogTracer::init() skipped (already initialized by subscriber): {}",
            e
        );
    }

    tracing::info!(
        target: "storymoss_lib::logging",
        log_dir = %log_dir.display(),
        retention_days = LOG_RETENTION_DAYS,
        "StoryMoss logging system initialized"
    );

    guard
}

/// 写入前端日志条目到后端日志文件
///
/// 通过 IPC 由前端调用，将前端错误/警告统一收集到后端日志。
/// v0.33.x fix: warn/error 除 tracing daily 文件外，同步写入
/// creative_workflow.log—— 此前仅走 tracing 通道，而 tracing 的 non-blocking
/// writer 在运行期静默丢弃记录 （WorkerGuard 随 setup 闭包结束被
/// drop），导致前端 warn/error 完全不可见。
#[tauri::command]
pub async fn write_frontend_log(
    app_handle: tauri::AppHandle,
    level: String,
    target: String,
    message: String,
    metadata: Option<serde_json::Value>,
) {
    match level.as_str() {
        "error" => {
            tracing::error!(
                target = "storymoss_lib::frontend",
                frontend_target = %target,
                metadata = ?metadata,
                "[FE] {}",
                message
            );
        }
        "warn" => {
            tracing::warn!(
                target = "storymoss_lib::frontend",
                frontend_target = %target,
                metadata = ?metadata,
                "[FE] {}",
                message
            );
        }
        "info" => {
            tracing::info!(
                target = "storymoss_lib::frontend",
                frontend_target = %target,
                metadata = ?metadata,
                "[FE] {}",
                message
            );
        }
        "debug" => {
            tracing::debug!(
                target = "storymoss_lib::frontend",
                frontend_target = %target,
                metadata = ?metadata,
                "[FE] {}",
                message
            );
        }
        _ => {
            tracing::info!(
                target = "storymoss_lib::frontend",
                frontend_target = %target,
                metadata = ?metadata,
                "[FE] {}",
                message
            );
        }
    }

    // warn/error 同步落入 creative_workflow.log（前端关键事件的已验证主通道）。
    // WorkflowLogger 初始化失败时 try_state 返回 None，静默降级为仅 tracing。
    if let Some(logger) =
        app_handle.try_state::<std::sync::Arc<crate::workflow_logger::WorkflowLogger>>()
    {
        match level.as_str() {
            "error" => logger.error(target, message, metadata),
            "warn" => logger.warn(target, message, metadata),
            _ => {}
        }
    }
}

/// 清理超过保留期限的日志文件
fn cleanup_old_logs(log_dir: &Path) {
    let cutoff = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        - LOG_RETENTION_DAYS * 24 * 60 * 60;

    let mut cleaned = 0usize;
    let mut skipped_large = 0usize;

    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // 检查文件扩展名或前缀是否匹配日志文件
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !file_name.starts_with("storymoss") {
                continue;
            }

            // 检查文件大小，超过上限则删除
            if let Ok(metadata) = entry.metadata() {
                if metadata.len() > LOG_FILE_MAX_SIZE {
                    let _ = fs::remove_file(&path);
                    skipped_large += 1;
                    continue;
                }

                // 检查修改时间
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                        if duration.as_secs() < cutoff {
                            continue; // 未过期
                        }
                    }
                }

                let _ = fs::remove_file(&path);
                cleaned += 1;
            }
        }
    }

    if cleaned > 0 || skipped_large > 0 {
        tracing::info!(
            target: "storymoss_lib::logging",
            cleaned,
            skipped_large,
            "Log cleanup completed"
        );
    }
}

/// 获取日志目录路径（供前端展示或导出）
#[tauri::command]
pub fn get_log_directory(app_handle: tauri::AppHandle) -> Result<String, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let log_dir = app_dir.join("logs");
    Ok(log_dir.to_string_lossy().to_string())
}

/// 获取最近日志文件的内容摘要（用于调试或问题报告）
#[tauri::command]
pub fn get_recent_logs(
    app_handle: tauri::AppHandle,
    lines: Option<usize>,
) -> Result<String, AppError> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app dir: {}", e))?;
    let log_dir = app_dir.join("logs");

    // 找到最新的日志文件
    let mut latest: Option<(PathBuf, SystemTime)> = None;
    if let Ok(entries) = fs::read_dir(&log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.starts_with("storymoss") {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if latest.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                        latest = Some((path, modified));
                    }
                }
            }
        }
    }

    let (path, _) = latest.ok_or("No log files found")?;
    let content = fs::read_to_string(&path).map_err(AppError::from)?;

    let lines = lines.unwrap_or(200);
    let collected: Vec<&str> = content.lines().collect();
    let start = collected.len().saturating_sub(lines);
    let recent: Vec<&str> = collected[start..].to_vec();

    Ok(recent.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// panic 现场报告必须包含消息与位置——这是 Windows 闪退诊断的唯一线索。
    /// PanicHookInfo 无公开构造函数，只能通过临时 hook + catch_unwind
    /// 捕获真实实例。
    #[test]
    fn format_panic_report_contains_message_and_location() {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = tx.send(format_panic_report(info));
        }));
        let result = std::panic::catch_unwind(|| {
            panic!("split boom");
        });
        std::panic::set_hook(prev_hook);

        assert!(result.is_err());
        let report = rx.recv().expect("panic hook should produce a report");
        assert!(report.contains("split boom"), "report: {}", report);
        assert!(report.contains("logging.rs"), "report: {}", report);
        assert!(report.contains("Backtrace"), "report: {}", report);
    }
}
