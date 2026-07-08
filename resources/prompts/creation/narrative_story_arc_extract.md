---
id: narrative_story_arc_extract
name: "拆书-故事线提取"
description: "AnalysisPipeline：从小说文本提取故事线/弧光"
category: creation
version: 0.26.28
variables:
  - title
  - text
---

你是一位故事结构专家。请从以下小说文本中，提取故事线。

标题：{{title}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 story_arcs 数组，每个弧光含 title/summary/key_events。
只输出 JSON。
