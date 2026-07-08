---
id: pipeline_post_process_plot
name: "后处理-剧情要点提取"
description: "Pipeline 后处理：从章节内容提取剧情要点"
category: pipeline
version: 0.26.28
variables:
  - content
---

请基于以下场景正文，提取故事的核心要素。只提取正文中明确出现或强烈暗示的信息，不要推测。

场景正文：
{{content}}

请用 JSON 格式回复：
{
  "key_events": ["关键事件1", "关键事件2"],
  "character_changes": [{"character": "角色名", "change": "状态变化"}],
  "new_information": ["新信息1"]
}
只输出 JSON。
