---
id: deconstruction_chapter_summary
name: "拆书-章节总结"
description: "总结小说章节的情节要点"
category: deconstruction
version: 0.26.28
variables:
  - text
---

请总结以下小说章节的情节要点。只输出 JSON。
{
  "chapter_title": "章节标题（如有）",
  "summary": "章节摘要（100-200字）",
  "key_events": ["关键事件1", "关键事件2"],
  "cliffhanger": "章末悬念（如有）"
}

小说章节：
{{text}}
