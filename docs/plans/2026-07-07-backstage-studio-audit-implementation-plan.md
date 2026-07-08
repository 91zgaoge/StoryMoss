# 幕后工作室审计 — 修改完善优化实施方案

> **关联审计**：[`docs/AUDIT_BACKSTAGE_STUDIO_v0.26.24.md`](../AUDIT_BACKSTAGE_STUDIO_v0.26.24.md)  
> **基准版本**：v0.26.24 → 目标 v0.26.25–v0.26.27（分三阶段交付）  
> **制定日期**：2026-07-07  
> **原则**：稳定性 > 范围；每阶段可独立发布；测试伴随代码变更

---

## 1. 目标与成功标准

### 1.1 总体目标

1. **Genesis 可观测性闭环**：仪表盘能准确反映 Quick + Background 步骤与非致命 errors。  
2. **创作路径可理解**：用户能区分「幕前 Genesis / 幕后 Wizard / 快速创作」并选对入口。  
3. **L2 资产页可维护 Genesis 产出**：角色可编辑、关系可管理、主要资产可溯源。  
4. **契约落地可见**：Story System 显示 Genesis 合同播种状态。  
5. **文档与实现一致**：USER_GUIDE 补 L4 诊断页、修正过度承诺。

### 1.2 阶段级成功标准

| 阶段 | 版本目标 | 退出标准 |
|------|----------|----------|
| Phase 1 | v0.26.25 | GenesisPanel 动态步骤 + errors；L1 路径引导；Stories Wizard 不重复建故事；vitest 新增 Panel 契约测试 |
| Phase 2 | v0.26.26 | 角色 edit + 关系 CRUD + 溯源徽章；Story System 合同卡；Scenes 续写跳转幕前 |
| Phase 3 | v0.26.27 | USER_GUIDE 更新；UsageStats 分组；Tracing↔Panel 互链；Foreshadowing 场景下拉 |

---

## 2. 阶段划分总览

```
Phase 1 (P0) ── Genesis 观测 + L1 路径 ── 约 3–5 人日
Phase 2 (P1) ── L2 资产 + L3 契约 ─────── 约 5–8 人日
Phase 3 (P2) ── L4 诊断 + 文档 polish ─── 约 3–5 人日
Phase 4 (P3) ── 架构债务（可选）────────── 单独 milestone
```

---

## 3. Phase 1 — Genesis 可观测性与 L1 路径统一（P0）

**目标版本**：v0.26.25  
**风险**：低–中（以前端为主，GenesisPanel 需对齐后端 JSON 契约）

### 3.1 任务 1.1：重构 GenesisPanel 步骤模型

**问题**：硬编码 8 步与后端 Quick(2)+Background(6) 不一致；`steps_json.errors` 未展示。

**改动文件**：
- `src-frontend/src/components/GenesisPanel.tsx`
- 新增 `src-frontend/src/utils/genesisSteps.ts`（纯函数：解析 steps_json、合并 progress 事件）
- 新增 `src-frontend/src/utils/__tests__/genesisSteps.test.ts`

**实现要点**：

1. 从 `run.steps_json` 解析步骤列表（优先 JSON；fallback 用 `current_step_number` + 后端步骤名常量）。
2. 步骤名常量与 `genesis.rs` 对齐：
   - Quick: `构思故事`, `撰写开篇`
   - Background: `选择创作策略`, `构建世界与骨架`, `场景规划`, `埋设伏笔`, `知识图谱`, `播种故事合同`
3. 渲染 `errors[]`（warning/error 分级图标 + 展开详情）。
4. 若 run 含 `story_id`，显示「打开故事」「打开幕前」按钮（`setCurrentStory` + `show_frontstage`）。
5. 在 `App.tsx` 或 Panel 内监听 `genesis-warnings`，toast + `loadRuns()`。

**契约测试**：
- fixture：`tests/fixtures/genesis_run_steps_sample.json`（含 errors 数组）
- 断言：8 步显示名、errors 计数、progress 百分比边界

**验收**：
- [ ] 真实 Genesis run 步骤名与后端日志一致
- [ ] 含 warning 的 run 在 Panel 可见非致命错误列表
- [ ] 点击「打开故事」跳转故事库并高亮

---

### 3.2 任务 1.2：L1 创作路径决策引导

**问题**：三路径无说明，用户误判。

**改动文件**：
- `src-frontend/src/pages/Dashboard.tsx`
- `src-frontend/src/pages/Stories.tsx`
- 新增 `src-frontend/src/components/CreationPathGuide.tsx`（可复用卡片）

**实现要点**：

在 Dashboard Hero 区下方、Stories AI 菜单旁增加折叠说明：

| 入口 | 适用场景 | 是否 Genesis Pipeline |
|------|----------|----------------------|
| 幕前「新写小说」 | 从零写第一章，后台自动补资产 | ✅ 是 |
| AI 创建故事（Wizard） | 先定世界观/角色再写作 | ❌ 预置资产 |
| 快速创作 | 已有故事壳，跑 CreationWorkflow | ❌ 另一引擎 |

**验收**：
- [ ] 新用户能在 30s 内理解推荐路径（幕前 Genesis）

---

### 3.3 任务 1.3：修复 Stories 页 Wizard 重复建故事

**问题**：对已有 `wizardStory` 调用 `createStoryWithWizard` 会新建重复故事。

**改动文件**：
- `src-frontend/src/pages/Stories.tsx`
- 后端（若需）：`src-tauri/src/creation_commands.rs` — 新增 `apply_wizard_to_story(story_id, …)` 或复用 update 路径

**方案 A（推荐，最小后端）**：
- 已有故事走 Wizard 时，完成后调用 **update** 系列 API（world/characters/scenes）而非 `createStoryWithWizard`。
- Wizard 完成 handler 分支：`if (wizardStory) applyToExisting else createNew`.

**方案 B（后端新命令）**：
- `apply_story_wizard_assets { story_id, world, characters, … }` 原子写入。

**验收**：
- [ ] 故事库选已有故事 → 向导创作 → 故事 ID 不变、资产更新
- [ ] Dashboard 新建 → 仍创建新 story

---

### 3.4 任务 1.4：仪表盘统计修正

**改动文件**：
- `src-frontend/src/pages/Dashboard.tsx`
- `src-frontend/src/hooks/useStories.ts` 或 aggregate helper

**实现要点**：
1. 第三张统计卡：改标签为「章节」或聚合真实 `scene_count`（若 Story 类型扩展字段）。
2. 统计卡 `onClick` → `setCurrentView('stories'|'characters'|'scenes')`。

**验收**：
- [ ] 标签与数据口径一致
- [ ] 点击跳转正确

---

### 3.5 Phase 1 测试清单

```bash
cd src-frontend && npx vitest run src/utils/__tests__/genesisSteps.test.ts
cd src-frontend && npx tsc --noEmit
# 可选 E2E：仪表盘 Genesis Panel 渲染
```

---

## 4. Phase 2 — L2 资产补齐与 L3 契约可见（P1）

**目标版本**：v0.26.26

### 4.1 任务 2.1：角色页编辑 + 关系 CRUD

**改动文件**：
- `src-frontend/src/pages/Characters.tsx`
- 新增 `src-frontend/src/components/CharacterEditModal.tsx`
- 新增 `src-frontend/src/components/CharacterRelationshipForm.tsx`
- `src-frontend/src/hooks/useCharacterRelationships.ts`（补 create/delete mutations，若缺）

**实现要点**：
1. 卡片 hover「编辑」→ `useUpdateCharacter`
2. 关系 Tab「添加关系」→ `createCharacterRelationship`（选角色 B + 类型 + 描述）
3. 删除关系（若后端有 delete API；无则 Phase 2 仅 create）

**验收**：
- [ ] 编辑 Genesis 生成的角色并保存
- [ ] 手动添加关系后在关系 Tab 可见

---

### 4.2 任务 2.2：L2 创世溯源徽章

**问题**：除伏笔外无法识别 Genesis 产出。

**改动文件**（前端）：
- `src-frontend/src/types/index.ts` — `Character` 增加可选 `source?: string` / `is_auto_generated?: boolean`（与后端对齐）
- `Characters.tsx`, `Scenes.tsx`（StoryTimeline 卡片）, `WorldBuilding.tsx`, `KnowledgeGraph/KnowledgeGraphView.tsx`

**改动文件**（后端，若字段未暴露）：
- `src-tauri/src/db/` 相关 query — SELECT 增加 source 列
- migration 若已有列则仅补 IPC 序列化

**UI**：与 Foreshadowing 一致的小徽章「创世」。

**验收**：
- [ ] Genesis 完成后 L2 主要资产显示徽章
- [ ] 手动创建不显示

---

### 4.3 任务 2.3：Story System 合同播种状态卡

**改动文件**：
- `src-frontend/src/pages/StorySystem.tsx`
- 可选 helper：`src-frontend/src/hooks/useGenesisContractStatus.ts`

**实现要点**：

Contracts Tab 顶部卡片：
- MASTER_SETTING 是否存在
- CHAPTER_1 contract 是否存在
- 来源：查 `getContractTree` + metadata 或 `genesis_runs` 最近 run 的 step 状态

若合同缺失且存在 failed Genesis run → 显示「重新播种」指引（链到 GenesisPanel / 日志）。

**验收**：
- [ ] Genesis 成功后两张合同均显示「已建立」
- [ ] 播种失败 run 显示警告 + errors 摘要

---

### 4.4 任务 2.4：Scenes ExecutionPanel 续写跳转幕前

**改动文件**：
- `src-frontend/src/components/ExecutionPanel.tsx`

**实现要点**：
- `continue_writing` / `continue_next_chapter` → `loggedInvoke('show_frontstage')` + toast
- 可选：IPC 传递 `story_id` + `scene_id` 供幕前预选

**验收**：
- [ ] 点击主行动打开幕前窗口

---

### 4.5 任务 2.5：世界构建 — Writing Style 子 Tab（可选本 Phase）

**改动文件**：
- `src-frontend/src/pages/WorldBuilding.tsx` — 增加 Tab「文风设定」
- 使用已有 `useWritingStyle` / `useUpdateWritingStyle`

**验收**：
- [ ] Wizard/Genesis 写入的文风可在幕后编辑

---

### 4.6 Phase 2 测试清单

```bash
cd src-frontend && npx vitest run
cd src-tauri && cargo test --lib
npx playwright test e2e/ --grep "character|scene"  # 若有相关 spec
```

---

## 5. Phase 3 — L4 诊断互链与文档（P2）

**目标版本**：v0.26.27

### 5.1 任务 3.1：TracingPanel ↔ GenesisPanel 互链

**改动**：
- `GenesisPanel.tsx`：每 run 增加「查看生成链路」→ `setCurrentView('tracing')` + 预选 trace_id（若 run 存 trace_id；否则按 session_id 过滤）
- `TracingPanel.tsx`：详情页增加「对应 Genesis 运行」链接

---

### 5.2 任务 3.2：Logs 深链

**改动**：
- `GenesisPanel` 失败 run → 「查看日志」→ `setCurrentView('logs')` + search 预填 `session_id`

---

### 5.3 任务 3.3：UsageStats 按 operation 分组

**改动**：
- `UsageStats.tsx` — 增加 Tab：全部 / bootstrap / smart_execute / 其他
- 后端若 `LlmCall` 已有 operation 字段则前端分组；否则 Phase 3 仅文档降级承诺

---

### 5.4 任务 3.4：Foreshadowing UX

**改动**：
- `setup_scene_id` 改为 `<select>` 绑定 `useScenes`
- 暴露 `useUpdatePayoffLedgerFields` 的 target_start/end_scene 编辑（折叠高级区）

---

### 5.5 任务 3.5：USER_GUIDE 更新

**改动文件**：`docs/USER_GUIDE.md`

**新增章节**：
- 3.17 生成链路（TracingPanel）
- 3.18 意图图诊断
- 3.19 日志查看

**修正章节**：
- 3.1 统计卡片可点击（Phase 1 后）
- 3.3 / 3.5 AI 生成说明（改为「向导/Genesis 预生成，幕后可编辑」）
- 3.11 伏笔看板（列表式，非 Kanban）
- 3.12 叙事分析（条形强度，非折线图）
- 3.14 用量统计（实际能力）

**同步**（推送时）：`CHANGELOG.md`, `AGENTS.md`, `PROJECT_STATUS.md`, `ROADMAP.md` — 登记本审计与 Phase 完成项。

---

## 6. Phase 4 — 架构债务（P3，单独 milestone）

> 不阻塞 Phase 1–3；需单独评审 blast radius。

| 项 | 说明 | 参考 |
|----|------|------|
| 策略选择移入 Quick Phase | 延迟 +10–20s，需数据支撑 | ROADMAP 债务、`genesis-audit-and-optimization-design.md` |
| SceneEditor 五阶段 vs PipelinePanel 统一 | 产品/UX  redesign | 本审计 S-2 |
| PipelinePanel chapterNumber 语义 | 文档化或改 API 为 scene_id | 场景优先架构 |
| WorldBuilding AI 生成 | 接 `generateWorldBuildingOptions` | Wizard 已有 |
| Characters AI 扩展 | 接 skill 或 wizard API | — |
| KG 手动 CRUD UI | 接 `createEntity`/`createRelation` | API 已有 |
| 叙事分析图表 | 引入 chart 库或降级文档 | — |

---

## 7. 文件.touch 矩阵（按 Phase）

| 文件 | Ph1 | Ph2 | Ph3 |
|------|-----|-----|-----|
| `components/GenesisPanel.tsx` | ✅ | | ✅ |
| `utils/genesisSteps.ts` | ✅ | | |
| `pages/Dashboard.tsx` | ✅ | | |
| `pages/Stories.tsx` | ✅ | | |
| `components/CreationPathGuide.tsx` | ✅ | | |
| `pages/Characters.tsx` | | ✅ | |
| `pages/StorySystem.tsx` | | ✅ | |
| `components/ExecutionPanel.tsx` | | ✅ | |
| `pages/WorldBuilding.tsx` | | ✅ | |
| `pages/UsageStats.tsx` | | | ✅ |
| `pages/Foreshadowing.tsx` | | | ✅ |
| `pages/TracingPanel.tsx` | | | ✅ |
| `docs/USER_GUIDE.md` | | | ✅ |
| `types/index.ts` | | ✅ | |
| `creation_commands.rs`（可选） | ✅ | | |

---

## 8. 风险与回滚

| 风险 | 概率 | 回滚策略 |
|------|------|----------|
| GenesisPanel 解析旧 runs JSON 失败 | 中 | fallback 旧 8 步 UI + 日志 warn |
| apply_wizard_to_story 后端改动回归 | 中 | feature flag；保留 create 路径 |
| Character source 字段后端无列 | 低 | 仅 foreshadowing 保持溯源，其余 Phase 4 |
| 文档大范围修改引发 CI | 低 | docs-only commit 独立 |

---

## 9. 验证与发布流程

每 Phase 合并前：

```bash
cd src-tauri && cargo test --lib
cd src-tauri && cargo +nightly fmt -- --check
cd src-frontend && npx tsc --noEmit
cd src-frontend && npx vitest run
python3 scripts/architecture_guard.py
```

发布后（用户规则）：
1. bump 版本四源 + tag  
2. 推送并 **监控 GitHub Actions 至全绿**  
3. 本地 `cargo tauri build`  
4. 更新 docs of record  

---

## 10. 里程碑时间表（建议）

| 里程碑 | 内容 | 目标日期 |
|--------|------|----------|
| M1 | Phase 1 完成 → v0.26.25 | +1 周 |
| M2 | Phase 2 完成 → v0.26.26 | +2 周 |
| M3 | Phase 3 完成 → v0.26.27 | +3 周 |
| M4 | Phase 4 评审立项 | 视 Genesis 运行数据 |

---

## 11. 开放问题（human-gated）

1. **Wizard 已有故事**：方案 A（纯前端 update 系列）vs 方案 B（新 Rust 命令）— 建议 Phase 1 实施前 30min 读 `creation_commands.rs` 定案。  
2. **Character/Scene source 字段**：需确认 DB schema 是否已有 `source` / `is_auto_generated`；若无，是否 Phase 2 加 migration。  
3. **Genesis run ↔ trace_id 关联**：`genesis_runs` 是否已存 trace_id；若无，Tracing 互链需后端小改。

---

_本计划随 Phase 完成在 `PROJECT_STATUS.md` / `ROADMAP.md` 更新状态。_
