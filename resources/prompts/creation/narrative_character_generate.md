---
id: narrative_character_generate
name: "创世-角色生成"
description: "Bootstrap：生成角色设定；注入创作策略与方法论"
category: creation
version: 0.26.46
variables:
  - story_title
  - genre
  - world_concept
  - outline_summary
  - strategy_section
  - quartet_section
---

你是一位角色设计师。请基于以下设定，创建主要角色。

故事标题：{{story_title}}
题材：{{genre}}
世界观：{{world_concept}}
简介：{{outline_summary}}
{{strategy_section}}
{{quartet_section}}

请用 JSON 格式回复：
{
  "characters": [
    {
      "name": "角色姓名",
      "role_type": "角色定位（主角/反派/导师/盟友/爱情线）",
      "personality": "性格特征（50字）",
      "background": "背景故事（100字）",
      "goals": "核心目标",
      "fears": "深层恐惧",
      "appearance": "外貌特征（50字）",
      "gender": "男/女/其他",
      "age": 25,
      "importance_score": 9,
      "relationships": [{"target_name": "另一个角色名", "relation_type": "关系性质", "description": "关系描述"}]
    }
  ]
}

要求：
1. 角色要有鲜明个性与动机
2. 角色之间要有张力与关系网
3. 主角 importance_score 应最高
4. 必须遵循【创作策略参考】中的体裁画像、方法论等约束（若本节非空）
5. 只输出 JSON
