//! Updater Module - 自动更新功能
//!
//! 提供应用自动检测更新和安装的功能
//! 基于 tauri-plugin-updater
//!
//! 下载源：GitHub Releases 优先，storymoss.top 备用。两个源共用同一签名公钥；
//! 只有网络故障允许换源，验签/解包/安装失败不得通过换源掩盖。

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Url};
use tauri_plugin_updater::{Error as UpdaterError, Update, Updater, UpdaterExt};

const UPDATE_MANIFEST_TIMEOUT: Duration = Duration::from_secs(20);
const UPDATE_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const UPDATE_READ_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateSource {
    GitHub,
    Website,
}

const UPDATE_SOURCES: [UpdateSource; 2] = [UpdateSource::GitHub, UpdateSource::Website];

impl UpdateSource {
    fn endpoint(self) -> &'static str {
        match self {
            Self::GitHub => {
                "https://github.com/91zgaoge/StoryMoss/releases/latest/download/latest.json"
            }
            Self::Website => "https://storymoss.top/releases/latest.json",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub Releases",
            Self::Website => "storymoss.top",
        }
    }

    fn other(self) -> Self {
        match self {
            Self::GitHub => Self::Website,
            Self::Website => Self::GitHub,
        }
    }
}

struct SourcedUpdate {
    update: Update,
    source: UpdateSource,
}

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

fn same_update_version(expected: &str, candidate: &str) -> bool {
    expected == candidate
}

fn should_retry_download(error: &UpdaterError) -> bool {
    matches!(error, UpdaterError::Reqwest(_) | UpdaterError::Network(_))
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
    format!(
        "无法通过 GitHub 或 storymoss.top 完成更新。请检查网络后重试；\
         也可手动前往 GitHub Releases（https://github.com/91zgaoge/StoryMoss/releases/latest）\
         或官网下载目录（https://storymoss.top/releases/）获取最新安装包。\
         自动更新清单为 latest.json。详情: {err}"
    )
}

fn updater_for_source(app_handle: &AppHandle, source: UpdateSource) -> Result<Updater, String> {
    let endpoint = Url::parse(source.endpoint())
        .map_err(|e| format!("Invalid updater endpoint {}: {}", source.endpoint(), e))?;

    app_handle
        .updater_builder()
        // 清单有 20s 总时限。插件不会把该时限传播给 Update::download，
        // 因此下载通过 configure_client 使用连接/读取超时，不限制大文件总时长。
        .timeout(UPDATE_MANIFEST_TIMEOUT)
        .configure_client(|client| {
            client
                .connect_timeout(UPDATE_CONNECT_TIMEOUT)
                .read_timeout(UPDATE_READ_TIMEOUT)
        })
        .endpoints(vec![endpoint])
        .map_err(|e| format!("Failed to configure {} updater: {}", source.label(), e))?
        .build()
        .map_err(|e| format!("Failed to build {} updater: {}", source.label(), e))
}

async fn check_source(
    app_handle: &AppHandle,
    source: UpdateSource,
) -> Result<Option<SourcedUpdate>, String> {
    let updater = updater_for_source(app_handle, source)?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("{}: {}", source.label(), e))?;

    Ok(update.map(|update| SourcedUpdate { update, source }))
}

/// 吸收单个源的检查结果：命中返回 Some；明确无更新返回 None（调用方继续
/// 检查下一源）；失败时把错误收进 failures 并返回 None。
fn absorb_update_check<T>(
    source: UpdateSource,
    result: Result<Option<T>, String>,
    failures: &mut Vec<String>,
) -> Option<T> {
    match result {
        Ok(Some(update)) => Some(update),
        Ok(None) => {
            log::info!("[Updater] No updates available from {}", source.label());
            None
        }
        Err(error) => {
            log::warn!("[Updater] {} failed: {}", source.label(), error);
            failures.push(error);
            None
        }
    }
}

/// 多源检查的最终裁决（纯函数，便于测试）：
/// 所有源都明确返回无更新时才 Ok(None)；任一源失败则整体报错（不能把
/// “某个源检查失败”误报成“没有更新”）。
fn conclude_update_checks<T>(found: Option<T>, failures: Vec<String>) -> Result<Option<T>, String> {
    if found.is_some() {
        return Ok(found);
    }
    if failures.is_empty() {
        Ok(None)
    } else {
        Err(format_updater_error(failures.join("；")))
    }
}

async fn find_update(app_handle: &AppHandle) -> Result<Option<SourcedUpdate>, String> {
    let mut failures = Vec::new();

    for source in UPDATE_SOURCES {
        let outcome = check_source(app_handle, source).await;
        if let Some(update) = absorb_update_check(source, outcome, &mut failures) {
            log::info!(
                "[Updater] Using {}: update {}",
                source.label(),
                update.update.version
            );
            return Ok(Some(update));
        }
        // Ok(None) 不再提前 return：继续检查后续源。
    }

    conclude_update_checks(None, failures)
}

async fn download_update(app_handle: &AppHandle, update: &Update) -> Result<Vec<u8>, UpdaterError> {
    // 每次下载（含换源重试）使用新的计数器，进度不会跨源累加。
    let downloaded = Arc::new(AtomicU64::new(0));
    let progress_app = app_handle.clone();

    let _ = progress_app.emit(
        "update-download-progress",
        UpdateDownloadProgress {
            downloaded: 0,
            total: None,
            percentage: 0.0,
        },
    );
    let bytes = update
        .download(
            move |chunk_length, content_length| {
                let progress =
                    accumulate_download_progress(&downloaded, chunk_length, content_length);
                let _ = progress_app.emit("update-download-progress", progress);
            },
            || {},
        )
        .await?;

    // 插件会在 download 返回前完成 minisign 验签；只有此时才宣布下载完成。
    let _ = app_handle.emit("update-download-complete", ());
    Ok(bytes)
}

async fn download_with_fallback(
    app_handle: &AppHandle,
    found: &SourcedUpdate,
) -> Result<Vec<u8>, String> {
    match download_update(app_handle, &found.update).await {
        Ok(bytes) => Ok(bytes),
        Err(error) if !should_retry_download(&error) => {
            // 验签/解包等安全或本地错误绝不换源重试。
            Err(format!("更新下载或验签失败，未尝试备用源: {error}"))
        }
        Err(primary_error) => {
            let fallback_source = found.source.other();
            log::warn!(
                "[Updater] {} download failed; trying {}: {}",
                found.source.label(),
                fallback_source.label(),
                primary_error
            );

            match check_source(app_handle, fallback_source).await {
                Ok(Some(candidate))
                    if same_update_version(&found.update.version, &candidate.update.version) =>
                {
                    log::info!(
                        "[Updater] Downloading {} from {}",
                        candidate.update.version,
                        fallback_source.label()
                    );
                    download_update(app_handle, &candidate.update)
                        .await
                        .map_err(|fallback_error| {
                            format_updater_error(format!(
                                "{} 下载失败: {}；{} 下载失败: {}",
                                found.source.label(),
                                primary_error,
                                fallback_source.label(),
                                fallback_error
                            ))
                        })
                }
                Ok(Some(candidate)) => Err(format!(
                    "备用源 {} 的版本 {} 与当前更新 {} 不一致，已停止安装",
                    fallback_source.label(),
                    candidate.update.version,
                    found.update.version
                )),
                Ok(None) => Err(format!(
                    "备用源 {} 没有版本 {} 的可安装更新",
                    fallback_source.label(),
                    found.update.version
                )),
                Err(fallback_error) => Err(format_updater_error(format!(
                    "{} 下载失败: {}；备用源检查失败: {}",
                    found.source.label(),
                    primary_error,
                    fallback_error
                ))),
            }
        }
    }
}

/// 检查是否有可用更新
#[tauri::command]
pub async fn check_update(app_handle: AppHandle) -> Result<CheckUpdateResult, String> {
    let current_version = app_handle.package_info().version.to_string();

    match find_update(&app_handle).await {
        Ok(Some(found)) => {
            let update = found.update;
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
            Err(e)
        }
    }
}

/// 下载并安装更新（带进度事件）
#[tauri::command]
pub async fn install_update(app_handle: AppHandle) -> Result<(), String> {
    let found = find_update(&app_handle)
        .await?
        .ok_or_else(|| "No update available".to_string())?;

    log::info!(
        "[Updater] Downloading update: {} from {}",
        found.update.version,
        found.source.label()
    );
    let bytes = download_with_fallback(&app_handle, &found).await?;

    // download 已完成签名验证；只有拿到合法字节后才进入安装。
    found
        .update
        .install(&bytes)
        .map_err(|e| format!("Failed to install update: {}", e))?;

    log::info!("[Updater] Update installed successfully");
    Ok(())
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
    fn github_is_primary_and_website_is_fallback() {
        assert_eq!(
            UPDATE_SOURCES,
            [UpdateSource::GitHub, UpdateSource::Website]
        );
        assert_eq!(
            UpdateSource::GitHub.endpoint(),
            "https://github.com/91zgaoge/StoryMoss/releases/latest/download/latest.json"
        );
        assert_eq!(
            UpdateSource::Website.endpoint(),
            "https://storymoss.top/releases/latest.json"
        );
        assert!(UPDATE_SOURCES
            .iter()
            .all(|source| source.endpoint().starts_with("https://")));
    }

    #[test]
    fn only_network_download_failures_can_retry() {
        assert!(should_retry_download(&UpdaterError::Network("404".into())));
        assert!(!should_retry_download(&UpdaterError::SignatureUtf8(
            "bad".into()
        )));
        assert!(!should_retry_download(&UpdaterError::PackageInstallFailed));
        assert!(!should_retry_download(&UpdaterError::Io(
            std::io::Error::other("disk")
        )));
    }

    #[test]
    fn no_update_from_first_source_does_not_short_circuit() {
        let mut failures = Vec::new();
        let mut found = None;

        // GitHub 明确无更新：必须继续检查 Website，而不是直接 Ok(None)。
        found = absorb_update_check(
            UpdateSource::GitHub,
            Ok::<Option<&str>, String>(None),
            &mut failures,
        );
        assert!(found.is_none());
        assert!(failures.is_empty());

        // Website 命中更新：最终应采用它。
        found = absorb_update_check(UpdateSource::Website, Ok(Some("0.95.0")), &mut failures);
        assert_eq!(
            conclude_update_checks(found, failures).unwrap(),
            Some("0.95.0")
        );
    }

    #[test]
    fn no_update_only_when_all_sources_report_none() {
        let mut failures = Vec::new();
        let mut found = None;
        for source in UPDATE_SOURCES {
            found = absorb_update_check(source, Ok::<Option<&str>, String>(None), &mut failures);
        }
        assert!(failures.is_empty());
        assert_eq!(conclude_update_checks(found, failures).unwrap(), None);
    }

    #[test]
    fn source_failure_without_update_is_an_error_not_none() {
        let mut failures = Vec::new();
        let mut found = absorb_update_check(
            UpdateSource::GitHub,
            Ok::<Option<&str>, String>(None),
            &mut failures,
        );
        found = absorb_update_check(
            UpdateSource::Website,
            Err("connection reset".into()),
            &mut failures,
        );
        assert!(found.is_none());
        let error = conclude_update_checks(found, failures).unwrap_err();
        assert!(error.contains("connection reset"));
        assert!(error.contains("GitHub"));
    }

    #[test]
    fn download_fallback_requires_same_version() {
        assert!(same_update_version("0.85.1", "0.85.1"));
        assert!(!same_update_version("0.85.1", "0.85.2"));
        assert!(!same_update_version("0.85.1", "0.84.2"));
    }

    #[test]
    fn format_updater_error_mentions_both_sources() {
        let msg = format_updater_error(
            "error sending request for url (https://storymoss.top/releases/latest.json): 404 Not Found",
        );
        assert!(msg.contains("latest.json"));
        assert!(msg.contains("storymoss.top"));
        assert!(msg.contains("GitHub"));
    }
}
