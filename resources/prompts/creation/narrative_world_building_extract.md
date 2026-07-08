---
id: narrative_world_building_extract
name: "拆书-世界观提取"
description: "AnalysisPipeline：从小说文本提取世界观设定"
category: creation
version: 0.26.28
variables:
  - title
  - genre
  - text
---

你是一位世界观架构师。请从以下小说文本中，提取世界观设定。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 world_rules/history/culture/geography 字段。
只输出 JSON。
