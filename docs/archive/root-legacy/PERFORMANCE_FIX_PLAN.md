# StoryMoss 智能创作流程卡顿问题 — 专家级代码审计与修复计划

## 1. 执行摘要

**审计范围**：StoryMoss v0.11.1（项目根目录 `/Users/yuzaimu/projects/StoryMoss`）中「智能创作」功能的完整链路，涵盖前端（`src-frontend/src`）、Tauri 后端（`src-tauri/src`）、数据库/向量层（SQLite + LanceDB）。

**基于用户反馈的关键上下文**：
1. 当前最严重症状：**点击「写一部小说/续写」等生成命令后，长期在个别进程上无响应，最后无输出**。
2. 典型文档长度：**千字级别**，排除「超长文档全量加载」为主要矛盾。
3. 运行模型：**本机与局域网模型（Ollama/本地 API）**，并发请求会互相挤占本地推理资源。
4. UI 行为微调：**可接受**。

**核心结论**：
- 智能创作功能本身的后端 LLM 调用、Agent 编排、流式输出机制已经具备完整能力；近期版本（v0.11.5–v0.11.7）也已对部分阻塞点做了修补。
- 根据用户反馈，**最致命的卡顿不在前端输入，而在后端生成链路**：`smart_execute` → `prepare_writer_context` → `AgentOrchestrator::generate_candidates` → `LlmService::execute_generation` 这条链路上存在同步阻塞、过度并发、超时策略偏长、取消不可靠等问题。在本地/局域网模型场景下，多个候选并行 + 长超时会导致进程长期挂起，最终无输出。
- 同时仍存在其他系统性性能缺陷：前端主线程高频 setState、后端 CPU 密集型任务阻塞 tokio worker、SQLite 同步查询锁竞争、全局 Mutex 缓存竞争等。

**最可能导致「无法正常生成内容」的根因组合**：
- 后端 `prepare_writer_context` / `build_writer_prompt` / `AgentOrchestrator::generate_candidates` 中的同步阻塞与长超时，导致 `smart_execute` 调用长时间无响应。
- 本地/局域网模型并发能力差，默认 2 候选并行 + 总超时 270s 会长期占用资源，最终失败或卡死。
- 前端 `FrontstageApp.tsx` 状态集中 + 高频事件，后端一旦延迟，UI 立刻表现为全面卡顿、按钮无响应。
- 编辑器每次输入都触发 IPC + 全量字数统计 + 自动保存 + 后台通知，进一步放大卡顿感。

**建议修复策略**：采用「先止血、再减压、再重构」的三阶段方案。

---

## 2. 审计方法与信息来源

- 只读扫描 `src-frontend/src`、`src-tauri/src`、`src-server/`、`src-server-web/`。
- 并行使用 4 名探索子代理分别覆盖：前端创作流程、Tauri 后端 AI 调用、服务端生成逻辑、全项目性能热点扫描。
- 交叉验证 git 历史 hotfix（v0.11.5–v0.11.7）与现有 `PERFORMANCE_OPTIMIZATION_REPORT.md`。
- 所有问题均标注具体文件路径与行号区间，便于复查。

---

## 3. 根因分析（按严重程度）

### P0 — 极可能导致明显卡顿或生成失败

#### P0-1 前端编辑器输入路径在主线程同步执行过重逻辑
- **位置**：`src-frontend/src/frontstage/FrontstageApp.tsx:1060–1105` `handleContentChange`
- **症状**：用户每按一个键都会触发：
  1. `setContent(newContent)` → 触发 `FrontstageApp` 重渲染；
  2. 两次正则统计中文字数/英文词数；
  3. `scheduleAutoSave(..., 2000)`（虽有防抖，但任务创建发生在关键路径）；
  4. `loggedInvoke('notify_backstage_content_changed', ...)` 发送 IPC。
- **后果**：长文本（数万字）时，按键即掉帧；IPC 频率等于按键频率。

#### P0-2 前端智能文思分析同步阻塞主线程
- **位置**：
  - `src-frontend/src/frontstage/ai-perception/SmartHintSystem.tsx:124–140`
  - `src-frontend/src/frontstage/ai-perception/textAnalyzer.ts:610–655`
- **症状**：输入停止 1.5s 后执行 `analyzeText`，对全文进行段落、句子、词汇、节奏、分布全量扫描；每次分析还 `document.createElement('div')` + `innerHTML` + `querySelectorAll('p')` 全量解析 HTML。
- **后果**：长文本下分析一次可达数百毫秒，期间 UI 完全冻结。

#### P0-3 打字机效果与心跳轮询高频 setState
- **位置**：
  - `src-frontend/src/frontstage/FrontstageApp.tsx:1353–1367`（16ms setInterval 打字机）
  - `src-frontend/src/frontstage/FrontstageApp.tsx:462–471`（1s 心跳更新 generationStatus）
  - `src-frontend/src/frontstage/components/FrontstageBottomBar.tsx:97–101`（1s pulseTick）
  - `src-frontend/src/frontstage/components/AiSuggestionBubble.tsx:153–304`（12s 轮询 + 递归 setTimeout）
- **症状**：大量定时器持续触发 setState，即使状态未变也触发重渲染。
- **后果**：与长生成叠加时，UI 被高频渲染占满，表现为「卡死」。

#### P0-4 Rust 后端 CPU 密集型任务阻塞 tokio worker
- **位置**：
  - `src-tauri/src/anti_ai/mod.rs:61–143`（反 AI 五维审查，全量遍历 + 正则）
  - `src-tauri/src/book_deconstruction/parser.rs:136–213`（txt/pdf/epub 大文件同步解析）
  - `src-tauri/src/vector/lancedb_store.rs:363–420`（hybrid_search RRF 融合，HashMap 归并 + 排序）
- **症状**：以上任务均通过 async Tauri command 调用，但内部计算未放入 `spawn_blocking`。
- **后果**：长文本/大文件/向量检索量大时，tokio worker 被长时间占用，前端 IPC 响应延迟、流式输出卡顿。

#### P0-5 SQLite 同步查询直接运行在 async 上下文
- **位置**：
  - `src-tauri/src/scene_commands.rs:156–365` `update_scene`
  - `src-tauri/src/creation_commands.rs:212–472` `create_story_with_wizard`
  - `src-tauri/src/agents/service.rs:746–1026` `prepare_writer_context`
  - `src-tauri/src/db/repositories.rs` 大量 `get_by_story` / `create` / `update` 方法
- **症状**：使用 `r2d2_sqlite` + `rusqlite`，但大量同步 DB 调用被 async command 直接 `await` 前执行，未包裹 `spawn_blocking`。
- **后果**：SQLite 读写阻塞当前 tokio worker；高并发时（前台写作 + 后台 Ingest + MemoryWriter）触发 `database is locked` 或长时间等待。

#### P0-6 全量加载 story 下所有 scenes/chapters/content
- **位置**：`src-tauri/src/db/repositories.rs:160–242` `SceneRepository::get_by_story`
- **症状**：`SELECT ... content, draft_content, outline_content ... FROM scenes WHERE story_id = ?1 ORDER BY sequence_number`，无 `LIMIT/OFFSET`，无延迟加载。
- **后果**：长篇小说单次查询反序列化巨量 JSON 字符串，IPC payload 巨大，切换故事/章节时前端卡顿。

#### P0-7 前端单组件承载 40+ 状态 + 大量全局事件监听
- **位置**：`src-frontend/src/frontstage/FrontstageApp.tsx:79–126, 199–216, 401–416, 520–951`
- **症状**：`FrontstageApp` 声明约 40+ 个 `useState`，注册 9+ 个 Tauri 事件监听器，每个监听器内直接 setState。
- **后果**：任何小状态变化都触发整个应用重渲染；后端事件密集时前端被状态更新淹没。

---

### P1 — 高概率造成间歇性卡顿或掉帧

#### P1-1 `useSyncStore` 单事件触发 9 个 query 失效
- **位置**：`src-frontend/src/hooks/useSyncStore.ts:355–418`
- **症状**：`dataRefresh/all` 同时 `invalidateQueries` 9 个 key，引发后端 IPC 瀑布。
- **后果**：Genesis 完成或批量更新后，前端一次性请求 stories/scenes/chapters/world_building/foreshadowings/storyOutlines/knowledgeGraph/characterRelationships/payoffLedger/storyTimeline。

#### P1-2 RichTextEditor 每次变更序列化 HTML
- **位置**：`src-frontend/src/frontstage/components/RichTextEditor.tsx:198–200`
- **症状**：`onUpdate: ({ editor }) => { onChange(editor.getHTML()); }`
- **后果**：长文档时每次按键都执行 O(n) HTML 序列化。

#### P1-3 Rust 提示词构建反复加载配置与风格 DNA
- **位置**：`src-tauri/src/agents/service.rs:1537–2154` `build_writer_prompt`
- **症状**：每次构建都 `spawn_blocking` 重新从磁盘加载 `AppConfig`、`GenreProfile`；风格混合时对每个 component 新建 `StyleDnaRepository` 并查询。
- **后果**：高频调用时反复 IO + 线程池排队，延迟首次 token 时间。

#### P1-4 候选阶段并发与超时策略仍偏激进
- **位置**：`src-tauri/src/agents/orchestrator.rs:943–1125`
- **症状**：`generate_candidates` 默认并行 2 个候选，总超时 `per_candidate_timeout * count + 30`，虽已加 `min(120)` 硬上限，但仍可能 2×120+30=270s；`join_all` 无信号量限流。
- **后果**：本地模型/低配置机器上瞬间并发压力高，用户取消前长时间无响应。

#### P1-5 SQLite 连接池与 busy_timeout 在高并发下不足
- **位置**：`src-tauri/src/db/connection.rs:76–84`
- **症状**：`max_size=20`、`busy_timeout=5000ms`；未暴露 `wal_autocheckpoint` 调优。
- **后果**：后台任务堆积时触发锁竞争或超时。

#### P1-6 全局 Mutex 缓存竞争
- **位置**：
  - `src-tauri/src/llm/service.rs:1434–1449` `LLM_SERVICE` 全局 Mutex
  - `src-tauri/src/lib.rs:76` `DB_POOL` 全局 Mutex
  - `src-tauri/src/creative_engine/context_builder.rs:39–128` `ContextCache` std Mutex
- **症状**：每次 LLM 调用/获取连接/构建上下文都需获取同一把锁。
- **后果**：并发高时线程阻塞在锁上。

#### P1-7 知识图谱无虚拟化，全量渲染
- **位置**：`src-frontend/src/components/KnowledgeGraph/KnowledgeGraphView.tsx:51–139`
- **症状**：所有 entities/relations 一次性计算布局并渲染为 ReactFlow nodes/edges。
- **后果**：实体数数百上千时布局计算 + DOM 节点创建造成明显卡顿。

---

### P2 — 中等风险，随数据量增长会恶化

- `FrontstageApp.tsx:2133–2138` render 阶段用 reduce + 正则计算全文字数。
- `StreamOutput.tsx:51–115, 174, 248–260` 每次渲染都用正则把 Markdown 转成 HTML 并 `dangerouslySetInnerHTML`。
- `NovelCreationWizard.tsx` 世界观/角色/文风选择页无虚拟化。
- `src-tauri/src/embeddings/provider.rs:248–300` Ollama embedding 串行逐条请求。
- `src-tauri/src/vector/lancedb_store.rs:238` 字符串拼接 filter，存在注入风险与无法利用索引。
- `StudioManager` 导入/导出整个故事 ZIP 在内存中构建。

---

## 4. 修复计划（三阶段）

### 阶段一：止血 — 让「生成有输出、不长期挂起」（预计 2–3 天）

**目标**：针对用户反馈的最致命症状——点击生成后长期在个别进程上无响应、最后无输出——优先修复后端生成链路阻塞与本地/局域网模型并发问题。

| 编号 | 修复项 | 关键文件 | 具体动作 |
|------|--------|----------|----------|
| 1.1 | 候选阶段并发与超时重构 | `agents/orchestrator.rs:943–1125` | 本地/局域网模型默认改为**单候选**；远端模型可配置 1–2 候选。总超时改为 `min(per_candidate_timeout * count + 30, 90)`（默认 90s 硬上限）。增加 `tokio::sync::Semaphore` 限制全局并发 Writer 数量（本地模型默认 1，远端默认 2）。 |
| 1.2 | LLM 调用层超时与取消加固 | `llm/service.rs:659–899`, `llm/commands.rs` | 增加 `connect_timeout`（默认 10s）；`reqwest` 响应体读取阶段也受整体超时控制；确保 `heartbeat_handle` 在 panic/early return 时通过 `scope`/`defer` 模式可靠中止；`LlmTimeout` 细分为连接超时与生成超时，连接超时可重试 1 次。 |
| 1.3 | 写作上下文准备 spawn_blocking 化 | `agents/service.rs:746–769`, `creative_engine/context_builder.rs:175–207` | 将 `prepare_writer_context` 及 `StoryContextBuilder::build` 中的同步 DB 查询块整体包裹 `spawn_blocking`；将 `ContextCache` 从 `thread_local! + std Mutex` 改为进程级 `tokio::sync::RwLock` 缓存，提升命中率并减少重复 DB 查询。 |
| 1.4 | 提示词构建减少重复 IO | `agents/service.rs:1537–2154` | `AppConfig` / `GenreProfile` / `StyleDna` 在服务初始化时加载并缓存；`build_writer_prompt` 使用已缓存引用；风格混合改为批量查询而非按 component 单独查询。 |
| 1.5 | SQLite 高频路径 spawn_blocking 化 | `scene_commands.rs:156–365`, `creation_commands.rs:212–472` | 将 `update_scene`、`create_story_with_wizard` 中的同步 DB 事务块包裹 `spawn_blocking`；优先保证 `smart_execute` 主链路不阻塞 tokio worker。 |
| 1.6 | 全局 Mutex 替换 | `llm/service.rs:1434–1449`, `lib.rs:76`, `context_builder.rs:39–128` | `LLM_SERVICE` / `DB_POOL` 改用 `tokio::sync::OnceCell` 或 `OnceLock` + `Arc`；`ContextCache` 改用 `tokio::sync::RwLock`。 |
| 1.7 | 前端生成状态可取消与反馈 | `FrontstageApp.tsx:787–891`, `useBackendActivityListener.ts` | 生成进行中显示精确阶段（准备上下文 / 候选生成 / Inspector / 改写 / 最终输出）与已用时间；取消按钮必须可靠调用 `agent_cancel_all_tasks` 并中止后端 `join_all`。 |
| 1.8 | 前端输入路径减负 | `FrontstageApp.tsx:1060–1105` | 字数统计移入 `requestIdleCallback`；`notify_backstage_content_changed` 节流至 300–500ms；autoSave 保持防抖但不在按键关键路径创建任务。 |
| 1.9 | 关闭/替换高频心跳 | `FrontstageApp.tsx:462–471`, `FrontstageBottomBar.tsx:97–101` | 将 1s 心跳改为基于事件驱动；`pulseTick` 用 CSS 动画替代 React state；16ms 打字机改为 `requestAnimationFrame` 或流式 chunk 直接驱动。 |

### 阶段二：减压 — 前端体验与大数据量优化（预计 3–4 天）

**目标**：在阶段一解决「生成无输出」问题后，进一步优化前端响应与大数据量场景。

| 编号 | 修复项 | 关键文件 | 具体动作 |
|------|--------|----------|----------|
| 2.1 | 字数统计增量化 | `FrontstageApp.tsx:2133–2138` | 在 `handleContentChange` 中基于增量 diff（字数变化）更新 totalWordCount，而非每次 reduce 全文章节。 |
| 2.2 | 场景/章节数据分页与延迟加载 | `db/repositories.rs` | `get_by_story` 增加 `limit/offset` 重载；前端按需加载当前章节附近内容；全文检索/统计由后端聚合返回。 |
| 2.3 | 合并 sync-event 失效 | `useSyncStore.ts:355–418` | 将 9 个独立 `invalidateQueries` 合并为一次批量刷新；后端 emit 时附带受影响资源列表，前端按需失效。 |
| 2.4 | LanceDB 查询优化 | `vector/lancedb_store.rs` | filter 改为参数化；`text_search` 增加 FTS 索引或改为前缀匹配；hybrid_search 利用 LanceDB 原生 hybrid API 减少一次查询。 |
| 2.5 | Embedding 批处理 | `embeddings/provider.rs` | 将 Ollama/Local embedding 改为批量请求（batch ≤ 32/64），减少网络/IO 往返。 |
| 2.6 | 前端状态拆分 | `FrontstageApp.tsx` | 将底部栏状态、生成状态、bootstrap 状态、活动状态拆分为独立 context/store 或子组件，避免单点重渲染。 |
| 2.7 | 文思分析异步化 | `SmartHintSystem.tsx`, `textAnalyzer.ts` | 将 `analyzeText` 放入 Web Worker；HTML 解析改为基于 TipTap editor state 的增量分析，避免 `innerHTML`；增加取消机制。 |
| 2.8 | RichTextEditor HTML 序列化节流 | `RichTextEditor.tsx:198–200` | `onUpdate` 中改用 `editor.getText()` 或增量 delta，仅在需要保存/IPC 时序列化完整 HTML；长文档场景使用防抖。 |

### 阶段三：重构 — 架构级优化与体验完善（预计 5–7 天）

**目标**：建立可持续扩展的智能创作性能基线。

| 编号 | 修复项 | 关键文件 | 具体动作 |
|------|--------|----------|----------|
| 3.1 | 前端事件聚合通道 | `FrontstageApp.tsx`, `useBackendActivityListener.ts` | 后端将 `orchestrator-step / agent-stage-update / llm-generating-progress` 合并为单一 `generation-status` 事件，前端统一消费。 |
| 3.2 | 知识图谱虚拟化/分层 | `KnowledgeGraphView.tsx` | 使用 ReactFlow 的 `onlyRenderVisibleElements`、节点分层加载、MiniMap 可选关闭。 |
| 3.3 | Agent 编排可观测性 | `agents/orchestrator.rs`, `llm/service.rs` | 为每个 generation 注入结构化 trace（阶段耗时、token 耗时、DB 耗时），输出到日志/指标，便于后续调优。 |
| 3.4 | 真实 tokenizer 与上下文预算 | `memory/tokenizer.rs`, `memory/query.rs` | 引入 `tiktoken-rs` 或 `tokenizers` crate 做真实 token 计数；预算控制按 token 而非字符。 |
| 3.5 | 后台任务队列与背压 | `agents/orchestrator.rs`, `scene_commands.rs` | 将 MemoryWriter + IngestPipeline 加入有界队列，避免用户连续创作时后台任务无限堆积。 |
| 3.6 | 端到端性能测试 | `e2e/` | 添加 1万/5万/10万字场景下的输入延迟、生成首 token 时间、故事切换耗时基准测试。 |

---

## 5. 执行组织与 Agent 分工

为高效完成本次审计修复，将每个阶段的任务拆分为可并行的子任务，由多个专用 agent 同时推进。每个 agent 负责独立模块，最终由主 agent 合并、验证并提交。

### 5.1 阶段一 Agent 分工（并行）

| Agent | 代号 | 负责项 | 关键文件 | 建议模型 | 依赖 |
|-------|------|--------|----------|----------|------|
| A1 | Backend-Orchestrator | 1.1 候选阶段并发与超时重构；1.2 LLM 调用层超时与取消加固 | `agents/orchestrator.rs`, `llm/service.rs`, `llm/commands.rs` | 大模型 | 无 |
| A2 | Backend-Context | 1.3 写作上下文准备 spawn_blocking 化；1.4 提示词构建减少重复 IO | `agents/service.rs`, `creative_engine/context_builder.rs` | 大模型 | 无 |
| A3 | Backend-Database | 1.5 SQLite 高频路径 spawn_blocking 化；1.6 全局 Mutex 替换 | `scene_commands.rs`, `creation_commands.rs`, `llm/service.rs`, `lib.rs`, `context_builder.rs` | 大模型 | 无 |
| A4 | Frontend-UX | 1.7 前端生成状态可取消与反馈；1.8 前端输入路径减负；1.9 关闭/替换高频心跳 | `FrontstageApp.tsx`, `FrontstageBottomBar.tsx`, `useBackendActivityListener.ts` | 大模型 | A1（事件定义） |
| A5 | QA-Stage1 | 阶段一验证：本地模型千字续写、小说创建成功率与耗时基准 | `e2e/`, Playwright | 快速模型 | A1–A4 |

**并行策略**：A1、A2、A3、A4 可同时启动；A5 在 A1–A4 完成后启动。

### 5.2 阶段二 Agent 分工（并行）

| Agent | 代号 | 负责项 | 关键文件 | 建议模型 | 依赖 |
|-------|------|--------|----------|----------|------|
| B1 | Frontend-Performance | 2.1 字数统计增量化；2.3 合并 sync-event 失效；2.6 前端状态拆分 | `FrontstageApp.tsx`, `useSyncStore.ts` | 大模型 | 阶段一完成 |
| B2 | Backend-DataLayer | 2.2 场景/章节数据分页与延迟加载 | `db/repositories.rs`, 前端调用点 | 大模型 | 阶段一完成 |
| B3 | Backend-Vector | 2.4 LanceDB 查询优化；2.5 Embedding 批处理 | `vector/lancedb_store.rs`, `embeddings/provider.rs` | 大模型 | 阶段一完成 |
| B4 | Frontend-Editor | 2.7 文思分析异步化；2.8 RichTextEditor HTML 序列化节流 | `SmartHintSystem.tsx`, `textAnalyzer.ts`, `RichTextEditor.tsx` | 大模型 | 阶段一完成 |
| B5 | QA-Stage2 | 阶段二验证：大数据量场景、输入延迟、故事切换 | `e2e/`, Playwright | 快速模型 | B1–B4 |

### 5.3 阶段三 Agent 分工（并行）

| Agent | 代号 | 负责项 | 关键文件 | 建议模型 | 依赖 |
|-------|------|--------|----------|----------|------|
| C1 | Frontend-Architecture | 3.1 前端事件聚合通道；3.2 知识图谱虚拟化/分层 | `FrontstageApp.tsx`, `useBackendActivityListener.ts`, `KnowledgeGraphView.tsx` | 大模型 | 阶段二完成 |
| C2 | Backend-Observability | 3.3 Agent 编排可观测性；3.5 后台任务队列与背压 | `agents/orchestrator.rs`, `llm/service.rs`, `scene_commands.rs` | 大模型 | 阶段二完成 |
| C3 | Backend-Tokenizer | 3.4 真实 tokenizer 与上下文预算 | `memory/tokenizer.rs`, `memory/query.rs`, Cargo.toml | 大模型 | 阶段二完成 |
| C4 | QA-Regression | 3.6 端到端性能测试 + 完整回归 | `e2e/`, Playwright, 自定义基准 | 快速模型 | C1–C3 |

### 5.4 主 Agent（ coordinating agent ）职责

- 启动并监控各子 agent；在关键合并点（阶段结束、接口变更）统一协调。
- 负责跨 agent 的接口对齐（如 A1 修改的事件名需同步给 A4；A2 缓存结构需同步给 C2）。
- 汇总各 agent 的修改，运行 `cargo check`、`npm run build`、E2E 测试，提交 PR。
- 若子 agent 之间出现冲突，由主 agent 裁决并回退或重新分配。

### 5.5 Agent 间协作规则

1. **接口先行**：修改公共结构/事件名/函数签名前，先由主 agent 确认并通知相关 agent。
2. **每日同步**：每个子 agent 完成当日任务后向主 agent 提交简报（修改文件、测试状态、阻塞点）。
3. **独立分支**：每个 agent 在独立 git 分支工作，阶段结束时由主 agent 合并到 `perf/stage-{N}` 集成分支。
4. **快速模型用途**：仅用于验证、文档、测试用例生成、简单重构；核心异步/并发逻辑由大模型处理。

---

## 6. 验证方案

### 6.1 量化指标

| 指标 | 当前预估 | 阶段一目标 | 阶段三目标 |
|------|----------|------------|------------|
| 生成命令最终无输出比例 | 高（本地/局域网模型） | ≤ 5% | ≤ 1% |
| 生成命令 90s 内完成比例 | 低 | ≥ 90% | ≥ 95% |
| 智能创作首次 token 时间 | 5–60s | ≤ 8s | ≤ 5s |
| `smart_execute` 平均总耗时 | 30–300s（常挂起） | ≤ 90s | ≤ 60s |
| 千字文档按键输入延迟（95th） | 50–150ms | ≤ 30ms | ≤ 16ms |
| 故事切换 IPC 调用数 | 9+ 并行 | ≤ 3 次 | ≤ 2 次 |
| 知识图谱 500 节点首屏时间 | > 3s | ≤ 2s | ≤ 1s |

### 6.2 验证工具

- **前端**：Chrome DevTools Performance、React Profiler、手动 1万/5万/10万字文档输入测试。
- **后端**：自定义 IPC 计时中间件、LLM 调用 trace、SQLite query 日志、`tokio-console`（可选）。
- **端到端**：Playwright E2E 基准用例。

### 6.3 每阶段交付物

- 阶段一：PR + 修复清单 + 输入延迟/生成响应基准对比。
- 阶段二：PR + 大数据量场景测试报告。
- 阶段三：PR + 架构优化文档 + 完整性能回归测试。

---

## 7. 风险与回滚策略

| 风险 | 缓解措施 |
|------|----------|
| `spawn_blocking` 引入后线程池饱和 | 监控阻塞任务队列长度；必要时调大 `max_blocking_threads` 或改用 dedicated runtime。 |
| 前端状态拆分导致 bug | 严格保留现有 props 接口；先新增状态容器再逐步迁移；每步跑 E2E。 |
| 缓存化导致配置热更新失效 | 缓存 TTL 5 分钟 + 提供手动刷新命令；开发模式禁用缓存。 |
| 分页/延迟加载破坏现有功能 | 保持原有全量 API 为兼容重载；前端新组件使用新 API。 |
| SQLite 锁竞争未完全消除 | 阶段一后评估是否需要升级到 `deadpool-sqlite` 或 `sqlx`。

---

## 8. 建议的审批后执行顺序

1. **先实施阶段一 1.1–1.7**：直接针对「生成无输出/长期挂起」问题，风险低、见效快。
2. **阶段一完成后跑 E2E + 本地模型生成测试**（千字续写、小说创建），确认生成成功率与响应时间恢复。
3. **再实施阶段一 1.8–1.9 + 阶段二 2.1–2.6**：解决前端输入卡顿与大数据量场景。
4. **最后阶段二 2.7–2.8 + 阶段三 3.1–3.6**：架构级重构，建立长期可维护性。

---

## 9. 用户反馈与已确认约束

| 问题 | 用户反馈 | 对计划的影响 |
|------|----------|--------------|
| 最严重症状 | 点击「写一部小说/续写」等生成命令后，长期在个别进程上无响应，最后无输出 | 阶段一优先修复后端生成链路阻塞与超时策略 |
| 典型文档长度 | 千字级别 | 全量加载 scenes/chapters 不是主要矛盾，优先级后移 |
| 使用模型 | 本机与局域网模型（Ollama/本地 API） | 默认单候选、限制并发、缩短超时、增加连接超时 |
| UI 行为微调 | 可接受 | 可移除 1s 心跳、用 CSS 动画替代 pulse、重构生成状态反馈 |

（如无需回答，可直接批准按上述三阶段执行。）
