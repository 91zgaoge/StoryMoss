# Bugfix Requirements Document — 设计-实现对齐 v5.7

## Introduction

在 v5.1→v5.6.4 六轮设计-实现对齐的基础上，本轮（v5.7）对项目全栈进行新一轮审计，聚焦以下设计目标：

1. **幕前 ↔ 幕后自动关联**：任何数据变更应在另一侧零延迟、零刷新对齐
2. **后台自动化闭环**：IngestPipeline、向量索引、知识图谱、伏笔追踪、能力进化、计划模板学习、工作流引擎应形成自维持的反馈环
3. **设计文档与实现对齐**：ARCHITECTURE.md / ROADMAP.md / FEATURES.md / PROJECT_STATUS.md 声明的能力应有可运行实现
4. **IPC 与数据层正确性**：删除级联、外键约束、事件覆盖、缓存失效

本轮审计扫描了 `src-tauri/src` 37 个模块（含 `state_sync`、`workflow`、`planner`、`capabilities`、`memory`、`creative_engine`、`mcp`、`task_system`、`narrative`、`db`）以及 `src-frontend/src` 的 Hook / 同步层，识别出 12 项设计-实现差距。每项差距按 bug 条件方法论记录当前（错误）行为、期望（正确）行为和必须保持不变的既有行为。

优先级规则：
- **P0**：运行时正确性问题，可能导致数据丢失、安全问题或核心流程失效
- **P1**：显性功能缺失或断链，影响用户体验和设计承诺
- **P2**：可观测性、代码整洁度、性能优化

---

## Bug Analysis

### Current Behavior (Defect)

**P0 — 核心正确性问题**

1.1 WHEN SQLite 连接从 r2d2 连接池获取 THEN the system 未执行 `PRAGMA foreign_keys = ON`，导致 `FOREIGN KEY ... ON DELETE CASCADE` 外键约束在实际运行时被忽略；删除 story 后 chapters / characters / scenes / kg_entities / kg_relations / foreshadowing_tracker / world_buildings / story_outlines 等 20+ 张表的关联记录变成孤儿数据

1.2 WHEN 用户在幕后调用 `delete_story(id)` THEN the system 仅执行 `DELETE FROM stories WHERE id = ?1` 而不在事务中显式清理关联表，依赖未开启的外键级联，造成残留数据污染后续查询

1.3 WHEN 用户在幕后调用 `delete_character(id)` THEN the system 仅删除 characters 记录，未清理 `character_relationships`（作为 source_character_id 或 target_character_id 的行）、未清理 `canonical_character_states` 等关联表，导致 KG 查询返回悬空引用

1.4 WHEN 幕后的"关系"标签页或图谱页面通过 `create_character_relationship` / `update_character_relationship` / `delete_character_relationship` 修改角色关系 THEN the system 既未提供这些 IPC 命令（仅有 `get_character_relationships`），也未发射 `characterRelationshipsUpdated` 同步事件；前端仅能通过 `Characters.tsx` 手动 `invalidateQueries` 局部刷新，幕前和其他页面看不到更新

1.5 WHEN 幕后或 planner 通过 `create_entity` / `update_entity` / `create_relation` 修改知识图谱 THEN the system 不发射任何同步事件（`StateSync` 没有 `emit_knowledge_graph_updated`），导致已打开的 KG 可视化页面、实体关系列表、幕前写作时的 Canonical State 缓存不会刷新，必须手动切换页面或重启

**P1 — 设计承诺的自动化闭环未闭合**

1.6 WHEN `auto_ingest_chapter` 或 Workflow `VectorIndex` 节点完成 Ingest 并向 kg_entities 写入新实体 THEN the system 不发射 `ingestion-completed` 或 `knowledge-graph-updated` 事件；`useSyncStore` 也没有 `ingestionCompleted` case，前端的 `['knowledge-graph', storyId]`、`['foreshadowings', storyId]` 缓存不会自动失效

1.7 WHEN `PlanExecutor::execute_plan` 在成功路径上调用 `template_library.record_success` THEN the system 仅写入内存中的 `Mutex<PlanTemplateLibrary>`（`PlanTemplateLibrary` 结构体只有 `templates: Vec<PlanTemplate>` 无持久化），应用重启后所有已学习模板丢失；设计文档声称"PlanTemplateLibrary（计划模板学习）：记录成功执行计划，类似请求复用或微调"但无法跨会话生效

1.8 WHEN 用户首次启动应用或故事数据库中已存在 >5 条能力执行记录 THEN the system 仅在启动 30 秒后触发一次 `evolve_capability_descriptions`（`lib.rs` 中 `tauri::async_runtime::spawn` 一次性任务），此后除非用户手动调用 IPC，否则能力描述不会持续进化；设计文档声称"能力进化反馈环闭合 + PlanExecutor 后台触发"但实际是启动时单次触发而非持续反馈

1.9 WHEN 后端数据修改命令（`update_story_outline` / `get_payoff_ledger` 更新 / `update_foreshadowing_status` 等）完成 THEN the system 发射 `dataRefresh` 事件但 `useSyncStore` 的 `case 'dataRefresh'` 分支未覆盖 `payoff-ledger` 资源类型，前端 `['payoff-ledger', storyId]` 缓存不会因 `dataRefresh` 自动失效（只有 Foreshadowing 页面的手动 window `dataRefreshed` 监听器会触发）

1.10 WHEN `WorkflowEngine::with_pool` 从数据库恢复未完成实例 THEN the system 仅在 `setup()` 时调用一次 `load_instances_from_db`，但不会自动将 `Pending` 状态的实例重新入队到 `WorkflowScheduler`；应用重启后恢复的实例停留在 `workflow_instances` 表但无任何线程推进它们，必须手动调用 `start_workflow_instance` 才能继续

**P2 — 可观测性与代码整洁**

1.11 WHEN 幕后设置中存在 `image` 类型的 LLM Profile THEN the system 在 `config::commands::test_model_connection` 或类似调用路径上返回 `"图像生成模型暂未实现"` 硬编码错误（`config/commands.rs:422` 注释 `// TODO: 实现图像生成模型`），但 UI 仍允许用户创建此类配置，形成死胡同

1.12 WHEN `IngestPipeline` 已将新实体/关系写入 kg_entities 和 kg_relations 表 THEN the system 不通过 `StateSync` 发射任何以 `story_id` 为目标的资源刷新事件；`useSyncStore.KEYS.knowledgeGraph` 虽在前端定义，但后端没有任何路径触发它失效（除 `storySelected` / `dataRefresh:all`），导致"越写越懂"的增量更新在前端不可见

### Expected Behavior (Correct)

**P0 — 核心正确性**

2.1 WHEN SQLite 连接从 r2d2 连接池获取 THEN the system SHALL 通过 `r2d2_sqlite::SqliteConnectionManager::with_init` 或等价机制，在每个连接建立时执行 `PRAGMA foreign_keys = ON`，使所有 `FOREIGN KEY ... ON DELETE CASCADE` 约束生效

2.2 WHEN 用户调用 `delete_story(id)` THEN the system SHALL 在事务中按依赖顺序显式清理或依赖已开启的外键级联删除 chapters / characters / scenes / world_buildings / kg_entities / kg_relations / foreshadowing_tracker / story_outlines / character_relationships / narrative_* 等关联表，并在事务提交后发射 `storyDeleted` + `dataRefresh(all)` 同步事件

2.3 WHEN 用户调用 `delete_character(id)` THEN the system SHALL 在事务中同时删除以该角色为 source 或 target 的 character_relationships 行、清理 canonical_character_states、从 scenes.characters_present JSON 中移除该 ID（或通过外键级联完成同等清理），最后发射 `characterDeleted` 事件

2.4 WHEN 前端修改角色关系 THEN the system SHALL 暴露 `create_character_relationship` / `update_character_relationship` / `delete_character_relationship` 三个 IPC 命令，每次成功修改后发射 `dataRefresh(story_id, "characterRelationships")` 事件，`useSyncStore` 既有的 `characterRelationships` 分支将自动失效 `['character-relationships', storyId]` 缓存

2.5 WHEN `create_entity` / `update_entity` / `create_relation` IPC 完成 THEN the system SHALL 通过 `StateSync::emit_data_refresh(story_id, "knowledgeGraph")` 发射同步事件，`useSyncStore` 的 `case 'knowledgeGraph'` 既有分支会刷新 `['knowledge-graph', storyId]` 缓存

**P1 — 自动化闭环**

2.6 WHEN `auto_ingest_chapter` 或 Workflow `VectorIndex` 节点成功将实体/关系写入 kg_entities THEN the system SHALL 发射 `dataRefresh(story_id, "knowledgeGraph")` 同步事件以及 `ingestion-completed` 事件（携带 entity_count / relation_count），前端 KG 可视化、伏笔看板、Canonical State 可自动刷新

2.7 WHEN `PlanExecutor` 记录成功计划到 `PlanTemplateLibrary` THEN the system SHALL 将模板持久化到 SQLite（新表 `plan_templates` 或 JSON 文件），应用重启时 `PlanExecutor::new` 从持久化存储加载已有模板，`find_template` 在跨会话下仍能命中

2.8 WHEN 应用正常运行期间累积 >=5 条新的 `ExecutionRecord` THEN the system SHALL 通过定时任务（例如每小时检查一次 + 阈值触发）或基于记录计数的触发器自动调用 `evolve_capability_descriptions`，而非仅在启动后 30 秒触发一次

2.9 WHEN `update_story_outline` / `update_payoff_ledger_fields` / `detect_overdue_payoffs` / `recommend_payoff_timing` 等命令修改伏笔账本 THEN the system SHALL 发射 `dataRefresh(story_id, "payoffLedger")`，`useSyncStore` SHALL 新增 `case 'payoffLedger'` 分支失效 `['payoff-ledger', storyId]` 缓存

2.10 WHEN `WorkflowEngine::with_pool` 启动时从数据库恢复状态为 `Pending` 或 `Running` 的工作流实例 THEN the system SHALL 将它们重新入队到 `WorkflowScheduler.queue`，由 `start_auto_drain` worker 在后台继续执行；已完成到终止状态（Completed / Failed / Cancelled）的实例不入队

**P2 — 可观测性与整洁度**

2.11 WHEN 用户尝试测试或使用 `image` 类型的 LLM Profile THEN the system SHALL 或（a）实现基本的图像生成连接测试（通过 OpenAI / Stable Diffusion 兼容端点），或（b）在 UI 配置列表明确标记 `image` 为"实验性，暂未实现"，阻止用户创建 image 类型 Profile

2.12 WHEN 任何路径完成 KG 更新（手动 / Ingest / Bootstrap / 拆书） THEN the system SHALL 统一经由 `StateSync` 发射事件，`useSyncStore` 消费这些事件并调用 `queryClient.invalidateQueries` 使所有依赖 KG 的查询自动重取

### Unchanged Behavior (Regression Prevention)

下列 v5.6.4 已验证可工作的行为必须保持不变：

3.1 WHEN 用户在幕后创建/更新/删除 Story / Character / Chapter / Scene / WorldBuilding / Foreshadowing / Outline THEN the system SHALL CONTINUE TO 发射对应的 `StoryCreated` / `StoryUpdated` / `StoryDeleted` / `CharacterCreated` / `CharacterUpdated` / `CharacterDeleted` / `ChapterCreated` / `ChapterUpdated` / `ChapterDeleted` / `SceneCreated` / `SceneUpdated` / `SceneDeleted` / `WorldBuildingUpdated` / `dataRefresh(foreshadowings)` / `dataRefresh(storyOutlines)` 同步事件

3.2 WHEN 用户从幕前保存章节内容 THEN the system SHALL CONTINUE TO 异步触发 `auto_ingest_chapter`，尊重 5 分钟冷却期 + 内容哈希去重 + 24 小时过期清理，并在向量存储初始化前将请求加入 `PENDING_VECTOR_INDEXES` 持久化队列

3.3 WHEN `WorkflowScheduler::schedule_execution` 被调用 THEN the system SHALL CONTINUE TO 执行幂等检查（若实例已在队列或正在运行则跳过）、按拓扑顺序并行执行同层节点、对失败节点最多重试 3 次、单节点执行 timeout 300 秒

3.4 WHEN `record_feedback` IPC 被前端调用 THEN the system SHALL CONTINUE TO 返回真实的 `Vec<LearningPoint>`（而非 mock），同步调用 `PreferenceMiner::mine` 并异步触发 `AdaptiveLearningEngine::mine_preferences` 持久化偏好

3.5 WHEN `HeartbeatMonitor::check_all` 检测到任务心跳超时且 retry_count < max_retries THEN the system SHALL CONTINUE TO 按指数退避 `30 * 2^retry_count` 秒重新调度任务，超过 max_retries 才标记为 Failed

3.6 WHEN 前端监听到 `sync-event` THEN the system SHALL CONTINUE TO 对 story / character / scene / chapter 的 created/updated/deleted 事件，以及 storySelected / worldBuildingUpdated / dataRefresh(stories|characters|scenes|chapters|worldBuilding|writingStyle|storyOutlines|foreshadowings|knowledgeGraph|characterRelationships|all) 完整覆盖对应的 `invalidateQueries` 调用，保持 v5.6.1 / v5.6.2 的缓存对称失效

3.7 WHEN Bootstrap 两阶段执行 THEN the system SHALL CONTINUE TO 在 quick phase 完成后立即返回第一章正文到前端，background phase（世界观/角色/场景/伏笔/KG）在 `tokio::spawn` 中异步完成并发射 `PipelineProgressEvent` + `data-refresh`

3.8 WHEN 所有 157+ `#[tauri::command]` 被前端调用 THEN the system SHALL CONTINUE TO 使用 `rename_all = "snake_case"` 参数约定，前端传入 `story_id` / `scene_id` 等 snake_case 字段名正确反序列化

3.9 WHEN `WorkflowEngine::register_workflow` 注册新工作流 THEN the system SHALL CONTINUE TO 通过 `has_cycle` DFS 检测循环依赖，拒绝含环的定义

3.10 WHEN QueryPipeline 五阶段检索（token_search + semantic_search + fuse_results + graph_expansion + budget_control + assemble_context）运行 THEN the system SHALL CONTINUE TO 在 Ollama / OpenAI embedding provider 不可用时 graceful 降级到纯 token 搜索，发射 `context-degraded` 事件通知前端

3.11 WHEN MCP 客户端通过 `connect_mcp_server` 建立连接 THEN the system SHALL CONTINUE TO 将 `McpClient` 缓存到全局 `MCP_CONNECTIONS: TokioMutex<HashMap<String, McpClient>>`，`call_mcp_tool` / `disconnect_mcp_server` / `get_mcp_connections` 复用同一连接，避免每次调用重新握手

3.12 WHEN 幕前用户 accept / reject AI 续写 THEN the system SHALL CONTINUE TO 调用 `record_feedback` 并展示返回的 `LearningPoint[]`（"系统学到了什么"），保持 v5.6.1 "记忆显性化"体验

---

## 推导的 Bug 条件与验证伪代码

### Bug Condition 总公式

```pascal
FUNCTION isBugCondition(X)
  INPUT: X of type SystemOperation
  OUTPUT: boolean

  RETURN (
    X matches any of:
      (1.1) new SQLite connection from r2d2 pool
      (1.2) delete_story(story_id)
      (1.3) delete_character(character_id)
      (1.4) mutate character_relationships
      (1.5) mutate kg_entities or kg_relations
      (1.6) auto_ingest_chapter success OR Workflow.VectorIndex completes
      (1.7) PlanExecutor.execute_plan success
      (1.8) app runtime > 30s AND new execution_records >= 5
      (1.9) mutate payoff_ledger
      (1.10) app startup AND workflow_instances table has Pending/Running rows
      (1.11) user configures or tests image-type LLM profile
      (1.12) any KG mutation path
  )
END FUNCTION
```

### 主要属性（Fix Checking）

```pascal
// Property 1: 外键级联生效（对应 1.1 / 1.2 / 1.3）
FOR ALL connections C from r2d2 pool DO
  ASSERT "PRAGMA foreign_keys" returns 1 on C
END FOR
FOR ALL X WHERE X = delete_story(id) DO
  post_delete_count("chapters WHERE story_id = id") = 0
  post_delete_count("scenes WHERE story_id = id") = 0
  post_delete_count("kg_entities WHERE story_id = id") = 0
  post_delete_count("character_relationships WHERE story_id = id") = 0
END FOR

// Property 2: 关系/图谱变更触发同步事件（对应 1.4 / 1.5 / 1.12）
FOR ALL X WHERE X mutates character_relationships OR kg_entities OR kg_relations DO
  ASSERT "sync-event" with resourceType ∈ {"characterRelationships", "knowledgeGraph"} emitted within 100ms
END FOR

// Property 3: Ingest 完成触发 KG 刷新（对应 1.6）
FOR ALL X WHERE X = auto_ingest_chapter(chapter_id) AND success DO
  ASSERT "ingestion-completed" emitted
  ASSERT "sync-event:dataRefresh(knowledgeGraph)" emitted
END FOR

// Property 4: Plan 模板跨会话持久化（对应 1.7）
given library L with template T recorded in session 1
after app restart, library L' loaded from disk in session 2
ASSERT L'.find_match(T.trigger_patterns[0]) returns Some(T)

// Property 5: 能力进化持续触发（对应 1.8）
FOR ALL X WHERE app uptime mod evolution_interval = 0 AND new_records >= 5 since last eval DO
  ASSERT evolve_capability_descriptions called
END FOR

// Property 6: Payoff Ledger 缓存失效（对应 1.9）
FOR ALL X WHERE X mutates payoff_ledger DO
  ASSERT sync-event "payoffLedger" emitted
  ASSERT frontend ['payoff-ledger', storyId] query marked stale
END FOR

// Property 7: Workflow 实例启动恢复（对应 1.10）
at setup() end:
  recovered = WorkflowEngine.instances.values().filter(status ∈ {Pending, Running})
  ASSERT all recovered instances enqueued in WorkflowScheduler.queue
```

### 保持不变性（Preservation Checking）

```pascal
FOR ALL X WHERE NOT isBugCondition(X) DO
  ASSERT F(X) = F'(X)
END FOR
```

即：v5.6.4 所有 217 个 Rust 单元测试、前端 Vitest 测试、3.1 至 3.12 列出的既有行为在修复后必须全部通过且表现一致。

### 验证入口

| 差距编号 | 验证方式 |
|---------|---------|
| 1.1–1.3 | `#[cfg(test)]` 集成测试：创建完整关联链 → `delete_story` → 断言所有子表无残留 |
| 1.4 | 单元测试：调用 `create_character_relationship` → 断言 `sync-event` 被捕获；UI 手动验证"关系"标签页跨窗口联动 |
| 1.5 | 集成测试：调用 `create_entity` → 观察 `sync-event` 频道；前端 KG 页面断言 `invalidateQueries` 触发 |
| 1.6 | 集成测试：mock LLM 返回固定实体 → `auto_ingest_chapter` → 捕获 `ingestion-completed` 与 `knowledgeGraph` dataRefresh |
| 1.7 | 集成测试：模板记录 → 关闭重建 PlanExecutor → `find_template` 仍命中 |
| 1.8 | 集成测试：模拟 6 条 ExecutionRecord → 推进计时器 → 断言 evolve 被调用一次以上 |
| 1.9 | 单元测试：`update_payoff_ledger_fields` → 捕获 dataRefresh payoffLedger |
| 1.10 | 集成测试：数据库预置 Pending 实例 → 启动 `WorkflowEngine::with_pool` + scheduler → 断言 queue 非空 |
| 1.11 | 手动验证：创建 image Profile → 观察 UI 阻止或基本连接测试行为 |
| 1.12 | 覆盖 1.5 / 1.6 的属性测试 |

---

## 优先级总览

| 编号 | 优先级 | 类别 | 一句话摘要 |
|------|--------|------|-----------|
| 1.1 | P0 | 数据正确性 | PRAGMA foreign_keys 未开启导致级联删除失效 |
| 1.2 | P0 | 数据正确性 | delete_story 不在事务中显式清理子表 |
| 1.3 | P0 | 数据正确性 | delete_character 不清理 character_relationships 和 canonical_character_states |
| 1.4 | P0 | 幕前幕后联动 | character_relationships 缺 CRUD IPC 和同步事件 |
| 1.5 | P0 | 幕前幕后联动 | KG 变更不发射同步事件 |
| 1.6 | P1 | 后台自动化 | Ingest 完成不触发 KG 缓存刷新 |
| 1.7 | P1 | 后台自动化 | PlanTemplateLibrary 仅在内存，重启丢失 |
| 1.8 | P1 | 后台自动化 | 能力进化仅启动时触发一次，未持续反馈 |
| 1.9 | P1 | 幕前幕后联动 | payoff-ledger 缓存 dataRefresh 分支缺失 |
| 1.10 | P1 | 后台自动化 | Workflow 实例重启后不自动重新入队 |
| 1.11 | P2 | 整洁度 | image-type LLM Profile 存在死胡同 |
| 1.12 | P2 | 可观测性 | KG 更新路径未统一经 StateSync |

批准后进入 Design 阶段：按差距顺序设计对应的修复方案（数据库迁移 43 用于 foreign_keys pragma、新增 StateSync 方法、PlanTemplateLibrary 持久化表、周期性能力进化任务、WorkflowEngine 启动恢复入队等），并在 tasks.md 拆解为可独立验证的实施步骤。
