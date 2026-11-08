# 幕后功能 × 主工作流集成清查（分析文档 + 实施方案）

> 日期：2026-11-07 ｜ 状态：待批准 ｜ 实施目标版本：v0.90.0 起，分阶段发版（原 v0.86–v0.89 版本号已被其他功能占用，基线重定）
> 范围：幕后全部 23 个导航视图 × 导演/管理/编审/主创四角色主工作流（AgencyStudio）的读写耦合。
> 不含：幕前界面重构、模型网关重写、UI 视觉重设计（v0.85.0 已完成）。
> 调研方式：三路并行代码清查（工作流地图 / 逐视图矩阵 / 历史文档交叉核对），全部结论带 file:line 证据。

## 0. 问题陈述

用户观察：「很多功能与导演主创管理编审 agent 这个主要工作流完全没有关联」。清查证实该观察属实，且根因不是个别页面遗漏，而是**结构性分裂**——仓库内同时存在三条 AI 管线，幕后有 6 个视图完全挂在与四角色无关的旧管线上，另有 7 个视图只有单向数据流。更关键的是，主工作流自身存在多处「断头路」：agency 跑了什么在生成链路页看不到、质量债导演看不见、学习中心的 instinct 永远进不了 prompt、agency 创世连 UI 入口都没有。

## 1. 结构性根因：三条并存管线

| 管线 | 代码位置 | 服务对象 | 幕后入口 |
|---|---|---|---|
| **Agency 四角色工作室**（本题主体） | `src-tauri/src/agency/`（coordinator/director/crew/tool_loop/tools/scheduler） | 创世、批量续写、导演派发修订 | 仅 AgencyStudio |
| **旧 AgentOrchestrator 单步管线** | `src-tauri/src/agents/orchestrator.rs` + `commands/orchestrator.rs` + `planner/` | smart_execute、TimeSliced、场景大纲/草稿一键生成 | SceneEditor 生成按钮、幕前 |
| **task_system 后台任务** | `src-tauri/src/task_system/`（8 执行器，lib.rs:404-463） | 拆书、指南蒸馏、级联改写、pipeline_review | Tasks 页、级联中心 |

关键枢纽：agency 续写的上游注入集中在 `creative_engine/write_time_bundle.rs`（`load_sync` :40-534，`to_prompt` :659-931）；agency ToolLoop 工具白名单只有 9 个（`agency/tools.rs:183-208`）：`board_read/board_write/board_revise/story_info/asset_query/asset_read/creative_context/corpus_search/corpus_read`。**skills、知识图谱查询、级联影响、质量债、叙事事件、意图图都不在工具面内。**

## 2. 主工作流地图（审计基准）

### 2.1 四角色输入/产出一览

| 角色 | 系统提示 | 关键数据源 | 工具 | 回写 |
|---|---|---|---|---|
| 导演 | `agency_director_system.md`（v0.82.2） | CrewSnapshot（runs/tasks/leases/alerts/最近 24 场/corpus_count，`crew.rs:41-197`）、语料 brief、聊天历史 | 无 ToolLoop（单发规划+执行，`director.rs:73-385`） | `director_actions`、`agency_agent_tasks` 派工、聊天/审批卡 |
| 管理 | `agency_producer_system.md` | 故事大纲、世界观、角色、卷纲（`render_volume_block_for_chapter`）、未回收伏笔、进度 | 7 读 + `board_write` | 板 Asset/Draft 区；物化 `characters/character_relationships/world_buildings/story_outlines/foreshadowing_tracker`（`materialize.rs`）；卷纲保存 |
| 编审 | `agency_editor_auditor_system.md` | 合同红线、世界观、大纲、知识边界/持有物、prose lint、文笔基线、情感计划、草稿 8000 字 | **只读 7**（无 board_write，tools.rs:201） | 板 Review 区 gate 记录（`record_gate_impl` :8718）、修订提案消息 |
| 主创 | `agency_lead_writer_system.md` | WriteTimeBundle 全量（红线/大纲/世界观/方法论扩展/叙事四元组/伏笔/关系/前文/张力/称呼/固定语料/检索块）+ 章契约 + 情感块 + 导演指令 | 7 读 + `board_write` + `board_revise` | 板 Draft 区、scenes 正文、`scene_outcomes`、情感三表、技巧日志、关系推断、资产回流 ingest |

### 2.2 前台与派发两条续写路径

- 幕前 Append/单章 → `write_beat_once`（直出，无工具）；
- 幕后导演派发/批量 → `run_continue_batch` → ToolLoop `write_chapter`（管线化 gate）。
- 两者共用 `WriteTimeBundle`/`continue_assets` 渲染、`ensure_assets` 预检、gate 与装配回写——**资产上游是统一的**，分裂在「谁触发」和「观测面」。

## 3. 23 视图连接矩阵（逐视图判定）

### 强连接（10）——双向读写都在主工作流内

| 视图 | 上游（→prompt/工具） | 下游（agency→视图） |
|---|---|---|
| agency-studio | 聊天+勾选语料进导演 prompt（chat.rs:96,149-164） | runs/board 五区/tasks/leases/情感/读者面板 |
| stories | 故事元数据、大纲、`methodology_id`→方法论扩展、style_dna | 管理物化大纲到 `story_outlines`（materialize.rs:364） |
| scenes | 场景大纲/戏剧目标进 bundle（write_time_bundle.rs:189-211）；导演快照 24 场 | 正文/章纲写回 `scenes`（coordinator.rs:6851） |
| characters | 角色卡+关系+生死进 bundle（:97-187）；asset_query 全卡 | 物化 INSERT（materialize.rs:327）、关系回填 |
| corpus | `corpus_search/read` 工具、pinned 语料块（coordinator.rs:5556/6244/9849）、导演 brief | 「移交导演」handoff（Corpus.tsx:268） |
| world_building | 世界观进 bundle（:438-492）+编审上下文 | `ensure_world_building` upsert（coordinator.rs:4378-4520） |
| foreshadowing | pending/overdue 进续写块（continue_assets.rs:1008）；卷纲读 top6 | 物化 INSERT（materialize.rs:375） |
| knowledge-graph | **仅 top-5 未归档实体单行摘要**进【相关设定】（facade.rs:23,56-94） | agency 后台回流写 KG（continue_loop.rs:481-487） |
| story-system | 合同红线置顶（:65-78）、runtime contract、体裁反模式、风格 DNA | 提交链 tab 读 SCENE_COMMIT（agency 走裸 update，弱下行） |
| maintenance | **active 文风偏好进主创 prompt**（style_learning.rs:111） | 质量债由 agency 多处写入（coordinator.rs:3163/5762/6021/6113/7019） |

### 单向（7）——只有半边数据流

| 视图 | 方向 | 缺口 |
|---|---|---|
| skills | 下（learning→技能列表） | **agency 从不调用 SkillExecutor**（agency 目录 grep 零命中；工具面无 skill） |
| book-deconstruction | 上（方法论扩展+参考场景 few-shot 进 prompt，write_time_bundle.rs:217-283） | 管理 ensure_assets 不读指南；蒸馏任务走 task_system |
| logs | 下（agency 调用带 context_label 进 workflow log，llm/service.rs:505-517） | — |
| usage-stats | 下（llm_calls 含 agency 计量） | 无 agency 语义维度 |
| agency-eval | 下（gate/checkpoint/agency_* token 聚合，agency/commands.rs:473-505） | 本职如此 |
| agency-learning | 下（观察流/gate/技巧统计；晋升物化技能） | **instinct 从不注入任何 prompt**（learning.rs:337 的 render_instinct 只写文件） |
| settings | 上（三档模型→四角色映射、提示词覆盖 prompt_overrides、run deadline） | **写作策略只影响旧管线**（write_time_bundle.rs:361-365：agency 用默认值） |

### 孤岛（6）——与主工作流零读写耦合

| 视图 | 实际挂载 | 关键证据 |
|---|---|---|
| dashboard | GenesisPanel 读 **已死的 `genesis_runs` 表**（全仓无写路径，repo 注释自认「创世写路径已切到 agency…Task 8 处理」） | genesis_run_repository.rs:9-11；`agency_start_genesis` 前端零调用 |
| tasks | task_system 控制台（8 类任务），**不显示 agency_agent_tasks** | task_system/models.rs:79-104；useTasks.ts 全走 task_system |
| cascade-center | 影响计算→人读；「触发改写」走 task_system 一次性执行器，**绕开导演**；agency 写路径不咨询 cascade_impacts | grep "cascade" 在 agency/ 与 write_time_bundle 中零命中 |
| tracing | TraceStore 只由旧编排器开 trace；**agency 调用不传 trace_id、不开 trace**——主工作流在生成链路页完全不可见 | agents/orchestrator.rs:483-495；coordinator.rs:395-413 |
| narrative-analysis | 事件/线索只进旧管线 writer 上下文（context_optimizer.rs:452-541）；deep_insight 只被旧 smart_execute/planner 消费；**agency 零消费**（agency/persist.rs:286 `recent_events: None`） | — |
| intention-graph | 只接旧 PlanExecutor 与网关意图路由；agency 目录零引用 | planner/executor.rs:18 |

## 4. 主工作流自身的断点（核心问题清单）

按「观测 → 调度 → 学习 → 数据」四类，共 12 项：

**观测断点（P0）**
- **G1 生成链路盲区**：agency 四角色的每次 LLM 调用不开 trace（coordinator.rs:395-413 不传 trace_id），Tracing 页看不到主工作流。
- **G2 Dashboard 读死表**：GenesisPanel/`genesis_runs` 无写路径；agency 创世（`agency_start_genesis`）无任何 UI 入口——用户在仪表盘看到的「运行记录」永远为空或过期。
- **G3 用量无角色维度**：usage-stats 只有 token 总量，看不出导演/主创/管理/编审各花了多少（数据已在 `llm_calls.purpose`，只差聚合展示）。

**调度断点（P1）**
- **G4 质量债不回流导演**：agency 写 quality_debts，但 CrewSnapshot 无 debts 字段（crew.rs:41-51），导演 repair 动作只能指向 tasks/runs；`record_debt` 导演动作是 **no-op**（director.rs:355）。
- **G5 级联改写绕开导演**：cascade_rewrite 一次性 LLM 改写，不经导演审批/派发。
- **G6 Tasks 页与 agency 任务两套系统**：作者在 Tasks 页看不到导演派发的子任务，也无法从那里重试/取消 agency 任务。
- **G7 SceneEditor 生成按钮走旧管线**：生成大纲/草稿直接调 AgentService OutlinePlanner（SceneEditor.tsx:162,187），与导演无关。
- **G8 ExecutionPanel 快捷动作是占位**：ai_continue/ai_revise 只跳转+toast（ExecutionPanel.tsx:99-153）。

**学习断点（P2）**
- **G9 instinct 不进 prompt**：学习中心产出的 instinct 只写成 .md 文件，写作路径零注入。
- **G10 learned 技能 agency 不执行**：学习晋升物化的技能只能被旧管线跑，agency 学习回路断头。
- **G11 写作策略设置对 agency 无效**：用户在设置页调的策略只覆盖旧管线。

**数据断点（P3）**
- **G12 叙事事件/追读债/insight 只喂旧管线**：`recent_events: None`（agency/persist.rs:286）、`chase_debt_text` 恒空（write_time_bundle.rs:529）、deep_insight 无人读。
- **G13 修订后不 re-ingest KG**（既有记录债，v0.80.6 未关闭）：导演修订的正文不回流资产/图谱。

## 5. 与既有记录债的关系（避免重复上报）

已修复不再上报：伏笔热路径（v0.82）、语料工具+交给导演（v0.74）、pinned 语料（v0.77）、情感回路（v0.75/76）、叙事契约+legacy 资产接线（v0.79/80）、导演派发闭环（v0.80.5/0.80.6/0.82.2）、幕前/幕后生成路径统一（agency-only 设计）。

本清查**新发现**（现役文档未立债）：G1-G12 全部、以及「task_system vs agency_agent_tasks 双轨」的结构性事实（历史文档有意留任 GenesisPanel，但未记录 Tasks 页与 agency 的割裂）。

本方案顺带关闭的**既有记录债**：G13（v0.80.6 修订不 re-ingest）、G4 部分对应导演富化门控遗留（v0.73.0 未关闭）。明确**不在本方案**：伏笔碎片窗口覆盖率（v0.84.2 未关闭，独立专项）、卷号→场景映射（同）、人类 A/B 横向验证（发版后统一安排）、图谱画布渲染（v0.72.2 未关闭，UI 专项）。

## 6. 实施方案（四阶段，每阶段独立发版可回滚）

### P0 观测闭合（目标 v0.90.0，已实施）——「让主工作流被看见」

| # | 改动 | 关键文件 | 验收探针 |
|---|---|---|---|
| A1 | agency 每次 LLM 调用携带 trace_id：run 级开 trace（复用 TraceStore），step 记录 role/phase/tool_loop 轮次；director 单发也开短 trace | coordinator.rs（AgencyLlm 构造、run_genesis/run_continue/run_batch_inner 入口）、agents/orchestrator.rs 的 TraceStore 复用 | 真机跑一次批量续写 → Tracing 页出现该 run 的 trace，含 writer/gate 步骤 |
| A2 | Dashboard「运行记录」改读 agency_runs（复用 crewStatus/`agency-run-progress` 事件），提供「发起创世」入口接 `agency_start_genesis`；`genesis_runs` 读路径与 GenesisPanel 退役（保留表） | GenesisPanel.tsx、Dashboard.tsx、agency/commands.rs | 仪表盘可见进行中/最近 agency run；点发起创世走四角色流程 |
| A3 | usage-stats 增加按 context_label（agency_writer/producer/editor/director/frontstage）分组的用量卡 | UsageStats.tsx、llm/commands.rs 聚合 | 设置页模型角色分配后，四角色用量分列显示 |

### P1 调度闭合（目标 v0.91.0，已实施）——「把孤岛变成导演的手」

| # | 改动 | 关键文件 | 验收探针 |
|---|---|---|---|
| B1 | CrewSnapshot 增加 `open_debts`（severity 排序 top5：kind/summary/建议窗口），导演 prompt 增 Debts 区块；`record_debt` 改为真实落库+回报；repair 语义扩展为「dispatch recompute/派发修订」 | crew.rs、director.rs（prompt+validate_action+executor）、quality_debt.rs | 构造一条质量债 → 导演巡检可见 → 指令「修复质量问题」产生可确认动作 |
| B2 | 级联中心「触发改写」增加「交给导演」模式：预填导演聊天（含影响清单），走派发 lead_writer target_chapter 修订；保留原 task_system 快速模式 | CascadeCenter.tsx、studio_commands.rs、chat_commands.rs | 影响报告页一键交给导演 → 审批卡显示目标章 → 修订装配 |
| B3 | Tasks 页新增「代理任务」页签（读 agency_agent_tasks，复用 crewStatus IPC），行内提供「请导演重试/取消」入口（预填聊天） | Tasks.tsx、useTasks.ts、agency/commands.rs | Tasks 页可见派发任务；一键请导演重试生成审批卡 |
| B4 | SceneEditor 生成大纲/草稿增加「派发」选项（走导演 dispatch OutlinePlanner 语义或主创），旧一键路径保留为「快速生成」 | SceneEditor.tsx、chat_commands.rs | 编辑器内派发 → 审批卡 → 产物写回大纲/草稿 |
| B5 | ExecutionPanel ai_continue/ai_revise 接真实动作：续写当前章（`agency_continue_chapter`）/ 修订（预填导演聊天 target_scene） | ExecutionPanel.tsx | 场景页快捷操作产生真实 run/审批卡，无占位 toast |

### P2 学习与设置闭合（目标 v0.92.0，已实施）——「学习回路闭环」

| # | 改动 | 关键文件 | 验收探针 |
|---|---|---|---|
| C1 | instinct 注入：新增 `render_instinct_block`（active、按命中率排序、≤N 条、预算上限），进入 `render_continue_assets` 动态区与 ToolLoop `creative_context`；设置页加开关与灰度 | learning.rs、continue_assets.rs、tools.rs、GeneralSettings.tsx | 批准一条 instinct → 续写 prompt 出现【已学模式】块；关闭开关后消失 |
| C2 | 工具面新增 `skill_execute`（白名单：仅 learned/approved、生成类技能、单次输出截断），先开放给主创/管理 | tools.rs、SkillManager 复用、roles 提示词工具说明 | 主创在 ToolLoop 中成功调用一次 learned 技能并产出可见文本 |
| C3 | 写作策略接入 agency：AppConfig 策略字段读入 coordinator，覆盖 bundle 默认值（与旧管线同语义） | write_time_bundle.rs:361-365、coordinator.rs 装配点 | 设置改冲突等级 → 续写提示词策略块变化 |

### P3 数据闭合（并入 v0.92.0 发布，已实施）——「叙事数据回流」

| # | 改动 | 关键文件 | 验收探针 |
|---|---|---|---|
| D1 | narrative events/threads 进 agency 写作上下文：复用 context_optimizer 构建函数，bundle 增 `narrative_events_text`，continue_assets 渲染 | context_optimizer.rs 抽取复用、write_time_bundle.rs、persist.rs:286 | 有叙事事件的章节续写 → prompt 出现事件历史块 |
| D2 | deep_insight 摘要进 CrewSnapshot（最近 1 份），导演巡检可见 | insight_executor.rs、crew.rs、director prompt | 有 insight 的故事 → 导演 prompt 出现洞察摘要 |
| D3 | 追读债接 agency：coordinator 在装配时设置 `chase_debt_text` | write_time_bundle.rs:529、coordinator.rs | 有追读债的故事 → 续写 prompt 追读债块非空 |
| D4 | 导演修订装配后 re-ingest KG（关闭 v0.80.6 记录债）：修订路径复用 spawn_asset_ingest | scheduler.rs:591/503、continue_loop.rs | 修订一幕后 KG 实体更新（探针断言 ingest 任务产生） |

### 统一约束

- 所有 prompt 新增块走既有预算体系（continue_assets 截断常量），总预算不放大；instinct/insight 块带设置开关，默认开、可回滚。
- 工具面扩张仅限 C2 一项，且强制白名单+输出截断，防提示词膨胀与提示注入面扩大。
- 不合并 task_system 与 agency 调度器（各有执行器生态）；只在展示层（B3）与触发层（B2）打通。
- 不改 `--ai-*` 组件 API、不动幕前、不改 IPC 已有字段语义（只增字段）。

## 7. 验收清单（整体）

- [ ] Tracing 页可见一次完整 agency 批量续写的全角色链路
- [ ] Dashboard 显示 agency run 并可发起创世；genesis_runs 死读路径退役
- [ ] 导演巡检可见质量债并产生真实修复动作（record_debt 不再 no-op）
- [ ] 级联改写可经导演审批派发；Tasks 页可见代理任务并请导演重试
- [ ] 编辑器/场景快捷动作不再有占位 toast
- [ ] 批准的 instinct 出现在续写 prompt；learned 技能可被主创调用
- [ ] 写作策略对 agency 生效；叙事事件/追读债/洞察进入对应 prompt
- [ ] 每阶段：cargo test 全量 + 新增契约测试 + tsc/vitest/playwright 基线不降 + 八文档同步

## 8. 风险与缓解

| 风险 | 缓解 |
|---|---|
| trace 全开带来开销/日志膨胀 | 采样：仅 agency run 级开 trace（每次运行 1 条），step 复用现有 step 机制；可加开关 |
| 导演 prompt 变长挤占预算 | Debts/insight 块各设上限（如 600/400 字符），走快照截断 |
| skill_execute 提示注入面 | 白名单 learned/approved、生成类；输出截断；不给文件/网络类技能 |
| GenesisPanel 退役影响老用户习惯 | 仪表盘新卡片承接同位置；数据源换成活的 agency_runs |
| 多阶段改动相互冲突 | 每阶段独立 tag 发版；C1/D1 等 prompt 块全部带开关 |
