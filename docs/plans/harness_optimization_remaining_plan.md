# StoryMoss Harness 完善与优化执行计划

> 基于附件《基于 Harness 框架的完善与优化方案》的未落地项，制定本计划并逐项执行。当前版本 v0.25.1，目标版本 v0.26.0。

---

## 目标

将 StoryMoss 从“能生成”推进到“持续生成、可验证、可恢复”，补齐剩余 7 个方向的完善与优化：

1. Context Rot 防御剩余项（分层摘要、预算可视化）
2. 验证循环前置（P1）
3. 工具懒加载与动态范围（P1）
4. 文件系统工作空间（P1）
5. 子代理协作模型（P2）
6. 数据飞轮与共同进化（P2）
7. Harness 可观测性（P2）

---

## 1. Context Rot 防御补齐

### 1.1 分层摘要降级（P0 剩余）

**实现位置**：`src-tauri/src/agents/orchestrator.rs` 的 `build_continuation_context`

**目标**：在续写上下文构建时，按场景距离分三级摘要：

- 远端场景（> 当前章前 3 章）：一句话摘要
- 中距离场景（前 2-3 章）：3 句话摘要
- 当前场景：完整内容

**步骤**：

1. 新增 `SceneDistance` 枚举：`Current`、`Near`、`Far`。
2. 新增 `summarize_scene_by_distance` 函数：
   - `Current` → 返回 `scenes.content` 尾部 2000 字。
   - `Near` → 从 `scene_commits.summary_text` 取 300 字，若不存在则用 LLM 压缩为 3 句话（使用工具模型，标记为后台）。
   - `Far` → 将 `summary_text` 压缩为 1 句话（规则或 LLM）。
3. 改造 `build_continuation_context`：按距离分类场景，拼接三段上下文。
4. 新增单元测试覆盖远/中/近场景。

**验收**：`cargo test --lib` 新增测试通过；`build_continuation_context` 输出长度随距离递减。

### 1.2 上下文预算可视化（P0 剩余）

**实现位置**：

- 后端：`src-tauri/src/creative_engine/context_prioritizer.rs`（已有 `ContextHealthMetrics`）
- 后端命令：`src-tauri/src/commands/orchestrator.rs` 或 `src-tauri/src/commands/diagnostics.rs` 新增 `get_context_health`（若已存在则扩展）
- 前端：`src-frontend/src/components/diagnostics/AgentDiagnosticsPanel.tsx` 或新建 `ContextHealthPanel.tsx`

**目标**：诊断卡片显示当前 Writer 提示词各模块 token 占用比例。

**步骤**：

1. 后端在 `ContextHealthMetrics` 中补充分模块 token（story / scene / style / assets / user_input 等）。
2. 在 `DiagnosticStore` 中记录 `context_health`，IPC 命令 `get_context_health` 返回结构化 JSON。
3. 前端诊断组件新增 `ContextHealthCard`：进度条显示 Critical/High/Normal/Background 的 token 占比。
4. 在 `build_writer_prompt` 完成后将 `ContextHealthMetrics` 写入 `DiagnosticStore`。

**验收**：前端诊断面板可见上下文健康指标；单元测试覆盖 metrics 序列化。

---

## 2. 验证循环前置（P1）

### 2.1 生成前约束门（Pre-Generation Gate）

**实现位置**：`src-tauri/src/agents/orchestrator.rs` 中 `execute_writer` / `smart_execute` 入口

**目标**：在调用 Writer 之前做轻量规则检查，避免生成后才发现不可用。

**检查项**：

1. 前文内容长度是否足够（避免重复开头）。若不足，提示用户或自动切换至“扩写”模式。
2. 是否存在活跃且未闭合的伏笔（`foreshadowings` 表 `status = 'active'`）。若有，注入“伏笔回收提示”。
3. 风格 DNA（`style_dna`）是否完整。若缺失，则降级到默认风格并记录告警。

**步骤**：

1. 新建 `PreGenerationGate` 结构体，包含 `check` 方法返回 `GateResult`。
2. 在 `execute_writer` 和 `smart_execute` 调用 Writer 前调用 `PreGenerationGate::check`。
3. 若检查失败但可恢复，返回 `GateResult::Warn`，并把提示追加到 `task.parameters` 的约束字段。
4. 若检查失败不可恢复，返回 `GateResult::Block` 并抛出 `AppError` 带 `ErrorSeverity::UserAction`。
5. 新增单元测试。

**验收**：生成前检查覆盖 3 项；测试验证 Warn/Block 分支。

### 2.2 生成中自检（In-Generation Self-Check）

**实现位置**：`src-tauri/src/creative_engine/sanitize.rs` 或 `src-tauri/src/agents/orchestrator.rs` 中 Call 3 后

**目标**：TriShot Call 3 输出后，按段落检查 AI 陈词、在世作者、世界观冲突，并触发 MiniRewrite。

**步骤**：

1. 新增 `MiniRewrite` 结构体，输入（段落、违规类型、修复提示），输出重写段落。
2. 新增 `InGenerationChecker`：
   - 按段落调用 `AntiAiCliche` 检查 27 个陈词；
   - 调用 `LivingAuthorGuard` 检查在世作者；
   - 调用 `WorldConsistencyChecker` 检查是否违反世界观规则（基于 `world_building.rules` 关键词匹配）。
3. 在 TriShot Call 3 输出后、返回前端前，运行 `InGenerationChecker`。
4. 若发现违规，仅重写违规段落，保留其他段落不变，避免整章重生成。
5. 记录 `mini_rewrite` 事件到 `WorkflowLogger`。

**验收**：单元测试覆盖陈词/在世作者/世界观违规检测与重写；不引入大量额外 token。

### 2.3 计算验证优先

**目标**：规则验证优先于 LLM-as-judge。

**步骤**：

1. 在 `AuditService` 中明确先执行 `check_continuity`、`check_character`、`check_style` 等规则检查；仅当规则检查无法判定且剩余预算足够时才调用 LLM 审计。
2. 在生成前约束门和生成中自检中，全部使用规则匹配，避免 LLM 调用。

**验收**：相关路径不依赖 LLM 完成基础验证。

---

## 3. 工具懒加载与动态范围（P1）

**实现位置**：`src-tauri/src/creative_engine/asset_capability_manifest.rs`、`prompts/synthesizer.rs`、`model_gateway/executor.rs`

**目标**：根据任务类型动态注入资产，避免 Writer 一次性接收过多创作资产。

**步骤**：

1. 将 `AssetCapabilityManifest` 从启动时全量构建改为懒加载服务：
   - 保留启动时索引，但延迟渲染完整提示词内容；
   - 新增 `load_for_task(task_type, story_id, genre, scene_id)` 接口。
2. 定义任务类型：`Continuation`、`Rewrite`、`Genesis`、`Audit`、`Insight`。
3. 按任务类型选择资产子集：
   - 续写：风格摘要、近章摘要、2-3 个相关角色、1-2 个未闭合伏笔；
   - 改写：风格 DNA、选中段上下文、Anti-AI 规则；
   - 创世：体裁画像、方法论文、四元组。
4. 在 `GatewayRequest` 中透传 `task_type`，在 `ModelGateway` 调度时按任务类型选择资产标签。
5. 在提示词合成器 `synthesize_prompt` 中，仅把选定资产注入 system prompt，未选定资产不进入 Writer 提示词。

**验收**：`AssetCapabilityManifest` 不再在启动时渲染全部文本；测试验证不同任务类型返回不同资产集合。

---

## 4. 文件系统工作空间（P1）

**实现位置**：`src-tauri/src/workspace/` 新建模块，IPC 命令在 `src-tauri/src/commands/workspace.rs`

**目标**：为每个故事/项目生成 `.storymoss/` 工作空间，并支持 Git 版本化。

**步骤**：

1. 新增 `WorkspaceService`：
   - 根据故事 ID 获取项目目录（`app_data_dir / stories / {story_id}`）。
   - 初始化 `.storymoss/` 目录。
2. 在 `.storymoss/` 下生成：
   - `AGENTS.md`：角色、目标、规则（从 `AppConfig` 和当前故事元数据生成）。
   - `MEMORY.md`：跨会话记忆摘要（从 `KnowledgeGraphRepository` 和 `scene_commits` 聚合）。
   - `LOOPS.md`：当前进行中任务状态（在 `WorkflowScheduler` 任务启动/完成时更新）。
   - `PROGRESS.md`：已完成章节摘要（每次 `ChapterCommitted` 后追加）。
3. 初始化 Git：
   - 若目录无 `.git`，执行 `git init`。
   - 在每次生成完成后（`ChapterCommitted`）自动 `git add .storymoss/` 并 `git commit -m "chore: update storymoss workspace after chapter commit"`。
4. 新增 IPC 命令：
   - `get_workspace_files(story_id)` 返回 `.storymoss/` 下文件内容；
   - `sync_workspace_memory(story_id)` 将数据库记忆写入 `MEMORY.md`。

**验收**：创建新故事时自动生成 `.storymoss/` 和 Git 仓库；每次提交后新增 commit；`cargo test` 通过。

---

## 5. 子代理协作模型（P2）

**实现位置**：`src-tauri/src/agents/subagents/` 新建模块

**目标**：引入 ContinuityAgent、StyleAgent、WorldAgent，主 Writer 生成后异步审查并返回 ReviewNotes。

**步骤**：

1. 新建 `Subagent` trait：定义 `async fn review(&self, context: &AgentContext, content: &str) -> ReviewNotes`。
2. 实现三个子代理：
   - `ContinuityAgent`：检查跨章节/场景一致性、伏笔回收、角色状态一致性；
   - `StyleAgent`：检查句长、对话比例、比喻密度、内心独白比例与风格 DNA 偏差；
   - `WorldAgent`：检查世界观规则、设定冲突、地理/时间一致性。
3. 在 `AgentOrchestrator` 的 `generate` 成功后，启动 `tokio::spawn` 并发执行三个子代理审查。
4. 将 `ReviewNotes` 写入 `DiagnosticStore` 和 `.storymoss/LOOPS.md`。
5. 若 ReviewNotes 包含 HIGH 级别问题，触发 `RevisionSuggested` SyncEvent，前端在审阅面板展示。

**验收**：三个子代理存在并返回结构化 ReviewNotes；并发执行不阻塞主流程；单元测试覆盖核心规则。

---

## 6. 数据飞轮与共同进化（P2）

**实现位置**：`src-tauri/src/commands/intent.rs`（`record_feedback`）、`src-tauri/src/learning/` 新建模块

**目标**：收集用户接受/拒绝/修改反馈，沉淀为偏好训练数据。

**步骤**：

1. 扩展 `RecordFeedbackRequest` 和 `UserFeedbackLog`：
   - 新增 `original_prompt`（来自 `DiagnosticStore` 的最后提示词）；
   - 新增 `generated_content`；
   - 新增 `subsequent_edit_diff`（在 `update_scene` 后计算与生成内容的文本差异）。
2. 在 `FrontstageApp` 接受/拒绝续写时，捕获当前提示词与生成的 `final_content`，写入 `user_feedback_log`。
3. 新增 `PreferencePairExporter`：
   - 每 N 条反馈或定时任务，将 `user_feedback_log` 导出为 `preference_pairs.jsonl`；
   - chosen = 用户编辑后最终内容；rejected = 原始生成内容。
4. 在 `.storymoss/` 下生成 `feedback/` 目录保存 `preference_pairs.jsonl`。

**验收**：`record_feedback` 记录完整字段；`preference_pairs.jsonl` 可正确生成；测试通过。

---

## 7. Harness 可观测性（P2）

**实现位置**：`src-tauri/src/tracing/` 新建模块、前端诊断面板

**目标**：每次生成拥有 trace_id，记录全链路耗时、token、工具调用、上下文预算、错误恢复。

**步骤**：

1. 新增 `GenerationTrace`：
   - 包含 `trace_id`（UUIDv7）、`request_id`、`story_id`、步骤列表（step name / start / end / tokens / status）。
2. 在 `GatewayExecutor::generate` 中记录每个候选模型的探测、调用、失败、恢复事件。
3. 在 `PlanExecutor` / `smart_execute` 中记录每个步骤的耗时与 token。
4. 将 `trace_id` 加入 `ErrorResponse` 和 `LlmGeneratingProgress` 的 IPC 负载，前端可以追溯。
5. 新增前端 `TracingPanel` 组件：
   - 在诊断卡片中增加“Tracing”标签页；
   - 展示 TriShot 调用链（Call 1 → Call 2 → Call 3）的耗时、模型、token、错误恢复。
6. 将 trace 数据写入 `logs/traces/` 或 SQLite `generation_traces` 表。

**验收**：每次生成有 trace_id；`GenerationTrace` 可序列化；前端 Tracing 面板可见调用链。

---

## 8. 验证与交付

### 测试矩阵

| 检查项 | 命令 |
|---|---|
| Rust 编译 | `cargo check --lib` |
| Rust 单元测试 | `cargo test --lib` |
| 前端类型检查 | `cd src-frontend && npx tsc --noEmit` |
| 前端格式检查 | `npm run format:check` |
| Rust 格式化 | `cargo +nightly fmt -- --check` |
| E2E（可选） | `npm test` |

### 版本与提交

- 完成全部计划后统一版本号为 **v0.26.0**。
- 提交信息：`feat: v0.26.0 Harness 完善与优化（Context Rot/验证/懒加载/工作空间/子代理/数据飞轮/Tracing）`
- 按项目规范新建 tag：`v0.26.0` 并推送。

---

## 执行顺序

1. 补齐 Context Rot（1.1 分层摘要、1.2 预算可视化）。
2. 验证循环前置（2.1 生成前约束门、2.2 生成中自检、2.3 计算验证优先）。
3. 工具懒加载与动态范围（3）。
4. 文件系统工作空间（4）。
5. 子代理协作模型（5）。
6. 数据飞轮与共同进化（6）。
7. Harness 可观测性（7）。
8. 验证、更新文档、版本号统一、提交/tag。
