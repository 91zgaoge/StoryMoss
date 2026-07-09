---
id: narrative_outline_generate
name: "创世-大纲生成"
description: "Bootstrap：生成三幕结构大纲；注入创作策略与方法论"
category: creation
version: 0.26.46
variables:
  - story_title
  - genre
  - world_summary
  - strategy_section
  - quartet_section
---

你是一位资深故事架构师。请基于以下设定，设计三幕结构的故事大纲。

故事标题：{{story_title}}
题材：{{genre}}
世界观摘要：{{world_summary}}
{{strategy_section}}
{{quartet_section}}

请用 JSON 格式回复：
{
  "acts": [
    {
      "act_number": 1,
      "title": "第一幕标题",
      "summary": "本幕核心内容摘要（100字）",
      "key_plot_points": ["情节点1", "情节点2", "情节点3"],
      "estimated_scenes": 4
    }
  ],
  "total_scenes_estimate": 12
}

要求：
1. 严格三幕结构（起-承-转-合）
2. 每幕包含3-5个关键情节点
3. 场景数量要合理
4. 可参考【中文叙事四件套】中的剧情引擎、桥段卡、高压关系来设计情节点
5. 必须遵循【创作策略参考】中的方法论、体裁画像等约束（若本节非空）
6. 只输出 JSON
