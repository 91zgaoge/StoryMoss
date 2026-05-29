//! 测试辅助工具
//!
//! 提供隔离的临时目录，用于单元测试中需要文件系统操作的场景
//! （如 AppConfig 读写、导出文件等）。

use std::path::PathBuf;

/// 创建隔离的临时应用目录，用于测试文件操作。
///
/// 返回 `(TempDir, PathBuf)` — 目录在 `TempDir` drop 时自动清理。
///
/// # 示例
/// ```rust,no_run
/// let (_tmp, app_dir) = temp_app_dir();
/// let mut config = AppConfig::default();
/// config.save(&app_dir).unwrap();
/// ```
pub fn temp_app_dir() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let path = dir.path().to_path_buf();
    (dir, path)
}
