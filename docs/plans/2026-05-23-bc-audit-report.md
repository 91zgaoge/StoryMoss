# StoryMoss B+C 组合审计报告：用户旅程断点 + 模块结构健康度

**审计日期**: 2026-05-23  
**审计范围**: 前端(src-frontend/src) ↔ 后端(src-tauri/src) IPC 接口完整性、模块依赖健康度、用户旅程端到端验证  
**审计方法**: 静态代码分析（前后端命令交叉对比、模块依赖图构建、页面级调用链追踪）  

---

## 执行摘要

本次审计发现 **7 个运行时断点**（前端调用命令未在后端注册）、**124 个休眠命令**（后端注册但前端从未调用）、**模型层严重碎片化**（2 套模型 + 6 套仓库共存）、以及 **lib.rs 中心化膨胀**（326 行命令注册块）。这些问题共同构成了**设计与实现脱节的系统性风险**。

| 维度 | 严重问题数 | 中等问题数 | 备注 |
|------|-----------|-----------|------|
| IPC 接口完整性 | 7 | 124 | 7 个运行时错误源 |
| 模型/仓库层 | 2 | 6 | v3 迁移未完成 |
| 模块耦合度 | 3 | 5 | db 模块 109 个依赖者 |
| 页面功能完整性 | 2 | 3 | Tasks 页空壳、CreationWizard 不完整 |
| 状态同步健康度 | 1 | 2 | 部分事件未覆盖 |

---

## B 部分：用户旅程断点审计

### B1. IPC 接口运行时断点（P0 — 立即修复）

以下 **7 个命令** 在前端代码中通过 `loggedInvoke()` 调用，但**未在后端 `lib.rs` 的 `generate_handler!` 中注册**。这些调用在运行时必然抛出 IPC 错误：

| # | 命令名 | 前端调用位置 | 所属功能域 | 影响 |
|---|--------|-------------|-----------|------|
| 1 | `get_quota_detail` | `services/tauri.ts:486` | 订阅/配额 | useSubscription Hook 初始化失败 |
| 2 | `check_auto_write_quota` | `services/tauri.ts:489` | 订阅/配额 | auto_write 配额检查失效 |
| 3 | `check_auto_revise_quota` | `services/tauri.ts:492` | 订阅/配额 | auto_revise 配额检查失效 |
| 4 | `get_chapter_commits` | `services/tauri.ts:753` | 章节/Commit | 注释称"不再显式调用"，但代码仍在 |
| 5 | `list_genesis_runs` | `services/tauri.ts:1216` | Genesis 引擎 | 创作向导历史记录无法加载 |
| 6 | `get_genesis_run` | `services/tauri.ts:1219` | Genesis 引擎 | 单条运行记录查看失败 |
| 7 | `get_latest_style_snapshot` | `services/tauri.ts:1241` | 风格快照 | 风格漂移检测前置数据缺失 |

**根因**: 这些命令可能曾在后端实现，但在某次重构中被移除或遗忘注册；或前端先行开发而后端未跟进。

**修复建议**:  
- 方案 A（推荐）: 在后端补全这 7 个命令的空实现（返回默认数据），立即消除运行时错误。
- 方案 B: 若功能已废弃，从前端移除调用代码。

---

### B2. 休眠命令审计（P1 — 清理或补全前端）

后端 `generate_handler!` 共注册了约 **170 个唯一命令名**，其中 **124 个（73%）** 在前端代码中**从未通过 `loggedInvoke` 调用**。

**按功能域分布的休眠命令 Top 5**:

| 功能域 | 休眠命令数 | 示例 |
|--------|-----------|------|
| Pipeline (commands_pipeline) | 35+ | `create_blueprint`, `create_draft`, `create_revision`, `merge_revision` 等 |
| Workflow | 8 | `register_workflow`, `create_workflow_instance`, `start_workflow_instance` 等 |
| Automation | 9 | `add_automation_trigger`, `add_automation_handler`, `trigger_story_created` 等 |
| Knowledge Base | 4 | `kb_import_text`, `kb_search`, `kb_stats` 等 |
| Agent (非核心) | 6+ | `agent_execute_stream`, `auto_write_cancel`, `auto_revise_cancel` 等 |

**具体观察**:

1. **Pipeline 系统**: 后端注册了完整的 35+ 个 Pipeline 命令（blueprint → draft → revision → review → post-process），但前端**仅**在 `WorkflowSettings.tsx` 中调用了 `list_workflows` 和 `reload_workflows`。Pipeline 的完整工作流 UI 缺失。

2. **Workflow 系统**: 后端注册了 `register_workflow`, `create_workflow_instance`, `start_workflow_instance`, `get_workflow_instance_status` 等，但前端仅查看工作流列表，无法创建或启动实例。

3. **Automation 系统**: 后端注册了 9 个自动化命令，但前端**没有任何页面或 Hook 调用它们**。`AutomationService::initialize()` 在启动时注册了默认规则，但用户无法通过 UI 查看或修改规则。

4. **Knowledge Base**: 后端注册了 4 个 KB 命令，但前端 KnowledgeGraph 页面使用的是 `useKnowledgeDistillation` Hook（调用 `distill_story_knowledge` 等），**从未调用** `kb_search` 或 `kb_import_text`。

**根因**: 这些系统属于"后端先行、前端滞后"或"功能废弃但未清理"的情况。大量命令占用了编译时间和二进制体积，却无任何用户价值。

**修复建议**:
- 对确认废弃的命令：从 `generate_handler!` 移除，减少攻击面和编译负担。
- 对计划中的功能：建立功能开关或标注 `#[deprecated]`，并关联前端开发任务。

---

### B3. 页面级功能断点

#### B3.1 Tasks 页面 — 空壳页面（P1）

**审计发现**: `src-frontend/src/pages/Tasks.tsx` **未导入任何自定义数据 Hook**（仅使用 `useState`, `useEffect`），也未调用任何 `loggedInvoke`。该页面是一个**空壳** — 虽有导航入口，但无法创建、查看或管理任务。

**与后端对比**: 后端完整注册了 Task System 命令（`create_task`, `update_task`, `delete_task`, `list_tasks`, `trigger_task`, `cancel_task`, `get_task_logs`），且 `useTasks.ts` Hook 已完整实现。但 Tasks 页面未使用这些 Hook。

**修复**: 在 Tasks 页面集成 `useTasks` Hook，或从 Sidebar 移除导航入口。

#### B3.2 CreationWizard 页面 — 半成品（P1）

**审计发现**: `CreationWizard.tsx` 仅使用 `useQuery` 获取 `styleDnas`，**未调用**后端注册的创作向导命令：
- `generate_world_building_options`
- `generate_character_profiles`
- `generate_writing_styles`
- `generate_first_scene`
- `create_story_with_wizard`

这些命令在 `commands_v3.rs` 中有完整实现，但前端向导页面未集成。

**修复**: 补全 CreationWizard 与后端向导命令的集成，或移除该页面入口。

#### B3.3 Subscription 配额系统 — 前端超前（P1）

`useSubscription.ts` 实现了完整的配额检查逻辑（`checkAutoWrite`, `checkAutoRevise`, `getQuotaDetail`），但对应的后端命令缺失（见 B1）。这导致：
- 免费用户无法正确看到配额状态
- `auto_write` / `auto_revise` 的调用前检查失效

**修复**: 优先实现 `get_quota_detail`, `check_auto_write_quota`, `check_auto_revise_quota` 三个命令。

---

### B4. 状态同步健康度

#### B4.1 事件覆盖度（P2）

`StateSync` 事件系统覆盖了 19 种事件类型（Story/Character/Scene/Chapter CRUD + WorldBuilding + Relationships + PayoffLedger + Ingestion + DataRefresh + Subscription + PayoffOverdue）。

**缺失覆盖**:
- `WorldBuildingDeleted` / `WorldBuildingCreated`: 只有 `WorldBuildingUpdated`
- `StyleDnaUpdated`: 风格 DNA 变更无同步事件
- `AnnotationResolved`: 批注解决无同步事件（前端需手动刷新）
- `TaskUpdated` / `TaskCreated`: Task System 无同步事件

**修复**: 为上述缺失场景补发同步事件，或在相关 Hook 的 `onSuccess` 回调中主动刷新关联查询。

#### B4.2 前端同步监听（健康）

`App.tsx` 通过 `useSyncStore` 统一监听 `sync-event`，并在 `currentStory` 变化时自动刷新 8 类关联数据。这是健康的模式。

---

## C 部分：模块结构健康度审计

### C1. 数据层碎片化 — 模型/仓库冗余（P0）

#### C1.1 双轨模型体系

`src-tauri/src/db/` 目录存在**并行未合并**的模型定义：

| 文件 | 结构体数 | 说明 |
|------|---------|------|
| `models.rs` | 40 | 旧版模型（v2 架构） |
| `models_v3.rs` | 45 | 新版模型（v3 架构） |

**问题**: 两套模型共存意味着：
1. 同一个业务概念可能在两个文件中有不同定义（如 `Story` vs `StoryV3`）
2. 新增字段需要在两个文件中同步修改
3. 编译器无法阻止混用，导致数据不一致风险

#### C1.2 六轨仓库体系

| 文件 | 仓库数 | 说明 |
|------|--------|------|
| `repositories.rs` | 5 | 旧版仓库 |
| `repositories_v3.rs` | 20 | v3 主仓库 |
| `repositories_narrative.rs` | ? | 叙事专用 |
| `repositories_story_system.rs` | ? | StorySystem 专用 |
| `repositories_pipeline.rs` | ? | Pipeline 专用 |
| `repositories_export.rs` | ? | 导出专用 |

**问题**: 仓库按功能域拆分为多个文件，但未按统一规则组织。部分仓库（如 `KnowledgeGraphRepository`）散落在 `repositories_v3.rs`，而 `SceneCommitRepository` 却在 `repositories_story_system.rs`。

**修复建议**:
- 短期：明确标注每个仓库的"主文件"归属，禁止在新代码中引用 `models.rs` 和 `repositories.rs`。
- 长期：将 `models.rs` 中的结构体逐步迁移到 `models_v3.rs`，然后删除旧文件；将分散的仓库文件合并到 `repositories_v3.rs` 或按领域模块拆分（如 `story_system::repositories`）。

---

### C2. 模块耦合度分析

#### C2.1 核心模块依赖分布

```
db        ← 109 个文件依赖（最高耦合）
llm       ← 37 个文件依赖
memory    ← 3 个文件依赖（过低，未充分利用）
vector    ← 直接嵌入 lib.rs（未模块化使用）
config    ← 设置相关模块依赖
creative_engine ← payoff_ledger 被 memory/orchestrator 引用
```

#### C2.2 db 模块作为"上帝模块"（P1）

`db` 模块被 109 个文件直接依赖，这意味着：
- 任何 `db` 模块的修改都会影响近半数代码文件
- `db` 模块的编译成为整个项目的瓶颈
- 无法独立测试上层业务逻辑（必须带真实 DB）

**具体依赖链示例**:
```
lib.rs → db::* (StoryRepository, CharacterRepository, ...)
commands_v3.rs → db::*
memory/orchestrator.rs → db::MemoryItemRepository, db::SceneCommitRepository
story_system/mod.rs → db::StoryContractRepository, db::SceneCommitRepository
pipeline/mod.rs → db::DraftRepository, db::RevisionRepository, ...
```

**修复建议**:
- 引入 **Repository Trait 层**：定义 `StoryRepo`, `CharacterRepo` 等 trait，上层依赖 trait 而非具体实现。
- 将 `db` 模块拆分为 `db-core`（连接池、基础类型）和 `db-repositories`（具体实现）。

#### C2.3 memory 模块利用率不足（P2）

`memory` 模块仅被 3 个文件依赖，与其设计目标（三层记忆：Working/Episodic/Semantic）不匹配。`MemoryOrchestrator` 的 `build_episodic_memory` 方法在最近的修复中才从空实现改为真实查询。

**根因**: `MemoryOrchestrator` 未与前端任何功能直接关联，处于"后端有实现、前端无入口"的状态。

---

### C3. lib.rs 中心化膨胀（P1）

`src-tauri/src/lib.rs` 的 `generate_handler!` 块长达 **326 行**，直接注册了约 170 个命令。此外，lib.rs 还包含：
- 多个直接定义的命令函数（`health_check`, `chat_completion`, `audit_story`, `anti_ai_review` 等）
- `ChatMessageItem` 等数据结构定义
- 窗口管理逻辑

**问题**:
1. **单一职责违反**: lib.rs 既是模块入口，又是命令注册中心，还是部分命令的实现地。
2. **合并冲突高发**: 任何新增命令都需要修改 lib.rs，导致多人协作时冲突频繁。
3. **领域混合**: `connect_mcp_server`（MCP）与 `create_story`（Story）与 `check_update`（Updater）在同一块中注册，无领域隔离。

**修复建议**:
- 按领域拆分命令注册：每个子模块提供自己的 `commands()` 函数，返回命令数组，lib.rs 仅做聚合。
- 将 lib.rs 中直接定义的命令迁移到对应模块（如 `chat_completion` → `chat::commands`）。

---

### C4. 架构版本混乱（P2）

项目存在明显的**多版本架构并存**痕迹：

| 版本标识 | 存在位置 | 状态 |
|----------|---------|------|
| v2 (legacy) | `models.rs`, `repositories.rs`, 部分 lib.rs 命令 | 应废弃 |
| v3 | `models_v3.rs`, `repositories_v3.rs`, `commands_v3.rs` | 当前主架构 |
| v4 | `style_dna`, `style_blend` 命令 | 部分集成 |
| v5 | `genesis engine`, `workflow` 命令 | 前端未完整对接 |
| v6 | `story_system`, `reading_power`, `anti_ai_review` | 部分实现 |
| v7 | `pipeline`, `knowledge_base` | 前端未对接 |

**问题**: 版本号散落在命令注释中（`// V3 Architecture commands`, `// v5.2.0`, `// v6.0.0`, `// v7.0.0`），但缺乏统一的版本迁移计划。新开发者无法判断应该使用 `models.rs` 还是 `models_v3.rs`。

**修复建议**:
- 删除所有版本号注释，改用模块命名空间表达架构边界。
- 制定明确的废弃时间表：v2 模型在 X 版本后删除，v3 成为唯一模型层。

---

### C5. 循环依赖风险（P2 — 潜在）

当前未发现显式循环依赖（`use crate::A` 与 `use crate::B` 互引），但存在**隐式循环风险**：

```
memory/orchestrator.rs → creative_engine::payoff_ledger::PayoffLedger
creative_engine/ (可能依赖) → memory/ (未来扩展时)
```

若 `creative_engine` 未来需要引用 `memory` 模块进行记忆检索，将形成循环依赖。

**修复建议**: 在 `memory` 与 `creative_engine` 之间引入 **抽象层**（trait 或事件总线），避免直接模块引用。

---

## 优化计划

### 第一阶段：止血（1-2 天）

1. **补全 7 个缺失命令**（B1）
   - 在 `lib.rs` 或对应模块中注册：`get_quota_detail`, `check_auto_write_quota`, `check_auto_revise_quota`, `get_chapter_commits`, `list_genesis_runs`, `get_genesis_run`, `get_latest_style_snapshot`
   - 若命令实现已存在但未注册，直接添加注册；若实现缺失，先做空实现返回默认值。

2. **移除或注释空壳页面入口**（B3）
   - Tasks 页面：若短期内不开发，从 Sidebar 移除入口。
   - CreationWizard：若短期内不开发，从 Sidebar 移除入口。

### 第二阶段：清理（3-5 天）

3. **休眠命令清理**（B2）
   - 对每个休眠命令，判断是"废弃"还是"待开发"。
   - 废弃的：从 `generate_handler!` 移除。
   - 待开发的：添加 `#[allow(unused)]` 并关联前端任务编号。

4. **模型层归一化启动**（C1）
   - 统计 `models.rs` 中仍在被引用的结构体。
   - 制定迁移清单，逐个将 v2 模型迁移到 `models_v3.rs`。
   - 禁止新项目代码引用 `models.rs`（通过 code review 规则）。

### 第三阶段：重构（1-2 周）

5. **db 模块解耦**（C2）
   - 为核心仓库定义 trait（`StoryRepository`, `CharacterRepository` 等）。
   - 上层模块（commands_v3, story_system）依赖 trait 而非具体类型。
   - 引入依赖注入框架（如 `shaku` 或手动 DI）。

6. **lib.rs 命令注册拆分**（C3）
   - 每个子模块暴露 `pub fn commands() -> Vec<Command>`。
   - lib.rs 的 `generate_handler!` 简化为聚合调用：
     ```rust
     .invoke_handler(tauri::generate_handler![
         ...db::commands(),
         ...llm::commands(),
         ...commands_v3::commands(),
         ...pipeline::commands(),
     ])
     ```

7. **补齐前端功能页面**（B3）
   - Pipeline 系统：设计 Blueprint → Draft → Revision 的 UI 工作流。
   - Task System：完成 Tasks 页面，集成 `useTasks` Hook。
   - Automation System：添加规则配置页面。

---

## 附录：数据收集脚本

以下脚本可用于持续监控 IPC 接口健康度：

```bash
# 1. 提取前端所有 IPC 调用
grep -roE "loggedInvoke\('[a-z_]+'" src-frontend/src/ | sed "s/.*'//" | sed "s/'//" | sort | uniq > frontend_cmds.txt

# 2. 提取后端所有注册命令
awk '/generate_handler!\[/,/\]/' src-tauri/src/lib.rs | grep -oE '[a-z_][a-z_0-9]*' | sort | uniq > backend_cmds.txt

# 3. 对比差异
comm -23 frontend_cmds.txt backend_cmds.txt  # 前端有但后端无（断点）
comm -13 frontend_cmds.txt backend_cmds.txt  # 后端有但前端无（休眠）

# 4. 模块依赖统计
grep -r "^use crate::db" src-tauri/src/ --include="*.rs" | wc -l
grep -r "^use crate::llm" src-tauri/src/ --include="*.rs" | wc -l
grep -r "^use crate::memory" src-tauri/src/ --include="*.rs" | wc -l
```

---

*报告结束*
