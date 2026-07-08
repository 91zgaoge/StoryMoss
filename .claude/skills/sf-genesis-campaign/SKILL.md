---
name: sf-genesis-campaign
description: 可执行、决策门控的 campaign——把策略选择（StrategySelectionStep）移入 Genesis quick_phase，让首章拿到方法论/体裁约束。何时加载：要推进“首章质量退化/策略未注入”这个活问题、要量化 quick phase 30-60s 承诺是否会被破坏、或被问“怎么把策略移到 quick phase”时。
---

# Campaign：策略选择移入 quick_phase

> 这是 StoryForge 当前最难的活问题（用户确认）。ROADMAP 已记为暂缓债务。本技能把它做成可执行、决策门控、可验证的战役。

## 背景与现状（必读）

- `GenesisPipeline::quick_phase_steps()` = 概念 → 撰写开篇；`StrategySelectionStep` 在 `background_steps()` 才执行。
- 后果：首章 `build_strategy_notes` 拿到 `selected_strategy = None`，仅以题材级 fallback 注入，缺方法论/体裁画像约束 → 首章质量退化。
- **暂缓原因（R10）**：多处文档（`ROADMAP`/`CHANGELOG`/`AGENTS_HISTORY`）明文承诺 quick phase 30-60s 返回首章；移入策略选择（~10-20s LLM）让延迟进入 60-90s，跨越用户可感知阈值且属低 reversibility 用户契约变更。

## 成功标准（可证伪）

- **S1（质量）**：首章与策略约束的契合度评分，移入后比 fallback 显著更高（预设阈值：契合度提升 ≥15%，或人工盲测胜率 ≥70%）。
- **S2（延迟，不可破坏）**：quick phase p95 ≤ 60s（用户契约）。若 S1 达成但 S2 破坏 → 回退或改异步投递（见解法菜单）。
- **S3（不回归）**：`cargo test --lib` 全绿；`narrative::genesis` 契约测试全绿；E2E `genesis-duplicate.spec.ts` 幽灵段落隐藏断言通过。

## 阶段 0 — 先量化再决定（gate：数据驱动）

**目的**：用真实运行数据回答“退化幅度是否显著”，而不是凭感觉迁移。

```bash
# 0.1 确保在干净 master 且 v0.26.19+ 的 genesis_runs 已接入
git checkout master && git pull
cd src-tauri && cargo test --lib narrative::genesis   # 基线全绿

# 0.2 准备对照样本：N≥3 个不同题材创意，每个跑两次创世
#     A 组：现状（strategy=None，题材 fallback）
#     B 组：手动注入 selected_strategy（用 background 阶段产出的策略反灌，模拟移入后的首章）
#     记录：首章文本 + selected_strategy + 耗时
```

**gate 0**：
- 预期：A/B 两组 quick_phase 耗时都 < 60s（B 组因是模拟，未加 LLM 调用）。
- **若 A 组已 ≥60s** → 先解决 quick phase 自身延迟（分支到“先治延迟”），不要叠加策略选择。
- 契合度评分用 LLM 评分器（盲测，A/B 打乱）或人工打分；记录到 `docs/plans/` 下一份对照报告。
- **决策**：若 B 组契合度提升 < 15% 且盲测胜率 < 70% → **关闭战役，保留 fallback，更新 ROADMAP 标注“已量化，不值得迁移”**。若达标 → 进入阶段 1。

## 阶段 1 — 提取纯函数 + 契约测试（gate：不改行为先锁行为）

**目的**：在动 quick_phase 顺序前，把 `quick_phase_steps()`/`background_steps()` 顺序与 `build_strategy_notes` 提取成纯函数并加契约测试，确保重排不破坏现状。

```bash
# 1.1 看 quick/background 顺序与 StrategySelectionStep 位置
rg -n 'quick_phase_steps|background_steps|StrategySelectionStep|build_strategy_notes' src-tauri/src/narrative/genesis.rs

# 1.2 提取 background_steps 为可观测纯函数（参考 select_first_chapter_content 模式）
#     写契约测试：6 步固定顺序（已有 background_steps 测试，扩展加入 strategy 位置断言）
cd src-tauri && cargo test --lib narrative::genesis
```

**gate 1**：`cargo test --lib narrative::genesis` 全绿；新测试锁定“现状顺序”。**若现有测试红** → 先修测试基线，不要在红基线上叠改。

## 阶段 2 — 最小迁移 + 延迟预算守卫（gate：S2 不破坏）

**目的**：把 `StrategySelectionStep` 移入 quick_phase，加预算守卫。

- 解法选择（按优先级）：
  1. **同步移入 + 预算守卫**（最简单）：策略选择作为 quick_phase 第 2 步，复用 `generate_with_fastest` 5s 探测 + 候选链；quick_phase 总预算守卫用 `total_start` 计算已耗时间（参考 v0.23.15 Call1/2 守卫模式）。
  2. **异步投递**（保 S2）：策略选择与首章生成 `tokio::join!` 并发，首章生成时策略并行跑，首章返回后若策略未到则用 fallback，策略到达后以 `genesis-warnings` 提示“策略已补齐，可重生成”。**注意**：`ParallelWorldOutlineCharacterStep` 的 `tokio::join!` 3 路已改串行 + `BACKGROUND_LLM_SEMAPHORE`，并发要尊重信号量。
  3. **策略缓存**（跨创世复用）：同题材/同创意的策略选择结果缓存，命中时 0s 注入。

```bash
# 2.1 实现 chosen 解法
# 2.2 本地跑真实创世 N≥3，记录 quick_phase 耗时
cd src-tauri && cargo tauri dev   # 手动创世，看日志 quick_phase 起止标记
```

**gate 2**：
- 预期：quick_phase p95 ≤ 60s（S2）。
- **若 > 60s** → 分支：若解法 1 → 改解法 2（异步）；若解法 2 仍超 → 改解法 3（缓存）；若全超 → **回退，战役标记“S2 不可破坏，保留 fallback”，更新 ROADMAP**。
- 预期：首章契合度 ≥ 阶段 0 的 B 组（S1）。

## 阶段 3 — 验证与晋升（gate：走 change-control）

```bash
cd src-tauri && cargo test --lib && cargo check && cargo +nightly fmt -- --check
cd src-frontend && npx tsc --noEmit && npm run format:check && npm run test:run
npx playwright test genesis-duplicate.spec.ts
python3 scripts/architecture_guard.py
```

**gate 3**：全绿 → 走 `sf-change-control` 发布流程：bump 版本（四源统一）→ 更新 docs of record（`ROADMAP` 移除暂缓债务条目，`CHANGELOG`/`AGENTS`/`ARCHITECTURE`/`USER_GUIDE`）→ 提交 → tag → 推送 → **`gh run list --limit 3` 监控 CI 到全绿**（强制）→ 本地 `cargo tauri build`。

## 已知错误路径（围栏）

- ❌ 直接把 `StrategySelectionStep` 拖到 quick_phase 头部不加预算守卫 → 必破坏 30-60s 承诺。
- ❌ 用 `tokio::join!` 并发策略 + 首章但忽略 `BACKGROUND_LLM_SEMAPHORE` → 并发过载回潮（v0.23.66 教训）。
- ❌ 把策略选择同步化阻塞后台 LLM → 触发“卡死”家族（见 `sf-debugging-playbook`），必须 `is_silent_background` 登记。
- ❌ 凭感觉迁移不量化 → 违反 R5；必须先过 gate 0。
- ❌ 修改 quick_phase 顺序不先锁现状契约 → 重排引入回归不可见。

## 何时 NOT 用本技能

- 已修复的 Genesis 第一章重复 saga → `sf-failure-archaeology` 战役 1。
- 通用 Genesis 架构 → `sf-architecture-contract` §3 分时介入。
- 创世调试 → `sf-debugging-playbook`。

## 出处与维护

- 重验证命令：
  - `rg -n 'quick_phase_steps|background_steps|StrategySelectionStep|build_strategy_notes|selected_strategy' src-tauri/src/narrative/genesis.rs`
  - `rg -n '策略选择移入 quick_phase|暂缓' ROADMAP.md`
  - `cat docs/plans/2026-07-06-genesis-audit-and-optimization-design.md 2>/dev/null | head -40`
- 易漂移项：quick_phase 步骤顺序、`genesis_runs` 表结构、文档承诺的延迟阈值。
- 最后核对：2026-07-07，v0.26.23（暂缓中）。
