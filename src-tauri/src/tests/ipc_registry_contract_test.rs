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

#[test]
fn corpus_commands_are_registered() {
    let src = handlers_source();
    for command in [
        "corpus::commands::corpus_list",
        "corpus::commands::corpus_get",
        "corpus::commands::corpus_create",
        "corpus::commands::corpus_update",
        "corpus::commands::corpus_delete",
        "corpus::commands::corpus_import_file",
        "corpus::commands::corpus_read_media",
        "corpus::commands::corpus_distill",
        "corpus::commands::corpus_promote",
        "corpus::commands::corpus_link",
        "corpus::commands::corpus_unlink",
    ] {
        assert!(
            src.contains(command),
            "{} 必须注册在 generate_handler! 中（语料页 IPC 契约）",
            command
        );
    }
}

#[test]
fn corpus_section_registers_at_least_one_command() {
    let src = handlers_source();
    let start = src
        .find("// Corpus commands (v0.74.0)")
        .expect("应保留 Corpus commands 段落注释");
    let rest = &src[start..];
    let section_end = rest[1..]
        .find("\n    // ")
        .map(|i| i + 1)
        .unwrap_or(rest.len());

    assert!(
        rest[..section_end].contains("corpus::commands::"),
        "Corpus 段落不能只有注释、没有命令"
    );
}

#[test]
fn director_chat_crew_commands_are_registered() {
    let src = handlers_source();
    for command in [
        "agency::chat_commands::agency_chat_send",
        "agency::chat_commands::agency_chat_sessions",
        "agency::chat_commands::agency_chat_messages",
        "agency::chat_commands::agency_chat_new_session",
        "agency::chat_commands::agency_chat_delete_session",
        "agency::chat_commands::agency_crew_status",
        "agency::chat_commands::agency_crew_dispatch",
        "agency::chat_commands::agency_crew_cancel_task",
        "agency::chat_commands::agency_crew_retry_task",
        "agency::chat_commands::agency_crew_release_lease",
        "agency::chat_commands::agency_director_execute",
        "agency::chat_commands::agency_director_reject",
        "agency::chat_commands::agency_director_patrol",
    ] {
        assert!(
            src.contains(command),
            "{} 必须注册在 generate_handler! 中（导演/剧组 IPC 契约）",
            command
        );
    }
}

#[test]
fn v149_lane_and_lease_migration_shipped() {
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/db/migrations/V149__corpus_and_director.rs"),
    )
    .expect("V149 迁移必须存在");
    assert!(src.contains("idx_agency_runs_one_active_per_lane"));
    assert!(src.contains("story_resource_leases"));
    assert!(src.contains("agency_agent_tasks"));
}
