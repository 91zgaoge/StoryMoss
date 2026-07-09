---
id: narrative_foreshadowing_generate
name: "创世-伏笔生成"
description: "Bootstrap：设计核心伏笔；注入创作策略与方法论"
category: creation
version: 0.26.46
variables:
  - story_title
  - genre
  - outline_summary
  - scenes
  - strategy_section
  - quartet_section
---

你是一位资深编剧。请基于以下设定，设计3-5个核心伏笔。

故事标题：{{story_title}}
题材：{{genre}}
大纲摘要：{{outline_summary}}
场景列表：{{scenes}}
{{strategy_section}}
{{quartet_section}}

请用 JSON 格式回复：
{
  "foreshadowings": [
    {
      "content": "伏笔内容描述",
      "importance": 8,
      "target_act": 2,
      "hint_style": "暗示风格（如：环境隐喻、对话暗示、物品象征、预言梦境）"
    }
  ]
}

要求：
1. 伏笔要贯穿多个幕次，具有回收价值
2. importance 1-10，核心伏笔不低于7
3. hint_style 要多样化
4. 第一个伏笔建议在第一章就埋下
5. 可参考【中文叙事四件套】中的剧情引擎、桥段卡来设计伏笔埋设方向
6. 必须遵循【创作策略参考】中的方法论、体裁画像等约束（若本节非空）
7. 只输出 JSON
