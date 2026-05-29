//! Updater Module - 自动更新功能
//!
//! 提供应用自动检测更新和安装的功能
//! 基于 tauri-plugin-updater
#![allow(unused_imports)]

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
            Err(format!("Failed to check update: {}", e))
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

            // 下载并安装更新，带进度回调
            update
                .download_and_install(
                    |chunk_length, content_length| {
                        let total = content_length.unwrap_or(0);
                        let percentage = if total > 0 {
                            (chunk_length as f32 / total as f32 * 100.0).min(100.0)
                        } else {
                            0.0
                        };
                        let progress = UpdateDownloadProgress {
                            downloaded: chunk_length as u64,
                            total: content_length,
                            percentage,
                        };
                        let _ = app.emit("update-download-progress", progress);
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
        Err(e) => Err(format!("Failed to check update: {}", e)),
    }
}

/// 获取当前版本
#[tauri::command]
pub fn get_current_version(app_handle: AppHandle) -> String {
    app_handle.package_info().version.to_string()
}
