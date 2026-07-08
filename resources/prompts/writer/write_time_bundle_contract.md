---
id: write_time_bundle_contract
name: "TimeSliced 合同约束"
description: "WriteTimeBundle 中追加的故事合同约束段"
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
{{#if core_tone}}- 基调：{{core_tone}}{{/if}}
{{#if pacing_strategy}}- 节奏：{{pacing_strategy}}{{/if}}
{{#if world_rules}}- 不可违反的世界规则：
{{world_rules}}{{/if}}
{{#if chapter_goal}}- 本章目标：{{chapter_goal}}{{/if}}
{{#if must_cover_nodes}}- 必须覆盖：
{{must_cover_nodes}}{{/if}}
{{#if forbidden_zones}}- 禁止区域：
{{forbidden_zones}}{{/if}}
