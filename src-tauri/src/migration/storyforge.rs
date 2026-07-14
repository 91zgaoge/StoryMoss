use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri::command;

const OLD_IDENTIFIER: &str = "com.storyforge.app";
const NEW_IDENTIFIER: &str = "com.storymoss.app";
const MIGRATION_MARKER: &str = ".storyforge_migrated";

pub fn storyforge_data_dir_from(moss_dir: &Path) -> Option<PathBuf> {
    let name = moss_dir.file_name()?.to_str()?;
    if name != NEW_IDENTIFIER {
        return None;
    }
    let parent = moss_dir.parent()?;
    Some(parent.join(OLD_IDENTIFIER))
}

pub fn storyforge_data_dir(app_handle: &AppHandle) -> Option<PathBuf> {
    let moss = app_handle.path().app_data_dir().ok()?;
    storyforge_data_dir_from(&moss)
}

pub fn moss_data_dir(app_handle: &AppHandle) -> Option<PathBuf> {
    app_handle.path().app_data_dir().ok()
}

pub fn migration_marker_path(app_handle: &AppHandle) -> Option<PathBuf> {
    Some(moss_data_dir(app_handle)?.join(MIGRATION_MARKER))
}

pub fn has_storyforge_data(app_handle: &AppHandle) -> bool {
    let Some(old) = storyforge_data_dir(app_handle) else {
        return false;
    };
    if !old.is_dir() {
        return false;
    }
    // 至少包含核心文件之一才认为有数据
    old.join("cinema_ai.db").exists() || old.join("config.json").exists()
}

pub fn migration_needed(app_handle: &AppHandle) -> bool {
    let Some(marker) = migration_marker_path(app_handle) else {
        return false;
    };
    !marker.exists() && has_storyforge_data(app_handle)
}

#[derive(Serialize)]
pub struct MigrationStatus {
    pub needed: bool,
    pub source_path: Option<String>,
}

#[derive(Serialize)]
pub struct MigrationResult {
    pub success: bool,
    pub message: String,
}

pub fn copy_directory_tree(src: &Path, dst: &Path, skip_existing: bool) -> io::Result<u64> {
    let mut copied = 0u64;
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let name = match src_path.file_name() {
            Some(n) => n,
            None => continue,
        };
        let dst_path = dst.join(name);
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copied += copy_directory_tree(&src_path, &dst_path, skip_existing)?;
        } else if ty.is_file() {
            if skip_existing && dst_path.exists() {
                continue;
            }
            fs::copy(&src_path, &dst_path)?;
            copied += 1;
        }
    }
    Ok(copied)
}

#[command]
pub async fn check_storyforge_migration(app_handle: AppHandle) -> Result<MigrationStatus, String> {
    let needed = migration_needed(&app_handle);
    let source_path = storyforge_data_dir(&app_handle).map(|p| p.to_string_lossy().to_string());
    Ok(MigrationStatus { needed, source_path })
}

#[command]
pub async fn migrate_storyforge_data(app_handle: AppHandle) -> Result<MigrationResult, String> {
    let Some(src) = storyforge_data_dir(&app_handle) else {
        return Err("无法定位 StoryForge 数据目录".to_string());
    };
    let Some(dst) = moss_data_dir(&app_handle) else {
        return Err("无法定位 StoryMoss 数据目录".to_string());
    };

    match copy_directory_tree(&src, &dst, true) {
        Ok(copied) => Ok(MigrationResult {
            success: true,
            message: format!("已复制 {} 个文件", copied),
        }),
        Err(e) => Err(format!("复制失败: {}", e)),
    }
}
