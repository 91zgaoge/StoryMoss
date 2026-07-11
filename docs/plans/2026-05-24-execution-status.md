# StoryMoss 全面完善计划 — 执行状态追踪

**更新日期**: 2026-05-24
**基准计划**: [2026-05-23-comprehensive-improvement-plan.md](./2026-05-23-comprehensive-improvement-plan.md)
**目的**: 防止上下文丢失，记录已完成的实际改动和剩余任务清单

---

## 执行摘要

本次执行会话完成了 **11 个完整领域项 + 1 个领域项的深入工作**，涉及约 35 个文件的实质性修改。

新增完成项：
- **E1 续**: 后端全部 sync 事件接入 + 前端 `useSyncStore` 补全监听
- **C2 续**: `commands_v3.rs` → `story_commands.rs` 重命名 + 全库版本号注释清理
- **E4**: Frontstage/Backstage 职责边界确认 + Stories.tsx 无效路由修复
- **C1**: `lib.rs` 命令注册拆分 → `handlers.rs` 独立文件
- **A1 续**: migration 65 修复 `ai_usage_quota` 表存在性检查 → `cargo test` 全绿（250 passed, 0 failed）
- **D1 Phase 1-2**: Cascade Rewriter 设计文档 + 模块骨架 + `entity_mentions` 表（Migration 73）+ Task System 集成 + Rewrite Engine（LLM Prompt 构建 + 段落提取 + 一致性验证）+ `trigger_cascade_rewrite` Tauri command
- **D1 Phase 3**: Backstage Diff 预览面板 + 用户确认流程（后端命令 + 前端 React 组件）
- **D1 Phase 4 续**: `update_character_relationship` 级联改写自动触发 hook
- **B3-续**: `models.rs` → `models_v3.rs` 完整迁移（~45 个结构体/enum）

---

## 已完成项目

### A1. 移除配额计量系统 ✅（测试回归修复）
**状态**: 已完成（代码已不存在）
- 前端 `useSubscription.ts` 已改为订阅层级功能门控（Pro/Enterprise 解锁全部功能）
- 后端配额检查代码（`check_platform_quota`、`auto_write_quota` 等）已完全移除
- **本次修复**: migration 65 在 `db/connection.rs` 中添加 `ai_usage_quota` 表存在性检查，避免新数据库/测试环境运行时 `ALTER TABLE` 报错
- 验收: `cargo check` 无残留配额相关代码，`cargo test` 全部通过
**状态**: 已完成（代码已不存在）
- 前端 `useSubscription.ts` 已改为订阅层级功能门控（Pro/Enterprise 解锁全部功能）
- 后端配额检查代码（`check_platform_quota`、`auto_write_quota` 等）已完全移除
- 验收: `cargo check` 无残留配额相关代码

### A3. Pipeline 数据管理命令内部化 ✅
**状态**: 已完成
- 删除 `src-tauri/src/commands_pipeline.rs`（原 37 个命令，已精简为 6 个并迁移）
- 迁移清单:
  - `get_story_chapter_drafts` → `pipeline/commands.rs`
  - `get_latest_pipeline_review` → `pipeline/commands.rs`
  - `get_story_llm_calls` / `get_recent_llm_calls` / `get_llm_call_stats` → `llm/commands.rs`
  - `update_character_state` → `commands_v3.rs`
- 更新 `lib.rs` 中 `generate_handler!` 注册
- 验收: `cargo check` 通过，前端调用不受影响

### A4. 休眠命令清理 ✅（大幅推进）
**状态**: 核心完成
**删除的函数定义**:
| 命令 | 所在文件 | 删除原因 |
|------|----------|----------|
| `auto_write_cancel` | `agents/commands.rs` | 前端无调用者 |
| `auto_revise_cancel` | `agents/commands.rs` | 前端无调用者 |
| `get_skills_by_category` | `commands/skill.rs` | 前端无调用者 |
| `analyze_story_structure` | `commands/orchestrator.rs` | 前端无调用者 |
| `get_task` | `task_system/commands.rs` | 前端无调用者 |
| `get_scene_characters` | `commands_v3.rs` | 前端无调用者 |
| `add_scene_character` | `commands_v3.rs` | 前端无调用者 |
| `remove_scene_character` | `commands_v3.rs` | 前端无调用者 |
| `set_scene_characters` | `commands_v3.rs` | 前端无调用者 |
| `get_character_scenes` | `commands_v3.rs` | 前端无调用者 |
| `apply_chapter_commit` | `commands/story_system.rs` | 前端无调用者 |
| `get_scene_commits` | `commands/story_system.rs` | 前端无调用者 |
| `get_feature_usage_stats` | `commands/anti_ai.rs` | 前端无调用者 |
- 清理 `lib.rs` 中所有"休眠命令"注释行
- 验收: 后端注册命令数从 ~170 降至 ~150，每个保留命令均有前端调用者

### B1. IngestPipeline 质量提升 ✅
**状态**: 已完成
**改动文件**: `src-tauri/src/memory/ingest.rs`
- **Prompt 工程**: `analyze_content()` prompt 增加 few-shot 示例（林枫/青云山）、严格实体类型枚举、反幻觉规则（"禁止编造未命名实体"）
- **Schema 严格化**: 新增 `validate_content_analysis()` 和 `validate_generated_knowledge()`，检查实体类型白名单、字段范围、空值
- **重试机制**: `analyze_content()` 和 `generate_knowledge()` 添加最多 3 次重试，失败时向 prompt 附加修正要求
- **实体链接**: 新增 `link_entities()` + `load_existing_entities()`，按名称匹配已有实体，合并属性，复用已有 ID
- **Bug 修复**: `convert_relations()` 中 `source_id`/`target_id` 原误用实体名称，现已使用链接后的实体 UUID
- 新增辅助函数: `merge_json_objects()`
- 验收: `cargo check` 通过，195 测试通过无新增回归

### B2. 记忆系统激活 ✅
**状态**: 已完成
**改动文件**:
- `src-tauri/src/agents/context_optimizer.rs`
- `src-tauri/src/commands_v3.rs`
- `src-tauri/src/memory/mod.rs`
- `src-tauri/src/memory/ingest.rs`
- 在 3 个 `AgentContext` 构造点注入 `MemoryOrchestrator::build_memory_pack()`，将 `memory_pack` 从硬编码 `None` 改为真实组装的三层记忆
- 降级保护: 构建失败时记录 warn 并回退到 `None`，不阻断生成流程

### B3. db 模型归一化 ✅
**状态**: 已完成
- 删除 `db/models.rs` 中未使用的结构体: `OAuthUrlResponse`, `AuthConfig`
- 删除 `db/repositories.rs` 中未使用的方法: `batch_update_states`, `find_by_id`, `find_by_email`, `find_session_by_token`, `delete_user_sessions`, `cleanup_expired_sessions`, `get_by_session`
- 同步 `db/connection.rs` 中 `kg_entities` 和 `kg_relations` 的 schema，添加缺失列:
  - `kg_entities`: `confidence_score`, `access_count`, `last_accessed`, `is_archived`, `archived_at`
  - `kg_relations`: `confidence_score`
- **本次完成**: `models.rs` 中全部 ~45 个结构体/enum 迁移至 `models_v3.rs`，`models.rs` 改为重新导出兼容层（`pub use super::models_v3::{...}`）
- 零命名冲突验证通过（`models.rs` 与 `models_v3.rs` 无重叠类型名）
- 所有现有引用（`crate::db::models::X`、`crate::db::models_v3::X`、`crate::db::X`）继续有效
- **剩余工作**: 无（模型归一化已完成，后续可考虑删除 `models.rs` 兼容层）

### C2. 版本号注释清理 ✅
**状态**: 已完成（文件重命名 + 注释清理）
- 文件重命名: `commands_v3.rs` → `story_commands.rs`，`lib.rs` 中所有引用同步更新
- 清理 `story_commands.rs` 中 `[commands_v3]` log 前缀为 `[story_commands]`
- 批量清理以下文件中的版本号注释:
  - `db/connection.rs`: 移除 `(v4.0 - ...)`、`-- v0.7.3:`、`// ==================== v6.0.0/v7.0.0:`
  - `db/models_v3.rs`: `//! V3 架构数据模型` → `//! 数据模型`
  - `db/repositories_v3.rs`: `//! V3 架构 Repository 层` → `//! Repository 层`
  - `db/repositories_narrative.rs`, `narrative/audit.rs`, `planner/template_learning.rs`, `planner/bootstrap.rs`
  - `telemetry/mod.rs`, `auth/mod.rs`, `error.rs`, `subscription/mod.rs`
  - `agents/mod.rs`, `agents/service.rs`, `agents/orchestrator.rs`
  - `memory/query.rs`, `creative_engine/context_builder.rs`, `creative_engine/style/mod.rs`
  - `analytics/mod.rs`, `task_system/scheduler.rs`, `tests/mod.rs`, `db/repositories.rs`
  - `lib.rs`, `llm/service.rs`, `book_deconstruction/executor.rs`
- **剩余工作**: 覆盖旧 `models.rs`（需等 B3 模型归一化完成后执行）

### E1. 状态同步事件补全 ✅
**状态**: 已完成（后端发射器全部接入）
**改动文件**:
- `state_sync/events.rs`: 新增事件变体
  - `WorldBuildingCreated` / `WorldBuildingDeleted`
  - `StyleDnaUpdated`
  - `TaskCreated` / `TaskUpdated` / `TaskCompleted`
  - `AnnotationCreated` / `AnnotationResolved`
- `state_sync/service.rs`: 新增 `emit_xxx` 方法 + 事件名称映射；所有方法泛型化（支持 `AppHandle<R>`）
- `commands_v3.rs`: `create_world_building` → `WorldBuildingCreated`，`delete_world_building` → `WorldBuildingDeleted`，`create_scene_annotation` → `AnnotationCreated`，`resolve_scene_annotation` → `AnnotationResolved`
- `commands/anti_ai.rs`: `evolve_style_from_anti_ai_review` → `StyleDnaUpdated`
- `task_system/service.rs`: `create_task` → `TaskCreated`，`update_task`/`cancel_task` → `TaskUpdated`，`run_task_internal` → `TaskCompleted`
- `state_sync/service.rs`: 所有 `emit_xxx` 方法泛型化为 `<R: Runtime>`，支持 `AppHandle<R>`
- 前端 `useSyncStore.ts`: 补全 `worldBuildingCreated/Deleted`、`styleDnaUpdated`、`taskCreated/Updated/Completed`、`annotationCreated/Resolved` 的缓存刷新逻辑；`default` 分支改为 `console.warn` 安全降级，避免运行时崩溃
- 前端 `generated/SyncEvent.ts`: 重新生成，包含全部 29 个事件变体
- 前端 `Stories.tsx`: 修复 `handleWizardCreate` 中无效的 `setCurrentView('creation-wizard')`，改为 toast 提示
- **剩余工作**: 无（前后端同步闭环已完成）

### E4. Frontstage/Backstage 职责明晰化 ✅
**状态**: 核心完成
- Frontstage（幕前）: 仅保留沉浸式写作、AI 续写/Ghost Text、Pipeline 执行触发（Refine/Review/Finalize），无管理界面
- Backstage（幕后）: 包含故事/角色/场景/世界观 CRUD、风格 DNA 调整、任务监控、知识图谱浏览、设置配置
- 状态同步: 幕后调整通过 `sync-event` 自动同步到幕前，`useSyncStore` 已覆盖全部事件
- Stories.tsx 中修复无效 `creation-wizard` view 切换
- **剩余工作**: Cascade Rewriter 预览入口（依赖 D1 完成后在幕后添加）

### C1. lib.rs 命令注册拆分 ✅
**状态**: 已完成
**改动文件**:
- `src-tauri/src/lib.rs`: `.invoke_handler(tauri::generate_handler![...])` → `.invoke_handler(include!("handlers.rs"))`
- 新增 `src-tauri/src/handlers.rs`: 包含完整的 `tauri::generate_handler![...]` 命令列表（~150 个命令）
**方案说明**:
- 经实验验证，`generate_handler!` 过程宏不支持在其参数列表中嵌套 `macro_rules!` 宏调用或 `include!`
- 可行的唯一纯 Rust 方案：将完整的 `tauri::generate_handler![...]` 调用移入独立文件 `handlers.rs`，在 `lib.rs` 中通过 `include!("handlers.rs")` 引用
- `include!` 在编译时先展开，插入的 token 再被 `generate_handler!` 过程宏处理
- 新增命令不再需要修改 `lib.rs`，只需编辑 `handlers.rs`
- `lib.rs` 中命令注册块从 ~250 行缩减为 1 行

### B4. db 仓库归一化 ✅
**状态**: 已完成
**改动文件**:
- `src-tauri/src/db/repositories_v3.rs`: 追加迁移自 `repositories.rs` 的 5 个仓库（`StoryRepository`, `CharacterRepository`, `ChapterRepository`, `UserRepository`, `GenesisRunRepository`）
- `src-tauri/src/db/repositories.rs`: 重写为兼容层（`pub use super::repositories_v3::{...}`）
- `src-tauri/src/db/repositories_v3.rs`: 为 `CharacterRelationshipRepository` 补全 `get_by_id` 方法
**说明**:
- 全部 5 个旧仓库已迁移至 `repositories_v3.rs`，`repositories.rs` 仅保留重新导出
- 零命名冲突验证通过，所有现有引用继续有效
- 后续可考虑删除 `repositories.rs` 兼容层

### B5. db 模块解耦（Repository Trait 层）✅
**状态**: 核心完成
**改动文件**:
- 新增 `src-tauri/src/db/traits.rs`: 定义 6 个核心 Repository Trait（`SceneRepo`, `StoryRepo`, `CharacterRepo`, `ChapterRepo`, `WorldBuildingRepo`, `WritingStyleRepo`）
- `src-tauri/src/db/repositories_v3.rs`: 为 6 个具体 Repository struct 实现对应 Trait
- `src-tauri/src/db/mod.rs`: 导出 traits 模块及核心 trait
- `src-tauri/src/story_commands.rs`:
  - 移除 `use crate::db::*` 通配符导入，改为精确导入（模型 + 具体仓库 + Trait）
  - 提取 `get_story_scenes_core(repo: &dyn SceneRepo, story_id: &str)` 业务逻辑函数
  - 新增 `#[cfg(test)]` 模块，包含手动 mock `SceneRepo` 实现及 2 个单元测试
**验收**:
- `story_commands.rs` 不再使用 `use crate::db::*`
- `cargo test` 通过，包含 2 个使用 mock 仓库的 trait 测试
- `cargo test --package storymoss`: 252 passed, 0 failed

---

## 剩余任务清单

### 高优先级（可立即执行）

| 编号 | 项目 | 领域 | 阻塞项 | 预估工作量 |
|------|------|------|--------|-----------|
| （无） | — | — | — | — |

### E2. Tasks 页面补全 ✅
**状态**: 核心完成
**改动文件**: `src-frontend/src/pages/Tasks.tsx`
- 任务类型下拉框补全：新增 `ai_generation`、`pipeline_review`、`ingest` 选项
- 失败任务支持一键重试：新增 `handleRetry` + 黄色重试按钮（复用 `trigger_task`）
- Pipeline Review 结果增强：解析 `overall_score` 并以彩色百分比徽章展示（≥80% 绿色 / ≥60% 黄色 / <60% 红色）
**验收**: `npm run build` 通过，无新增报错

### D2. Task System 升级为全局后台作业中心 ✅
**状态**: 核心基础设施完成
**改动文件**:
- `src-tauri/src/agents/executor.rs`: 新增 `AiGenerationExecutor`，将 AgentOrchestrator 接入 Task System
- `src-tauri/src/pipeline/executor.rs`: 新增 `PipelineReviewExecutor`，将 Pipeline refine/review/finalize 接入 Task System
- `src-tauri/src/task_system/commands.rs`: 新增便捷命令 `run_ai_generation_task`、`run_pipeline_task`
- `src-tauri/src/agents/mod.rs`: 暴露 `executor` 模块
- `src-tauri/src/pipeline/mod.rs`: 暴露 `executor` 模块
- `src-tauri/src/lib.rs`: 注册两个新执行器到 TaskService
- `src-tauri/src/handlers.rs`: 注册两个新便捷命令
**设计要点**:
- `AiGenerationExecutor` 解析 payload 构建 `AgentTask`，通过 `AgentOrchestrator` 执行，支持 Full/Fast 模式
- `PipelineReviewExecutor` 支持 operation 字段分发到 refine/review/finalize，通过 `TaskPipelineCallbacks` 将 pipeline 进度回写到 TaskExecutionContext
- 便捷命令封装 payload 构造和 `CreateTaskRequest`，前端可直接调用创建任务
**验收**: `cargo check` 通过，`cargo test` 250 passed / 0 failed

### D1. Cascade Rewriter ✅
**状态**: Phase 1-4 全部完成
**设计文档**: [2026-05-24-cascade-rewriter-design.md](./2026-05-24-cascade-rewriter-design.md)
**已完成**:
- 设计文档：架构、数据模型、执行流程、集成点、验收标准
- 模块骨架：`creative_engine::cascade_rewriter`（`mod.rs` + `models.rs` + `repository.rs` + `change_detector.rs` + `impact_analyzer.rs` + `rewrite_engine.rs` + `executor.rs` + `commands.rs`）
- 数据库迁移 73：`entity_mentions` 表 + 索引
- Task System 集成：`TaskType::CascadeRewrite` 枚举扩展 + `CascadeRewriteExecutor` 注册 + `trigger_cascade_rewrite` Tauri command
- Phase 2: Rewrite Engine 实现（段落提取 + Prompt 构建 + LLM 调用 + 长度/实体保留验证）
- Phase 3: Backstage Diff 预览面板 + 用户确认流程
  - 后端命令：`get_cascade_rewrite_result`、`apply_cascade_rewrite`（应用改写到 scene content）、`reject_cascade_rewrite`
  - 前端组件：`Tasks.tsx` 中新增 `CascadeRewriteDetail`，展示 `original_text` vs `rewritten_text` 对比，支持单条/批量 接受/拒绝
- Phase 4: ChangeDetector 自动 hook 到实体更新命令
  - `commands/character.rs` `update_character`: 对比 `personality`/`goals`/`appearance`/`background`，变更时自动创建 `CascadeRewrite` Task
  - `story_commands.rs` `update_world_building`: 对比 `concept`/`history`/`rules`/`cultures`，变更时自动创建 `CascadeRewrite` Task
  - `story_commands.rs` `update_character_relationship`: 对比 `relationship_type`/`description`/`dynamic`，变更时自动创建 `CascadeRewrite` Task（通过两个角色的 mentions 间接触发）
  - `db/repositories_v3.rs` `WorldBuildingRepository`: 新增 `get_by_id` 方法
  - `db/repositories_v3.rs` `CharacterRelationshipRepository`: 新增 `get_by_id` 方法
- Phase 4: `entity_mentions` 的自动构建 —— `update_scene` Ingest Pipeline 中批量保存实体后，自动提取实体名称在场景文本中的出现位置并写入 `entity_mentions` 表
**剩余工作**: 无（D1 级联改写闭环已完成）

### 中优先级

| 编号 | 项目 | 领域 | 阻塞项 | 预估工作量 |
|------|------|------|--------|-----------|
| E3 | CreationWizard 决策 | E | 产品决策 | 1 天 + 1-2 周 |

### A5. Task System 命令保留决策 ✅
**状态**: 已验证，无需改动
**说明**:
- 保留命令：`list_tasks`, `get_task_logs`, `cancel_task` — 均有前端调用者（Tasks 页面）
- `create_task`, `update_task`, `delete_task`, `trigger_task` — 均有前端调用者（Tasks 页面管理功能）
- `run_ai_generation_task`, `run_pipeline_task` — 有前端调用者（便捷命令）
- 内部系统（Cascade Rewriter, Pipeline, Character update）已直接使用 `TaskService::create_task()` 等方法，不经过 IPC
- 结论：Task System IPC 边界已对齐，无需裁剪

### 已确认非任务（无需执行）

| 编号 | 项目 | 说明 |
|------|------|------|
| A2 | 补全 4 个运行时断点 | 经检查，4 个命令后端均有实现且前端有调用者，非断点 |
| C3 | memory ↔ creative_engine 解耦 | 经检查，`memory/` 目录下无 `crate::creative_engine` 直接引用，已完成 |

---

## 关键代码位置速查

| 功能 | 文件路径 | 备注 |
|------|----------|------|
| IngestPipeline | `src-tauri/src/memory/ingest.rs` | 新增重试+实体链接 |
| MemoryPack 注入 | `src-tauri/src/agents/context_optimizer.rs` | `build_full_context()` 中组装 |
| Pipeline 命令 | `src-tauri/src/pipeline/commands.rs` | 新增 2 个查询命令 |
| LLM 统计命令 | `src-tauri/src/llm/commands.rs` | 新增 3 个统计命令 |
| State Sync 事件 | `src-tauri/src/state_sync/events.rs` | 新增 8 个事件变体 |
| State Sync 发射器 | `src-tauri/src/state_sync/service.rs` | 新增 8 个 `emit_xxx` 方法 |
| 命令注册 | `src-tauri/src/lib.rs` | `include!("handlers.rs")` |
| 命令列表 | `src-tauri/src/handlers.rs` | 包含全部 ~150 个命令注册 |
| Cascade Rewriter | `src-tauri/src/creative_engine/cascade_rewriter/` | 设计完成，模块骨架 + migration 73 |
| Entity Mention 索引 | `src-tauri/src/db/connection.rs` | Migration 73: `entity_mentions` 表 |
| ChangeDetector Hook | `src-tauri/src/commands/character.rs` | `update_character` 敏感字段对比 + 自动创建 Task |
| ChangeDetector Hook | `src-tauri/src/story_commands.rs` | `update_world_building` 敏感字段对比 + 自动创建 Task |
| Entity Mention 自动构建 | `src-tauri/src/story_commands.rs` | `update_scene` Ingest Pipeline 中自动提取实体引用索引 |
| TaskType 扩展 | `src-tauri/src/task_system/models.rs` | 新增 `AiGeneration`/`PipelineReview`/`Ingest` |
| Tasks 页面增强 | `src-frontend/src/pages/Tasks.tsx` | 任务类型选择 + 执行结果 JSON 预览 |

---

## 回归测试基线

- `cargo check`: 通过（190 warnings，主要是未使用代码警告）
- `cargo test --package storymoss`: **252 passed, 0 failed**
- `npm run build`: 通过
- **无新增回归**

---

## 本次会话新增完成项

| 项目 | 说明 |
|------|------|
| D1 Phase 3 | Backstage Diff 预览面板：`get_cascade_rewrite_result`/`apply_cascade_rewrite`/`reject_cascade_rewrite` 后端命令 + 前端 `CascadeRewriteDetail` 组件 |
| D1 Phase 4 续 | `update_character_relationship` 级联改写自动触发（对比 relationship_type/description/dynamic） |
| B3-续 | `models.rs` → `models_v3.rs` 完整迁移（~45 个结构体/enum），`models.rs` 改为重新导出兼容层 |
| D1 Phase 4 | `update_character` / `update_world_building` 级联改写自动触发 |
| D1 Phase 4 | `entity_mentions` 自动构建（`update_scene` Ingest Pipeline） |
| D2 核心 | `AiGenerationExecutor` + `PipelineReviewExecutor` + 注册到 TaskService |
| D2 便捷命令 | `run_ai_generation_task` + `run_pipeline_task` Tauri commands |
| D3 Pipeline 追踪 | `run_refine`/`run_review`/`run_finalize`/`repair_finalize` 自动创建 Task 并回写进度/结果 |
| E2 补全 | Tasks 页面：新增任务类型、重试按钮、Pipeline Review 评分徽章 |
| B3 启动 | `DynamicTrait` 从 `models.rs` 迁移到 `models_v3.rs` |
| 编译清理 | 移除 10+ 个 unused import/warning |
| B4 完成 | `repositories.rs` → `repositories_v3.rs` 迁移（5 个旧仓库），`repositories.rs` 改为兼容层 |
| B5 完成 | 新增 `db/traits.rs`（6 个核心 Repository Trait），`story_commands.rs` 移除 `use crate::db::*` 通配符，提取 `get_story_scenes_core` + mock 测试 |
| A5 验证 | Task System 命令边界已对齐：所有 IPC 命令均有前端调用者，内部系统已直接使用 `TaskService` 方法 |
| C3 验证 | `memory/` 目录下无 `crate::creative_engine` 直接引用，已解耦 |

**Git Commit**: `c926f9e`

*文档由执行会话自动生成，用于防止上下文丢失。*
