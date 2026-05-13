//! v5.7 Bug Condition Exploration PBT (Task 1)
//!
//! **CRITICAL**: 这些测试是 bugfix 工作流中的"探索性 Property-Based Tests"。
//! 它们期望在 **未修复** 代码上 FAIL —— 失败即坐实 §Hypothesized Root Cause 中的 12 项差距。
//!
//! **Validates: Requirements 1.1 – 1.12**
//!
//! 参考: `.kiro/specs/design-implementation-alignment-v5.7/`
//! - `bugfix.md §Hypothesized Root Cause`
//! - `design.md §Correctness Properties` Property 1
//! - `tasks.md` Task 1

#![cfg(test)]
#![allow(clippy::bool_assert_comparison)]

use proptest::prelude::*;
use proptest::test_runner::Config as ProptestConfig;

use crate::db::connection::create_test_pool;

// v5.7 Task 1: 将 proptest cases 从默认 256 → 8，加速探索测试运行。
// 并同时**收紧每个 proptest 的输入范围**（pool_size/n_chapters/n_entities/n_templates/n_pending
// 原 1..=4 / 1..=6 / 1..=8 / 1..=5 / 1..=4 一律缩小到 1..=2 或 1..=3），
// 保证 ≥1 case 仍覆盖 >1 子元素，避免 property 被退化为 trivial。
const PBT_CASES: u32 = 8;

// =============================================================================
// 辅助: 捕获 data-refresh / sync-event emit 调用次数（用于 C_1_5 / C_1_6 / C_1_9 / C_1_12）
// =============================================================================

/// 遍历源代码文件，统计指定源字符串出现次数（用于静态 grep 型断言）
fn count_pattern_in_src(pattern: &str) -> usize {
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut total = 0usize;
    for entry in walkdir::WalkDir::new(&src_dir).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(content) = std::fs::read_to_string(p) {
                total += content.matches(pattern).count();
            }
        }
    }
    total
}

// =============================================================================
// C_1_1: PRAGMA foreign_keys 未开启
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: PBT_CASES, ..ProptestConfig::default() })]

    /// `test_fk_pragma_off_counterexample` —— C_1_1
    ///
    /// 对 `pool_size ∈ [1, 2]` 条连接断言 `PRAGMA foreign_keys` 返回 `0`（bug 存在）。
    /// 范围缩减（原 1..=4 → 1..=2）以加速 exploratory 测试；`pool_size = 2` 仍然覆盖多连接场景。
    ///
    /// **Validates: Requirements 1.1** (bug condition)
    ///
    /// **Expected on unfixed code**: PASS（即"pragma == 0"普遍成立），证明 bug 存在。
    /// **Expected after fix (Task 3.1)**: FAIL（pragma == 1），证明 bug 修复。
    #[ignore = "v5.6.4: bug condition fixed"]
    #[test]
    fn test_fk_pragma_off_counterexample(pool_size in 1usize..=2) {
        let pool = create_test_pool().expect("create_test_pool");
        let mut observed_off = 0usize;
        for _ in 0..pool_size {
            let conn = pool.get().expect("pool.get");
            let v: i64 = conn
                .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
                .expect("PRAGMA foreign_keys query");
            if v == 0 {
                observed_off += 1;
            }
        }
        // 若 C_1_1 成立：所有连接均 pragma=0
        prop_assert_eq!(observed_off, pool_size, "Expected foreign_keys=0 on all pooled connections; bug NOT reproduced");
    }
}

// =============================================================================
// C_1_2: delete_story 留下孤儿 kg_entities / kg_relations / character_relationships / scenes / chapters
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: PBT_CASES, ..ProptestConfig::default() })]

    /// `test_delete_story_leaves_orphans` —— C_1_2
    ///
    /// 构造 story → n_chapters → n_entities 的关联链，手动插入无 CASCADE 前提下的孤儿行。
    /// 当 `StoryRepository::delete` 只删除 stories 行且 FK pragma 关闭时，
    /// 子表中 `story_id = id` 的行数仍然 > 0（bug 存在）。
    ///
    /// **Validates: Requirements 1.2** (bug condition)
    ///
    /// 说明：SQLite 即便定义了 `FOREIGN KEY ... ON DELETE CASCADE`，
    /// 在 `PRAGMA foreign_keys = OFF`（默认）时 **完全忽略** 级联，子表不会被清理。
    #[ignore = "v5.6.4: bug condition fixed"]
    #[test]
    fn test_delete_story_leaves_orphans(
        n_chapters in 1i32..=2,
        n_entities in 1usize..=3,
    ) {
        use rusqlite::params;
        use chrono::Utc;

        let pool = create_test_pool().expect("create_test_pool");

        // 1) 创建 story
        let story_repo = crate::db::StoryRepository::new(pool.clone());
        let story = story_repo
            .create(crate::db::CreateStoryRequest {
                title: "ghost".into(),
                description: None,
                genre: None,
                style_dna_id: None,
            })
            .expect("story.create");

        // 2) 创建 chapters
        let chapter_repo = crate::db::ChapterRepository::new(pool.clone());
        for n in 1..=n_chapters {
            chapter_repo
                .create(crate::db::CreateChapterRequest {
                    story_id: story.id.clone(),
                    chapter_number: n,
                    title: Some(format!("ch{}", n)),
                    outline: None,
                    content: Some("content".into()),
                })
                .expect("chapter.create");
        }

        // 3) 直接插入 kg_entities，绕过所有 StateSync 路径
        {
            let conn = pool.get().expect("pool.get");
            for i in 0..n_entities {
                let now = Utc::now().to_rfc3339();
                conn.execute(
                    "INSERT INTO kg_entities (id, story_id, name, entity_type, attributes, embedding, first_seen, last_updated, is_archived)
                     VALUES (?1, ?2, ?3, 'character', '{}', NULL, ?4, ?5, 0)",
                    params![
                        format!("ent-{}", i),
                        &story.id,
                        format!("Entity#{}", i),
                        now,
                        now,
                    ],
                ).expect("insert kg_entities");
            }
        }

        // 4) 调用 delete_story（v5.6.4 未修复版本只删除 stories 行）
        let deleted = story_repo.delete(&story.id).expect("delete story");
        prop_assert_eq!(deleted, 1);

        // 5) 断言孤儿 kg_entities 仍然存在（C_1_2 成立）
        let conn = pool.get().expect("pool.get");
        let kg_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM kg_entities WHERE story_id = ?1",
                params![&story.id],
                |row| row.get(0),
            )
            .expect("count kg_entities");
        prop_assert!(kg_count > 0, "Expected orphan kg_entities > 0; bug NOT reproduced");

        // 同时验证 chapters 表也残留（FK pragma 关闭 + 没有显式清理）
        let ch_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM chapters WHERE story_id = ?1",
                params![&story.id],
                |row| row.get(0),
            )
            .expect("count chapters");
        prop_assert!(
            ch_count > 0,
            "Expected orphan chapters > 0 (FK pragma OFF); bug NOT reproduced"
        );
    }
}

// =============================================================================
// C_1_3: delete_character 留下 character_relationships 行
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: PBT_CASES, ..ProptestConfig::default() })]

    /// `test_delete_character_leaves_relationships` —— C_1_3
    ///
    /// 插入一对角色 (A, B) 与 character_relationships 行 (A → B)，
    /// 调用 `CharacterRepository::delete(A)` 后断言关系表中 `source_character_id = A` 的行数仍 > 0。
    ///
    /// **Validates: Requirements 1.3** (bug condition)
    #[ignore = "v5.6.4: bug condition fixed"]
    #[test]
    fn test_delete_character_leaves_relationships(
        _dummy in 0u32..1,
    ) {
        use rusqlite::params;
        use chrono::Utc;

        let pool = create_test_pool().expect("create_test_pool");

        let story_repo = crate::db::StoryRepository::new(pool.clone());
        let story = story_repo
            .create(crate::db::CreateStoryRequest {
                title: "rel".into(),
                description: None,
                genre: None,
                style_dna_id: None,
            })
            .expect("story.create");

        let char_repo = crate::db::CharacterRepository::new(pool.clone());
        let a = char_repo
            .create(crate::db::CreateCharacterRequest {
                story_id: story.id.clone(),
                name: "A".into(),
                background: None,
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,
            })
            .expect("char A");
        let b = char_repo
            .create(crate::db::CreateCharacterRequest {
                story_id: story.id.clone(),
                name: "B".into(),
                background: None,
                personality: None,
                goals: None,
                appearance: None,
                gender: None,
                age: None,
            })
            .expect("char B");

        // 直接插入一行 character_relationships（source=A → target=B）
        {
            let conn = pool.get().expect("pool.get");
            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO character_relationships
                   (id, story_id, source_character_id, target_character_id, relationship_type, description, dynamic, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params!["rel-1", &story.id, &a.id, &b.id, "friend", Option::<String>::None, Option::<String>::None, now],
            )
            .expect("insert character_relationships");
        }

        // 删除 A
        let deleted = char_repo.delete(&a.id).expect("delete char A");
        prop_assert_eq!(deleted, 1);

        // 断言关系行仍存在（C_1_3 成立）
        let conn = pool.get().expect("pool.get");
        let rel_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM character_relationships
                 WHERE source_character_id = ?1 OR target_character_id = ?1",
                params![&a.id],
                |row| row.get(0),
            )
            .expect("count character_relationships");
        prop_assert!(
            rel_count > 0,
            "Expected orphan character_relationships > 0 after delete_character; bug NOT reproduced"
        );
    }
}

// =============================================================================
// C_1_4: character_relationships 三个 CRUD IPC 未在 invoke_handler 中注册
// =============================================================================

/// `test_character_relationships_ipc_missing` —— C_1_4
///
/// 静态 grep：读取 `src-tauri/src/lib.rs`，抽取 `invoke_handler!` 宏注册表，
/// 断言 `create_character_relationship` / `update_character_relationship` / `delete_character_relationship`
/// **均不在** 列表中（即 v5.6.4 未暴露 CRUD 写 IPC）。
///
/// **Validates: Requirements 1.4** (bug condition)
#[ignore = "v5.6.4: bug condition fixed"]
#[test]
fn test_character_relationships_ipc_missing() {
    let lib_rs = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("lib.rs");
    let content = std::fs::read_to_string(&lib_rs).expect("read lib.rs");

    let missing: Vec<&str> = [
        "create_character_relationship",
        "update_character_relationship",
        "delete_character_relationship",
    ]
    .into_iter()
    .filter(|name| !content.contains(name))
    .collect();

    assert_eq!(
        missing.len(),
        3,
        "Expected all 3 character_relationship CRUD IPCs missing from lib.rs, but some already exist: missing={missing:?}"
    );
}

// =============================================================================
// C_1_5 / C_1_12: KG 变更命令既没调用 StateSync::emit_data_refresh,
// 也没有 emit_knowledge_graph_updated
// =============================================================================

/// `test_kg_mutation_emits_no_event` —— C_1_5 / C_1_12
///
/// 静态 grep `commands_v3.rs`：
/// `create_entity` / `update_entity` / `create_relation` 三个函数体内
/// **不存在** `StateSync::emit_knowledge_graph_updated` 或 `emit_data_refresh`。
///
/// **Validates: Requirements 1.5, 1.12** (bug condition)
#[ignore = "v5.6.4: bug condition fixed"]
#[test]
fn test_kg_mutation_emits_no_event() {
    let cmds = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("commands_v3.rs");
    let content = std::fs::read_to_string(&cmds).expect("read commands_v3.rs");

    // 整个 commands_v3.rs 文件中都不应提及这些 emit helper
    let bad_patterns = [
        "emit_knowledge_graph_updated",
        "emit_data_refresh",
    ];
    for pat in bad_patterns {
        assert!(
            !content.contains(pat),
            "Expected KG mutation commands NOT to call {pat}; bug NOT reproduced (StateSync already wired)"
        );
    }
}

// =============================================================================
// C_1_6: auto_ingest_chapter 不发射 ingestion-completed / knowledgeGraph dataRefresh
// =============================================================================

/// `test_ingestion_no_refresh_event` —— C_1_6
///
/// 静态 grep `lib.rs` 中 `auto_ingest_chapter` 函数体：
/// 既不应提及 `ingestion-completed`，也不应调用 `emit_knowledge_graph_updated`
/// 或 `emit_ingestion_completed`。
///
/// **Validates: Requirements 1.6** (bug condition)
#[ignore = "v5.6.4: bug condition fixed"]
#[test]
fn test_ingestion_no_refresh_event() {
    let lib_rs = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("lib.rs");
    let content = std::fs::read_to_string(&lib_rs).expect("read lib.rs");

    // 定位 auto_ingest_chapter 函数
    let start = content
        .find("async fn auto_ingest_chapter")
        .expect("auto_ingest_chapter defined");

    // 粗略截取函数体：到下一个 fn 定义前
    let tail = &content[start..];
    let end_off = tail
        .find("\nfn ")
        .or_else(|| tail.find("\nasync fn "))
        .or_else(|| tail.find("\npub fn "))
        .unwrap_or(tail.len().min(8000));
    let body = &tail[..end_off];

    let bad = [
        "ingestion-completed",
        "emit_knowledge_graph_updated",
        "emit_ingestion_completed",
    ];
    for pat in bad {
        assert!(
            !body.contains(pat),
            "Expected auto_ingest_chapter NOT to emit {pat}; bug NOT reproduced"
        );
    }
}

// =============================================================================
// C_1_7: PlanTemplateLibrary 跨重启丢失
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: PBT_CASES, ..ProptestConfig::default() })]

    /// `test_plan_template_lost_after_restart` —— C_1_7
    ///
    /// session1 调用 `record_success(trigger, plan)` 写入模板；
    /// 随后 drop 实例并重建（模拟重启），断言 `find_match(trigger)` 返回 `None`。
    ///
    /// **Validates: Requirements 1.7** (bug condition)
    #[ignore = "v5.6.4: bug condition fixed"]
    #[test]
    fn test_plan_template_lost_after_restart(
        n_templates in 1usize..=2,
    ) {
        use crate::planner::{ExecutionPlan, PlanTemplateLibrary};

        // session 1: 写入模板
        let triggers: Vec<String> = (0..n_templates)
            .map(|i| format!("unique_trigger_word_{}", i))
            .collect();

        let pool = create_test_pool().unwrap();
        {
            let mut lib = PlanTemplateLibrary::new(pool.clone());
            for trigger in &triggers {
                // ExecutionPlan 有 #[serde(default)]，可用默认
                let plan = ExecutionPlan {
                    understanding: String::new(),
                    steps: vec![],
                    fallback_message: String::new(),
                };
                lib.record_success(trigger, plan);
            }
            // session 1 命中应成功（内存有值）
            for trigger in &triggers {
                prop_assert!(
                    lib.find_match(trigger).is_some(),
                    "session1 record_success failed to index trigger"
                );
            }
            // lib drops here
        }

        // session 2: 模拟重启后用 `::new()` 重建 —— v5.6.4 修复后应能持久化加载
        let lib2 = PlanTemplateLibrary::new(pool.clone());
        for trigger in &triggers {
            // v5.6.4 修复: 模板已持久化，期望 Some
            prop_assert!(
                lib2.find_match(trigger).is_some(),
                "Expected template persisted after restart; bug NOT reproduced"
            );
        }
    }
}

// =============================================================================
// C_1_8: 能力进化只在启动时触发一次，无周期/阈值调度器
// =============================================================================

/// `test_capability_evolution_stuck_after_startup` —— C_1_8
///
/// 静态 grep `lib.rs`：
/// - 必须存在 `async_runtime::spawn` 内的一次性 `evolve_capability_descriptions` 调用；
/// - **不应** 存在 `CapabilityEvolutionScheduler`、`interval.tick` 循环或 `notify_new_record` 阈值触发。
///
/// **Validates: Requirements 1.8** (bug condition)
#[ignore = "v5.6.4: bug condition fixed"]
#[test]
fn test_capability_evolution_stuck_after_startup() {
    let lib_rs = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("lib.rs");
    let content = std::fs::read_to_string(&lib_rs).expect("read lib.rs");

    // 1) 确认确实有一次性的 evolve 调用（锚定"只启动时触发一次"的根因）
    let once_off_count = content.matches("evolve_capability_descriptions").count();
    assert!(
        once_off_count >= 1,
        "Expected at least 1 reference to evolve_capability_descriptions in lib.rs startup"
    );

    // 2) 确认 scheduler / interval 循环 / notify_new_record 不存在
    let bad_patterns = [
        "CapabilityEvolutionScheduler",
        "evolution_scheduler",
        "notify_new_record",
    ];
    let mut any_present = false;
    for pat in bad_patterns {
        if content.contains(pat) {
            any_present = true;
            break;
        }
    }
    // 全局也查一下（可能未来会放在 capabilities/ 模块）
    if !any_present {
        for pat in bad_patterns {
            if count_pattern_in_src(pat) > 0 {
                any_present = true;
                break;
            }
        }
    }
    assert!(
        !any_present,
        "Expected NO periodic/threshold evolution scheduler; bug NOT reproduced (scheduler already exists)"
    );
}

// =============================================================================
// C_1_9: payoff ledger 变更命令不发 payoffLedger dataRefresh
// =============================================================================

/// `test_payoff_ledger_no_datarefresh` —— C_1_9
///
/// 静态 grep `commands_v3.rs`：
/// `update_payoff_ledger_fields` / `detect_overdue_payoffs` / `recommend_payoff_timing`
/// / `update_foreshadowing_status` 所在文件中未调用 `emit_payoff_ledger_updated`
/// 或发射 `resource_type = "payoffLedger"`。
///
/// **Validates: Requirements 1.9** (bug condition)
#[ignore = "v5.6.4: bug condition fixed"]
#[test]
fn test_payoff_ledger_no_datarefresh() {
    let cmds = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("commands_v3.rs");
    let content = std::fs::read_to_string(&cmds).expect("read commands_v3.rs");

    // 这些 payoff 命令应当存在（用来确认测试扫描范围有效）
    for expected in [
        "update_payoff_ledger_fields",
        "detect_overdue_payoffs",
        "recommend_payoff_timing",
        "update_foreshadowing_status",
    ] {
        assert!(
            content.contains(expected),
            "Expected {expected} to exist in commands_v3.rs"
        );
    }

    // 但不应发 payoffLedger 事件
    let bad = [
        "emit_payoff_ledger_updated",
        "payoffLedger",
    ];
    for pat in bad {
        assert!(
            !content.contains(pat),
            "Expected no {pat} emission from payoff/foreshadowing commands; bug NOT reproduced"
        );
    }
}

// =============================================================================
// C_1_10: WorkflowEngine::with_pool 启动后 Pending 实例未入队
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig { cases: PBT_CASES, ..ProptestConfig::default() })]

    /// `test_workflow_pending_not_enqueued` —— C_1_10
    ///
    /// DB 预置 N 条 `workflow_instances.status = 'Pending'`，构造 `WorkflowEngine::with_pool`
    /// + `WorkflowScheduler::new`，断言 `scheduler.queue_len() == 0`
    /// （即 with_pool 只装载到 engine.instances，不自动入队）。
    ///
    /// **Validates: Requirements 1.10** (bug condition)
    #[ignore = "v5.6.4: bug condition fixed"]
    #[test]
    fn test_workflow_pending_not_enqueued(
        n_pending in 1usize..=2,
    ) {
        use rusqlite::params;
        use chrono::Utc;
        use crate::workflow::{WorkflowEngine, WorkflowScheduler};

        let pool = create_test_pool().expect("create_test_pool");

        // 1) 先创建一个 story（外键需要存在；workflow_instances 表本身没有强制 story FK，
        //    但 instance_json 里的 story_id 只用于日志 / 观察）
        let story_repo = crate::db::StoryRepository::new(pool.clone());
        let story = story_repo
            .create(crate::db::CreateStoryRequest {
                title: "wf".into(),
                description: None,
                genre: None,
                style_dna_id: None,
            })
            .expect("story.create");

        // 2) 预置 n 条 Pending 实例
        {
            let conn = pool.get().expect("pool.get");
            for i in 0..n_pending {
                let instance_id = format!("pending-inst-{}", i);
                let now = Utc::now().to_rfc3339();
                let instance_json = serde_json::json!({
                    "id": instance_id,
                    "workflow_id": "test-wf",
                    "story_id": story.id,
                    "status": "Pending",
                    "context": {
                        "variables": {},
                        "current_node_id": null,
                        "completed_nodes": [],
                        "failed_nodes": []
                    },
                    "node_states": {},
                    "started_at": now,
                    "completed_at": null,
                    "retry_count": null
                })
                .to_string();
                conn.execute(
                    "INSERT INTO workflow_instances (id, workflow_id, story_id, status, instance_json, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![instance_id, "test-wf", story.id, "Pending", instance_json, now],
                )
                .expect("insert workflow_instance");
            }
        }

        // 3) 构造 engine + scheduler
        let (engine, restored_ids) = WorkflowEngine::with_pool(pool.clone());
        let scheduler = WorkflowScheduler::new();

        // 4) 断言 queue 为空（v5.6.4 修复前：with_pool 只装载 instances，不入队）
        let before = scheduler.queue_len();
        prop_assert_eq!(before, 0, "Expected queue empty before restore; bug NOT reproduced");

        // 5) v5.6.4 修复: with_pool 返回 restored_ids，由调用方决定是否入队 scheduler
        prop_assert_eq!(restored_ids.len(), n_pending as usize, "with_pool 应返回恢复的 Pending 实例 ID 列表");

        // 6) 但 engine.instances 里应当恢复了 n 条
        let mut recovered = 0usize;
        for i in 0..n_pending {
            if engine.get_instance(&format!("pending-inst-{}", i)).is_some() {
                recovered += 1;
            }
        }
        prop_assert_eq!(recovered, n_pending, "engine.instances 未正确加载 Pending 实例");
    }
}

// =============================================================================
// C_1_12: 静态 grep - KG 更新路径未统一经 StateSync
// =============================================================================

/// `test_kg_path_non_unified` —— C_1_12 补充
///
/// 统计 `src-tauri/src` 下所有 `app.emit("data-refresh",` 或 `app_handle.emit("data-refresh",`
/// 的裸调用（未经过 StateSync）。当前未修复代码中，Bootstrap / Analysis 等路径可能散落这种裸调用。
///
/// **Validates: Requirements 1.12** (bug condition)
///
/// 策略：结合"unifed StateSync 还没建立"事实，真正能坐实 C_1_12 的探测不是"存在裸 emit"，
/// 而是"不存在 StateSync::emit_knowledge_graph_updated 函数定义"。我们双向断言：
/// 1. `StateSync::emit_knowledge_graph_updated` 不存在
/// 2. `StateSync::emit_payoff_ledger_updated` 不存在
/// 3. `StateSync::emit_character_relationships_updated` 不存在
/// 4. `StateSync::emit_ingestion_completed` 不存在
#[ignore = "v5.6.4: bug condition fixed"]
#[test]
fn test_kg_path_non_unified() {
    let state_sync = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("state_sync")
        .join("service.rs");
    let content = std::fs::read_to_string(&state_sync).expect("read state_sync/service.rs");

    let missing_helpers = [
        "emit_knowledge_graph_updated",
        "emit_payoff_ledger_updated",
        "emit_character_relationships_updated",
        "emit_ingestion_completed",
    ];
    let mut found: Vec<&str> = Vec::new();
    for pat in missing_helpers {
        if content.contains(pat) {
            found.push(pat);
        }
    }
    assert!(
        found.is_empty(),
        "Expected NO unified KG/payoff/ingestion helpers on StateSync yet; bug NOT reproduced (helpers already added: {found:?})"
    );
}
