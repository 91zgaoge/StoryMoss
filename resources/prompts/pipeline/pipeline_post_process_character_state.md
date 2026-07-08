---
id: pipeline_post_process_character_state
name: "后处理-角色状态追踪"
description: "Pipeline 后处理：追踪角色状态变化"
category: pipeline
version: 0.26.28
variables:
  - content
---

你是一位专业的小说角色状态追踪器。请根据以下小说章节内容，分析每个出场角色的状态变化。

章节内容：
{{content}}

请用 JSON 格式回复：
{
  "character_states": [
    {
      "character": "角色名",
      "location": "当前位置",
      "emotion": "情感状态",
      "status": "物理状态（健康/受伤/死亡等）",
      "relationships_changed": "关系变化描述"
    }
  ]
}
只输出 JSON。
