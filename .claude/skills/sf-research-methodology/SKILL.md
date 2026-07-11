---
name: sf-research-methodology
description: 把直觉变成被接受结果的纪律（证据门槛、假设须先报数字、idea 生命周期、好想法来源）。何时加载：要做研究/实验、要提一个新机制/新阈值、要把实验 flag 转为默认或退役、或被问“怎么验证/怎么让这个想法被接受”时。
---

# StoryMoss 研究方法论

## 证据门槛（必须满足才叫“结果”）

1. **一个机制必须解释所有观察，包括反面**。若机制解释了 7 个案例但第 8 个反例不解释 → 机制错或不完整，不能发布。例：第一章重复 saga 早期“前端追加了两次”假说被 `append_text_check.occurrences=1` 这一反例推翻。
2. **经受指派的对抗性反驳**。主动构造“如果是这个机制，那么 X 应该也发生”的反驳实验，X 不发生则机制存疑。
3. **假设须先报数字再跑**（hypothesis-predicts-numbers-before-running）。例：分时介入 Phase 0 先预设“质量差 < 30% 阈值”再跑 A/B，得到 7.9% 才算验证。先跑再解释 = 拟合，不算。

## idea 生命周期（从实验 flag 到采纳或退役）

```
直觉 → 实验 flag（配置轴，见 sf-config-and-flags）
     → 小规模 A/B（预设阈值）
     → 达标 → 扩样本 + 对抗性反驳
            → 通过 → 走 sf-change-control 晋升为默认（更新 ARCHITECTURE/USER_GUIDE）
            → 不通过 → 退役并文档化（ROADMAP 标注“已验证不值得”，避免后人重提）
     → 未达阈值 → 退役并文档化
```

**关键**：退役也要文档化。v0.26.20 的 `wix.language: zh-CN` 尝试、v0.26.7–.14 的散布布尔守卫——都是被退役的方向，必须在 `ROADMAP`/`LESSONS_LEARNED` 留痕，否则后人重提。

## 好想法的历史来源（本项目实证）

1. **`creative_workflow.log` 时间线对照**：把卡住时刻对齐日志阶段标记 → 卡点 = 断点。v0.26.23 续写卡死、v0.23.18 600s 超时都源于此。
2. **日志反例推翻直觉**：v0.26.14 日志证明“第一章重复”不是前端追加两次，而是 LLM 正文自身首尾回环 → 转向生成侧闸门。
3. **A/B 盲测预设阈值**：v0.13.0 分时介入架构成立的关键证据。
4. **纯函数提取 + 契约测试**：竞态路径提取纯函数（`select_first_chapter_content`/`world_concept_for_character_prompt`）使竞态可测。
5. **跨层 golden 双跑**：算法漂移用共享 fixture 锁定。

## 何时 NOT 用本技能

- 具体分析配方（A/B、阈值推导、状态机证明）→ `sf-proof-and-analysis-toolkit`。
- 开放前沿题目 → `sf-research-frontier`。
- 已修复战役 → `sf-failure-archaeology`。

## 出处与维护

- 重验证命令：
  - `rg -n 'Phase 0|A/B|盲测|7\.9%|30%' docs/ ARCHITECTURE.md README.md | head`
  - `ls docs/plans/`（设计文档与对照报告习惯位置）
  - `ls docs/archive/LESSONS_LEARNED.md docs/archive/AGENTS_HISTORY.md`
- 易漂移项：方法论条目随每次战役演进。
- 最后核对：2026-07-07，v0.26.23。
