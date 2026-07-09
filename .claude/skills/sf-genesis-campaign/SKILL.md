---
name: sf-genesis-campaign
description: 历史战役——把策略选择移入 Genesis quick_phase（v0.26.28 已完成）。何时加载：被问“策略是否还在后台/首章有无方法论”时，或要继续方法论全链路修复时。
---

# Campaign：策略选择移入 quick_phase

> **状态（2026-07-09）：主目标已关闭。** 策略选择已在 v0.26.28 前移至 quick phase。
>
> 后续工作请改走：
> - 审计：`docs/audits/2026-07-09-methodology-in-genesis-audit.md`
> - 设计：`docs/plans/2026-07-09-methodology-in-genesis-remediation-design.md`
> - 计划：`docs/superpowers/plans/2026-07-09-methodology-in-genesis-remediation.md`

## 当前事实（必读）

- `quick_phase_steps()` ≈ 概念 →（题材画像确保）→ **策略选择** → 开篇骨架 → 撰写开篇。
- 首章经 `build_strategy_notes` + WriteTimeBundle 注入方法论。
- Background 模板须含 `{{strategy_section}}` / `{{quartet_section}}`（v0.26.46 修复外部化断链）。
- 创世结束按方法论推进 `methodology_step`（雪花→4，HDWB→2）。

## 历史成功标准（考古用，勿再执行迁移）

- S1 质量 / S2 延迟≤60s / S3 测试不回归——原用于「是否迁移策略」决策；迁移已完成。

## 何时 NOT 用本技能

- Genesis 第一章重复 → `sf-failure-archaeology`
- 通用架构 → `sf-architecture-contract`
- 方法论 background/步进 → 用上述 remediation 文档
