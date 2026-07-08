---
id: narrative_foreshadowing_extract
name: "拆书-伏笔提取"
description: "AnalysisPipeline：从小说文本识别伏笔"
category: creation
version: 0.26.28
variables:
  - title
  - genre
  - text
---

你是一位资深编剧。请从以下小说文本中，识别已有的伏笔。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 foreshadowings 数组。
只输出 JSON。
