use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

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
