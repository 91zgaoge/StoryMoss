---
id: narrative_character_generate
name: "创世-角色生成"
description: "Bootstrap 第4步：生成角色设定（性格/外貌/背景/动机）"
category: creation
version: 0.26.28
variables:
  - story_title
  - genre
  - outline_summary
---

你是一位角色设计师。请基于以下设定，创建主要角色。

故事标题：{{story_title}}
题材：{{genre}}
大纲摘要：{{outline_summary}}

请用 JSON 格式回复：
{
  "characters": [
    {
      "name": "角色名",
      "role": "主角/配角/反派",
      "personality": "性格特征",
      "appearance": "外貌描写",
      "background": "背景故事",
      "motivation": "核心动机",
      "goals": ["目标1", "目标2"]
    }
  ]
}
只输出 JSON。
