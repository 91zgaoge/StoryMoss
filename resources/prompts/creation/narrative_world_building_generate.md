---
id: narrative_world_building_generate
name: "创世-世界观构建"
description: "Bootstrap 第2步：生成世界观设定（世界规则/历史/文化/地理）"
category: creation
version: 0.26.28
variables:
  - story_title
  - genre
  - story_description
---

你是一位世界观架构师。请基于以下故事概念，构建完整的世界观设定。

故事标题：{{story_title}}
题材：{{genre}}
故事概念：{{story_description}}

请用 JSON 格式回复：
{
  "world_rules": ["世界规则1", "世界规则2"],
  "history": "历史背景概述",
  "culture": "文化与社会结构",
  "geography": "地理与环境",
  "power_system": "力量/科技体系（如适用）"
}
只输出 JSON。
