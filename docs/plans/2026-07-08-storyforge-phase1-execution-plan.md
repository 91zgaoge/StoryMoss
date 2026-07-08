# StoryForge 综合优化计划 — 阶段一执行方案

> **关联计划**：`docs/plans/2026-07-07-storyforge-comprehensive-optimization-plan.md`  
> **当前版本**：v0.26.31（已修复 6 项 UI/后端热修复并通过 CI）  
> **目标版本**：v0.26.32  
> **日期**：2026-07-08

---

## 一、已完成工作（v0.26.31）

### 用户反馈问题（6 项）

| # | 问题 | 修复文件 | 状态 |
|---|------|----------|------|
| 1 | 顶部状态栏字数统计滞后 / 始终显示“保存中” | `FrontstageApp.tsx` | ✅ 已修复 |
| 2 | 顶部状态栏 `12px` 字体大小点击无响应 | `FrontstageHeader.tsx`, `FrontstageApp.tsx`, `Settings.tsx`, `GeneralSettings.tsx`, `sync.rs` | ✅ 已修复 |
| 3 | 底部状态栏后台任务图标显示为缺字符号 | `FrontstageBottomBar.tsx`（lucide-react SVG） | ✅ 已修复 |
| 4 | 顶部状态栏整体精细化 | `FrontstageHeader.tsx` | ✅ 已修复 |
| 5 | 小说初始化策略 JSON 解析失败：`missing field rationale` | `strategy.rs`, `selector.rs` | ✅ 已修复 |
| 6 | 获取角色报错：`no such column: source` | `connection.rs` | ✅ 已修复 |

### 验证结果

- `npx vitest run`：213 passed / 3 skipped
- `npx tsc --noEmit`：通过
- `cargo test --lib`：677 passed
- `cargo +nightly fmt -- --check`：通过
- `cargo clippy --lib`：通过
- `python3 scripts/architecture_guard.py`：PASSED
- GitHub Actions run `28929492984`：✅ 全部成功（包含 Windows tauri-build）
- Tag `v0.26.31` 已推送：`https://github.com/91zgaoge/StoryForge/actions/runs/28929492984`

### GitHub Issues

- `gh issue list --state open`：无待处理问题。

---

## 二、与综合优化计划的对照

`docs/plans/2026-07-07-storyforge-comprehensive-optimization-plan.md` 规划了 v0.26.25–v0.26.28 四个阶段。当前实际版本为 v0.26.31，说明原计划版本号未按预期推进。以下按阶段核对完成度：

| 阶段 | 目标版本 | 完成度 | 说明 |
|------|----------|--------|------|
| 阶段一：可观测性与测试基线 | v0.26.25 | ~70% | GenesisPanel 已重构，Stories Wizard 重复创建已修复，Dashboard 统计卡和 L1 创作入口仍有小缺口，memory/ingest 缺测试 |
| 阶段二：L2 资产补齐与领域层止血 | v0.26.26 | 0% | 角色编辑/关系、溯源徽章、Story System 合同卡等均未开始 |
| 阶段三：L4 诊断互链、文档与依赖解耦 | v0.26.27 | ~30% | GenesisPanel 已具备跳转 story/幕前/链路/日志的能力，但 trace_id 后端未持久化；其余未开始 |
| 阶段四：架构债务与工程体验 | v0.26.28 | 0% | prompts 外部化、迁移脚本拆分等均未开始 |

> **结论**：原计划中的大部分内容尚未执行。本阶段将首先补齐阶段一的剩余缺口，使阶段一达到可发布状态。

---

## 三、阶段一剩余任务（v0.26.32）

### 任务 1：L1 创作入口 UX 统一

**现状问题**：
- `CreationPathGuide.tsx` 已存在，但当前为纯展示卡片，不可点击。
- Dashboard 首页主按钮“AI 创建故事”实际打开的是 `NovelCreationWizard`（幕后向导），而 `CreationPathGuide` 中推荐的“幕前 Genesis Pipeline”路径没有一键入口。

**改动目标**：
- 让 `CreationPathGuide` 的三张卡片可点击，分别触发对应创作路径。
- Dashboard 主按钮“AI 创建故事”改为启动推荐的 Genesis/幕前流程（`smart_execute` 或 `show_frontstage`）。
- 保持 Stories 页面的 `CreationPathGuide` 与 Dashboard 行为一致。

**改动文件**：
- `src-frontend/src/components/CreationPathGuide.tsx`
- `src-frontend/src/pages/Dashboard.tsx`
- `src-frontend/src/pages/Stories.tsx`

**验收标准**：
- Dashboard “AI 创建故事”按钮点击后进入幕前 Genesis 创作流程。
- `CreationPathGuide` 中“幕前 Genesis Pipeline”卡片可点击并进入同一流程。
- “幕后 AI 向导”卡片点击后打开 Wizard。
- “幕后快速创作”卡片点击后打开快速创作弹窗/流程。
- 新增/更新 `CreationPathGuide` 的单元测试。

---

### 任务 2：仪表盘统计卡修正

**现状问题**：
- 当前三个统计卡为：故事、角色、章节。
- “章节”标签与跳转目标 `scenes` 视图不一致（应为“场景”）。
- 缺少“字数”统计卡，而故事列表中已展示 `word_count`。
- 统计值来自 Zustand store 的 `stories` 数组，可能与 `useStories` 查询数据不同步。

**改动目标**：
- 将“章节”改为“场景”，目标仍为 `scenes` 视图；或新增独立的“章节”视图。
- 新增“字数”统计卡，点击可跳转幕前/统计页。
- 统计值优先使用 `useStories` 查询结果，避免 store 滞后。

**改动文件**：
- `src-frontend/src/pages/Dashboard.tsx`

**验收标准**：
- 统计卡标签与跳转视图一致。
- 字数统计卡显示所有故事的总字数。
- 点击卡片正确跳转。
- 新增 Dashboard 统计卡单元测试。

---

### 任务 3：为 `memory/ingest.rs` 补齐首批特征测试

**现状问题**：
- `memory/ingest.rs` 目前 **0 测试**。
- 该模块负责伏笔、角色状态、叙事事件、KG 实体/关系的持久化，是审计 P0-1/P0-2 的核心路径。

**改动目标**：
- 为非 LLM 路径编写可稳定运行的 happy path 和 error path 测试。
- 优先覆盖：
  - `get_recent_jobs`：空结果、pending/completed/failed 混合状态。
  - `extract_json`：正常 markdown 代码块、不完整 JSON、无法解析输入。
  - `build_event_chain`：事件排序与因果链构建。
  - `ingest_with_cancel`：取消令牌已触发时立即返回错误。

**改动文件**：
- `src-tauri/src/memory/ingest.rs`（新增 `#[cfg(test)] mod tests`）
- 必要时提取 `extract_json` / `build_event_chain` 为纯函数以便测试。

**验收标准**：
- `cargo test --lib memory::ingest` 至少通过 2 条测试（1 happy + 1 error）。
- 测试不依赖外部 LLM 调用（使用 mock 或纯输入/输出函数）。
- `cargo test --lib` 全量通过。

---

### 任务 4：文档与版本同步

**改动文件**：
- `CHANGELOG.md`：登记 v0.26.32 阶段一剩余项。
- `PROJECT_STATUS.md`：更新阶段一完成状态。
- `README.md`：如有必要，更新最近版本说明。
- `src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src-frontend/package.json`：版本 bump 到 v0.26.32。
- `Cargo.lock`：同步更新。

---

## 四、验证命令

```bash
# 前端
cd src-frontend && npx vitest run
cd src-frontend && npx tsc --noEmit

# 后端
cd /Users/yuzaimu/projects/StoryForge && cargo test --lib
cd /Users/yuzaimu/projects/StoryForge && cargo +nightly fmt -- --check
cd /Users/yuzaimu/projects/StoryForge && cargo clippy --lib

# 架构守卫
cd /Users/yuzaimu/projects/StoryForge && python3 scripts/architecture_guard.py

# 格式化
cd src-frontend && npm run format:check
```

---

## 五、执行顺序

1. 任务 1：L1 创作入口 UX 统一
2. 任务 2：仪表盘统计卡修正
3. 任务 3：`memory/ingest.rs` 测试补齐
4. 任务 4：文档与版本同步
5. 本地验证
6. Commit / tag / push
7. 监控 CI

---

## 六、风险与回滚

| 风险 | 概率 | 缓解措施 |
|------|------|----------|
| Dashboard 主按钮改动影响用户习惯 | 中 | 保留“手动创建”按钮；AI 按钮明确标注“AI 幕前创作” |
| `memory/ingest` 测试需要 mock LLM | 中 | 只测非 LLM 纯函数路径；避免引入复杂 mock |
| 统计卡数据源切换导致 UI 闪烁 | 低 | 保持 Zustand store 作为 fallback |
