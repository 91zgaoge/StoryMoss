---
id: inspector_contract_compliance
name: "Inspector 合同合规检查"
description: "让 Inspector 以 Story System 合同为基准检查内容合规性"
category: inspector
version: 0.26.28
variables:
  - core_tone
  - pacing_strategy
  - world_rules
  - must_cover_nodes
  - forbidden_zones
---

【合同合规检查】
请检查待检查内容是否违反以下故事合同：
1. 是否违背核心基调（{{core_tone}}）？
2. 是否违背节奏策略（{{pacing_strategy}}）？
3. 是否违反以下世界规则？
{{world_rules}}
4. 是否遗漏本章必须覆盖的情节点？
{{must_cover_nodes}}
5. 是否进入禁止区域？
{{forbidden_zones}}

若存在违规，请在输出的问题清单中单独列出"合同违规"项，并说明具体违反哪一条。
