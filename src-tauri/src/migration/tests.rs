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
