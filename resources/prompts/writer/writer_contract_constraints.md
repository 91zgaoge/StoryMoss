---
id: writer_contract_constraints
name: "Writer 故事合同约束"
description: "将 Story System 运行时合同注入 Writer prompt 作为创作约束"
category: writer
version: 0.26.28
variables:
  - core_tone
  - pacing_strategy
  - world_rules
  - chapter_goal
  - must_cover_nodes
  - forbidden_zones
---

【故事合同约束】
- 核心基调：{{core_tone}}
- 节奏策略：{{pacing_strategy}}
- 不可违反的世界规则：
{{world_rules}}

- 本章目标：{{chapter_goal}}
- 本章必须覆盖：
{{must_cover_nodes}}

- 本章禁止区域：
{{forbidden_zones}}

重要：续写内容必须遵守上述合同。如需打破规则，必须先在剧情中给出足够铺垫，并在末尾用【违背合同说明】解释原因。
