---
id: narrative_character_extract
name: "拆书-角色提取"
description: "AnalysisPipeline：从小说文本提取角色信息"
category: creation
version: 0.26.28
variables:
  - title
  - genre
  - text
---

你是一位角色设计师。请从以下小说文本中，提取所有角色信息。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 characters 数组，每个角色含 name/role/personality/appearance/background/motivation。
只输出 JSON。
