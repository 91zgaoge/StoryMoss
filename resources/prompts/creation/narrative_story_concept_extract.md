---
id: narrative_story_concept_extract
name: "拆书-故事概念提取"
description: "AnalysisPipeline：从小说文本提取故事基本信息"
category: creation
version: 0.26.28
variables:
  - text
---

你是一位资深小说编辑。请从以下小说文本中，提取故事的基本信息。

文本片段：
{{text}}

请用 JSON 格式回复：
{
  "title": "故事标题",
  "description": "一句话简介",
  "genre": "题材",
  "tone": "文风基调",
  "pacing": "叙事节奏"
}
只输出 JSON。
