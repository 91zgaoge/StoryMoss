use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::Serialize;
use serde_json::Value;
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

fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

fn quote_identifier(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

pub fn merge_sqlite_databases(target: &Path, source: &Path) -> Result<u64, String> {
    let conn = Connection::open(target).map_err(|e| format!("打开目标数据库失败: {}", e))?;

    let source_path = source.to_string_lossy();
    let attach_sql = format!("ATTACH DATABASE '{}' AS old", escape_sql_string(&source_path));
    conn.execute(&attach_sql, [])
        .map_err(|e| format!("ATTACH 旧数据库失败: {}", e))?;

    let merge_result: Result<u64, String> = (|| {
        conn.execute("BEGIN IMMEDIATE", [])
            .map_err(|e| format!("开始事务失败: {}", e))?;

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
            let target_table = quote_identifier(&table);
            let source_table = format!("old.{}", quote_identifier(&table));
            let sql = format!("INSERT OR IGNORE INTO {} SELECT * FROM {}", target_table, source_table);
            let n = conn
                .execute(&sql, [])
                .map_err(|e| format!("合并表 {} 失败: {}", table, e))?;
            count += n as u64;
        }

        // 处理 sqlite_sequence：仅当目标库也存在该内部表时才合并，避免无效写入
        let has_source_seq: bool = conn
            .query_row("SELECT 1 FROM old.sqlite_master WHERE name='sqlite_sequence' AND type='table'", [], |_| Ok(true))
            .unwrap_or(false);
        let has_target_seq: bool = conn
            .query_row("SELECT 1 FROM sqlite_master WHERE name='sqlite_sequence' AND type='table'", [], |_| Ok(true))
            .unwrap_or(false);
        if has_source_seq && has_target_seq {
            let mut seq_stmt = conn
                .prepare("SELECT name, seq FROM old.sqlite_sequence")
                .map_err(|e| format!("读取旧库 sqlite_sequence 失败: {}", e))?;
            let seqs: Vec<(String, i64)> = seq_stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(|e| format!("读取旧库 sqlite_sequence 失败: {}", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("读取旧库 sqlite_sequence 失败: {}", e))?;
            drop(seq_stmt);

            for (name, seq) in seqs {
                conn.execute(
                    "INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES (?1, ?2)",
                    [&name, &seq.to_string()],
                )
                .map_err(|e| format!("合并 sqlite_sequence 行 {} 失败: {}", name, e))?;
            }
        }

        conn.execute("COMMIT", [])
            .map_err(|e| format!("提交事务失败: {}", e))?;
        Ok(count)
    })();

    if merge_result.is_err() {
        let _ = conn.execute("ROLLBACK", []);
    }
    let detach_result = conn.execute("DETACH DATABASE old", []);

    match merge_result {
        Ok(count) => detach_result.map(|_| count).map_err(|e| format!("DETACH 旧数据库失败: {}", e)),
        Err(e) => Err(e),
    }
}

pub fn merge_json_values(target: Value, source: Value) -> Value {
    match (target, source) {
        (Value::Object(mut t), Value::Object(s)) => {
            for (k, v) in s {
                if let Some(existing) = t.get_mut(&k) {
                    let taken = std::mem::take(existing);
                    *existing = merge_json_values(taken, v);
                } else {
                    t.insert(k, v);
                }
            }
            Value::Object(t)
        }
        (t, _) => t,
    }
}

pub fn merge_json_config(target: &Path, source: &Path) -> Result<(), String> {
    if !source.exists() {
        return Ok(());
    }
    let source_text = fs::read_to_string(source).map_err(|e| format!("读取旧 config.json 失败: {}", e))?;
    let source_value: Value = serde_json::from_str(&source_text).map_err(|e| format!("解析旧 config.json 失败: {}", e))?;

    let target_value = if target.exists() {
        let text = fs::read_to_string(target).map_err(|e| format!("读取新 config.json 失败: {}", e))?;
        serde_json::from_str(&text).unwrap_or(Value::Object(Default::default()))
    } else {
        Value::Object(Default::default())
    };

    let merged = merge_json_values(target_value, source_value);
    fs::write(target, serde_json::to_string_pretty(&merged).map_err(|e| format!("序列化 config.json 失败: {}", e))?)
        .map_err(|e| format!("写入 config.json 失败: {}", e))?;
    Ok(())
}

pub fn backup_and_prepare<R: tauri::Runtime>(app_handle: &AppHandle<R>) -> Result<Option<PathBuf>, String> {
    let dst = app_handle.path().app_data_dir().map_err(|e| format!("无法定位 StoryMoss 数据目录: {}", e))?;
    backup_and_prepare_dir(&dst)
}

pub(super) fn backup_and_prepare_dir(dst: &Path) -> Result<Option<PathBuf>, String> {
    if !dst.exists() {
        return Ok(None);
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("获取时间戳失败: {}", e))?
        .as_secs();
    let backup = dst.with_extension(format!("app.bak.{}", timestamp));
    fs::rename(dst, &backup).map_err(|e| format!("备份目录失败: {}", e))?;
    Ok(Some(backup))
}

pub fn rollback_backup(backup: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        fs::remove_dir_all(target).map_err(|e| format!("清理目标目录失败: {}", e))?;
    }
    fs::rename(backup, target).map_err(|e| format!("恢复备份失败: {}", e))?;
    Ok(())
}

#[command]
pub async fn migrate_storyforge_data(app_handle: AppHandle) -> Result<MigrationResult, String> {
    let Some(src) = storyforge_data_dir(&app_handle) else {
        return Err("无法定位 StoryForge 数据目录".to_string());
    };
    let Some(dst) = moss_data_dir(&app_handle) else {
        return Err("无法定位 StoryMoss 数据目录".to_string());
    };

    let backup = backup_and_prepare(&app_handle)?;

    let result = (|| -> Result<MigrationResult, String> {
        fs::create_dir_all(&dst).map_err(|e| format!("创建目标目录失败: {}", e))?;
        let copied = copy_directory_tree(&src, &dst, true)
            .map_err(|e| format!("复制文件失败: {}", e))?;

        let target_db = dst.join("cinema_ai.db");
        let source_db = src.join("cinema_ai.db");
        let mut merged = 0u64;
        if target_db.exists() && source_db.exists() {
            merged = merge_sqlite_databases(&target_db, &source_db)?;
        }

        let target_cfg = dst.join("config.json");
        let source_cfg = src.join("config.json");
        if target_cfg.exists() || source_cfg.exists() {
            merge_json_config(&target_cfg, &source_cfg)?;
        }

        if let Some(marker) = migration_marker_path(&app_handle) {
            fs::write(&marker, "").map_err(|e| format!("写入迁移标记失败: {}", e))?;
        }

        Ok(MigrationResult {
            success: true,
            message: format!("已复制 {} 个文件，合并 {} 条数据库记录", copied, merged),
        })
    })();

    match result {
        Ok(res) => {
            if let Some(b) = backup {
                if let Err(e) = fs::remove_dir_all(b) {
                    log::warn!("删除迁移备份目录失败: {}", e);
                }
            }
            Ok(res)
        }
        Err(e) => {
            if let Some(b) = backup {
                if let Err(err) = rollback_backup(&b, &dst) {
                    log::error!("迁移失败且回滚备份失败: {}", err);
                }
            } else if dst.exists() {
                // 目标目录原本不存在，迁移失败后清理任何已创建的部分目录
                if let Err(err) = fs::remove_dir_all(&dst) {
                    log::error!("迁移失败且清理部分创建的目标目录失败: {}", err);
                }
            }
            Err(e)
        }
    }
}
