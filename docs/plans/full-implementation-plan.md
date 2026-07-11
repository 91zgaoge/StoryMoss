# StoryMoss 全面实施计划

> 由 2026-05-16 重大设计周期产出。涵盖 Q1–Q70 全部设计-代码差距检视结果。
> 
> 批准状态：待批准  
> 批准人：__________  
> 批准日期：__________

---

## 一、已确认的总体架构原则（不可谈判）

所有实施任务必须以以下四条原则为过滤器：

1. **沉浸式不可侵犯原则**：Frontstage 的任何技术实现，如果可能导致编辑器失焦、闪烁、卡顿、或强制用户等待，则该实现必须被重构。
2. **单点真理原则（Single Source of Truth）**：`frontstageStore` 拥有正在编辑的文本，后端 DB 拥有已提交的文本，两者之间不允许存在第三个可写缓存层。
3. **分层守卫原则**：配额检查、取消令牌、错误分类必须在架构最底层（`LlmService.generate()`）实现一次，上层零成本继承。任何绕过的路径都是 bug。
4. **本地数据主权原则**：在线账户只为增值服务（平台模型配额、云端备份），基础写作功能（本地编辑、本地保存、本地模型）在离线时完全可用，且数据永不因网络状态丢失。

---

## 二、实施波浪划分

| Wave | 主题 | 周期 | 目标 |
|------|------|------|------|
| Wave 1 | 止血 | 1–2 周 | 堵住经济损失、修复致命用户体验、停止静默失败 |
| Wave 2 | 架构骨架 | 2–3 周 | 建立正确的编排层、事件流、状态机、错误体系 |
| Wave 3 | 智能层落地 | 3–4 周 | 注入 MemoryPack、补齐 Pipeline、实现 StyleDNA/角色动态 |
| Wave 4 | 工程化 | 持续 | 类型安全、测试、性能、安全、组件拆分 |

**依赖规则**：Wave N 的任务不得假设 Wave N-1 的抽象已存在。若存在跨 Wave 依赖，需在 Wave N-1 中预留接口。

---

## 三、Wave 1：止血（1–2 周）

### 3.1 后端 LLM 层守卫（P0）

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W1-B1 | 统一配额检查入口 | 在 `LlmService.generate()` 和 `stream_generate()` 入口处加入配额预检。按 `ModelSource` 区分：平台模型检查配额，本地/自购模型跳过。 | `src-tauri/src/llm/service.rs` | 所有 AI 入口（Agent、PlanExecutor、WorkflowScheduler、Bootstrap、Deconstruction、Pipeline）调用 LLM 时自动受配额约束；绕过路径数为 0 |
| W1-B2 | 同步 LLM 调用取消令牌 | 给 `generate_with_context_and_pipeline` 接入 `CancellationToken`。即使非流式请求，也要包装在可取消的 future 中并注册到 `cancel_senders`。 | `src-tauri/src/llm/service.rs` | Bootstrap、Deconstruction、AgentOrchestrator 循环、PlanExecutor 步骤均可被取消；取消后后台不再继续 burn token |
| W1-B3 | StoryContextBuilder 错误分级 | 区分致命错误（核心数据缺失）和可降级错误（辅助数据缺失）。致命错误返回 `Err`，可降级错误返回 `Ok` 但填充 `AgentContext.warnings`。 | `src-tauri/src/creative_engine/context_builder.rs` | AI 不再在空上下文上生成；前端能收到 "context unavailable" 或 "generating with limited context" 的明确反馈 |

### 3.2 模型来源抽象（P1，W1-B1 的前置）

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W1-B4 | 定义 `ModelSource` 枚举 | `Platform` / `LocalOllama` / `UserOwned`。平台模型走配额+用量统计，本地/自购模型跳过配额。 | `src-tauri/src/llm/mod.rs` | `LlmService.generate()` 签名包含 `model_source: ModelSource`；所有调用点传入正确来源 |
| W1-B5 | API key 安全存储 | 自购模型的 key 从 `config.json` 迁移到系统 keychain（tauri-plugin-stronghold 或 OS native）。`config.json` 只保留非敏感配置。 | `src-tauri/src/config.rs`, keychain 插件 | 磁盘上不存在明文 API key；Capability 中移除 `fs:allow-app-write-recursive` 全局权限 |

### 3.3 前端 IPC 收敛（P1）

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W1-F1 | 收敛裸 `invoke()` 调用 | 将 `FrontstageApp.tsx`、`useBookDeconstruction.ts`、`SceneEditor.tsx`、`ConnectionStatus.tsx` 等所有裸 `invoke()` 改为通过 `services/tauri.ts` 的 `loggedInvoke`。 | 上述文件 + `services/tauri.ts` | `grep -r "invoke(" src-frontend/src/ --include="*.tsx" --include="*.ts" \| grep -v "loggedInvoke" \| grep -v "from '@tauri-apps/api"` 返回空 |
| W1-F2 | 修复 IPC 审计正则 | 更新 `verify-ipc-manifest.py`，使其匹配 `invoke('cmd')` 和 `invoke<Type>('cmd')` 两种形式。或强制统一使用泛型形式。 | `verify-ipc-manifest.py` | 脚本能捕获 100% 的 IPC 调用点，零盲端点 |

### 3.4 离线模式基础（P1）

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W1-F3 | 网络状态感知 | 前端监听 Tauri `online`/`offline` 事件，全局显示网络状态。离线时功能分级：写作完全可用，平台 AI 禁用并说明原因，本地 Ollama 仍可用。 | `src-frontend/src/stores/networkStore.ts` (新建) | 断网时 Frontstage 可继续写作；Ghost Text 若配置为本地模型则仍触发；平台模型调用返回明确离线提示 |
| W1-B6 | 离线配额快照 | 配额数据本地缓存（SQLite），联网时校准。离线时使用快照，允许一个"离线宽限额度"（如 10 次平台调用），联网后校准。 | `src-tauri/src/quota/mod.rs` (新建) | 离线时平台模型有有限可用额度；联网后校准不超支 |

---

## 四、Wave 2：架构骨架（2–3 周）

### 4.1 编排层重建

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W2-B1 | AgentOrchestrator 引入 `GenerationMode` | 定义 `GenerationMode::Fast`（单轮 LLM，跳过 Inspector/StyleChecker）和 `GenerationMode::Full`（完整四步循环）。Ghost Text 走 Fast，标准写作走 Full。 | `src-tauri/src/agents/orchestrator.rs` | Ghost Text 不再是一个"未知路径"，而是 Orchestrator 的配置项 |
| W2-B2 | PlanExecutor 写步骤嵌套 Orchestrator | 删除 `execute_writer_raw` 裸写。PlanExecutor 任何文本生成步骤必须调用 `AgentOrchestrator::generate(mode=Full)`。 | `src-tauri/src/planner/executor.rs` | PlanExecutor 的文本生成步骤 100% 经过 Orchestrator；配额和取消自动生效 |
| W2-B3 | WorkflowScheduler 节点嵌套 Orchestrator | `WriteChapter` 等 DAG 节点同样嵌套调用 Orchestrator，禁止直接调用 Writer Agent。 | `src-tauri/src/workflows/*.json` + scheduler | Workflow 节点不再绕过 Orchestrator |
| W2-B4 | CapabilityRegistry 全局单例 | 重构 `build_default_registry()` 为全局单例，支持 `load_evolved_descriptions()`。EvolutionEngine 的演化结果在启动时加载。 | `src-tauri/src/capabilities/mod.rs`, `evolution.rs` | 注册表启动时加载演化结果；运行时动态注册 MCP 能力（W4） |

### 4.2 CHAPTER_COMMIT 事件化

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W2-B5 | CHAPTER_COMMIT 自动触发 | `update_chapter` 成功后自动触发 `CHAPTER_COMMIT`，带 30s debounce。移除前端显式调用 `apply_chapter_commit`。 | `src-tauri/src/commands_v3.rs`, `events/mod.rs` | 保存后 30s 内无新编辑则自动 commit；前端不需要组装复杂参数 |
| W2-B6 | 吸收 `auto_ingest` 进 Projection Writers | 删除 `commands_v3.rs` 中 120+ 行的内联 auto-ingest 逻辑。将其功能拆分为 `VectorProjectionWriter`（向量化）和 `MemoryProjectionWriter`（知识图谱）。`auto_ingest_chapter` 命令标记为废弃。 | `src-tauri/src/commands_v3.rs`, `projections/*.rs` | 单次 commit 只有一个索引入口；无重复向量；无命令层内联业务逻辑 |
| W2-B7 | Projection Writer 性能基准 | 测量 5 个 Writer 在典型章节（3000 字）下的总执行时间。若超过 2s，引入并行执行或异步队列。 | `src-tauri/src/projections/mod.rs` | 典型场景下 commit 后处理 < 2s；UI 无卡顿 |

### 4.3 错误体系与状态机

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W2-B8 | 定义 `AppError` 枚举 | `QuotaExceeded`, `LlmTimeout`, `DbLocked`, `ContextUnavailable`, `ValidationFailed` 等。所有内部 API 返回 `Result<T, AppError>`。 | `src-tauri/src/error.rs` (新建) | IPC 序列化为 `{ code, message, data }`；前端根据 `code` 渲染不同恢复 UI |
| W2-B9 | GenesisRun 状态机 | 新建 `genesis_runs` 表，7 步增量持久化。每步完成后立即保存，支持暂停/恢复/失败重试。 | `src-tauri/src/workflows/genesis.rs` + migration | Genesis 可在任何步骤暂停并恢复；取消时保留已完成部分 |
| W2-B10 | 配额预检（Genesis） | Genesis 第一步前估算总 token 消耗，配额不足时立即返回，不触发任何 LLM 调用。 | `src-tauri/src/workflows/genesis.rs` | 用户在 Genesis 第一步就知道配额是否足够 |

### 4.4 前端骨架调整

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W2-F1 | 状态管理三角解耦 | `frontstageStore` 持有编辑中内容（唯一可写源）；`appStore` 的 chapters 列表只做展示；`sync-event` 不覆盖当前编辑章节的内容。 | `src-frontend/src/stores/*` | 保存过程中编辑器不丢焦点；同步事件不导致内容回滚 |
| W2-F2 | 废弃 DOM CustomEvent | 删除 `backstage-data-refreshed` DOM 事件。双窗口通信统一走 Tauri `sync-event`。 | `src-frontend/src/App.tsx`, `useSyncStore.ts` | 只存在一套跨窗口事件系统；无重复触发 |
| W2-F3 | 根除 `renderKey` | 找到 `renderKey` 掩盖的根因（可能是 `sync-event` 直接 `setContent`），修复后删除所有 `key={renderKey}` 用法。 | `src-frontend/src/App.tsx`, `FrontstageApp.tsx` | 无 brute-force redraw；渲染由数据变化自然驱动 |

---

## 五、Wave 3：智能层落地（3–4 周）

### 5.1 MemoryPack 注入

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W3-B1 | MemoryPack 注入 AgentContext | `StoryContextBuilder` 调用 `QueryPipeline` + `MemoryOrchestrator`，将 `MemoryPack` 附加到 `AgentContext`。`previous_chapters` 字段吸收进 `working_memory` 或退役。 | `src-tauri/src/memory/orchestrator.rs`, `creative_engine/context_builder.rs` | Writer Agent 的 prompt 包含语义检索、图扩展、预算控制后的记忆上下文 |
| W3-B2 | QueryPipeline 搜索双状态 | QueryPipeline 同时搜索 `narrative_*` 表的 `active` 和 `reference` 状态，使 Deconstruction 材料成为创作记忆的一部分。 | `src-tauri/src/memory/query_pipeline.rs` | Deconstruction 的小说元素可被 AI 在生成时引用 |

### 5.2 存储统一

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W3-B3 | Deconstruction 存储统一 | 废弃 `reference_*` 表（Migration 16/17）。Deconstruction 元素进入 `narrative_*` 表，`ElementSource::Extracted`。区分靠 `status` 字段（`reference` vs `active`）。 | 数据库 migration + 导入逻辑 | "一键转换为 story" 不再复制数据，只改 `status` 字段 |

### 5.3 3-review Pipeline 补齐

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W3-B4 | Refine 真实 LLM 调用 | `refine_draft` 接入真实 LLM，结构化 JSON 输出，解析后应用修改。 | `src-tauri/src/pipeline/refine.rs` | Refine 不再返回原文；返回带有修改标记的 draft |
| W3-B5 | Review 真实 LLM 调用 | `review_draft` 接入真实 LLM，结构化评分 + 维度分析，返回真实分数（非固定 85.0）。 | `src-tauri/src/pipeline/review.rs` | Review 返回多维评分和具体建议 |
| W3-B6 | style_analysis 实现 | 每 5 章触发时，计算当前 StyleDNA 六维向量，保存并演化。 | `src-tauri/src/pipeline/style_analysis.rs` | StyleDNA 每 5 章更新一次；数据进入数据库 |

### 5.4 StyleDNA 与角色动态

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W3-B7 | StyleDNA schema + 计算 | 数据库表存储六维向量；实现每个维度的计算函数（sentence length, dialogue ratio, metaphor density, inner monologue, emotional exposure, rhythm）。 | `src-tauri/src/style/dna.rs` (新建) + migration | 给定文本可计算六维向量；精度可测 |
| W3-B8 | StyleDNA feedback 闭环 | Anti-AI Review 和 Pipeline Review 的结果作为 feedback 输入，驱动 StyleDNA 演化。 | `src-tauri/src/style/evolution.rs` | Review 发现的问题可映射到 StyleDNA 维度调整 |
| W3-B9 | 角色动态状态 | 明确"角色动态状态"定义（情感、关系、目标进展）。每次 CHAPTER_COMMIT 后由 Character PostProcessor 更新，进入 MemoryPack。 | `src-tauri/src/characters/dynamic_state.rs` (新建) | AI 生成时知道"角色 A 对主角态度已从友好变为怀疑" |
| W3-B10 | ForeshadowingTracker 注入 MemoryPack | Tracker 的逾期/待闭合伏笔状态进入 MemoryPack，约束 AI 生成时收敛伏笔。 | `src-tauri/src/foreshadowing/tracker.rs` | AI 生成后续内容时主动处理逾期伏笔 |

### 5.5 前端智能层 UX

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W3-F1 | Genesis 进度可视化 | 7 步进度条，已完成步骤可展开查看/编辑，当前步骤显示实时日志，支持"暂停并退出"。 | `src-frontend/src/components/GenesisPanel.tsx` (新建) | 用户可随时查看/暂停/恢复 Genesis；不阻塞写作 |
| W3-F2 | StyleDNA 雷达图 | Backstage 展示当前 StyleDNA 六维雷达图；用户可手动调整某一维，AI 后续生成遵循约束。 | `src-frontend/src/components/StyleDnaRadar.tsx` (新建) | 可视化六维指纹；手动调整即时生效 |
| W3-F3 | WenSi 模式统一路径 | 确认 WenSi `off`/`passive`/`active` 三态下，Ghost Text 和 Slash Menu 都统一走 Orchestrator（Fast/Full），不在前端硬编码多路径。 | `src-frontend/src/frontstage/tiptap/AiSuggestionNode.tsx`, Slash Menu | 所有 AI 交互走统一后端管道；前端只负责触发条件和展示 |

---

## 六、Wave 4：工程化（持续）

### 6.1 错误处理全面替换

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W4-B1 | 替换 `map_err(|e| e.to_string())` | 分阶段替换 450 处 String 错误为 `AppError`。优先替换 IPC 边界和 LLM 调用路径。 | 35 个文件 | 核心路径（LLM、DB、IPC）100% 使用 `AppError`；前端可区分错误类型 |

### 6.2 MCP 动态注册

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W4-B2 | MCP 动态注册 | MCP 服务器连接时动态注册到全局 `CapabilityRegistry`。`PlanGenerator` 引用实时注册表。`PlanExecutor` 实现 `CapabilitySource::McpTool` 分发。 | `src-tauri/src/mcp/*.rs`, `capabilities/mod.rs`, `planner/*.rs` | LLM 生成的 plan 可包含 MCP 步骤；步骤被保留并正确分发 |

### 6.3 配置持久化迁移

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W4-B3 | AppConfig 用户级迁移 | 用户级配置（模型选择、API key 引用、主题、快捷键）迁移到 SQLite。`config.json` 只保留启动级配置（数据库路径、日志级别）。 | `src-tauri/src/config.rs` | 配置变更无需重启；多设备同步时只需同步 SQLite |

### 6.4 前端类型安全

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W4-F1 | ts-rs 输出路径修复 | 修复 ts-rs 生成到 `src-frontend/src/src-frontend/` 的错误路径。配置正确输出目录或添加构建脚本移动文件。 | `Cargo.toml` ts-rs 配置 或 build script | ts-rs 输出到正确目录；与手工维护的 `generated/` 合并或替代 |
| W4-F2 | 类型命名统一 | ts-rs 生成的 snake_case 类型与前端 camelCase 惯例对齐。写转换脚本或统一使用 snake_case（不推荐）。 | `src-frontend/src/generated/*.ts` | 前端类型与后端 Rust 类型 1:1 对应；无手工维护的重复定义 |

### 6.5 测试策略落地

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W4-F3 | useSyncStore 回归测试 | 取消 skip，使用 mock Tauri event 验证 `invalidateQueries` 被正确调用。 | `src-frontend/src/hooks/__tests__/useSyncStore.bug.spec.ts` | 测试通过；可防止 sync 逻辑回归 |
| W4-B4 | StoryContextBuilder 错误测试 | 验证 DB 错误时返回 `Err` 而非空默认值。 | `src-tauri/src/creative_engine/context_builder.rs` | 单元测试覆盖致命错误路径 |
| W4-B5 | LlmService 配额 mock 测试 | mock LLM 服务，验证配额不足时返回 `AppError::QuotaExceeded` 且无 HTTP 请求发出。 | `src-tauri/src/llm/service.rs` | 配额逻辑 100% 单元测试覆盖 |
| W4-F4 | E2E 断言补齐 | `e2e/storymoss.spec.ts` 至少加入一个核心断言：保存章节后重进 Frontstage，内容仍存在。 | `e2e/storymoss.spec.ts` | E2E 有实际断言；非纯截图 |
| W4-F5 | 删除 `--disable-web-security` | 从 `playwright.config.ts` 移除该标志。修复因此暴露的跨域问题（如有）。 | `playwright.config.ts` | 测试环境不再关闭同源策略 |

### 6.6 前端性能与架构

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W4-F6 | Tiptap 性能基准 | 测试 10万/50万/100万字文档的输入延迟、保存序列化时间、内存占用。 | 测试脚本 + `FrontstageApp.tsx` | 有基准数据；明确性能拐点 |
| W4-F7 | 自动保存非阻塞化 | 将序列化+IPC 调用移入 Web Worker 或 React Transition，避免主线程阻塞输入。 | `src-frontend/src/frontstore/autoSave.ts` (新建) | 自动保存过程中打字无 stutter |
| W4-F8 | FrontstageApp.tsx 拆分 | 将 83KB 的 FrontstageApp.tsx 按职责拆分为：编辑器容器、AI 面板、工具栏、状态栏、键盘快捷键处理等子组件。 | `src-frontend/src/frontstage/components/*.tsx` | 单文件 < 200 行；职责单一；无循环依赖 |
| W4-F9 | Settings.tsx 拆分 | 将 72KB 的 Settings.tsx 按设置类别拆分为独立子页面或标签组件。 | `src-frontend/src/pages/settings/*.tsx` | 同上 |
| W4-F10 | SceneEditor.tsx 拆分 | 将 46KB 的 SceneEditor.tsx 拆分。 | `src-frontend/src/components/scene-editor/*.tsx` | 同上 |

### 6.7 安全与配置清理

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W4-B6 | Capability 权限收紧 | `main-capability.json` 移除 `fs:allow-app-write-recursive` 和 `http:allow-fetch` 全局允许。改为精确路径和精确域名。 | `src-tauri/capabilities/main-capability.json` | 权限最小化；每项权限有明确理由 |
| W4-B7 | 删除 Tauri v1 遗留配置 | 删除嵌套的 `src-tauri/src-tauri/tauri.conf.json`。 | `src-tauri/src-tauri/` | 无 v1 遗留文件 |
| W4-B8 | 删除硬编码 IP | 将 `config.json` 中的 `http://10.62.239.13:17098/v1` 改为环境变量或首次启动配置向导。 | `src-tauri/config.json` | 无硬编码内网地址 |
| W4-F11 | 删除僵尸依赖 | 移除 `react-router-dom`、`@dnd-kit/*` 等未使用依赖。评估 `framer-motion` 和 `react-hook-form` 是否必要。 | `src-frontend/package.json` | `package.json` 中无已确认僵尸依赖 |

### 6.8 数据完整性

| # | 任务 | 描述 | 关键文件 | 验收标准 |
|---|------|------|----------|----------|
| W4-B9 | Scene/Chapter 写入语义澄清 | 明确 `update_scene` 和 `update_chapter` 的语义边界。编辑器写入 Scene；`chapters.content` 由后端自动聚合。 | `src-tauri/src/commands_v3.rs` | 前端调用 `update_scene`；后端在 scene 保存后自动刷新 chapter 缓存 |
| W4-B10 | 导出聚合完整性 | 导出 txt/pdf/epub 时，按 `scenes.order_index` 排序聚合，而非直接读 `chapters.content` 缓存。 | `src-tauri/src/export/*.rs` | 导出结果与所有 scene 内容 1:1 对应；无遗漏无重复 |
| W4-B11 | Scene 管理 UX | Backstage 增加 Scene 排序、合并、拆分功能，使 1:N 模型能力对用户可见。 | `src-frontend/src/pages/BackstageScenes.tsx` (新建) | 用户可管理 Scene 与 Chapter 的关系 |

---

## 七、跨 Wave 依赖图

```
W1-B4 (ModelSource) ──→ W1-B1 (配额检查)
                          │
                          ▼
W2-B1 (GenerationMode) ──→ W2-B2 (PlanExecutor 嵌套)
                          │
                          ▼
W2-B5 (CHAPTER_COMMIT) ──→ W2-B6 (吸收 auto-ingest)
                          │
                          ▼
W3-B1 (MemoryPack) ──────→ W3-B9 (角色动态)
                          │
                          ▼
W3-B10 (Foreshadowing) ──→ W3-B7 (StyleDNA)
                          │
                          ▼
W4-B1 (AppError) ────────→ 所有错误处理改进
```

**关键路径**：W1-B4 → W1-B1 → W2-B1 → W2-B2 → W2-B5 → W2-B6 → W3-B1 → W3-B4/W3-B5 → W4-B1

---

## 八、风险与缓释

| 风险 | 概率 | 影响 | 缓释措施 |
|------|------|------|----------|
| Tiptap 大文档性能瓶颈导致重写编辑器 | 中 | 极高 | W4-F6 优先做基准测试；若拐点 < 20万字，提前评估 Slate/Monaco 替代 |
| AppError 替换 450 处 map_err 工作量超预期 | 高 | 中 | 分 3 个 Sprint，每 Sprint 替换 ~150 处；允许新旧共存过渡期 |
| Scene/Chapter 1:N 重构影响现有用户数据 | 中 | 高 | 写数据迁移脚本；在测试数据库上验证 1:1 现有数据无损转换 |
| MCP 动态注册引入外部依赖不可控 | 低 | 中 | MCP 步骤默认禁用；用户显式启用 |
| 离线宽限额度被滥用 | 中 | 中 | 宽限额度上限低（10次）；联网后立即校准并扣减；异常模式上报 |

---

## 九、验收与发布标准

### Wave 1 出口标准
- [ ] 所有 AI 入口配额检查不可绕过（代码审计 + mock 测试）
- [ ] 取消按钮在 Bootstrap/Genesis/Agent 循环中 3 秒内生效
- [ ] StoryContextBuilder 返回空字符时前端显示明确警告
- [ ] 前端裸 `invoke()` 数量为 0
- [ ] 离线时本地写作功能 100% 可用

### Wave 2 出口标准
- [ ] PlanExecutor 和 WorkflowScheduler 写步骤 100% 经过 Orchestrator
- [ ] CHAPTER_COMMIT 自动触发，前端无需显式调用
- [ ] auto_ingest 逻辑从命令层完全移除
- [ ] Genesis 支持暂停/恢复/失败重试
- [ ] `renderKey` 从代码库完全删除

### Wave 3 出口标准
- [ ] Writer Agent prompt 包含 MemoryPack 内容
- [ ] Refine/Review 返回真实 LLM 结果（非 placeholder）
- [ ] StyleDNA 六维向量可计算、可展示、可演化
- [ ] Deconstruction "一键转换"只改 status 字段

### Wave 4 出口标准
- [ ] 核心路径零 `map_err(|e| e.to_string())`
- [ ] MCP 步骤可被 LLM plan 生成并正确执行
- [ ] Tiptap 100万字输入延迟 < 100ms
- [ ] E2E 测试有核心断言且通过
- [ ] 安全审计通过（无全局 fs/http 权限，无明文 key）

---

## 十、附录：已检视频隙索引（Q1–Q70）

| Q# | 领域 | 核心差距 |
|----|------|----------|
| Q1-Q15 | 后端编排层 | AgentOrchestrator 名存实亡；三条并行路径 |
| Q16-Q19 | Ghost Text / Orchestrator | Ghost Text 是"未知路径"；需要 Fast/Full 双路径 |
| Q20-Q30 | 前端架构 | IPC 绕过；双窗口通信混乱；状态管理三角 |
| Q31-Q35 | 错误处理 / 静默失败 | 450 处 String 错误；StoryContextBuilder 永不 Err |
| Q36-Q40 | CHAPTER_COMMIT / 投影 | 显式命令；auto_ingest 旁路；Projection Writers 不完整 |
| Q41-Q45 | 安全 / 配置 | Capability 过宽；硬编码 IP；v1 配置遗留 |
| Q46-Q50 | 测试 / 质量 | 唯一单元测试是 1+1；E2E 零断言；被 skip 的 bug 测试 |
| Q51-Q55 | 类型安全 / 依赖 | ts-rs 路径错误；僵尸依赖；手工维护类型 |
| Q56 | IPC 审计盲区 | 裸 invoke 绕过 verify-ipc-manifest.py |
| Q57 | 双窗口通信 | sync-event + DOM CustomEvent 两套系统并存 |
| Q58 | 前端状态管理 | frontstageStore / appStore / DB 三角同步竞态 |
| Q59 | Genesis 可恢复性 | 7 步无 checkpoint；LLM 不可取消；cancel 语义不明 |
| Q60 | WenSi / Slash Menu / Ghost Text | 8 个命令路径混乱；WenSi 三态未统一 |
| Q61 | ForeshadowingTracker | 轮询 vs 事件驱动不明；时间窗口定义模糊；未注入 MemoryPack |
| Q62 | StyleDNA / 角色动态 | 六维向量空壳；feedback 闭环断裂；style_analysis placeholder |
| Q63 | 用量统计 / 隐私 | 统计范围不明；上传 vs 本地边界不清；与本地主权张力 |
| Q64 | 模型来源抽象 | 平台/本地/自购模型无区分；config.json 全局静态；key 明文存储 |
| Q65 | Vector Store 重复索引 | auto_ingest 与 VectorProjectionWriter 重复工作；5min cooldown 隐患 |
| Q66 | Chapter vs Scene 1:N | 编辑器可能直接写 Chapter；Scene 成"幽灵表"；导出聚合机制不明 |
| Q67 | Tiptap 长篇性能 | 全文档 DOM 渲染；序列化阻塞主线程；undo 内存爆炸；无基准 |
| Q68 | Tauri 更新 / 数据丢失 | install_update 未检查未保存状态；重启可能丢数据 |
| Q69 | 离线模式 UX | 无在线/离线状态指示；功能降级策略缺失；配额离线决策未定义 |
| Q70 | （本计划整合期） | — |

---

*文档版本：v1.0*  
*生成日期：2026-05-16*  
*下一步：等待批准 → 按 Wave 分解为可执行任务 → 进入执行阶段*
