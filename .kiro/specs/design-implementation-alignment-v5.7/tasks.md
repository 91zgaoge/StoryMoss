# Implementation Plan — 设计-实现对齐 v5.7

> 本任务列表严格遵循 bugfix 工作流的"先探索、再保持、后实施、最后校验"顺序。
> - **Property 1**（探索测试）先于任何修复编写，运行在 **未修复** 代码上必须 FAIL，以此坐实 12 项差距的存在与根因假设。
> - **Property 2**（保持性测试）也在未修复代码上 PASS，锚定 v5.6.4 的既有行为作为"不应改变"的基线。
> - 随后按 Step A→E 的顺序实施修复，每一步实施后分别回放 Property 1（期望转为 PASS）与 Property 2（期望保持 PASS）。
>
> 测试载体：
> - 后端（Rust）：`src-tauri/src/**/tests`，使用 `#[cfg(test)]` + `proptest`（property-based）+ `serial_test`（涉及全局单例时）。
> - 前端（TypeScript）：`src-frontend/src/**/__tests__`，使用 `vitest` + `fast-check`（property-based）。
> - 集成：`e2e/*.spec.ts`（Playwright + Chromium，已配置）。

---

- [ ] 1. Write bug condition exploration test (PBT)
  - **Property 1: Bug Condition** - 12 项设计-实现差距联合复现
  - **CRITICAL**: 此测试在未修复代码（当前 `main`）上必须 FAIL；失败即坐实 bug 存在。
  - **DO NOT attempt to fix the test or the code when it fails** — 失败是预期结果，用来验证 §Hypothesized Root Cause。
  - **GOAL**: 对 `isBugCondition(X)` 的 12 个子条件各生成一个最小 counterexample，观测失败模式并记录。
  - **Scoped PBT Approach**: 这 12 项差距均为确定性 bug，property 被显式缩放到"已知触发输入"：
    - 后端 proptest：
      - `test_fk_pragma_off_counterexample`：`proptest!` 生成 `pool_size ∈ [1, 8]`，对每条连接断言 `query_row("PRAGMA foreign_keys", NO_PARAMS, |r| r.get::<_, i64>(0))` 等于 `0`（证明 C_1_1 成立）。
      - `test_delete_story_leaves_orphans`：`proptest!` 生成完整关联链 `story → n_chapters ∈ [1, 10] → n_scenes ∈ [1, 5 per chapter] → n_entities ∈ [0, 8]`，调用 `delete_story(id)` 后断言 `SELECT COUNT(*) FROM kg_entities WHERE story_id = ?` > 0（证明 C_1_2）。
      - `test_delete_character_leaves_relationships`：生成随机 `(A, B, relation_type)`，插入 `character_relationships`，`delete_character(A)` 后断言关系表中 `source_character_id = A OR target_character_id = A` 的行数 > 0（证明 C_1_3）。
      - `test_character_relationships_ipc_missing`：遍历 `invoke_handler!` 注册表，断言 `create_character_relationship` / `update_character_relationship` / `delete_character_relationship` **不在** 列表中（证明 C_1_4）。
      - `test_kg_mutation_emits_no_event`：`mock StateSync` 捕获所有 `emit`，对 `create_entity` / `update_entity` / `create_relation` 断言 100ms 窗口内无 `resourceType = "knowledgeGraph"` 事件（证明 C_1_5 / C_1_12）。
      - `test_ingestion_no_refresh_event`：mock LLM 返回 `∈ [1, 5]` 个实体，触发 `auto_ingest_chapter`，断言 `ingestion-completed` 与 `knowledgeGraph` dataRefresh 事件数量均为 0（证明 C_1_6）。
      - `test_plan_template_lost_after_restart`：`proptest!` 生成 `∈ [1, 20]` 个 `(trigger, plan)` pair，在 session1 `record_success`，随后 drop `PlanExecutor` 并用相同 `DbPool` 重建，断言 `find_template` 返回 `None`（证明 C_1_7）。
      - `test_capability_evolution_stuck_after_startup`：推进虚拟时钟 `∈ [30s, 3h]`，写入 `n ∈ [5, 50]` 条新 `ExecutionRecord`，断言 `evolve_capability_descriptions` 调用次数 ≤ 1（证明 C_1_8）。
      - `test_payoff_ledger_no_datarefresh`：对 `update_payoff_ledger_fields` / `detect_overdue_payoffs` / `recommend_payoff_timing` / `update_foreshadowing_status` 断言均无 `resourceType = "payoffLedger"` 事件（证明 C_1_9）。
      - `test_workflow_pending_not_enqueued`：DB 预置 `n ∈ [1, 10]` 条 `workflow_instances.status = 'Pending'`，调用 `WorkflowEngine::with_pool` + `WorkflowScheduler::start_auto_drain`，2 秒后断言 `scheduler.queue` 长度为 0（证明 C_1_10）。
    - 前端 vitest + fast-check：
      - `useSyncStore.bug.spec.ts`：对随机生成的 `DataRefresh { resourceType: 'payoffLedger' }` 事件，断言 `queryClient.invalidateQueries` 从未被以 `['payoff-ledger', ...]` 调用（证明 C_1_9 前端侧）。
      - `LlmProfileForm.bug.spec.tsx`：渲染表单 → 选择 `image` → 提交 → 断言**成功创建且无警告**（证明 C_1_11 UI 死胡同）。
    - 静态 grep 断言：
      - `test_kg_path_non_unified`：`grep -r 'app.emit("data-refresh"' src-tauri/src` 命中数 > 0（证明 C_1_12 散落路径）。
  - **测试断言**（修复后期望翻转为 PASS，详见 §Correctness Properties）：
    - PRAGMA foreign_keys 在每条连接上返回 `1`
    - `delete_story` / `delete_character` 后所有子表行数为 `0`
    - 三个 `character_relationship` CRUD IPC 已注册且每次调用发 `characterRelationships` 事件
    - 所有 KG / Payoff / Ingest 变更各自发对应事件
    - 计划模板跨重启仍被命中；能力进化周期 + 阈值触发
    - Pending / Running 工作流实例启动即入队；image Profile 被 UI/后端显式拦截
  - **运行指令（在未修复分支）**：
    - `cargo test --package storyforge --test bug_condition_v57 -- --nocapture`
    - `cd src-frontend && npm run test -- bug.spec`
  - **EXPECTED OUTCOME**: 所有上述子测试 FAIL（确认 12 项差距均可复现）。任一子测试未按预期失败，必须回到 `bugfix.md §Hypothesized Root Cause` 重新分析，禁止继续实施。
  - 记录每个子测试的 counterexample 到 `test-counterexamples.md`（放在 spec 目录下）。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 1.10, 1.11, 1.12_

- [ ] 2. Write preservation property tests (BEFORE implementing fix)
  - **Property 2: Preservation** - v5.6.4 既有行为基线
  - **IMPORTANT**: 采用"先观察、后断言"方法论：全部测试先在 **未修复** 代码上运行并 PASS，作为 v5.6.4 的基线；修复后重跑必须继续 PASS。
  - **GOAL**: 锁定 `bugfix.md §3.1–3.12` 列出的所有不变行为，防止 Step A–E 任何修复引入回归。
  - **Scoped PBT Approach**：
    - 后端 proptest（`src-tauri/src/tests/preservation_v57.rs`）：
      - `test_non_delete_writes_emit_same_events`：`proptest!` 生成 `∈ [1, 50]` 个随机 `create_story` / `update_scene` / `create_chapter` 操作，捕获事件流 → 与 v5.6.4 快照 (`snapshot/events_v5_6_4.json`) 逐条比对（`StoryCreated` / `StoryUpdated` / `SceneCreated` / `SceneUpdated` / `ChapterCreated` 集合完全相等）。
      - `test_non_kg_reads_unchanged`：随机 `list_stories` / `get_story_characters` / `get_story_chapters` 调用在相同 DB fixture 下返回字段级等价。
      - `test_non_image_profile_crud_preserved`：对 `chat` / `completion` / `embedding` 三种 `provider_type`，随机 `create` / `test_model_connection` / `update` / `delete`，断言与 v5.6.4 逐步骤等价。
      - `test_non_template_plan_path_preserved`：构造不匹配任何模板的 `trigger`，断言 `PlanExecutor` 仍走 `PlanGenerator.generate` → `record_success` 完整路径，发射事件与 v5.6.4 一致。
      - `test_evolution_silent_when_under_threshold`：`proptest!` 生成 `new_record_count ∈ [0, 4]`，推进虚拟时钟 `∈ [0, 24h]`，断言 `evolve_capability_descriptions` 被调用 0 次。
      - `test_terminal_workflow_not_enqueued`：`proptest!` 生成 `status ∈ {Completed, Failed, Cancelled}` 实例，`with_pool` 后断言 `scheduler.queue` 不包含这些 `instance_id`。
      - `test_auto_ingest_invariants`：5 分钟冷却 + 内容哈希去重 + `PENDING_VECTOR_INDEXES` 持久化行为：`proptest!` 生成 `∈ [2, 20]` 次 `auto_ingest_chapter(same_chapter_id)` 在 5 分钟内，断言只触发 1 次实际 ingest。
      - `test_scheduler_idempotency`：相同 `instance_id` 连续 `schedule_execution` `∈ [2, 10]` 次，queue 中只存在 1 份。
      - `test_heartbeat_backoff`：`retry_count ∈ [0, 5]`，断言退避时长等于 `30 * 2^retry_count` 秒。
      - `test_has_cycle_rejects_loops`：随机生成 DAG 与带环图各 `∈ [3, 15]` 节点，断言带环图被 `register_workflow` 拒绝。
      - `test_query_pipeline_degrades_gracefully`：随机 provider 不可用场景，断言返回纯 token 搜索结果且发射 `context-degraded`。
      - `test_mcp_connection_pool_reuse`：同 server_id 连续 `connect_mcp_server` `∈ [2, 5]` 次，断言 `MCP_CONNECTIONS` 条目数不变。
    - 前端 vitest + fast-check（`src-frontend/src/hooks/__tests__/useSyncStore.preservation.spec.ts`）：
      - `test_existing_sync_event_cases_preserved`：`fc.array(fc.constantFrom('stories','characters','scenes','chapters','worldBuilding','writingStyle','storyOutlines','foreshadowings','knowledgeGraph','characterRelationships','all'))` 随机序列，断言每个 case 触发的 `invalidateQueries` 列表与 v5.6.4 快照（`snapshot/invalidations_v5_6_4.json`）逐条相等。
      - `test_frontstage_accept_reject_still_calls_record_feedback`：`fc.record({accepted: fc.boolean()})` 随机，断言 `record_feedback` 被调用且返回的 `LearningPoint[]` 被消费。
    - 快照采集脚本：`scripts/capture-v5_6_4-baseline.ps1` 在未修复分支上跑 10 轮 smoke 测试，导出事件与 invalidation 列表，提交到 `src-tauri/src/tests/snapshot/` 与 `src-frontend/src/hooks/__tests__/snapshot/`。
  - **运行指令（在未修复分支）**：
    - `cargo test --package storyforge --test preservation_v57 -- --nocapture`
    - `cd src-frontend && npm run test -- preservation.spec`
  - **EXPECTED OUTCOME**: 全部 PASS（确认基线正确录制）。任一 FAIL 说明快照不代表 v5.6.4 真实行为，需重采。
  - 这些测试将在 Step A–E 每一步完成后重跑，作为回归门禁。
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 3.11, 3.12_

---

## Fix Implementation — Step A: 数据层根因（差距 1.1 / 1.2 / 1.3）

- [ ] 3. Step A — Enable FK pragma & harden cascade deletes

  - [ ] 3.1 Enable `PRAGMA foreign_keys = ON` on every pooled connection
    - 修改 `src-tauri/src/db/connection.rs`：
      - `init_db` 内部将 `SqliteConnectionManager::file(&db_path)` 替换为 `SqliteConnectionManager::file(&db_path).with_init(|c| c.execute_batch("PRAGMA foreign_keys = ON;"))`。
      - `create_test_pool` 同步改为 `SqliteConnectionManager::memory().with_init(...)`，保持与生产一致。
      - 若未来要追加 pragma（例如 `busy_timeout`），在同一 `execute_batch` 中串联以避免多次往返。
    - 不修改既有 migration 版本号；仅在 `CHANGELOG.md` 锚点记录。
    - _Bug_Condition: C_1_1 (PRAGMA foreign_keys 未开启)_
    - _Expected_Behavior: Property 1 — "对任一新连接 C，`query('PRAGMA foreign_keys') = 1`"_
    - _Preservation: §3.1–3.11 读写路径行为不变，不引入额外副作用_
    - _Requirements: 2.1_

  - [ ] 3.2 Transactional cascade cleanup for `delete_story`
    - 修改 `src-tauri/src/lib.rs::delete_story`（或 `StoryRepository::delete`）：
      - 开启 `let tx = conn.transaction()?;`。
      - 按依赖顺序执行防御性 `DELETE`：
        1. `DELETE FROM kg_relations WHERE story_id = ?1`
        2. `DELETE FROM kg_entities WHERE story_id = ?1`
        3. `DELETE FROM character_relationships WHERE story_id = ?1`
        4. `DELETE FROM foreshadowing_tracker WHERE story_id = ?1`
        5. `DELETE FROM scenes WHERE story_id = ?1`
        6. `DELETE FROM chapters WHERE story_id = ?1`
        7. `DELETE FROM world_buildings WHERE story_id = ?1`
        8. `DELETE FROM story_outlines WHERE story_id = ?1`
        9. `DELETE FROM stories WHERE id = ?1`
      - `tx.commit()?` 后发射 `StateSync::emit_story_deleted(app, story_id)` 与 `StateSync::emit_data_refresh(app, Some(story_id), "all")` 兜底全资源刷新。
    - 即使 Task 3.1 被回滚（FK pragma 关闭），此兜底也保证无孤儿。
    - _Bug_Condition: C_1_2_
    - _Expected_Behavior: 删除后 chapters/scenes/kg_entities/kg_relations/character_relationships/foreshadowing_tracker/story_outlines/world_buildings 中 `story_id = id` 的行数为 0_
    - _Preservation: 非 delete 路径不受影响；既有 `storyDeleted` 事件继续发射_
    - _Requirements: 2.2_

  - [ ] 3.3 Transactional cleanup for `delete_character`
    - 修改 `src-tauri/src/repositories/character_repository.rs::delete`（及 `lib.rs::delete_character`）：
      - `let tx = conn.transaction()?;`
      - `DELETE FROM character_relationships WHERE source_character_id = ?1 OR target_character_id = ?1`
      - `DELETE FROM canonical_character_states WHERE character_id = ?1`
      - `DELETE FROM characters WHERE id = ?1`
      - `tx.commit()?` 后沿用 `emit_character_deleted(app, character_id)` 并追加 `emit_data_refresh(Some(story_id), "characterRelationships")`。
    - _Bug_Condition: C_1_3_
    - _Expected_Behavior: 删除后 character_relationships 中与该角色相关的所有行、canonical_character_states 中 character_id 对应行均为 0_
    - _Preservation: `list_characters` / `get_character` 等读路径返回字段不变_
    - _Requirements: 2.3_

  - [ ] 3.4 Unit tests for Step A
    - 新增 `db::connection::tests::test_foreign_keys_pragma_enabled`：`create_test_pool` 取 3 条连接，均断言 `PRAGMA foreign_keys` 返回 `1`。
    - 新增 `repositories::story::tests::test_delete_story_cascades_all_related`：构造完整关联链 → `delete_story` → 断言所有子表 COUNT = 0。
    - 新增 `repositories::character::tests::test_delete_character_removes_relationships`：插入 A-B 关系 → 删除 A → 关系为空。
    - 运行 `cargo test --package storyforge -- db::connection repositories::story repositories::character`，全部 PASS。
    - _Requirements: 2.1, 2.2, 2.3_

---

## Fix Implementation — Step B: StateSync 扩展（差距 1.5 / 1.6 / 1.9 / 1.12）

- [ ] 4. Step B — Extend StateSync & useSyncStore

  - [ ] 4.1 Add emit helpers on `StateSync`
    - 修改 `src-tauri/src/state_sync/service.rs`，新增方法（内部复用 `emit_data_refresh`）：
      - `pub fn emit_knowledge_graph_updated(app: &AppHandle, story_id: &str)` → `emit_data_refresh(app, Some(story_id), "knowledgeGraph")`
      - `pub fn emit_character_relationships_updated(app: &AppHandle, story_id: &str)` → `emit_data_refresh(app, Some(story_id), "characterRelationships")`
      - `pub fn emit_payoff_ledger_updated(app: &AppHandle, story_id: &str)` → `emit_data_refresh(app, Some(story_id), "payoffLedger")`
      - `pub fn emit_ingestion_completed(app: &AppHandle, story_id: &str, entity_count: usize, relation_count: usize)` → 双重发射：`app.emit("ingestion-completed", IngestionCompletedPayload { story_id, entity_count, relation_count })` + `emit_data_refresh(app, Some(story_id), "knowledgeGraph")`。
    - 新建 `IngestionCompletedPayload` 结构体（`Serialize`），放在 `state_sync/events.rs`。
    - 保持既有 16 种 `SyncEvent` 不变，不新增枚举分支。
    - _Bug_Condition: 为 C_1_5 / C_1_6 / C_1_9 / C_1_12 提供统一出口_
    - _Expected_Behavior: 新增方法是纯增量 API，单测直接捕获 `sync-event` / `ingestion-completed`_
    - _Preservation: 既有 emit 方法签名与行为保持不变_
    - _Requirements: 2.5, 2.6, 2.9, 2.12_

  - [ ] 4.2 Wire KG mutation commands to `emit_knowledge_graph_updated`
    - 在 `src-tauri/src/commands_v3.rs` 的 `create_entity` / `update_entity` / `delete_entity` / `create_relation` / `update_relation` / `delete_relation` 成功分支（DB commit 之后）调用 `StateSync::emit_knowledge_graph_updated(&app, &story_id)`。
    - 搜索 `grep -r 'kg_entities\|kg_relations' src-tauri/src` 定位所有旁路写路径（Bootstrap / Ingest / Analysis），统一改走 `emit_knowledge_graph_updated`。
    - 删除任何散落的 `app.emit("data-refresh", ...)` KG 自由格式调用（对应 C_1_12）。
    - _Bug_Condition: C_1_5 / C_1_12_
    - _Expected_Behavior: KG 变更 100ms 内发射 `resourceType = "knowledgeGraph"` 事件_
    - _Preservation: 非 KG 写路径的事件集合保持一致_
    - _Requirements: 2.5, 2.12_

  - [ ] 4.3 Wire ingest pipeline to `emit_ingestion_completed`
    - `src-tauri/src/lib.rs::auto_ingest_chapter` 成功路径：统计本次新增 `entity_count` / `relation_count`，调用 `StateSync::emit_ingestion_completed(&app, &story_id, entity_count, relation_count)`。
    - `src-tauri/src/workflow/scheduler.rs::run_instance` 中 `NodeType::VectorIndex` 完成路径同样调用。
    - 注意：遵守既有 5 分钟冷却与哈希去重——仅在真实写入发生时发射，跳过的 ingest 不发事件。
    - _Bug_Condition: C_1_6_
    - _Expected_Behavior: Ingest 成功后 `ingestion-completed` + `knowledgeGraph` dataRefresh 均发射_
    - _Preservation: 冷却 / 去重 / `PENDING_VECTOR_INDEXES` 持久化队列行为不变_
    - _Requirements: 2.6_

  - [ ] 4.4 Wire payoff ledger mutations to `emit_payoff_ledger_updated`
    - 在 `update_payoff_ledger_fields` / `detect_overdue_payoffs` / `recommend_payoff_timing` / `update_foreshadowing_status` 成功分支调用 `StateSync::emit_payoff_ledger_updated(&app, &story_id)`。
    - _Bug_Condition: C_1_9_
    - _Expected_Behavior: Payoff 变更后发射 `resourceType = "payoffLedger"` 事件_
    - _Preservation: Foreshadowing 页面既有手动 `window dataRefreshed` 监听器继续工作_
    - _Requirements: 2.9_

  - [ ] 4.5 Frontend — add `payoffLedger` case & ingestion listener
    - `src-frontend/src/hooks/useSyncStore.ts`：
      - `KEYS` 常量追加 `payoffLedger: (storyId?: string) => storyId ? ['payoff-ledger', storyId] as const : ['payoff-ledger'] as const`。
      - `case 'dataRefresh'` → `switch (resourceType)` 新增：
        ```ts
        case 'payoffLedger':
          queryClient.invalidateQueries({ queryKey: KEYS.payoffLedger(storyId) });
          break;
        ```
      - 监听 `ingestion-completed` 事件（可内联在 `useSyncStore` 或拆 `useIngestionEvents`），收到后失效 `['knowledge-graph', storyId]` + `['foreshadowings', storyId]`。
    - 新增单测 `useSyncStore.spec.ts`：
      - 断言 `DataRefresh { resourceType: 'payoffLedger' }` 触发 `['payoff-ledger', storyId]` 失效。
      - 断言 `ingestion-completed` 触发 `['knowledge-graph', storyId]` 失效。
    - _Bug_Condition: C_1_9（前端侧）_
    - _Expected_Behavior: 对 payoffLedger 资源类型触发对应 invalidateQueries_
    - _Preservation: 既有 11 种 case（stories/…/all）invalidation 列表保持不变_
    - _Requirements: 2.9, 2.6_

  - [ ] 4.6 Unit tests for Step B
    - 新增 `state_sync::service::tests::test_emit_knowledge_graph_updated` / `test_emit_character_relationships_updated` / `test_emit_payoff_ledger_updated` / `test_emit_ingestion_completed`：mock `AppHandle` 捕获 emit，断言 resource_type 与 payload 字段。
    - 运行 `cargo test state_sync` + `cd src-frontend && npm run test -- useSyncStore`，全部 PASS。
    - _Requirements: 2.5, 2.6, 2.9, 2.12_

---

## Fix Implementation — Step C: character_relationships CRUD IPC（差距 1.4）

- [ ] 5. Step C — Expose character_relationships CRUD

  - [ ] 5.1 Add repository methods
    - 新建 `src-tauri/src/repositories/character_relationships_repository.rs`（若不存在则创建）：
      - `pub fn insert(conn: &Connection, rel: &NewCharacterRelationship) -> Result<String>`（返回新 id）。
      - `pub fn update(conn: &Connection, id: &str, relation_type: Option<&str>, description: Option<&str>) -> Result<usize>`。
      - `pub fn delete_by_id(conn: &Connection, id: &str) -> Result<usize>`。
    - 确保 SQL 单条执行，返回受影响行数；`story_id` 字段必填以便 `emit_character_relationships_updated`。
    - _Bug_Condition: C_1_4（写路径完全缺失）_
    - _Requirements: 2.4_

  - [ ] 5.2 Register three IPC commands
    - 在 `src-tauri/src/lib.rs` 追加：
      - `#[tauri::command(rename_all = "snake_case")] async fn create_character_relationship(story_id: String, source_character_id: String, target_character_id: String, relation_type: String, description: Option<String>, app: AppHandle, pool: State<'_, DbPool>) -> Result<String, String>`
      - `update_character_relationship(id: String, relation_type: Option<String>, description: Option<String>, story_id: String, app: AppHandle, pool: State<'_, DbPool>)`
      - `delete_character_relationship(id: String, story_id: String, app: AppHandle, pool: State<'_, DbPool>)`
    - 每个命令在成功分支调用 `StateSync::emit_character_relationships_updated(&app, &story_id)`。
    - 在 `invoke_handler!` 宏中追加三条命令注册（保持 `rename_all = "snake_case"` 一致性）。
    - _Bug_Condition: C_1_4_
    - _Expected_Behavior: IPC 可被前端调用且每次成功发 `characterRelationships` 事件_
    - _Preservation: 既有 `get_character_relationships` 读路径不变_
    - _Requirements: 2.4_

  - [ ] 5.3 Add frontend mutation hooks
    - 修改 `src-frontend/src/hooks/useCharacterRelationships.ts`（或新建）：
      - `useCreateCharacterRelationship()` / `useUpdateCharacterRelationship()` / `useDeleteCharacterRelationship()`，使用 `@tanstack/react-query` `useMutation`，`onSuccess` 时无需手动 invalidate（`useSyncStore case 'characterRelationships'` 会自动触发）。
      - 参数字段使用 snake_case 与后端对齐。
    - _Requirements: 2.4_

  - [ ] 5.4 Unit & integration tests for Step C
    - 新增 `character_relationships_repository::tests::{test_insert, test_update, test_delete}`。
    - 新增命令集成测试 `tests::ipc::character_relationships`：调用三个 IPC，断言 DB 行变化 + `characterRelationships` 事件发射。
    - 前端新增 `useCharacterRelationships.spec.ts`：mock `invoke`，断言 mutation 触发对应命令名与参数。
    - _Requirements: 2.4_

---

## Fix Implementation — Step D: 后台自动化闭环（差距 1.7 / 1.8 / 1.10）

- [ ] 6. Step D — Close background automation loops

  - [ ] 6.1 Migration 43 — persist `plan_templates`
    - 新建 `src-tauri/migrations/043_plan_templates.sql`（或等价入口）：
      ```sql
      CREATE TABLE IF NOT EXISTS plan_templates (
        id TEXT PRIMARY KEY,
        trigger_pattern TEXT NOT NULL,
        plan_json TEXT NOT NULL,
        success_count INTEGER NOT NULL DEFAULT 1,
        last_used_at TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_plan_templates_pattern ON plan_templates(trigger_pattern);
      ```
    - 在 `db::migrations::run_migrations` 注册版本 43。
    - _Bug_Condition: C_1_7（模板跨重启丢失）_
    - _Requirements: 2.7_

  - [ ] 6.2 Persist `PlanTemplateLibrary` through `DbPool`
    - 修改 `src-tauri/src/planner/template_learning.rs`：
      - `PlanTemplateLibrary::with_pool(pool: DbPool) -> Result<Self>`：构造时从 `plan_templates` 加载所有行到内存 `templates: Vec<PlanTemplate>`。
      - `record_success(&mut self, template: PlanTemplate)`：`INSERT OR REPLACE INTO plan_templates (id, trigger_pattern, plan_json, success_count, last_used_at, created_at) VALUES (?, ?, ?, COALESCE((SELECT success_count FROM plan_templates WHERE id = ?), 0) + 1, ?, ?)`；同时更新内存副本。
      - `find_match(&self, trigger: &str) -> Option<&PlanTemplate>`：保持内存优先匹配逻辑不变。
    - `PlanExecutor::new` 改为接受 `DbPool`，内部 `PlanTemplateLibrary::with_pool(pool)`；在 `lib.rs` 注入点同步传递。
    - _Bug_Condition: C_1_7_
    - _Expected_Behavior: 重启后 `find_match` 仍命中 session1 记录的模板_
    - _Preservation: 非命中路径仍走完整 `PlanGenerator.generate`_
    - _Requirements: 2.7_

  - [ ] 6.3 `CapabilityEvolutionScheduler` — periodic + threshold triggering
    - 新建 `src-tauri/src/capabilities/evolution_scheduler.rs`：
      - 结构体持有 `Arc<ExecutionRecordStore>` + `last_eval_record_count: AtomicUsize` + `evolution_interval: Duration`（默认 1h，可通过环境变量 `STORYFORGE_EVOLUTION_INTERVAL_SECS` 覆盖）。
      - `tokio::spawn` 主循环 `interval.tick().await`，每轮：
        - 读取当前 `record_count`；若 `record_count - last_eval_record_count >= 5`，调用 `evolve_capability_descriptions`；成功后 `last_eval_record_count.store(record_count)`。
      - 提供 `notify_new_record()`：`PlanExecutor::execute_plan` 成功路径追加记录后立即调用，若阈值达成则 spawn 一次立即评估（与主循环用 `Mutex<bool> running` 互斥去重）。
    - `lib.rs::setup`：保留启动 30 秒首次触发（既有行为），追加 `CapabilityEvolutionScheduler::start(app.handle(), store.clone())`。
    - _Bug_Condition: C_1_8_
    - _Expected_Behavior: 累积 ≥ 5 条新记录 AND 距上次评估 ≥ interval 时自动触发 ≥ 1 次_
    - _Preservation: 未累积到阈值时静默；启动 30 秒首次触发保留_
    - _Requirements: 2.8_

  - [ ] 6.4 `WorkflowEngine::with_pool` — re-enqueue pending/running instances
    - 修改 `src-tauri/src/workflow/mod.rs`：
      - 提供 `pub async fn restore_pending_instances(&self, scheduler: &WorkflowScheduler)`：遍历 `self.instances`，对 `status ∈ {Pending, Running}` 的实例调用 `scheduler.schedule_execution(instance_id)`（复用既有幂等检查）。
      - 终止态（`Completed` / `Failed` / `Cancelled`）不入队。
    - `lib.rs::setup` 时序：`WorkflowEngine::with_pool(pool)` → `WorkflowScheduler::start_auto_drain(engine.clone())` → `engine.restore_pending_instances(&scheduler).await`。
    - _Bug_Condition: C_1_10_
    - _Expected_Behavior: 重启 2 秒内，所有 Pending/Running 实例进入 queue_
    - _Preservation: 终止态实例不被误入队；`schedule_execution` 幂等性（§3.3）保留_
    - _Requirements: 2.10_

  - [ ] 6.5 Unit tests for Step D
    - `planner::template_learning::tests::test_plan_template_persists_across_restart`：session1 写入 → drop → `with_pool` 重建 → `find_match` 命中。
    - `capabilities::evolution_scheduler::tests::test_triggers_on_threshold`：虚拟时钟推进 + mock 记录，断言 `evolve_*` 被调用 ≥ 1。
    - `capabilities::evolution_scheduler::tests::test_silent_under_threshold`：< 5 条记录不触发。
    - `workflow::engine::tests::test_with_pool_reenqueues_pending`：预置 Pending + Completed 实例 → 仅 Pending 入队。
    - 全部运行 `cargo test planner capabilities workflow` 通过。
    - _Requirements: 2.7, 2.8, 2.10_

---

## Fix Implementation — Step E: P2 整洁度（差距 1.11 / 1.12 收尾）

- [ ] 7. Step E — Image-profile dead-end & KG path unification

  - [ ] 7.1 Backend — structured error for image profile
    - 修改 `src-tauri/src/config/commands.rs::test_model_connection`：当 `profile.provider_type == "image"` 时返回 `Err("unsupported_type:image".to_string())`（替代硬编码中文字符串），便于前端识别。
    - _Bug_Condition: C_1_11_
    - _Requirements: 2.11_

  - [ ] 7.2 Frontend — mark image profile experimental & block creation
    - 修改 `src-frontend/src/components/Settings/LlmProfileForm.tsx`：
      - `provider_type` 下拉的 `image` 选项 label 改为 `image (实验性，暂未实现)`，`disabled: true`（默认不可选）；如需调试可通过 `?debug=1` query 解锁。
      - 若用户仍强行提交 `image`：表单级校验拦截并 toast "此类型暂未实现，敬请期待"。
      - 监听 `test_model_connection` 返回 `unsupported_type:image` 时同样展示 toast。
    - _Bug_Condition: C_1_11_
    - _Expected_Behavior: UI 阻止创建 image Profile 或后端返回结构化错误_
    - _Preservation: chat / completion / embedding 三类流程完全不变_
    - _Requirements: 2.11_

  - [ ] 7.3 Unify all KG mutation paths through StateSync
    - `grep -n 'app.emit("data-refresh"' src-tauri/src` 对所有命中点逐一审查：
      - 合法（走 `StateSync::emit_data_refresh` 内部）→ 忽略。
      - 裸 `app.emit("data-refresh", ...)` 绕开 StateSync 的 KG 相关调用 → 替换为 `StateSync::emit_knowledge_graph_updated`。
    - 审查 Bootstrap (`bootstrap.rs`) / Analysis (`analysis_pipeline.rs`) / 手动 CRUD (`commands_v3.rs`) / Ingest (`auto_ingest_chapter` + `VectorIndex`) 四类写入点。
    - 新增冒烟测试 `tests::kg_emission_unified`：对上述四类入口触发一次，断言各自发射一次 `knowledgeGraph` 事件且零裸 `data-refresh` 调用。
    - _Bug_Condition: C_1_12_
    - _Expected_Behavior: 所有 KG 变更统一经 StateSync_
    - _Preservation: 非 KG 写路径不受影响_
    - _Requirements: 2.12_

---

## Verification (cross-Step回放)

- [ ] 8. Verify bug condition exploration test now passes (ALL 12 sub-conditions)
  - **Property 1: Expected Behavior** - 12 项差距修复后全部转为 PASS
  - **IMPORTANT**: 重跑 Task 1 的 **同一** 测试集，不新增、不改断言。
  - 运行 `cargo test --package storyforge --test bug_condition_v57 -- --nocapture`。
  - 运行 `cd src-frontend && npm run test -- bug.spec`。
  - 运行 `grep -r 'app.emit("data-refresh"' src-tauri/src | grep -v state_sync` → 期望零命中（C_1_12 最终校验）。
  - **EXPECTED OUTCOME**: 所有子测试 PASS（确认 12 项差距全部修复）。
  - 若任一仍 FAIL：回到对应 Step 重新审查，禁止修改测试。
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9, 2.10, 2.11, 2.12_

- [ ] 9. Verify preservation property tests still pass
  - **Property 2: Preservation** - v5.6.4 基线无回归
  - **IMPORTANT**: 重跑 Task 2 的 **同一** 测试集，不新增、不放宽断言。
  - 运行 `cargo test --package storyforge --test preservation_v57 -- --nocapture`。
  - 运行 `cd src-frontend && npm run test -- preservation.spec`。
  - 对照 `snapshot/events_v5_6_4.json` 与 `snapshot/invalidations_v5_6_4.json`，所有比对点逐条相等。
  - **EXPECTED OUTCOME**: 全部 PASS（确认零回归）。
  - 若任一 FAIL：定位是 Step A–E 哪一步引入回归，必须修复而非放宽断言。
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 3.11, 3.12_

- [ ] 10. Integration & E2E validation
  - [ ] 10.1 `cargo test` 全量回归（期望 ≥ 217/217，允许新增测试使总数上升，但既有 217 个必须全部 PASS）。
  - [ ] 10.2 `cd src-frontend && npm run test` 前端测试全通过。
  - [ ] 10.3 `cd src-frontend && npm run build` 构建通过、无类型错误。
  - [ ] 10.4 E2E（Playwright）：
    - `e2e/delete-story-cascade.spec.ts`：Bootstrap 创建故事 → 填充子数据 → `delete_story` → 断言幕后 Stories/Characters/KG 页面立即空态。
    - `e2e/character-relationship-crud.spec.ts`：幕后"关系"标签 → 新建/编辑/删除 → 另一窗口自动刷新。
    - `e2e/ingest-refresh.spec.ts`：保存章节 → 5 秒内 KG 页面自动出现新节点。
    - `e2e/workflow-resume.spec.ts`：运行中的 Workflow → kill + restart → 10 秒内继续执行。
    - `e2e/payoff-ledger-sync.spec.ts`：更新 payoff 状态 → Foreshadowing 页面 5 秒内反映。
    - `e2e/image-profile-blocked.spec.ts`：Settings → 选 image → 表单阻止提交并 toast 提示。
  - [ ] 10.5 手动抽查：应用重启 → 幕后故事列表、幕前编辑器、后台进度条 UX 与 v5.6.4 视觉一致。
  - _Requirements: 2.1–2.12, 3.1–3.12_

---

- [ ] 11. Checkpoint — Ensure all tests pass and documentation is synchronized
  - [ ] 11.1 `cargo check` 零错误、零新增警告。
  - [ ] 11.2 `cargo test` 全绿（原 217 + 新增 Step A–E 单测）。
  - [ ] 11.3 `cd src-frontend && npm run test` 全绿。
  - [ ] 11.4 `cd src-frontend && npm run build` 通过。
  - [ ] 11.5 `cargo tauri build` Windows 产物生成（`.exe` / `.msi` / `-setup.exe`），复制到项目根目录。
  - [ ] 11.6 按 `AGENTS.md` 的"代码更新后必做"清单同步以下文档（仅限本次修改影响到的章节）：
    - `CHANGELOG.md` — 追加 v5.7 条目，列出 12 项差距修复与 Step A–E 对应变更。
    - `README.md` — 更新版本号 / 功能列表 / 截图（若 UI 变化）。
    - `AGENTS.md` — "最近完成的功能" 追加 v5.7 摘要，更新编译状态。
    - `PROJECT_STATUS.md` / `ROADMAP.md` / `ARCHITECTURE.md` / `TESTING.md` — 版本号与状态同步。
  - [ ] 11.7 统一版本号：`Cargo.toml` / `tauri.conf.json` / `src-frontend/package.json` / Git tag → `v5.7.0`。
  - [ ] 11.8 若过程中有任何"确定性已知失败但仍需追因"的测试被暂时 `#[ignore]`，必须先移除标记后再进入 checkpoint；不允许带 ignore 发布。
  - **有疑问时主动与用户确认**：如遇"不确定是否属于保持性行为"的边界情况（例如 Bootstrap 路径是否属于"KG 手动变更"范畴），在此 checkpoint 前统一澄清。
  - _Requirements: ALL (2.1–2.12 + 3.1–3.12)_
