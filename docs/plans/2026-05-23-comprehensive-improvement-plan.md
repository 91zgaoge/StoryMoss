# StoryMoss 全面完善与优化计划

**制定日期**: 2026-05-23  
**制定依据**: B+C 组合审计报告 + CONTEXT.md 已知间隙 + 架构方向讨论共识  
**计划性质**: 全集式 backlog，待优先级排序后分批执行  

---

## 计划总览

本计划涵盖 5 大领域、21 个具体项目。所有项目均已明确目标、范围、依赖关系和验收标准，供后续优先级排序使用。

```
领域 A: 接口契约修复（前端 ↔ 后端 IPC 边界清理）
领域 B: 数据层归一化（模型/仓库/记忆体系梳理）
领域 C: 架构层重构（模块解耦、注册去中心化）
领域 D: 功能层补全（Cascade Rewriter、Task 中心、Pipeline 闭环）
领域 E: 体验层完善（状态同步、双窗口职责、页面功能补全）
```

---

## 领域 A: 接口契约修复

### A1. 移除配额计量系统（已共识）

**目标**: 消除与商业模式矛盾的配额检查逻辑。软件订阅解锁功能，不介入模型用量计费。

**范围**:
- 前端: 删除 `useSubscription.ts` 中的 `getQuotaDetail`/`checkAutoWriteQuota`/`checkAutoReviseQuota` 调用，UI 改为功能开关展示（"Pro 功能" vs "免费版限制"）。
- 后端: 删除 `lib.rs` 中对 `get_quota_detail`/`check_auto_write_quota`/`check_auto_revise_quota` 的注册（如存在实现也一并删除）。
- 后端: 清理 `agents/commands.rs` 和 `llm/service.rs` 中的 `check_platform_quota` 调用，改为功能门控（feature-gating）。

**依赖**: 无（可独立执行）。

**验收标准**:
- 前端订阅 UI 不再显示字数配额，只显示功能解锁状态。
- `cargo check` 无残留配额相关代码。

---

### A2. 补全运行时断点（4 个非配额命令）

**目标**: 消除运行时 IPC 错误。

**范围**:
- `get_chapter_commits`（`tauri.ts:753`）: 确认是否仍需。注释称"不再显式调用"，若属实则删除前端调用代码。
- `list_genesis_runs` / `get_genesis_run`（`tauri.ts:1216-1219`）: Genesis 引擎运行记录查询。后端需补全实现或前端移除调用。
- `get_latest_style_snapshot`（`tauri.ts:1241`）: 风格漂移检测前置数据。后端需补全实现。

**依赖**: A1（配额移除后，断点清单更清晰）。

**验收标准**:
- 前端所有 `loggedInvoke` 调用均有对应后端注册命令。
- 运行时不出现 IPC "command not found" 错误。

---

### A3. Pipeline 数据管理命令内部化（已共识）

**目标**: Pipeline 是全自动设计，数据层对用户不可见。

**范围**:
- `commands_pipeline.rs` 中 35+ 个 CRUD 命令（`create_blueprint`, `create_draft`, `create_revision`, `get_draft_revisions`, `merge_revision` 等）降级为 `pipeline/` 模块的内部函数。
- `lib.rs` 的 `generate_handler!` 中移除这些命令的注册。
- `FrontstageApp.tsx` 保留的 `runRefine`/`runReview`/`runFinalize`/`getPipelineActiveDraft` 继续作为唯一暴露的 Pipeline 接口。

**依赖**: 无（可独立执行）。

**验收标准**:
- `commands_pipeline.rs` 文件删除，函数迁移到 `pipeline/` 内部模块。
- 前端仍能正常触发 Refine/Review/Finalize。
- `cargo check` 通过。

---

### A4. 休眠命令清理

**目标**: 移除无调用者的后端命令，减少攻击面和编译负担。

**范围**:
- **Workflow 系统**: `register_workflow`, `create_workflow_instance`, `start_workflow_instance`, `get_workflow_instance_status` 等。保留 `list_workflows`/`reload_workflows`（前端 WorkflowSettings 使用）。其余降级为内部函数或删除。
- **Automation 系统**: `add_automation_trigger`, `add_automation_handler` 等。`AutomationService::initialize()` 在启动时注册默认规则，这些命令若未来不在幕后提供管理 UI，则全部移除。
- **Knowledge Base**: `kb_import_text`, `kb_search`, `kb_stats`。KB 由 `IngestPipeline` 和 `PostProcess` 内部调用，不暴露为 IPC。
- **Agent 非核心命令**: `agent_execute_stream`, `auto_write_cancel`, `auto_revise_cancel` 等。确认是否被调用，未调用则移除。

**依赖**: A3（Pipeline 内部化后，再清理其他系统，避免交叉影响）。

**验收标准**:
- 后端注册命令数从 ~170 降至仅包含"用户直接触发的操作"。
- 每个保留的命令都有明确的前端调用者或内部调度者。

---

### A5. Task System 命令保留决策

**目标**: 明确 Task System 的 IPC 边界。Task 是必须的，但需区分"用户管理任务"与"系统内部创建任务"。

**范围**:
- 保留 `list_tasks`（幕后任务列表页）、`get_task`（任务详情）、`cancel_task`（用户取消）。
- `create_task`/`trigger_task` 若仅由内部系统调用（如 Cascade Rewriter 触发时自动创建任务），则降级为内部函数。
- `get_task_logs` 保留（用户查看任务执行日志）。

**依赖**: D3（Task System 升级方案确定后，再精确裁剪命令）。

**验收标准**:
- Task System 的 IPC 边界与"用户可见"和"系统内部"职责对齐。

---

## 领域 B: 数据层归一化

### B1. IngestPipeline 质量提升（记忆系统前置条件）

**目标**: 为记忆系统激活提供高质量的数据基础（准确的实体提取、关系识别、向量化）。

**范围**:
- **Prompt 工程**: 优化 LLM 实体提取 prompt，增加 few-shot 示例，减少幻觉实体。
- **Schema 严格化**: JSON 输出增加更严格的 schema 验证，失败时重试（而非静默接受不完整数据）。
- **增量 Ingest**: 目前 `update_scene` 触发 Ingest 时可能重复处理全文。改为仅处理变更部分（diff-based ingest），减少 LLM 调用量和延迟。
- **实体消歧**: 同一角色在不同场景中的提及可能被提取为不同实体。引入实体链接（entity linking）机制，基于名称相似度和上下文将提及关联到同一实体。

**依赖**: 无（可独立执行，但收益在 B2 激活后体现）。

**验收标准**:
- 手动抽查 10 个场景的 Ingest 结果，实体提取准确率 > 90%（对比人工标注）。
- 无重复实体（同一角色只对应一个 KG 节点）。

---

### B2. 记忆系统激活（已共识 — 最优先架构债务）

**目标**: 让 `memory_pack` 从 `None` 变为真实组装的三层记忆。

**范围**:
- **预组装策略**: `MemoryPack` 不在 AI 生成时实时组装（延迟过高），而是在 `SCENE_COMMIT` 时预组装并缓存。
- **QueryPipeline 接入**: `StoryContextBuilder` 在 `GenerationMode::Full` 下调用 `QueryPipeline` + `MemoryOrchestrator` 生成 `MemoryPack`，存入缓存。
- **AgentContext 注入**: `AgentOrchestrator` 从缓存读取 `MemoryPack`，填入 `AgentContext.memory_pack`。
- **降级保护**: 若 QueryPipeline 失败或缓存未命中，回退到 `previous_chapters`（确保不阻断生成）。
- **缓存失效**: 当 `SCENE_COMMIT` 触发时，清除该 Story 的 `MemoryPack` 缓存，下次生成时重新组装。

**依赖**: B1（Ingest 质量提升后激活，避免垃圾进垃圾出）。

**验收标准**:
- `AgentContext.memory_pack` 不再永远为 `None`。
- `format_memory_pack_for_prompt` 输出的记忆文本包含相关实体、历史事件、风格提示（而非仅时间排序的章节摘要）。
- Ghost Text（Fast 模式）不触发 QueryPipeline（保持低延迟）。

---

### B3. db 模型归一化（v2 → v3）

**目标**: 消除双轨模型体系，统一为 `models_v3.rs`。

**范围**:
- **引用审计**: 统计 `models.rs` 中 40 个结构体的引用分布，识别哪些仍在被使用。
- **迁移清单**: 为每个仍在使用的 v2 模型制定迁移方案（字段映射、默认值、数据库迁移脚本）。
- **逐步替换**: 按依赖链从叶子到根逐个替换引用（先替换不依赖其他 v2 模型的结构体）。
- **删除旧文件**: 当 `models.rs` 中所有结构体均迁移完成，删除文件并更新 `db/mod.rs`。

**依赖**: 无（可长期逐步执行）。

**验收标准**:
- `models.rs` 文件删除。
- `cargo check` 无 `db::models` 引用。

---

### B4. db 仓库归一化

**目标**: 消除六轨仓库体系，统一按领域组织。

**范围**:
- **合并策略**: 将 `repositories.rs`（5 个旧仓库）、`repositories_narrative.rs`、`repositories_pipeline.rs`、`repositories_story_system.rs`、`repositories_export.rs` 中的仓库迁移到统一位置。
- **两种方案**:
  - **方案 A**（集中式）: 全部合并到 `repositories_v3.rs`（简单，但文件可能过大）。
  - **方案 B**（领域式）: 每个领域模块自包含仓库（如 `story_system::repositories`）。推荐方案 B，与 lib.rs 拆分方向一致。
- **迁移规则**: 新代码禁止引用 `repositories.rs`；旧代码逐步迁移。

**依赖**: B3（模型归一化后，仓库归一化更顺畅）。

**验收标准**:
- `repositories.rs`, `repositories_narrative.rs`, `repositories_pipeline.rs`, `repositories_story_system.rs`, `repositories_export.rs` 文件删除。
- 每个仓库有明确的领域归属。

---

### B5. db 模块解耦（Repository Trait 层）

**目标**: 打破 `db` 上帝模块，上层依赖抽象而非具体实现。

**范围**:
- **Trait 定义**: 为核心实体定义 Repository Trait（如 `StoryRepo`, `CharacterRepo`, `SceneRepo`）。
- **依赖注入**: `commands_v3`, `story_system`, `memory` 等模块通过 trait 对象或泛型参数获取仓库，而非直接 `use crate::db::*`。
- **测试收益**: 引入 trait 后，业务逻辑单元测试可使用 mock 仓库，无需真实数据库。

**依赖**: B4（仓库归一化后，再定义 trait 层，避免 trait 定义分散）。

**验收标准**:
- `commands_v3.rs` 不再直接 `use crate::db::*`，而是 `use crate::db::repositories::{StoryRepo, CharacterRepo}`（trait）。
- 存在至少一个使用 mock 仓库的单元测试。

---

## 领域 C: 架构层重构

### C1. lib.rs 命令注册拆分

**目标**: 消除 326 行中心化命令注册块，按领域分散职责。

**范围**:
- 每个子模块暴露 `pub fn commands() -> Vec<Command>`（或类似聚合函数）。
- `lib.rs` 的 `generate_handler!` 简化为：
  ```rust
  .invoke_handler(tauri::generate_handler![
      ...core::commands(),
      ...llm::commands(),
      ...commands_v3::commands(),
      ...pipeline::commands(), // 仅保留 run_refine/run_review/run_finalize
      ...settings::commands(),
      ...updater::commands(),
  ])
  ```
- lib.rs 中直接定义的命令（`health_check`, `chat_completion`, `audit_story`, `anti_ai_review` 等）迁移到对应模块（如 `chat::commands`, `audit::commands`）。

**依赖**: A3, A4, A5（休眠命令清理后，注册块自然瘦身，再拆分更容易）。

**验收标准**:
- `lib.rs` 的 `generate_handler!` 块 < 50 行。
- 新增命令不需要修改 `lib.rs`（只需在对应模块的 `commands()` 中添加）。

---

### C2. 版本号注释清理

**目标**: 消除 v2-v7 版本标识的混乱，用模块命名空间表达架构边界。

**范围**:
- 删除所有 `// V3 Architecture commands`, `// v5.2.0`, `// v6.0.0`, `// v7.0.0` 等注释。
- `commands_v3.rs` 重命名为更具领域意义的名称（如 `story_commands.rs` 或拆分为 `scene_commands.rs`, `character_commands.rs` 等）。
- `models_v3.rs` 在 B3 完成后成为唯一模型文件，可重命名为 `models.rs`（覆盖旧文件）。

**依赖**: B3, B4, C1（模型/仓库/命令归一化后，再清理版本标识）。

**验收标准**:
- 代码库中无 `v3`/`v5`/`v6`/`v7` 版本号注释。
- 文件名和模块名表达领域含义，而非版本号。

---

### C3. memory ↔ creative_engine 解耦

**目标**: 消除隐式循环依赖风险。

**范围**:
- 目前 `memory/orchestrator.rs` 直接引用 `creative_engine::payoff_ledger::PayoffLedger`。
- 引入抽象层：定义 `PayoffLedgerTrait` 或事件总线，让 `memory` 模块通过 trait/事件与 `creative_engine` 交互，而非直接模块引用。

**依赖**: 无（可独立执行）。

**验收标准**:
- `memory/` 目录下无任何 `use crate::creative_engine` 引用。

---

## 领域 D: 功能层补全

### D1. Cascade Rewriter（级联改写器）

**目标**: 实现"幕后调整自动改写正文"的核心原则。

**范围**:
- **触发源**: 当用户在幕后修改角色、世界观、故事线时，自动触发 Cascade Rewriter（或让用户点击"应用变更到正文"后触发）。
- **变更影响分析**: 基于 knowledge graph 中的实体引用，识别哪些场景涉及被修改的实体。
- **文本对齐**: 在场景正文中定位与变更实体相关的段落（需要 entity-mention 索引）。
- **增量改写**: 调用 LLM 仅改写受影响段落，保持其他部分不变。prompt 需包含：变更前后对比、原始段落、改写约束（风格一致、不改变未提及内容）。
- **一致性验证**: 改写后检查前后场景的连贯性（可通过 rule-based heuristic 或轻量 LLM call）。
- **用户确认**: 在幕后展示改写预览（diff 视图），让用户批量确认或拒绝。

**架构定位**: Cascade Rewriter 作为独立模块 `creative_engine::cascade_rewriter`，由 Task System 驱动执行。

**依赖**: B2（记忆系统激活后，knowledge graph 质量足够支撑实体引用索引）。

**验收标准**:
- 修改角色性格设定后，系统能识别出涉及该角色的 3 个场景，并生成改写预览。
- 用户可以在 Tasks 页面看到"级联改写"任务的进度和结果。

---

### D2. Task System 升级为全局后台作业中心

**目标**: 让所有后台工作（AI 生成、Pipeline 执行、级联改写、Ingest）都纳入 Task 统一管理。

**范围**:
- **统一追踪**: `AgentOrchestrator`、`Pipeline`、`WorkflowScheduler`、`Cascade Rewriter` 在执行长作业时，都向 `TaskService` 注册任务。
- **标准化进度**: 所有任务遵循统一的进度事件协议（`TaskProgressEvent`、`TaskHeartbeatEvent`）。
- **规范显示**: 前端 Tasks 页面展示：任务列表、进度条、日志输出、结果预览、取消按钮。
- **任务分类**: `ai_generation`（AI 生成）、`pipeline_review`（Pipeline 审校）、`cascade_rewrite`（级联改写）、`ingest`（知识提取）。
- **用户触发**: 用户在幕前点击"续写"、"审校"、"应用变更"时，前端显示任务创建确认，然后在 Tasks 页面追踪进度。

**依赖**: D1（Cascade Rewriter 需要 Task System 驱动）；E2（Tasks 页面补全）。

**验收标准**:
- 每次 AI 生成长文本（>1000 字）都对应一个可见的任务条目。
- 用户可以在 Tasks 页面取消正在进行的 Pipeline Review 或 Cascade Rewrite。

---

### D3. Pipeline 执行纳入 Task 追踪

**目标**: `runRefine`/`runReview`/`runFinalize` 的执行过程对用户可见。

**范围**:
- `FrontstageApp.tsx` 调用 `runRefine` 时，后端不仅执行 Pipeline，还创建一个 `Task` 记录。
- Pipeline 的每个阶段（Refine → Review → Finalize）更新任务进度和日志。
- 前端通过 Tasks 页面或幕前状态栏显示当前 Pipeline 进度。

**依赖**: D2（Task System 升级完成后）。

**验收标准**:
- 用户在幕前触发"审校"后，可以在 Tasks 页面看到 Refine/Review/Finalize 的逐阶段进度。

---

## 领域 E: 体验层完善

### E1. 状态同步事件补全

**目标**: 消除状态同步盲区。

**范围**:
- 新增 `WorldBuildingCreated` / `WorldBuildingDeleted`（目前只有 `WorldBuildingUpdated`）。
- 新增 `StyleDnaUpdated`（风格 DNA 变更后同步）。
- 新增 `TaskCreated` / `TaskUpdated` / `TaskCompleted`（Task System 任务状态变更后同步）。
- 新增 `AnnotationResolved` / `AnnotationCreated`（批注变更后同步）。

**依赖**: D2（Task System 升级后，Task 事件才有意义）。

**验收标准**:
- 每个新增事件都有对应的 `StateSync::emit_xxx` 方法和前端 `useSyncStore` 监听逻辑。

---

### E2. Tasks 页面补全

**目标**: 从空壳页面变为功能完整的后台作业中心。

**范围**:
- 集成 `useTasks` Hook（已完整实现，只需页面接入）。
- 任务列表：显示任务类型、状态、进度、创建时间、关联故事。
- 任务详情：显示日志输出、结果预览（如 Cascade Rewrite 的 diff、Pipeline Review 的评分）。
- 操作按钮：取消进行中任务、重新失败任务。
- 实时更新：通过 `sync-event` 或 WebSocket 推送任务状态变更。

**依赖**: D2（Task System 升级后，页面才有数据可展示）。

**验收标准**:
- 用户可以在 Tasks 页面看到所有后台作业的完整生命周期。

---

### E3. CreationWizard 决策

**目标**: 明确创作向导是补全还是移除。

**范围**:
- **补全方案**: 集成 `generate_world_building_options`、`generate_character_profiles`、`generate_writing_styles`、`generate_first_scene`、`create_story_with_wizard` 命令，实现 7 步创作向导 UI。
- **移除方案**: 从 Sidebar 移除导航入口，保留后端命令以备未来使用（但移出 `generate_handler!`）。

**依赖**: 无（产品决策）。

**验收标准**: 明确决策并执行。

---

### E4. 幕前/幕后职责明晰化

**目标**: 强化双窗口设计的边界。

**范围**:
- **幕前（Frontstage）**: 仅保留沉浸式写作、AI 续写/Ghost Text、Pipeline 执行触发（Refine/Review/Finalize）。参数全部默认，无管理界面。
- **幕后（Backstage）**: 所有管理功能——故事/角色/场景/世界观的 CRUD、风格 DNA 调整、任务监控、知识图谱浏览、设置配置。
- **自动改写入口**: Cascade Rewriter 的预览和确认界面只在幕后提供。幕前用户修改正文时，系统静默触发 Ingest，但不触发级联改写（避免打断写作流）。
- **状态同步**: 幕后调整触发 `sync-event`，幕前窗口自动刷新（已部分实现，需补全 E1 的缺失事件）。

**依赖**: E1（状态同步事件补全后，双窗口联动更完整）。

**验收标准**:
- 新用户从幕前开始写作，无需理解任何管理概念。
- 专业用户在幕后调整设定后，幕前正文自动同步更新（通过级联改写或投影刷新）。

---

## 依赖关系图

```
B1 (Ingest 质量)
  └─→ B2 (记忆激活)
        └─→ D1 (Cascade Rewriter)
              └─→ D2 (Task 中心)
                    ├─→ D3 (Pipeline 追踪)
                    ├─→ E1 (事件补全)
                    └─→ E2 (Tasks 页面)

B3 (模型归一化)
  └─→ B4 (仓库归一化)
        └─→ B5 (db 解耦)
              └─→ C1 (lib.rs 拆分)
                    └─→ C2 (版本清理)

A1 (移除配额)
  └─→ A2 (补全断点)

A3 (Pipeline 内部化)
  └─→ A4 (休眠命令清理)
        └─→ A5 (Task 命令裁剪)
              └─→ C1 (lib.rs 拆分)

D2 (Task 中心)
  └─→ E4 (双窗口职责)
```

---

## 附录：待优先级排序的清单

| 编号 | 项目 | 领域 | 预估工作量 | 阻塞项 |
|------|------|------|-----------|--------|
| A1 | 移除配额计量 | A | 1 天 | 无 |
| A2 | 补全 4 个运行时断点 | A | 1-2 天 | A1 |
| A3 | Pipeline CRUD 内部化 | A | 2 天 | 无 |
| A4 | 休眠命令清理 | A | 2-3 天 | A3 |
| A5 | Task System 命令裁剪 | A | 1 天 | D2 |
| B1 | IngestPipeline 质量提升 | B | 3-5 天 | 无 |
| B2 | 记忆系统激活 | B | 3-5 天 | B1 |
| B3 | db 模型归一化 | B | 1-2 周 | 无 |
| B4 | db 仓库归一化 | B | 1 周 | B3 |
| B5 | db 模块解耦（Trait 层） | B | 1-2 周 | B4 |
| C1 | lib.rs 命令注册拆分 | C | 3-5 天 | A3, A4, A5 |
| C2 | 版本号注释清理 | C | 1 天 | B3, B4, C1 |
| C3 | memory ↔ creative_engine 解耦 | C | 2-3 天 | 无 |
| D1 | Cascade Rewriter | D | 1-2 周 | B2 |
| D2 | Task System 升级 | D | 1 周 | D1（架构对接） |
| D3 | Pipeline 纳入 Task 追踪 | D | 2-3 天 | D2 |
| E1 | 状态同步事件补全 | E | 2-3 天 | D2 |
| E2 | Tasks 页面补全 | E | 3-5 天 | D2 |
| E3 | CreationWizard 决策 | E | 1 天（决策）+ 1-2 周（补全） | 产品决策 |
| E4 | 双窗口职责明晰化 | E | 2-3 天 | E1 |

---

*计划草案完成，等待优先级排序和分批决策。*
