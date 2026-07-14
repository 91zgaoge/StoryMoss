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
