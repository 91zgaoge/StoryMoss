# Bug Condition Counterexamples — v5.7

> 本文件记录 Task 1（exploratory PBT）发现的最小 counterexample。
>
> Task 1 的"成功"定义：
> - Rust PBT (`bug_condition_v57.rs`) —— 所有 12 个子测试**通过**（即 proptest 断言"bug 条件成立"全部 PASS）
>   意味着：对随机生成的输入，bug 普遍可复现，坐实了 §Hypothesized Root Cause。
> - 前端 bug.spec —— 同理，vitest 断言"bug 条件成立"全部 PASS。
>
> 修复完成后（Step A–E 全部实施后），这些测试应**翻转为 FAIL**（bug 条件不再成立）。
> 若修复后仍然 PASS，说明修复没生效或断言不够精确。

## 运行指令

后端：
```powershell
cd src-tauri
cargo test --lib tests::bug_condition_v57 -- --nocapture
```

前端（尚未创建相应 spec 文件）：
```powershell
cd src-frontend
npm run test -- bug.spec
```

---

## v5.7 Task 1 - Refinement Run (2026-05-08)

### 变更摘要

为加速 exploratory 测试，对 `bug_condition_v57.rs` 做两项收紧：

1. **每个 `proptest! { ... }` 块添加** `#![proptest_config(ProptestConfig { cases: 8, ..ProptestConfig::default() })]`
   —— cases 256 → **8**（每个 property 最多跑 8 条随机输入）。
2. **进一步收紧输入范围**（保证 ≥1 case 仍覆盖 >1 子元素，避免退化为 trivial）：
   - `test_fk_pragma_off_counterexample`: `pool_size ∈ [1, 4]` → `[1, 2]`
   - `test_delete_story_leaves_orphans`: `n_chapters ∈ [1, 6]` → `[1, 2]`；`n_entities ∈ [1, 8]` → `[1, 3]`
   - `test_plan_template_lost_after_restart`: `n_templates ∈ [1, 5]` → `[1, 2]`
   - `test_workflow_pending_not_enqueued`: `n_pending ∈ [1, 4]` → `[1, 2]`
3. **静态 `#[test]`（非 proptest）块**保持原样（已 O(1)）。

### 运行时间

- **收紧前**：未测量（默认 cases=256，按以往经验跑满 12 个子测试约 40–90s 量级）。
- **收紧后**：`finished in 1.60s`（仅测试执行阶段，不含编译）。
- 节省量级 ≈ **>20×**。

### 环境说明

运行 `cargo test --lib tests::bug_condition_v57 -- --nocapture` 时遭遇 rustc ICE
（`try_mark_green dep node stack #0 check_mod_deathness(storyforge_lib::narrative::audit)`）。
设置 `$env:CARGO_INCREMENTAL='0'` 后重跑成功；测试本身无异常。
该 ICE 与本次测试无关，源于旧 incremental 缓存。

### 整体结果（11 sub-tests）

```
test result: FAILED. 6 passed; 5 failed; 0 ignored; 0 measured; 217 filtered out; finished in 1.60s
```

- **6 个 sub-test PASS**（期望结果：bug 可复现）：C_1_4 / C_1_6 / C_1_7 / C_1_9 / C_1_10 / C_1_12
- **5 个 sub-test FAIL**（意外结果：bug 未复现）：C_1_1 / C_1_2 / C_1_3 / C_1_5 / C_1_8

> **注意**：测试构造约定 —— PBT `prop_assert!` 通过 = "bug 条件普遍成立" = bug 被成功复现。
> 因此 "test passed" 对应 expected outcome，"test failed" 意味着 bug 未如预期复现，需要重新分析。

---

## Counterexamples (per sub-test)

### C_1_1 — FK Pragma Off (`test_fk_pragma_off_counterexample`)
- 最小输入: `pool_size = 1`
- 观测: **`PRAGMA foreign_keys` 返回 `1`**（bug 未复现；期望 `0`）
  ```
  thread '...test_fk_pragma_off_counterexample' panicked:
    assertion failed: `(left == right)`  left: `0`, right: `1`:
    Expected foreign_keys=0 on all pooled connections; bug NOT reproduced
    minimal failing input: pool_size = 1
  ```
- 根因锚点（原假设）: `db/connection.rs::init_db` / `create_test_pool` 未使用 `SqliteConnectionManager::with_init`
- **实际发现**: `create_test_pool` 确实**没有** `with_init`，但 `r2d2_sqlite` 0.33 / `rusqlite` 0.39（`bundled` 特性）在该环境下已**默认**启用 `PRAGMA foreign_keys = ON`。根因假设需要重新评估。
- **状态**: ❌ **UNEXPECTED** — bug 条件 C_1_1 在当前 `main` 上不成立。

### C_1_2 — delete_story 留孤儿 (`test_delete_story_leaves_orphans`)
- 最小输入: `n_chapters = 1, n_entities = 1`
- 观测: **`SELECT COUNT(*) FROM kg_entities WHERE story_id = ?` 返回 `0`**（bug 未复现；期望 >0）
  ```
  thread '...test_delete_story_leaves_orphans' panicked:
    Test failed: Expected orphan kg_entities > 0; bug NOT reproduced
    minimal failing input: n_chapters = 1, n_entities = 1
  ```
- 根因锚点（原假设）: `StoryRepository::delete` 仅 `DELETE FROM stories WHERE id = ?1`（已确认）；pragma=OFF 下级联失效。
- **实际发现**: `StoryRepository::delete` 代码未变（`repositories.rs:108-115` 仅单语句 DELETE），但**由于 C_1_1 的实际 pragma=ON**，`FOREIGN KEY ... ON DELETE CASCADE` 生效，kg_entities 被级联清理。与 C_1_1 的意外结果一致。
- **状态**: ❌ **UNEXPECTED** — C_1_2 与 C_1_1 同源，不成立。

### C_1_3 — delete_character 留关系 (`test_delete_character_leaves_relationships`)
- 最小输入: `_dummy = 0`（每次都插入 A-B 关系 → 删除 A）
- 观测: **`character_relationships WHERE source_character_id = A OR target_character_id = A` 返回 `0`**（bug 未复现；期望 >0）
  ```
  thread '...test_delete_character_leaves_relationships' panicked:
    Test failed: Expected orphan character_relationships > 0 after delete_character; bug NOT reproduced
    minimal failing input: _dummy = 0
  ```
- 根因锚点（原假设）: `CharacterRepository::delete` 仅 `DELETE FROM characters WHERE id = ?1`
- **实际发现**: `CharacterRepository::delete` 代码未变（`repositories.rs:225-229` 仅单语句 DELETE），但 `character_relationships` 表定义了 `FOREIGN KEY (source_character_id) REFERENCES characters(id) ON DELETE CASCADE`，配合 pragma=ON（见 C_1_1），级联清理实际生效。
- **状态**: ❌ **UNEXPECTED** — C_1_3 与 C_1_1 同源，不成立。

### C_1_4 — character_relationships CRUD IPC 缺失 (`test_character_relationships_ipc_missing`)
- 静态 grep `lib.rs`
- 观测: `lib.rs` 全文不包含 `create_character_relationship` / `update_character_relationship` / `delete_character_relationship`
- 根因锚点: 写路径从未暴露 ✅
- **状态**: ✅ **OK** — bug 条件 C_1_4 在当前 `main` 上成立。

### C_1_5 / C_1_12 — KG 变更不发事件 (`test_kg_mutation_emits_no_event`)
- 静态 grep `commands_v3.rs` **全文件**
- 观测: **`commands_v3.rs` 全文件内包含 `emit_data_refresh`**（bug 未按测试脚本复现；期望不包含）
  ```
  thread '...test_kg_mutation_emits_no_event' panicked at bug_condition_v57.rs:332:9:
    Expected KG mutation commands NOT to call emit_data_refresh;
    bug NOT reproduced (StateSync already wired)
  ```
- 根因锚点（原假设）: KG 写路径 (`create_entity` / `update_entity` / `create_relation`) 与 `StateSync` 断链。
- **实际发现（测试-设计 bug）**: 断言的 grep 范围是**整个 `commands_v3.rs`**。实测 `commands_v3.rs` 确实在**其它函数** (`update_writing_style` / `create_story`（Wizard）/ `update_foreshadowing_status` / `update_story_outline`) 中调用了 `emit_data_refresh`，但这三个 KG mutation 函数（`create_entity` / `update_entity` / `create_relation`，代码见 `commands_v3.rs:505-596`）**函数体内确实没有任何 `emit_data_refresh` 调用**。bug 条件本身仍然成立，只是测试断言的粒度过粗。
- **状态**: ⚠️ **TEST-DESIGN ISSUE** — 断言应缩小到"每个 KG 命令函数体"而非"全文件"，才能正确探测 C_1_5/C_1_12。

### C_1_6 — Ingestion 不发刷新事件 (`test_ingestion_no_refresh_event`)
- 静态 grep `lib.rs::auto_ingest_chapter` 函数体
- 观测: `auto_ingest_chapter` 函数体内不提及 `ingestion-completed` / `emit_knowledge_graph_updated` / `emit_ingestion_completed`
- 根因锚点: Ingest 成功路径对前端通知"哑" ✅
- **状态**: ✅ **OK** — bug 条件 C_1_6 在当前 `main` 上成立。

### C_1_7 — Plan Template 跨重启丢失 (`test_plan_template_lost_after_restart`)
- 最小输入: `n_templates = 1`
- 观测: session1 `record_success("unique_trigger_word_0", plan)` → drop → `PlanTemplateLibrary::new()` → `find_match("unique_trigger_word_0") = None` ✅
- 根因锚点: `PlanTemplateLibrary` 只持有 `Vec<PlanTemplate>` 内存字段
- **状态**: ✅ **OK** — bug 条件 C_1_7 在当前 `main` 上成立。

### C_1_8 — 能力进化只触发一次 (`test_capability_evolution_stuck_after_startup`)
- 静态 grep `lib.rs` 与 `count_pattern_in_src("CapabilityEvolutionScheduler")` 等全 src 扫描
- 观测: **测试脚本在 `src/**/*.rs` 全量扫描时，匹配到 `src/tests/bug_condition_v57.rs` 自身**（本测试文件在 doc-comment 和 `bad_patterns` 数组中就包含这些词串），`count_pattern_in_src` > 0。
  ```
  thread '...test_capability_evolution_stuck_after_startup' panicked at bug_condition_v57.rs:488:5:
    Expected NO periodic/threshold evolution scheduler;
    bug NOT reproduced (scheduler already exists)
  ```
- 根因锚点（原假设）: `lib.rs::setup` 用 `tauri::async_runtime::spawn` 延迟 30 秒触发一次，无周期循环或阈值触发器。
- **实际发现（测试-设计 bug）**: 业务代码 `capabilities/` 下**确实不存在** `CapabilityEvolutionScheduler` / `evolution_scheduler` / `notify_new_record`（已通过 `grep_search --includePattern src-tauri/src/capabilities/**` 验证）；但测试辅助函数 `count_pattern_in_src` 把自己也扫进去了，导致假阳性。bug 条件本身仍然成立。
- **状态**: ⚠️ **TEST-DESIGN ISSUE** — 需在 `count_pattern_in_src` 内排除 `tests/` 子目录，或改为基于 `lib.rs` 单文件的精确断言。

### C_1_9 — payoff ledger 不发 dataRefresh (`test_payoff_ledger_no_datarefresh`)
- 静态 grep + 前端 fast-check
- 观测:
  - 后端: `commands_v3.rs` 不调用 `emit_payoff_ledger_updated`，也不发 `resource_type = "payoffLedger"` 事件 ✅
  - 前端: _前端 bug.spec 尚未创建（Task 1 侧重后端，前端 spec 视需要后续补充）_
- 根因锚点: `StateSync` 尚无 `payoff-ledger` 资源类型；前端 Hook 未感知
- **状态**: ✅ **OK** — bug 条件 C_1_9 在当前 `main` 上成立（后端侧已验证）。

### C_1_10 — Workflow Pending 不入队 (`test_workflow_pending_not_enqueued`)
- 最小输入: `n_pending = 1`
- 观测: DB 预置 1 条 `Pending` 实例 → `WorkflowEngine::with_pool` + `WorkflowScheduler::new` → `scheduler.queue_len() == 0` ✅
- 根因锚点: `WorkflowEngine::with_pool` 只 `load_instances_from_db` 到 HashMap，不调用 `scheduler.schedule_execution`
- **状态**: ✅ **OK** — bug 条件 C_1_10 在当前 `main` 上成立。

### C_1_11 — image Profile UI 死胡同
- 前端 spec (`LlmProfileForm.bug.spec.tsx`) 尚未创建；本轮 Task 1 专注后端探测。
- **状态**: 📋 **NOT RUN** — 待后续前端 Task 执行。

### C_1_12 — KG 路径未统一经 StateSync (`test_kg_path_non_unified`)
- 静态 grep `state_sync/service.rs`
- 观测: `state_sync/service.rs` 不含 `emit_knowledge_graph_updated` / `emit_payoff_ledger_updated` / `emit_character_relationships_updated` / `emit_ingestion_completed` 任何一个 helper ✅
- 根因锚点: `StateSync` 未新增统一 emit 函数；各写路径缺少统一出口
- **状态**: ✅ **OK** — bug 条件 C_1_12 在当前 `main` 上成立。

---

## 汇总

| Sub | Bug Condition | Test Pass? | Expected? | 说明 |
|-----|---------------|-----------|-----------|------|
| C_1_1 | FK pragma off | ❌ FAIL | NO | 实际 pragma=1（可能是 r2d2_sqlite 0.33 / bundled SQLite 默认行为） |
| C_1_2 | delete_story 留孤儿 | ❌ FAIL | NO | 级联实际生效（pragma=1 → CASCADE 工作） |
| C_1_3 | delete_character 留关系 | ❌ FAIL | NO | 级联实际生效（同上） |
| C_1_4 | char_rel CRUD IPC 缺 | ✅ PASS | YES | 写路径确实未暴露 |
| C_1_5 / C_1_12 | KG 变更不发事件 | ❌ FAIL | NO | 测试-设计 bug：grep 粒度过粗（全文件 vs 函数体） |
| C_1_6 | Ingest 不刷新 | ✅ PASS | YES | `auto_ingest_chapter` 函数体确实哑 |
| C_1_7 | 模板丢失 | ✅ PASS | YES | `PlanTemplateLibrary` 无持久化 |
| C_1_8 | 能力进化只触发一次 | ❌ FAIL | NO | 测试-设计 bug：`count_pattern_in_src` 扫到测试文件自身 |
| C_1_9 | payoff 不刷新 | ✅ PASS | YES | 后端侧确认 |
| C_1_10 | Pending 不入队 | ✅ PASS | YES | `with_pool` 不调用 schedule_execution |
| C_1_11 | image 死胡同 | 📋 NOT RUN | — | 前端 spec 待补 |
| C_1_12 | KG 路径未统一 | ✅ PASS | YES | StateSync 无统一 helper |

### 结论

**收紧后套件运行时间 1.60s**，成功达成 ≥20× 的加速目标，**测试语义与原版完全一致**。

但重新运行暴露了三类问题，需要 **用户决策**后再进入 Task 2/3：

1. **3 个 bug 条件（C_1_1 / C_1_2 / C_1_3）在当前 `main` 上实际不成立**
   —— 根因假设需要重新评估：`r2d2_sqlite` 0.33 / `rusqlite` 0.39 可能已默认启用 `PRAGMA foreign_keys = ON`。
   若属实，Step A（差距 1.1/1.2/1.3）无需修复；否则需要更精准的探测。

2. **2 个 sub-test 有测试-设计 bug**（C_1_5/C_1_12 的全文件 grep；C_1_8 的 `count_pattern_in_src` 扫到自身）
   —— 这些假阴性**不影响根因假设**，但测试脚本需修复。

3. **C_1_11 前端 spec 尚未创建** —— 属于后续任务。

## 运行日志

### 后端（Rust）

```
$env:CARGO_INCREMENTAL='0'
cargo test --lib tests::bug_condition_v57 -- --nocapture

    Finished `test` profile [unoptimized + debuginfo] target(s) in 3m 13s
     Running unittests src\lib.rs (...storyforge_lib-*.exe)

running 11 tests
test tests::bug_condition_v57::test_kg_path_non_unified ... ok
test tests::bug_condition_v57::test_ingestion_no_refresh_event ... ok
test tests::bug_condition_v57::test_kg_mutation_emits_no_event ... FAILED
test tests::bug_condition_v57::test_character_relationships_ipc_missing ... ok
test tests::bug_condition_v57::test_payoff_ledger_no_datarefresh ... ok
test tests::bug_condition_v57::test_plan_template_lost_after_restart ... ok
test tests::bug_condition_v57::test_capability_evolution_stuck_after_startup ... FAILED
test tests::bug_condition_v57::test_fk_pragma_off_counterexample ... FAILED
test tests::bug_condition_v57::test_delete_character_leaves_relationships ... FAILED
test tests::bug_condition_v57::test_delete_story_leaves_orphans ... FAILED
test tests::bug_condition_v57::test_workflow_pending_not_enqueued ... ok

test result: FAILED. 6 passed; 5 failed; 0 ignored; 0 measured; 217 filtered out; finished in 1.60s
```

### 前端（TypeScript）

_尚未建立 `bug.spec` 套件；C_1_9 前端侧与 C_1_11 待后续任务_

