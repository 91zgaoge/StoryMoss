use std::path::PathBuf;
use super::storyforge::storyforge_data_dir_from;

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
    assert_eq!(fs::read_to_string(dst.path().join("b.txt")).unwrap(), "old-b");
}

use rusqlite::Connection;
use super::storyforge::merge_sqlite_databases;

#[test]
fn merge_sqlite_keeps_target_conflicts() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target.db");
    let source = dir.path().join("source.db");

    let t = Connection::open(&target).unwrap();
    t.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);", []).unwrap();
    t.execute("INSERT INTO items VALUES (1, 'target-1'), (2, 'target-2');", []).unwrap();
    drop(t);

    let s = Connection::open(&source).unwrap();
    s.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);", []).unwrap();
    s.execute("INSERT INTO items VALUES (1, 'source-1'), (3, 'source-3');", []).unwrap();
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
fn merge_sqlite_rolls_back_on_table_error() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target.db");
    let source = dir.path().join("source.db");

    let t = Connection::open(&target).unwrap();
    t.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT);", []).unwrap();
    t.execute("INSERT INTO items VALUES (1, 'target-1'), (2, 'target-2');", []).unwrap();
    drop(t);

    let s = Connection::open(&source).unwrap();
    s.execute("CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT, extra TEXT);", []).unwrap();
    s.execute("INSERT INTO items VALUES (3, 'source-3', 'extra');", []).unwrap();
    drop(s);

    let result = merge_sqlite_databases(&target, &source);
    assert!(result.is_err());

    let t = Connection::open(&target).unwrap();
    let names: Vec<String> = t
        .prepare("SELECT name FROM items ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|x| x.unwrap())
        .collect();
    assert_eq!(names, vec!["target-1", "target-2"]);
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

use super::storyforge::{backup_moss_dir, restore_backup};

#[test]
fn backup_and_restore_roundtrip() {
    let dir = TempDir::new().unwrap();
    let dst = dir.path().join("com.storymoss.app");
    fs::create_dir(&dst).unwrap();
    fs::write(dst.join("file.txt"), "original").unwrap();

    let backup = backup_moss_dir(&dst).unwrap().unwrap();
    assert!(!dst.exists());
    assert!(backup.exists());

    restore_backup(&backup, &dst).unwrap();
    assert!(dst.exists());
    assert!(!backup.exists());
    assert_eq!(fs::read_to_string(dst.join("file.txt")).unwrap(), "original");
}
