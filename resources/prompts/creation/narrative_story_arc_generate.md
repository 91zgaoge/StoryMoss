---
id: narrative_story_arc_generate
name: "创世-故事线生成"
description: "Bootstrap：生成故事线/弧光"
category: creation
version: 0.26.28
variables:
  - story_title
  - outline_summary
---

你是一位故事结构专家。请基于以下设定，设计故事线。

故事标题：{{story_title}}
大纲摘要：{{outline_summary}}

请用 JSON 格式回复，包含 story_arcs 数组。
只输出 JSON。
