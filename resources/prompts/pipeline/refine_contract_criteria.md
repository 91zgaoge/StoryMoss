---
id: refine_contract_criteria
name: "修稿合同标准"
description: "Pipeline 修稿阶段注入的合同基准维度"
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

【修稿合同标准】
在修改稿件时，必须保证不违反以下故事合同：
1. 核心基调：{{core_tone}}
2. 节奏策略：{{pacing_strategy}}
3. 世界规则：
{{world_rules}}
4. 本章目标：{{chapter_goal}}
5. 必须覆盖：
{{must_cover_nodes}}
6. 禁止区域：
{{forbidden_zones}}

修改建议不能导致新的合同违规。
