use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri::command;
use rusqlite::Connection;

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

pub fn merge_sqlite_databases(target: &Path, source: &Path) -> Result<u64, String> {
    let mut conn = Connection::open(target).map_err(|e| format!("打开目标数据库失败: {}", e))?;
    let source_path = source.to_string_lossy();

    conn.execute(&format!("ATTACH DATABASE '{}' AS old", source_path.replace('\'', "''")), [])
        .map_err(|e| format!("ATTACH 旧数据库失败: {}", e))?;

    let mut count = 0u64;
    let mut stmt = conn
        .prepare("SELECT name FROM old.sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
        .map_err(|e| format!("读取旧库表列表失败: {}", e))?;
    let tables: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| format!("枚举旧库表失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("枚举旧库表失败: {}", e))?;
    drop(stmt);

    for table in tables {
        let sql = format!("INSERT OR IGNORE INTO \"{}\" SELECT * FROM old.\"{}\"", table, table);
        match conn.execute(&sql, []) {
            Ok(n) => count += n as u64,
            Err(e) => {
                let _ = conn.execute("DETACH DATABASE old", []);
                return Err(format!("合并表 {} 失败: {}", table, e));
            }
        }
    }

    // 处理 sqlite_sequence：旧库自增值仅用于未设置的表
    let has_seq: bool = conn
        .query_row("SELECT 1 FROM old.sqlite_master WHERE name='sqlite_sequence' AND type='table'", [], |_| Ok(true))
        .unwrap_or(false);
    if has_seq {
        let seqs: Vec<(String, i64)> = conn
            .prepare("SELECT name, seq FROM old.sqlite_sequence")
            .map_err(|e| format!("读取旧库 sqlite_sequence 失败: {}", e))?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|e| format!("读取旧库 sqlite_sequence 失败: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取旧库 sqlite_sequence 失败: {}", e))?;
        for (name, seq) in seqs {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES (?1, ?2)",
                [&name, &seq.to_string()],
            );
        }
    }

    conn.execute("DETACH DATABASE old", [])
        .map_err(|e| format!("DETACH 旧数据库失败: {}", e))?;
    Ok(count)
}

#[command]
pub async fn migrate_storyforge_data(app_handle: AppHandle) -> Result<MigrationResult, String> {
    let Some(src) = storyforge_data_dir(&app_handle) else {
        return Err("无法定位 StoryForge 数据目录".to_string());
    };
    let Some(dst) = moss_data_dir(&app_handle) else {
        return Err("无法定位 StoryMoss 数据目录".to_string());
    };

    let copied = copy_directory_tree(&src, &dst, true)
        .map_err(|e| format!("复制文件失败: {}", e))?;

    let target_db = dst.join("cinema_ai.db");
    let source_db = src.join("cinema_ai.db");
    let mut merged = 0u64;
    if target_db.exists() && source_db.exists() {
        merged = merge_sqlite_databases(&target_db, &source_db)?;
    }

    // 写入迁移标记
    if let Some(marker) = migration_marker_path(&app_handle) {
        fs::write(&marker, "").map_err(|e| format!("写入迁移标记失败: {}", e))?;
    }

    Ok(MigrationResult {
        success: true,
        message: format!("已复制 {} 个文件，合并 {} 条数据库记录", copied, merged),
    })
}
