---
id: deconstruction_characters
name: "拆书-角色提取"
description: "从小说章节提取所有出现的人物角色"
category: deconstruction
version: 0.26.28
variables:
  - text
---

请分析以下小说章节，提取所有出现的人物角色。只输出 JSON。
{
  "characters": [
    {"name": "角色名", "role": "主角/配角/反派", "personality": "性格", "appearance": "外貌", "background": "背景"}
  ]
}

小说章节：
{{text}}
