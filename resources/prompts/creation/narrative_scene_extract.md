---
id: narrative_scene_extract
name: "拆书-场景提取"
description: "AnalysisPipeline：从小说文本提取场景信息"
category: creation
version: 0.26.28
variables:
  - title
  - genre
  - text
---

你是一位大纲规划师。请从以下小说文本中，提取场景结构。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 scenes 数组。
只输出 JSON。
