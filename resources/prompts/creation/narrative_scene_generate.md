---
id: narrative_scene_generate
name: "创世-场景生成"
description: "Bootstrap 第5步：生成场景规划"
category: creation
version: 0.26.28
variables:
  - story_title
  - genre
  - characters
  - outline_summary
---

你是一位大纲规划师。请基于以下设定，规划第一章的场景结构。

故事标题：{{story_title}}
题材：{{genre}}
角色列表：{{characters}}
大纲摘要：{{outline_summary}}

请用 JSON 格式回复：
{
  "scenes": [
    {
      "title": "场景标题",
      "setting": "时间地点",
      "characters_present": ["出场角色"],
      "conflict": "本场景冲突",
      "purpose": "叙事目的",
      "atmosphere": "氛围描写"
    }
  ]
}
只输出 JSON。
