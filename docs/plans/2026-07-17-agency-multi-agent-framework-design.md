# StoryMoss 多代理创作框架（创世 2.0 / Agency）设计

> 实现状态：P1-P3 已完成（2026-07-17，除真机验收外）。
> 灵感来源：[affaan-m/ECC](https://github.com/affaan-m/ECC)（Everything Claude Code）的五大机制——代币优化、记忆持久性、持续学习、验证循环、并行化与子代理协调。

## 背景

StoryMoss 现有智能创作流程（GenesisPipeline + TriShot）是硬编码的顺序流水线：概念→题材→策略→骨架→首章→后台资产步骤。它稳定但存在结构性局限：

- 无 LLM 工具调用循环（`tool_calls` 零处理），代理无法用工具自主行动；
- 无代理间通信/共享工作内存，Orchestrator 是硬编码流水线，Agent 注册靠枚举；
- 子代理（Continuity/Style/World）是纯规则实现，非 LLM 代理；
- 无代理级评估框架，无法度量"代理行为"的质量；
- 无持续学习能力，无法从创作会话中沉淀可复用模式。

本设计将创世流程重构为**主创 agent + 管理 agent + 编辑审计 agent 三角色黑板模型并行协作**的通用自治代理框架，并整合 ECC 的五大机制，实现代理持续学习、自主形成技能，提高创世效率与质量。

## 关键决策（已经用户逐项批准）

| # | 决策点 | 结论 |
|---|--------|------|
| 1 | 实施形态 | **内置 Rust 核心**：在 `src-tauri` 内新建多代理模块，复用现有 LLM/路由/提示词/审计基础设施，经 IPC 与前端打通 |
| 2 | 旧创世流程策略 | **完全替换**：新框架端到端验收后，旧 GenesisPipeline/TriShot 路径同版本删除，打 git tag 留回退点 |
| 3 | 协作模型 | **黑板模型**：三代理围绕共享创作黑板异步并行，黑板变更为主协调通道，消息总线仅用于提案与告警 |
| 4 | 前端范围 | **完整代理可视化**：代理工作室页（状态卡/黑板视图/时间线/学习中心/评估仪表盘） |
| 5 | 学习产物形态 | **双轨制**：instinct 文件层积累置信度 → 晋升物化为内置技能/提示词变体，人工确认生效 |
| 6 | 运行时路线 | **通用自治代理框架**：ReAct 工具调用循环 + agent 间消息总线，三角色是其上实例 |

## 目标

- 创世流程重构为三角色并行协作框架，统一输出装配器保证"稳定持续输出小说内容"的质量门；
- 整合 ECC 五机制：代币优化（模型路由/提示词预算/后台化）、记忆持久性（跨会话快照恢复）、持续学习（观察→instinct→晋升）、验证循环（grader 分级/pass 指标/检查点）、并行化（黑板+消息总线+迭代检索）；
- 代理可持续学习，学习产物经置信度积累与人工确认后物化为技能，形成数据飞轮；
- 完全替换旧创世路径。

## 总体架构

```
幕前/幕后前端（新增代理工作室页）
   │  IPC + 事件流（agent 活动 / 黑板变更 / 评估指标 三类新事件）
   ▼
commands/agency.rs（新 IPC 入口，最终取代 smart_execute 的创世分支）
   ▼
src-tauri/src/agency/（新核心模块）
   ├── runtime/       通用自治代理运行时：ReAct 工具循环 + 消息总线 + 并发预算
   ├── blackboard/    黑板：共享创作状态协作视图（SQLite 真源 + 内存快照 + 版本化）
   ├── roles/         三角色实例：lead_writer / producer / editor_auditor
   ├── coordination/  协调器：事件驱动调度、冲突仲裁、统一输出装配
   ├── eval/          验证循环：grader 分级、pass@k/pass^k、检查点
   ├── learning/      持续学习：观察→instinct→晋升（双轨）
   └── session/       记忆持久性：会话快照、恢复、双层摘要
   ▼
复用层（不改接口）：LlmService / UnifiedModelRouter / model_gateway /
   PromptRegistry / story_system 合同 / creative_engine / memory+KG /
   audit 五维审计 / task_system / skills / MCP
```

核心原则：

- **SQLite 仍是唯一真源**，黑板是其上的协作视图；场景正文唯一真相源仍为 `scenes.content`；
- 所有 LLM 调用仍经 `LlmService`（保留 PromptCache、协作式取消、`llm_calls` 成本落表）；
- 架构守护（`scripts/architecture_guard.py`）与 MODULE_CHARTER 同步更新。

## 模块设计

### 1. 代理运行时（`agency/runtime/`）

- **ReAct 工具循环**（`tool_loop.rs`）：LLM 以严格 JSON schema 输出 action（`{tool, args}` 或 `{final}`），runtime 解析 → 执行注册工具 → 结果回灌，直至产出 final 或触发熔断（最大轮数/超时/成本上限）。新建 storymoss 空白的 `tool_calls` 能力，复用 `LlmService` 流式与取消。
- **工具注册表**：统一 `AgentTool` trait（name/schema/execute/副作用标记），适配层包装现有能力：资产 CRUD、记忆/KG 查询、审计调用、MCP 工具、技能执行、黑板读写。**按角色白名单授权**（ECC agents frontmatter 的 tools 隔离模式）：主创不能改调度，管理不能改正文，编辑只读草稿区 + 写审查区。
- **消息总线**（`bus.rs`）：agent 间结构化消息（proposal / note / alert 三型，避免自由对话的 token 失控），落 `agent_messages` 表可追溯。
- **并发预算**：替代单一 `BACKGROUND_LLM_SEMAPHORE`——按角色×优先级分配 LLM 并发额度与 token 预算，超预算自动降级模型或排队。

### 2. 黑板（`agency/blackboard/`）

- 新增 `agency_board` 系列表：board_items（type = asset / draft / review_note / decision / task；状态、版本号、生产者、消费者、story_id）。
- 分区：**资产区 / 草稿区 / 审查区 / 调度区**。单一写入者原则——每分区只有对应角色能直写，其他角色提交"提案"经协调器仲裁。
- 内存快照 + Tauri 事件推送前端；版本号乐观锁解决冲突。

### 3. 三角色（`agency/roles/`）

| 角色 | 模型角色 | 职责 | 工具白名单要点 |
|------|----------|------|----------------|
| 主创 LeadWriter | Creative | 概念→骨架→章节正文写作与修订，消费黑板资产与审查意见 | 读资产区/审查区，写草稿区 |
| 管理 Producer | Tool/Balanced | 资产生产供给（世界观/角色/大纲/伏笔/KG）、模型调度决策（经 router 打分）、工具/技能调用、进度与预算管理 | 写资产区/调度区，读全局 |
| 编辑审计 EditorAuditor | （LLM 驱动，取代纯规则 subagents） | 实时审查草稿：连续性/风格/世界观/合同兑现/AI 腔/追读力；结构化 ReviewNotes；执掌质量门 | 读草稿区/资产区，写审查区 |

### 4. 创世 2.0 流程（`agency/coordination/`）

1. **启动阶段**：三代理并行——主创出概念与首章，管理同步生产世界观/角色/大纲（现 quick+background 两阶段的角色化重组），保留"首章先行"体验。
2. **稳态循环**（章节级流水线并行）：主创写 N+1 ‖ 编辑审 N ‖ 管理备 N+2 资产。
3. **统一输出装配器**：只有通过质量门（grader 加权分 ≥ 阈值）的草稿才能写入 `scenes` 真源并投递前端；未过门带审查意见回流主创修订（上限 3 轮，超限转人工）。
4. 冲突仲裁、进度追踪、取消/暂停/恢复由协调器负责，状态落 `agency_runs` 表（`genesis_runs` 状态机的泛化）。

## ECC 机制移植映射

### 代币优化

| ECC 机制（源文件） | StoryMoss 落地 |
|---|---|
| agents frontmatter `model:` 按角色配模型（agents/*.md） | 角色×任务模型路由表落配置，复用 `ModelRole::Creative/Tool/Background` |
| 后台分析固定低价模型（llm-summary.js / observer-loop.sh 的 haiku 档） | 学习 analyzer、摘要等后台任务强制 Background 档 |
| SessionStart 注入 8000 字符硬上限（session-start.js） | 每角色系统提示模块化 + 注入按 token 预算硬截断（参数化）；`ContextPrioritizer` 升级为按预算裁剪 |
| agent-compress 三档目录（catalog/summary/full + lazyLoad） | 黑板/资产注入三档压缩，代理先读目录再按需取全文（迭代检索） |
| post-edit-accumulator 批量处理 + async hooks | 审计/观察/摘要走 task_system 异步；编辑累积→集中批量检查 |

### 记忆持久性

| ECC 机制 | StoryMoss 落地 |
|---|---|
| Stop hook 机械提取 + 廉价模型五段摘要双层（session-end.js） | `agency/session/` 会话快照：黑板状态+代理进度+决策日志定时落 `agency_sessions` 表；机械提取兜底 + Background 档模型摘要增强 |
| PreCompact 抢救性摘要 | 上下文压缩前（LlmService 侧）触发快照 |
| stale-replay 防护包装（issue #1534） | 恢复注入内容包"历史参考，非活指令"标记 |
| worktree 维度会话匹配 | 按 story 维度匹配与隔离 |
| ~/.claude/session-data 文件层 | `.storymoss` 工作区新增 `sessions/`、`learning/` 目录，沿用 WorkspaceService git 自动提交 |

### 持续学习（双轨）

| ECC 机制 | StoryMoss 落地 |
|---|---|
| observe.sh hook 式观察 → observations.jsonl（脱敏/轮转） | 观察点：LLM 调用、用户编辑生成内容、审查结论、修订接受/拒绝 → `.storymoss/learning/observations.jsonl`（脱敏、10MB 轮转） |
| observer-loop 后台分析 → instinct（trigger/action/confidence/证据/作用域） | 后台 analyzer（Background 档模型）→ instinct Markdown（frontmatter 同构） |
| 置信度：按次数初始化，+0.05 采纳 / −0.1 纠正 / −0.02 周衰减 | 同参数沿用 |
| /promote 跨项目晋升 + /evolve 物化 | 置信度 ≥0.8 且跨 story 复现 → 晋升提案 → **学习中心用户确认** → 物化为 SkillManifest 技能或 PromptRegistry 变体（经 `prompt_overrides` 表 V093） |
| 5 层防自观察守卫 | analyzer 会话与代理内部调用不打观察点；观察按 story 隔离 |

### 验证循环

| ECC 机制 | StoryMoss 落地 |
|---|---|
| grader 四级（code→rule→model→human，确定性优先） | 章节级：code（字数/格式/禁则）→ rule（合同红线/连续性，复用现有）→ model（五维审计+追读力 rubric 1-5 须引证据）→ human（用户采纳/修改率后置信号） |
| eval-harness：YAML 场景 + pass@k / pass^k + baseline.json | `evals/` 代理级场景（给定制版资产→期望行为/输出），pass@k（能力）与 pass^k（回归）双指标，baseline 纳入 CI |
| checkpoint 检查点对比（checkpoint.md） | 创世里程碑（概念/骨架/首章/每 5 章）自动快照指标，支持"现在 vs 当时"对比 |

### 并行化与子代理协调

| ECC 机制 | StoryMoss 落地 |
|---|---|
| worktree-per-worker + task/handoff/status 文件三件套 | 黑板分区 + 提案/决策表（内置 Rust 不需要 git worktree，语义同构） |
| 工具白名单能力隔离 | 角色工具白名单（见上） |
| iterative-retrieval 四相循环（DISPATCH→EVALUATE→REFINE→LOOP） | 资产检索采用迭代检索：先 catalog 宽泛匹配→相关度打分→按需取 full，最多 3 轮 |
| 子代理只回摘要 | 代理间消息与黑板写入强制摘要化，全文经引用拉取 |

## 前端可视化（代理工作室页）

新增页面（React + Zustand + TanStack Query + reactflow）：

- **三代理状态卡**：当前动作、使用模型、token/成本实时表；
- **黑板视图**：资产流/审查意见流/提案的实时展示；
- **代理时间线**：事件流回放（TraceStore trace_id 串联）；
- **学习中心**：instinct 列表、置信度、晋升确认 UI；
- **评估仪表盘**：章节评分趋势、质量门通过率、pass@k/pass^k。

## 替换与迁移策略

1. P1 端到端验收标准：创世 2.0 产出质量 ≥ 旧流程基线（eval harness 判定 + 人工抽检）。
2. 验收通过即在发布周期内切换：`smart_execute` 创世分支指向新框架，旧 `genesis.rs` / `agents/orchestrator.rs` TriShot 路径同版本删除。
3. 切换前打 git tag（如 `pre-agency-legacy`）留回退点。
4. `docs/` 八文档（ARCHITECTURE/ROADMAP/PROJECT_STATUS 等）与 AGENTS.md 同步更新；版本号 minor 升级。

## 实施分期

| 期 | 内容 | 验收 |
|----|------|------|
| P1 框架骨架 | runtime（ReAct+总线+工具注册表）+ blackboard + 三角色最小集 + **串行**协调，创世端到端跑通 | 串行模式产出完整首章；单测覆盖 runtime/黑板 |
| P2 并行化 + 切换 | 稳态并行循环、并发预算、统一输出装配；验收后**删除旧路径** | 并行稳态连续产出 3 章过质量门；旧代码移除，测试全绿 |
| P3 代币优化 + 记忆持久性 | 注入预算裁剪、三档目录、会话快照恢复 | 同等产出 token 成本可测下降；重启恢复现场 |
| P4 验证循环 | grader 分级、eval harness、检查点、评估仪表盘 | evals 场景纳入 CI；仪表盘数据正确 |
| P5 持续学习 + 可视化收尾 | 观察→instinct→晋升双轨、学习中心、代理工作室完整版 | 端到端学习闭环（观察→instinct→确认→技能生效） |

## 测试与错误处理

- 基线保护：`cargo test --lib`（770 passed）+ vitest（292 passed）全程保持绿；
- 新模块单测：runtime 循环、黑板仲裁、grader、晋升门槛；
- 故障隔离：单代理崩溃不阻塞黑板其他分区；LLM 失败走 model_gateway 候选链；质量门修订循环上限 3 轮；所有代理动作落 TraceStore 可回放。

## 风险

| 风险 | 缓解 |
|------|------|
| `genesis.rs`（3881 行）/ `orchestrator.rs`（4578 行）删除波及面大 | 替换前列调用点清单；architecture_guard 同步；tag 回退点 |
| ReAct 循环 token 成本不可控 | 角色白名单 + 轮数/成本熔断 + 并发预算；后台任务锁 Background 档 |
| 学习产物污染创作 | 置信度门槛 + 人工确认 + 作用域隔离 + 周衰减 |
| 三代理并行产出不一致 | 统一输出装配器质量门；scenes 单一写入者 |
