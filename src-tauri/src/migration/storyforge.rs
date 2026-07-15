use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};

const OLD_IDENTIFIER: &str = "com.storyforge.app";
const NEW_IDENTIFIER: &str = "com.storymoss.app";
const MIGRATION_MARKER: &str = ".storyforge_migrated";
const MIGRATION_FAILED_MARKER: &str = ".storyforge_migration_failed";
const SOURCE_SCHEMA: &str = "legacy";

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

pub fn migration_failed_marker_path(app_handle: &AppHandle) -> Option<PathBuf> {
    Some(moss_data_dir(app_handle)?.join(MIGRATION_FAILED_MARKER))
}

pub(super) fn has_storyforge_data_at(old: &Path) -> bool {
    if !old.is_dir() {
        return false;
    }
    match old.read_dir() {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => false,
    }
}

pub(super) fn migration_needed_at(moss_dir: &Path, old_dir: &Path) -> bool {
    if moss_dir.join(MIGRATION_MARKER).exists() {
        return false;
    }
    if moss_dir.join(MIGRATION_FAILED_MARKER).exists() {
        return false;
    }
    has_storyforge_data_at(old_dir)
}

pub fn migration_needed(app_handle: &AppHandle) -> bool {
    let Some(moss) = moss_data_dir(app_handle) else {
        return false;
    };
    let Some(old) = storyforge_data_dir(app_handle) else {
        return false;
    };
    migration_needed_at(&moss, &old)
}

#[derive(Serialize)]
pub struct MigrationResult {
    pub success: bool,
    pub message: String,
    pub needs_restart: bool,
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

fn copy_directory_tree_recoverable(src: &Path, dst: &Path) -> io::Result<u64> {
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
            copied += copy_directory_tree_recoverable(&src_path, &dst_path)?;
        } else if ty.is_file() {
            fs::copy(&src_path, &dst_path)?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn remove_dir_contents(dir: &Path) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ty = entry.file_type()?;
        if ty.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

fn quote_identifier(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn table_columns(
    conn: &Connection,
    schema: &str,
    table: &str,
) -> Result<Vec<String>, rusqlite::Error> {
    let sql = format!(
        "PRAGMA {}.table_info({})",
        quote_identifier(schema),
        quote_identifier(table)
    );
    let mut stmt = conn.prepare(&sql)?;
    let cols = stmt
        .query_map([], |row| row.get::<_, String>("name"))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(cols)
}

pub fn merge_sqlite_databases(target: &Path, source: &Path) -> Result<u64, String> {
    let conn = Connection::open(target).map_err(|e| format!("打开目标数据库失败: {}", e))?;

    let source_path = source.to_string_lossy();
    let attach_sql = format!(
        "ATTACH DATABASE '{}' AS {}",
        escape_sql_string(&source_path),
        quote_identifier(SOURCE_SCHEMA)
    );
    conn.execute(&attach_sql, [])
        .map_err(|e| format!("ATTACH 旧数据库失败: {}", e))?;

    conn.execute("PRAGMA foreign_keys = OFF", [])
        .map_err(|e| format!("禁用外键失败: {}", e))?;

    let merge_result: Result<u64, String> = (|| {
        conn.execute("BEGIN IMMEDIATE", [])
            .map_err(|e| format!("开始事务失败: {}", e))?;

        let mut count = 0u64;
        let mut stmt = conn
            .prepare("SELECT name FROM legacy.sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
            .map_err(|e| format!("读取旧库表列表失败: {}", e))?;
        let tables: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| format!("枚举旧库表失败: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("枚举旧库表失败: {}", e))?;
        drop(stmt);

        for table in tables {
            // 跳过目标库中不存在的表，避免旧库特有表导致失败
            let target_exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                    [&table],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if !target_exists {
                continue;
            }

            let target_cols = table_columns(&conn, "main", &table)
                .map_err(|e| format!("读取目标表 {} 列失败: {}", table, e))?;
            let source_cols = table_columns(&conn, SOURCE_SCHEMA, &table)
                .map_err(|e| format!("读取旧表 {} 列失败: {}", table, e))?;

            // 只合并两库共有的列，避免旧库缺少新列导致 SELECT * 失败
            let common_cols: Vec<String> = source_cols
                .into_iter()
                .filter(|c| target_cols.contains(c))
                .collect();
            if common_cols.is_empty() {
                continue;
            }

            let target_table = quote_identifier(&table);
            let col_list = common_cols
                .iter()
                .map(|c| quote_identifier(c))
                .collect::<Vec<_>>()
                .join(", ");
            let source_col_list = common_cols
                .iter()
                .map(|c| quote_identifier(c))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "INSERT OR IGNORE INTO {} ({}) SELECT {} FROM {}.{}",
                target_table,
                col_list,
                source_col_list,
                quote_identifier(SOURCE_SCHEMA),
                target_table
            );
            let n = conn
                .execute(&sql, [])
                .map_err(|e| format!("合并表 {} 失败: {}", table, e))?;
            count += n as u64;
        }

        // 处理 sqlite_sequence：仅当目标库也存在该内部表时才合并，避免无效写入
        let has_source_seq: bool = conn
            .query_row(
                "SELECT 1 FROM legacy.sqlite_master WHERE name='sqlite_sequence' AND type='table'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        let has_target_seq: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE name='sqlite_sequence' AND type='table'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if has_source_seq && has_target_seq {
            let mut seq_stmt = conn
                .prepare("SELECT name, seq FROM legacy.sqlite_sequence")
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

    let restore_fk = || {
        conn.execute("PRAGMA foreign_keys = ON", [])
            .map(|_| ())
            .map_err(|e| format!("恢复外键失败: {}", e))
    };

    let merge_result = if merge_result.is_err() {
        let _ = conn.execute("ROLLBACK", []);
        match restore_fk() {
            Ok(()) => merge_result,
            Err(restore_err) => {
                merge_result.map_err(|e| format!("{}（恢复外键失败: {}）", e, restore_err))
            }
        }
    } else {
        match restore_fk() {
            Ok(()) => merge_result,
            Err(restore_err) => Err(restore_err),
        }
    };

    let detach_result = conn.execute(
        &format!("DETACH DATABASE {}", quote_identifier(SOURCE_SCHEMA)),
        [],
    );

    match merge_result {
        Ok(count) => detach_result
            .map(|_| count)
            .map_err(|e| format!("DETACH 旧数据库失败: {}", e)),
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
    let source_text =
        fs::read_to_string(source).map_err(|e| format!("读取旧 config.json 失败: {}", e))?;
    let source_value: Value = serde_json::from_str(&source_text)
        .map_err(|e| format!("解析旧 config.json 失败: {}", e))?;

    let target_value = if target.exists() {
        let text =
            fs::read_to_string(target).map_err(|e| format!("读取新 config.json 失败: {}", e))?;
        serde_json::from_str(&text)
            .map_err(|e| format!("新 config.json 格式无效，无法合并: {}", e))?
    } else {
        Value::Object(Default::default())
    };

    let merged = merge_json_values(target_value, source_value);
    fs::write(
        target,
        serde_json::to_string_pretty(&merged)
            .map_err(|e| format!("序列化 config.json 失败: {}", e))?,
    )
    .map_err(|e| format!("写入 config.json 失败: {}", e))?;
    Ok(())
}

fn backup_destination_dir(dst: &Path) -> Result<PathBuf, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("获取时间戳失败: {}", e))?
        .as_secs();
    let backup = dst.with_extension(format!("app.bak.{}", timestamp));
    Ok(backup)
}

pub fn backup_and_prepare_dir(dst: &Path) -> Result<Option<PathBuf>, String> {
    if !dst.exists()
        || dst
            .read_dir()
            .map_err(|e| format!("读取目标目录失败: {}", e))?
            .next()
            .is_none()
    {
        return Ok(None);
    }
    let backup = backup_destination_dir(dst)?;
    copy_directory_tree_recoverable(dst, &backup)
        .map_err(|e| format!("备份目标目录失败: {}", e))?;
    Ok(Some(backup))
}

pub fn rollback_backup(backup: &Path, target: &Path) -> Result<(), String> {
    remove_dir_contents(target).map_err(|e| format!("清理目标目录失败: {}", e))?;
    copy_directory_tree_recoverable(backup, target)
        .map_err(|e| format!("从备份恢复目标目录失败: {}", e))?;
    Ok(())
}

pub(super) fn rollback_or_cleanup(backup: Option<&Path>, dst: &Path) -> Result<(), String> {
    if let Some(b) = backup {
        rollback_backup(b, dst)
    } else if dst.exists() {
        // 目标目录原本为空/不存在，迁移失败后清理任何已创建的部分目录
        remove_dir_contents(dst).map_err(|e| format!("清理部分创建的目标目录失败: {}", e))
    } else {
        Ok(())
    }
}

fn write_empty_marker(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建标记父目录失败: {}", e))?;
    }
    fs::write(path, "").map_err(|e| format!("写入标记失败: {}", e))
}

fn write_migration_marker(app_handle: &AppHandle) -> Result<(), String> {
    let Some(marker) = migration_marker_path(app_handle) else {
        return Err("无法定位迁移标记路径".to_string());
    };
    write_empty_marker(&marker)
}

fn write_migration_failed_marker(app_handle: &AppHandle) -> Result<(), String> {
    let Some(marker) = migration_failed_marker_path(app_handle) else {
        return Err("无法定位迁移失败标记路径".to_string());
    };
    write_empty_marker(&marker)
}

fn run_migration_with_backup(
    src: &Path,
    dst: &Path,
    app_handle: &AppHandle,
) -> Result<MigrationResult, String> {
    fs::create_dir_all(dst).map_err(|e| format!("创建目标目录失败: {}", e))?;

    let backup = backup_and_prepare_dir(dst)?;

    let result = (|| -> Result<MigrationResult, String> {
        // 1. 复制旧文件到新目录，跳过已存在的 StoryMoss 文件
        let copied =
            copy_directory_tree(src, dst, true).map_err(|e| format!("复制文件失败: {}", e))?;

        // 2. 合并数据库
        let target_db = dst.join("cinema_ai.db");
        let source_db = src.join("cinema_ai.db");
        let mut merged = 0u64;
        if target_db.exists() && source_db.exists() {
            merged = merge_sqlite_databases(&target_db, &source_db)?;
        }

        // 3. 合并配置
        let target_cfg = dst.join("config.json");
        let source_cfg = src.join("config.json");
        if target_cfg.exists() || source_cfg.exists() {
            merge_json_config(&target_cfg, &source_cfg)?;
        }

        // 4. 写入迁移标记
        write_migration_marker(app_handle)?;

        Ok(MigrationResult {
            success: true,
            message: format!("已复制 {} 个文件，合并 {} 条数据库记录", copied, merged),
            needs_restart: false,
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
            if let Err(rollback_err) = rollback_or_cleanup(backup.as_deref(), dst) {
                return Err(format!("{}（回滚失败: {}）", e, rollback_err));
            }
            if let Err(marker_err) = write_migration_failed_marker(app_handle) {
                return Err(format!("{}（写入失败标记失败: {}）", e, marker_err));
            }
            Err(e)
        }
    }
}

/// 在应用启动 setup 阶段同步执行 StoryForge → StoryMoss 数据迁移。
/// 必须在数据库初始化之前调用，以避免新库被锁定。
pub fn run_storyforge_migration(app_handle: &AppHandle) -> Result<MigrationResult, String> {
    let Some(src) = storyforge_data_dir(app_handle) else {
        return Err("无法定位 StoryForge 数据目录".to_string());
    };
    let Some(dst) = moss_data_dir(app_handle) else {
        return Err("无法定位 StoryMoss 数据目录".to_string());
    };

    log::info!(
        "[Migration] Starting StoryForge → StoryMoss migration from {} to {}",
        src.display(),
        dst.display()
    );

    let result = run_migration_with_backup(&src, &dst, app_handle);
    match &result {
        Ok(res) => log::info!("[Migration] {}", res.message),
        Err(e) => {
            log::error!("[Migration] StoryForge migration failed: {}", e);
        }
    }
    result
}
