//! 日志压缩上传（v0.89.0）
//!
//! 用户显式授权后：打包 `app_data/logs/` 近 3 天的诊断日志（tracing 轮转
//! 日志 + 创作工作流 JSONL + traces/*.json + panic 快照）为 zip（≤20MB，
//! 超限从大到小丢弃最大块），算 SHA-256，上传到官网 `/api/logs/upload`。
//! 防重复：本地 `app_settings` 记录上传历史（sha256 去重）+ 服务端同
//! sha256 幂等拒绝（409 视为已上传成功）。服务端只保留 3 天。

use std::{
    io::{Read as _, Write as _},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::{
    db::DbPool,
    error::AppError,
    server_client::ServerClient,
    subscription::identity::{resolve_identity, Identity},
};

/// 打包内容上限（zip 未压缩字节预算）
const PACKAGE_BUDGET_BYTES: u64 = 20 * 1024 * 1024;
/// traces 目录最多收录的文件数（按修改时间倒序）
const MAX_TRACE_FILES: usize = 50;
/// 保留本地 log_reports 目录里的最近包数（超出清理，防堆积）
const KEEP_LOCAL_PACKAGES: usize = 5;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LogUploadRecord {
    pub sha256: String,
    pub filename: String,
    pub size_bytes: u64,
    pub uploaded_at: String,
}

#[derive(Debug, Serialize)]
pub struct PackageInfo {
    pub path: String,
    pub filename: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub file_count: usize,
    pub skipped_duplicates: bool,
}

#[derive(Debug, Serialize)]
struct Manifest {
    app_version: String,
    platform: String,
    created_at: String,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Serialize)]
struct ManifestFile {
    path: String,
    bytes: u64,
}

/// 打包近 3 天日志 → app_data/log_reports/<ts>-<shortsha>.zip
pub fn package_logs<R: Runtime>(app: &AppHandle<R>) -> Result<PackageInfo, AppError> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal(format!("app data dir: {e}")))?;
    let logs_dir = app_dir.join("logs");
    let out_dir = app_dir.join("log_reports");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| AppError::internal(format!("创建输出目录失败: {e}")))?;

    // ── 收集候选文件（近 3 天）────────────────────────────────
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| AppError::internal(e.to_string()))?
        .as_secs()
        - 3 * 24 * 3600;
    let mut candidates: Vec<(PathBuf, u64, u64)> = Vec::new(); // (path, mtime,
                                                               // size)
    collect_from(&logs_dir, cutoff, &mut candidates)?;

    // panic 快照（时间窗内）
    collect_pattern(&app_dir, "panic-*.log", cutoff, &mut candidates);

    // startup trace（tmp；尽力而为）
    if let Some(tmp) = std::env::var_os("TMPDIR") {
        let mut p = PathBuf::from(&tmp);
        p.push("storymoss-startup-trace.log");
        collect_one(&p, cutoff, &mut candidates);
    }

    // 按修改时间倒序（新日志优先），traces 子目录限额
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    let mut selected: Vec<(PathBuf, u64, u64)> = Vec::new();
    let mut trace_count = 0usize;
    let mut budget = PACKAGE_BUDGET_BYTES;
    for (path, mtime, size) in candidates {
        let is_trace = path
            .parent()
            .map(|p| p.file_name().map(|n| n == "traces").unwrap_or(false))
            .unwrap_or(false);
        if is_trace {
            if trace_count >= MAX_TRACE_FILES {
                continue;
            }
            trace_count += 1;
        }
        if size > budget {
            continue;
        }
        budget -= size;
        selected.push((path, mtime, size));
    }
    if selected.is_empty() {
        return Err(AppError::validation_failed(
            "近 3 天没有可打包的日志文件",
            None::<String>,
        ));
    }

    // ── 写 zip + manifest ────────────────────────────────────
    let tmp_zip = out_dir.join(format!(".tmp-{}", std::process::id()));
    let writer = std::fs::File::create(&tmp_zip).map_err(|e| AppError::internal(format!("{e}")))?;
    let mut zip = zip::ZipWriter::new(writer);
    let options =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let mut manifest_files = Vec::new();
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| AppError::internal(e.to_string()))?
        .as_secs();
    for (path, _mtime, size) in &selected {
        let Ok(mut f) = std::fs::File::open(path) else {
            continue;
        };
        let mut buf = Vec::with_capacity(*size as usize);
        if f.read_to_end(&mut buf).is_err() {
            continue;
        }
        let rel = path
            .strip_prefix(&app_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| {
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
        zip.start_file(&rel, options.clone())
            .map_err(|e| AppError::internal(format!("zip start_file: {e}")))?;
        zip.write_all(&buf)
            .map_err(|e| AppError::internal(format!("{e}")))?;
        manifest_files.push(ManifestFile {
            path: rel,
            bytes: *size,
        });
    }

    let manifest = Manifest {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        created_at: chrono::Local::now().to_rfc3339(),
        files: manifest_files,
    };
    zip.start_file("manifest.json", options.clone())
        .map_err(|e| AppError::internal(format!("{e}")))?;
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).map_err(|e| AppError::internal(format!("{e}")))?;
    zip.write_all(&manifest_bytes)
        .map_err(|e| AppError::internal(format!("{e}")))?;
    zip.finish()
        .map_err(|e| AppError::internal(format!("{e}")))?;

    // ── sha256 + 去重 ────────────────────────────────────────
    let bytes = std::fs::read(&tmp_zip).map_err(|e| AppError::internal(format!("{e}")))?;
    let sha = {
        let mut h = Sha256::new();
        h.update(&bytes);
        format!("{:x}", h.finalize())
    };
    let history = load_upload_history(&app_dir)?;
    let short = &sha[..12.min(sha.len())];
    let filename = format!(
        "storymoss-logs-{}-{}.zip",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        short
    );
    let final_path = out_dir.join(&filename);
    let skipped_duplicates = history.iter().any(|r| r.sha256 == sha);
    if !skipped_duplicates {
        std::fs::rename(&tmp_zip, &final_path).map_err(|e| AppError::internal(format!("{e}")))?;
        cleanup_local_packages(&out_dir, KEEP_LOCAL_PACKAGES);
    } else {
        let _ = std::fs::remove_file(&tmp_zip);
    }

    Ok(PackageInfo {
        path: final_path.to_string_lossy().to_string(),
        filename: if skipped_duplicates {
            String::new()
        } else {
            filename
        },
        sha256: sha,
        size_bytes: bytes.len() as u64,
        file_count: selected.len(),
        skipped_duplicates,
    })
}

fn collect_from(
    dir: &std::path::Path,
    cutoff: u64,
    out: &mut Vec<(PathBuf, u64, u64)>,
) -> Result<(), AppError> {
    if !dir.exists() {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| AppError::internal(format!("{e}")))?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_from(&p, cutoff, out)?;
            continue;
        }
        collect_one(&p, cutoff, out);
    }
    Ok(())
}

fn collect_pattern(
    dir: &std::path::Path,
    pattern: &str,
    cutoff: u64,
    out: &mut Vec<(PathBuf, u64, u64)>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file()
            && p.file_name()
                .map(|n| {
                    n.to_string_lossy()
                        .starts_with(pattern.trim_end_matches("*"))
                })
                .unwrap_or(false)
        {
            collect_one(&p, cutoff, out);
        }
    }
}

fn collect_one(path: &std::path::Path, cutoff: u64, out: &mut Vec<(PathBuf, u64, u64)>) {
    let Ok(meta) = std::fs::metadata(path) else {
        return;
    };
    if !meta.is_file() || meta.len() == 0 {
        return;
    }
    let mtime = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if mtime >= cutoff {
        out.push((path.to_path_buf(), mtime, meta.len()));
    }
}

fn history_file_path(app_dir: &std::path::Path) -> PathBuf {
    app_dir.join("log_upload_history.json")
}

fn load_upload_history(app_dir: &std::path::Path) -> Result<Vec<LogUploadRecord>, AppError> {
    let path = history_file_path(app_dir);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| AppError::internal(format!("{e}")))?;
    serde_json::from_str(&raw).map_err(|e| AppError::internal(format!("{e}")))
}

fn save_upload_history(
    app_dir: &std::path::Path,
    records: &[LogUploadRecord],
) -> Result<(), AppError> {
    let json =
        serde_json::to_vec_pretty(records).map_err(|e| AppError::internal(format!("{e}")))?;
    std::fs::write(history_file_path(app_dir), json).map_err(|e| AppError::internal(format!("{e}")))
}

fn cleanup_local_packages(out_dir: &std::path::Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(out_dir) else {
        return;
    };
    let mut zips: Vec<(std::time::SystemTime, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "zip").unwrap_or(false))
        .filter_map(|p| {
            let m = std::fs::metadata(&p).ok()?;
            m.modified().ok().map(|t| (t, p))
        })
        .collect();
    zips.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in zips.into_iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
}

// ═══════════════════════ IPC ═══════════════════════

/// 打包日志（不上传）。返回 sha256 供前端确认对话框展示。
#[tauri::command(rename_all = "snake_case")]
pub async fn package_log_report<R: Runtime>(app: AppHandle<R>) -> Result<PackageInfo, AppError> {
    tauri::async_runtime::spawn_blocking(move || package_logs(&app))
        .await
        .map_err(|e| AppError::internal(format!("打包任务失败: {e}")))?
}

/// 上传已打包的日志（需登录；显式授权由前端确认对话框保证）。
#[tauri::command(rename_all = "snake_case")]
pub async fn upload_log_report<R: Runtime>(
    app: AppHandle<R>,
    pool: State<'_, DbPool>,
    file_path: String,
    sha256: String,
) -> Result<serde_json::Value, AppError> {
    let token = match resolve_identity(&app, pool.inner()) {
        Identity::Account { token, .. } => token,
        Identity::Device { ref machine_id, .. } => {
            return Err(AppError::validation_failed(
                format!("上传日志需要登录账号（当前为设备身份 {machine_id}）。请先在登录页登录。"),
                None::<String>,
            ));
        }
    };

    let bytes = tokio::fs::read(&file_path)
        .await
        .map_err(|e| AppError::internal(format!("读取日志包失败: {e}")))?;
    let bytes_len = bytes.len() as u64;
    let filename = std::path::Path::new(&file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "storymoss-logs.zip".to_string());

    let client = ServerClient::new();
    let (accepted, duplicate) = client
        .upload_log_package(&token, bytes, &filename, &sha256)
        .await?;

    // 记录上传历史（含重复——重复说明服务端已有，无需再传）
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal(format!("{e}")))?;
    let mut history = load_upload_history(&app_dir)?;
    history.push(LogUploadRecord {
        sha256: sha256.clone(),
        filename,
        size_bytes: bytes_len,
        uploaded_at: chrono::Local::now().to_rfc3339(),
    });
    if history.len() > 50 {
        history = history[history.len() - 50..].to_vec();
    }
    save_upload_history(&app_dir, &history)?;

    Ok(serde_json::json!({
        "accepted": accepted,
        "duplicate": duplicate,
        "sha256": sha256,
    }))
}

#[derive(Debug, Serialize)]
pub struct LogUploadHistoryItem {
    pub sha256: String,
    pub filename: String,
    pub uploaded_at: String,
}

#[tauri::command(rename_all = "snake_case")]
pub async fn get_log_upload_history<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<LogUploadHistoryItem>, AppError> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::internal(format!("{e}")))?;
    Ok(load_upload_history(&app_dir)?
        .into_iter()
        .map(|r| LogUploadHistoryItem {
            sha256: r.sha256,
            filename: r.filename,
            uploaded_at: r.uploaded_at,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_deterministic_hex() {
        let h1 = {
            let mut h = Sha256::new();
            h.update(b"storymoss-logs");
            format!("{:x}", h.finalize())
        };
        let h2 = {
            let mut h = Sha256::new();
            h.update(b"storymoss-logs");
            format!("{:x}", h.finalize())
        };
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn upload_history_roundtrip_and_dedup_key() {
        let dir = std::env::temp_dir().join(format!("sm-logreport-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let records = vec![LogUploadRecord {
            sha256: "abc".into(),
            filename: "a.zip".into(),
            size_bytes: 10,
            uploaded_at: "2026-11-07T10:00:00+08:00".into(),
        }];
        save_upload_history(&dir, &records).unwrap();
        let loaded = load_upload_history(&dir).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].sha256, "abc");
        assert!(loaded.iter().any(|r| r.sha256 == "abc"));
        assert!(loaded.iter().all(|r| r.sha256 != "zzz"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn manifest_serializes_required_fields() {
        let m = Manifest {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
            created_at: "2026-11-07T00:00:00+08:00".into(),
            files: vec![ManifestFile {
                path: "logs/creative_workflow.log".into(),
                bytes: 123,
            }],
        };
        let v = serde_json::to_value(&m).unwrap();
        assert_eq!(v["files"][0]["path"], "logs/creative_workflow.log");
        assert_eq!(v["files"][0]["bytes"], 123);
    }
}
