# 设计-实现对齐 v5.7 Bugfix Design

## Overview

本轮（v5.7）设计-实现对齐针对 `bugfix.md` 识别的 12 项差距给出统一修复方案。差距集中在三个维度：

1. **数据层正确性（P0，差距 1.1–1.5）**：SQLite 连接池未开启 `PRAGMA foreign_keys`，导致声明式 `ON DELETE CASCADE` 外键约束失效；`delete_story` / `delete_character` 不在事务内显式清理关联表；`character_relationships` 缺 CRUD IPC；`kg_entities` / `kg_relations` 变更不触发同步事件。
2. **后台自动化闭环（P1，差距 1.6–1.10）**：Ingest 成功不发射 KG 刷新事件；`PlanTemplateLibrary` 仅驻内存、重启丢失；能力进化仅启动时单次触发；`payoff_ledger` 缺 `dataRefresh` case；`WorkflowEngine::with_pool` 启动时不把恢复的 `Pending/Running` 实例重新入队。
3. **可观测性与整洁度（P2，差距 1.11–1.12）**：`image` 类型 LLM Profile 死胡同；KG 更新未统一经 `StateSync`。

修复策略遵循 bug 条件方法论：把每个差距形式化为 `isBugCondition(X)` 子条件（见下文联合公式），以 Fix Checking（对 C(X) 全体成立）与 Preservation Checking（对 ¬C(X) 全体等价）两段验证闭环。绝大多数修复是**增量式**的：新增事件、开启 pragma、补 CRUD、持久化一个既有结构体，对既有 217 个 Rust 单测和前端 Vitest 测试零破坏。

修复的总体顺序：
- **Step A（数据层根因）**：`SqliteConnectionManager::with_init` 开启 `foreign_keys`，一次性修复 1.1/1.2/1.3 的级联失效症状；同步加固 `delete_story`/`delete_character` 事务清理兜底。
- **Step B（同步事件扩展）**：`StateSync` 新增 `emit_knowledge_graph_updated` / `emit_character_relationships_updated` / `emit_payoff_ledger_updated` / `emit_ingestion_completed`；`useSyncStore` 新增 `payoffLedger` case。
- **Step C（CRUD 缺口）**：补齐 `create_character_relationship` / `update_character_relationship` / `delete_character_relationship` IPC。
- **Step D（闭环修复）**：`PlanTemplateLibrary` 持久化到新表 `plan_templates`（Migration 43）；能力进化改为周期 + 阈值触发；`WorkflowEngine::with_pool` 启动后将恢复实例重新入队。
- **Step E（P2 整洁度）**：`image` Profile UI 标注或阻止；统一 KG 更新路径经 `StateSync`。

## Glossary

- **Bug_Condition (C)**：本次 12 项差距的联合判定函数 `isBugCondition(X)`，对任一 `X` 命中其中一子条件即视为"buggy 输入"。
- **Property (P)**：期望行为，即 `expectedBehavior(result)`——详见 §Correctness Properties。
- **Preservation**：`¬C(X)` 输入（非 buggy 场景）必须保持 v5.6.4 已验证行为不变。
- **StateSync**：`src-tauri/src/state_sync/service.rs` 中的同步事件发射器，当前覆盖 16 种 `SyncEvent`；本次在此基础上扩展若干 emit 方法与 `DataRefresh` 资源类型。
- **useSyncStore**：`src-frontend/src/hooks/useSyncStore.ts` 中的前端同步监听 Hook，消费 `sync-event` 并调用 `queryClient.invalidateQueries`。
- **PlanTemplateLibrary**：`src-tauri/src/planner/template_learning.rs` 中记录成功执行计划的结构体；当前仅保存在 `Mutex<PlanTemplateLibrary>` 内存中。
- **CapabilityEvolutionEngine**：`src-tauri/src/capabilities/evolution.rs` 提供 `evolve_capability_descriptions()`，分析 `ExecutionRecord` 并 LLM 生成描述改进。
- **WorkflowEngine**：`src-tauri/src/workflow/mod.rs::with_pool` 从数据库恢复未完成工作流实例；`WorkflowScheduler::start_auto_drain` 后台 worker 消费队列。
- **r2d2_sqlite::SqliteConnectionManager**：连接池管理器；`with_init` 闭包可在每个新连接建立时执行 SQL（本次用它开启 `PRAGMA foreign_keys = ON`）。
- **IngestPipeline**：`auto_ingest_chapter` + Workflow `VectorIndex` 节点负责把章节内容转成向量 + KG 实体/关系。
- **Payoff Ledger**：`foreshadowing_tracker` 账本，跟踪伏笔 setup/payoff 时间窗口与风险信号。

## Bug Details

### Bug Condition

12 项差距按触发输入分为三类：数据层写入（1.1–1.5）、后台自动化触发（1.6–1.10）、UI / 通道统一（1.11–1.12）。联合 bug 条件如下：

**Formal Specification:**

```
FUNCTION isBugCondition(X)
  INPUT: X of type SystemOperation
  OUTPUT: boolean

  // ---- 数据层正确性（P0） ----
  C_1_1 := X = acquire_connection_from_pool()
          AND query("PRAGMA foreign_keys") on X != 1

  C_1_2 := X = delete_story(story_id)
          AND exists_row_in_any_of(
                chapters, scenes, kg_entities, kg_relations,
                character_relationships, foreshadowing_tracker,
                story_outlines, world_buildings
              ) WHERE row.story_id = story_id AFTER X

  C_1_3 := X = delete_character(character_id)
          AND exists_row_in_any_of(
                character_relationships WHERE source_character_id = character_id OR target_character_id = character_id,
                canonical_character_states WHERE character_id = character_id
              ) AFTER X

  C_1_4 := X ∈ {create_character_relationship, update_character_relationship, delete_character_relationship}
          AND (IPC_not_exposed(X) OR sync_event("characterRelationships") NOT emitted)

  C_1_5 := X ∈ {create_entity, update_entity, create_relation}
          AND sync_event("knowledgeGraph") NOT emitted within 100ms

  // ---- 后台自动化闭环（P1） ----
  C_1_6 := X = (auto_ingest_chapter(chapter_id) SUCCESS) OR (Workflow.VectorIndex COMPLETES)
          AND (ingestion_completed NOT emitted OR sync_event("knowledgeGraph") NOT emitted)

  C_1_7 := X = app_restart() after session with PlanExecutor.record_success(T)
          AND PlanTemplateLibrary_after_restart.find_match(T.trigger) = None

  C_1_8 := X = tick(uptime > 30s)
          AND new_execution_records_since_last_evolve >= 5
          AND evolve_capability_descriptions NOT called since last tick window

  C_1_9 := X ∈ {update_payoff_ledger_fields, detect_overdue_payoffs,
                recommend_payoff_timing, update_foreshadowing_status}
          AND sync_event("payoffLedger") NOT emitted
          AND useSyncStore has no 'payoffLedger' case

  C_1_10 := X = WorkflowEngine::with_pool() at startup
           AND EXISTS instance WHERE instance.status ∈ {Pending, Running}
           AND instance.id NOT IN WorkflowScheduler.queue AFTER X

  // ---- P2 可观测性与整洁度 ----
  C_1_11 := X = (user creates llm_profile WITH provider_type = "image")
          AND backend returns "图像生成模型暂未实现" AND UI allowed creation without warning

  C_1_12 := X = any KG mutation path (manual/Ingest/Bootstrap/Analysis)
          AND dispatch NOT routed through StateSync

  RETURN (C_1_1 OR C_1_2 OR C_1_3 OR C_1_4 OR C_1_5
          OR C_1_6 OR C_1_7 OR C_1_8 OR C_1_9 OR C_1_10
          OR C_1_11 OR C_1_12)
END FUNCTION
```

### Examples

具体 bug 表现（v5.6.4 可复现）：

- **1.1/1.2**：在幕后删除一部故事 → `stories` 表记录消失，但 `chapters` / `scenes` / `kg_entities` / `character_relationships` 中该 `story_id` 的行依然存在；后续创建新故事时 `list_stories` 不受影响，但 `get_story_characters(old_id)` 仍返回角色，造成幽灵数据。
- **1.3**：角色 A 与 B 存在关系 → 删除 A → `character_relationships` 仍有 `(source=A, target=B)` 行 → 幕后"关系"标签页渲染时 `character_name` 解析为 "未知"，控制台出现 "character not found" 警告。
- **1.4**：用户在"关系"标签页点击"新建关系"，前端只能调用 `getCharacterRelationships` 查询，没有 `createCharacterRelationship` IPC 可调用；即便手写 `invoke('create_character_relationship', {...})` 也会得到 `CommandNotFound`。
- **1.5**：手动调用 `create_entity` 新增一个实体 → 数据库写入成功，但打开的 KG 可视化页面（另一个窗口）不刷新，必须 F5 或切换故事才能看到新实体。
- **1.6**：章节保存触发 `auto_ingest_chapter` → LLM 抽取出 3 个新角色实体写入 `kg_entities` → 前端 `['knowledge-graph', storyId]` 缓存未失效，KG 图依然显示旧节点。
- **1.7**：用户某次输入"写一段雨中告别"，`PlanExecutor` 成功执行并记录模板 → 用户关闭应用 → 次日重启后再次输入相同内容，`find_template` 返回 `None`，LLM 重新规划（浪费 token）。
- **1.8**：应用启动 30 秒触发一次进化 → 用户连续写作 3 小时产生 20 条新 `ExecutionRecord` → `capability_registry` 的描述一直停留在启动时的版本，"越写越懂"无效。
- **1.9**：调用 `update_payoff_ledger_fields(id, {status: "resolved"})` → 数据库更新，但 Foreshadowing 页面 `['payoff-ledger', storyId]` 缓存未失效，UI 仍显示 "pending"；用户必须切换故事或刷新。
- **1.10**：应用运行时启动了一个大型 Workflow 实例 → 应用意外关闭 → 重启后 `workflow_instances` 表中 `status='Pending'` 的行被 `load_instances_from_db` 读进 `engine.instances`，但 `WorkflowScheduler.queue` 为空、`start_auto_drain` 取不到任何实例，必须用户手动点击 "启动" 按钮才能继续。
- **1.11**：用户在 Settings 新建 LLM Profile，`provider_type` 下拉选择 `image` → 保存成功 → 点击"测试连接" → 返回错误 "图像生成模型暂未实现"，但 Profile 仍在列表里占位，无法使用也无法明确区分。
- **1.12（复合）**：Bootstrap 流程给新故事创建 KG 实体 → 部分路径走 `StateSync::emit_story_created`，部分路径只写数据库不发事件，造成 KG 可视化刷新表现不一致。

## Expected Behavior

### Preservation Requirements

修复必须保留 `bugfix.md §3.1–3.12` 中所列的 v5.6.4 既有行为，核心包括：

**Unchanged Behaviors:**
- 既有 16 种 `SyncEvent`（Story/Character/Scene/Chapter/WorldBuilding/DataRefresh）继续按原语义发射，前端 `useSyncStore` 既有 case 行为不变。
- `auto_ingest_chapter` 的 5 分钟冷却期、内容哈希去重、24 小时过期清理、`PENDING_VECTOR_INDEXES` 持久化队列行为不变。
- `WorkflowScheduler::schedule_execution` 幂等检查（queue / running 去重）、拓扑并行、节点 3 次重试、300 秒超时不变。
- `record_feedback` IPC 返回 `Vec<LearningPoint>` 并异步触发 `AdaptiveLearningEngine::mine_preferences`，真实偏好挖掘链路不变。
- `HeartbeatMonitor::check_all` 指数退避 `30 * 2^retry_count` 秒与 `max_retries` 终止条件不变。
- Bootstrap 两阶段（quick / background）的进度事件、`tokio::spawn` 异步行为不变。
- 157+ `#[tauri::command]` 的 `rename_all = "snake_case"` 约定不变，前端传参字段名不变。
- `WorkflowEngine::register_workflow` 的 `has_cycle` DFS 检测与拒绝逻辑不变。
- QueryPipeline 五阶段检索在 embedding provider 不可用时 graceful 降级到 token 搜索的行为不变。
- MCP `MCP_CONNECTIONS` 全局连接池复用、连接建立/释放语义不变。
- 幕前 accept/reject → `record_feedback` → 展示 `LearningPoint[]` 的"记忆显性化"体验不变。

**Scope:**
所有不命中 §Bug Condition 任何子条件的输入必须完全不受本次修复影响。特别包括：
- 其他模型类型（chat/embedding/completion）的 LLM Profile 创建与测试流程
- 非删除类数据读写（list_stories/get_story_characters/update_scene/get_chapter 等）
- Ingest 的冷却 / 去重 / 清理内部状态机
- 既有 `dataRefresh` case（stories/characters/scenes/chapters/worldBuilding/writingStyle/storyOutlines/foreshadowings/knowledgeGraph/characterRelationships/all）对应的缓存失效路径
- 非 KG 路径的同步事件（storyCreated/chapterUpdated/sceneDeleted 等）

## Hypothesized Root Cause

Based on the bug description, the most likely issues are:

1. **SQLite Pragma 遗漏**：`r2d2_sqlite::SqliteConnectionManager::file(&db_path)` 直接用默认构造器，未使用 `with_init` 在每个新连接建立时执行 `PRAGMA foreign_keys = ON`。SQLite 默认该开关为 OFF，声明式外键 `ON DELETE CASCADE` 被静默忽略，导致 1.1/1.2/1.3 的级联删除症状。修复点集中在 `src-tauri/src/db/connection.rs::init_db` 与 `create_test_pool`。

2. **删除命令未事务化 + 未防御性清理**：`delete_story` / `delete_character` 假定级联生效（实际未生效，见根因 1），且未在事务里把应该清理的关联表显式 `DELETE`，双重失败导致孤儿记录。即便未来关闭 pragma，显式 DELETE 也应作为兜底。

3. **StateSync 覆盖不完整**：`SyncEvent` 枚举没有 KG / 关系 / Payoff / Ingest 专用事件；`StateSync` 虽有 `emit_data_refresh(resource_type)` 通道，但所有 KG / 关系 / Payoff 的写路径（`create_entity`、`update_entity`、`create_relation`、`create_character_relationship`、`update_payoff_ledger_fields`）都没调用它，导致前端缓存失效链断裂。

4. **CRUD IPC 缺失**：`character_relationships` 表在 v5.0 创世时只用作读路径（`get_character_relationships`），写路径仅在 Bootstrap 内部执行，从未对外暴露 `create` / `update` / `delete` IPC，前端手动修改关系时无可用通道。

5. **内存状态未持久化**：`PlanTemplateLibrary` 结构体只有 `templates: Vec<PlanTemplate>` 字段，`record_success` 只写 `Mutex`，应用关闭后内存释放即丢失；没有任何持久化层（表、JSON 文件、sled）。

6. **一次性任务 vs 持续反馈**：`lib.rs` 在应用启动 30 秒后用 `tauri::async_runtime::spawn` 单次触发 `evolve_capability_descriptions`，没有定时器或阈值触发器维持长期反馈环。

7. **启动恢复未入队**：`WorkflowEngine::with_pool` 的 `load_instances_from_db` 只把 DB 记录灌回 `engine.instances` HashMap，但没有遍历它们把 `Pending/Running` 状态的 `instance_id` 调用 `scheduler.schedule_execution` 或直接 `queue.push_back`。启动恢复只恢复了"存在"，没有恢复"被调度"。

8. **UI 和后端对 image-type 未达成一致**：后端早有 TODO 占位，但 UI 的 Profile 创建表单没有同步把 `image` 从选项里移除或降级为"实验性"；形成一个死胡同入口。

## Correctness Properties

Property 1: Bug Condition - Foreign Key Cascades, Atomic Deletes, and Sync Events

_For any_ `X` where the bug condition holds (`isBugCondition(X)` returns true)，修复后的系统 SHALL：

- 若 `C_1_1`：每个新建连接上 `PRAGMA foreign_keys` 查询返回 `1`；
- 若 `C_1_2`：`delete_story(id)` 返回后，`chapters / scenes / kg_entities / kg_relations / character_relationships / foreshadowing_tracker / story_outlines / world_buildings` 中 `story_id = id` 的行数均为 `0`；
- 若 `C_1_3`：`delete_character(id)` 返回后，`character_relationships` 中 `source_character_id = id OR target_character_id = id` 的行数为 `0`，且 `canonical_character_states` 中 `character_id = id` 的行数为 `0`；
- 若 `C_1_4`：暴露 `create_character_relationship` / `update_character_relationship` / `delete_character_relationship` IPC，每次成功调用发射 `sync-event` 携带 `resourceType = "characterRelationships"`；
- 若 `C_1_5` 或 `C_1_12`：`create_entity` / `update_entity` / `create_relation` 成功后发射 `sync-event` 携带 `resourceType = "knowledgeGraph"`；
- 若 `C_1_6`：`auto_ingest_chapter` 或 Workflow `VectorIndex` 成功后额外发射 `ingestion-completed` 事件（携带 `entity_count / relation_count`）与 `sync-event(knowledgeGraph)`；
- 若 `C_1_7`：`PlanExecutor` 成功路径写入的模板经重启后仍可被 `find_template(trigger)` 命中；
- 若 `C_1_8`：运行时每满足"新 `ExecutionRecord` ≥ 5 且距上次进化 ≥ interval（默认 1h）" 时自动调用 `evolve_capability_descriptions` 至少一次；
- 若 `C_1_9`：`update_payoff_ledger_fields` / `detect_overdue_payoffs` / `recommend_payoff_timing` / `update_foreshadowing_status` 成功后发射 `sync-event(payoffLedger)`，`useSyncStore` 新增 `case 'payoffLedger'` 命中 `['payoff-ledger', storyId]`；
- 若 `C_1_10`：`WorkflowEngine::with_pool` 返回后，所有 `status ∈ {Pending, Running}` 的恢复实例均进入 `WorkflowScheduler.queue`（不含终止态 Completed/Failed/Cancelled）；
- 若 `C_1_11`：用户选择 `image` 类型 Profile 时，UI 明确给出"实验性，暂未实现"标记并阻止创建，或后端返回真实基本连接测试结果。

**Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 2.9, 2.10, 2.11, 2.12**

Property 2: Preservation - Non-Bug Inputs Unchanged

_For any_ `X` where `isBugCondition(X)` returns false，修复后的系统 SHALL 产生与 v5.6.4 原始实现逐比特等价的观察结果：
- 所有 `¬C_1_1`：连接建立不引入额外副作用（除查询 `foreign_keys = ON`），既有读写查询性能不退化；
- 所有 `¬C_1_2 / ¬C_1_3`：未删除场景下 `chapters` / `characters` / `character_relationships` 的行保持完整；
- 所有非 KG / 非关系 / 非 Payoff / 非 Ingest 的写操作：发射的同步事件集合与原版本一致；
- 所有 `¬C_1_7`：非模板命中路径仍走完整 `PlanGenerator` 生成流程；
- 所有 `¬C_1_8`：无新 `ExecutionRecord` 累积时进化任务保持静默；
- 所有 `¬C_1_10`：终止态（Completed/Failed/Cancelled）实例不重新入队；
- 所有 `¬C_1_11`：非 image 类型 Profile 创建、测试、编辑、删除路径不变；
- 所有前端既有 `sync-event` case（stories/characters/scenes/chapters/worldBuilding/writingStyle/storyOutlines/foreshadowings/knowledgeGraph/characterRelationships/all）触发的 `invalidateQueries` 调用集合保持一致。

**Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 3.10, 3.11, 3.12**

## Fix Implementation

### Changes Required

Assuming our root cause analysis is correct, fix implementation is partitioned into 5 steps (A–E).

#### Step A — 数据层根因（差距 1.1 / 1.2 / 1.3）

**File**: `src-tauri/src/db/connection.rs`

**Function**: `init_db` and `create_test_pool`

**Specific Changes**:
1. **PRAGMA foreign_keys = ON**：将 `SqliteConnectionManager::file(&db_path)` 改为 `SqliteConnectionManager::file(&db_path).with_init(|c| c.execute_batch("PRAGMA foreign_keys = ON;"))`。`create_test_pool` 同样改为 `SqliteConnectionManager::memory().with_init(...)`，保证单元测试与生产环境一致。
2. **保留 busy_timeout 等既有 pragma 行为**：若 `with_init` 闭包已有其它语句（当前没有），在同一 `execute_batch` 里串联 `PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000;` 等。
3. **验证迁移不破坏级联**：Migration 43 不新增表，仅作为 CHANGELOG 锚点记录此变更（可选，或沿用既有 migration 版本号）。

**File**: `src-tauri/src/lib.rs` (`delete_story`) 与 `src-tauri/src/repositories/character_repository.rs` (`delete_character`)

**Function**: `delete_story(id)` / `delete_character(id)`

**Specific Changes**:
4. **事务化显式清理（防御性）**：`delete_story` 在开启 `conn.transaction()` 后按依赖顺序 `DELETE FROM kg_relations WHERE story_id = ?; DELETE FROM kg_entities WHERE story_id = ?; DELETE FROM character_relationships WHERE story_id = ?; DELETE FROM foreshadowing_tracker WHERE story_id = ?; DELETE FROM scenes WHERE story_id = ?; DELETE FROM chapters WHERE story_id = ?; DELETE FROM world_buildings WHERE story_id = ?; DELETE FROM story_outlines WHERE story_id = ?; DELETE FROM stories WHERE id = ?;` 然后 `commit`。即便根因 1 的 pragma 回归，也能兜底。
5. **delete_character 补关系清理**：在 `CharacterRepository::delete` 内同一事务 `DELETE FROM character_relationships WHERE source_character_id = ?1 OR target_character_id = ?1; DELETE FROM canonical_character_states WHERE character_id = ?1; DELETE FROM characters WHERE id = ?1;`。
6. **事务提交后发事件**：沿用既有 `emit_story_deleted` / `emit_character_deleted`，并额外 `emit_data_refresh(Some(story_id), "all")` 强制全资源刷新（兜底）。

#### Step B — StateSync / useSyncStore 扩展（差距 1.5 / 1.6 / 1.9 / 1.12）

**File**: `src-tauri/src/state_sync/events.rs` 与 `src-tauri/src/state_sync/service.rs`

**Function**: `SyncEvent` 枚举不需要新分支（复用 `DataRefresh`），新增便捷 emit 方法。

**Specific Changes**:
7. **新增 emit 便捷方法**：
   - `StateSync::emit_knowledge_graph_updated(app, story_id)` → 内部 `emit_data_refresh(Some(story_id), "knowledgeGraph")`
   - `StateSync::emit_character_relationships_updated(app, story_id)` → `emit_data_refresh(Some(story_id), "characterRelationships")`
   - `StateSync::emit_payoff_ledger_updated(app, story_id)` → `emit_data_refresh(Some(story_id), "payoffLedger")`
   - `StateSync::emit_ingestion_completed(app, story_id, entity_count, relation_count)` → 双重发射：`app.emit("ingestion-completed", payload)` + `emit_data_refresh(Some(story_id), "knowledgeGraph")`。
8. **替换所有 KG / 关系写路径**：
   - `kg_entities` 命令（`commands_v3.rs::create_entity` / `update_entity`）、`kg_relations` 命令（`create_relation`）调用 `emit_knowledge_graph_updated`。
   - `character_relationships` 命令调用 `emit_character_relationships_updated`。
   - `foreshadowing / payoff_ledger` 命令调用 `emit_payoff_ledger_updated`。
   - `auto_ingest_chapter` / `WorkflowScheduler::run_instance` 中的 VectorIndex 节点完成路径调用 `emit_ingestion_completed`。
9. **禁止旁路**：搜索并替换任何绕开 `StateSync` 的裸 `app.emit("data-refresh", ...)` KG 事件。

**File**: `src-frontend/src/hooks/useSyncStore.ts`

**Specific Changes**:
10. **新增 `payoffLedger` case**：在 `case 'dataRefresh'` → `switch (resourceType)` 中新增 `case 'payoffLedger'`：调用 `queryClient.invalidateQueries({ queryKey: KEYS.payoffLedger(storyId) })`，并在 `KEYS` 常量定义 `payoffLedger: (storyId?) => storyId ? ['payoff-ledger', storyId] : ['payoff-ledger']`。
11. **`ingestion-completed` 可选监听**：在 `useSyncStore` 或独立 Hook `useIngestionEvents` 内监听 `ingestion-completed` 事件，失效 `['knowledge-graph', storyId]` 与 `['foreshadowings', storyId]` 缓存。

#### Step C — character_relationships CRUD IPC（差距 1.4）

**File**: `src-tauri/src/repositories/character_relationships_repository.rs`（新建或扩展）与 `src-tauri/src/lib.rs`

**Function**: 新增 3 个 `#[tauri::command(rename_all = "snake_case")]`：
- `create_character_relationship(story_id, source_character_id, target_character_id, relation_type, description, app)`
- `update_character_relationship(id, relation_type, description, app)`
- `delete_character_relationship(id, story_id, app)`

**Specific Changes**:
12. **实现 Repository 方法**：`insert` / `update` / `delete_by_id`，均在单条 SQL 内完成；返回受影响行数。
13. **注册 IPC**：在 `lib.rs::invoke_handler!` 追加三条命令，确保 `rename_all = "snake_case"` 一致。
14. **成功后发事件**：每次成功调用 `StateSync::emit_character_relationships_updated(app, story_id)`，前端既有 `case 'characterRelationships'` 会自动失效缓存。
15. **前端 Hook 补齐**：`useCharacterRelationships.ts` 新增 `useCreateCharacterRelationship` / `useUpdateCharacterRelationship` / `useDeleteCharacterRelationship` Mutation Hook，沿用 `@tanstack/react-query` 风格。

#### Step D — 后台自动化闭环（差距 1.7 / 1.8 / 1.10）

**File**: `src-tauri/src/planner/template_learning.rs` 与 `src-tauri/src/db/connection.rs`

**Function**: `PlanTemplateLibrary::new / record_success / find_match`

**Specific Changes**:
16. **Migration 43 新增 `plan_templates` 表**：
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
17. **`PlanTemplateLibrary::with_pool(pool)`**：构造时从 `plan_templates` 加载已有模板到 `templates: Vec<PlanTemplate>`；`record_success` 同时 `INSERT OR REPLACE`（以 trigger 为键）并 `success_count += 1`；`find_match` 优先内存命中，miss 时查表补齐。
18. **`PlanExecutor::new(app_handle)` 改用 `with_pool(pool)`**：在 `lib.rs` 注入时传递 `DbPool`。

**File**: `src-tauri/src/lib.rs` 与 `src-tauri/src/capabilities/evolution.rs`

**Function**: 启动时一次性 `evolve_capability_descriptions` 改为"周期 + 阈值"。

**Specific Changes**:
19. **新增 `CapabilityEvolutionScheduler`**：独立结构体，`tokio::spawn` 循环 `interval = 1h`（可配置），每轮检查 `ExecutionRecordStore.get_statistics()`；若距上次评估的新增记录数 `>= 5`，调用 `evolve_capability_descriptions`，记录 `last_eval_record_count` 到内存（或 SQLite `kv` 表）。
20. **阈值立即触发**：`PlanExecutor::execute_plan` 成功路径追加记录后也检查阈值，满足时立即 spawn 一次（与 scheduler 互斥锁去重）。
21. **保留启动后 30 秒首次触发**：保持既有语义，仅追加长期循环。

**File**: `src-tauri/src/workflow/mod.rs` 与 `src-tauri/src/workflow/scheduler.rs`

**Function**: `WorkflowEngine::with_pool`

**Specific Changes**:
22. **启动恢复后重新入队**：在 `load_instances_from_db` 返回后，遍历 `self.instances`，把 `status ∈ {Pending, Running}` 的 `instance_id` 调用 `scheduler.schedule_execution(id)`（复用幂等检查）；终止态实例不入队。
23. **签名扩展**：`with_pool(pool, Option<Arc<WorkflowScheduler>>)` 或暴露 `restore_pending_instances(&self, scheduler)` 方法在 `lib.rs::setup` 中显式调用，时序：`with_pool` → `start_auto_drain` → `restore_pending_instances`。

#### Step E — P2 整洁度（差距 1.11 / 1.12）

**File**: `src-tauri/src/config/commands.rs` 与 `src-frontend/src/components/Settings/LlmProfileForm.tsx`

**Specific Changes**:
24. **UI 降级 image Profile**：在 `provider_type` 下拉选项中，给 `image` 加后缀"(实验性，暂未实现)"并默认禁用；表单提交前拦截，toast 提示"此类型暂未实现，敬请期待"。
25. **后端返回结构化错误**：`test_model_connection` 对 `image` 类型返回 `Err("unsupported_type:image")`，前端识别并展示同上 toast。
26. **KG 路径统一**：在 Bootstrap / Analysis / 手动 CRUD / Ingest 四个 KG 写入点，全部通过 `StateSync::emit_knowledge_graph_updated` 发事件，删除任何散落的 `app.emit("data-refresh", ...)` 自由格式调用。

## Testing Strategy

### Validation Approach

The testing strategy follows a two-phase approach: first, surface counterexamples that demonstrate the bug on unfixed code, then verify the fix works correctly and preserves existing behavior.

两阶段与 bug 条件的映射：
- **Phase 1（Exploratory）**：对 12 个子 bug 条件各给一个最小复现，在未修复代码上跑观测失败现象，确认根因假设。
- **Phase 2（Fix + Preservation）**：对同一集合的复现运行修复后代码断言通过；再对 ¬C(X) 覆盖的既有行为集合跑 217 Rust 单元测试 + 前端 Vitest + v5.6.4 回归套件。

### Exploratory Bug Condition Checking

**Goal**: Surface counterexamples that demonstrate the bug BEFORE implementing the fix. Confirm or refute the root cause analysis. If we refute, we will need to re-hypothesize.

**Test Plan**: 在 unfixed 分支上跑以下 12 个测试，观察失败模式是否与 `bugfix.md §1.x` 描述一致。全部应失败才能确认根因；任一未按预期失败就回到 §Hypothesized Root Cause 重分析。

**Test Cases**:
1. **FK Pragma Off Test**：创建完整关联链 story→chapters→scenes→kg_entities→character_relationships → `DELETE FROM stories WHERE id = ?` → 断言所有子表仍有对应行（will fail on unfixed code，即断言"仍有行"成立，对应 bug 存在）。
2. **delete_story Leaves Orphans Test**：调用 `delete_story(id)` → 断言 `SELECT COUNT(*) FROM kg_entities WHERE story_id = ?` > 0（will fail on unfixed code = bug 存在）。
3. **delete_character Leaves Relationships Test**：`delete_character(A)` → `SELECT COUNT(*) FROM character_relationships WHERE source_character_id = A OR target_character_id = A` > 0（will fail on unfixed code）。
4. **character_relationships IPC Missing Test**：`invoke('create_character_relationship', ...)` → 期望得到 `CommandNotFound` 或 `Err`（will fail on unfixed code，表现为命令不存在）。
5. **KG Mutation No Event Test**：订阅 `sync-event` → `create_entity(...)` → 100ms 内未收到 `resourceType = "knowledgeGraph"`（will fail on unfixed code）。
6. **Ingestion No Refresh Test**：mock LLM 返回 3 个实体 → `auto_ingest_chapter` → 订阅 `ingestion-completed` 与 `sync-event(knowledgeGraph)` → 收到 0 条（will fail on unfixed code）。
7. **Template Lost After Restart Test**：session1: `record_success(T)` → 重建 `PlanExecutor`（模拟重启）→ `find_template(T.trigger) = None`（will fail on unfixed code）。
8. **Capability Evolution Stuck Test**：spawn 应用运行 2h，期间写入 10 条 `ExecutionRecord` → 检查 `evolve_capability_descriptions` 调用次数 = 1（will fail on unfixed code = 只在启动时触发一次）。
9. **payoff_ledger No DataRefresh Test**：订阅 `sync-event` → `update_payoff_ledger_fields(...)` → 无 `resourceType = "payoffLedger"` 事件（will fail on unfixed code）。
10. **Workflow Pending Stays Test**：DB 预置 `workflow_instances.status = 'Pending'` → 启动应用 → 2 秒后检查 `WorkflowScheduler.queue` 长度 = 0（will fail on unfixed code）。
11. **Image Profile Dead End Test**：UI 允许创建 `image` Profile → 连接测试返回硬编码错误 → UI 无降级标识（will fail on unfixed code = UI 无标识）。
12. **KG Update Path Non-Unified Test**：grep `app.emit("data-refresh"` → 存在绕开 `StateSync` 的调用点（will fail on unfixed code = 找到散落点 > 0）。

**Expected Counterexamples**:
- 1/2/3：`SELECT COUNT(*)` 返回非零，证实孤儿数据
- 4：命令注册表不含 `create_character_relationship`
- 5/6/9/12：事件订阅器捕获数组长度为 0
- 7：`find_match` 返回 `None`
- 8：`spy.call_count == 1`
- 10：queue 为空
- 11：UI 渲染无警告文案
- Possible causes: FK pragma 未开启、事件 emit 遗漏、CRUD 未暴露、内存状态未持久化、调度器未恢复

### Fix Checking

**Goal**: Verify that for all inputs where the bug condition holds, the fixed function produces the expected behavior.

**Pseudocode:**

```
FOR ALL X WHERE isBugCondition(X) DO
  CASE X OF
    WHEN X = acquire_connection():
      ASSERT query(X, "PRAGMA foreign_keys") = 1
    WHEN X = delete_story(id):
      ASSERT row_count_in_related_tables(id) = 0
    WHEN X = delete_character(id):
      ASSERT relationship_rows_of(id) = 0
    WHEN X ∈ KG_mutations:
      ASSERT sync_event("knowledgeGraph") emitted within 100ms
    WHEN X ∈ relationship_mutations:
      ASSERT sync_event("characterRelationships") emitted
    WHEN X ∈ payoff_mutations:
      ASSERT sync_event("payoffLedger") emitted
    WHEN X = ingest_completion:
      ASSERT ingestion-completed AND sync_event("knowledgeGraph") emitted
    WHEN X = restart_with_recorded_template:
      ASSERT find_template(T.trigger) = Some(T)
    WHEN X = periodic_tick AND new_records >= 5:
      ASSERT evolve_capability_descriptions called
    WHEN X = startup_with_pending_instances:
      ASSERT queue contains all pending instance ids
    WHEN X = image_profile_creation:
      ASSERT UI blocks OR backend returns structured error
  END CASE
END FOR
```

### Preservation Checking

**Goal**: Verify that for all inputs where the bug condition does NOT hold, the fixed function produces the same result as the original function.

**Pseudocode:**

```
FOR ALL X WHERE NOT isBugCondition(X) DO
  ASSERT F_v5_6_4(X) = F_v5_7(X)
END FOR
```

**Testing Approach**: Property-based testing is recommended for preservation checking because:
- It generates many test cases automatically across the input domain
- It catches edge cases that manual unit tests might miss
- It provides strong guarantees that behavior is unchanged for all non-buggy inputs

**Test Plan**: 先在 unfixed v5.6.4 分支上采集"非 bug 场景"观测值（数据库快照、事件流、缓存键集合），再在 fixed 分支上对同一场景比对。

**Test Cases**:
1. **Non-delete Write Preservation**：`create_story` / `update_scene` / `create_chapter` 观察既有 `sync-event` 集合与 v5.6.4 完全一致（无新增、无遗漏、字段名不变）。
2. **Non-KG Read Preservation**：`list_stories` / `get_story_characters` / `get_story_chapters` 在修复前后返回结构完全等价。
3. **Non-image Profile Preservation**：chat / completion / embedding 三类 LLM Profile 的创建、测试、编辑、删除流程与 v5.6.4 一致。
4. **Non-template Plan Preservation**：`PlanExecutor` 在没有匹配模板时仍走完整 `PlanGenerator.generate` → `record_success` 路径，各阶段事件不变。
5. **Non-overflow Evolution Preservation**：运行时未累积 5 条新记录时，周期性 scheduler 静默，不 spawn LLM 调用。
6. **Non-pending Workflow Preservation**：`status ∈ {Completed, Failed, Cancelled}` 的实例不入队、不影响 `scheduler.queue`。
7. **Existing useSyncStore case Preservation**：既有 `case 'stories'|'characters'|'scenes'|'chapters'|'worldBuilding'|'writingStyle'|'storyOutlines'|'foreshadowings'|'knowledgeGraph'|'characterRelationships'|'all'` 触发的 `invalidateQueries` 调用列表与 v5.6.4 一致。
8. **217 Rust Tests Preservation**：`cargo test` 217/217 全通过。
9. **Vitest Preservation**：`npm run test` 前端测试全通过，快照无变化。

### Unit Tests

- **db::connection**：新增 `test_foreign_keys_pragma_enabled`（连接 × 3 皆返回 1）。
- **repositories::story**：新增 `test_delete_story_cascades_all_related`（构造完整树、删除、断言全部子表 count = 0）。
- **repositories::character**：新增 `test_delete_character_removes_relationships`（A-B 关系 → 删 A → 关系为空）。
- **state_sync::service**：新增 `test_emit_knowledge_graph_updated` / `test_emit_character_relationships_updated` / `test_emit_payoff_ledger_updated` / `test_emit_ingestion_completed`。
- **character_relationships Repository**：新增 `create / update / delete` CRUD 三连测试。
- **planner::template_learning**：新增 `test_plan_template_persists_across_restart`（用 `create_test_pool` + `with_pool` 模拟重启）。
- **workflow::engine**：新增 `test_with_pool_reenqueues_pending_instances`（预置 Pending 实例 → `with_pool` + `restore` → queue 非空）。

### Property-Based Tests

- **FK 级联不变量**：随机生成含 0–20 章节 / 0–30 场景 / 0–50 实体的 `Story`，执行 `delete_story` → 断言子表全空。
- **事件发射不变量**：对 100 个随机 KG 变更操作，断言 `sync-event(knowledgeGraph)` 一一对应发射。
- **非 bug 输入保持不变量**：对 1000 个随机 `update_scene` / `create_chapter` / `update_story` 操作，对比 v5.6.4 vs v5.7 的事件序列逐项相等。
- **模板持久化不变量**：随机 100 条 trigger-plan pair，写入 → 重启 → 每条均可命中。

### Integration Tests

- **Full delete flow**：Bootstrap 创建故事 → 生成完整子数据（chapters/scenes/characters/KG/relations/foreshadowings）→ `delete_story` → 查询全部子表 count = 0，且前端 `sync-event(all)` 触发 `['stories']` 失效。
- **character_relationships CRUD E2E**：前端 Hook `useCreateCharacterRelationship` → 后端 IPC → DB 写入 → `sync-event(characterRelationships)` → 另一窗口 `useCharacterRelationships` 自动刷新。
- **Ingest → KG 可视化刷新 E2E**：保存章节 → `auto_ingest_chapter` → 3 实体写入 → KG 页面 React Query 5 秒内自动刷新渲染新节点。
- **Template 持久化 E2E**：执行一次带模板记录的计划 → 关闭 + 重启应用 → 同输入重跑 → `PlanExecutor` 日志命中 "using cached template"。
- **Workflow 恢复 E2E**：运行中的大型 Workflow 实例 → kill 进程 → 重启 → 实例无需手动触发、在 2–10 秒内继续执行。
- **Payoff Ledger 刷新 E2E**：Foreshadowing 页面打开 → 从另一命令更新 payoff 状态 → 页面 5 秒内自动反映新状态。
- **Image Profile UI 死胡同修复**：Settings → 新建 Profile → `image` 下拉被禁用或显示"实验性，暂未实现"。
