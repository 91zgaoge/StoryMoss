# StoryMoss 综合优化实施方案

> **关联审计**：[`docs/AUDIT_BACKSTAGE_STUDIO_v0.26.24.md`](../AUDIT_BACKSTAGE_STUDIO_v0.26.24.md)  
> **健康仪表盘**：基于 Brooks-Lint Health Dashboard（v0.26.24，Composite Score 66/100）  
> **基准版本**：v0.26.24 → 目标 v0.26.25–v0.26.28（分四阶段交付）  
> **制定日期**：2026-07-07  
> **原则**：稳定性 > 范围；每阶段可独立发布；测试伴随代码变更

---

## 总体原则

1. **稳定性 > 范围**：每阶段可独立发布，合并前必须通过类型检查 + 单元测试 + 架构守卫。
2. **先止血再重构**：先补测试、解循环依赖，再大规模拆分 UI/领域模型。
3. **功能与架构同频**：Backstage 审计的 Genesis 缺口与 Health Dashboard 的架构缺口高度重叠，合并处理。
4. **文档随行**：每阶段同步更新 `USER_GUIDE.md`、`ARCHITECTURE.md`、`AGENTS.md`、`CHANGELOG.md`。

---

## 阶段一：可观测性与测试基线（v0.26.25，约 1 周）

**目标**：让用户能准确看到 Genesis 进度，同时给最高风险模块补上第一批测试。

| 任务 | 改动文件 | 验收标准 |
|------|----------|----------|
| 重构 `GenesisPanel` 步骤模型 | `src-frontend/src/components/GenesisPanel.tsx`、新增 `src-frontend/src/utils/genesisSteps.ts` + 测试 | 步骤与后端 Quick(2)+Background(6) 对齐；展示 `steps_json.errors`；可跳转到 story/幕前 |
| 统一 L1 创作入口 UX | `Dashboard.tsx`、`Stories.tsx`、新增 `CreationPathGuide.tsx` | 新用户 30 秒内能区分三条创作路径 |
| 修复 Stories Wizard 重复建故事 | `Stories.tsx`、必要时 `creation_commands.rs` | 已有故事走 update，ID 不变 |
| 仪表盘统计卡修正 | `Dashboard.tsx` | 标签与口径一致；点击跳转 |
| 为高频后端模块写首批特征测试 | `model_gateway/executor.rs`、`db/repositories.rs`、`memory/ingest.rs` | 每个模块至少 1 条 happy path + 1 条错误路径 |

### 阶段一验证命令

```bash
cd src-frontend && npx vitest run src/utils/__tests__/genesisSteps.test.ts
cd src-frontend && npx tsc --noEmit
cd src-tauri && cargo test --lib model_gateway db memory
python3 scripts/architecture_guard.py
```

---

## 阶段二：L2 资产补齐与领域层止血（v0.26.26，约 1.5–2 周）

**目标**：补全角色/关系/溯源等最大功能空洞，并开始把贫血领域模型和巨型组件拆小。

| 任务 | 改动文件 | 验收标准 |
|------|----------|----------|
| 角色页编辑 + 关系 CRUD | `Characters.tsx`、新增 `CharacterEditModal.tsx`、`CharacterRelationshipForm.tsx` | 可编辑 Genesis 产出角色；可手动增删关系 |
| L2 创世溯源徽章 | `types/index.ts`、`Characters.tsx`、`Scenes.tsx`、`WorldBuilding.tsx`、`KnowledgeGraphView.tsx` | Genesis 产出资产显示「创世」徽章；手动创建不显示 |
| Story System 合同播种状态卡 | `StorySystem.tsx` | 显示 MASTER_SETTING + CHAPTER_1 合同状态；失败 run 有警告摘要 |
| Scenes 续写跳转幕前 | `ExecutionPanel.tsx` | 点击主行动打开幕前 |
| 拆分 `StorySystem.tsx` | 新增 `tabs/ContractsTab.tsx` 等 | 原文件 < 200 行，只做 tab 路由 |
| 注入 repository traits 到 `creative_engine` | `creative_engine/context_builder.rs`、`db/traits.rs` | 领域代码不再直接 `use crate::db::repositories::*` |
| 拆分 `db/repositories.rs` | 新建 `db/repositories/*.rs` | 每个 repo 独立文件，原文件仅做 re-export |

### 阶段二验证命令

```bash
cd src-frontend && npx vitest run
cd src-tauri && cargo test --lib
npx playwright test e2e/ --grep "character|scene|genesis"
python3 scripts/architecture_guard.py
```

---

## 阶段三：L4 诊断互链、文档与依赖解耦（v0.26.27，约 1 周）

**目标**：补齐诊断页互链和文档，同时清理前端循环依赖。

| 任务 | 改动文件 | 验收标准 |
|------|----------|----------|
| TracingPanel ↔ GenesisPanel 互链 | `GenesisPanel.tsx`、`TracingPanel.tsx` | run 可跳链路，链路可跳 run |
| Logs 深链 | `GenesisPanel.tsx` | 失败 run 一键跳转日志并预填 `session_id` |
| UsageStats 按 operation 分组 | `UsageStats.tsx` | 全部 / bootstrap / smart_execute / 其他 |
| Foreshadowing UX 改进 | `Foreshadowing.tsx` | `setup_scene_id` 下拉；Ledger 字段可编辑 |
| 解耦前端 `components ↔ stores ↔ hooks ↔ frontstage` | 新增 `types/editor.ts`、`hooks/contracts/*`、`stores/contracts/*` | `appStore.ts` 不再从 `components/EditorSettings.tsx` import；循环依赖数降为 0 |
| 解耦 Tauri `creative_engine ↔ llm` 与 `model_gateway ↔ router` | 在 `ports/` 或 `domain/` 提取共享 trait | 两对模块不再互相 import |
| 更新 `USER_GUIDE.md` | `docs/USER_GUIDE.md` | 补 L4 诊断页、修正过度承诺、与实现一致 |
| 同步元文档 | `CHANGELOG.md`、`AGENTS.md`、`PROJECT_STATUS.md`、`ROADMAP.md` | 登记审计与阶段完成项 |

### 阶段三验证命令

```bash
cd src-frontend && npx vitest run && npx tsc --noEmit
cd src-tauri && cargo test --lib && cargo +nightly fmt -- --check
npx playwright test e2e/
python3 scripts/architecture_guard.py
```

---

## 阶段四：架构债务与工程体验（v0.26.28 或单独 milestone）

**目标**：处理不阻塞前三个阶段、但会长期拖累速度的根因。

| 任务 | 改动文件 | 收益 |
|------|----------|------|
| 外部化 prompts | `src-tauri/src/prompts/registry.rs` → `resources/prompts/**/*.md` | 2,900 行函数消失；prompt 编辑无需重新编译 |
| 迁移脚本拆分 | `src-tauri/src/db/connection.rs` → `migrations/*.rs` | schema 演进可 review、可回滚 |
| 世界构建 AI 生成 | `WorldBuilding.tsx` | 接 Wizard/Genesis 预生成能力 |
| KG 手动 CRUD UI | `KnowledgeGraph.tsx` | API 已有，补 UI |
| Characters AI 扩展 | `Characters.tsx` | 接 skill/wizard API |
| 叙事分析图表 | `NarrativeAnalysis.tsx` | 引入图表库或降级文档 |
| 策略选择移入 Quick Phase | `genesis.rs` | 延迟 +10–20s，需数据支撑 |

---

## 风险与回滚

| 风险 | 概率 | 回滚策略 |
|------|------|----------|
| GenesisPanel 解析旧 runs JSON 失败 | 中 | fallback 旧 8 步 UI + 日志 warn |
| repository trait 注入引入编译错误 | 中 | 分小步重构，每次 `cargo check` |
| 前端循环依赖拆解影响热更新 | 低 | 用 `madge` 或 `scripts/architecture_guard.py` 验证 |
| 高频后端模块测试不稳定 | 中 | 先用 `serial_test`，后续引入共享 pool + 事务回滚 |
| 文档大范围修改引发 CI | 低 | docs-only commit 独立 |

---

## 需要人工拍板的开放问题

1. **Wizard 已有故事**：方案 A（纯前端 update 系列）vs 方案 B（后端新增原子命令）？
2. **Character/Scene source 字段**：DB schema 是否已有？没有是否加 migration？
3. **Genesis run ↔ trace_id 关联**：后端是否已存 trace_id？
4. **Prompt 外部化格式**：Markdown frontmatter、TOML、还是 JSON？
5. **Repository trait 注入优先级**：是否允许阶段二中部分 command 仍用具体 repo，还是要求全部改完？

---

## 建议的下一步

立即开始阶段一，因为：
- 它同时覆盖审计报告中的 P0 项和 Health Dashboard 中的最高风险测试缺口；
- 改动以前端 + 小范围后端测试为主，blast radius 可控；
- 阶段一完成后，v0.26.25 的用户体验和稳定性都会有明显可感知的提升。
