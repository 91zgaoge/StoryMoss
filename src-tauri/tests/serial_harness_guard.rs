//! 长篇稳定性回归门的源码守卫（P2-6，v0.71.0）。
//!
//! 与 `golden_continue_harness.rs` 同风格：不编译内部件，只对源码做存在性断言，
//! 防止 harness 被删除/失去关键探针（回归门变空门）。

use std::{fs, path::PathBuf};

fn read(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("读取 {} 失败: {e}", path.display()))
}

#[test]
fn serial_harness_module_is_registered_and_cfg_test() {
    let mod_rs = read("agency/mod.rs");
    assert!(
        mod_rs.contains("#[cfg(test)]\nmod serial_harness;"),
        "agency/mod.rs 必须以 cfg(test) 注册 serial_harness 模块"
    );
}

#[test]
fn serial_harness_keeps_cross_beat_probes_and_violation_markers() {
    let harness = read("agency/serial_harness.rs");
    for needle in [
        // 跨拍状态约束的四个探针/状态源
        "refresh_after_text",
        "dead_names",
        "increment_replays_completed_deaths",
        "detect_possession_conflicts",
        "detect_knowledge_leaks",
        // 四类违规句（若被改写，回归门会失真）
        "明成公主再次扑上来",
        "凑到灯下细看",
        "密道入口在井底石缝",
        // 端到端断言锚点（违规不落库、合规落库）
        "违规重演稿不得落库",
        "知识越界稿不得落库",
    ] {
        assert!(
            harness.contains(needle),
            "serial_harness 缺少关键片段: {needle}"
        );
    }
}
