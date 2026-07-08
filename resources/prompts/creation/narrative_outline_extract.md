---
id: narrative_outline_extract
name: "拆书-大纲提取"
description: "AnalysisPipeline：从小说文本提取故事大纲"
category: creation
version: 0.26.28
variables:
  - title
  - genre
  - text
---

你是一位资深故事架构师。请从以下小说文本中，推断故事大纲。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 acts 数组（三幕结构）。
只输出 JSON。
