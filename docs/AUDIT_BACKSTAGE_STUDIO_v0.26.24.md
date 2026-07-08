# StoryForge 幕后工作室（Backstage）全面审计报告

> **版本基准**：v0.26.24  
> **审计日期**：2026-07-07  
> **审计范围**：`index.html` → `App.tsx` 及幕后全部侧栏模块（19 页 + 设置 10 子 Tab + 关联组件/Hooks/IPC）  
> **审计方法**：代码检视（executed）、与 `docs/USER_GUIDE.md` / `ARCHITECTURE.md` 对照、Genesis 后端步骤契约交叉验证  
> **关联文档**：`docs/AUDIT_后台资产与智能创作流程.md`（2026-06-20 后端资产审计）、`docs/plans/2026-07-06-genesis-audit-and-optimization-design.md`（Genesis P0 设计）

---

## 0. 执行摘要

### 0.1 核心结论

StoryForge 幕后工作室在 **资产消费与配置** 层面功能覆盖面广（19 个导航模块），但存在三类系统性问题：

1. **创世（Genesis）路径分裂**：幕前 `smart_execute` + Genesis Pipeline 是主创世路径；幕后另有 `NovelCreationWizard` + `createStoryWithWizard` 与 `runCreationWorkflow` 两条平行路径，三者语义重叠、UI 无决策引导，用户易误判「已走完整创世」。
2. **Genesis 观测与消费断层**：`GenesisPanel` 步骤模型与后端 Quick（2 步）+ Background（6 步）不一致；v0.26.19 的 `steps_json.errors` 非致命错误未在面板展示；L2 资产页除伏笔外均无「创世来源」溯源 UI。
3. **文档与实现漂移**：USER_GUIDE 承诺的「AI 生成角色/世界观」「统计卡片可点击」「叙事分析图表」「用量时间维度」等，在 L2/L4 层多数未实现；TracingPanel / Logs / 意图图已实现但未文档化。

### 0.2 分层完整度评分

| 层级 | 范围 | Genesis 关系 | 完整度 | 主要短板 |
|------|------|-------------|--------|----------|
| **L1** | 仪表盘、故事库、GenesisPanel、Wizard | 触发/观测 | **70%** | 三路径混乱；Panel 步骤/errors 不对齐 |
| **L2** | 场景、角色、世界构建、伏笔、知识图谱 | **产出主消费面** | **75%** | 角色无编辑/关系 CRUD；溯源缺失 |
| **L3** | 设置、技能、MCP、Story System | 配置与契约 | **85%** | 合同播种状态不可见 |
| **L4** | 叙事分析、拆书、统计、链路、日志 | 事后诊断 | **70%** | 图表/分组缺失；工具未互链 |
| **L5** | 任务、登录、更新、Sync | 基础设施 | **90%** | 任务与 Genesis pipeline 独立 |

### 0.3 优先行动（Top 5）

| 优先级 | 项 | 预期收益 |
|--------|-----|----------|
| P0 | 重构 `GenesisPanel`（动态步骤 + errors + story 跳转） | 对齐 v0.26.19 可观测性契约 |
| P0 | 统一 L1 创作入口 UX + 修复 Stories Wizard 重复建故事 | 消除用户路径误判 |
| P1 | L2 角色编辑 + 关系 CRUD + 创世溯源徽章 | 补齐最大功能空洞 |
| P1 | Story System 合同播种状态卡 | Genesis 契约落地可见 |
| P2 | USER_GUIDE 与 L4 诊断页文档化 + UsageStats 按 operation 分组 | 降低支持成本 |

详细实施方案见：**[`docs/plans/2026-07-07-backstage-studio-audit-implementation-plan.md`](./plans/2026-07-07-backstage-studio-audit-implementation-plan.md)**

---

## 1. 架构总览

### 1.1 入口与导航

| 层级 | 文件 | 职责 |
|------|------|------|
| 入口 | `src-frontend/index.html` → `main.tsx` → `App.tsx` | 幕后主壳 |
| 导航 | `components/Sidebar.tsx` | 19 个 `ViewType` + 「开幕前写作」 |
| 全局状态 | `stores/appStore.ts` | `currentStory` / `currentView` |
| 数据缓存 | TanStack Query + hooks | 按 storyId 分区 |
| 实时同步 | `hooks/useSyncStore.ts` | 后端 store-events → invalidate Query |
| 幕前联动 | `backstage-update` / `backstage-shown` | 内容变更通知、窗口恢复刷新 |

### 1.2 侧栏模块清单

```
仪表盘 | 故事 | 角色 | 世界构建 | 场景 | 知识图谱 | 技能 | MCP | 拆书
任务 | 伏笔看板 | 叙事分析 | Story System | 用量统计 | 写作统计
意图图 | 日志 | 生成链路 | 设置
```

设置子 Tab（10）：模型管理、路由模拟器、模型健康、Agent 配置、创作方法论、工作流、通用设置、提示词、数据统计、账号与登录。

### 1.3 三条创作启动路径

```
┌─────────────────────────────────────────────────────────────────┐
│ 路径 A（主 Genesis）— 幕前                                        │
│   用户「新写 XX 小说」→ smart_execute                             │
│   Quick: 构思故事 → 撰写开篇（30–60s，第一章自动接受）               │
│   Background: 策略 → 世界骨架 → 场景 → 伏笔 → KG → 合同（6 步）    │
├─────────────────────────────────────────────────────────────────┤
│ 路径 B — 幕后 Wizard                                              │
│   Dashboard/Stories → NovelCreationWizard → createStoryWithWizard │
│   预置世界观/角色/文风/首场景 + KG ingest；不触发 Genesis Pipeline │
├─────────────────────────────────────────────────────────────────┤
│ 路径 C — 幕后快速创作                                              │
│   Stories → AI 创作 → runCreationWorkflow                         │
│   CreationWorkflowEngine 多阶段；与 Genesis 不同引擎               │
└─────────────────────────────────────────────────────────────────┘
```

**判定**：真正「智能创作流程-创世」= **路径 A**。幕后 L1 负责观测（GenesisPanel）与预置资产（B/C），不负责触发 A。

### 1.4 Genesis 后端步骤契约（executed：`genesis.rs`）

| 阶段 | 步骤 | step name |
|------|------|-----------|
| Quick（2） | 1 | 构思故事 |
| Quick（2） | 2 | 撰写开篇 |
| Background（6） | 1 | 选择创作策略 |
| Background（6） | 2 | 构建世界与骨架（内部 emit 构建世界/故事大纲/塑造角色） |
| Background（6） | 3 | 场景规划 |
| Background（6） | 4 | 埋设伏笔 |
| Background（6） | 5 | 知识图谱 |
| Background（6） | 6 | 播种故事合同 |

`GenesisPanel` 硬编码 8 步（缺「选择创作策略」「播种故事合同」；将一步拆成三步），与后端 `total_steps` / `step_number` 可能不一致。

---

## 2. L1 — 创作入口与 Genesis 观测

### 2.1 仪表盘 `Dashboard.tsx`

| 能力 | IPC/组件 | 状态 |
|------|----------|------|
| AI/手动创建 | `useCreateStory`, `NovelCreationWizard`, `createStoryWithWizard` | ✅ |
| Genesis 运行记录 | `GenesisPanel` embedded | ⚠️ 步骤/errors 不对齐 |
| 统计卡片 | stories/characters/chapter_count 聚合 | ⚠️ 「场景」误用 chapter_count；不可点击 |
| 最近编辑 | 跳转 scenes | ✅ |

### 2.2 故事库 `Stories.tsx`

| 能力 | 说明 | 状态 |
|------|------|------|
| CRUD + 导出 + Style DNA/Blend | 完整 | ✅ |
| 故事概览 | 大纲/场景 stage/伏笔/AI 操作回滚 | ✅ |
| AI 菜单 | 快速创作 + 向导创作 + 三模式 | ⚠️ 与 Genesis 路径无说明 |
| **Wizard  on 已有故事** | 调用 `createStoryWithWizard` **新建**故事 | ❌ 可能重复建库 |

### 2.3 GenesisPanel `components/GenesisPanel.tsx`

| 能力 | IPC | 状态 |
|------|-----|------|
| 列表/详情 | `listGenesisRuns`, `getGenesisRun` | ✅ |
| 实时进度 | `usePipelineProgress(genesis)` | ✅ |
| 暂停 | `cancelGenesisPipeline` | ✅ |
| errors 数组 | `steps_json.errors` | ❌ 未展示 |
| genesis-warnings | 事件监听 | ❌ 仅幕前 |
| 跳转 story | — | ❌ |

### 2.4 NovelCreationWizard

- 策略 → 世界观 → 角色 → 文风 → 首场景：流程完整 ✅  
- 与 Genesis `StrategySelectionStep` 双轨，可能策略不一致 ⚠️

---

## 3. L2 — Genesis 产出资产层（深度）

### 3.1 场景 `Scenes.tsx`

#### 组件树

```
Scenes
├── StoryTimeline（左：序列/拖拽/execution_stage 色带）
├── SceneEditor（中：编辑态）
│   ├── SceneAuditPanel（审校 → audit_scene）
│   └── SceneAnnotationPanel（批注）
├── VersionTimeline + DiffViewer（预览态）
├── PipelinePanel（右：编辑态 → refine/review/finalize）
└── ExecutionPanel（右：浏览态 → 叙事相位 + 主行动）
```

#### IPC 映射

| 能力 | 命令 |
|------|------|
| CRUD/排序 | `get_story_scenes`, `create_scene`, `update_scene`, `delete_scene`, `reorder_scenes` |
| AI 大纲/草稿 | `generate_scene_outline`, `generate_scene_draft` |
| 审校 | `audit_scene` |
| Pipeline | `run_refine`, `run_review`, `run_finalize`, `merge_revision`… |
| 叙事状态 | `get_canonical_state`, `get_story_word_count` |

#### Genesis 数据流

`SceneGenerationStep` → scenes metadata；`FirstChapterGenerationStep` → scenes[0].content → 幕前自动接受。

#### 问题清单

| ID | 严重度 | 问题 |
|----|--------|------|
| S-1 | 中 | 预览态无 AI 入口（文档承诺扩写/润色） |
| S-2 | 中 | SceneEditor 五阶段 vs PipelinePanel 流水线概念重复 |
| S-3 | 低 | 无 Genesis 场景溯源标记 |
| S-4 | 中 | `continue_writing` 不打开幕前 |
| S-5 | 低 | PipelinePanel 使用 `sequence_number` 作 chapterNumber，需 UI 标注 |

**完整度**：★★★★☆

---

### 3.2 角色 `Characters.tsx`

#### 组件树

```
Characters
├── Tab 资料：卡片 + CharacterStatePanel（动态状态）
├── Tab 关系：RelationshipCard（只读）
└── 创建 Modal（仅 create，无 edit）
```

#### IPC 映射

| 能力 | UI | API |
|------|-----|-----|
| 列表/创建/删除 | ✅ | `get_story_characters`, `create_character`, `delete_character` |
| 更新资料 | ❌ | `update_character`（hook 存在未用） |
| 动态状态 | ✅ | `update_character_state` |
| 关系 CRUD | ❌ | `createCharacterRelationship` 等仅在 `genesis.ts` |

#### Genesis 数据流

`ParallelWorldOutlineCharacterStep` → characters + relationships（失败记入 genesis errors）。

#### 问题清单

| ID | 严重度 | 问题 |
|----|--------|------|
| C-1 | **高** | 无编辑角色入口 |
| C-2 | **高** | 无关系手动增删改 |
| C-3 | **高** | 文档「AI 生成角色」未实现 |
| C-4 | 中 | 无「关联场景」跳转 |
| C-5 | 中 | 无创世溯源；`Character` 类型无 source 字段 |

**完整度**：★★☆☆☆（L2 最大空洞）

---

### 3.3 世界构建 `WorldBuilding.tsx`

| 区块 | IPC | 状态 |
|------|-----|------|
| 核心概念/规则/文化/历史 | `get/update_world_building` | ✅ 编辑强 |
| 空态初始化 | `create_world_building` | ✅ |
| AI 生成世界观 | — | ❌ |
| 文风 WritingStyle | `useWritingStyle` hook 存在 | ❌ 本页未用 |

**完整度**：★★★☆☆

---

### 3.4 伏笔看板 `Foreshadowing.tsx`

| 能力 | IPC | 状态 |
|------|-----|------|
| CRUD + 状态变更 | `get/create/update_foreshadowing*` | ✅ |
| Payoff Ledger + 逾期 + 推荐 | `get_payoff_ledger`, `detect_overdue`, `recommend_payoff_timing` | ✅ |
| 创世徽章 | `is_auto_generated` | ✅ **L2 唯一溯源** |
| Kanban 列视图 | — | ❌（列表+统计） |
| setup_scene 选择 | 手填 ID | ⚠️ 应改下拉 |
| Ledger 字段编辑 | hook 有 UI 无 | ⚠️ |

**完整度**：★★★★★（L2 最强）

---

### 3.5 知识图谱 `KnowledgeGraph.tsx`

| Tab | 功能 | 状态 |
|-----|------|------|
| graph | 力导向可视化 | ✅ |
| memory | Retention + 归档遗忘实体 | ✅ |
| archived | 恢复 | ✅ |
| distillation | 知识蒸馏摘要 | ✅ |
| 手动 entity/relation CRUD | API 有 | ❌ UI 无 |
| Genesis 节点溯源 | — | ❌ |

**完整度**：★★★★☆

---

### 3.6 L2 Genesis 消费矩阵

| Genesis 步骤 | 写入 | L2 页 | 可编辑 | 可溯源 |
|-------------|------|-------|--------|--------|
| 撰写开篇 | scenes[0].content | Scenes | ✅ | ❌ |
| 构建世界与骨架 | world + outline | WorldBuilding, Stories | ✅ | ❌ |
| 塑造角色 | characters, rels | Characters | 部分 | ❌ |
| 场景规划 | scenes[] | Scenes | ✅ | ❌ |
| 埋设伏笔 | foreshadowings | Foreshadowing | ✅ | ✅ |
| 知识图谱 | entities/relations | KnowledgeGraph | 部分 | ❌ |
| 播种合同 | contracts | Story System (L3) | ✅ | ❌ |

---

## 4. L3 — AI 基础设施层

### 4.1 设置子系统

| Tab | Genesis/续写关联 | 完整度 |
|-----|-----------------|--------|
| 模型管理 | 全部 LLM | ★★★★★ |
| 路由模拟器 | Agent 路由调试 | ★★★★ |
| 模型健康 | 死模型 skip | ★★★★ |
| Agent 配置 | Writer/Planner 等 | ★★★★★ |
| 创作方法论 | Genesis 提示词 | ★★★★ |
| 工作流 | 只读模板列表 | ★★☆ |
| 通用设置 | `smart_execute_total_timeout_secs` 等 | ★★★★★ |
| 提示词 | **`Creation` 类 = 创世模板** | ★★★★★ |
| 数据统计 | feature telemetry | ★★★ |
| 账号 | 订阅 | ★★★ |

**缺口**：无「创世/续写」配置分组导航；PromptsPanel 与 Genesis 步骤无交叉链接。

### 4.2 技能 `Skills.tsx`

- 10 类分类、import/enable/execute/edit：完整 ★★★★☆  
- 与 Genesis 无直接耦合；幕前调用，幕后管理

### 4.3 MCP `Mcp.tsx`

- 内置 + 外部 server：★★★☆☆  
- `connectMcpServer` 标记 deprecated；与 Genesis 无集成

### 4.4 Story System `StorySystem.tsx`

| Tab | Genesis 关联 |
|-----|-------------|
| contracts | **ContractSeedingStep 产出** MASTER_SETTING + CHAPTER_1 |
| commits | 章节版本化 |
| reading | 追读力 / Chase Debt |
| memory | Memory Pack |
| audit / anti-ai | 质量闸门 |
| genres / style-dna | StrategySelection 选用 |

**缺口**：无「合同已由创世播种」状态指示；Projection Health 入口不显著。

---

## 5. L4 — 分析与诊断层

| 模块 | 数据源 | Genesis 关联 | 完整度 | 文档 |
|------|--------|-------------|--------|------|
| 叙事分析 | `analyzeNarrativeStructure`, events, threads, deep_insight | 间接 | ★★★（无图表） | 过度承诺 |
| 拆书 | `pipeline_type: analysis` | 独立 | ★★★（转故事不导航） | OK |
| 用量统计 | `getLlmCallStats` | 混在 general | ★★★（无时间/功能分组） | 过度承诺 |
| 写作统计 | `getWritingAnalytics` | 无 | ★★★★ | OK |
| **生成链路** | `listRecentGenerationTraces` | **直接追踪 smart_execute** | ★★★★ | **未文档化** |
| **意图图** | `get_intention_graph_diagnostics` | Orchestrator | ★★★★ | **未文档化** |
| **日志** | workflow + system logs | creative_workflow | ★★★★ | **未文档化** |

---

## 6. L5 — 运维扩展层

| 模块 | 说明 | 完整度 |
|------|------|--------|
| 任务 `Tasks.tsx` | CRUD、cron、心跳、级联改写 | ★★★★☆ |
| DataLoader | 启动 hydration | ★★★★★ |
| **useSyncStore** | Genesis 后台写入 → L2 Query 自动刷新 | ★★★★★ **关键联动** |
| ConnectionStatus / Updater / Login | 基础设施 | ★★★★★ |

---

## 7. 横切：useSyncStore 与 Genesis

Genesis Background 阶段写入 character/scene/world/foreshadowing/KG 时，`useSyncStore` 监听 store-events 并 invalidate 对应 QueryKey。**这是 L2 与 Genesis 之间最重要的自动联动**，使用户在仪表盘看 Genesis 进度时，切到 L2 页无需手动刷新即可看到新资产。

监听覆盖：`characters`, `scenes`, `chapters`, `world_building`, `foreshadowings`, `knowledge-graph`, `character-relationships`, `story-outline` 等。

---

## 8. 文档 vs 实现差异

| USER_GUIDE 承诺 | 实际 | 严重度 |
|----------------|------|--------|
| 仪表盘统计卡片可点击 | 不可点击 | 低 |
| 角色「AI 生成角色」 | 无 | 中 |
| 世界构建「AI 生成世界观」 | 仅初始化空壳 | 中 |
| 角色「关联场景」 | 无 | 中 |
| 伏笔 Kanban 列视图 | 列表+统计 | 低 |
| 叙事分析图表 | 文本/条形 | 中 |
| 用量今日/本周/本月 | 无 | 低 |
| 意图图/生成链路/日志 | 已实现 | 中（文档缺失） |
| Genesis errors 仪表盘详情 | Panel 未展示 errors[] | **高** |

---

## 9. 风险 register

| 风险 | 影响 | 缓解 |
|------|------|------|
| 三路径创作混淆 | 用户以为 Wizard = Genesis | L1 决策引导卡 + 文档 |
| Stories Wizard 重复建故事 | 数据重复 | 已有故事走 update 路径 |
| GenesisPanel 进度失真 | 误判卡住/完成 | 动态步骤模型 |
| 角色页无法编辑 Genesis 产出 | 用户改设定困难 | P1 补 CRUD |
| 合同未播种不可见 | 续写质量下降难排查 | Story System 状态卡 |
| SceneEditor/Pipeline 双轨 | 学习成本高 | 统一命名/指南 |

---

## 10. 审计方法说明

- **executed**：直接阅读 `src-frontend/src/pages/*`, `components/*`, `hooks/*`, `genesis.rs`  
- **inspected**：对照 `docs/USER_GUIDE.md`, `ARCHITECTURE.md`, 既有审计文档  
- **assumed**：未做 Playwright 全路径 E2E；Windows/Linux 幕后 UI 未逐平台截图验证  

---

## 11. 参考文件索引

| 类型 | 路径 |
|------|------|
| 幕后入口 | `src-frontend/src/App.tsx` |
| 导航 | `src-frontend/src/components/Sidebar.tsx` |
| Genesis 面板 | `src-frontend/src/components/GenesisPanel.tsx` |
| Genesis 后端 | `src-tauri/src/narrative/genesis.rs` |
| 同步中心 | `src-frontend/src/hooks/useSyncStore.ts` |
| 用户指南 | `docs/USER_GUIDE.md` |
| 实施计划 | `docs/plans/2026-07-07-backstage-studio-audit-implementation-plan.md` |

---

_审计人：AI Assistant（Cursor Agent）_  
_下次复审建议：Phase 1 落地后（目标 v0.26.25）_
