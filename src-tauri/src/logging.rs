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

    // 将 log crate 的日志桥接到 tracing
    if let Err(e) = tracing_log::LogTracer::init() {
        tracing::warn!(
            "[logging] LogTracer::init() failed (log crate logger may already be set): {}",
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
/// 通过 IPC 由前端调用，将前端错误/警告统一收集到后端日志
#[tauri::command]
pub fn write_frontend_log(
    level: String,
    target: String,
    message: String,
    #[allow(unused_variables)] metadata: Option<serde_json::Value>,
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
