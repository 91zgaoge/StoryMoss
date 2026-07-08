---
id: deconstruction_world_building
name: "拆书-世界观提取"
description: "从小说章节提取世界观设定"
category: deconstruction
version: 0.26.28
variables:
  - text
---

请分析以下小说章节，提取世界观设定。只输出 JSON。
{
  "world_rules": ["规则1"],
  "history": "历史背景",
  "culture": "文化设定",
  "geography": "地理设定"
}

小说章节：
{{text}}
