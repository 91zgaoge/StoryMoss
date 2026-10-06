//! Golden continue harness 的**存在性与门控守卫**。
//!
//! 真正的 harness 是 `src/agency/golden_harness.rs`（`#[cfg(test)] mod`），
//! 不是集成测试：驱动真实续写需要 `LoopLlm`（签名引用私有模块
//! `crate::router::TaskType` / `crate::error::AppError`）、
//! `AgencyCoordinator::for_test`、`db::create_test_pool` 等 crate 内部件，
//! 集成测试碰不到（本文件与 `write_time_bundle_contract_test.rs` 一样只做
//! 源码守卫）。
//!
//! 守卫两件事：
//! 1. harness 仍在、仍被 `src/agency/mod.rs` 注册；
//! 2. 仍是 `#[ignore]` + 环境变量门控 —— 否则 CI 会试图跑一个需要真实模型 /
//!    网络的测量。
//!
//! 跑法见 `src/agency/golden_harness.rs` 顶部文档：
//! `STORYMOSS_GOLDEN_MODEL=<profile 或 model> cargo test --lib golden_continue
//! -- --ignored --nocapture`。

fn read(path: &str) -> String {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("读取 {} 失败: {e}", full.display()))
}

#[test]
fn golden_harness_module_is_registered_and_cfg_test() {
    let mod_rs = read("src/agency/mod.rs");
    assert!(
        mod_rs.contains("#[cfg(test)]\nmod golden_harness;"),
        "src/agency/mod.rs 必须注册 #[cfg(test)] mod golden_harness;"
    );
}

#[test]
fn golden_harness_stays_ignore_gated_and_env_driven() {
    let src = read("src/agency/golden_harness.rs");
    assert!(
        src.contains("#[ignore"),
        "harness 必须保持 #[ignore]：CI 不得执行（需要真实模型与网络）"
    );
    for needle in [
        "STORYMOSS_GOLDEN_MODEL",
        "STORYMOSS_GOLDEN_PROFILE",
        "STORYMOSS_GOLDEN_OUT",
        "STORYMOSS_GOLDEN_TIMEOUT_SECS",
        // 指标口径必须来自生产同一批纯函数，不得自造。
        "probe_increment_ex",
        "trim_self_repetition",
        "strip_existing_overlap",
    ] {
        assert!(
            src.contains(needle),
            "golden_harness.rs 缺少 `{needle}`（门控或指标口径被改动？）"
        );
    }
    // 固定 fixture 必须仍在（改名字/删开篇会让历史 golden 报告不可比）。
    assert!(src.contains("const GOLDEN_OPENING"));
    assert!(src.contains("const GOLDEN_CHARACTERS"));
}
