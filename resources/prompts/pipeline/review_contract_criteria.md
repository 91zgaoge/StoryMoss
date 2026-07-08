---
id: review_contract_criteria
name: "审稿合同标准"
description: "Pipeline 审稿阶段注入的合同基准维度"
category: pipeline
version: 0.26.28
variables:
  - core_tone
  - pacing_strategy
  - world_rules
  - chapter_goal
  - must_cover_nodes
  - forbidden_zones
---

【审稿合同标准】
以故事合同为基准，判断稿件是否存在以下问题：
1. 设定冲突（违反世界规则）
2. 节奏偏离（违背节奏策略）
3. 基调不一致
4. 情节点遗漏（未覆盖必须节点）
5. 反套路/禁止区域触碰

合同信息：
- 核心基调：{{core_tone}}
- 节奏策略：{{pacing_strategy}}
- 世界规则：
{{world_rules}}
- 本章目标：{{chapter_goal}}
- 必须覆盖：
{{must_cover_nodes}}
- 禁止区域：
{{forbidden_zones}}
