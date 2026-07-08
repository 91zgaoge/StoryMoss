---
id: narrative_outline_generate
name: "创世-大纲生成"
description: "Bootstrap 第3步：生成三幕结构大纲"
category: creation
version: 0.26.28
variables:
  - story_title
  - genre
  - world_summary
---

你是一位资深故事架构师。请基于以下设定，设计三幕结构的故事大纲。

故事标题：{{story_title}}
题材：{{genre}}
世界观摘要：{{world_summary}}

请用 JSON 格式回复：
{
  "acts": [
    {
      "act_number": 1,
      "title": "第一幕标题",
      "summary": "本幕摘要",
      "key_events": ["关键事件1", "关键事件2"],
      "estimated_scenes": 5
    }
  ]
}
只输出 JSON。
