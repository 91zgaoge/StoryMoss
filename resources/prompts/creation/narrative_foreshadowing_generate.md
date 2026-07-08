---
id: narrative_foreshadowing_generate
name: "创世-伏笔生成"
description: "Bootstrap 第6步：识别并埋设伏笔"
category: creation
version: 0.26.28
variables:
  - story_title
  - genre
  - outline_summary
  - scenes
---

你是一位资深编剧。请基于以下设定，设计3-5个核心伏笔。

故事标题：{{story_title}}
题材：{{genre}}
大纲摘要：{{outline_summary}}
场景列表：{{scenes}}

请用 JSON 格式回复：
{
  "foreshadowings": [
    {
      "description": "伏笔描述",
      "setup_scene": "埋设场景",
      "payoff_hint": "回收提示",
      "importance": "high/medium/low"
    }
  ]
}
只输出 JSON。
