# StoryMoss 执行计划 — Grill-with-Docs 对齐产出

> 产出日期：2026-05-18
> 来源：`/grill-with-docs` 深度对齐 session
> 状态：设计已对齐，等待执行

---

## 一、已对齐的核心架构决策（已确认，无需再审）

### 1. 场景化叙事：1:N 聚合编辑

| 决策点 | 结论 |
|--------|------|
| 数据库关系 | `scenes` 表新增 `chapter_id` 外键；`chapters.scene_id` 保留为代表 Scene（首 Scene） |
| `sequence_number` | 保持 story 级别全局唯一；Chapter 内显示用 `ROW_NUMBER()` |
| 编辑器界面 | TipTap 自定义 `scene-divider` Node；每个 divider 显示 Scene 元数据卡片 |
| 保存策略 | 纯文本编辑 → 精准推送单个 Scene；结构调整（divider 增删改）→ 整章推送重切分 |
| Projection Writers | 底层（State/Index/Memory/Vector）Scene 粒度；Summary 两层（Scene 摘要 + Chapter 摘要按需聚合） |
| 拆分元数据 | Lazy 推断（`CHAPTER_COMMIT` 30s 防抖时统一生成） |
| 删除 divider | 软删除 + lazy 清理；30 秒内可撤销 |

### 2. 编排层：AgentOrchestrator 单一网关

| 决策点 | 结论 |
|--------|------|
| 单一网关 | `AgentOrchestrator` 是所有 AI 文本生成的唯一入口；废弃 `AgentService::execute_writer` |
| `GenerationMode` | 组合策略：默认自动判断 + 上层 `mode_override` 可覆盖 |
| 自动判断规则 | 1. `mode_override` → 2. `quality_required/publish_ready` → `Full` → 3. `ghost_text/inline_suggestion` → `Fast` → 4. `expected_output_length > 1000` → `Full` → 5. 输入长度 `< 200` → `Fast` → 6. 默认 `Full` |
| Hooks 迁移 | `BeforeAiWrite` / `AfterAiWrite` 从 `AgentService` 迁移到 `AgentOrchestrator::generate` 入口/出口 |

### 3. MemoryPack 注入

| 决策点 | 结论 |
|--------|------|
| `Fast` 模式 | 轻量上下文（`previous_chapters` 缓存），不走完整 `QueryPipeline` |
| `Full` 模式 | `StoryContextBuilder` 调用 `QueryPipeline` + `MemoryOrchestrator` → `MemoryPack` 注入 `AgentContext` |
| `previous_chapters` | 吸收进 `MemoryPack.working_memory` |

### 4. 错误处理体系

| 决策点 | 结论 |
|--------|------|
| 统一错误枚举 | `AppError`（`QuotaExceeded`、`LlmTimeout`、`DbLocked`、`ValidationFailed`、`ContextDegraded` 等） |
| 内部 API | 全部返回 `Result<T, AppError>` |
| IPC 序列化 | 结构化 JSON `{ code, message, data }` |
| Silent failure | 致命缺失 → `Err`；辅助缺失 → `AgentContext.warnings` + 前端降级提示 |

### 5. 配额与取消

| 决策点 | 结论 |
|--------|------|
| 配额检查 | 移入 `LlmService.generate()` / `stream_generate()` 最底层；所有上层自动继承 |
| 取消机制 | `generate_with_context_and_pipeline` 返回 `(request_id, Result)`；上层通过 `PipelineCallbacks` 暴露 `request_id` |

### 6. 三层质量系统联动

| 链路 | 机制 |
|------|------|
| Anti-AI → Pipeline Review | `text_annotations` (`anti_ai_flag`) 注入 `build_review_prompt` |
| Pipeline Review → Reading Power | Critical/high `review_issues` 自动创建 `ChaseDebt` |
| Reading Power → Pipeline Review | Pending `OverrideContract`s 注入 `review_focus` |
| Reading Power → Anti-AI | `StyleDNA` 偏差阈值触发 "style drift" 标记 |

### 7. Book Deconstruction 同构化

| 决策点 | 结论 |
|--------|------|
| 废弃表 | `reference_books`、`reference_characters`、`reference_scenes` |
| 统一目标 | `narrative_characters`、`narrative_scenes`、`narrative_world_buildings` |
| 来源标记 | 拆书 → `source: Extracted, status: Reference`；创世 → `source: Generated, status: Active` |
| 一键激活 | `UPDATE ... SET status = 'active'`（无需跨表复制） |

### 8. MCP 工具注册

| 决策点 | 结论 |
|--------|------|
| 动态注册 | `McpClient` 连接成功后自动注册 tools 到全局 `CapabilityRegistry` |
| PlanGenerator | 从动态 registry 读取可用能力（非静态列表） |
| PlanExecutor | 新增 `CapabilitySource::McpTool` 分发分支 |

---

## 二、执行任务清单

### 任务 1：更新 CONTEXT.md（已完成）

- **状态**：✅ 已完成
- **描述**：修正 `src-tauri/src/CONTEXT.md` 中过时的 Known gaps 描述
- **关键修改**：
  - Refine/Review 已改为 Working 状态
  - LLM 取消改为"底层已支持，上层未暴露 request_id"
  - CHAPTER_COMMIT 改为"v0.7.1 已解决"
  - Orchestration 改为"大部分已修复，剩余 execute_writer 废弃"

---

### 任务 2：废弃 `AgentService::execute_writer`

- **优先级**：P0（阻塞所有上层调用路径）
- **描述**：将 `AgentService::execute_writer` 的职责迁移到 `AgentOrchestrator`，确立单一网关
- **关键文件**：
  - `src-tauri/src/agents/service.rs`（删除 `execute_writer`）
  - `src-tauri/src/agents/orchestrator.rs`（接收 hooks）
  - `src-tauri/src/planner/executor.rs`（改为直接调 `AgentOrchestrator`）
  - `src-tauri/src/workflow/scheduler.rs`（`Revise` 节点改为直接调 `AgentOrchestrator`）
  - `src-tauri/src/commands_v3.rs`（所有 IPC 命令改为直接调 `AgentOrchestrator`）
- **验收标准**：
  - `AgentService::execute_writer` 方法不存在或被标记 `#[deprecated]`
  - `cargo grep execute_writer` 返回 0 处非测试调用
  - `AgentOrchestrator::generate` 成为所有 Writer 调用的唯一入口
  - Hooks 在 `AgentOrchestrator::generate` 中正确触发（BeforeAiWrite 在生成前，AfterAiWrite 在生成后）

---

### 任务 3：暴露 LLM 取消 `request_id` 到上层

- **优先级**：P0（用户体验，防止 token 浪费）
- **描述**：让前端能够取消同步生成任务
- **关键文件**：
  - `src-tauri/src/llm/service.rs`（`generate_with_context_and_pipeline` 返回 `(String, Result)`）
  - `src-tauri/src/agents/orchestrator.rs`（存储 `request_id` 到 `AgentTask.metadata`）
  - `src-tauri/src/pipeline/types.rs`（`PipelineCallbacks` 新增 `on_request_id`）
  - `src-frontend/src/services/tauri.ts`（`cancel_generation` 命令传 `request_id`）
- **验收标准**：
  - Bootstrap、PlanExecutor、WorkflowScheduler 长耗时操作均暴露 `request_id`
  - 前端点击"取消"后，后端 `tokio::select!` 正确中断，HTTP 连接关闭
  - 取消后 `cancel_senders` 正确清理，无内存泄漏

---

### 任务 4：定义 `AppError` 统一错误枚举

- **优先级**：P0（基础设施，影响所有模块）
- **描述**：替换 450 处 `map_err(|e| e.to_string())`
- **关键文件**：
  - `src-tauri/src/error.rs`（新增 `AppError` enum）
  - `src-tauri/src/llm/service.rs`（首批迁移）
  - `src-tauri/src/agents/orchestrator.rs`（迁移）
  - `src-tauri/src/lib.rs`（IPC 层序列化 `AppError` → JSON）
  - `src-frontend/src/services/error-handler.ts`（新增，匹配 code 渲染恢复 UI）
- **验收标准**：
  - `AppError` 定义至少包含：QuotaExceeded、LlmTimeout、DbLocked、ValidationFailed、ContextDegraded
  - IPC 返回 JSON 包含 `{ code, message, data }`
  - 前端对 QuotaExceeded 显示"升级"按钮，对 LlmTimeout 显示"检查模型配置"提示
  - 零 `map_err(|e| e.to_string())` 在核心路径（llm/agents/pipeline）中

---

### 任务 5：`style_analysis` PostProcess 补全

- **优先级**：P1（3-review Pipeline 唯一真实缺口）
- **描述**：实现 `run_style_analysis` 空壳函数
- **关键文件**：
  - `src-tauri/src/pipeline/post_process.rs`（实现 3 个 TODO）
  - `src-tauri/src/pipeline/style_analysis.rs`（已有 `should_trigger_style_analysis`）
  - `src-tauri/src/creative_engine/style/dna.rs`（`StyleDNA` 计算）
  - `src-tauri/src/db/repositories_v3.rs`（`StyleSnapshotRepository`）
- **验收标准**：
  - 每 5 章自动触发，读取最近 5 章定稿内容拼接
  - 调用 LLM 分析六维向量（句长、对话比、隐喻密度、内心独白、情感暴露、节奏）
  - 保存 `StyleSnapshot` 并计算与上一周期的 `StyleDnaDelta`
  - 更新 story 的 `writing_style` 字段

---

### 任务 6：1:N 聚合编辑数据库 schema 变更

- **优先级**：P1（大功能，但可独立实施）
- **描述**：实现 Chapter-Scene 1:N 关系
- **关键文件**：
  - `src-tauri/src/db/connection.rs`（Migration：`scenes` 表添加 `chapter_id`）
  - `src-tauri/src/db/repositories.rs`（`ChapterRepository`、`SceneRepository` 查询逻辑）
  - `src-tauri/src/db/repositories_story_system.rs`（`chapter_commits` 等表适配）
- **验收标准**：
  - `scenes` 表有 `chapter_id` 字段
  - `SELECT * FROM scenes WHERE chapter_id = ? ORDER BY sequence_number` 返回正确顺序
  - 现有 `chapters.scene_id` 保留，指向首 Scene（向后兼容）
  - `chapter_commits` 支持按 Chapter 粒度 commit（不强制绑定单个 scene_id）

---

### 任务 7：实现 TipTap `scene-divider` Node

- **优先级**：P1（依赖任务 6 完成）
- **描述**：Frontstage 聚合编辑界面
- **关键文件**：
  - `src-frontend/src/frontstage/`（TipTap schema 扩展）
  - `src-frontend/src/services/tauri.ts`（保存策略：精准推送 vs 整章推送）
  - `src-tauri/src/commands_v3.rs`（`update_scene` 精准推送、`update_chapter_aggregate` 整章推送）
- **验收标准**：
  - `scene-divider` 作为不可删除的块级 Node（除非显式"合并 Scene"操作）
  - 显示 Scene 元数据卡片（戏剧目标、冲突类型）
  - 插入 divider 即时完成；新 Scene 元数据 30s lazy 推断
  - 删除 divider 软删除 + 30s 可撤销

---

### 任务 8：MCP 工具动态注册

- **优先级**：P2（功能扩展）
- **描述**：打通 MCP 工具到 PlanExecutor
- **关键文件**：
  - `src-tauri/src/mcp/client.rs`（连接成功后注册 tools）
  - `src-tauri/src/capabilities/mod.rs`（全局 `CapabilityRegistry`）
  - `src-tauri/src/planner/mod.rs`（移除 `mcp.*` 过滤逻辑）
  - `src-tauri/src/planner/executor.rs`（新增 `McpTool` 分发）
- **验收标准**：
  - MCP 服务器连接后，`CapabilityRegistry` 包含动态注册的 capabilities
  - `PlanGenerator` prompt 包含 MCP 工具描述
  - `PlanExecutor` 正确分发 `CapabilitySource::McpTool` 到 `McpClient::call_tool`

---

### 任务 9：Book Deconstruction 存储同构化

- **优先级**：P2（技术债务，长期收益）
- **描述**：废弃 `reference_*` 表，统一使用 `narrative_*` 表
- **关键文件**：
  - `src-tauri/src/db/connection.rs`（Migration：数据迁移 + 表废弃）
  - `src-tauri/src/book_deconstruction/repository.rs`（写入目标改为 `narrative_*`）
  - `src-tauri/src/book_deconstruction/executor.rs`（标记 `source: Extracted, status: Reference`）
  - `src-tauri/src/memory/query.rs`（搜索范围扩展至 `Reference` 状态）
- **验收标准**：
  - `reference_books` 等表数据迁移到 `narrative_world_buildings`
  - 拆书产出标记正确
  - "一键 convert to story" 改为单表 UPDATE
  - `QueryPipeline` 检索结果包含 Reference 材料

---

## 三、执行优先级与依赖图

```
Wave 1: 基础设施（先做）
├── 任务 2: 废弃 execute_writer ──┬──→ 任务 3: 暴露 request_id
├── 任务 4: AppError ─────────────┤
└── 任务 5: style_analysis ───────┘

Wave 2: 1:N 聚合编辑
├── 任务 6: schema 变更 ──────────→ 任务 7: scene-divider

Wave 3: 功能扩展
├── 任务 8: MCP 注册
└── 任务 9: Book Deconstruction 同构化
```

**启动建议**：从 Wave 1 并行启动任务 2、4、5。任务 2 和 4 都是签名变更，影响面广但集中；任务 5 是独立功能补全。

---

## 四、关键代码锚点速查

| 概念 | 文件路径 | 行号/符号 |
|------|---------|----------|
| `AgentOrchestrator::generate` | `src-tauri/src/agents/orchestrator.rs` | line 129 |
| `AgentService::execute_writer` | `src-tauri/src/agents/service.rs` | line 451 |
| `execute_writer_raw` | `src-tauri/src/agents/service.rs` | line 322 |
| `cancel_senders` | `src-tauri/src/llm/service.rs` | line 69, 445 |
| `generate_with_context_and_pipeline` | `src-tauri/src/llm/service.rs` | line 190 |
| `stream_generate` | `src-tauri/src/llm/service.rs` | line 496 |
| `AntiAiReviewer::review` | `src-tauri/src/anti_ai/mod.rs` | line 60 |
| `review_draft` | `src-tauri/src/pipeline/review.rs` | line 12 |
| `refine_draft` | `src-tauri/src/pipeline/refine.rs` | line 12 |
| `run_style_analysis` | `src-tauri/src/pipeline/post_process.rs` | line 338 |
| `ReadingPowerEvaluator::evaluate` | `src-tauri/src/reading_power/evaluator.rs` | line 45 |
| `ElementSource` / `ElementStatus` | `src-tauri/src/narrative/elements.rs` | line 42, 53 |
| `Capability::from_mcp_tool` | `src-tauri/src/capabilities/mod.rs` | line 65 |
| `CapabilitySource::McpTool` | `src-tauri/src/capabilities/mod.rs` | line 111 |
| `scenes.chapter_id`（待添加） | `src-tauri/src/db/connection.rs` | Migration |
| `chapters.scene_id`（保留兼容） | `src-tauri/src/db/connection.rs` | Migration 37 |

---

## 五、风险与注意事项

1. **编排层重构（任务 2）** 是签名变更炸弹。所有调用 `AgentService::execute_writer` 的上层都需要改。建议先做编译时 grep 确认调用点清单。
2. **AppError（任务 4）** 改动面极大（450 处）。建议分文件批次迁移，不要一次性改完 35 个文件。
3. **1:N schema（任务 6）** 需要小心处理 `sequence_number` 的唯一性约束和 `previous_scene_id` / `next_scene_id` 链表指针。
4. **TipTap `scene-divider`（任务 7）** 是前端大改动。需要确保 `scene-divider` 的 Node schema 与后端的 Scene 切分逻辑严格对齐。
5. **Book Deconstruction（任务 9）** 的数据迁移不可逆。必须先备份 `reference_*` 表数据到 JSON，再执行 Migration。
