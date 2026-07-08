---
id: deconstruction_story_arc
name: "拆书-故事线提取"
description: "从多章节内容提取故事线和情节发展"
category: deconstruction
version: 0.26.28
variables:
  - text
---

请基于以下多章节内容，提取故事线和情节发展。只输出 JSON。
{
  "story_arcs": [
    {"title": "故事线标题", "start_chapter": 1, "summary": "故事线摘要", "resolution": "解决方式（未解决/已解决）"}
  ]
}

章节内容：
{{text}}
