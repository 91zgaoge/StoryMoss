# StoryMoss 幕后工作室（Backstage）全面审计报告

> **版本基准**：v0.26.34  
> **审计日期**：2026-07-09  
> **相对基线**：[`docs/AUDIT_BACKSTAGE_STUDIO_v0.26.24.md`](./AUDIT_BACKSTAGE_STUDIO_v0.26.24.md)（2026-07-07）  
> **审计范围**：幕后全部侧栏模块（19 页）+ 设置 10 子 Tab + 关联组件 / Hooks / IPC / Genesis 后端契约  
> **审计方法**：代码检视（executed）+ 与 v0.26.24 审计对照 + `PROJECT_STATUS` / `USER_GUIDE` / `genesis.rs` 交叉验证  
> **关联文档**：  
> - [`docs/plans/2026-07-07-backstage-studio-audit-implementation-plan.md`](./plans/2026-07-07-backstage-studio-audit-implementation-plan.md)  
> - [`docs/AUDIT_后台资产与智能创作流程.md`](./AUDIT_后台资产与智能创作流程.md)

---

## 0. 执行摘要

### 0.1 一句话结论

自 v0.26.24 审计以来，**Phase 1–4 与 Stage 1–4 的绝大部分 P0/P1 项已落地**（GenesisPanel、创作路径引导、角色 CRUD、溯源徽章、合同状态卡、诊断互链、策略前移 Quick Phase、提示词外部化等）。幕后工作室已从「资产丰盛但观测断层」进入 **「观测闭环基本成立、局部体验与数据口径仍有债」** 阶段。

当前主要矛盾不再是「功能缺失」，而是：

1. **数据口径与语义残留**（仪表盘「场景」仍用 `chapter_count`）  
2. **路径引导与真实行为不完全一致**（CreationPathGuide「快速创作」实际打开手动创建）  
3. **Wizard 应用路径半闭环**（`applyWizardToStory` 无 KG 摄取、可能重复角色）  
4. **幕后仍不监听 `genesis-warnings`**（仅幕前 toast）  
5. **SceneEditor 五阶段 vs PipelinePanel 双轨** 仍未统一

### 0.2 分层完整度（相对 v0.26.24）

| 层级 | 范围 | v0.26.24 | **v0.26.34** | 变化 |
|------|------|----------|--------------|------|
| **L1** | 仪表盘 / 故事 / GenesisPanel / Wizard | 70% | **90%** | ↑ 路径引导、Panel 动态步骤、errors、互链 |
| **L2** | 场景 / 角色 / 世界 / 伏笔 / KG | 75% | **92%** | ↑ 角色编辑/关系/AI、溯源、世界 AI、KG CRUD |
| **L3** | 设置 / 技能 / MCP / Story System | 85% | **93%** | ↑ 合同播种卡、提示词目录/导入修复 |
| **L4** | 叙事分析 / 拆书 / 统计 / 链路 / 日志 | 70% | **88%** | ↑ 图表、Usage 分组、USER_GUIDE 3.17–3.19 |
| **L5** | 任务 / Sync / 登录 / 更新 | 90% | **92%** | → 稳定；Sync 仍是 Genesis→L2 关键联动 |

### 0.3 v0.26.24 → v0.26.34 已关闭项对照

| 原审计项 | 状态 | 落地版本 |
|----------|------|----------|
| GenesisPanel 硬编码 8 步 / 缺 errors | ✅ 已关闭 | v0.26.25–28（`genesisSteps.ts` + Panel） |
| L1 三路径无引导 | ✅ 已关闭 | v0.26.32（`CreationPathGuide`） |
| Stories Wizard 重复建故事 | ✅ 已关闭 | `applyWizardToStory` |
| 仪表盘统计不可点击 | ✅ 已关闭 | v0.26.32 |
| 角色无编辑 / 无关系 CRUD / 无 AI | ✅ 已关闭 | v0.26.28–33 |
| L2 溯源徽章缺失 | ✅ 已关闭 | v0.26.28–31（含 schema 兜底） |
| Story System 合同播种不可见 | ✅ 已关闭 | `ContractsTab` |
| Scenes 续写不打开幕前 | ✅ 已关闭 | `ExecutionPanel` → `show_frontstage` |
| Tracing / Logs 未文档化、未互链 | ✅ 已关闭 | v0.26.27 + USER_GUIDE 3.17–3.19 |
| UsageStats 无 operation 分组 | ✅ 已关闭 | 前端启发式分组 |
| 伏笔 setup_scene 手填 ID | ✅ 已关闭 | `<select>` + `useScenes` |
| 世界构建无 AI 生成 | ✅ 已关闭 | v0.26.28 |
| KG 无手动 CRUD | ✅ 已关闭 | v0.26.28–33 |
| 策略选择在 Background | ✅ 已关闭 | v0.26.28 前移 Quick Phase |
| 提示词导入 `promptId` 静默失败 | ✅ 已关闭 | v0.26.34 |

### 0.4 当前残留 Top 问题（按优先级）

| ID | 优先级 | 问题 | 影响 |
|----|--------|------|------|
| R1 | **P1** | Dashboard「场景」统计仍聚合 `chapter_count` | 标签与数据口径不一致，误导用户 |
| R2 | **P1** | `CreationPathGuide.onQuick` → 打开「手动创建」而非 Stories 快速创作 | 路径说明与行为不符 |
| R3 | **P1** | `applyWizardToStory` 无 KG 摄取；重复跑会叠加角色/场景 | Wizard 半闭环、数据膨胀 |
| R4 | **P2** | 幕后 `App`/`GenesisPanel` 不监听 `genesis-warnings` | 用户在幕后看不到非致命错误 toast |
| R5 | **P2** | SceneEditor 五阶段 vs PipelinePanel 双轨未统一 | 学习成本高、语义混淆 |
| R6 | **P2** | PipelinePanel 仍用 `sequence_number` 作 `chapterNumber` | 场景优先架构下易误解 |
| R7 | **P2** | WorldBuilding 无 Writing Style 编辑 Tab | 文风数据无处维护 |
| R8 | **P3** | UsageStats 分组靠字符串启发式，非后端 `operation_type` | 分组可能不准 |
| R9 | **P3** | 伏笔仍非 Kanban；Ledger 字段编辑 UI 仍弱 | 体验债 |
| R10 | **P3** | 角色「关联场景」跳转仍无 | 文档/体验小缺口 |

---

## 1. 架构总览（当前态）

### 1.1 入口与导航

| 层级 | 文件 | 职责 |
|------|------|------|
| 入口 | `index.html` → `main.tsx` → `App.tsx` | 幕后主壳 |
| 导航 | `components/Sidebar.tsx` | 19 个 `ViewType` + 「开幕前写作」 |
| 全局状态 | `stores/appStore.ts` | `currentStory` / `currentView` / Genesis↔诊断深链字段 |
| 数据缓存 | TanStack Query + hooks | 按 storyId 分区 |
| 实时同步 | `hooks/useSyncStore.ts` | store-events → invalidate Query |
| 幕前联动 | `backstage-update` / `backstage-shown` | 内容变更、窗口恢复刷新 |

**appStore 新增深链字段（v0.26.27+）**：

- `selectedGenesisSessionId` / `setSelectedGenesisSessionId`
- `tracingFilter` / `setTracingFilter`
- `logsSearchQuery` / `setLogsSearchQuery`

### 1.2 侧栏模块清单（未变）

```
仪表盘 | 故事 | 角色 | 世界构建 | 场景 | 知识图谱 | 技能 | MCP | 拆书
任务 | 伏笔看板 | 叙事分析 | Story System | 用量统计 | 写作统计
意图图 | 日志 | 生成链路 | 设置
```

设置子 Tab（10）：模型管理、路由模拟器、模型健康、Agent 配置、创作方法论、工作流、通用设置、提示词、数据统计、账号与登录。

### 1.3 三条创作路径（当前 UX）

| 路径 | 入口 | 引擎 | Genesis Pipeline |
|------|------|------|------------------|
| **A 推荐** | Dashboard「AI 创建故事」/ CreationPathGuide 幕前卡 → `show_frontstage` | `smart_execute` + Genesis | ✅ |
| **B** | CreationPathGuide「幕后 AI 向导」/ Stories 向导 | `NovelCreationWizard` + `createStoryWithWizard` 或 `applyWizardToStory` | ❌ 预置资产 |
| **C** | Stories「AI 创作 → 快速创作」 | `runCreationWorkflow` | ❌ 另一引擎 |

**判定**：主创世仍是路径 A。L1 已有明确引导卡；但 Guide 的「快速创作」回调在 Dashboard 误绑到手动创建（见 R2）。

### 1.4 Genesis 后端步骤契约（v0.26.28+）

| 阶段 | 步数 | 步骤名 |
|------|------|--------|
| Quick | 3 | 构思故事 → **选择创作策略** → 撰写开篇 |
| Background | 5 | 构建世界与骨架 → 场景规划 → 埋设伏笔 → 知识图谱 → 播种故事合同 |

前端 `genesisSteps.ts` 常量已对齐：

```ts
QUICK_PHASE_STEP_NAMES = ['构思故事', '选择创作策略', '撰写开篇']
BACKGROUND_PHASE_STEP_NAMES = ['构建世界与骨架', '场景规划', '埋设伏笔', '知识图谱', '播种故事合同']
```

与 `genesis.rs` 单测契约一致（executed）。

---

## 2. L1 — 创作入口与 Genesis 观测

### 2.1 仪表盘 `Dashboard.tsx`

| 能力 | 状态 | 说明 |
|------|------|------|
| AI 创建故事主按钮 | ✅ | 改为 `show_frontstage`（走 Genesis） |
| CreationPathGuide | ✅ | 三卡可点击 |
| 统计卡可点击 | ✅ | 跳转 stories/characters/scenes |
| 字数统计卡 | ✅ | 新增 |
| GenesisPanel embedded | ✅ | 动态步骤 + errors |
| 手动创建 / Wizard 新建 | ✅ | Wizard 仍走 `createStoryWithWizard`（新建合理） |

**残留 R1**：

```ts
const totalScenes = statsSource.reduce((sum, s) => sum + (s.chapter_count || 0), 0);
// label: '场景'
```

`Story` 类型仅有 `chapter_count` / `character_count` / `word_count`，**无 `scene_count`**。标签已改为「场景」，数据仍是章节数。后端已有 `get_story_word_count` 返回 `scene_count`，但未用于仪表盘聚合。

**残留 R2**：`CreationPathGuide onQuick={() => setIsModalOpen(true)}` 打开的是手动创建表单，与文案「幕后快速创作 / runCreationWorkflow」不符。

**完整度**：★★★★☆

---

### 2.2 故事库 `Stories.tsx`

| 能力 | 状态 |
|------|------|
| CRUD / 导出 / Style DNA / Blend | ✅ |
| 故事概览（大纲/stage/伏笔/AI 操作回滚） | ✅ |
| CreationPathGuide | ✅ |
| 已有故事 Wizard → `applyWizardToStory` | ✅ 不再重复建库 |
| 快速创作 `runCreationWorkflow` | ✅ |

**残留 R3（`applyWizardToStory`）**：

- 注释已声明：无 KG 摄取、无事务原子性  
- 每次应用都会 `create_character` / `create_scene(sequence=1)`，**重复运行会叠加角色与场景**，无去重/替换策略  
- 相对 `createStoryWithWizard` 的 `ingested_entities` 反馈缺失

**完整度**：★★★★☆

---

### 2.3 GenesisPanel + `genesisSteps.ts`

| 能力 | 状态 |
|------|------|
| 动态步骤（Quick3+Background5） | ✅ |
| `steps_json.errors` 分级展示 | ✅ |
| 进度百分比由步骤状态计算 | ✅ |
| 打开幕前 | ✅ |
| 查看生成链路（tracingFilter） | ✅ |
| 查看日志（logsSearchQuery） | ✅ |
| session 深链选中 | ✅ |
| 暂停 pipeline | ✅ |

**残留 R4**：`genesis-warnings` 事件仅在 `FrontstageApp` 监听；`App.tsx` / `GenesisPanel` **无 listen**。用户若只开幕后，非致命错误依赖轮询 `getGenesisRun` 刷新 `steps_json`，无即时 toast。

**完整度**：★★★★★（观测主路径已闭环；实时 toast 仍偏幕前）

---

### 2.4 NovelCreationWizard

- 策略 → 世界 → 角色 → 文风 → 首场景：完整 ✅  
- Dashboard 新建走 `createStoryWithWizard`（含 KG ingest）✅  
- Stories 已有故事走 `applyWizardToStory`（半闭环）⚠️  
- 与 Genesis Quick Phase 策略选择仍双轨，但策略已前移，首章可消费 `selected_strategy` ✅

---

## 3. L2 — Genesis 产出资产层

### 3.1 场景 `Scenes.tsx`

#### 组件树（未变结构，能力增强）

```
Scenes
├── StoryTimeline（含 is_auto_generated「创世」徽章）
├── SceneEditor（规划→大纲→草稿→审校→定稿 + 批注）
│   ├── SceneAuditPanel
│   └── SceneAnnotationPanel
├── VersionTimeline + DiffViewer
├── PipelinePanel（refine/review/finalize）
└── ExecutionPanel（续写 → show_frontstage ✅）
```

| 维度 | 评分 | 说明 |
|------|------|------|
| CRUD / 版本 / AI 大纲草稿 | ★★★★★ | 完整 |
| Genesis 溯源 | ★★★★☆ | Timeline 有创世徽章 |
| 续写跳转幕前 | ★★★★★ | 已修 |
| 双轨语义 | ★★☆☆☆ | R5/R6 仍在 |

**R5/R6**：编辑态右侧 `PipelinePanel` 与 Editor 内五阶段并行；`chapterNumber={selectedScene.sequence_number}` 未改。

---

### 3.2 角色 `Characters.tsx`（原最大空洞 → 已补齐）

| 能力 | v0.26.24 | v0.26.34 |
|------|----------|----------|
| 创建 / 删除 | ✅ | ✅ |
| 编辑 Modal | ❌ | ✅ `CharacterEditModal` |
| 关系创建 | ❌ | ✅ `CharacterRelationshipForm` |
| 关系删除 | ❌ | ✅ `useDeleteCharacterRelationship` |
| AI 扩展 | ❌ | ✅ `generateCharacterProfiles` |
| 创世徽章 | ❌ | ✅ `is_auto_generated` |
| 动态状态 | ✅ | ✅ `CharacterStatePanel` |
| 关联场景跳转 | ❌ | ❌ **R10** |

**完整度**：★★★★☆（从 ★★ 跃升）

---

### 3.3 世界构建 `WorldBuilding.tsx`

| 能力 | 状态 |
|------|------|
| 规则 / 文化 / 概念 / 历史编辑 | ✅ |
| AI 生成世界观 Modal | ✅ |
| 创世徽章 | ✅ |
| Writing Style Tab | ❌ **R7**（`useWritingStyle` hook 存在，页未用） |

**完整度**：★★★★☆

---

### 3.4 伏笔看板 `Foreshadowing.tsx`

| 能力 | 状态 |
|------|------|
| Ledger / 逾期 / 推荐 | ✅ |
| 创世徽章 | ✅ |
| setup_scene `<select>` | ✅ |
| Kanban 列视图 | ❌ **R9** |
| Ledger 字段高级编辑 | ⚠️ 仍弱 |

**完整度**：★★★★★（L2 仍最强）

---

### 3.5 知识图谱 `KnowledgeGraph.tsx`

| Tab / 能力 | 状态 |
|------------|------|
| graph / memory / archived / distillation | ✅ |
| 新建实体 / 添加关系 | ✅ |
| 归档实体 / 删除关系 | ✅（v0.26.33） |
| 创世溯源（实体详情） | ✅ `is_auto_generated` |

**完整度**：★★★★★

---

### 3.6 L2 Genesis 消费矩阵（更新）

| Genesis 步骤 | 写入 | L2 页 | 可编辑 | 可溯源 |
|-------------|------|-------|--------|--------|
| 撰写开篇 | scenes[0].content | Scenes | ✅ | ✅ |
| 构建世界与骨架 | world + outline + characters | World / Stories / Characters | ✅ | ✅ |
| 场景规划 | scenes[] | Scenes | ✅ | ✅ |
| 埋设伏笔 | foreshadowings | Foreshadowing | ✅ | ✅ |
| 知识图谱 | entities/relations | KnowledgeGraph | ✅ | ✅ |
| 播种合同 | contracts | Story System | ✅ | ✅（状态卡） |

---

## 4. L3 — AI 基础设施层

### 4.1 设置子系统

| Tab | 完整度 | 备注 |
|-----|--------|------|
| 模型管理 / 路由 / 健康 / Agent | ★★★★★ | |
| 创作方法论 | ★★★★ | |
| 工作流 | ★★☆ | 仍只读模板列表 |
| 通用设置 | ★★★★★ | 含 `smart_execute_total_timeout_secs` |
| **提示词** | ★★★★★ | v0.26.34：`prompt_id` 修复、打开目录、刷新、错误展示 |
| 数据统计 / 账号 | ★★★ | |

提示词 `Creation` / `Strategy` 类与 Genesis 步骤仍无交叉深链（小体验债）。

### 4.2 技能 / MCP

- Skills：管理完整，与 Genesis 无直接耦合 ★★★★☆  
- MCP：可用，偏高级用户 ★★★☆☆

### 4.3 Story System（已拆 Tab）

```
story-system/
  ContractsTab | CommitsTab | ReadingPowerTab | MemoryTab
  AuditTab | AntiAiTab | GenresTab | StyleDnaTab
```

**ContractsTab**（关键）：

- MASTER_SETTING / CHAPTER_1 播种状态卡 ✅  
- 失败 Genesis run + errors 摘要 ✅  
- 跳转仪表盘 / 日志 ✅  
- 手动创建合同兜底 ✅  

**完整度**：★★★★★

---

## 5. L4 — 分析与诊断层

| 模块 | 完整度 | 相对 v0.26.24 |
|------|--------|---------------|
| 叙事分析 | ★★★★☆ | + ReadingPowerChart SVG |
| 拆书 | ★★★☆☆ | 转故事后导航仍可加强 |
| 用量统计 | ★★★★☆ | + bootstrap/smart_execute/other 分组（启发式，R8） |
| 写作统计 | ★★★★☆ | 稳定 |
| 生成链路 | ★★★★★ | + Genesis 互链 + USER_GUIDE |
| 意图图 | ★★★★ | 已文档化 |
| 日志 | ★★★★★ | + session 预填深链 |

---

## 6. L5 — 运维扩展层

| 模块 | 完整度 | 说明 |
|------|--------|------|
| 任务 | ★★★★☆ | CRUD / cron / 心跳 / 级联改写 |
| useSyncStore | ★★★★★ | Genesis 后台写入 → L2 自动刷新 |
| DataLoader / ConnectionStatus / Updater / Login | ★★★★★ | |

---

## 7. 横切：Schema 与溯源基础设施

v0.26.28–31 为溯源补齐了关键底座：

| 项 | 状态 |
|----|------|
| `characters/scenes/world_buildings/kg_entities` 的 `source` / `is_auto_generated` | ✅ 新库建表 + V103 迁移 + init 兜底 |
| 前端类型与徽章 UI | ✅ |
| 旧库缺列热修 | ✅ v0.26.30 |

这是 L2 溯源从「仅伏笔」扩展到「全资产」的前提，已验证闭环。

---

## 8. 文档一致性

| 项 | 状态 |
|----|------|
| USER_GUIDE 3.17–3.19（链路/意图图/日志） | ✅ |
| Genesis errors 仪表盘可见 | ✅ |
| 创作路径说明 | ✅（CreationPathGuide + 指南） |
| 仪表盘「场景」口径 | ⚠️ 文档/UI 称场景，数据仍是 chapter_count |
| 角色「关联场景」 | ⚠️ 仍未实现 |

---

## 9. 与实施计划完成度

对照 [`2026-07-07-backstage-studio-audit-implementation-plan.md`](./plans/2026-07-07-backstage-studio-audit-implementation-plan.md)：

| Phase | 计划目标 | 完成度 |
|-------|----------|--------|
| Phase 1 P0 | Panel / 路径 / Wizard / 统计 | **~95%**（统计口径 R1、Guide quick 回调 R2 残留） |
| Phase 2 P1 | 角色 / 溯源 / 合同 / 续写幕前 | **~98%**（关联场景 R10、文风 Tab R7） |
| Phase 3 P2 | 互链 / Usage / 伏笔 select / 文档 | **~95%**（Usage 启发式 R8） |
| Phase 4 P3 | 策略前移 / KG CRUD / 世界 AI / 图表 / prompts 外部化 | **~90%**（SceneEditor/Pipeline 统一未做） |

---

## 10. 风险 Register（当前）

| 风险 | 严重度 | 缓解建议 |
|------|--------|----------|
| 仪表盘场景数误导 | 中 | 聚合真实 scene_count 或改回「章节」标签 |
| Guide「快速创作」误导 | 中 | 回调改为跳转 Stories + 打开 AI 菜单，或改文案 |
| applyWizard 重复叠加 | 中 | 加幂等策略 / 后端 `apply_story_wizard_assets` |
| 幕后无 genesis-warnings toast | 低–中 | App 或 GenesisPanel 订阅事件 |
| Pipeline/Editor 双轨 | 低 | 产品统一命名或合并入口 |
| Usage 分组不准 | 低 | 后端透传 operation_type |

---

## 11. 修改完善建议（下一迭代）

### P1（建议 v0.26.35）

1. **修正仪表盘场景统计**  
   - 方案 A：Story 列表 API 增加 `scene_count`  
   - 方案 B：临时改标签为「章节」并保留 chapter_count  
   - 推荐 A，与场景优先架构一致  

2. **修正 CreationPathGuide.onQuick**  
   - Dashboard：跳转 `stories` 并触发快速创作引导，或隐藏 quick 回调直至有明确入口  

3. **加固 applyWizardToStory**  
   - 重复角色按 name 去重或「替换模式」确认  
   - 补 KG ingest 或明确 UI 提示「不含知识图谱摄取」  
   - 首场景 sequence 冲突检测  

### P2（建议 v0.26.36）

4. 幕后监听 `genesis-warnings` → toast + `GenesisPanel.loadRuns()`  
5. WorldBuilding 增加 Writing Style Tab（接 `useWritingStyle`）  
6. PipelinePanel UI 标注「场景序号（章节投影）」或改 API 为 `scene_id`  
7. 角色卡片「出场场景」→ 过滤 Scenes 并跳转  

### P3（债务池）

8. SceneEditor 与 PipelinePanel 产品统一  
9. UsageStats 后端 operation 字段  
10. 伏笔 Kanban / Ledger 高级编辑  
11. 拆书转故事后自动 `setCurrentStory` + 导航  

---

## 12. 验证基线（审计时项目声明）

来自 `AGENTS.md` / `PROJECT_STATUS.md`（inspected，非本轮重跑）：

| 检查 | 状态 |
|------|------|
| `cargo test --lib` | 685 passed / 0 failed / 2 ignored |
| `npx vitest run` | 237 passed / 3 skipped |
| `npx tsc --noEmit` | 零错误 |
| `cargo +nightly fmt -- --check` | ✅ |
| `architecture_guard.py` | ✅ |

本审计为 **代码检视**，未重新执行全量测试套件。

---

## 13. 审计方法说明

| 标签 | 含义 |
|------|------|
| **executed** | 直接阅读当前源码（pages/components/hooks/utils/genesis.rs） |
| **inspected** | 对照 PROJECT_STATUS、USER_GUIDE、旧审计、git log |
| **assumed** | 未做全路径 Playwright E2E；未在运行时复现 Genesis |

---

## 14. 参考文件索引

| 类型 | 路径 |
|------|------|
| 本报告 | `docs/AUDIT_BACKSTAGE_STUDIO_v0.26.34.md` |
| 前版审计 | `docs/AUDIT_BACKSTAGE_STUDIO_v0.26.24.md` |
| 实施计划 | `docs/plans/2026-07-07-backstage-studio-audit-implementation-plan.md` |
| Genesis 步骤工具 | `src-frontend/src/utils/genesisSteps.ts` |
| GenesisPanel | `src-frontend/src/components/GenesisPanel.tsx` |
| 路径引导 | `src-frontend/src/components/CreationPathGuide.tsx` |
| Wizard 应用 | `src-frontend/src/utils/applyWizardToStory.ts` |
| 合同状态 | `src-frontend/src/pages/story-system/ContractsTab.tsx` |
| Genesis 后端 | `src-tauri/src/narrative/genesis.rs` |
| 提示词面板 | `src-frontend/src/pages/settings/PromptsPanel.tsx` |

---

## 15. 总结

| 维度 | 结论 |
|------|------|
| 相对 v0.26.24 | **大幅改善**：原 Top5 P0/P1 几乎全部关闭 |
| 当前健康度 | 幕后工作室 **生产可用且可观测**；剩余多为口径/半闭环/体验债 |
| Genesis 关联 | 观测闭环成立；消费面可编辑可溯源；主触发仍在幕前 |
| 建议首要动作 | 修 R1 场景统计口径 + R2 Guide 回调 + R3 Wizard 幂等 |

_审计人：AI Assistant（Cursor Agent）_  
_下次复审建议：P1 三项落地后（目标 v0.26.35）_

---

## 实施关闭记录（v0.26.35）

> **状态**：R1–R11 **全部落地**（2026-07-09）。本文件保留为审计基线；实现细节见 CHANGELOG / AGENTS v0.26.35。

| ID | 状态 |
|----|------|
| R1–R11 | ✅ 已关闭于 v0.26.35 |

