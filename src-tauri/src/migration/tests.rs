use std::path::PathBuf;

use super::storyforge::{migration_needed_at, storyforge_data_dir_from};

#[test]
fn replaces_last_path_component() {
    let moss = PathBuf::from("/home/user/.local/share/com.storymoss.app");
    let old = storyforge_data_dir_from(&moss);
    assert_eq!(
        old,
        Some(PathBuf::from("/home/user/.local/share/com.storyforge.app"))
    );
}

use std::fs;

use tempfile::TempDir;

use super::storyforge::copy_directory_tree;

#[test]
fn copy_directory_tree_skips_existing_files() {
    let src = TempDir::new().unwrap();
    let dst = TempDir::new().unwrap();
    fs::write(src.path().join("a.txt"), "old").unwrap();
    fs::write(dst.path().join("a.txt"), "new").unwrap();
    fs::write(src.path().join("b.txt"), "old-b").unwrap();

    let copied = copy_directory_tree(src.path(), dst.path(), true).unwrap();
    assert_eq!(copied, 1);
    assert_eq!(fs::read_to_string(dst.path().join("a.txt")).unwrap(), "new");
    assert_eq!(
        fs::read_to_string(dst.path().join("b.txt")).unwrap(),
        "old-b"
    );
}

use rusqlite::Connection;

use super::storyforge::merge_sqlite_databases;

#[test]
fn merge_sqlite_keeps_target_conflicts() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target.db");
    let source = dir.path().join("source.db");

    let t = Connection::open(&target).unwrap();
    t.execute(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);",
        [],
    )
    .unwrap();
    t.execute(
        "INSERT INTO items VALUES (1, 'target-1'), (2, 'target-2');",
        [],
    )
    .unwrap();
    drop(t);

    let s = Connection::open(&source).unwrap();
    s.execute(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);",
        [],
    )
    .unwrap();
    s.execute(
        "INSERT INTO items VALUES (1, 'source-1'), (3, 'source-3');",
        [],
    )
    .unwrap();
    drop(s);

    let merged = merge_sqlite_databases(&target, &source).unwrap();
    assert_eq!(merged, 1);

    let t = Connection::open(&target).unwrap();
    let names: Vec<String> = t
        .prepare("SELECT name FROM items ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert_eq!(names, vec!["target-1", "target-2", "source-3"]);
}

#[test]
fn merge_sqlite_ignores_extra_source_columns() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target.db");
    let source = dir.path().join("source.db");

    let t = Connection::open(&target).unwrap();
    t.execute(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);",
        [],
    )
    .unwrap();
    t.execute(
        "INSERT INTO items VALUES (1, 'target-1'), (2, 'target-2');",
        [],
    )
    .unwrap();
    drop(t);

    let s = Connection::open(&source).unwrap();
    s.execute(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, extra TEXT);",
        [],
    )
    .unwrap();
    s.execute("INSERT INTO items VALUES (3, 'source-3', 'extra');", [])
        .unwrap();
    drop(s);

    let merged = merge_sqlite_databases(&target, &source).unwrap();
    assert_eq!(merged, 1);

    let t = Connection::open(&target).unwrap();
    let names: Vec<String> = t
        .prepare("SELECT name FROM items ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert_eq!(names, vec!["target-1", "target-2", "source-3"]);
}

#[test]
fn merge_sqlite_skips_missing_target_tables() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target.db");
    let source = dir.path().join("source.db");

    let t = Connection::open(&target).unwrap();
    t.execute(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);",
        [],
    )
    .unwrap();
    drop(t);

    let s = Connection::open(&source).unwrap();
    s.execute(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);",
        [],
    )
    .unwrap();
    s.execute("INSERT INTO items VALUES (1, 'source-1');", [])
        .unwrap();
    s.execute("CREATE TABLE legacy (id INTEGER PRIMARY KEY);", [])
        .unwrap();
    s.execute("INSERT INTO legacy VALUES (42);", []).unwrap();
    drop(s);

    let merged = merge_sqlite_databases(&target, &source).unwrap();
    // legacy 表在目标库不存在，应被跳过；items 合并 1 条
    assert_eq!(merged, 1);

    let t = Connection::open(&target).unwrap();
    let count: i64 = t
        .query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
    let legacy_exists: bool = t
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='legacy'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    assert!(
        !legacy_exists,
        "legacy table should not be created in target"
    );
}

use serde_json::json;

use super::storyforge::merge_json_values;

#[test]
fn merge_json_preserves_target_keys() {
    let target = json!({"a": "new", "b": {"x": 2}});
    let source = json!({"a": "old", "b": {"x": 1, "y": 3}, "c": 4});
    let merged = merge_json_values(target, source);
    assert_eq!(merged, json!({"a": "new", "b": {"x": 2, "y": 3}, "c": 4}));
}

use super::storyforge::{
    backup_and_prepare_dir, merge_json_config, rollback_backup, rollback_or_cleanup,
};

#[test]
fn migration_needed_false_when_failed_marker_exists() {
    let dir = TempDir::new().unwrap();
    let dst = dir.path().join("com.storymoss.app");
    fs::create_dir_all(&dst).unwrap();
    let old = dir.path().join("com.storyforge.app");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("cinema_ai.db"), "").unwrap();

    // 有旧数据且无任一标记时，需要迁移
    assert!(migration_needed_at(&dst, &old));

    // 写入失败标记后，不再需要迁移
    fs::write(dst.join(".storyforge_migration_failed"), "").unwrap();
    assert!(!migration_needed_at(&dst, &old));
}

#[test]
fn backup_and_restore_roundtrip() {
    let dir = TempDir::new().unwrap();
    let dst = dir.path().join("com.storymoss.app");
    fs::create_dir(&dst).unwrap();
    fs::write(dst.join("file.txt"), "original").unwrap();

    let backup = backup_and_prepare_dir(&dst).unwrap().unwrap();
    // 复制式备份保留原目录，同时生成备份副本
    assert!(dst.exists());
    assert!(backup.exists());
    assert_eq!(
        fs::read_to_string(dst.join("file.txt")).unwrap(),
        "original"
    );
    assert_eq!(
        fs::read_to_string(backup.join("file.txt")).unwrap(),
        "original"
    );

    // 模拟迁移写入新文件
    fs::write(dst.join("new.txt"), "new").unwrap();

    rollback_backup(&backup, &dst).unwrap();
    assert!(dst.exists());
    assert!(backup.exists());
    assert_eq!(
        fs::read_to_string(dst.join("file.txt")).unwrap(),
        "original"
    );
    assert!(
        !dst.join("new.txt").exists(),
        "rollback should remove files not in backup"
    );
}

#[test]
fn backup_and_prepare_returns_none_when_target_missing() {
    let dir = TempDir::new().unwrap();
    let dst = dir.path().join("com.storymoss.app");
    assert!(!dst.exists());

    let backup = backup_and_prepare_dir(&dst).unwrap();
    assert!(backup.is_none());
}

#[test]
fn backup_and_prepare_returns_none_when_target_empty() {
    let dir = TempDir::new().unwrap();
    let dst = dir.path().join("com.storymoss.app");
    fs::create_dir(&dst).unwrap();

    let backup = backup_and_prepare_dir(&dst).unwrap();
    assert!(backup.is_none());
}

#[test]
fn rollback_backup_cleans_partial_destination() {
    let dir = TempDir::new().unwrap();
    let dst = dir.path().join("com.storymoss.app");
    fs::create_dir(&dst).unwrap();
    fs::write(dst.join("partial.txt"), "partial-data").unwrap();

    let backup = dir.path().join("com.storymoss.app.bak.12345");
    fs::create_dir(&backup).unwrap();
    fs::write(backup.join("original.txt"), "original-data").unwrap();

    rollback_backup(&backup, &dst).unwrap();

    assert!(
        !dst.join("partial.txt").exists(),
        "partial file should be removed before restore"
    );
    assert!(
        dst.join("original.txt").exists(),
        "original file should be restored"
    );
    assert_eq!(
        fs::read_to_string(dst.join("original.txt")).unwrap(),
        "original-data"
    );
    assert!(
        backup.exists(),
        "copy-based backup should remain after restore"
    );
}

#[test]
fn rollback_or_cleanup_clears_partial_destination_when_no_backup() {
    let dir = TempDir::new().unwrap();
    let dst = dir.path().join("com.storymoss.app");
    fs::create_dir(&dst).unwrap();
    fs::write(dst.join("partial.txt"), "partial-data").unwrap();

    rollback_or_cleanup(None, &dst);

    assert!(dst.exists(), "destination directory should remain");
    assert!(
        !dst.join("partial.txt").exists(),
        "destination contents should be cleaned up when no backup existed"
    );
}

#[test]
fn merge_json_config_errors_on_malformed_target() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("config.json");
    let source = dir.path().join("old-config.json");

    fs::write(&target, "not valid json").unwrap();
    fs::write(&source, r#"{"key": "value"}"#).unwrap();

    let result = merge_json_config(&target, &source);
    assert!(
        result.is_err(),
        "malformed target config should fail instead of being overwritten"
    );
}

#[test]
fn merge_json_config_creates_target_when_missing() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("config.json");
    let source = dir.path().join("old-config.json");

    fs::write(&source, r#"{"key": "value", "nested": {"a": 1}}"#).unwrap();

    merge_json_config(&target, &source).unwrap();

    let content = fs::read_to_string(&target).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(parsed["key"], "value");
    assert_eq!(parsed["nested"]["a"], 1);
}
