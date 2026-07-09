//! Updater Module - 自动更新功能
//!
//! 提供应用自动检测更新和安装的功能
//! 基于 tauri-plugin-updater
//!
//! 下载源：`plugins.updater.endpoints` → GitHub Releases
//! `https://github.com/91zgaoge/StoryForge/releases/latest/download/latest.json`
#![allow(unused_imports)]

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// 更新信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub pub_date: String,
    pub signature: String,
}

/// 检查更新结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckUpdateResult {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_info: Option<UpdateInfo>,
}

/// 下载进度事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub percentage: f32,
}

/// 将单次 chunk 累加为下载进度（契约：percentage 基于累计字节，非单 chunk）。
pub(crate) fn accumulate_download_progress(
    downloaded: &AtomicU64,
    chunk_length: usize,
    content_length: Option<u64>,
) -> UpdateDownloadProgress {
    let total_downloaded =
        downloaded.fetch_add(chunk_length as u64, Ordering::Relaxed) + chunk_length as u64;
    let total = content_length.unwrap_or(0);
    let percentage = if total > 0 {
        (total_downloaded as f32 / total as f32 * 100.0).min(100.0)
    } else {
        0.0
    };
    UpdateDownloadProgress {
        downloaded: total_downloaded,
        total: content_length,
        percentage,
    }
}

fn format_updater_error(err: impl std::fmt::Display) -> String {
    let msg = err.to_string();
    let lower = msg.to_lowercase();
    if lower.contains("404")
        || lower.contains("not found")
        || lower.contains("failed to fetch")
        || lower.contains("error decoding response body")
    {
        format!(
            "无法从 GitHub 读取更新清单（latest.json）。\
             请确认最新正式版 Release 已包含 latest.json：\
             https://github.com/91zgaoge/StoryForge/releases/latest 。详情: {msg}"
        )
    } else {
        format!("Failed to check update: {msg}")
    }
}

/// 检查是否有可用更新
#[tauri::command]
pub async fn check_update(app_handle: AppHandle) -> Result<CheckUpdateResult, String> {
    let updater = app_handle
        .updater()
        .map_err(|e| format!("Failed to get updater: {}", e))?;

    let current_version = app_handle.package_info().version.to_string();

    match updater.check().await {
        Ok(Some(update)) => {
            log::info!(
                "[Updater] Update available: {} -> {}",
                current_version,
                update.version
            );

            Ok(CheckUpdateResult {
                has_update: true,
                current_version,
                latest_version: Some(update.version.clone()),
                update_info: Some(UpdateInfo {
                    version: update.version,
                    notes: update.body.unwrap_or_default(),
                    pub_date: update.date.map(|d| d.to_string()).unwrap_or_default(),
                    signature: update.signature,
                }),
            })
        }
        Ok(None) => {
            log::info!(
                "[Updater] No updates available, current version: {}",
                current_version
            );
            Ok(CheckUpdateResult {
                has_update: false,
                current_version,
                latest_version: None,
                update_info: None,
            })
        }
        Err(e) => {
            log::error!("[Updater] Failed to check update: {}", e);
            Err(format_updater_error(e))
        }
    }
}

/// 下载并安装更新（带进度事件）
#[tauri::command]
pub async fn install_update(app_handle: AppHandle) -> Result<(), String> {
    let updater = app_handle
        .updater()
        .map_err(|e| format!("Failed to get updater: {}", e))?;

    match updater.check().await {
        Ok(Some(update)) => {
            log::info!("[Updater] Downloading update: {}", update.version);

            let app = app_handle.clone();
            let downloaded = Arc::new(AtomicU64::new(0));

            // 下载并安装更新，带进度回调（chunk 累加）
            update
                .download_and_install(
                    {
                        let app = app.clone();
                        let downloaded = Arc::clone(&downloaded);
                        move |chunk_length, content_length| {
                            let progress = accumulate_download_progress(
                                &downloaded,
                                chunk_length,
                                content_length,
                            );
                            let _ = app.emit("update-download-progress", progress);
                        }
                    },
                    || {
                        log::info!("[Updater] Download completed");
                        let _ = app.emit("update-download-complete", ());
                    },
                )
                .await
                .map_err(|e| format!("Failed to install update: {}", e))?;

            log::info!("[Updater] Update installed successfully");
            Ok(())
        }
        Ok(None) => Err("No update available".to_string()),
        Err(e) => Err(format_updater_error(e)),
    }
}

/// 获取当前版本
#[tauri::command]
pub fn get_current_version(app_handle: AppHandle) -> String {
    app_handle.package_info().version.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulate_download_progress_sums_chunks() {
        let downloaded = AtomicU64::new(0);
        let p1 = accumulate_download_progress(&downloaded, 25, Some(100));
        assert_eq!(p1.downloaded, 25);
        assert!((p1.percentage - 25.0).abs() < f32::EPSILON);

        let p2 = accumulate_download_progress(&downloaded, 75, Some(100));
        assert_eq!(p2.downloaded, 100);
        assert!((p2.percentage - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn format_updater_error_mentions_github_on_404() {
        let msg = format_updater_error(
            "error sending request for url (https://github.com/.../latest.json): 404 Not Found",
        );
        assert!(msg.contains("latest.json"));
        assert!(msg.contains("GitHub"));
    }
}
