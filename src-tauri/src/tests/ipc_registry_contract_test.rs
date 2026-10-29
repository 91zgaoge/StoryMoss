//! IPC 注册表契约测试。
//!
//! 背景：`get_canonical_state` 曾在一次 refactor 回滚（b4448325）中从
//! `handlers.rs` 被删掉，只剩下一行段落注释；前端执行面板仍在调用，
//! 每次挂载打三条失败 IPC 并静默显示「铺垫阶段」。当时
//! `verify-ipc-manifest.py` 把它列进白名单，CI 因此是绿的。
//!
//! 这里从源码层钉住"注释下面必须真有命令"，避免同类漂移再来一次。

fn handlers_source() -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/handlers.rs"),
    )
    .expect("handlers.rs 必须存在")
}

#[test]
fn get_canonical_state_is_registered() {
    let src = handlers_source();
    assert!(
        src.contains("canonical_state::commands::get_canonical_state"),
        "get_canonical_state 必须注册在 generate_handler! 中，否则执行面板会静默降级"
    );
}

#[test]
fn canonical_state_section_registers_at_least_one_command() {
    let src = handlers_source();
    let start = src
        .find("// Canonical state commands")
        .expect("应保留 Canonical state 段落注释");
    let rest = &src[start..];
    let section_end = rest[1..]
        .find("\n    // ")
        .map(|i| i + 1)
        .unwrap_or(rest.len());

    assert!(
        rest[..section_end].contains("::"),
        "Canonical state 段落不能只有注释、没有命令"
    );
}
