---
name: sf-reference
description: StoryForge 领域理论包（中级工程师/Sonnet 级模型缺乏的背景知识，在本项目的具体应用而非教科书）。何时加载：要改 TipTap/ProseMirror、Tauri IPC、rusqlite/r2d2、LanceDB、Zustand/TanStack、ts-rs、CJK 分词、KMP border、RRF、艾宾浩斯、合同驱动等，或不理解某个模块背后的原理时。
---

# StoryForge 领域参考

> 每条都是“在本项目怎么用/为什么”，不是通用教材。

## Tauri 2.4 IPC（Command + Event + State）

- **Command**：Rust `#[tauri::command] async fn foo(state: State<'_, DbPool>, ...) -> Result<T, AppError>`；`handlers.rs` 的 `generate_handler![]` 宏集中注册；前端 `services/api/core.ts` 的 `loggedInvoke<T>` 调用（参数脱敏 + 耗时 + 统一错误）。
- **Event**：Rust `state_sync/events.rs` 的 `SyncEvent`（`#[derive(TS)]` 自动生成 TS 到 `src-frontend/src/generated/`）；前端 `hooks/useSyncStore.ts` 监听 `sync-event` 精确失效 TanStack Query 缓存。16+ 种事件。
- **State**：Tauri `app.manage(x)` 注入，`state::<T>()` 读取。**陷阱**：读取未 manage 的 state 会启动 panic（Issue #4 / v0.23.6）；注入顺序敏感。

## TipTap / ProseMirror（幕前编辑器）

- 幽灵文本（Ghost Text）：`generatedText` state 驱动 `shouldShowGhostTree`（仅 `!!generatedText` 渲染，避免空幽灵容器残留——v0.26.13）。
- 竞态：TipTap DOM 滞后于 React `content` prop；用 `latestContentRef`（React 同步快照）作重复检测基准而非 `editorRef.getText()`（v0.26.9）。
- 自动保存 2s debounce + onChange 200ms IPC 节流；`appendText` 后立即 `getHTML()` 同步 store（v0.26.11）。
- `RichTextEditorRef` 暴露 `getHTML()`/`appendText()`。

## rusqlite + r2d2（SQLite 连接池）

- `DbPool` = r2d2 池。**必须** `.connection_timeout(5s)`（v0.23.19 教训：默认无限阻塞 → 600s）。
- 高频 DB 写入走 `spawn_blocking` fire-and-forget，**不阻塞 tokio worker**（`record_llm_call` 模式）。
- `MigrationRunner`（自定义，因 rusqlite 0.39 未用 refinery 默认特性）：扫描 `V###__*.sql`，按 version 排序，`schema_migrations` 表追踪，每迁移独立事务，幂等跳过 `duplicate column`/`already exists`。

## LanceDB + SQLite 向量兜底

- 向量存储：`LanceVectorStore` + SQLite `kg_entities.embedding` BLOB 持久化。
- **LanceDB 持久化 blocked**：Arrow 依赖与当前工具链冲突；当前 SQLite 兜底。
- CJK Bigram Tokenizer：中文二元组分词，BM25 + 向量余弦 + RRF 融合（`score = Σ 1/(k+rank)`，k=60）。

## Zustand + TanStack Query

- `stores/appStore.ts`（Zustand）：`currentStory`/`stories[]`/UI 状态。
- `useSyncStore.ts`：监听 Rust `sync-event`，按事件类型精确 invalidate TanStack Query 缓存。
- `App.tsx`：`currentStory` 变化时 `cancelQueries` + `invalidateQueries`。
- `backendActivityStore`：后台活动计数；订阅去抖（v0.25.1）；`isAnyBackendActive` 决定输入框是否禁用。

## ts-rs 类型生成

- Rust enum 加 `#[derive(TS)]` → 编译时生成 `src-frontend/src/generated/*.ts`。
- 前端 `assertUnreachable(x: never): never` 在 switch default 兜底；新增 variant 编译失败 → 强制穷尽匹配。

## KMP 最长 border（自重复检测）

- `trim_self_repetition`（Rust `utils/text.rs`）/ `trimSelfRepetition`（TS `utils/`）：归一化后做 KMP 最长 border 检测，保守阈值（重复长度 ≥30 字且 ≥ 全文 8%）裁掉尾部重复前缀；段落级先检测“后半段==前半段”/“末段==首段”。
- **跨层一致**：`tests/fixtures/trim_golden.json` 7 用例 Rust + TS 双跑。

## 艾宾浩斯遗忘曲线（记忆保留）

- `R(t) = R₀·e^(-λt) + Σ强化奖励`；λ 架构级 0.01 / 默认 0.05 / 瞬态 0.1。
- 五级优先级 Critical(>0.8)/High(0.6-0.8)/Medium(0.4-0.6)/Low(0.2-0.4)/Forgotten(<0.2)。
- `MemoryOrchestrator` 三层预算：Working 50% / Episodic 30% / Semantic 20%（write 任务）。**注**：`CONTEXT.md` 标注 `MemoryOrchestrator` 已实现但「Pending: wiring StoryContextBuilder to call QueryPipeline + MemoryOrchestrator for Full mode」——即预算定义在代码中，尚未完全接入 Full 生成路径，当前行为可能未充分体现该预算。

## 合同驱动（Story System v6.0.0）

- 四级合同 `MASTER_SETTING → Volume → Chapter → Review`；`SCENE_COMMIT` 写后真源（state/entity/events/projection_status JSON）。
- 5 Projection Writer：State/Index/Summary/Memory/Vector；`ContractTree`/`RuntimeContract` 动态合并上层合同 → 运行时约束。
- 防幻觉三定律：合同即法律、设定即物理、发明需识别。

## 分时介入三时间线（v0.13.0）

- 热路径 `WriteTimeBundle`（红线 + 角色核心 + 场景大纲 + 题材反模式）直连 LLM <15s。
- 温路径 `AuditExecutor` 7 维 Inspector 后台 30-90s，inline 标注（`ai_audit` 类型，severity 红黄蓝）回流。
- 冷路径 `InsightExecutor` 每 5 段深度报告 → 叙事分析页。
- `DebtIndicator` 顶栏实时未处理标注数。

## ContextPrioritizer（v0.25.0）

- `ContextChunk` 按 Critical/High/Normal/Background 排序；Critical 在开头 + 结尾轻量摘要双重锚定，缓解 "Lost in the Middle"。

## 何时 NOT 用本技能

- 具体怎么改/能不能改 → `sf-architecture-contract`。
- 失败模式 → `sf-debugging-playbook`。
- 配置开关 → `sf-config-and-flags`。

## 出处与维护

- 重验证命令：
  - `rg -n 'connection_timeout|spawn_blocking' src-tauri/src/llm src-tauri/src/db | head`
  - `rg -n '#\[derive\(TS\)\]' src-tauri/src | head`
  - `ls src-tauri/src/db/migrations/ | head`（迁移命名）
  - `rg -n 'latestContentRef|shouldShowGhostTree' src-frontend/src | head`
- 易漂移项：依赖版本、阈值常量、IPC 事件清单。
- 最后核对：2026-07-07，v0.26.23。
