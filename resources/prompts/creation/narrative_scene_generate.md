---
id: narrative_scene_generate
name: "创世-场景生成"
description: "Bootstrap：生成场景规划；注入创作策略与方法论"
category: creation
version: 0.26.46
variables:
  - story_title
  - genre
  - characters
  - outline_summary
  - strategy_section
  - quartet_section
---

你是一位大纲规划师。请基于以下设定，规划核心场景结构。

故事标题：{{story_title}}
题材：{{genre}}
角色列表：{{characters}}
大纲摘要：{{outline_summary}}
{{strategy_section}}
{{quartet_section}}

请用 JSON 格式回复：
{
  "scenes": [
    {
      "sequence_number": 1,
      "title": "场景标题",
      "summary": "场景内容摘要（100字）",
      "dramatic_goal": "本场景的戏剧目标",
      "external_pressure": "外部压力/阻碍",
      "conflict_type": "man_vs_man|man_vs_self|man_vs_society|man_vs_nature|man_vs_technology|man_vs_fate|man_vs_supernatural|man_vs_time|man_vs_morality|man_vs_identity|faction_vs_faction",
      "setting_location": "地点",
      "setting_time": "时间",
      "characters_present": ["角色名1", "角色名2"]
    }
  ]
}

要求：
1. 场景之间要有因果关系
2. 每个场景都要推动情节或揭示人物
3. 必须遵循【创作策略参考】中的场景结构方法论、体裁画像等约束（若本节非空）
4. 只输出 JSON
