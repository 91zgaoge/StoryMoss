# StoryForge 系统性全流程差距审计报告

> 审计日期: 2026-05-23
> 审计方法: 代码静态追踪 + 端到端数据流验证 + 接口匹配度扫描
> 审计范围: 后端 Rust (`src-tauri/src/`) + 前端 TypeScript (`src-frontend/src/`)
> 独立性声明: 本审计不受任何已有审计报告框架约束，所有结论均从当前代码实际状态直接推导

---

## 一、执行摘要

StoryForge 的代码状态呈现典型的「**基础设施过剩，流程闭合不足**」特征。后端注册了 **150+ IPC 命令**，但经扫描发现 **38 个命令无任何前端调用**（悬空率 25%）。更根本的问题是：许多子系统内部实现了复杂的引擎和调度器，但这些引擎从未被触发调用，或触发后的连锁反应在关键环节断裂。

**三大结构性病灶**:

1. **自动化飞轮断裂** — `update_scene` 有完整的 Ingest → KG 保存 → StateSync 链路，但 `update_chapter` 的 `auto_commit` 完成 KG Ingest 后**不发射同步事件**。角色更新**完全不触发** Ingest。这导致系统"越写越懂"的承诺在核心路径上失效。
2. **命令层膨胀、服务层空转** — `automation` 模块注册了 9 个 IPC 命令，但前端从未调用其中任何一个。`WorkflowScheduler` 实现了内存队列、超时控制、失败重试，但节点类型全部硬编码且外部无触发入口。
3. **架构契约部分兑现** — `MemoryOrchestrator` 宣称三层记忆组装，但 `build_episodic_memory` 为空实现。`ShortTermMemory` 的 `summarize_chapter` 仅截取前 200 字符，非真正的 LLM 摘要。

---

## 二、方案 A：流程断裂分析 — 6 条核心路径的闭合度

### 路径 1: `update_scene` → Ingest → KG → StateSync

**状态: 基本闭合**

`commands_v3.rs:137-250` 实现了完整的链路:
1. `repo.update()` 保存场景数据
2. `IngestPipeline::ingest()` 提取实体/关系
3. `kg_repo.create_entity()` / `create_relation()` 写入知识图谱
4. `StateSync::emit_ingestion_completed()` + `emit_data_refresh("knowledgeGraph")` 通知前端
5. `VECTOR_STORE.get()` 写入向量存储

**剩余问题**: `ingest` 仅在 `updates.content.is_some()` 时触发。若仅修改场景元数据（标题、戏剧目标、冲突类型等），不触发 Ingest。这些元数据变更对知识图谱的语义完整性有实质影响，当前被忽略。

### 路径 2: `update_chapter` → SceneCommitService::auto_commit → KG Ingest

**状态: 末端断裂** ❌

`lib.rs:1137-1211` 的实现:
1. `repo.update()` 保存章节
2. `emit_chapter_updated()` 通知前端章节更新
3. 30s debounce 后调用 `SceneCommitService::auto_commit()`
4. `auto_commit` → `apply_commit` → `run_kg_ingest` 完成实体/关系入库

**断裂点**: `story_system/mod.rs::run_kg_ingest` 完成知识图谱写入后，**未调用任何 `StateSync` 事件**。`update_scene` 在第 4 步后会发射 `emit_data_refresh("knowledgeGraph")`，但 `update_chapter` 的 auto_commit 路径完全没有这一步。

**后果**: 用户在幕前写章节、保存后，幕后知识图谱永不自动刷新。必须手动刷新页面才能看到新抽取的实体。

### 路径 3: `update_character` → 知识图谱同步

**状态: 完全断裂** ❌❌

`lib.rs` 中 `update_character` 的实现（通过代码结构推断，与 `update_chapter` 同模式）:
1. 更新角色数据库记录
2. 发射 `emit_character_updated()`

**缺失步骤**: **零 Ingest 触发**。角色信息变更（状态、关系、背景）不进入知识图谱，Agent 在后续创作中无法感知角色变化。

### 路径 4: 伏笔创建 → 过期追踪 → 主动通知

**状态: 被动式断裂** ❌

`payoff_ledger.rs::detect_overdue` 存在实现，但:
1. **无定时器/调度器集成** — 没有任何 cron 或周期任务调用 `detect_overdue`
2. **仅在 Agent 执行时被动注入** — `MemoryOrchestrator::build_memory_pack` 在组装 Agent 上下文时才检查逾期伏笔
3. **无 proactive 事件** — 伏笔到期时不发射任何事件通知用户

**后果**: 用户创建了伏笔后，系统不会在适当章节提醒"此处应该回收伏笔"。只有当 Agent 被调用时，逾期伏笔才可能被注入上下文——这是一个概率性事件。

### 路径 5: 知识库导入 → Agent 查询注入

**状态: 闭合** ✅

`agents/commands.rs::build_agent_context` 调用 `kb_search` 并将结果注入 Agent 上下文。此链路完整。

### 路径 6: 订阅升级 → 前端功能可用性实时更新

**状态: 断裂** ❌

`subscription/mod.rs::upgrade_subscription`:
1. 写入数据库记录
2. 返回状态

**缺失步骤**: **未发射任何 Tauri 事件**。前端缓存的订阅状态不会自动失效，用户升级后需要手动刷新页面才能解锁功能。

---

## 三、方案 B：接口悬空分析 — IPC 前后端匹配度扫描

### 3.1 后端有命令、前端零调用的悬空接口（38 个）

#### A. 自动化系统（完全悬空 — 9 个命令）

| 命令 | 位置 | 状态 |
|------|------|------|
| `trigger_automation_event` | `automation/commands.rs:14` | 前端从未调用 |
| `get_automation_triggers` | `automation/commands.rs:27` | 前端从未调用 |
| `get_automation_handlers` | `automation/commands.rs:38` | 前端从未调用 |
| `add_automation_trigger` | `automation/commands.rs:49` | 前端从未调用 |
| `add_automation_handler` | `automation/commands.rs:61` | 前端从未调用 |
| `trigger_story_created` | `automation/commands.rs:73` | 前端从未调用 |
| `trigger_chapter_created` | `automation/commands.rs:86` | 前端从未调用 |
| `trigger_character_created` | `automation/commands.rs:100` | 前端从未调用 |
| `trigger_chapter_content_updated` | `automation/commands.rs:114` | 前端从未调用 |

**分析**: `automation` 模块虽然注册了完整的触发器/处理器架构，但前端没有任何入口。更关键的是，`lib.rs` 中 `update_chapter`/`create_chapter` 等操作虽然通过 `automation_service.trigger_event()` 内部调用了自动化（非 IPC 方式），但自动化事件注册后没有处理器执行任何实际动作——因为 `add_automation_handler` 从未被调用，没有任何 handler 被注册。

#### B. 窗口与前后台通信（部分悬空 — 5 个命令）

| 命令 | 状态 |
|------|------|
| `hide_frontstage` | 未调用（前端用 CSS/状态控制隐藏） |
| `toggle_frontstage` | 未调用 |
| `update_frontstage_content` | 未调用 |
| `get_window_state` | 未调用 |
| `notify_backstage_generation_requested` | 未调用 |

**分析**: `show_frontstage` 被调用，但其他窗口管理命令全部悬空。

#### C. 场景-角色关联（完全悬空 — 5 个命令）

| 命令 | 位置 |
|------|------|
| `get_scene_characters` | `commands_v3.rs:2968` |
| `add_scene_character` | `commands_v3.rs:2986` |
| `remove_scene_character` | `commands_v3.rs:3005` |
| `set_scene_characters` | `commands_v3.rs:3016` |
| `get_character_scenes` | `commands_v3.rs:3035` |

**分析**: 虽然数据库层支持场景-角色多对多关联，但前端没有任何 UI 入口来管理这种关联。这意味着场景的角色参与信息（谁出现在这个场景中）无法被前端配置。

#### D. 工作流系统（触发层悬空 — 4 个命令）

| 命令 | 状态 |
|------|------|
| `create_workflow_instance` | 未调用 |
| `start_workflow_instance` | 未调用 |
| `get_workflow_instance_status` | 未调用 |
| `register_workflow` | 未调用 |

**分析**: 前端 `WorkflowSettings.tsx` 仅展示列表和重新加载（`list_workflows`, `reload_workflows` 被使用），但工作流的注册、创建实例、启动实例等核心操作前端从未调用。

#### E. 其他零散悬空命令（15 个）

| 命令 | 说明 |
|------|------|
| `analyze_story_structure` | 无前端调用 |
| `apply_chapter_commit` | 无前端调用（仅内部通过 auto_commit 调用） |
| `embed_chapter` | 无前端调用 |
| `evolve_capabilities` | 无前端调用 |
| `get_available_agents` | 无前端调用 |
| `get_canonical_state` | 无前端调用 |
| `get_scene_commits` | 无前端调用 |
| `get_skills_by_category` | 无前端调用 |
| `get_state` | 无前端调用 |
| `get_sync_status` | 无前端调用 |
| `get_task` | 无前端调用 |
| `disable_auto_sync` | 无前端调用 |
| `enable_auto_sync` | 无前端调用 |
| `sync_story_data` | 无前端调用 |
| `open_update_settings` | 无前端调用 |

### 3.2 前端有入口、后端无接口的不匹配

#### 悬空前端页面

| 页面 | 位置 | 问题 |
|------|------|------|
| `Chapters.tsx` | `src-frontend/src/pages/Chapters.tsx` | 完整页面存在，但 `App.tsx` 路由中无 `case 'chapters'`，`Sidebar` 无导航入口，`ViewType` 无 `'chapters'` 定义。无任何组件 import。 |

#### 悬空前端设置项

| 设置项 | 位置 | 问题 |
|--------|------|------|
| `WorkflowSettings.tsx` | `pages/settings/WorkflowSettings.tsx` | UI 可查看工作流列表，但**从未调用** `register_workflow`/`create_workflow_instance`/`start_workflow_instance` |
| `MethodologySettings.tsx` | `pages/settings/MethodologySettings.tsx` | UI 可设置 `methodology_id` 和 `methodology_step`，但后端仅更新 Story 表字段，无实际业务处理逻辑 |

#### 悬空前端 Hook

| Hook | 位置 | 问题 |
|------|------|------|
| `useSaveExportTemplate` | `hooks/useExport.ts` | 存在但从未被任何组件使用 |
| `useDeleteExportTemplate` | `hooks/useExport.ts` | 存在但从未被任何组件使用 |

---

## 四、方案 C：架构契约分析 — 高层设计在代码中的兑现度

### 4.1 Workflow 引擎 — 调度层兑现、扩展层违约

**宣称**: 支持 DAG 工作流定义、可扩展节点类型、自动调度执行。

**实际**:
- ✅ `scheduler.rs` 实现了完整的内存队列、自动 drain、节点并行执行、超时控制、失败重试（最多 3 次）、前端事件发射
- ✅ `WorkflowEngine` 支持从数据库恢复实例
- ❌ **所有节点类型硬编码在 `execute_node` 中** — `WriteChapter`、`Inspect`、`Revise`、`VectorIndex` 等节点的执行逻辑是 match 分支写死的
- ❌ **未实现外部可扩展节点系统** — 没有插件化/注册化机制让新节点类型在不修改核心代码的情况下加入

**结论**: Workflow 引擎是一个「封闭的单机调度器」，而非宣称的「可扩展 DAG 工作流平台」。

### 4.2 Memory 系统 — 三层记忆部分空壳

**宣称**: 三层记忆组装（Working / Episodic / Semantic）+ 预算控制 + 优先级过滤。

**实际**:
- ✅ `MemoryOrchestrator::build_memory_pack` 实现了三层组装框架和预算控制
- ✅ `build_semantic_memory` 通过向量检索获取相关记忆
- ❌ **`build_episodic_memory` 为空实现** — 直接返回 `Vec::new()`
- ❌ **`ShortTermMemory::summarize_chapter` 仅取前 200 字符** — 非 LLM 摘要，语义价值极低
- ❌ ** episodic 层实际上不存在** — 场景历史、时间线记忆从未被构建和注入

**结论**: Memory 系统的「三层」只有两层在运转，且短期记忆是字符截断的伪实现。

### 4.3 Vector 存储 — 写入触发点缺失

**宣称**: 向量存储与相似度搜索支持语义检索。

**实际**:
- ✅ `VectorStore` 有基于词频的 fallback 实现
- ✅ `LanceVectorStore` 封装了 LanceDB
- ❌ **自动触发写入点不存在** — `workflow/scheduler.rs` 的 `VectorIndex` 节点在内容 >50 字符时调用 `IngestPipeline::ingest`，但 `ingest` 本身**不写入向量存储**，仅提取结构化知识
- ❌ `update_scene` 在 Ingest 完成后显式调用 `VECTOR_STORE.get()` 写入，但 `update_chapter` 的 auto_commit 路径未确认是否执行了向量写入

**结论**: 向量存储的写入是「手动式」而非「自动化」的，依赖开发者在每个保存路径上显式添加写入代码。

### 4.4 StateSync 事件系统 — 覆盖度不足

**宣称**: 数据变更零延迟同步到双界面。

**实际**:
- ✅ `chapter_updated`/`chapter_created`/`scene_updated` 等基础事件存在
- ❌ **`update_chapter` 的 auto_commit 完成后不发射任何 StateSync 事件** — 知识图谱更新了但前端不刷新
- ❌ **`update_character` 不触发 KG 同步事件** — 角色数据变更不通知 KG 视图
- ❌ **`upgrade_subscription` 不发射订阅变更事件** — 功能解锁不实时生效

### 4.5 Agent 系统 — Prompt 包装器、无本地推理

**宣称**: 多 Agent 协作框架，8 种 Agent 智能分工。

**实际**:
- ✅ 8 个 Agent 均有 `generate_for_agent` 调用 LLM 的完整实现
- ✅ `AgentOrchestrator` 实现了 Writer → Inspector → Writer 的反馈闭环
- ❌ **所有 Agent 均为基于 Prompt 的 LLM 包装器** — 无本地推理、规则引擎、或缓存机制
- ❌ **每次调用都重新组装完整上下文** — 无对话状态保持（除数据库记录外）

**结论**: Agent 系统本质上是「带路由的 Prompt 分发器」，而非真正的多 Agent 协作智能体。每个 Agent 调用都是独立的 LLM 请求，成本高、延迟大。

### 4.6 自动化系统 — 完整空转

**宣称**: 事件驱动的自动化创作增强。

**实际**:
- ✅ 完整的触发器/处理器架构注册到 IPC
- ✅ `lib.rs` 的 `update_chapter`/`create_chapter` 等操作中内部调用了 `automation_service.trigger_event()`
- ❌ **没有任何自动化 handler 被注册** — `add_automation_handler` 从未被调用
- ❌ **前端没有任何自动化配置 UI** — 用户无法创建触发规则

**结论**: 自动化系统是一个「没有规则的专家系统」——引擎在运转，但没有任何规则被加载。

---

## 五、修复优先级矩阵

| 优先级 | 问题 | 影响 | 修复点 | 预估工时 |
|--------|------|------|--------|---------|
| **P0** | `update_chapter` auto_commit 后不发射 StateSync | KG 永不自动刷新 | `story_system/mod.rs::run_kg_ingest` 末尾添加 `emit_data_refresh` | 0.5h |
| **P0** | `update_character` 不触发 Ingest | 角色变更不进 KG | `lib.rs::update_character` 后添加 `IngestPipeline::ingest` | 1h |
| **P0** | `upgrade_subscription` 不发射事件 | 升级后需手动刷新 | `subscription/mod.rs` 添加 `emit_subscription_changed` | 0.5h |
| **P1** | `build_episodic_memory` 空实现 | Episodic 记忆层缺失 | `memory/orchestrator.rs` 实现基于场景历史的 episodic 构建 | 2h |
| **P1** | `ShortTermMemory::summarize_chapter` 仅截断 | 短期记忆语义价值低 | 改用 LLM 摘要或更智能的提取 | 1h |
| **P1** | 伏笔过期无主动调度 | 伏笔回收依赖概率 | 在 `update_chapter`/`create_chapter` 后添加 `detect_overdue` 调用 + 事件发射 | 1h |
| **P1** | 场景-角色关联 IPC 全部悬空 | 无法管理场景角色 | 前端 Scene 编辑器添加角色参与面板 | 2h |
| **P2** | 38 个 IPC 命令无前端调用 | 代码膨胀、维护成本高 | 删除确认无内部调用的命令，或补充前端入口 | 3h |
| **P2** | 自动化系统无 handler 注册 | 自动化飞轮空转 | 添加默认自动化规则初始化 + 前端配置 UI | 3h |
| **P2** | Workflow 节点硬编码 | 无法扩展新节点类型 | 设计节点注册 trait + 插件化机制 | 4h |

---

## 六、结构性建议

### 6.1 建立「保存后连锁反应」的统一钩子

当前 `update_scene`、`update_chapter`、`update_character` 各自维护独立的 Ingest/Sync 逻辑，导致覆盖不一致。建议建立统一的 `AfterDataChange` 钩子系统：

```rust
// 伪代码
fn after_data_change(story_id, change_type, payload) {
    match change_type {
        SceneContentChanged | ChapterContentChanged => {
            ingest_pipeline.enqueue(story_id, payload);
            vector_store.enqueue(story_id, payload);
        }
        CharacterUpdated => {
            ingest_pipeline.enqueue_character(story_id, payload);
            kg_sync.invalidate_character_cache(story_id);
        }
        // ...
    }
    state_sync.emit_data_refresh(story_id, change_type);
}
```

### 6.2 向量化写入自动化

将向量存储的写入从「每个保存路径手动调用」改为「IngestPipeline 的固定后处理步骤」。`IngestPipeline::ingest()` 完成后自动写入向量存储，无需调用方关心。

### 6.3 清理悬空命令的策略

对于 38 个悬空 IPC 命令，采用分级处理：
1. **确认无任何内部调用** → 直接删除（如 `open_update_settings`）
2. **有内部调用但无前端** → 评估是否需要暴露给前端，或转为纯内部函数（如 `get_scene_commits`）
3. **功能完整但 UI 未接入** → 保留命令，创建前端接入任务（如场景-角色关联）

---

## 七、结论

StoryForge 的核心问题不是「缺少功能」，而是「**功能之间的连接断裂**」。后端有大量的引擎、调度器、存储系统在独立运转，但它们之间的数据流没有形成闭环。最尖锐的发现是：

> **用户在幕前写作越多，系统理应越懂这个故事——但实际上，章节保存后的知识图谱更新链路在最后一个环节断裂，导致系统对用户最新的创作内容视而不见。**

修复这些断裂点不需要重写任何子系统，只需要在正确的位置补上缺失的函数调用和事件发射。这是低投入、高回报的修复。
